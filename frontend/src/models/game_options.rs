//! Options > Game (VibeSkua's Game Options): every bot option with its section, group, label and
//! description, its value live and as saved in `options.json`, and what Save, Revert and Defaults
//! do. Changes apply to `bot.options` at once (the engine re-applies them every 250 ms); Save
//! keeps them. The egui window (`gui/turu/game_options.rs`) lays this out as VibeSkua's list;
//! a native shell reads [`view`] and sends [`set`], [`save`], [`revert`], [`reset`].
//!
//! What a window changed itself ("Edited" against "Changed outside", Revert mine) is that window's
//! state, not the bot's: each shell keeps its own set of edited options and passes it in.

use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use turu_bot::Bot;
use turu_bot::options::{BotOptions, HuntPriority};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Visuals,
    Combat,
    Drops,
    Relogin,
    Gameplay,
}

impl Section {
    pub const ALL: [Section; 5] = [Self::Visuals, Self::Combat, Self::Drops, Self::Relogin, Self::Gameplay];

    pub fn chip(self) -> &'static str {
        match self {
            Self::Visuals => "Visuals",
            Self::Combat => "Combat",
            Self::Drops => "Drops",
            Self::Relogin => "Relogin",
            Self::Gameplay => "Gameplay",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Visuals => "Performance & Visuals",
            Self::Combat => "Combat & Targeting",
            Self::Drops => "Drops & Looting",
            Self::Relogin => "Auto-Relogin & Network",
            Self::Gameplay => "Gameplay & Character",
        }
    }
}

pub enum Field {
    Bool(fn(&mut BotOptions) -> &mut bool),
    /// A number of at least the minimum, with an optional unit ("ms").
    Int(fn(&mut BotOptions) -> &mut i32, i32, Option<&'static str>),
    /// Set FPS: 0 leaves the frame rate to the game's own setting.
    Fps,
    /// A text box with a hint for when it's empty.
    Text(fn(&mut BotOptions) -> &mut String, &'static str),
    Server,
    Priority,
    ReloadMap,
}

pub struct Entry {
    pub section: Section,
    pub label: &'static str,
    /// What the option does, in a line or two under its label.
    pub description: &'static str,
    pub field: Field,
    /// The small heading this option sits under inside its section ("" for none).
    pub group: &'static str,
    /// The option this one only matters with (Private Number: Private Rooms): when that is off,
    /// the row is indented and dimmed, but stays editable.
    pub requires: Option<fn(&BotOptions) -> bool>,
}

macro_rules! b {
    ($section:ident, $label:expr, $f:ident, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Bool(|o| &mut o.$f),
            group: "",
            requires: None,
        }
    };
}
macro_rules! n {
    ($section:ident, $label:expr, $f:ident, $min:expr, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Int(|o| &mut o.$f, $min, None),
            group: "",
            requires: None,
        }
    };
    ($section:ident, $label:expr, $f:ident, $min:expr, $suffix:expr, $description:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: Field::Int(|o| &mut o.$f, $min, Some($suffix)),
            group: "",
            requires: None,
        }
    };
}

/// An entry that isn't a plain switch or number box.
fn entry(section: Section, label: &'static str, field: Field, description: &'static str) -> Entry {
    Entry {
        section,
        label,
        description,
        field,
        group: "",
        requires: None,
    }
}

/// `entries` under the small heading `group`.
fn grouped(group: &'static str, entries: Vec<Entry>) -> Vec<Entry> {
    entries.into_iter().map(|e| Entry { group, ..e }).collect()
}

/// The heading the [`UNUSED`] options sit under, at the end of their section.
pub const UNUSED_GROUP: &str = "Not used by Turu yet";

