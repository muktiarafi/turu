//! Options > HotKeys' model: VibeSkua's action catalog (`Skua.Core/AppStartup/HotKeys.cs`), the
//! key gestures bound to them (`HotKeys.json`, VibeSkua's `"Action|Gesture"` strings), editing
//! them (a key taken from another action moves, a removal can be undone, defaults restored), and
//! running an action for a tab. No egui: a key is its name (egui's, [`KEY_NAMES`]); the egui
//! window (`gui::turu::hotkeys_window`) and the key presses it reads (`gui::turu::hotkeys`) use it,
//! as a native shell could through `/models/hotkeys`.
//!
//! Keys reach Turu through egui, so a hotkey works while Turu's window has focus (VibeSkua's
//! main window handler; its Windows-only global hook isn't ported). Platforms without keyboard
//! shortcuts (`Host::has_hotkeys` false: both Android apps) neither run nor show them.

use super::tab_state::lock;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::{Arc, Mutex};
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
pub enum Run {
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
    pub run: Run,
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
        "TogglePlayerHud",
        "Player HUD",
        "Shows or hides the HP and MP of the players in your cell over the game.",
        Display,
        Run::Own,
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

/// egui's key names (`egui::Key::name`, in `Key::ALL`'s order), which gestures are saved and
/// matched by. A test in the egui window checks they still match egui's.
pub const KEY_NAMES: &[&str] = &[
    "Down",
    "Left",
    "Right",
    "Up",
    "Escape",
    "Tab",
    "Backspace",
    "Enter",
    "Insert",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Copy",
    "Cut",
    "Paste",
    "Space",
    "Colon",
    "Comma",
    "Minus",
    "Period",
    "Plus",
    "Equals",
    "Semicolon",
    "OpenBracket",
    "CloseBracket",
    "OpenCurlyBracket",
    "CloseCurlyBracket",
    "Backtick",
    "Backslash",
    "Slash",
    "Pipe",
    "Questionmark",
    "Exclamationmark",
    "Quote",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
    "F21",
    "F22",
    "F23",
    "F24",
    "F25",
    "F26",
    "F27",
    "F28",
    "F29",
    "F30",
    "F31",
    "F32",
    "F33",
    "F34",
    "F35",
    "BrowserBack",
    "ShiftLeft",
    "ShiftRight",
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "SuperLeft",
    "SuperRight",
    "IntlBackslash",
];

/// Keys a gesture can't be: a modifier alone, Escape (it cancels), the clipboard keys.
const UNBINDABLE: &[&str] = &[
    "Escape",
    "Copy",
    "Cut",
    "Paste",
    "ShiftLeft",
    "ShiftRight",
    "ControlLeft",
    "ControlRight",
    "AltLeft",
    "AltRight",
    "SuperLeft",
    "SuperRight",
];

/// Keys that move a caret, not type.
const NAVIGATION: &[&str] = &[
    "Down",
    "Left",
    "Right",
    "Up",
    "Insert",
    "Delete",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "BrowserBack",
];

/// A key with modifiers: VibeSkua's `KeyGesture` ("Ctrl+Shift+F5"). Ctrl is Cmd on macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gesture {
    /// One of [`KEY_NAMES`].
    pub key: &'static str,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Gesture {
    /// The key named `key` (one of [`KEY_NAMES`], any case) alone.
    pub fn plain(key: &str) -> Option<Self> {
        Some(Self {
            key: key_name(key)?,
            ctrl: false,
            shift: false,
            alt: false,
        })
    }

    /// A press of `key` with these modifiers, if the key can be bound.
    pub fn of_press(key: &str, ctrl: bool, shift: bool, alt: bool) -> Option<Self> {
        let key = key_name(key)?;
        (!UNBINDABLE.contains(&key)).then_some(Self { key, ctrl, shift, alt })
    }

