//! Options > HotKeys' model: VibeSkua's action catalog (`Skua.Core/AppStartup/HotKeys.cs`), the
//! key gestures bound to them (`HotKeys.json`, VibeSkua's `"Action|Gesture"` strings), and the
//! dispatch that runs them for the active tab. The window is `hotkeys_window.rs`.
//!
//! Keys reach Turu through egui, so a hotkey works while Turu's window has focus (VibeSkua's
//! main window handler; its Windows-only global hook isn't ported). Platforms without keyboard
//! shortcuts (`Platform::has_hotkeys` false) neither run nor show them; Android has them for a
//! hardware keyboard.

use super::TabUi;
use super::auto;
use egui::{Context, Key, Modifiers};
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use turu_bot::Bot;
use turu_bot::options::BotOptions;

/// The catalog's sections, in the order the window lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Script,
    Combat,
    Display,
    Player,
    GameUi,
    Army,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Script,
        Group::Combat,
        Group::Display,
        Group::Player,
        Group::GameUi,
        Group::Army,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Script => "Script & windows",
            Group::Combat => "Combat",
            Group::Display => "Display & performance",
            Group::Player => "Player & drops",
            Group::GameUi => "Game UI",
            Group::Army => "Army",
        }
    }

    /// The chip's label: one word, so the chips fit a narrow window.
    pub fn short(self) -> &'static str {
        match self {
            Group::Script => "Script",
            Group::Combat => "Combat",
            Group::Display => "Display",
            Group::Player => "Player",
            Group::GameUi => "Game UI",
            Group::Army => "Army",
        }
    }
}

