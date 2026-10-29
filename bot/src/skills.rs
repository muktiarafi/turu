//! The skill timer and skill providers (VibeSkua `ScriptSkill`, `AdvancedSkillProvider`,
//! `AdvancedSkillCommand`, `AdvancedSkillContainer`), plus the background-thread helper the
//! drop, quest and skill loops share.
//!
//! Skill strings use the Advanced Skills syntax: `"1 | 2 | 3 | 4 H<50"`. Each entry is a
//! slot digit (0 auto attack .. 5 potion) followed by rules, case-insensitive:
//! `H<50` / `H>50#` health (% unless `#`), `M<..` mana, `PH<..` a party member's health,
//! `A>"Aura" 3 TARGET` an aura's stacks (self unless the word contains "target"),
//! `MA<"A" 1 & MA>"B" 2` several auras (`&` and, `:` or), `WW500` wait 500 ms, and a
//! trailing `S` skips the skill (takes the next one) when a rule fails.

use crate::Bot;
use crate::api::{inventory, player};
use crate::control::mark_script_thread;
use crate::error::{BotError, BotResult};
use crate::models::{Monster, Skill};
use parking_lot::Mutex;
use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::Value;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------
// Background threads
// ---------------------------------------------------------------------------------------

struct WorkerHandle {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

/// One background loop owned by the running script (VibeSkua's "Drops" timer, "Quest
/// Thread" and "Skill Timer"). The thread is marked as a script thread, so it honours
/// Stop and Pause; [`Worker::stop`] ends it between Flash calls.
#[derive(Default)]
pub(crate) struct Worker {
    slot: Mutex<Option<WorkerHandle>>,
}

impl Worker {
    /// The loop runs and wasn't asked to stop.
    pub(crate) fn running(&self) -> bool {
        self.slot
            .lock()
            .as_ref()
            .is_some_and(|h| !h.cancel.load(Ordering::Acquire) && !h.thread.is_finished())
    }

    /// Starts `body` on a new thread unless the loop already runs. `body` must return soon
    /// after its cancel flag is set (sleep with [`nap`]).
    pub(crate) fn start(
        &self,
        name: &str,
        body: impl FnOnce(&AtomicBool) + Send + 'static,
    ) -> bool {
        let mut slot = self.slot.lock();
        if slot
            .as_ref()
            .is_some_and(|h| !h.cancel.load(Ordering::Acquire) && !h.thread.is_finished())
        {
            return false;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let spawned = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                mark_script_thread(true);
                body(&flag);
                mark_script_thread(false);
            });
        match spawned {
            Ok(thread) => {
                *slot = Some(WorkerHandle { cancel, thread });
                true
            }
            Err(e) => {
                tracing::warn!("Couldn't start the {name} thread: {e}");
                false
            }
        }
    }

    /// Asks the loop to end without waiting (safe from event handlers).
    pub(crate) fn cancel(&self) {
        if let Some(h) = self.slot.lock().as_ref() {
            h.cancel.store(true, Ordering::Release);
        }
    }

    /// Asks the loop to end and waits up to `wait` for it.
    pub(crate) fn stop(&self, wait: Duration) {
        self.cancel();
        let deadline = Instant::now() + wait;
        loop {
            let done = self
                .slot
                .lock()
                .as_ref()
                .is_none_or(|h| h.thread.is_finished());
            if done || Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Sleeps `ms` in short slices. `Ok(false)` when `cancel` was set; on a script thread, fails
/// with [`BotError::Stopped`] when the script stops and blocks while it's paused.
pub(crate) fn nap(bot: &Bot, ms: u64, cancel: &AtomicBool) -> BotResult<bool> {
    let deadline = Instant::now() + Duration::from_millis(ms);
    loop {
        if cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(true);
        }
        let slice = (deadline - now).min(Duration::from_millis(50));
        bot.sleep(slice.as_millis().max(1) as u64)?;
    }
}

// ---------------------------------------------------------------------------------------
// Modes
// ---------------------------------------------------------------------------------------

/// VibeSkua `SkillUseMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillUseMode {
    /// Use a skill only when it's off cooldown; otherwise move on.
    #[default]
    UseIfAvailable,
    /// Wait (up to the skill timeout) for the skill's cooldown, then use it.
    WaitForCooldown,
}

impl SkillUseMode {
    /// Case-insensitive; anything else is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("UseIfAvailable") {
            Some(Self::UseIfAvailable)
        } else if text.eq_ignore_ascii_case("WaitForCooldown") {
            Some(Self::WaitForCooldown)
        } else {
            None
        }
    }
}

/// VibeSkua `ClassUseMode` names: the modes a class can have saved skill sets for.
pub const CLASS_USE_MODES: [&str; 8] = [
    "Base", "Atk", "Def", "Farm", "Solo", "Supp", "Dodge", "Ultra",
];

/// VibeSkua `genericSkills`: used when a class has no saved set.
pub const GENERIC_SKILLS: &str = "0 | 1 | 2 | 3 | 4";

