//! Plugins > Butler (VibeSkua ButlerV5's roster and settings windows): this account and whom it
//! follows, the other online accounts running Butler with Follow and Summon (and why each rests),
//! Summon all, Send to player, Butler's options as option rows, and its debug log.
//!
//! Butler's own state is the bot's (`bot.butler`); what this adds per tab ([`State`]) is the last
//! action's outcome and the roster as read (at most once a second). Every function here locks the
//! state only briefly and never while calling into Butler, so a shell may call them from a frame.

use super::Control;
use super::form::{KindView, OptionValue, OptionView, SectionView};
use super::scheduler::{Note, Tint};
use super::tab_state::{PerTab, lock};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::butler::debug_log::Level;
use turu_bot::butler::sync::SyncData;
use turu_bot::butler::{self, ButlerSettings, ClassType, FollowSource, ParkSpot};

/// How often the roster is read again.
pub const REFRESH: Duration = Duration::from_secs(1);
/// The log lines a view carries (the newest).
const LOG_LINES: usize = 300;

#[derive(Default)]
pub struct State {
    pub message: Option<Note>,
    roster: Vec<SyncData>,
    /// Send to player's recent names (a file), read with the roster.
    recent: Vec<String>,
    read_at: Option<Instant>,
    /// The newest log lines as last taken, the log's version then, and how many it held.
    log: Vec<ButlerLogLine>,
    log_version: Option<u64>,
    log_count: u32,
    /// Streamer Mode's names, worked out with the roster (`None` while it's off).
    masks: Option<Masks>,
}

pub static STATE: PerTab<State> = PerTab::new();

// --- Streamer Mode ----------------------------------------------------------------------

/// The names Butler shows, each with what Streamer Mode shows instead: an account is "Account n"
/// as in the Accounts list (its place in `accounts.json`), one that isn't there "Alt n", and this
/// one its tab's label ("Tab 1").
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Masks(Vec<(String, String)>);

impl Masks {
    /// Turu: VibeSkua's ButlerV5 shows the names whatever Streamer Mode says; here they are part
    /// of the window being streamed. `None` while Streamer Mode is off.
    fn build(bot: &Bot, roster: &[SyncData]) -> Option<Self> {
        if !bot.opts().streamer_mode {
            return None;
        }
        let butler = &bot.butler;
        let saved = turu_bot::accounts::Accounts::load(&bot.paths.accounts_file());
        let mut names: Vec<String> = roster
            .iter()
            .flat_map(|a| [a.username.clone(), a.following.clone()])
            .collect();
        names.extend(butler.following().map(|(master, _)| master));
        names.extend(butler.followers());
        let me = butler.username();
        Some(Self::named(&me, &crate::turu::tab_label(bot), names, &saved))
    }

    fn named(me: &str, my_label: &str, names: Vec<String>, saved: &turu_bot::accounts::Accounts) -> Self {
        let mut pairs: Vec<(String, String)> = Vec::new();
        if !me.trim().is_empty() {
            pairs.push((me.trim().to_owned(), my_label.to_owned()));
        }
        let mut alts = 0;
        for name in names {
            let name = name.trim();
            if name.is_empty() || pairs.iter().any(|(n, _)| n.eq_ignore_ascii_case(name)) {
                continue;
            }
            let label = match saved
                .accounts
                .iter()
                .position(|a| a.username.eq_ignore_ascii_case(name))
            {
                Some(i) => format!("Account {}", i + 1),
                None => {
                    alts += 1;
                    format!("Alt {alts}")
                }
            };
            pairs.push((name.to_owned(), label));
        }
        // Longest first, so a name inside another isn't replaced there first.
        pairs.sort_by_key(|(n, _)| std::cmp::Reverse(n.len()));
        Self(pairs)
    }

    /// What to show for `name`.
    pub fn name(&self, name: &str) -> String {
        self.0
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name.trim()))
            .map_or_else(|| "another account".into(), |(_, label)| label.clone())
    }

    /// `text` with each name in it (as a whole word, in any case) replaced.
    pub fn text(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (name, label) in &self.0 {
            out = super::replace_word(&out, name, label);
        }
        out
    }
}