/// What an action does.
#[derive(Clone, Copy)]
enum Run {
    /// Turu's own: windows, the script, Auto, the bank, one-off game actions.
    Own,
    /// Flips a Game option (`BotOptions`).
    Option(fn(&mut BotOptions) -> &mut bool),
    /// Flips an AQW Lite setting: `litePreference.data.<key>`, or `dOptions["<key>"]`.
    Lite(&'static str),
    LiteOption(&'static str),
    /// Flips a Game option here and sends the new value to every tab and Army client.
    ArmyOption(&'static str, fn(&mut BotOptions) -> &mut bool),
}

/// A bindable action: VibeSkua's name (saved), title and description.
pub struct Action {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub group: Group,
    run: Run,
}

macro_rules! option {
    ($field:ident) => {
        Run::Option(|o| &mut o.$field)
    };
}

macro_rules! army_option {
    ($field:ident, $name:literal) => {
        Run::ArmyOption($name, |o| &mut o.$field)
    };
}

const fn action(id: &'static str, title: &'static str, description: &'static str, group: Group, run: Run) -> Action {
    Action {
        id,
        title,
        description,
        group,
        run,
    }
}

use Group::{Army, Combat, Display, GameUi, Player, Script};

/// VibeSkua's actions (its `_actionList`), with its titles (`GetFormattedTitle`) and
/// descriptions (`GetDescription`, where it has one; Turu's otherwise).
pub static ACTIONS: &[Action] = &[
    // Script & windows
    action(
        "ToggleScript",
        "Start Script",
        "Starts the loaded script, or stops the running one.",
        Script,
        Run::Own,
    ),
    action(
        "StopScript",
        "Stop Script",
        "Stops the currently running bot script.",
        Script,
        Run::Own,
    ),
    action(
        "LoadScript",
        "Load Script",
        "Opens the file browser to load a new script.",
        Script,
        Run::Own,
    ),
    action(
        "SearchScripts",
        "Search Scripts",
        "Opens Search Scripts to find a script.",
        Script,
        Run::Own,
    ),
    action(
        "OpenConsole",
        "Open Console",
        "Opens the Console to run a snippet.",
        Script,
        Run::Own,
    ),
    action("OpenBank", "Open Bank", "Opens your bank interface.", Script, Run::Own),
    // Combat
    action(
        "ToggleAutoAttack",
        "Auto Attack",
        "Starts Auto Attack with the Auto popup's class, or stops it.",
        Combat,
        Run::Own,
    ),
    action(
        "ToggleAutoHunt",
        "Auto Hunt",
        "Starts Auto Hunt with the Auto popup's class, or stops it.",
        Combat,
        Run::Own,
    ),
    action(
        "ToggleAggroMonsters",
        "Aggro Monsters",
        "Provokes all monsters in the current room, causing them to attack you simultaneously.",
        Combat,
        option!(aggro_monsters),
    ),
    action(
        "ToggleAggroAllMonsters",
        "Aggro All Monsters",
        "Provokes all monsters in the MAP, causing them to attack you simultaneously.",
        Combat,
        option!(aggro_all_monsters),
    ),
    action(
        "ToggleAttackWithoutTarget",
        "Attack Without Target",
        "Uses skills even without a target. Use with caution.",
        Combat,
        option!(attack_without_target),
    ),
    action(
        "ToggleInfiniteRange",
        "Infinite Range",
        "Allows you to attack targets from any range without moving.",
        Combat,
        option!(infinite_range),
    ),
    action(
        "ToggleMagnetise",
        "Magnetise",
        "Teleports all targeted monsters directly to you.",
        Combat,
        option!(magnetise),
    ),
    action(
        "ToggleRestPackets",
        "Rest Packets",
        "Sends a rest packet every second, so you heal when not in combat.",
        Combat,
        option!(rest_packets),
    ),
    // Display & performance
    action(
        "ToggleLagKiller",
        "Lag Killer",
        "Disables drawing the world to reduce lag and CPU usage.",
        Display,
        option!(lag_killer),
    ),
    action(
        "ToggleHeadlessMode",
        "Headless Mode",
        "Hides the game renderer completely to drop CPU and GPU usage.",
        Display,
        option!(headless_mode),
    ),
    action(
        "ToggleStreamerMode",
        "Streamer Mode",
        "Anonymizes your name and guild and hides the room number.",
        Display,
        option!(streamer_mode),
    ),
    action(
        "ToggleHidePlayers",
        "Hide Players",
        "Hides all player avatars to reduce lag.",
        Display,
        option!(hide_players),
    ),
    action(
        "ToggleDisableFX",
        "Disable FX",
        "Disables all player combat animations to improve frame-rate.",
        Display,
        option!(disable_fx),
    ),
    action(
        "ToggleShowFPS",
        "Show FPS",
        "Toggles the in-game FPS counter.",
        Display,
        option!(show_fps),
    ),
    action(
        "ToggleSkipCutscenes",
        "Skip Cutscenes",
        "Skips cutscenes automatically.",
        Display,
        option!(skip_cutscenes),
    ),
    action(
        "ToggleDisableDeathAds",
        "Disable Death Ads",
        "Disables the death advertisement.",
        Display,
        option!(disable_death_ads),
    ),
    // Player & drops
    action(
        "PlayerRest",
        "Player Rest",
        "Makes your character sit and rest.",
        Player,
        Run::Own,
    ),
    action("JumpToHome", "Jump To Home", "Joins your house map.", Player, Run::Own),
    action(
        "Logout",
        "Logout",
        "Logs your character out to the login screen.",
        Player,
        Run::Own,
    ),
    action(
        "RejectAllDrops",
        "Reject All Drops",
        "Rejects every item in the drop list now.",
        Player,
        Run::Own,
    ),
    action(
        "ToggleRejectAllDrops",
        "Reject Drops Always",
        "Rejects all items that drop, until turned off.",
        Player,
        option!(reject_all_drops),
    ),
    // Game UI (AQW Lite settings)
    action(
        "ToggleCustomDropsUI",
        "Custom Drops UI",
        "Toggles the custom drops user interface.",
        GameUi,
        Run::Lite("bCustomDrops"),
    ),
    action(
        "ToggleDraggableDrops",
        "Draggable Drops",
        "Lets you drag the drops UI around the screen.",
        GameUi,
        Run::Lite("bDraggable"),
    ),
    action(
        "ToggleQuestLogTurnIns",
        "Quest Log Turn-Ins",
        "Allows turning in quests directly from the quest log.",
        GameUi,
        Run::Lite("bQuestLog"),
    ),
    action(
        "ToggleQuestPinner",
        "Quest Pinner",
        "Toggles the quest pinner.",
        GameUi,
        Run::Lite("bQuestPin"),
    ),
    action(
        "ToggleQuestProgressNotifications",
        "Quest Progress Notifications",
        "Toggles notifications for quest progress.",
        GameUi,
        Run::Lite("bQuestNotif"),
    ),
    action(
        "ToggleDisableQuestPopup",
        "Disable Quest Popup",
        "Disables the popup for quest completions.",
        GameUi,
        Run::Lite("bDisQPopup"),
    ),
    action(
        "ToggleDisableQuestTracker",
        "Disable Quest Tracker",
        "Disables the on-screen quest tracker.",
        GameUi,
        Run::Lite("bDisQTracker"),
    ),
    action(
        "ToggleChatUI",
        "Chat UI",
        "Toggles the chat UI.",
        GameUi,
        Run::Lite("bChatUI"),
    ),
    action(
        "ToggleChatFilter",
        "Chat Filter",
        "Toggles the chat filter.",
        GameUi,
        Run::Lite("bChatFilter"),
    ),
    action(
        "ToggleAurasUI",
        "Auras UI",
        "Toggles the class actives and auras UI.",
        GameUi,
        Run::Lite("bAuras"),
    ),
    action(
        "ToggleHideUI",
        "Hide UI",
        "Hides the game's interface completely.",
        GameUi,
        Run::Lite("bHideUI"),
    ),
    action(
        "ToggleVisualSkillCooldowns",
        "Visual Skill Cooldowns",
        "Toggles the cooldown indicators on skills.",
        GameUi,
        Run::Lite("bSkillCD"),
    ),
    action(
        "ToggleBattlePets",
        "Battle Pets",
        "Toggles battle pets.",
        GameUi,
        Run::Lite("bBattlepet"),
    ),
    action(
        "ToggleStaticPlayerArt",
        "Static Player Art",
        "Draws players as static art to improve performance.",
        GameUi,
        Run::Lite("bCachePlayers"),
    ),
    action(
        "ToggleDisableDamageNumbers",
        "Disable Damage Numbers",
        "Hides the floating damage numbers in combat.",
        GameUi,
        Run::Lite("bDisDmgDisplay"),
    ),
    action(
        "ToggleDisableDamageStrobe",
        "Disable Damage Strobe",
        "Disables the damage strobe effect.",
        GameUi,
        Run::Lite("bDisDmgStrobe"),
    ),
    action(
        "ToggleDisableRedWarning",
        "Disable Red Warning",
        "Disables the red screen warning.",
        GameUi,
        Run::LiteOption("disRed"),
    ),
    action(
        "ToggleDisableSoundFx",
        "Disable Sound FX",
        "Disables all sound effects in the game.",
        GameUi,
        Run::Lite("bDisSoundFX"),
    ),
    action(
        "ToggleDisableAuraAnimations",
        "Disable Aura Animations",
        "Disables aura animations.",
        GameUi,
        Run::Lite("bDisAuraAnim"),
    ),
    action(
        "ToggleDisableMonsterAnimation",
        "Disable Monster Animation",
        "Disables monster animations.",
        GameUi,
        Run::Lite("bDisMonAnim"),
    ),
    action(
        "ToggleDisableSelfAnimation",
        "Disable Self Animation",
        "Disables your own animations.",
        GameUi,
        Run::Lite("bDisSelfMAnim"),
    ),
    action(
        "ToggleDisableSkillAnimation",
        "Disable Skill Animation",
        "Stops skill animations to improve performance.",
        GameUi,
        Run::Lite("bDisSkillAnim"),
    ),
    action(
        "ToggleDisableWeaponAnimation",
        "Disable Weapon Animation",
        "Stops weapon animations.",
        GameUi,
        Run::Lite("bDisWepAnim"),
    ),
    action(
        "ToggleFreezeMonsterPosition",
        "Freeze Monster Position",
        "Keeps monsters where they stand.",
        GameUi,
        Run::Lite("bFreezeMons"),
    ),
    action(
        "ToggleInvisibleMonsters",
        "Invisible Monsters",
        "Makes all monsters invisible to improve performance.",
        GameUi,
        Run::Lite("bHideMons"),
    ),
    action(
        "ToggleShowMonsterType",
        "Show Monster Type",
        "Shows each monster's type.",
        GameUi,
        Run::Lite("bMonsType"),
    ),
    action(
        "ToggleHideGroundItems",
        "Hide Ground Items",
        "Hides items that drop on the ground.",
        GameUi,
        Run::Lite("bDisGround"),
    ),
    action(
        "ToggleShowYourGroundItemOnly",
        "Show Your Ground Item Only",
        "Shows only the ground items that are yours.",
        GameUi,
        Run::LiteOption("groundSelf"),
    ),
    action(
        "ToggleHideHealingBubbles",
        "Hide Healing Bubbles",
        "Hides the green healing bubbles.",
        GameUi,
        Run::Lite("bDisHealBubble"),
    ),
    action(
        "ToggleShowYourAuraAnimationOnly",
        "Show Your Aura Animation Only",
        "Shows only your own aura animations.",
        GameUi,
        Run::LiteOption("auraAnimSelf"),
    ),
    action(
        "ToggleHidePlayerNames",
        "Hide Player Names",
        "Hides other players' names.",
        GameUi,
        Run::Lite("bHideNames"),
    ),
    action(
        "ToggleHideGuildNamesOnly",
        "Hide Guild Names Only",
        "Hides only players' guild names.",
        GameUi,
        Run::LiteOption("hideGuild"),
    ),
    action(
        "ToggleHideYourNameOnly",
        "Hide Your Name Only",
        "Hides only your own name.",
        GameUi,
        Run::LiteOption("hideSelf"),
    ),
    action(
        "ToggleShowNameTags",
        "Show Name Tags",
        "Toggles name tags.",
        GameUi,
        Run::LiteOption("showNames"),
    ),
    action(
        "ToggleShowShadows",
        "Show Shadows",
        "Toggles shadows.",
        GameUi,
        Run::LiteOption("showShadows"),
    ),
    action(
        "ToggleUntargetDead",
        "Untarget Dead",
        "Untargets dead monsters automatically.",
        GameUi,
        Run::Lite("bUntargetDead"),
    ),
    action(
        "ToggleUntargetSelf",
        "Untarget Self",
        "Stops you from targeting yourself.",
        GameUi,
        Run::Lite("bUntargetSelf"),
    ),
    // Army: every tab and every account Army started
    action(
        "ArmyStartScripts",
        "Army Start Scripts",
        "Starts the loaded script on every tab and Army client.",
        Army,
        Run::Own,
    ),
    action(
        "ArmyStopScripts",
        "Army Stop Scripts",
        "Stops the script on every tab and Army client.",
        Army,
        Run::Own,
    ),
    action(
        "ArmyLoadScriptAll",
        "Load Script to All",
        "Opens Army Control to load one script on every client.",
        Army,
        Run::Own,
    ),
    action(
        "ArmyLoginAll",
        "Army Login All",
        "Logs in every tab and Army client.",
        Army,
        Run::Own,
    ),
    action(
        "ArmyLogoutAll",
        "Army Logout All",
        "Logs out every tab and Army client.",
        Army,
        Run::Own,
    ),
    action(
        "ArmyToggleLagKiller",
        "Army Lag Killer",
        "Toggles Lag Killer here and on every client.",
        Army,
        army_option!(lag_killer, "lagKiller"),
    ),
    action(
        "ArmyToggleHeadlessMode",
        "Army Headless Mode",
        "Toggles Headless Mode here and on every client.",
        Army,
        army_option!(headless_mode, "headlessMode"),
    ),
    action(
        "ArmyToggleHidePlayers",
        "Army Hide Players",
        "Toggles Hide Players here and on every client.",
        Army,
        army_option!(hide_players, "hidePlayers"),
    ),
    action(
        "ArmyToggleDisableFX",
        "Army Disable FX",
        "Toggles Disable FX here and on every client.",
        Army,
        army_option!(disable_fx, "disableFx"),
    ),
    action(
        "ArmyToggleInfiniteRange",
        "Army Infinite Range",
        "Toggles Infinite Range here and on every client.",
        Army,
        army_option!(infinite_range, "infiniteRange"),
    ),
    action(
        "ArmyToggleUseFunctionBasedSkills",
        "Army Function-Based Skills",
        "Toggles Use Function-Based Skills here and on every client.",
        Army,
        army_option!(use_function_based_skills, "useFunctionBasedSkills"),
    ),
    action(
        "ArmyToggleStreamerMode",
        "Army Streamer Mode",
        "Toggles Streamer Mode here and on every client.",
        Army,
        army_option!(streamer_mode, "streamerMode"),
    ),
];

/// The action named `id` (VibeSkua's name, any case).
pub fn find(id: &str) -> Option<&'static Action> {
    ACTIONS.iter().find(|a| a.id.eq_ignore_ascii_case(id))
}

/// A key with modifiers: VibeSkua's `KeyGesture` ("Ctrl+Shift+F5"). Ctrl is Cmd on macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gesture {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Gesture {
    pub const fn plain(key: Key) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    /// Reads VibeSkua's text: modifiers then the key, by egui's name or VibeSkua's (Avalonia's
    /// `D1`, `NumPad1`, `OemMinus`, ...), any case.
    pub fn parse(text: &str) -> Option<Self> {
        let mut gesture = Self::plain(Key::A);
        let mut rest = text.trim();
        loop {
            let lower = rest.to_ascii_lowercase();
            let skip = if lower.starts_with("ctrl+") || lower.starts_with("cmd+") {
                gesture.ctrl = true;
                rest.find('+')
            } else if lower.starts_with("ctl+") {
                gesture.ctrl = true;
                Some(3)
            } else if lower.starts_with("shift+") {
                gesture.shift = true;
                Some(5)
            } else if lower.starts_with("alt+") {
                gesture.alt = true;
                Some(3)
            } else {
                break;
            };
            rest = rest[skip? + 1..].trim_start();
        }
        gesture.key = key_named(rest)?;
        Some(gesture)
    }

    /// The saved text (VibeSkua's order: Ctrl, Shift, Alt).
    pub fn text(self) -> String {
        self.with_ctrl("Ctrl")
    }

    /// As the keycap shows it: "Cmd" for Ctrl on macOS.
    pub fn label(self) -> String {
        self.with_ctrl(if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" })
    }

    fn with_ctrl(self, ctrl: &str) -> String {
        let mut text = String::new();
        if self.ctrl {
            text += ctrl;
            text += "+";
        }
        if self.shift {
            text += "Shift+";
        }
        if self.alt {
            text += "Alt+";
        }
        text + self.key.name()
    }

    /// The gesture of a key press, if the key can be bound (not a modifier alone, Escape, or a
    /// clipboard key).
    pub fn of_press(key: Key, modifiers: Modifiers) -> Option<Self> {
        let unbindable = matches!(
            key,
            Key::Escape
                | Key::Copy
                | Key::Cut
                | Key::Paste
                | Key::ShiftLeft
                | Key::ShiftRight
                | Key::ControlLeft
                | Key::ControlRight
                | Key::AltLeft
                | Key::AltRight
                | Key::SuperLeft
                | Key::SuperRight
        );
        (!unbindable).then_some(Self {
            key,
            ctrl: modifiers.command,
            shift: modifiers.shift,
            alt: modifiers.alt,
        })
    }

    /// Whether a press with `modifiers` is this gesture: the same modifiers, no more.
    fn matches(self, key: Key, modifiers: Modifiers) -> bool {
        // On macOS, Control (not Cmd) is a modifier gestures can't hold: it must be up.
        let stray_ctrl = cfg!(target_os = "macos") && modifiers.ctrl;
        key == self.key
            && modifiers.command == self.ctrl
            && modifiers.shift == self.shift
            && modifiers.alt == self.alt
            && !stray_ctrl
    }

    /// Whether it also types: a letter, digit, punctuation or Space without Ctrl or Alt. It
    /// fires while you type in the game's chat too (the window says so).
    pub fn types(self) -> bool {
        !self.ctrl && !self.alt && !is_function_key(self.key) && !is_navigation_key(self.key)
    }
}

fn is_function_key(key: Key) -> bool {
    (Key::F1 as usize..=Key::F35 as usize).contains(&(key as usize))
}

fn is_navigation_key(key: Key) -> bool {
    matches!(
        key,
        Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::ArrowUp
            | Key::Insert
            | Key::Delete
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
            | Key::BrowserBack
    )
}

/// A key by egui's name or VibeSkua's.
fn key_named(name: &str) -> Option<Key> {
    let upper = name.trim().to_ascii_uppercase();
    let digit = |s: &str| {
        s.parse::<u8>()
            .ok()
            .filter(|d| *d <= 9)
            .and_then(|d| Key::from_name(&d.to_string()))
    };
    let alias = match upper.as_str() {
        "" => return None,
        "/" | "DIVIDE" | "OEMQUESTION" => Key::Slash,
        "-" | "SUBTRACT" | "OEMMINUS" => Key::Minus,
        "+" | "ADD" | "OEMPLUS" => Key::Plus,
        "." | "DECIMAL" | "OEMPERIOD" => Key::Period,
        "," | "OEMCOMMA" => Key::Comma,
        ";" | "OEMSEMICOLON" => Key::Semicolon,
        "'" | "OEMQUOTES" => Key::Quote,
        "`" | "OEMTILDE" => Key::Backtick,
        "[" | "OEMOPENBRACKETS" => Key::OpenBracket,
        "]" | "OEMCLOSEBRACKETS" => Key::CloseBracket,
        "\\" | "OEMPIPE" => Key::Backslash,
        "RETURN" => Key::Enter,
        "BACK" => Key::Backspace,
        "PRIOR" => Key::PageUp,
        "NEXT" => Key::PageDown,
        _ => {
            if let Some(d) = upper
                .strip_prefix("NUMPAD")
                .or(upper.strip_prefix("NUM"))
                .or(upper.strip_prefix('D'))
                && let Some(key) = digit(d)
            {
                return Some(key);
            }
            return Key::ALL
                .iter()
                .copied()
                .find(|k| k.name().eq_ignore_ascii_case(name.trim()));
        }
    };
    Some(alias)
}

/// An action and its key, if it has one ("Unassigned" in VibeSkua's text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub action: &'static str,
    pub gesture: Option<Gesture>,
}

impl Binding {
    pub fn action(&self) -> &'static Action {
        find(self.action).expect("bindings name known actions")
    }
}