// ---------------------------------------------------------------------------------------
// Skill string parsing (AdvancedSkillProvider.Load / ParseUseRule)
// ---------------------------------------------------------------------------------------

/// One aura condition: `stacks >= value` (`greater`) or `stacks <= value`.
#[derive(Debug, Clone, PartialEq)]
pub struct AuraCheck {
    /// Lowercase (skill strings are lowercased before parsing); compared case-insensitively.
    pub name: String,
    pub stacks: f32,
    pub greater: bool,
    /// On the target instead of yourself.
    pub target: bool,
}

/// VibeSkua `SkillRule` with its values.
#[derive(Debug, Clone, PartialEq)]
pub enum SkillRule {
    None,
    Health {
        greater: bool,
        value: i64,
        percent: bool,
    },
    Mana {
        greater: bool,
        value: i64,
        percent: bool,
    },
    /// Any player in your cell (`world.players`) matches.
    PartyHealth {
        greater: bool,
        value: i64,
        percent: bool,
    },
    Aura(AuraCheck),
    /// All (`or == false`) or any of the checks.
    MultiAura {
        checks: Vec<AuraCheck>,
        or: bool,
    },
    /// Sleeps `ms` before using the skill.
    Wait {
        ms: i64,
    },
}

/// VibeSkua `UseRule`: a rule and whether a failure skips to the next skill.
#[derive(Debug, Clone, PartialEq)]
pub struct UseRule {
    pub rule: SkillRule,
    pub skip: bool,
}

impl UseRule {
    fn none() -> Self {
        Self {
            rule: SkillRule::None,
            skip: false,
        }
    }
}

/// One entry of a skill sequence: the slot and its rules.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillCommand {
    pub skill: i64,
    pub rules: Vec<UseRule>,
}

/// VibeSkua `AdvancedSkillProvider.Load`: lowercases, splits on `|` and parses each entry
/// that starts with a digit.
pub fn parse_skills(text: &str) -> Vec<SkillCommand> {
    let lower = text.to_lowercase();
    lower
        .split('|')
        .filter_map(|command| {
            let trimmed = command.trim();
            let first = trimmed.chars().next()?;
            let skill = first.to_digit(10)? as i64;
            let rest = &trimmed[first.len_utf8()..];
            let rules = if rest.is_empty() {
                vec![UseRule::none()]
            } else {
                parse_use_rule(rest)
            };
            Some(SkillCommand { skill, rules })
        })
        .collect()
}

struct Cursor {
    c: Vec<char>,
    pos: usize,
}

impl Cursor {
    fn peek(&self) -> Option<char> {
        self.c.get(self.pos).copied()
    }

    fn at(&self, offset: usize) -> Option<char> {
        self.c.get(self.pos + offset).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek() == Some(' ') {
            self.pos += 1;
        }
    }

    /// `>` is greater (consumed), `<` is consumed, anything else is "less".
    fn comparison(&mut self) -> bool {
        match self.peek() {
            Some('>') => {
                self.pos += 1;
                true
            }
            Some('<') => {
                self.pos += 1;
                false
            }
            _ => false,
        }
    }

    fn take_while(&mut self, f: impl Fn(char) -> bool) -> String {
        let start = self.pos;
        while self.peek().is_some_and(&f) {
            self.pos += 1;
        }
        self.c[start..self.pos].iter().collect()
    }

    /// `"name"` with `\"` escapes, trimmed.
    fn quoted(&mut self) -> String {
        self.pos += 1;
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == '"' {
                break;
            }
            if ch == '\\' && self.at(1) == Some('"') {
                self.pos += 2;
            } else {
                self.pos += 1;
            }
        }
        let raw: String = self.c[start..self.pos.min(self.c.len())].iter().collect();
        if self.peek() == Some('"') {
            self.pos += 1;
        }
        raw.replace("\\\"", "\"").trim().to_owned()
    }

    /// Digits and dots as a float (0 when missing or malformed).
    fn float(&mut self) -> f32 {
        let text = self.take_while(|ch| ch.is_ascii_digit() || ch == '.');
        text.parse().unwrap_or(0.0)
    }

    /// The aura target word: "target" if it contains it, else self.
    fn target(&mut self) -> bool {
        if self.peek().is_some_and(char::is_alphabetic) {
            let word = self.take_while(char::is_alphabetic);
            word.to_uppercase().contains("TARGET")
        } else {
            false
        }
    }

    /// `<n>` then an optional `#` (flat) or `%`; `None` without digits.
    fn amount(&mut self) -> Option<(i64, bool)> {
        let digits = self.take_while(|ch| ch.is_ascii_digit());
        if digits.is_empty() {
            return None;
        }
        let value = digits.parse().unwrap_or(i64::MAX);
        let mut percent = true;
        match self.peek() {
            Some('#') => {
                percent = false;
                self.pos += 1;
            }
            Some('%') => self.pos += 1,
            _ => {}
        }
        Some((value, percent))
    }
}