/// Streamer Mode's names as last worked out (`None` while it's off).
pub fn masks(bot: &Arc<Bot>) -> Option<Masks> {
    if !bot.opts().streamer_mode {
        return None;
    }
    let cached = lock(&STATE.of(bot)).masks.clone();
    cached.or_else(|| {
        roster(bot, true);
        lock(&STATE.of(bot)).masks.clone()
    })
}

/// The last action's outcome, as every shell shows it.
pub fn say(bot: &Arc<Bot>, result: Result<String, String>) {
    lock(&STATE.of(bot)).message = Some(match result {
        Ok(text) => Note {
            text,
            tint: Tint::Muted,
        },
        Err(text) => Note {
            text,
            tint: Tint::Danger,
        },
    });
}

pub fn message(bot: &Arc<Bot>) -> Option<Note> {
    let note = lock(&STATE.of(bot)).message.clone()?;
    Some(match masks(bot) {
        Some(masks) => Note {
            text: masks.text(&note.text),
            ..note
        },
        None => note,
    })
}

/// The other online accounts, read at most once a second (or now, `fresh`).
pub fn roster(bot: &Arc<Bot>, fresh: bool) -> Vec<SyncData> {
    let state = STATE.of(bot);
    let stale = fresh || lock(&state).read_at.is_none_or(|t| t.elapsed() >= REFRESH);
    if stale {
        // Read outside the lock: Butler has locks of its own.
        let roster = bot.butler.roster();
        let recent = bot.butler.recent_targets();
        let masks = Masks::build(bot, &roster);
        let mut s = lock(&state);
        s.roster = roster;
        s.recent = recent;
        s.masks = masks;
        s.read_at = Some(Instant::now());
    }
    lock(&state).roster.clone()
}

/// The newest log lines, taken again only when the log changed.
fn log(bot: &Arc<Bot>) -> (Vec<ButlerLogLine>, u32) {
    let state = STATE.of(bot);
    let version = bot.butler.log_version();
    if lock(&state).log_version != Some(version) {
        let lines = bot.butler.log_lines();
        let skip = lines.len().saturating_sub(LOG_LINES);
        let log: Vec<ButlerLogLine> = lines[skip..]
            .iter()
            .map(|l| ButlerLogLine {
                text: l.text.clone(),
                tint: level_tint(l.level),
            })
            .collect();
        let mut s = lock(&state);
        s.log = log;
        s.log_count = lines.len() as u32;
        s.log_version = Some(version);
    }
    let s = lock(&state);
    (s.log.clone(), s.log_count)
}

// --- the header ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ButlerHeader {
    /// This account ("" while logged out).
    pub me: String,
    /// "Following Hero (summoned)", "Not following anyone", ...
    pub state: String,
    pub tint: Tint,
    /// Butler is on (Settings' Enable Butler).
    pub enabled: bool,
    /// Stop following is offered.
    pub following: bool,
    /// Release all is offered (someone is summoned).
    pub summoning: bool,
}

/// This account and whom it follows; `masks` (Streamer Mode's) hides the names.
pub fn header(bot: &Bot, masks: Option<&Masks>) -> ButlerHeader {
    let butler = &bot.butler;
    let following = butler.following();
    let followers = butler.followers();
    let (state, tint) = match &following {
        Some((master, _)) if butler.is_parked() => (format!("Parked, waiting for {master}"), Tint::Warning),
        Some((master, source)) => {
            let how = match source {
                FollowSource::Ordered => "summoned",
                FollowSource::Manual => "manual",
            };
            (format!("Following {master} ({how})"), Tint::Success)
        }
        None if butler.is_parking() => ("Going to the park spot".into(), Tint::Muted),
        None if !followers.is_empty() => (format!("Summoning {}", followers.join(", ")), Tint::Success),
        None => ("Not following anyone".into(), Tint::Muted),
    };
    let me = butler.username();
    let (me, state) = match masks {
        Some(masks) if !me.is_empty() => (masks.name(&me), masks.text(&state)),
        Some(masks) => (me, masks.text(&state)),
        None => (me, state),
    };
    ButlerHeader {
        me,
        state,
        tint,
        enabled: butler.enabled(),
        following: following.is_some(),
        summoning: !followers.is_empty(),
    }
}

