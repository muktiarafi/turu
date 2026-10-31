//! The Combat > Skills editor's model (VibeSkua `SkillItemViewModel`, `SkillRulesViewModel`,
//! `AuraCheckViewModel`): one skill of a sequence with the rules the editor shows, read from
//! and written back to the skill-string syntax (see [`crate::skills`]).

use crate::skills::Cursor;
use std::fmt::Write;

/// One check of a multi-aura rule (VibeSkua `AuraCheckViewModel`).
#[derive(Debug, Clone, PartialEq)]
pub struct AuraRow {
    pub name: String,
    pub stacks: f32,
    /// `>=` rather than `<=`.
    pub greater: bool,
    /// On the target rather than yourself.
    pub target: bool,
}

impl Default for AuraRow {
    fn default() -> Self {
        Self {
            name: String::new(),
            stacks: 0.0,
            greater: true,
            target: false,
        }
    }
}

/// A skill's rules as the editor shows them (VibeSkua `SkillRulesViewModel`). A value of 0
/// means "no such rule".
#[derive(Debug, Clone, PartialEq)]
pub struct SkillRules {
    /// "Enable Rule": without it the skill is written with no rules.
    pub use_rule: bool,
    pub multi_aura: bool,
    pub health_greater: bool,
    pub health: i64,
    pub health_percent: bool,
    pub mana_greater: bool,
    pub mana: i64,
    pub mana_percent: bool,
    pub party_health_greater: bool,
    pub party_health: i64,
    pub party_health_percent: bool,
    /// Milliseconds to wait before using the skill.
    pub wait: i64,
    /// "Skip if not available": a failed rule moves on to the next skill.
    pub skip: bool,
    pub aura_greater: bool,
    pub aura_stacks: f32,
    pub aura_target: bool,
    pub aura_name: String,
    pub auras: Vec<AuraRow>,
    /// Any of the multi-aura checks rather than all of them.
    pub auras_or: bool,
}

impl Default for SkillRules {
    fn default() -> Self {
        Self {
            use_rule: false,
            multi_aura: false,
            health_greater: true,
            health: 0,
            health_percent: true,
            mana_greater: true,
            mana: 0,
            mana_percent: true,
            party_health_greater: true,
            party_health: 0,
            party_health_percent: true,
            wait: 0,
            skip: false,
            aura_greater: true,
            aura_stacks: 0.0,
            aura_target: false,
            aura_name: String::new(),
            auras: Vec::new(),
            auras_or: false,
        }
    }
}

impl SkillRules {
    /// VibeSkua `RemoveAuraCheck`: the last one removed turns Multi Aura off.
    pub fn remove_aura(&mut self, index: usize) {
        if index < self.auras.len() {
            self.auras.remove(index);
        }
        if self.auras.is_empty() {
            self.multi_aura = false;
        }
    }
}

/// One skill of the sequence being edited (VibeSkua `SkillItemViewModel`).
#[derive(Debug, Clone, PartialEq)]
pub struct SkillItem {
    /// The slot: 0 auto attack .. 5 potion.
    pub skill: i64,
    pub rules: SkillRules,
}

impl SkillItem {
    /// A skill with a copy of `rules` (the editor's Use Rules when a slot button is clicked).
    pub fn new(skill: i64, rules: &SkillRules) -> Self {
        Self {
            skill,
            rules: rules.clone(),
        }
    }