/// VibeSkua's first-run bindings (`ClientSettings.InitializeDefaults`).
pub fn defaults() -> Vec<Binding> {
    [
        ("ToggleScript", Key::F10),
        ("LoadScript", Key::F9),
        ("OpenBank", Key::F2),
        ("OpenConsole", Key::F3),
        ("ToggleAutoAttack", Key::F4),
        ("ToggleAutoHunt", Key::F5),
        ("ToggleLagKiller", Key::F6),
    ]
    .into_iter()
    .map(|(action, key)| Binding {
        action: find(action).map(|a| a.id).unwrap_or(action),
        gesture: Some(Gesture::plain(key)),
    })
    .collect()
}

/// Reads VibeSkua's strings: unknown actions and repeats are dropped; a key that can't be read
/// leaves its action unassigned.
pub fn from_strings(lines: &[String]) -> Vec<Binding> {
    let mut bindings: Vec<Binding> = Vec::new();
    for line in lines {
        let (name, gesture) = line.split_once('|').unwrap_or((line, ""));
        let Some(action) = find(name.trim()) else { continue };
        if bindings.iter().any(|b| b.action == action.id) {
            continue;
        }
        bindings.push(Binding {
            action: action.id,
            gesture: Gesture::parse(gesture),
        });
    }
    bindings
}