// --- the roster ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ButlerChip {
    pub text: String,
    pub tint: Tint,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ButlerAccount {
    pub username: String,
    /// What to show for it: the username, or "Account 2" in Streamer Mode.
    pub label: String,
    /// "fighting", "following me", "busy: Farm.ts", "other server", ...
    pub chips: Vec<ButlerChip>,
    /// "battleon-1234 · Twilly · Legion Revenant".
    pub place: String,
    /// This account has it summoned: Release, else Summon (with why it rests).
    pub summoned: bool,
    pub summon: Control,
    /// This account follows it: Stop, else Follow (with why it rests).
    pub followed: bool,
    pub follow: Control,
}

/// A roster row: its chips, and what Follow and Summon may do (VibeSkua's rules); `masks`
/// (Streamer Mode's) hides the names, and the place's room number.
pub fn account(bot: &Bot, account: &SyncData, masks: Option<&Masks>) -> ButlerAccount {
    let butler = &bot.butler;
    let me = butler.username();
    let enabled = butler.enabled();
    let following = butler.following().map(|(m, _)| m);
    let summoned = butler
        .followers()
        .iter()
        .any(|f| f.eq_ignore_ascii_case(&account.username));
    let followed = following
        .as_deref()
        .is_some_and(|m| m.eq_ignore_ascii_case(&account.username));
    let follows_me = !me.is_empty() && account.following.eq_ignore_ascii_case(&me);
    let follows_other = !account.following.is_empty() && !follows_me;
    let my_server = butler.server();
    let other_server =
        !account.server.is_empty() && !my_server.is_empty() && !account.server.eq_ignore_ascii_case(&my_server);

    let mut chips = Vec::new();
    let mut chip = |text: String, tint| chips.push(ButlerChip { text, tint });
    if account.attacking {
        chip("fighting".into(), Tint::Warning);
    }
    if follows_me {
        chip("following me".into(), Tint::Success);
    } else if follows_other {
        chip(format!("following {}", account.following), Tint::Muted);
    } else if summoned {
        chip("summoned".into(), Tint::Muted);
    }
    if account.script_on {
        let script = if account.script_name.is_empty() {
            "script"
        } else {
            &account.script_name
        };
        chip(format!("busy: {script}"), Tint::Warning);
    }
    if account.parking {
        chip("parking".into(), Tint::Muted);
    }
    if other_server {
        chip("other server".into(), Tint::Danger);
    }

    let summon_block = if summoned {
        None
    } else if !enabled {
        Some("Butler is disabled".to_owned())
    } else if let Some(master) = &following {
        Some(format!("You're following {master}: stop before summoning anyone"))
    } else {
        butler::unavailable(bot, account).map(|why| format!("{} {why}", account.username))
    };
    let follow_block = if followed {
        None
    } else if !enabled {
        Some("Butler is disabled".to_owned())
    } else if bot.scripts.is_running() {
        Some("A script is running here: stop it first".to_owned())
    } else if let Some(master) = &following {
        Some(format!("You're following {master}: stop first"))
    } else if summoned {
        Some(format!("Release {} before following them", account.username))
    } else if follows_me {
        Some(format!("{} follows you", account.username))
    } else if follows_other {
        Some(format!("{} follows {}", account.username, account.following))
    } else if other_server {
        Some(format!("{} is on {}", account.username, account.server))
    } else {
        None
    };
    let map = if masks.is_some() {
        account.map.clone()
    } else {
        account.map_with_room()
    };
    let place: Vec<String> = [map, account.server.clone(), account.class_name.clone()]
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect();
    let mut row = ButlerAccount {
        username: account.username.clone(),
        label: account.username.clone(),
        chips,
        place: place.join(" \u{B7} "),
        summoned,
        summon: summon_block.map_or_else(Control::on, Control::off),
        followed,
        follow: follow_block.map_or_else(Control::on, Control::off),
    };
    if let Some(masks) = masks {
        row.label = masks.name(&row.username);
        for chip in &mut row.chips {
            chip.text = masks.text(&chip.text);
        }
        row.summon.reason = masks.text(&row.summon.reason);
        row.follow.reason = masks.text(&row.follow.reason);
    }
    row
}