/// Every field, in VibeSkua's order and with its names, but for Gameplay, which is grouped under
/// small headings, and the options Turu doesn't read yet, which go last in their section. The
/// descriptions say what Turu does with each option (some are only read by scripts).
pub fn entries() -> Vec<Entry> {
    let mut list = vec![
        b!(
            Visuals,
            "Lag Killer",
            lag_killer,
            "Hides the game world to cut lag and CPU use; the bot keeps playing"
        ),
        b!(Visuals, "Hide Players", hide_players, "Hides player characters"),
        b!(
            Visuals,
            "Disable FX",
            disable_fx,
            "Turns off combat animations for a higher frame rate"
        ),
        b!(
            Visuals,
            "Disable Collisions",
            disable_collisions,
            "Turns off the map's walls and barriers"
        ),
        b!(Visuals, "Show FPS", show_fps, "Shows the game's frame-rate counter"),
        entry(
            Section::Visuals,
            "Set FPS",
            Field::Fps,
            "Frame rate to force in game, up to 240 (▾ for the usual ones). Off (0) leaves it to the game's own FPS setting. On Android, above 60 asks the display for a faster refresh rate (a 90, 120 or 144 Hz screen)",
        ),
        entry(
            Section::Visuals,
            "Reload Map",
            Field::ReloadMap,
            "Loads the current map again, for when it's stuck or drawn wrong",
        ),
        b!(
            Combat,
            "Aggro Monsters",
            aggro_monsters,
            "Makes every monster in your room attack you. It can keep you in combat, so quests won't turn in"
        ),
        b!(
            Combat,
            "Aggro All Monsters",
            aggro_all_monsters,
            "As Aggro Monsters, for every monster on the map"
        ),
        b!(
            Combat,
            "Attack Without Target",
            attack_without_target,
            "Uses skills even with nothing targeted. Use with caution"
        ),
        b!(
            Combat,
            "Infinite Range",
            infinite_range,
            "Hits targets at any distance without walking to them"
        ),
        b!(Combat, "Magnetise", magnetise, "Pulls targeted monsters to you"),
        n!(
            Combat,
            "Hunt Delay",
            hunt_delay,
            0,
            "ms",
            "The least time between room jumps while hunting monsters"
        ),
        n!(
            Combat,
            "Hunt Buffer",
            hunt_buffer,
            0,
            "In VibeSkua, kills to wait for before picking up drops while hunting"
        ),
        b!(
            Combat,
            "Rest Packets",
            rest_packets,
            "Rests about once a second out of combat, so you heal between fights"
        ),
        b!(
            Combat,
            "Safe Timings",
            safe_timings,
            "Scripts that read it (CoreBots) wait for actions such as pickups to finish before going on. Keep it on"
        ),
        entry(
            Section::Combat,
            "Hunt Priority",
            Field::Priority,
            "In VibeSkua, which monster a hunt attacks first",
        ),
        b!(
            Drops,
            "Accept AC Drops",
            accept_ac_drops,
            "Picks up every AC item that drops, even ones the script would reject"
        ),
        b!(
            Drops,
            "Accept All Drops",
            accept_all_drops,
            "Picks up every item that drops. Wins over Reject All Drops"
        ),
        b!(
            Drops,
            "Reject All Drops",
            reject_all_drops,
            "Rejects every drop, wanted ones too. Accept AC Drops still picks up AC items first"
        ),
        entry(
            Section::Relogin,
            "Relogin Server",
            Field::Server,
            "The server auto-relogin logs into",
        ),
        b!(
            Relogin,
            "Auto Relogin",
            auto_relogin,
            "Logs back in after a disconnect or kick, to Relogin Server"
        ),
        b!(
            Relogin,
            "Auto Relogin Any",
            auto_relogin_any,
            "Relogs to any server but the last one, which is more likely to succeed"
        ),
        b!(
            Relogin,
            "Relogin Fallback",
            relogin_fallback,
            "When Relogin Server is full or offline, relogs to any free server. Off, it keeps trying Relogin Server"
        ),
        b!(
            Relogin,
            "Retry Relogin",
            retry_relogin,
            "When Relogin Tries attempts fail, auto-relogin tries again after 1, 2, 5, then every 10 minutes"
        ),
        b!(
            Relogin,
            "Safe Relogin",
            safe_relogin,
            "Waits longer between login attempts (10 s, then 3 s more each try) so the server can drop the old session"
        ),
        n!(
            Relogin,
            "Relogin Tries",
            relogin_tries,
            1,
            "Login attempts before auto-relogin gives up"
        ),
        n!(
            Relogin,
            "Login Timeout",
            login_timeout,
            1000,
            "ms",
            "How long to wait for the world to load after logging in"
        ),
    ];
    list.extend(grouped(
        "World",
        vec![
            b!(
                Gameplay,
                "Private Rooms",
                private_rooms,
                "Joins maps in a numbered private room (Private Number) instead of a public one"
            ),
            Entry {
                requires: Some(|o| o.private_rooms),
                ..n!(
                    Gameplay,
                    "Private Number",
                    private_number,
                    0,
                    "The room number private joins use; 0 uses 100000. Used only while Private Rooms is on"
                )
            },
            n!(
                Gameplay,
                "Walk Speed",
                walk_speed,
                1,
                "Your character's walk speed; the game's normal speed is 8"
            ),
            b!(
                Gameplay,
                "Skip Cutscenes",
                skip_cutscenes,
                "Skips cutscenes as they start"
            ),
            b!(
                Gameplay,
                "Disable Death Ads",
                disable_death_ads,
                "Hides the ad shown when you die. On by default: each ad stays in memory, so a bot that dies often keeps growing"
            ),
            b!(
                Gameplay,
                "Headless Mode",
                headless_mode,
                "Covers the game and stops drawing it (2 FPS, low quality, muted) to save CPU; the bot keeps playing. Not the same as --headless, which runs Turu with no window"
            ),
        ],
    ));
    list.extend(grouped(
        "Privacy & appearance",
        vec![
            b!(
                Gameplay,
                "Streamer Mode",
                streamer_mode,
                "Hides other players, your name and guild, gold and chat, in game and in Turu's tabs and title, for streaming"
            ),
            entry(
                Section::Gameplay,
                "Custom Name",
                Field::Text(|o| &mut o.custom_name, "Empty: your real name"),
                "A name shown on your character, on your screen only",
            ),
            entry(
                Section::Gameplay,
                "Custom Guild",
                Field::Text(|o| &mut o.custom_guild, "Empty: your real guild"),
                "A guild name shown on your character, on your screen only",
            ),
        ],
    ));
    list.extend(grouped(
        "Timing & tries",
        vec![
            n!(
                Gameplay,
                "Action Delay",
                action_delay,
                0,
                "ms",
                "The pause between the bot's actions, such as accepting quests or moving items"
            ),
            n!(
                Gameplay,
                "Join Map Tries",
                join_map_tries,
                1,
                "Attempts to join a map before giving up"
            ),
            n!(
                Gameplay,
                "Load Timeout",
                load_timeout,
                1000,
                "ms",
                "How long a loading screen may stay up before Turu logs out (and relogs, with Auto Relogin)"
            ),
            n!(
                Gameplay,
                "Maximum Tries",
                maximum_tries,
                1,
                "Attempts to move an item between bank and inventory before giving up"
            ),
            n!(
                Gameplay,
                "Quest Accept And Complete Tries",
                quest_accept_and_complete_tries,
                1,
                "Attempts to accept or turn in a quest before giving up"
            ),
        ],
    ));
    list.push(b!(
        Gameplay,
        "Use Function Based Skills",
        use_function_based_skills,
        "In VibeSkua, the choice between skill files and function-based skills"
    ));
    // The unused options last in their section (a stable sort keeps the rest in order), under
    // their own heading.
    list.sort_by_key(|e| (e.section as usize, UNUSED.contains(&e.label)));
    for e in &mut list {
        if UNUSED.contains(&e.label) {
            e.group = UNUSED_GROUP;
        }
    }
    list
}