    /// Reads VibeSkua's text: modifiers then the key, by egui's name or VibeSkua's (Avalonia's
    /// `D1`, `NumPad1`, `OemMinus`, ...), any case.
    pub fn parse(text: &str) -> Option<Self> {
        let (mut ctrl, mut shift, mut alt) = (false, false, false);
        let mut rest = text.trim();
        loop {
            let lower = rest.to_ascii_lowercase();
            let skip = if lower.starts_with("ctrl+") || lower.starts_with("cmd+") {
                ctrl = true;
                rest.find('+')
            } else if lower.starts_with("ctl+") {
                ctrl = true;
                Some(3)
            } else if lower.starts_with("shift+") {
                shift = true;
                Some(5)
            } else if lower.starts_with("alt+") {
                alt = true;
                Some(3)
            } else {
                break;
            };
            rest = rest[skip? + 1..].trim_start();
        }
        Some(Self {
            key: key_named(rest)?,
            ctrl,
            shift,
            alt,
        })
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
        text + self.key
    }

    /// Whether a press of `key` with these modifiers is this gesture: the same modifiers, no
    /// more. `stray_ctrl`: macOS's Control (not Cmd) is held, which no gesture can hold.
    pub fn matches(self, key: &str, ctrl: bool, shift: bool, alt: bool, stray_ctrl: bool) -> bool {
        key == self.key && ctrl == self.ctrl && shift == self.shift && alt == self.alt && !stray_ctrl
    }

    /// Whether it also types: a letter, digit, punctuation or Space without Ctrl or Alt. It
    /// fires while you type in the game's chat too (the window says so).
    pub fn types(self) -> bool {
        !self.ctrl && !self.alt && !is_function_key(self.key) && !self.navigates()
    }

    /// Whether its key moves a caret (left alone while a text box has focus).
    pub fn navigates(self) -> bool {
        NAVIGATION.contains(&self.key)
    }
}

fn is_function_key(key: &str) -> bool {
    key.strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=35).contains(&n))
}

/// The [`KEY_NAMES`] entry for `name`, any case.
fn key_name(name: &str) -> Option<&'static str> {
    KEY_NAMES.iter().copied().find(|k| k.eq_ignore_ascii_case(name.trim()))
}

/// A key by egui's name or VibeSkua's.
fn key_named(name: &str) -> Option<&'static str> {
    let upper = name.trim().to_ascii_uppercase();
    let digit = |s: &str| {
        s.parse::<u8>()
            .ok()
            .filter(|d| *d <= 9)
            .and_then(|d| key_name(&d.to_string()))
    };
    let alias = match upper.as_str() {
        "" => return None,
        "/" | "DIVIDE" | "OEMQUESTION" => "Slash",
        "-" | "SUBTRACT" | "OEMMINUS" => "Minus",
        "+" | "ADD" | "OEMPLUS" => "Plus",
        "." | "DECIMAL" | "OEMPERIOD" => "Period",
        "," | "OEMCOMMA" => "Comma",
        ";" | "OEMSEMICOLON" => "Semicolon",
        "'" | "OEMQUOTES" => "Quote",
        "`" | "OEMTILDE" => "Backtick",
        "[" | "OEMOPENBRACKETS" => "OpenBracket",
        "]" | "OEMCLOSEBRACKETS" => "CloseBracket",
        "\\" | "OEMPIPE" => "Backslash",
        "RETURN" => "Enter",
        "BACK" => "Backspace",
        "PRIOR" => "PageUp",
        "NEXT" => "PageDown",
        _ => {
            if let Some(d) = upper
                .strip_prefix("NUMPAD")
                .or(upper.strip_prefix("NUM"))
                .or(upper.strip_prefix('D'))
                && let Some(key) = digit(d)
            {
                return Some(key);
            }
            return key_name(name);
        }
    };
    key_name(alias)
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
        ("ToggleScript", "F10"),
        ("LoadScript", "F9"),
        ("OpenBank", "F2"),
        ("OpenConsole", "F3"),
        ("ToggleAutoAttack", "F4"),
        ("ToggleAutoHunt", "F5"),
        ("ToggleLagKiller", "F6"),
    ]
    .into_iter()
    .map(|(action, key)| Binding {
        action: find(action).map(|a| a.id).unwrap_or(action),
        gesture: Gesture::plain(key),
    })
    .collect()
}