/// Summon all's rule.
pub fn summon_all_control(bot: &Bot) -> Control {
    if !bot.butler.enabled() {
        Control::off("Butler is disabled")
    } else if bot.butler.following().is_some() {
        Control::off("You're following someone: stop first")
    } else {
        Control::on()
    }
}

/// Follow, off the caller's thread (it waits for a previous follow to end), saying how it went.
pub fn follow(bot: &Arc<Bot>, master: String) {
    crate::turu::spawn_action_for(bot.clone(), "Butler Follow", move |bot| {
        let result = butler::start_follow(bot, &master).map(|()| format!("Following {master}."));
        say(bot, result);
        Ok(())
    });
}

/// Send to player, off the caller's thread (it calls the game).
pub fn send_to_player(bot: &Arc<Bot>, target: String) {
    crate::turu::spawn_action_for(bot.clone(), "Butler Send to player", move |bot| {
        let result = butler::send_all_to_player(bot, &target);
        say(bot, result);
        Ok(())
    });
}

// --- the settings, as option rows ---------------------------------------------------------

pub const EVERY_CLIENT: &str = "Every client";
pub const WHILE_FOLLOWING: &str = "While following";
pub const THIS_ACCOUNT: &str = "This account";

pub fn sections() -> Vec<SectionView> {
    vec![
        SectionView {
            chip: EVERY_CLIENT,
            title: "Settings (every client)",
        },
        SectionView {
            chip: WHILE_FOLLOWING,
            title: "While following",
        },
        SectionView {
            chip: THIS_ACCOUNT,
            title: "This account",
        },
    ]
}

fn park_key(spot: ParkSpot) -> &'static str {
    match spot {
        ParkSpot::House => "House",
        ParkSpot::Whitemap => "Whitemap",
        ParkSpot::Stay => "Stay",
    }
}

const CLASSES: [(ClassType, &str); 5] = [
    (ClassType::None, "None"),
    (ClassType::Farm, "Farm"),
    (ClassType::Solo, "Solo"),
    (ClassType::Dodge, "Dodge"),
    (ClassType::Boss, "Boss"),
];

fn class_key(class: ClassType) -> &'static str {
    CLASSES.iter().find(|(c, _)| *c == class).map_or("None", |(_, k)| k)
}

fn switch(label: &'static str, description: &'static str, section: &'static str, on: bool) -> OptionView {
    OptionView {
        label,
        description,
        section,
        group: "",
        kind: KindView::Switch,
        value: OptionValue::Switch(on),
        saved: OptionValue::Switch(on),
        differs: false,
        applies: true,
        unused: false,
        restart: false,
    }
}

fn with(mut o: OptionView, kind: KindView, value: OptionValue) -> OptionView {
    o.kind = kind;
    o.saved = value.clone();
    o.value = value;
    o
}