/// VibeSkua's strings: `"Action|Gesture"`, `"Action|Unassigned"`.
pub fn to_strings(bindings: &[Binding]) -> Vec<String> {
    bindings
        .iter()
        .map(|b| format!("{}|{}", b.action, b.gesture.map_or("Unassigned".into(), |g| g.text())))
        .collect()
}

/// The bindings in `path`; VibeSkua's defaults when there's no file. A file that can't be read
/// is kept as `HotKeys.json.bad` and the defaults are used.
pub fn load(path: &Path) -> Vec<Binding> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return defaults();
    };
    match serde_json::from_str::<Vec<String>>(&text) {
        Ok(lines) => from_strings(&lines),
        Err(e) => {
            tracing::warn!("Couldn't read {}: {e}; using the default hotkeys", path.display());
            let _ = std::fs::rename(path, path.with_extension("json.bad"));
            defaults()
        }
    }
}

pub fn save(path: &Path, bindings: &[Binding]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&to_strings(bindings)).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

/// The app's bindings, shared by the tabs (read from `HotKeys.json` on first use).
static BINDINGS: Mutex<Option<Vec<Binding>>> = Mutex::new(None);

/// The last frame the window waited for a key to bind: the press is the window's then, not an
/// action's. (A frame number rather than a flag, so a tab switched away mid-wait can't leave
/// the hotkeys off.)
static CAPTURE_FRAME: AtomicU64 = AtomicU64::new(u64::MAX);