/// Reads VibeSkua's strings: unknown actions and repeats are dropped; a key that can't be read
/// leaves its action unassigned.
pub fn from_strings(lines: &[String]) -> Vec<Binding> {
    let mut bindings: Vec<Binding> = Vec::new();
    for line in lines {
        let (name, gesture) = line.split_once('|').unwrap_or((line, ""));
        let Some(action) = find(name.trim()) else {
            continue;
        };
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

// --- the app's bindings ------------------------------------------------------------------------

/// The bindings, shared by the tabs (read from their file on first use), and the last removal.
pub struct Store {
    file: std::path::PathBuf,
    bindings: Mutex<Option<Vec<Binding>>>,
    /// The last binding removed and where it was, for Undo.
    removed: Mutex<Option<(usize, Binding)>>,
}

impl Store {
    pub fn new(file: std::path::PathBuf) -> Self {
        Self {
            file,
            bindings: Mutex::new(None),
            removed: Mutex::new(None),
        }
    }

    fn with<R>(&self, f: impl FnOnce(&mut Vec<Binding>) -> R) -> R {
        let mut guard = lock(&self.bindings);
        f(guard.get_or_insert_with(|| load(&self.file)))
    }

    /// A copy of the bindings.
    pub fn bindings(&self) -> Vec<Binding> {
        self.with(|b| b.clone())
    }

    /// Replaces the bindings and writes them to the file.
    pub fn set(&self, bindings: Vec<Binding>) -> std::io::Result<()> {
        let result = save(&self.file, &bindings);
        *lock(&self.bindings) = Some(bindings);
        result
    }

    /// Changes the bindings with `f` and saves them, answering what `f` answered.
    fn change<R>(&self, f: impl FnOnce(&mut Vec<Binding>) -> Result<R, String>) -> Result<R, String> {
        let mut bindings = self.bindings();
        let answer = f(&mut bindings)?;
        self.set(bindings)
            .map_err(|e| format!("Couldn't save the hotkeys: {e}"))?;
        Ok(answer)
    }

    /// Gives `action` the key `gesture`: an action holding it loses it (its title is answered).
    pub fn assign(&self, action: &str, gesture: Gesture) -> Result<Option<&'static str>, String> {
        self.change(|bindings| {
            let index = bindings
                .iter()
                .position(|b| b.action.eq_ignore_ascii_case(action))
                .ok_or("That action has no hotkey row")?;
            let mut moved = None;
            for (i, other) in bindings.iter_mut().enumerate() {
                if i != index && other.gesture == Some(gesture) {
                    other.gesture = None;
                    moved = Some(other.action().title);
                }
            }
            bindings[index].gesture = Some(gesture);
            Ok(moved)
        })
    }

    /// Takes `action`'s key away; true when it had one.
    pub fn clear_key(&self, action: &str) -> Result<bool, String> {
        self.change(|bindings| {
            let binding = bindings
                .iter_mut()
                .find(|b| b.action.eq_ignore_ascii_case(action))
                .ok_or("That action has no hotkey row")?;
            Ok(binding.gesture.take().is_some())
        })
    }

    /// Adds `action` from the catalog, with no key yet.
    pub fn add(&self, action: &str) -> Result<(), String> {
        let action = find(action).ok_or("No such action")?;
        self.change(|bindings| {
            if !bindings.iter().any(|b| b.action == action.id) {
                bindings.push(Binding {
                    action: action.id,
                    gesture: None,
                });
            }
            Ok(())
        })
    }

    /// Removes `action`'s row; [`Self::undo_remove`] puts it back.
    pub fn remove(&self, action: &str) -> Result<Binding, String> {
        let removed = self.change(|bindings| {
            let index = bindings
                .iter()
                .position(|b| b.action.eq_ignore_ascii_case(action))
                .ok_or("That action has no hotkey row")?;
            Ok((index, bindings.remove(index)))
        })?;
        *lock(&self.removed) = Some(removed.clone());
        Ok(removed.1)
    }

    /// Puts the last removed row back where it was, without a key another action took since.
    pub fn undo_remove(&self) -> Result<&'static Action, String> {
        let (index, mut binding) = lock(&self.removed).take().ok_or("Nothing to put back")?;
        self.change(|bindings| {
            if bindings
                .iter()
                .any(|b| b.gesture.is_some() && b.gesture == binding.gesture)
            {
                binding.gesture = None;
            }
            let action = binding.action();
            if !bindings.iter().any(|b| b.action == binding.action) {
                bindings.insert(index.min(bindings.len()), binding);
            }
            Ok(action)
        })
    }

    /// VibeSkua's defaults again.
    pub fn reset(&self) -> Result<(), String> {
        self.change(|bindings| {
            *bindings = defaults();
            Ok(())
        })
    }
}