/// Butler's options (saved at once, shared by every client) and this account's own (none while
/// logged out), as the shells' option rows; [`set_option`] takes a row's label and value.
pub fn options(settings: &ButlerSettings, me: &str) -> Vec<OptionView> {
    let s = settings;
    let mut out = vec![
        switch(
            "Enable Butler",
            "Off: no client broadcasts, follows or summons. An active follow stops where it stands.",
            EVERY_CLIENT,
            s.enabled,
        ),
        switch(
            "Only attack when the master attacks",
            "Off: always fight beside the master.",
            EVERY_CLIENT,
            s.passive_attack,
        ),
        switch(
            "Leech mode",
            "Stay in the master's room at the Enter cell and never fight: for dragging a weak alt along.",
            EVERY_CLIENT,
            s.leech_mode,
        ),
        switch(
            "Always goto the master",
            "Keep sending goto while already in the master's room: fixes a butler stuck in the wrong cell after \
             dying, at the cost of a goto every Goto delay. Ignored in leech mode and when Copy master's walking \
             is on.",
            EVERY_CLIENT,
            s.always_goto,
        ),
        switch(
            "Copy master's walking",
            "When following in the same room and cell, walk where the master walks instead of standing still after \
             teleporting beside them. Goto, cell jumps, direct joins and rescues still handle map and room changes. \
             Ignored in leech mode and takes over from Always goto.",
            EVERY_CLIENT,
            s.copy_walk,
        ),
        with(
            switch(
                "Goto delay",
                "Time between gotos while chasing the master (250 ms is the loop's floor).",
                EVERY_CLIENT,
                false,
            ),
            KindView::Number {
                min: 250,
                max: Some(10_000),
                unit: Some("ms"),
            },
            OptionValue::Number(s.goto_delay as i64),
        ),
        with(
            switch(
                "Failed gotos before rescue",
                "Gotos that don't reach the master before joining its room from its file (how locked maps are \
                 entered). Lower reacts faster but can trigger on map-change lag.",
                EVERY_CLIENT,
                false,
            ),
            KindView::Number {
                min: 1,
                max: Some(10),
                unit: None,
            },
            OptionValue::Number(i64::from(s.rescue_threshold)),
        ),
        with(
            switch(
                "Park location",
                "Where a butler goes when it's released or its master goes offline. Stay can leave it in a boss room \
                 taking hits.",
                EVERY_CLIENT,
                false,
            ),
            KindView::Choice {
                choices: ["House", "Whitemap", "Stay"]
                    .iter()
                    .map(|k| ((*k).to_owned(), (*k).to_owned()))
                    .collect(),
            },
            OptionValue::Text(park_key(s.park).to_owned()),
        ),
        switch(
            "Quest bypasses",
            "When the butler lands on a map with quest-locked cells or monsters, fake the gate quest on this client \
             so the cell opens and the monsters can be targeted. Only on maps in the built-in table; the server is \
             never told anything.",
            WHILE_FOLLOWING,
            s.quest_bypass,
        ),
        with(
            switch(
                "Custom bypass quest IDs",
                "Extra quest IDs to fake when the follow starts, separated by commas (1234, 5678). For a map whose \
                 cells stay locked for butlers and isn't in the built-in table yet. Empty: none.",
                WHILE_FOLLOWING,
                false,
            ),
            KindView::Text { hint: "1234, 5678" },
            OptionValue::Text(s.quest_bypass_custom.clone()),
        ),
        switch(
            "Follow master's quests",
            "While following, mirror the master's active and registered quests: the butler registers the same quests \
             here, with the chosen rewards, so it accepts them, collects its own drops and turns them in from its own \
             inventory. Applied again after a relogin; removed when the follow ends.",
            WHILE_FOLLOWING,
            s.quest_sync,
        ),
        switch(
            "Unlock all quests (experimental)",
            "Sets every quest slot to the maximum on this client, so every quest-locked cell, monster and map opens \
             without a bypass list. Overrides Quest bypasses while on. Client side only and undone by a relogin, but \
             it scrambles this account's quest log until then: avoid it on an account you play by hand.",
            WHILE_FOLLOWING,
            s.quest_unlock_all,
        ),
        switch(
            "Fake level 100",
            "Shows this client as level 100 so level-gated maps (icestormunder) let the butler in. Client side only, \
             but your character shows as level 100 to you until you relog.",
            WHILE_FOLLOWING,
            s.fake_level,
        ),
        switch(
            "AntiLag while summoned",
            "Turn on AntiLag (Lag Killer, the Lite animation cuts, hidden monsters) while this account is summoned: \
             good for background alts. Your own settings come back when the follow ends.",
            WHILE_FOLLOWING,
            s.anti_lag_summoned,
        ),
        switch(
            "AntiLag while following",
            "The same, but only while following by hand (Follow in this window), not when summoned.",
            WHILE_FOLLOWING,
            s.anti_lag_following,
        ),
        switch(
            "Debug logs",
            "Record Butler's diagnostics in the debug log. Off: nothing is recorded.",
            WHILE_FOLLOWING,
            s.debug_logs,
        ),
        switch(
            "Auto-save logs",
            "Also append every log line to a file in the butler_logs folder as it happens, one file per account.",
            WHILE_FOLLOWING,
            s.auto_save_log,
        ),
    ];
    if !me.is_empty() {
        let account = s.account(me);
        out.push(with(
            switch(
                "Class type",
                "The CoreBots class (Options > CoreBots) this account equips when it starts following. None keeps the \
                 current gear.",
                THIS_ACCOUNT,
                false,
            ),
            KindView::Choice {
                choices: CLASSES
                    .iter()
                    .map(|(_, k)| ((*k).to_owned(), (*k).to_owned()))
                    .collect(),
            },
            OptionValue::Text(class_key(account.class_type).to_owned()),
        ));
        out.push(switch(
            "Obey summons",
            "Off: this account ignores every summon. Turn it off on the account you play by hand.",
            THIS_ACCOUNT,
            account.obey_summons,
        ));
    }
    out
}