/// Hunt Priority's choices, as the combo box shows them.
pub fn priority_label(priority: HuntPriority) -> &'static str {
    match priority {
        HuntPriority::None => "None",
        HuntPriority::HighHP => "Highest HP",
        HuntPriority::LowHP => "Lowest HP",
        HuntPriority::Close => "Closest",
    }
}

/// "1 option", "3 options".
pub fn options(n: usize) -> String {
    if n == 1 {
        "1 option".to_owned()
    } else {
        format!("{n} options")
    }
}

impl Field {
    /// Whether `a` and `b` hold the same value for this option.
    pub fn same(&self, a: &mut BotOptions, b: &mut BotOptions) -> bool {
        match self {
            Field::Bool(access) => *access(a) == *access(b),
            Field::Int(access, ..) => *access(a) == *access(b),
            Field::Text(access, _) => *access(a) == *access(b),
            Field::Fps => a.set_fps == b.set_fps,
            Field::Server => a.relogin_server == b.relogin_server,
            Field::Priority => a.hunt_priority == b.hunt_priority,
            Field::ReloadMap => true,
        }
    }
}

/// The labels of the options whose values differ between `a` and `b`, in the window's order.
pub fn differing(entries: &[Entry], a: &BotOptions, b: &BotOptions) -> Vec<&'static str> {
    let (mut a, mut b) = (a.clone(), b.clone());
    entries
        .iter()
        .filter(|e| !e.field.same(&mut a, &mut b))
        .map(|e| e.label)
        .collect()
}