/// VibeSkua `AdvancedSkillProvider.ParseUseRule`: the rules after the slot digit.
pub fn parse_use_rule(text: &str) -> Vec<UseRule> {
    if text.trim().is_empty() {
        return vec![UseRule::none()];
    }
    let mut cur = Cursor {
        c: text.chars().collect(),
        pos: 0,
    };
    let skip = cur.c.last() == Some(&'s');
    let mut rules = Vec::new();
    let mut multi = Vec::new();
    let mut multi_or = false;
    let push = |rules: &mut Vec<UseRule>, rule| rules.push(UseRule { rule, skip });

    while let Some(ch) = cur.peek() {
        if ch.is_whitespace() {
            cur.pos += 1;
            continue;
        }
        if ch == 'm' && cur.at(1) == Some('a') {
            cur.pos += 2;
            let greater = cur.comparison();
            let name = if cur.peek() == Some('"') {
                cur.quoted()
            } else {
                String::new()
            };
            cur.skip_spaces();
            let stacks = cur.float();
            cur.skip_spaces();
            let target = cur.target();
            cur.skip_spaces();
            if let Some(op @ ('&' | ':')) = cur.peek() {
                multi_or = op == ':';
                cur.pos += 1;
            }
            if !name.is_empty() {
                multi.push(AuraCheck {
                    name,
                    stacks,
                    greater,
                    target,
                });
            }
            continue;
        }
        if matches!(ch, 'p' | 'P') && matches!(cur.at(1), Some('h' | 'H')) {
            cur.pos += 2;
            let greater = cur.comparison();
            if let Some((value, percent)) = cur.amount() {
                push(
                    &mut rules,
                    SkillRule::PartyHealth {
                        greater,
                        value,
                        percent,
                    },
                );
            }
            continue;
        }
        match ch {
            'h' | 'H' | 'm' | 'M' => {
                cur.pos += 1;
                let greater = cur.comparison();
                if let Some((value, percent)) = cur.amount() {
                    let rule = if matches!(ch, 'h' | 'H') {
                        SkillRule::Health {
                            greater,
                            value,
                            percent,
                        }
                    } else {
                        SkillRule::Mana {
                            greater,
                            value,
                            percent,
                        }
                    };
                    push(&mut rules, rule);
                }
            }
            'a' | 'A' => {
                cur.pos += 1;
                let greater = cur.comparison();
                let name = if cur.peek() == Some('"') {
                    cur.quoted()
                } else {
                    cur.take_while(|c| c != ' ' && !c.is_ascii_digit() && c != '.')
                };
                cur.skip_spaces();
                let stacks = cur.float();
                cur.skip_spaces();
                let target = cur.target();
                if !name.is_empty() {
                    push(
                        &mut rules,
                        SkillRule::Aura(AuraCheck {
                            name,
                            stacks,
                            greater,
                            target,
                        }),
                    );
                }
            }
            'w' | 'W' => {
                cur.pos += 1;
                if cur.peek() == Some('w') {
                    cur.pos += 1;
                }
                let digits = cur.take_while(|c| c.is_ascii_digit());
                if !digits.is_empty() {
                    push(
                        &mut rules,
                        SkillRule::Wait {
                            ms: digits.parse().unwrap_or(i64::MAX),
                        },
                    );
                }
            }
            _ => cur.pos += 1,
        }
    }
    if !multi.is_empty() {
        push(
            &mut rules,
            SkillRule::MultiAura {
                checks: multi,
                or: multi_or,
            },
        );
    }
    if rules.is_empty() {
        rules.push(UseRule::none());
    }
    rules
}

// ---------------------------------------------------------------------------------------
// AdvancedSkills.json (AdvancedSkillContainer.LoadFromJson / ConvertSkillsToString)
// ---------------------------------------------------------------------------------------

/// A saved skill set (VibeSkua `AdvancedSkill`): one class in one mode.
#[derive(Debug, Clone, PartialEq)]
pub struct AdvancedSkill {
    pub class_name: String,
    /// "Base", "Atk", ... (see [`CLASS_USE_MODES`]); kept as written in the file.
    pub mode: String,
    /// The sequence in skill-string syntax.
    pub skills: String,
    pub skill_timeout: i64,
    pub skill_use_mode: SkillUseMode,
    pub reset_on_target: bool,
}

/// A JSON object read in file order.
struct Ordered<T>(Vec<(String, T)>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Ordered<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for V<T> {
            type Value = Ordered<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Ordered(entries))
            }
        }
        d.deserialize_map(V(PhantomData))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillModeJson {
    #[serde(default = "default_use_mode")]
    skill_use_mode: String,
    #[serde(default = "default_timeout")]
    skill_timeout: i64,
    #[serde(default)]
    reset_combo_on_target_change: bool,
    #[serde(default)]
    skills: Vec<SkillJson>,
}

fn default_use_mode() -> String {
    "UseIfAvailable".into()
}