/// Called each frame the window waits for a key.
pub fn mark_capturing(ctx: &Context) {
    CAPTURE_FRAME.store(ctx.cumulative_frame_nr(), Ordering::Relaxed);
}

fn capturing(ctx: &Context) -> bool {
    let frame = CAPTURE_FRAME.load(Ordering::Relaxed);
    frame != u64::MAX && ctx.cumulative_frame_nr().saturating_sub(frame) <= 1
}

/// `f` of the bindings.
fn with_bindings<R>(f: impl FnOnce(&[Binding]) -> R) -> R {
    let mut guard = BINDINGS.lock().unwrap_or_else(PoisonError::into_inner);
    f(guard.get_or_insert_with(|| load(&crate::turu::bot().paths.hotkeys_file())))
}

/// A copy of the bindings.
pub fn bindings() -> Vec<Binding> {
    with_bindings(<[Binding]>::to_vec)
}

/// Replaces the bindings and writes them to `HotKeys.json`.
pub fn set_bindings(bindings: Vec<Binding>) -> std::io::Result<()> {
    let path = crate::turu::bot().paths.hotkeys_file();
    let result = save(&path, &bindings);
    *BINDINGS.lock().unwrap_or_else(PoisonError::into_inner) = Some(bindings);
    result
}

/// " (F10)": an action's key for a tooltip, or "" when it has none (or there are no hotkeys).
pub fn hint(action: &str) -> String {
    if !crate::platform::platform().has_hotkeys() {
        return String::new();
    }
    with_bindings(|bindings| {
        bindings
            .iter()
            .find(|b| b.action == action)
            .and_then(|b| b.gesture)
            .map_or(String::new(), |g| format!(" ({})", g.label()))
    })
}