impl Field {
    /// Sets this option in `to` to its value in `from`.
    pub fn copy(&self, from: &BotOptions, to: &mut BotOptions) {
        let mut from = from.clone();
        match self {
            Field::Bool(access) => *access(to) = *access(&mut from),
            Field::Int(access, ..) => *access(to) = *access(&mut from),
            Field::Text(access, _) => *access(to) = std::mem::take(access(&mut from)),
            Field::Fps => to.set_fps = from.set_fps,
            Field::Server => to.relogin_server = from.relogin_server,
            Field::Priority => to.hunt_priority = from.hunt_priority,
            Field::ReloadMap => {}
        }
    }
}

/// Revert mine: `live` with the options changed here (`edited`) put back to `saved`, and
/// everything else (what a script set) left as it is.
pub fn revert_mine(
    entries: &[Entry],
    edited: &HashSet<&'static str>,
    live: &BotOptions,
    saved: &BotOptions,
) -> BotOptions {
    let mut result = live.clone();
    for entry in entries.iter().filter(|e| edited.contains(e.label)) {
        entry.field.copy(saved, &mut result);
    }
    result
}

/// What Defaults sets: the default options, but Relogin Try Delay and Relogin Watchdog, which
/// Application Options shows (and saves), stay as they are.
pub fn defaults(current: &BotOptions) -> BotOptions {
    BotOptions {
        relogin_try_delay: current.relogin_try_delay,
        relogin_watchdog: current.relogin_watchdog,
        ..BotOptions::default()
    }
}

/// Whether the search (`needle`, lowercase) finds `entry`: in its label or description, or as the
/// start of a word of its section's title ("combat", "network"). Matching anywhere in titles
/// would let "and" or "&" find every option.
pub fn matches(entry: &Entry, needle: &str) -> bool {
    needle.is_empty()
        || entry.label.to_lowercase().contains(needle)
        || entry.description.to_lowercase().contains(needle)
        || (needle.chars().count() >= 3
            && entry
                .section
                .title()
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .any(|word| word.starts_with(needle)))
}

/// Set FPS's ceiling: Ruffle runs up to 1000, but past the fastest phone and desktop displays
/// (240 Hz) the extra frames are never seen and only cost CPU.
pub const MAX_FPS: i32 = 240;

/// Set FPS's ▾: Off, AQW's own 24, then the usual display refresh rates.
pub const FPS_PRESETS: [i32; 9] = [0, 24, 30, 60, 90, 120, 144, 165, 240];

/// Options VibeSkua has that Turu stores but nothing reads yet (their descriptions say so): shown
/// greyed, as Application Options' "coming soon" rows.
pub const UNUSED: [&str; 3] = ["Hunt Buffer", "Hunt Priority", "Use Function Based Skills"];

/// The status line while the live options differ from `options.json`: who changed what. A
/// script's changes stay after it stops (but Auto Relogin, which stopping turns off), so Save
/// would keep them too; Revert puts the saved options back (a window asks first while the script
/// still runs). `edited`: what this window changed itself.
pub fn unsaved_status(
    entries: &[Entry],
    edited: &HashSet<&'static str>,
    live: &BotOptions,
    saved: &BotOptions,
    running: bool,
) -> String {
    let differs = differing(entries, live, saved);
    let mine = differs.iter().filter(|l| edited.contains(*l)).count();
    let outside: Vec<&str> = differs.iter().copied().filter(|l| !edited.contains(l)).collect();
    if outside.is_empty() {
        return match mine {
            0 => "Unsaved changes: they apply now; Save keeps them for next time.".to_owned(),
            1 => "You changed 1 option: it applies now; Save keeps it for next time.".to_owned(),
            n => format!("You changed {n} options: they apply now; Save keeps them for next time."),
        };
    }
    let names = match outside.as_slice() {
        [one] => format!("{one} was"),
        [a, b] => format!("{a} and {b} were"),
        [a, b, rest @ ..] => format!("{a}, {b} and {} more were", rest.len()),
        [] => unreachable!(),
    };
    let yours = if mine > 0 {
        format!(" You changed {}.", options(mine))
    } else {
        String::new()
    };
    let revert = if running {
        "Revert asks first while the script runs"
    } else {
        "Revert puts the saved options back"
    };
    format!("{names} changed outside this window, most likely by a script.{yours} Save keeps them too; {revert}.")
}