fn default_timeout() -> i64 {
    100
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillJson {
    #[serde(default)]
    skill_id: i64,
    #[serde(default)]
    rules: Option<Vec<RuleJson>>,
    #[serde(default)]
    skip_on_match: bool,
    #[serde(default)]
    is_multi_aura: bool,
    #[serde(default)]
    multi_aura_operator: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuleJson {
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    value: Option<f32>,
    #[serde(default)]
    comparison: Option<String>,
    #[serde(default)]
    aura_name: Option<String>,
    #[serde(default)]
    aura_target: Option<String>,
    #[serde(default)]
    timeout: Option<i64>,
    #[serde(default)]
    is_percentage: Option<bool>,
}

/// `{value}` as C# prints a nullable float (nothing for null).
fn number(value: Option<f32>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

fn aura_text(prefix: &str, rule: &RuleJson, suffix: &str) -> String {
    format!(
        "{prefix}{}\"{}\" {}{}{suffix}",
        if rule.comparison.as_deref() == Some("greater") {
            ">"
        } else {
            "<"
        },
        rule.aura_name.as_deref().unwrap_or_default(),
        number(rule.value),
        if rule.aura_target.as_deref() == Some("target") {
            " TARGET"
        } else {
            ""
        },
    )
}

/// VibeSkua `ConvertRulesToString`.
fn rules_to_string(rules: &[RuleJson], is_multi_aura: bool, operator: Option<&str>) -> String {
    let mut parts = Vec::new();
    let multi: Vec<&RuleJson> = rules
        .iter()
        .filter(|r| is_multi_aura && r.kind == "MultiAura")
        .collect();
    let single: Vec<&RuleJson> = rules
        .iter()
        .filter(|r| !is_multi_aura && r.kind == "Aura")
        .collect();
    for rule in rules
        .iter()
        .filter(|r| !matches!(r.kind.as_str(), "MultiAura" | "Aura" | "Skip"))
    {
        let percent = if rule.is_percentage.unwrap_or(true) {
            "%"
        } else {
            "#"
        };
        let cmp = if rule.comparison.as_deref() == Some("greater") {
            ">"
        } else {
            "<"
        };
        match rule.kind.as_str() {
            "Health" => parts.push(format!("H{cmp}{}{percent}", number(rule.value))),
            "Mana" => parts.push(format!("M{cmp}{}{percent}", number(rule.value))),
            "PartyHealth" => parts.push(format!("PH{cmp}{}{percent}", number(rule.value))),
            "Wait" => parts.push(format!(
                "WW{}",
                rule.timeout.map(|t| t.to_string()).unwrap_or_default()
            )),
            _ => {}
        }
    }
    if is_multi_aura && !multi.is_empty() {
        let op = if operator == Some("OR") { ":" } else { "&" };
        for (i, rule) in multi.iter().enumerate() {
            parts.push(aura_text(
                "MA",
                rule,
                if i + 1 < multi.len() { op } else { "" },
            ));
        }
    } else if single.len() > 1 {
        for (i, rule) in single.iter().enumerate() {
            parts.push(aura_text(
                "MA",
                rule,
                if i + 1 < single.len() { "&" } else { "" },
            ));
        }
    } else if let [rule] = single.as_slice() {
        parts.push(aura_text("A", rule, ""));
    }
    parts.join(" ")
}

/// VibeSkua `ConvertSkillsToString`.
fn skills_to_string(skills: &[SkillJson]) -> String {
    skills
        .iter()
        .map(|skill| {
            let mut text = skill.skill_id.to_string();
            if let Some(rules) = skill.rules.as_ref().filter(|r| !r.is_empty()) {
                text.push(' ');
                text.push_str(&rules_to_string(
                    rules,
                    skill.is_multi_aura,
                    skill.multi_aura_operator.as_deref(),
                ));
            }
            if skill.skip_on_match {
                text.push('S');
            }
            text
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Parses `AdvancedSkills.json` (`{ "<Class>": { "<Mode>": { skillUseMode, skillTimeout,
/// resetComboOnTargetChange, skills: [{ skillId, rules: [...] }] } } }`) into skill sets in
/// file order. A malformed file gives no sets (VibeSkua never publishes a partial set).
pub fn parse_advanced_skills(json: &str) -> Vec<AdvancedSkill> {
    let Ok(Ordered(classes)) = serde_json::from_str::<Ordered<Ordered<SkillModeJson>>>(json) else {
        return Vec::new();
    };
    classes
        .into_iter()
        .flat_map(|(class_name, Ordered(modes))| {
            modes.into_iter().map(move |(mode, set)| AdvancedSkill {
                class_name: class_name.clone(),
                mode,
                skills: skills_to_string(&set.skills),
                skill_timeout: set.skill_timeout,
                skill_use_mode: if set.skill_use_mode == "UseIfAvailable" {
                    SkillUseMode::UseIfAvailable
                } else {
                    SkillUseMode::WaitForCooldown
                },
                reset_on_target: set.reset_combo_on_target_change,
            })
        })
        .collect()
}

/// The user's skill sets (`AdvancedSkills.json` in the data directory, else the shipped one).
pub fn load_advanced_skills(paths: &crate::paths::Paths) -> Vec<AdvancedSkill> {
    parse_advanced_skills(&paths.load_advanced_skills())
}

/// VibeSkua `LoadAdvanced(className, autoEquip, mode)`'s choice: the class's set in `mode`,
/// else its first set; `None` when the class has none.
pub fn find_skill_set<'a>(
    sets: &'a [AdvancedSkill],
    class_name: &str,
    mode: &str,
) -> Option<&'a AdvancedSkill> {
    let class_name = class_name.trim();
    let mut of_class = sets
        .iter()
        .filter(|s| s.class_name.trim().eq_ignore_ascii_case(class_name));
    let first = of_class.clone().next()?;
    Some(
        of_class
            .find(|s| s.mode.eq_ignore_ascii_case(mode.trim()))
            .unwrap_or(first),
    )
}

// ---------------------------------------------------------------------------------------
// The provider (AdvancedSkillProvider + AdvancedSkillCommand)
// ---------------------------------------------------------------------------------------

/// The skill sequence being played and where it is.
#[derive(Debug, Clone)]
pub struct SkillProvider {
    commands: Vec<SkillCommand>,
    index: usize,
    /// Restart the sequence when the target changes (VibeSkua `ResetOnTarget`).
    pub reset_on_target: bool,
    last_target_key: Option<String>,
}

impl SkillProvider {
    pub fn new(skills: &str) -> Self {
        Self {
            commands: parse_skills(skills),
            index: 0,
            reset_on_target: false,
            last_target_key: None,
        }
    }

    pub fn commands(&self) -> &[SkillCommand] {
        &self.commands
    }

    /// VibeSkua `GetNextSkill`: `(sequence index, slot)` and advances; `None` when empty.
    ///
    /// (VibeSkua also reads `world.actions.active[i].isOK` here to hold the sequence during
    /// the global cooldown, but that path never resolves in skua.swf, so it always advances.)
    pub fn next_skill(&mut self) -> Option<(usize, i64)> {
        let command = self.commands.get(self.index)?;
        let result = (self.index, command.skill);
        self.index += 1;
        if self.index >= self.commands.len() {
            self.index = 0;
        }
        Some(result)
    }

    /// Back to the first skill.
    pub fn reset(&mut self) {
        self.index = 0;
    }

    fn on_player_death(&mut self) {
        self.last_target_key = None;
        self.reset();
    }

    /// VibeSkua `OnTargetReset`: with `reset_on_target`, restarts the sequence when the
    /// target changes or is lost.
    fn on_target_reset(&mut self, bot: &Bot) -> BotResult<bool> {
        if !self.reset_on_target {
            return Ok(false);
        }
        let key = if player::has_target(bot)? {
            player::target(bot)?
                .map(|t: Monster| format!("{}:{}:{}:{}", t.map_id, t.id, t.cell, t.name))
        } else {
            None
        };
        let changed = match (&self.last_target_key, &key) {
            (Some(_), None) => true,
            (Some(last), Some(now)) => last != now,
            (None, _) => false,
        };
        self.last_target_key = key;
        if changed {
            self.reset();
        }
        Ok(changed)
    }

    /// VibeSkua `ShouldUse`: `Some(true)` use it, `Some(false)` don't, `None` skip to the
    /// next skill.
    fn should_use(&self, bot: &Bot, index: usize, can_use: bool) -> BotResult<Option<bool>> {
        let Some(command) = self.commands.get(index) else {
            return Ok(Some(true));
        };
        if command
            .rules
            .first()
            .is_none_or(|r| r.rule == SkillRule::None)
        {
            return Ok(Some(true));
        }
        let leaf = if command
            .rules
            .iter()
            .any(|r| matches!(r.rule, SkillRule::Health { .. } | SkillRule::Mana { .. }))
        {
            match bot.flash.get_opt::<Value>("world.myAvatar.dataLeaf") {
                Ok(v) => v,
                Err(BotError::Stopped) => return Err(BotError::Stopped),
                Err(_) => None,
            }
        } else {
            None
        };
        let mut should = true;
        for rule in &command.rules {
            if !player::alive(bot)? {
                return Ok(Some(false));
            }
            match &rule.rule {
                SkillRule::Health {
                    greater,
                    value,
                    percent,
                } => {
                    let (now, max) = match &leaf {
                        Some(l) => (field(l, "intHP"), field(l, "intHPMax")),
                        None => (player::health(bot)?, player::max_health(bot)?),
                    };
                    should = now != 0 && compare(now, max, *greater, *value, *percent);
                }
                SkillRule::Mana {
                    greater,
                    value,
                    percent,
                } => {
                    let (now, max) = match &leaf {
                        Some(l) => (field(l, "intMP"), field(l, "intMPMax")),
                        None => (player::mana(bot)?, player::max_mana(bot)?),
                    };
                    should = compare(now, max, *greater, *value, *percent);
                }
                SkillRule::Aura(check) => {
                    should = auras_match(bot, std::slice::from_ref(check), false)?
                }
                SkillRule::MultiAura { checks, or } => should = auras_match(bot, checks, *or)?,
                SkillRule::PartyHealth {
                    greater,
                    value,
                    percent,
                } => {
                    should = party_health(bot, *greater, *value, *percent)?;
                }
                SkillRule::Wait { ms } => {
                    if rule.skip && !can_use {
                        return Ok(None);
                    }
                    bot.sleep((*ms).max(0) as u64)?;
                }
                SkillRule::None => {}
            }
            if rule.skip && !should {
                return Ok(None);
            }
            if !should {
                break;
            }
        }
        Ok(Some(should))
    }
}

fn field(value: &Value, key: &str) -> i64 {
    crate::api::packets::num(value.get(key)).unwrap_or_default()
}

/// Health/mana comparison: percent of max (truncated) or the flat value.
fn compare(now: i64, max: i64, greater: bool, value: i64, percent: bool) -> bool {
    let amount = if percent {
        if max == 0 {
            return false;
        }
        (now as f64 / max as f64 * 100.0) as i64
    } else {
        now
    };
    if greater {
        amount >= value
    } else {
        amount <= value
    }
}

/// `GetAuraSnapshots(subject)`: the stacks of an aura (0 when missing).
fn aura_stacks(bot: &Bot, target: bool, name: &str) -> BotResult<f32> {
    if target && !player::has_target(bot)? {
        return Ok(0.0);
    }
    let raw = bot.flash.call(
        "GetAuraSnapshots",
        vec![if target { "Target" } else { "Self" }.into()],
    )?;
    let snapshots: Vec<Value> = serde_json::from_str(raw.trim()).unwrap_or_default();
    Ok(snapshots
        .iter()
        .find(|s| {
            s.get("nam")
                .and_then(Value::as_str)
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .map_or(0.0, |s| field(s, "n") as f32))
}

/// VibeSkua `AuraUseRule` / `MultiAuraUseRule`.
fn auras_match(bot: &Bot, checks: &[AuraCheck], or: bool) -> BotResult<bool> {
    if checks.is_empty() {
        return Ok(true);
    }
    for check in checks {
        let stacks = aura_stacks(bot, check.target, &check.name)?;
        let ok = if check.greater {
            stacks >= check.stacks
        } else {
            stacks <= check.stacks
        };
        if or && ok {
            return Ok(true);
        }
        if !or && !ok {
            return Ok(false);
        }
    }
    Ok(!or)
}

/// VibeSkua `PartyHealthUseRule`: any living player in your cell matches.
fn party_health(bot: &Bot, greater: bool, value: i64, percent: bool) -> BotResult<bool> {
    let players: Vec<Value> = match bot.flash.get_opt("world.players") {
        Ok(p) => p.unwrap_or_default(),
        Err(BotError::Stopped) => return Err(BotError::Stopped),
        Err(_) => return Ok(false),
    };
    if players.is_empty() {
        return Ok(false);
    }
    let cell = player::cell(bot)?;
    Ok(players.iter().any(|p| {
        let in_cell = p
            .get("strFrame")
            .and_then(Value::as_str)
            .is_some_and(|f| !f.is_empty() && f == cell);
        let leaf = p.get("dataLeaf").cloned().unwrap_or(Value::Null);
        let (hp, max) = (field(&leaf, "intHP"), field(&leaf, "intHPMax"));
        in_cell && hp != 0 && (!percent || max != 0) && compare(hp, max, greater, value, percent)
    }))
}

// ---------------------------------------------------------------------------------------
// The skill timer (ScriptSkill)
// ---------------------------------------------------------------------------------------

/// `canUseSkill(index)`: a living target, off global and skill cooldown, usable.
pub fn can_use_skill(bot: &Bot, index: i64) -> BotResult<bool> {
    bot.flash.call_bool("canUseSkill", vec![index.into()])
}

/// `useSkill(index)`: uses it if it's off cooldown.
pub fn use_skill(bot: &Bot, index: i64) -> BotResult<bool> {
    bot.flash.call_bool("useSkill", vec![index.into()])
}

/// `world.actions.active`.
pub fn skills(bot: &Bot) -> BotResult<Vec<Skill>> {
    Ok(Skill::list_from(
        bot.flash
            .get_opt("world.actions.active")?
            .unwrap_or(Value::Null),
    ))
}

struct Settings {
    provider: Option<Arc<Mutex<SkillProvider>>>,
    use_mode: SkillUseMode,
}

#[derive(Default)]
struct SkillShared {
    settings: Mutex<Option<Settings>>,
    /// VibeSkua `SkillTimeout` (-1 waits forever).
    timeout: AtomicI64,
    paused: AtomicBool,
}

/// Runs the skill rotation on its own thread while a script fights (VibeSkua
/// `ScriptSkill`'s "Skill Timer": every 100 ms, while you have a target, picks the next
/// skill of the sequence and uses it if its rules allow).
pub struct SkillRunner {
    shared: Arc<SkillShared>,
    worker: Worker,
}

impl Default for SkillRunner {
    fn default() -> Self {
        let shared = SkillShared::default();
        shared.timeout.store(-1, Ordering::Relaxed);
        Self {
            shared: Arc::new(shared),
            worker: Worker::default(),
        }
    }
}

/// VibeSkua `SkillInterval`.
const SKILL_INTERVAL: u64 = 100;

impl SkillRunner {
    /// VibeSkua `TimerRunning`.
    pub fn timer_running(&self) -> bool {
        self.worker.running()
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    /// Stops the timer (waits up to 200 ms). The thread cancels the auto attack and the
    /// target on its way out. Called when a script ends; never calls Flash itself.
    pub fn stop(&self) {
        self.worker.stop(Duration::from_millis(200));
        self.shared.paused.store(false, Ordering::Relaxed);
    }

    /// VibeSkua `Pause`: the timer idles until [`SkillRunner::resume`].
    pub fn pause(&self) {
        if self.timer_running() {
            self.shared.paused.store(true, Ordering::Relaxed);
        }
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Relaxed);
    }

    /// The loaded sequence (`None` before anything was loaded).
    pub fn provider(&self) -> Option<SkillProvider> {
        let settings = self.shared.settings.lock();
        settings
            .as_ref()
            .and_then(|s| s.provider.as_ref())
            .map(|p| p.lock().clone())
    }

    fn load(&self, provider: SkillProvider, timeout: Option<i64>, use_mode: SkillUseMode) {
        if let Some(timeout) = timeout {
            self.shared.timeout.store(timeout, Ordering::Relaxed);
        }
        *self.shared.settings.lock() = Some(Settings {
            provider: Some(Arc::new(Mutex::new(provider))),
            use_mode,
        });
    }

    /// VibeSkua `LoadAdvanced(skills, timeout, mode)`: a sequence given as text.
    pub fn load_skills(&self, skills: &str, timeout: i64, use_mode: SkillUseMode) {
        self.load(SkillProvider::new(skills), Some(timeout), use_mode);
    }

    /// VibeSkua `LoadAdvanced(className, autoEquip, mode)`: the class's saved set for `mode`
    /// (else its first set, else the generic `0 | 1 | 2 | 3 | 4`). With `auto_equip`, equips
    /// the class first. Calls Flash: not for the UI thread.
    pub fn load_advanced(
        &self,
        bot: &Bot,
        class_name: &str,
        auto_equip: bool,
        mode: &str,
    ) -> BotResult<()> {
        if class_name.eq_ignore_ascii_case("generic") {
            self.load(
                SkillProvider::new(GENERIC_SKILLS),
                None,
                SkillUseMode::UseIfAvailable,
            );
            return Ok(());
        }
        if auto_equip {
            inventory::equip_item(bot, crate::api::NameOrId::Name(class_name))?;
        }
        let sets = load_advanced_skills(&bot.paths);
        match find_skill_set(&sets, class_name, mode) {
            Some(set) => {
                let mut provider = SkillProvider::new(&set.skills);
                provider.reset_on_target = set.reset_on_target;
                self.load(provider, Some(set.skill_timeout), set.skill_use_mode);
            }
            None => self.load(
                SkillProvider::new(GENERIC_SKILLS),
                None,
                SkillUseMode::UseIfAvailable,
            ),
        }
        Ok(())
    }

    /// VibeSkua `Start`: starts the timer with the loaded sequence (the generic one if none).
    pub fn start(&self, bot: &Arc<Bot>) {
        {
            let mut settings = self.shared.settings.lock();
            if settings.is_none() {
                *settings = Some(Settings {
                    provider: Some(Arc::new(Mutex::new(SkillProvider::new(GENERIC_SKILLS)))),
                    use_mode: SkillUseMode::UseIfAvailable,
                });
            }
        }
        let weak = Arc::downgrade(bot);
        let shared = self.shared.clone();
        self.worker.start("Skill Timer", move |cancel| {
            skill_timer(&weak, &shared, cancel)
        });
    }

    /// `skills.startSkills(text, timeout, mode)`.
    pub fn start_skills(&self, bot: &Arc<Bot>, skills: &str, timeout: i64, use_mode: SkillUseMode) {
        self.load_skills(skills, timeout, use_mode);
        self.start(bot);
    }

    /// `skills.startAdvanced(className?, autoEquip, mode)`: without a class, the equipped
    /// class's set (no re-equip), or the generic set when no class is equipped.
    pub fn start_advanced(
        &self,
        bot: &Arc<Bot>,
        class_name: Option<&str>,
        auto_equip: bool,
        mode: &str,
    ) -> BotResult<()> {
        match class_name.map(str::trim).filter(|c| !c.is_empty()) {
            Some(class) => self.load_advanced(bot, class, auto_equip, mode)?,
            None => {
                let class = inventory::current_class(bot)?.unwrap_or_else(|| "generic".into());
                self.load_advanced(bot, &class, false, mode)?;
            }
        }
        self.start(bot);
        Ok(())
    }
}

/// The current provider and mode (the script may load a new sequence while the timer runs).
fn current(shared: &SkillShared) -> Option<(Arc<Mutex<SkillProvider>>, SkillUseMode)> {
    let settings = shared.settings.lock();
    let s = settings.as_ref()?;
    Some((s.provider.clone()?, s.use_mode))
}

fn skill_timer(weak: &Weak<Bot>, shared: &SkillShared, cancel: &AtomicBool) {
    let mut cache: Option<Vec<Skill>> = None;
    let mut last_rank = -1;
    if let Some(bot) = weak.upgrade()
        && let Ok(list) = skills(&bot)
        && !list.is_empty()
    {
        cache = Some(list);
    }
    loop {
        let Some(bot) = weak.upgrade() else { return };
        let result = (|| -> BotResult<bool> {
            while shared.paused.load(Ordering::Relaxed) {
                if !nap(&bot, SKILL_INTERVAL, cancel)? {
                    return Ok(false);
                }
            }
            if cancel.load(Ordering::Acquire) {
                return Ok(false);
            }
            let Some((provider, use_mode)) = current(shared) else {
                return nap(&bot, SKILL_INTERVAL, cancel);
            };
            if !player::alive(&bot)? {
                provider.lock().on_player_death();
            }
            let attack = if bot.opts().attack_without_target {
                player::loaded(&bot)? && player::playing(&bot)?
            } else {
                player::has_target(&bot)? && player::loaded(&bot)? && player::playing(&bot)?
            };
            if attack {
                poll(
                    &bot,
                    shared,
                    &provider,
                    use_mode,
                    &mut cache,
                    &mut last_rank,
                    cancel,
                )?;
            }
            provider.lock().on_target_reset(&bot)?;
            nap(&bot, SKILL_INTERVAL, cancel)
        })();
        match result {
            Ok(true) => {}
            Ok(false) | Err(BotError::Stopped) => break,
            Err(e) => {
                tracing::debug!("Skill timer: {e}");
                if !matches!(nap(&bot, SKILL_INTERVAL, cancel), Ok(true)) {
                    break;
                }
            }
        }
    }
    // VibeSkua `AdvancedSkillProvider.Stop`: cancel the auto attack and the target. Done
    // here (not in `SkillRunner::stop`, which may run on the UI thread), off the script.
    mark_script_thread(false);
    if let Some(bot) = weak.upgrade() {
        let _ = bot
            .flash
            .call_game_function("world.cancelAutoAttack", vec![]);
        let _ = bot.flash.call_game_function("world.cancelTarget", vec![]);
    }
    if let Some((provider, _)) = current(shared) {
        let mut p = provider.lock();
        p.last_target_key = None;
        p.reset();
    }
}

/// VibeSkua `ScriptSkill._Poll`.
fn poll(
    bot: &Bot,
    shared: &SkillShared,
    provider: &Mutex<SkillProvider>,
    use_mode: SkillUseMode,
    cache: &mut Option<Vec<Skill>>,
    last_rank: &mut i64,
    cancel: &AtomicBool,
) -> BotResult<()> {
    // A class rank-up enables skills: refresh the cached action bar.
    let rank: Option<Value> = bot.flash.get_opt("world.myAvatar.objData.iRank")?;
    let rank = crate::api::packets::num(rank.as_ref()).unwrap_or_default();
    if rank > *last_rank || cache.is_none() {
        let list = skills(bot)?;
        if !list.is_empty() {
            *cache = Some(list);
        }
    }
    *last_rank = rank;
    if cancel.load(Ordering::Acquire) {
        return Ok(());
    }

    let Some((index, slot)) = provider.lock().next_skill() else {
        return Ok(());
    };
    let can_use = can_use_skill(bot, slot)?;
    let decision = provider.lock().should_use(bot, index, can_use)?;
    let slot = match decision {
        Some(true) => slot,
        None => match provider.lock().next_skill() {
            Some((_, slot)) => slot,
            None => return Ok(()),
        },
        Some(false) => return Ok(()),
    };
    let usable = cache
        .as_ref()
        .and_then(|c| c.get(slot as usize))
        .is_none_or(|s| s.is_ok);
    if !usable {
        return Ok(());
    }
    let without_target = bot.opts().attack_without_target;
    match use_mode {
        SkillUseMode::UseIfAvailable => {
            if without_target || can_use_skill(bot, slot)? {
                use_skill(bot, slot)?;
            }
        }
        SkillUseMode::WaitForCooldown => {
            if without_target || wait_for_cooldown(bot, shared, slot, cancel)? {
                use_skill(bot, slot)?;
            }
        }
    }
    Ok(())
}

/// VibeSkua `Wait.ForTrue(() => CanUseSkill(skill), SkillTimeout, SkillInterval)`; also ends
/// when the timer is stopped.
fn wait_for_cooldown(
    bot: &Bot,
    shared: &SkillShared,
    slot: i64,
    cancel: &AtomicBool,
) -> BotResult<bool> {
    let timeout = shared.timeout.load(Ordering::Relaxed);
    let mut tries = 0;
    loop {
        if crate::api::wait::safe(can_use_skill(bot, slot))? {
            return Ok(true);
        }
        if timeout > 0 && tries >= timeout {
            return Ok(false);
        }
        if !nap(bot, SKILL_INTERVAL, cancel)? {
            return Ok(false);
        }
        tries += 1;
    }
}