/// The actions whose keys were pressed this frame; their key presses are taken from egui. A key
/// that types (no Ctrl or Alt) is left alone while a Turu text box has focus.
fn pressed(ctx: &Context) -> Vec<&'static str> {
    if !crate::platform::platform().has_hotkeys() || capturing(ctx) {
        return Vec::new();
    }
    let typing = ctx.egui_wants_keyboard_input();
    let bindings = bindings();
    let mut actions = Vec::new();
    ctx.input_mut(|input| {
        input.events.retain(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            else {
                return true;
            };
            let hit = bindings.iter().find(|b| {
                b.gesture.is_some_and(|g| {
                    g.matches(*key, *modifiers) && !(typing && (g.types() || is_navigation_key(g.key)))
                })
            });
            match hit {
                Some(binding) => {
                    actions.push(binding.action);
                    false
                }
                None => true,
            }
        });
    });
    actions
}

/// Runs the actions whose keys were pressed, for the active tab.
pub(super) fn run_pressed(ctx: &Context, tab: &mut TabUi) {
    for id in pressed(ctx) {
        if let Some(action) = find(id) {
            run(ctx, tab, action);
        }
    }
}

/// Runs `action` for the active tab (VibeSkua `ExecuteHotkeyAction`). What it did, or why it
/// did nothing, goes to the script log, since a key press has nothing else to show it.
fn run(ctx: &Context, tab: &mut TabUi, action: &Action) {
    let bot = crate::turu::bot();
    let log = |text: String| bot.log(format!("[HotKeys] {text}"));
    match action.run {
        Run::Option(field) => {
            let on = flip(&bot, field);
            log(format!("{} {}", action.title, on_off(on)));
        }
        Run::ArmyOption(name, field) => {
            let on = flip(&bot, field);
            log(format!("{} {} (sent to every client)", action.title, on_off(on)));
            let mut options = serde_json::Map::new();
            options.insert(name.into(), on.into());
            broadcast(turu_bot::army::Command::SetOptions(Value::Object(options)));
        }
        Run::Lite(key) => flip_lite(action.title, key, false),
        Run::LiteOption(key) => flip_lite(action.title, key, true),
        Run::Own => run_own(ctx, tab, action, &bot),
    }
}