/// The option row `label` set to `value`, in `settings` (this account's for its own rows).
pub fn set_option(settings: &mut ButlerSettings, me: &str, label: &str, value: &Json) -> Result<(), String> {
    let on = || value.as_bool().ok_or_else(|| format!("{label} takes on or off"));
    let number = || value.as_i64().ok_or_else(|| format!("{label} takes a number"));
    let text = || {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{label} takes text"))
    };
    let s = settings;
    match label {
        "Enable Butler" => s.enabled = on()?,
        "Only attack when the master attacks" => s.passive_attack = on()?,
        "Leech mode" => s.leech_mode = on()?,
        "Always goto the master" => s.always_goto = on()?,
        "Copy master's walking" => s.copy_walk = on()?,
        "Goto delay" => s.goto_delay = number()?.clamp(250, 10_000) as u64,
        "Failed gotos before rescue" => s.rescue_threshold = number()?.clamp(1, 10) as u32,
        "Park location" => {
            s.park = match text()?.as_str() {
                "House" => ParkSpot::House,
                "Whitemap" => ParkSpot::Whitemap,
                "Stay" => ParkSpot::Stay,
                other => return Err(format!("no park location {other}")),
            }
        }
        "Quest bypasses" => s.quest_bypass = on()?,
        "Custom bypass quest IDs" => s.quest_bypass_custom = text()?,
        "Follow master's quests" => s.quest_sync = on()?,
        "Unlock all quests (experimental)" => s.quest_unlock_all = on()?,
        "Fake level 100" => s.fake_level = on()?,
        "AntiLag while summoned" => s.anti_lag_summoned = on()?,
        "AntiLag while following" => s.anti_lag_following = on()?,
        "Debug logs" => s.debug_logs = on()?,
        "Auto-save logs" => s.auto_save_log = on()?,
        "Class type" | "Obey summons" => {
            if me.is_empty() {
                return Err("Log in to set this account's options".into());
            }
            let mut account = s.account(me);
            if label == "Class type" {
                let key = text()?;
                account.class_type = CLASSES
                    .iter()
                    .find(|(_, k)| *k == key)
                    .map(|(c, _)| *c)
                    .ok_or_else(|| format!("no class type {key}"))?;
            } else {
                account.obey_summons = on()?;
            }
            s.set_account(me, account);
        }
        other => return Err(format!("no option {other}")),
    }
    Ok(())
}

/// Sets and saves an option.
pub fn set(bot: &Bot, label: &str, value: &Json) -> Result<(), String> {
    let butler = &bot.butler;
    let mut settings = butler.settings();
    set_option(&mut settings, &butler.username(), label, value)?;
    butler
        .set_settings(settings)
        .map_err(|e| format!("Couldn't save the Butler settings: {e}"))
}