    /// VibeSkua `SkillItemViewModel(string)`: one entry of a skill string, as the saved sets
    /// write it (`4 H>50% A>"Focus" 3 TARGET`); `None` unless it starts with a digit.
    ///
    /// Two of VibeSkua's slips are fixed: a `<` check after a `>` one in a multi-aura rule
    /// stays `<`, and the skip flag is read from the end of the entry, so `... TARGETS` keeps it.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let first = text.chars().next()?;
        let skill = first.to_digit(10)? as i64;
        let rest = text[first.len_utf8()..].trim();
        let mut rules = SkillRules::default();
        if rest.is_empty() {
            return Some(Self { skill, rules });
        }
        let mut cur = Cursor::new(rest);
        let mut aura_greater = None;
        let mut aura_name = String::new();
        let mut aura_stacks = 0.0;
        // `H`, `M` and `PH` default to "<" when parsed (the editor's own default is ">").
        let amount = |cur: &mut Cursor| {
            let greater = cur.comparison();
            let (value, percent) = cur.amount().unwrap_or((0, true));
            cur.skip_spaces();
            (greater, value, percent)
        };
        while let Some(ch) = cur.peek() {
            let next = cur.at(1);
            if ch == 'W' && next == Some('W') {
                rules.use_rule = true;
                cur.pos += 2;
                let digits = cur.take_while(|c| c.is_ascii_digit());
                if !digits.is_empty() {
                    rules.wait = digits.parse().unwrap_or(i64::MAX);
                }
                cur.skip_spaces();
            } else if ch == 'P' && next == Some('H') {
                rules.use_rule = true;
                cur.pos += 2;
                (
                    rules.party_health_greater,
                    rules.party_health,
                    rules.party_health_percent,
                ) = amount(&mut cur);
            } else if ch == 'H' {
                rules.use_rule = true;
                cur.pos += 1;
                (rules.health_greater, rules.health, rules.health_percent) = amount(&mut cur);
            } else if ch == 'M' && next == Some('A') {
                rules.use_rule = true;
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
                    if rules.auras.is_empty() {
                        rules.auras_or = op == ':';
                    }
                    cur.pos += 1;
                }
                if !name.is_empty() {
                    rules.auras.push(AuraRow {
                        name,
                        stacks,
                        greater,
                        target,
                    });
                }
                cur.skip_spaces();
            } else if ch == 'M' {
                rules.use_rule = true;
                cur.pos += 1;
                (rules.mana_greater, rules.mana, rules.mana_percent) = amount(&mut cur);
            } else if ch == 'A' {
                rules.use_rule = true;
                cur.pos += 1;
                aura_greater = Some(cur.comparison());
                (aura_name, aura_stacks) = if cur.peek() == Some('"') {
                    let name = cur.quoted();
                    cur.skip_spaces();
                    (name, cur.float())
                } else {
                    cur.unquoted_aura()
                };
                cur.skip_spaces();
                rules.aura_target = cur.target();
                cur.skip_spaces();
            } else if ch == 'S' {
                rules.use_rule = true;
                rules.skip = true;
                cur.pos += 1;
                cur.skip_spaces();
            } else {
                cur.pos += 1;
            }
        }
        if rest.ends_with(['S', 's']) {
            rules.use_rule = true;
            rules.skip = true;
        }
        if let Some(greater) = aura_greater {
            rules.aura_greater = greater;
        }
        rules.multi_aura = !rules.auras.is_empty();
        if !rules.multi_aura {
            rules.aura_name = aura_name;
            rules.aura_stacks = aura_stacks;
        }
        Some(Self { skill, rules })
    }

    /// VibeSkua `SkillItemViewModel.ToString`, the editor list's line:
    /// `4 - [Health > 50%] - [Skip if not available]`.
    pub fn display(&self) -> String {
        let r = &self.rules;
        let mut out = self.skill.to_string();
        if !r.use_rule {
            return out;
        }
        let cmp = |greater: bool| if greater { " > " } else { " < " };
        let unit = |percent: bool| if percent { "%]" } else { "#]" };
        if r.wait != 0 {
            let _ = write!(out, " - [Wait for {}]", r.wait);
        }
        if r.health != 0 {
            let _ = write!(
                out,
                " - [Health{}{}{}",
                cmp(r.health_greater),
                r.health,
                unit(r.health_percent)
            );
        }
        if r.mana != 0 {
            let _ = write!(
                out,
                " - [Mana{}{}{}",
                cmp(r.mana_greater),
                r.mana,
                unit(r.mana_percent)
            );
        }
        if r.party_health != 0 {
            let _ = write!(
                out,
                " - [Party HP{}{}{}",
                cmp(r.party_health_greater),
                r.party_health,
                unit(r.party_health_percent)
            );
        }
        if r.multi_aura && !r.auras.is_empty() {
            let _ = write!(
                out,
                " - [Multi-Aura ({})",
                if r.auras_or { "OR" } else { "AND" }
            );
            for check in &r.auras {
                let op = if check.greater { ">" } else { "<" };
                let _ = write!(out, " '{}'{op}{}", check.name, check.stacks);
            }
            out.push(']');
        } else if r.aura_stacks != 0.0 || !r.aura_name.is_empty() {
            let target = if r.aura_target { "Target" } else { "Self" };
            let _ = write!(out, " - [Aura ({target})");
            if !r.aura_name.is_empty() {
                let _ = write!(out, " '{}'", r.aura_name);
            }
            let _ = write!(out, "{}{}]", cmp(r.aura_greater), r.aura_stacks);
        }
        if r.skip {
            out.push_str(" - [Skip if not available]");
        }
        out
    }

    /// VibeSkua `SkillItemViewModel.Convert`: the entry in skill-string syntax.
    pub fn convert(&self) -> String {
        let r = &self.rules;
        let mut out = self.skill.to_string();
        if !r.use_rule {
            return out;
        }
        let cmp = |greater: bool| if greater { ">" } else { "<" };
        let unit = |percent: bool| if percent { "%" } else { "#" };
        let target = |target: bool| if target { " TARGET" } else { "" };
        if r.wait != 0 {
            let _ = write!(out, " WW{}", r.wait);
        }
        if r.health != 0 {
            let _ = write!(
                out,
                " H{}{}{}",
                cmp(r.health_greater),
                r.health,
                unit(r.health_percent)
            );
        }
        if r.mana != 0 {
            let _ = write!(
                out,
                " M{}{}{}",
                cmp(r.mana_greater),
                r.mana,
                unit(r.mana_percent)
            );
        }
        if r.party_health != 0 {
            let _ = write!(
                out,
                " PH{}{}{}",
                cmp(r.party_health_greater),
                r.party_health,
                unit(r.party_health_percent)
            );
        }
        if r.multi_aura && !r.auras.is_empty() {
            let op = if r.auras_or { ":" } else { "&" };
            for (i, check) in r.auras.iter().enumerate() {
                let suffix = if i + 1 < r.auras.len() { op } else { "" };
                let _ = write!(
                    out,
                    " MA{}\"{}\" {}{}{suffix}",
                    cmp(check.greater),
                    check.name,
                    check.stacks,
                    target(check.target)
                );
            }
        } else if !r.aura_name.is_empty() {
            let _ = write!(
                out,
                " A{}\"{}\" {}{}",
                cmp(r.aura_greater),
                r.aura_name,
                r.aura_stacks,
                target(r.aura_target)
            );
        }
        if r.skip {
            out.push('S');
        }
        out
    }
}

/// VibeSkua `EditSkill(message)`: a saved set's skill string as editor items.
pub fn parse_sequence(skills: &str) -> Vec<SkillItem> {
    skills
        .split('|')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(SkillItem::parse)
        .collect()
}

/// VibeSkua `SaveSkills`: the items joined into one skill string.
pub fn sequence_string(items: &[SkillItem]) -> String {
    items
        .iter()
        .map(SkillItem::convert)
        .collect::<Vec<_>>()
        .join(" | ")
}