/// The app's bindings. A test build never touches the real data folder's file.
pub static STORE: std::sync::LazyLock<Store> = std::sync::LazyLock::new(|| {
    Store::new(if cfg!(test) {
        std::env::temp_dir()
            .join(format!("turu-test-{}", std::process::id()))
            .join("HotKeys.json")
    } else {
        turu_bot::paths::Paths::new().hotkeys_file()
    })
});

/// " (F10)": an action's key for a tooltip, or "" when it has none (or there are no hotkeys).
pub fn hint(action: &str) -> String {
    if !crate::host::host().has_hotkeys() {
        return String::new();
    }
    STORE
        .bindings()
        .iter()
        .find(|b| b.action == action)
        .and_then(|b| b.gesture)
        .map_or(String::new(), |g| format!(" ({})", g.label()))
}

// --- running an action -------------------------------------------------------------------------

/// What an action needs of the UI it ran from: its windows, the bank, Auto, the script button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// Done here.
    Nothing,
    /// Start or stop the script, as the Script Loader's button does.
    ToggleScript,
    /// Open this window (`Windows::open`'s key); `pick` the Script Loader's file picker.
    Window {
        key: &'static str,
        title: &'static str,
        pick: bool,
    },
    OpenBank,
    /// Toggle Auto Attack (`hunt` false) or Auto Hunt.
    ToggleAuto {
        hunt: bool,
    },
}

/// Runs `action` for `bot`'s tab (VibeSkua `ExecuteHotkeyAction`), or says what the UI must do.
/// What it did, or why it did nothing, goes to the script log, since a key press has nothing
/// else to show it.
pub fn run(bot: &Arc<Bot>, action: &Action) -> Needs {
    let log = |text: String| bot.log(format!("[HotKeys] {text}"));
    match action.run {
        Run::Option(field) => {
            let on = flip(bot, field);
            log(format!("{} {}", action.title, on_off(on)));
        }
        Run::ArmyOption(name, field) => {
            let on = flip(bot, field);
            log(format!("{} {} (sent to every client)", action.title, on_off(on)));
            let mut options = serde_json::Map::new();
            options.insert(name.into(), on.into());
            broadcast(bot, turu_bot::army::Command::SetOptions(Value::Object(options)));
        }
        Run::Lite(key) => flip_lite(bot, action.title, key, false),
        Run::LiteOption(key) => flip_lite(bot, action.title, key, true),
        Run::Own => return run_own(bot, action),
    }
    Needs::Nothing
}