fn run_own(ctx: &Context, tab: &mut TabUi, action: &Action, bot: &Arc<Bot>) {
    use turu_bot::api::{drops, map, player, servers};
    use turu_bot::army::Command;
    let w = &mut tab.windows;
    match action.id {
        "ToggleScript" => super::script_loader::toggle_script(),
        "StopScript" => {
            if bot.scripts.status().is_active() {
                turu_bot::script::stop(bot);
            }
        }
        "LoadScript" => {
            w.script_loader = true;
            if crate::platform::platform().has_file_dialogs() {
                tab.script_loader.pick_script(ctx);
            }
            raise(ctx, "Script Loader");
        }
        "SearchScripts" => {
            w.script_repo = true;
            raise(ctx, "Search Scripts");
        }
        "OpenConsole" => {
            w.console = true;
            raise(ctx, "Console");
        }
        "OpenBank" => tab.nav.open_bank(),
        "ToggleAutoAttack" => tab.nav.toggle_auto(auto::AutoMode::Attack),
        "ToggleAutoHunt" => tab.nav.toggle_auto(auto::AutoMode::Hunt),
        "PlayerRest" => crate::turu::spawn_action("Rest", |bot| player::rest(bot, false)),
        "JumpToHome" => crate::turu::spawn_action("Jump To Home", |bot| {
            map::join(bot, "home", "Enter", "Spawn", false, true)
        }),
        "Logout" => crate::turu::spawn_action("Logout", |bot| servers::logout(bot)),
        "RejectAllDrops" => crate::turu::spawn_action("Reject All Drops", |bot| drops::reject_all_drops(bot)),
        "ArmyStartScripts" => broadcast(Command::StartScript(None)),
        "ArmyStopScripts" => broadcast(Command::StopScript),
        "ArmyLoginAll" => broadcast(Command::Login),
        "ArmyLogoutAll" => broadcast(Command::Logout),
        "ArmyLoadScriptAll" => {
            w.army = true;
            raise(ctx, "Army Control");
        }
        other => tracing::warn!("Hotkey {other} has no action"),
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// Flips a Game option; returns its new value.
fn flip(bot: &Bot, field: fn(&mut BotOptions) -> &mut bool) -> bool {
    let mut options = bot.options.write();
    let value = field(&mut options);
    *value = !*value;
    *value
}

/// Flips an AQW Lite setting on a bot thread (it lives in the game).
fn flip_lite(title: &'static str, key: &'static str, in_options: bool) {
    use turu_bot::api::lite;
    crate::turu::spawn_action(title, move |bot| {
        let current = if in_options {
            lite::get_option(bot, key)?
        } else {
            lite::get(bot, key)?
        };
        let on = !current.as_ref().and_then(Value::as_bool).unwrap_or(false);
        if in_options {
            lite::set_option(bot, key, on)?;
        } else {
            lite::set(bot, key, on)?;
        }
        bot.log(format!("[HotKeys] {title} {}", on_off(on)));
        Ok(())
    });
}

/// Sends `command` to every tab and every account Army started; the outcomes go to the active
/// tab's log.
fn broadcast(command: turu_bot::army::Command) {
    let label = command.label();
    let bot = crate::turu::bot();
    bot.log(format!("[HotKeys] {label} on every client"));
    let local = crate::turu::tabs()
        .into_iter()
        .map(|(_, bot)| (crate::turu::tab_label(&bot), bot))
        .collect();
    crate::turu::army().broadcast(command, local, None, move |name, result| {
        if let Err(e) = result {
            bot.log(format!("[HotKeys] {label} failed on {name}: {e}"));
        }
    });
}

/// Puts the window titled `title` over the others.
fn raise(ctx: &Context, title: &str) {
    let id = egui::Id::new(("turu_window", title));
    ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
    ctx.request_repaint();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_is_listed_once() {
        for (i, a) in ACTIONS.iter().enumerate() {
            assert!(ACTIONS[..i].iter().all(|b| b.id != a.id), "{} twice", a.id);
            assert!(!a.description.is_empty() && !a.title.is_empty());
        }
        for b in defaults() {
            assert!(find(b.action).is_some(), "{}", b.action);
        }
    }

    #[test]
    fn reads_and_writes_vibeskua_gestures() {
        let read = |s| Gesture::parse(s).map(|g| g.text());
        assert_eq!(read("F10").as_deref(), Some("F10"));
        assert_eq!(read("ctrl+shift+c").as_deref(), Some("Ctrl+Shift+C"));
        assert_eq!(read("Alt+Shift+Ctrl+F5").as_deref(), Some("Ctrl+Shift+Alt+F5"));
        assert_eq!(read("Ctl+D1").as_deref(), Some("Ctrl+1"));
        assert_eq!(read("NumPad7").as_deref(), Some("7"));
        assert_eq!(read("OemMinus").as_deref(), Some("Minus"));
        assert_eq!(read("Shift+Space").as_deref(), Some("Shift+Space"));
        assert_eq!(read("Ctrl+"), None);
        assert_eq!(read("Unassigned"), None);
        assert_eq!(read(""), None);
    }

    #[test]
    fn strings_round_trip() {
        let lines: Vec<String> = [
            "ToggleScript|F10",
            "togglelagkiller|Ctrl+F6",
            "Nope|F1",
            "ToggleScript|F2",
            "OpenBank|Unassigned",
        ]
        .map(String::from)
        .to_vec();
        let bindings = from_strings(&lines);
        assert_eq!(
            to_strings(&bindings),
            ["ToggleScript|F10", "ToggleLagKiller|Ctrl+F6", "OpenBank|Unassigned"]
        );
    }

    #[test]
    fn matches_exact_modifiers() {
        let g = Gesture::parse("Shift+F5").unwrap();
        let shift = Modifiers::SHIFT;
        assert!(g.matches(Key::F5, shift));
        assert!(!g.matches(Key::F5, Modifiers::NONE));
        assert!(!g.matches(Key::F5, shift | Modifiers::ALT));
        assert!(Gesture::parse("Q").unwrap().types());
        assert!(!Gesture::parse("Alt+Q").unwrap().types());
        assert!(!Gesture::parse("F6").unwrap().types());
        assert_eq!(Gesture::of_press(Key::Escape, Modifiers::NONE), None);
    }
}