/// Hunt Priority's choices, in the drop-down's order.
pub const PRIORITIES: [HuntPriority; 4] = [
    HuntPriority::None,
    HuntPriority::HighHP,
    HuntPriority::LowHP,
    HuntPriority::Close,
];

/// Game Options for a shell: every option, live and saved, and what differs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct GameOptionsView {
    pub sections: Vec<SectionView>,
    pub options: Vec<OptionView>,
    /// Warnings about options that fight each other ("Accept All Drops and Reject All Drops are
    /// both on: ...").
    pub warnings: Vec<String>,
    /// How many options differ from `options.json`.
    pub unsaved: usize,
    /// Logged in (Reload Map needs it).
    pub in_game: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SectionView {
    /// Its short name, for a filter chip ("Combat").
    pub chip: &'static str,
    /// Its heading ("Combat & Targeting").
    pub title: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct OptionView {
    /// Its label, which is also its key for [`set`].
    pub label: &'static str,
    pub description: &'static str,
    /// The section's chip.
    pub section: &'static str,
    /// The small heading inside its section ("" for none).
    pub group: &'static str,
    pub kind: KindView,
    pub value: OptionValue,
    pub saved: OptionValue,
    pub differs: bool,
    /// It matters now: the option it depends on (Private Rooms for Private Number) is on.
    pub applies: bool,
    /// Turu stores it but nothing reads it yet.
    pub unused: bool,
}

/// An option's value, typed for the shells (a switch's, a number's, a text's or a choice's key).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(C)]
pub enum OptionValue {
    Switch(bool),
    Number(i64),
    /// Text, a server's name, or a choice's key ("HighHP").
    Text(String),
    /// An action's (Reload Map): none.
    None,
}

impl OptionValue {
    /// As JSON, as [`set`] takes it.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Switch(on) => json!(on),
            Self::Number(n) => json!(n),
            Self::Text(text) => json!(text),
            Self::None => Value::Null,
        }
    }
}

/// How an option is edited.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
#[repr(C)]
pub enum KindView {
    Switch,
    #[serde(rename_all = "camelCase")]
    Number {
        min: i32,
        unit: Option<&'static str>,
    },
    /// Set FPS: 0 is Off; `presets` for a menu.
    Fps {
        max: i32,
        presets: Vec<i32>,
    },
    Text {
        hint: &'static str,
    },
    /// A server's name (Relogin Server).
    Server,
    /// One of `choices`: (key, words).
    Choice {
        choices: Vec<(String, String)>,
    },
    /// A button (Reload Map), not a value; `login`: it needs one. (One-word fields in variants:
    /// facet-generate doesn't rename inside them as serde does.)
    Action {
        label: &'static str,
        login: bool,
    },
}

fn kind_view(field: &Field) -> KindView {
    match field {
        Field::Bool(_) => KindView::Switch,
        Field::Int(_, min, unit) => KindView::Number { min: *min, unit: *unit },
        Field::Fps => KindView::Fps {
            max: MAX_FPS,
            presets: FPS_PRESETS.to_vec(),
        },
        Field::Text(_, hint) => KindView::Text { hint },
        Field::Server => KindView::Server,
        Field::Priority => KindView::Choice {
            choices: PRIORITIES
                .iter()
                .map(|p| (format!("{p:?}"), priority_label(*p).to_owned()))
                .collect(),
        },
        Field::ReloadMap => KindView::Action {
            label: "Reload",
            login: true,
        },
    }
}

impl Field {
    /// This option's value in `options`, as JSON (`null` for an action).
    pub fn get(&self, options: &BotOptions) -> OptionValue {
        let mut options = options.clone();
        match self {
            Field::Bool(access) => OptionValue::Switch(*access(&mut options)),
            Field::Int(access, ..) => OptionValue::Number(i64::from(*access(&mut options))),
            Field::Text(access, _) => OptionValue::Text(access(&mut options).clone()),
            Field::Fps => OptionValue::Number(i64::from(options.set_fps)),
            Field::Server => OptionValue::Text(options.relogin_server.clone()),
            Field::Priority => OptionValue::Text(format!("{:?}", options.hunt_priority)),
            Field::ReloadMap => OptionValue::None,
        }
    }