// --- the view a shell reads -------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ButlerView {
    pub header: ButlerHeader,
    pub message: Option<Note>,
    pub accounts: Vec<ButlerAccount>,
    pub summon_all: Control,
    /// Recent Send to player targets.
    pub recent: Vec<String>,
    pub sections: Vec<SectionView>,
    pub options: Vec<OptionView>,
    /// The newest log lines, oldest first.
    pub log: Vec<ButlerLogLine>,
    /// All the lines the log holds (more than `log` carries when long).
    pub log_count: u32,
    /// Debug logs is on (else nothing is recorded).
    pub logging: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ButlerLogLine {
    pub text: String,
    pub tint: Tint,
}

pub fn level_tint(level: Level) -> Tint {
    match level {
        Level::Info => Tint::Plain,
        Level::Success => Tint::Success,
        Level::Warning => Tint::Warning,
        Level::Error => Tint::Danger,
    }
}

pub fn view(bot: &Arc<Bot>) -> ButlerView {
    let butler = &bot.butler;
    let roster = roster(bot, false);
    let settings = butler.settings();
    let (log, log_count) = log(bot);
    let masks = masks(bot);
    // Send to player's recent names are other players': none to pick from in Streamer Mode.
    let recent = match masks {
        Some(_) => Vec::new(),
        None => lock(&STATE.of(bot)).recent.iter().take(8).cloned().collect(),
    };
    let log = match &masks {
        Some(masks) => log
            .into_iter()
            .map(|l| ButlerLogLine {
                text: masks.text(&l.text),
                ..l
            })
            .collect(),
        None => log,
    };
    ButlerView {
        header: header(bot, masks.as_ref()),
        message: message(bot),
        accounts: roster.iter().map(|a| account(bot, a, masks.as_ref())).collect(),
        summon_all: summon_all_control(bot),
        recent,
        sections: sections(),
        options: options(&settings, &butler.username()),
        log,
        log_count,
        logging: settings.debug_logs,
    }
}