fn run_own(bot: &Arc<Bot>, action: &Action) -> Needs {
    use turu_bot::api::{drops, map, player, servers};
    use turu_bot::army::Command;
    let spawn = |name: &str, work: fn(&Arc<Bot>) -> turu_bot::BotResult<()>| {
        crate::turu::spawn_action_for(bot.clone(), name, work);
    };
    match action.id {
        "ToggleScript" => return Needs::ToggleScript,
        "TogglePlayerHud" => {
            let on = !bot.settings.read().show_player_hud;
            turu_bot::vitals::set_shown(bot, on);
            bot.log(format!("[HotKeys] {} {}", action.title, on_off(on)));
        }
        "StopScript" => {
            if bot.scripts.status().is_active() {
                turu_bot::script::stop(bot);
            }
        }
        "LoadScript" => {
            return Needs::Window {
                key: "script_loader",
                title: "Script Loader",
                pick: true,
            };
        }
        "SearchScripts" => {
            return Needs::Window {
                key: "script_repo",
                title: "Search Scripts",
                pick: false,
            };
        }
        "OpenConsole" => {
            return Needs::Window {
                key: "console",
                title: "Console",
                pick: false,
            };
        }
        "ArmyLoadScriptAll" => {
            return Needs::Window {
                key: "army",
                title: "Army Control",
                pick: false,
            };
        }
        "OpenBank" => return Needs::OpenBank,
        "ToggleAutoAttack" => return Needs::ToggleAuto { hunt: false },
        "ToggleAutoHunt" => return Needs::ToggleAuto { hunt: true },
        "PlayerRest" => spawn("Rest", |bot| player::rest(bot, false)),
        "JumpToHome" => spawn("Jump To Home", |bot| {
            map::join(bot, "home", "Enter", "Spawn", false, true)
        }),
        "Logout" => spawn("Logout", |bot| servers::logout(bot)),
        "RejectAllDrops" => spawn("Reject All Drops", |bot| drops::reject_all_drops(bot)),
        "ArmyStartScripts" => broadcast(bot, Command::StartScript(None)),
        "ArmyStopScripts" => broadcast(bot, Command::StopScript),
        "ArmyLoginAll" => broadcast(bot, Command::Login),
        "ArmyLogoutAll" => broadcast(bot, Command::Logout),
        other => tracing::warn!("Hotkey {other} has no action"),
    }
    Needs::Nothing
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
fn flip_lite(bot: &Arc<Bot>, title: &'static str, key: &'static str, in_options: bool) {
    use turu_bot::api::lite;
    crate::turu::spawn_action_for(bot.clone(), title, move |bot| {
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

/// Sends `command` to every tab (the shell's others too, in a client) and every account Army
/// started; the outcomes go to `bot`'s log.
fn broadcast(bot: &Arc<Bot>, command: turu_bot::army::Command) {
    let label = command.label();
    bot.log(format!("[HotKeys] {label} on every client"));
    let local = crate::turu::tabs()
        .into_iter()
        .map(|(_, bot)| (crate::turu::tab_label(&bot), bot))
        .collect();
    // In a shell's client, the shell's other tabs too (each in its own process).
    for peer in crate::turu::shell_peers() {
        let (bot, label, command) = (bot.clone(), label.clone(), command.clone());
        let _ = std::thread::Builder::new()
            .name("Turu Army Command".into())
            .spawn(move || {
                if let Err(e) = turu_bot::army::send_to_api(peer.api, &peer.token, &command) {
                    bot.log(format!("[HotKeys] {label} failed on {}: {e}", peer.label));
                }
            });
    }
    let bot = bot.clone();
    crate::turu::army().broadcast(command, local, None, move |name, result| {
        if let Err(e) = result {
            bot.log(format!("[HotKeys] {label} failed on {name}: {e}"));
        }
    });
}

// --- the view ----------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct HotKeysView {
    /// The platform has keyboard shortcuts (not Android).
    pub available: bool,
    pub bindings: Vec<HotKeyRow>,
    /// The actions with no row yet, in the catalog's order.
    pub catalog: Vec<HotKeyAction>,
    /// The groups' labels, in order.
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct HotKeyRow {
    pub action: HotKeyAction,
    /// "Ctrl+F5" (Cmd on macOS); empty with no key.
    pub key: String,
    /// The key also types (it fires in the game's chat too).
    pub types: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct HotKeyAction {
    pub id: String,
    pub title: String,
    pub description: String,
    pub group: String,
}

fn action_view(a: &Action) -> HotKeyAction {
    HotKeyAction {
        id: a.id.into(),
        title: a.title.into(),
        description: a.description.into(),
        group: a.group.label().into(),
    }
}

pub fn view() -> HotKeysView {
    let bindings = STORE.bindings();
    HotKeysView {
        available: crate::host::host().has_hotkeys(),
        catalog: ACTIONS
            .iter()
            .filter(|a| !bindings.iter().any(|b| b.action == a.id))
            .map(action_view)
            .collect(),
        bindings: bindings
            .iter()
            .map(|b| HotKeyRow {
                action: action_view(b.action()),
                key: b.gesture.map(|g| g.label()).unwrap_or_default(),
                types: b.gesture.is_some_and(|g| g.types()),
            })
            .collect(),
        groups: Group::ALL.iter().map(|g| g.label().to_owned()).collect(),
    }
}

/// Assign's answer: the action that lost the key (its title), or empty.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct KeyMoved {
    pub moved: String,
}

/// HotKeys' actions (`POST /models/hotkeys/do`); `id` is an action's ("OpenBank").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum HotKeysAct {
    /// Gives `key` (one of [`KEY_NAMES`]) with its modifiers to the action: answers `moved`, the
    /// action that lost the key, or empty.
    Assign {
        id: String,
        key: String,
        ctrl: bool,
        shift: bool,
        alt: bool,
    },
    Clear {
        id: String,
    },
    Add {
        id: String,
    },
    Remove {
        id: String,
    },
    UndoRemove,
    Reset,
}

pub fn act(store: &Store, act: HotKeysAct) -> Result<Value, String> {
    match act {
        HotKeysAct::Assign {
            id,
            key,
            ctrl,
            shift,
            alt,
        } => {
            let gesture = Gesture::of_press(&key, ctrl, shift, alt).ok_or("That key can't be a hotkey")?;
            let moved = store.assign(&id, gesture)?;
            serde_json::to_value(KeyMoved {
                moved: moved.unwrap_or_default().to_owned(),
            })
            .map_err(|e| e.to_string())
        }
        HotKeysAct::Clear { id } => store.clear_key(&id).map(|_| json!({})),
        HotKeysAct::Add { id } => store.add(&id).map(|()| json!({})),
        HotKeysAct::Remove { id } => store.remove(&id).map(|_| json!({})),
        HotKeysAct::UndoRemove => store.undo_remove().map(|_| json!({})),
        HotKeysAct::Reset => store.reset().map(|()| json!({})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        let dir = std::env::temp_dir().join(format!(
            "turu-hotkeys-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Store::new(dir.join("HotKeys.json"))
    }

    #[test]
    fn every_action_is_listed_once() {
        for (i, a) in ACTIONS.iter().enumerate() {
            assert!(ACTIONS[..i].iter().all(|b| b.id != a.id), "{} twice", a.id);
            assert!(!a.description.is_empty() && !a.title.is_empty());
        }
        for b in defaults() {
            assert!(find(b.action).is_some() && b.gesture.is_some(), "{}", b.action);
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
        assert!(g.matches("F5", false, true, false, false));
        assert!(!g.matches("F5", false, false, false, false));
        assert!(!g.matches("F5", false, true, true, false));
        assert!(!g.matches("F5", false, true, false, true));
        assert!(Gesture::parse("Q").unwrap().types());
        assert!(!Gesture::parse("Alt+Q").unwrap().types());
        assert!(!Gesture::parse("F6").unwrap().types());
        assert!(!Gesture::parse("Home").unwrap().types());
        assert_eq!(Gesture::of_press("Escape", false, false, false), None);
    }

    #[test]
    fn editing_moves_keys_and_undoes_a_removal() {
        let store = store();
        let f5 = Gesture::plain("F5").unwrap();
        // F5 is Auto Hunt's by default: giving it to Auto Attack takes it from there.
        assert_eq!(store.assign("ToggleAutoAttack", f5), Ok(Some("Auto Hunt")));
        let hunt = store
            .bindings()
            .into_iter()
            .find(|b| b.action == "ToggleAutoHunt")
            .unwrap();
        assert_eq!(hunt.gesture, None);
        store.remove("OpenBank").unwrap();
        assert!(!store.bindings().iter().any(|b| b.action == "OpenBank"));
        // F2 went to another action meanwhile: the bank comes back without it.
        store.add("OpenConsole").unwrap();
        store.assign("OpenConsole", Gesture::plain("F2").unwrap()).unwrap();
        store.undo_remove().unwrap();
        let bank = store.bindings().into_iter().find(|b| b.action == "OpenBank").unwrap();
        assert_eq!(bank.gesture, None);
        assert!(store.undo_remove().is_err());
        let escape = serde_json::from_value(json!({
            "action": "assign", "id": "OpenBank", "key": "Escape", "ctrl": false, "shift": false, "alt": false,
        }))
        .unwrap();
        assert_eq!(act(&store, escape), Err("That key can't be a hotkey".into()));
        store.reset().unwrap();
        assert_eq!(store.bindings(), defaults());
        // Saved as VibeSkua's strings.
        let saved: Vec<String> = serde_json::from_str(&std::fs::read_to_string(&store.file).unwrap()).unwrap();
        assert_eq!(saved, to_strings(&defaults()));
    }
}