    /// Sets this option in `options` from JSON, checked as its control checks it.
    pub fn set(&self, options: &mut BotOptions, value: &Value) -> Result<(), String> {
        let wrong = || format!("not a value for this option: {value}");
        match self {
            Field::Bool(access) => *access(options) = value.as_bool().ok_or_else(wrong)?,
            Field::Int(access, min, _) => {
                let n = value.as_i64().ok_or_else(wrong)?;
                *access(options) = i32::try_from(n).map_err(|_| wrong())?.max(*min);
            }
            Field::Fps => {
                let n = value.as_i64().ok_or_else(wrong)?;
                options.set_fps = n.clamp(0, MAX_FPS as i64) as i32;
            }
            Field::Text(access, _) => *access(options) = value.as_str().ok_or_else(wrong)?.to_owned(),
            Field::Server => options.relogin_server = value.as_str().ok_or_else(wrong)?.to_owned(),
            Field::Priority => options.hunt_priority = serde_json::from_value(value.clone()).map_err(|_| wrong())?,
            Field::ReloadMap => return Err("Reload Map is an action, not an option".into()),
        }
        Ok(())
    }
}

/// `options.json` as saved (the defaults when there's none, as the bot starts with).
pub fn saved(bot: &Bot) -> BotOptions {
    bot.paths.load_options().unwrap_or_default()
}

/// Game Options' view of `bot` now.
pub fn view(bot: &Bot) -> GameOptionsView {
    let live = bot.options.read().clone();
    let saved = saved(bot);
    let entries = entries();
    let differs = differing(&entries, &live, &saved);
    let mut warnings = Vec::new();
    if live.accept_all_drops && live.reject_all_drops {
        warnings.push(
            "Accept All Drops and Reject All Drops are both on: Accept All wins, so every drop is picked up.".into(),
        );
    }
    GameOptionsView {
        sections: Section::ALL
            .iter()
            .map(|s| SectionView {
                chip: s.chip(),
                title: s.title(),
            })
            .collect(),
        options: entries
            .iter()
            .map(|e| OptionView {
                label: e.label,
                description: e.description,
                section: e.section.chip(),
                group: e.group,
                kind: kind_view(&e.field),
                value: e.field.get(&live),
                saved: e.field.get(&saved),
                differs: differs.contains(&e.label),
                applies: e.requires.is_none_or(|requires| requires(&live)),
                unused: UNUSED.contains(&e.label),
            })
            .collect(),
        warnings,
        unsaved: differs.len(),
        in_game: bot.state.lock().in_game,
    }
}

/// Sets the option labelled `label` to `value`, live.
pub fn set(bot: &Bot, label: &str, value: &Value) -> Result<(), String> {
    let entries = entries();
    let entry = entries
        .iter()
        .find(|e| e.label == label)
        .ok_or_else(|| format!("no option named {label}"))?;
    if UNUSED.contains(&entry.label) {
        return Err(format!("Turu doesn't use {label} yet"));
    }
    if matches!(entry.field, Field::ReloadMap) {
        reload_map(bot);
        return Ok(());
    }
    entry.field.set(&mut bot.options.write(), value)
}

/// Save: the live options to `options.json`.
pub fn save(bot: &Bot) -> Result<(), String> {
    bot.save_options().map_err(|e| e.to_string())
}

/// Revert: the saved options back, all of them. With `edited`, only those (Revert mine).
pub fn revert(bot: &Bot, edited: Option<&HashSet<&'static str>>) {
    let saved = saved(bot);
    let mut options = bot.options.write();
    *options = match edited {
        Some(edited) => revert_mine(&entries(), edited, &options, &saved),
        None => saved,
    };
}

/// Defaults: every option back to its default (but the two Application Options shows).
pub fn reset(bot: &Bot) {
    let mut options = bot.options.write();
    *options = defaults(&options);
}

/// Reload Map: loads the current map again, on a bot thread.
pub fn reload_map(bot: &Bot) {
    if bot.state.lock().in_game {
        crate::turu::spawn_action("Reload Map", |bot| {
            turu_bot::api::dispatch(bot, "map.reload", &[]).map(|_| ())
        });
    }
}