/// `follow`, `summon`, `release` (`name`), `stop-follow`, `release-all`, `summon-all`, `send`
/// (`player`), `set` (`label`, `value`), `clear-log`, `save-log`, `copy-log` (answers `text`),
/// `dismiss` (the message).
pub fn act(bot: &Arc<Bot>, action: &str, body: &Json) -> Result<Json, String> {
    let text = |key: &str| {
        body.get(key)
            .and_then(Json::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{key} is missing"))
    };
    // Follow on a row as it is now, not as the view a tick ago showed it: `start_follow` checks
    // less than the button (it would switch masters, or follow one that follows you).
    let masks = masks(bot);
    let row_now = |name: &str| {
        roster(bot, true)
            .iter()
            .find(|a| a.username.eq_ignore_ascii_case(name))
            .map(|a| account(bot, a, masks.as_ref()))
            .ok_or_else(|| match &masks {
                Some(masks) => format!("{} isn't online with Butler", masks.name(name)),
                None => format!("{name} isn't online with Butler"),
            })
    };
    match action {
        "follow" => {
            let name = text("name")?;
            let row = row_now(&name)?;
            if !row.follow.enabled {
                return Err(row.follow.reason);
            }
            if !row.followed {
                follow(bot, row.username);
            }
        }
        // Not a toggle: a tap on a view a tick old must not undo itself.
        "summon" => say(bot, butler::summon(bot, &text("name")?)),
        "release" => say(bot, butler::release(bot, &text("name")?)),
        "stop-follow" => {
            butler::stop_follow(bot);
            say(bot, Ok("Stopped following.".into()));
        }
        "release-all" => {
            butler::release_all(bot);
            say(bot, Ok("Released all followers.".into()));
        }
        "summon-all" => say(bot, butler::summon_all(bot)),
        "send" => {
            let player = text("player")?;
            if player.trim().is_empty() {
                return Err("Name the player".into());
            }
            send_to_player(bot, player.trim().to_owned());
        }
        "set" => set(bot, &text("label")?, body.get("value").ok_or("value is missing")?)?,
        "clear-log" => bot.butler.clear_log(),
        "save-log" => say(
            bot,
            bot.butler
                .save_log()
                .map(|path| format!("Saved the log to {}", path.display()))
                .map_err(|e| format!("Couldn't save the log: {e}")),
        ),
        "copy-log" => return Ok(json!({ "text": bot.butler.log_text() })),
        "dismiss" => lock(&STATE.of(bot)).message = None,
        other => return Err(format!("no action {other}")),
    }
    // Show the new state now rather than at the next read.
    roster(bot, true);
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_and_summon_dont_toggle() {
        let bot = Arc::new(Bot::new());
        // Nobody is summoned: Release says so and summons nobody.
        assert!(act(&bot, "release", &json!({ "name": "Questwalker" })).is_ok());
        assert!(bot.butler.followers().is_empty());
        assert_eq!(message(&bot).unwrap().tint, Tint::Danger);
        // Not online: Summon refuses rather than adding a name.
        assert!(act(&bot, "summon", &json!({ "name": "Questwalker" })).is_ok());
        assert!(bot.butler.followers().is_empty());
        assert!(act(&bot, "follow", &json!({ "name": "Questwalker" })).is_err());
    }

    #[test]
    fn every_option_row_sets_its_field() {
        let mut s = ButlerSettings::default();
        let rows = options(&s, "Questwalker");
        assert_eq!(rows.len(), 19);
        for row in &rows {
            let value = match &row.kind {
                KindView::Switch => json!(true),
                KindView::Number { min, .. } => json!(min),
                KindView::Choice { choices } => json!(choices.last().unwrap().0),
                KindView::Text { .. } => json!("1234"),
                other => panic!("unexpected kind {other:?}"),
            };
            set_option(&mut s, "Questwalker", row.label, &value).unwrap_or_else(|e| panic!("{}: {e}", row.label));
        }
        assert_eq!(s.goto_delay, 250);
        assert_eq!(s.park, ParkSpot::Stay);
        assert_eq!(s.account("questwalker").class_type, ClassType::Boss);
        assert_eq!(s.quest_bypass_custom, "1234");
        assert!(set_option(&mut s, "Questwalker", "Goto delay", &json!(99_999)).is_ok());
        assert_eq!(s.goto_delay, 10_000, "bounded as the window bounds it");
        assert!(set_option(&mut s, "", "Obey summons", &json!(false)).is_err());
        assert!(set_option(&mut s, "Questwalker", "Leech mode", &json!("yes")).is_err());
        assert_eq!(options(&s, "").len(), 17, "no account rows while logged out");
    }

    /// Streamer Mode: a saved account is "Account n" (its place in the list), another "Alt n",
    /// this one its tab's label; rows name nobody and drop the room number.
    #[test]
    fn streamer_mode_hides_butlers_names() {
        let mut saved = turu_bot::accounts::Accounts::default();
        for name in ["Brambleox", "Questwalker"] {
            saved.upsert(turu_bot::accounts::Account {
                username: name.into(),
                ..Default::default()
            });
        }
        let names = vec!["questwalker".into(), "Zephyrtest".into(), String::new(), "Hero".into()];
        let masks = Masks::named("Hero", "Tab 1", names, &saved);
        assert_eq!(masks.name("Questwalker"), "Account 2");
        assert_eq!(masks.name("Zephyrtest"), "Alt 1");
        assert_eq!(masks.name("hero"), "Tab 1");
        assert_eq!(masks.name("Stranger"), "another account");
        assert_eq!(masks.text("Following Questwalker (summoned)"), "Following Account 2 (summoned)");
        let bot = Bot::new();
        let alt = SyncData {
            username: "Questwalker".into(),
            map: "battleon".into(),
            room: "1234".into(),
            following: "Zephyrtest".into(),
            ..Default::default()
        };
        let row = account(&bot, &alt, Some(&masks));
        assert_eq!((row.username.as_str(), row.label.as_str()), ("Questwalker", "Account 2"));
        assert!(!row.place.contains("1234"));
        let shown = format!("{:?} {:?} {:?}", row.chips, row.summon, row.follow);
        assert!(!shown.contains("Questwalker") && !shown.contains("Zephyrtest"), "{shown}");
        let plain = account(&bot, &alt, None);
        assert_eq!(plain.label, "Questwalker");
        assert!(plain.place.contains("battleon-1234"));
    }
}
