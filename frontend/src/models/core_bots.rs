//! Options > CoreBots (VibeSkua `CoreBotsView`): the per-character CoreBots Options file
//! (`options/CBO_Storage(<username>).txt`) that CoreBots' `setOptions()` reads. A [`Draft`] is
//! the window's state: which character, its file as read, the Loadout's roles (class, mode, gear),
//! the Options and Other switches; it loads when opened and saves when closed. The egui window
//! (`gui/turu/core_bots.rs`) holds one; a native shell holds one per tab in the slot ([`view`],
//! [`act`]), saved when its page closes, never a copy in the shell (it would be lost with the
//! panel, or saved to another character).

use super::auto::SkillSets;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use turu_bot::Bot;
use turu_bot::api::inventory::{self, Container};
use turu_bot::api::player;
use turu_bot::models::ItemBase;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// VibeSkua's `[Current]` class entry: the class equipped when the bot runs.
pub const CURRENT_CLASS: &str = "[Current]";

/// `ClassUseMode` (the values the file may hold for a mode).
pub const CLASS_USE_MODES: [&str; 8] = ["Base", "Atk", "Def", "Farm", "Solo", "Supp", "Dodge", "Ultra"];

#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Text(String),
}

impl Value {
    pub fn to_file(&self) -> String {
        match self {
            // C# bool.ToString()
            Value::Bool(b) => if *b { "True" } else { "False" }.to_owned(),
            Value::Int(n) => n.to_string(),
            Value::Text(s) => s.clone(),
        }
    }

    /// The file's `text` as this kind of value (VibeSkua `SetValues`; ints out of range become
    /// 100000); `None` when it doesn't parse.
    pub fn parse_like(&self, text: &str) -> Option<Value> {
        match self {
            Value::Bool(_) => match text.to_ascii_lowercase().as_str() {
                "true" => Some(Value::Bool(true)),
                "false" => Some(Value::Bool(false)),
                _ => None,
            },
            Value::Int(_) => Some(Value::Int(
                text.parse::<i64>()
                    .ok()
                    .and_then(|n| i32::try_from(n).ok())
                    .unwrap_or(100000),
            )),
            Value::Text(_) => Some(Value::Text(text.to_owned())),
        }
    }
}

pub struct Opt {
    pub label: &'static str,
    pub description: &'static str,
    /// Shown after a number ("ms").
    pub unit: &'static str,
    pub tag: &'static str,
    pub value: Value,
    pub default: Value,
}

pub fn opt(label: &'static str, tag: &'static str, default: Value) -> Opt {
    Opt {
        label,
        description: "",
        unit: "",
        tag,
        value: default.clone(),
        default,
    }
}

impl Opt {
    /// What the option does, from CoreBots' own comments on it (`CoreBots.cs`) or, where it has
    /// none, from what CoreBots and its farms do with it. VibeSkua shows none.
    fn about(self, description: &'static str) -> Self {
        Self { description, ..self }
    }

    fn unit(self, unit: &'static str) -> Self {
        Self { unit, ..self }
    }
}

/// The Options tab (VibeSkua `CoreBots.CreateOptions`), in file order.
pub fn options() -> Vec<Opt> {
    use Value::{Bool, Int, Text};
    vec![
        opt("Private Rooms", "PrivateRooms", Bool(true)).about("Join private rooms rather than public ones"),
        opt("Public on Difficult Parts", "PublicDifficult", Bool(false))
            .about("Use public rooms where the enemy is tough"),
        opt("Bank Misc. AC items on Start-Up", "BankMiscAC", Bool(true))
            .about("Clean your inventory by banking Misc. AC items before the bot starts"),
        opt("Logger in Chat", "LoggerInChat", Bool(true)).about("Also write the bot's log in AQW's chat"),
        opt("Force Off MessageBoxes", "MessageBoxCheck", Bool(false))
            .about("Show no message boxes unless absolutely necessary"),
        opt("Should Rest", "RestCheck", Bool(false)).about("Rest after killing a monster"),
        opt("Disable AutoEnhance", "DisableAutoEnhance", Bool(false))
            .about("Don't enhance a class when CoreBots equips it"),
        opt("Disable BestGear", "DisableBestGear", Bool(false))
            .about("Don't swap in your best gold, EXP, REP or class point gear while farming"),
        opt("Anti Lag", "AntiLag", Bool(true)).about("Lag killer, invisible monsters and 10 FPS while the bot runs"),
        opt("Incognito Mode", "IncognitoMode", Bool(true))
            .about("Turn off whispers and party, friend, duel and guild invites while the bot runs"),
        opt("Room Number", "PrivateRoomNr", Int(100000))
            .about("The private room's number; above 99999 picks a random room"),
        opt("Action Delay", "ActionDelayNr", Int(700))
            .about("Delay between common actions; 700 is the safe number")
            .unit("ms"),
        opt("Exit Combat Delay", "ExitCombatNr", Int(1600))
            .about("Delay to get out of combat; 1600 is the safe number")
            .unit("ms"),
        opt("Hunt Delay", "HuntDelayNr", Int(100))
            .about("Delay between jumping rooms while hunting; raise it if the bot jumps too much")
            .unit("ms"),
        opt("Accept and Complete Tries", "QuestTriesNr", Int(20))
            .about("How many times accepting or completing a quest is tried"),
        opt("Loaded Quests Max.", "QuestMaxNr", Int(150)).about("How many quests the bot can have loaded at once"),
        opt("Map after stopping the Bot", "StopLocationSelect", Text("Home".into()))
            .about("A map, Home (your house, else whitemap) or None to stay where you are"),
    ]
}

/// The Other tab (VibeSkua `CoreBots.CreateOtherOptions`): categories of switches.
pub fn other_options() -> Vec<(&'static str, Vec<Opt>)> {
    let b = |label, tag, default, description| opt(label, tag, Value::Bool(default)).about(description);
    vec![
        (
            "Boosters",
            vec![
                b(
                    "Use Gold Boosts when farming gold",
                    "doGoldBoost",
                    false,
                    "CoreFarms uses a Gold Boost from your inventory while it farms gold",
                ),
                b(
                    "Use Class Boosts when ranking up a class",
                    "doClassBoost",
                    false,
                    "CoreFarms uses a Class Boost while it ranks up a class",
                ),
                b(
                    "Use Reputation Boosts when farming REP",
                    "doRepBoost",
                    false,
                    "CoreFarms uses a Reputation Boost while it farms reputation",
                ),
                b(
                    "Use Experience Boosts when farming EXP",
                    "doExpBoost",
                    false,
                    "CoreFarms uses an Experience Boost while it farms EXP",
                ),
            ],
        ),
        (
            "Nation Farms",
            vec![
                b(
                    "Sell Voucher of Nulgath if not needed",
                    "Nation_SellMemVoucher",
                    true,
                    "Sell the Voucher of Nulgath when the farm doesn't need it",
                ),
                b(
                    "Do Swindles Return during Supplies",
                    "Nation_ReturnPolicyDuringSupplies",
                    true,
                    "Run Swindles Return Policy while farming Supplies, when you have the Unidentified items it needs",
                ),
                b(
                    "Ultra Alteon for Supplies",
                    "UltraAlteonForSupplies",
                    false,
                    "Use Ultra Alteon when farming Supplies instead of Escherion",
                ),
            ],
        ),
        (
            "Bludrut Brawl (PvP)",
            vec![b(
                "Kill ads before boss",
                "PvP_SoloPvPBoss",
                false,
                "Kill the brawlers and restorers in the PvP before the boss",
            )],
        ),
        (
            "Bot Creators Only",
            vec![b(
                "Story Bot Test Mode",
                "BCO_Story_TestBot",
                false,
                "Turn off CoreStory's isCompleteBefore checks; they stay on for quests you complete once",
            )],
        ),
    ]
}

/// One of the four class roles (Solo = 1, Farm = 2, Dodge = 3, Boss = 4 in the gear keys).
pub struct Role {
    pub name: &'static str,
    /// What the role is for, beside its name (VibeSkua's badge: "SINGLE TARGET", ...).
    pub purpose: &'static str,
    /// The role's dot.
    pub color: u32,
    pub class: Option<String>,
    /// The selected skill set mode (VibeSkua keeps the mode string and the `ClassUseMode`
    /// in sync; every shipped mode is a `ClassUseMode`).
    pub mode: Option<String>,
    pub modes: Vec<String>,
    pub use_equipment: bool,
    /// Helm, Armor, Cape, Weapon, Pet, Ground item.
    pub gear: [String; 6],
}

pub const SLOTS: [(&str, &str); 6] = [
    ("Helm", "Helm"),
    ("Armor", "Armor"),
    ("Cape", "Cape"),
    ("Weapon", "Weapon"),
    ("Pet", "Pet"),
    ("Ground", "GroundItem"),
];

pub fn roles() -> [Role; 4] {
    let role = |name, purpose, color: u32| Role {
        name,
        purpose,
        color,
        class: None,
        mode: None,
        modes: Vec::new(),
        use_equipment: false,
        gear: Default::default(),
    };
    [
        role("Solo", "Single target", 0x34D399),
        role("Farm", "AoE farming", 0xFBBF24),
        role("Dodge", "Survivability", 0x93C5FD),
        role("Boss", "Heavy bosses", 0xC4B5FD),
    ]
}

impl Role {
    /// VibeSkua `SelectedSoloClass` setter: lists the class's modes and picks the first when
    /// none is picked. `[Current]` with an equipped class that has no skill set offers `Base`
    /// and gives the class the default set (`EnsureSkillEntryExists`).
    pub fn select_class(&mut self, class: String, sets: &mut SkillSets, current_class: Option<&str>) {
        let resolved = if class == CURRENT_CLASS {
            current_class.unwrap_or_default()
        } else {
            &class
        };
        let mut modes = sets.modes_for(resolved);
        modes.sort();
        if modes.is_empty() && class == CURRENT_CLASS {
            modes.push("Base".into());
            if !resolved.is_empty() {
                sets.ensure(resolved);
            }
        }
        if self.mode.is_none() {
            self.mode = modes.first().cloned();
        }
        self.modes = modes;
        self.class = Some(class);
    }

    /// The mode as the file stores it (a `ClassUseMode` name).
    pub fn use_mode(&self) -> &'static str {
        self.mode
            .as_deref()
            .and_then(|m| CLASS_USE_MODES.iter().find(|u| u.eq_ignore_ascii_case(m)))
            .copied()
            .unwrap_or("Base")
    }
}

/// Why the game was read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// The window opened: load quietly.
    Open,
    Load,
    LoadCharacter,
    Save,
}

/// What a bot thread read from the game for the window.
pub struct Fetched {
    pub purpose: Purpose,
    pub username: String,
    pub current_class: Option<String>,
    pub items: Vec<ItemBase>,
}

/// The inventory lists the dropdowns offer (VibeSkua `PlayerClasses`, `Helms`, ...).
#[derive(Default)]
pub struct Owned {
    /// Classes with `[Current]` first.
    pub classes: Vec<String>,
    /// Per slot of [`SLOTS`].
    pub gear: [Vec<String>; 6],
}

pub struct Draft {
    pub username: String,
    pub current_class: Option<String>,
    /// The last inventory read, for the dropdown lists.
    pub items: Vec<ItemBase>,
    pub owned: Owned,
    pub sets: SkillSets,
    pub roles: [Role; 4],
    pub options: Vec<Opt>,
    pub other: Vec<(&'static str, Vec<Opt>)>,
    /// What was read from or saved to each character's file (VibeSkua `_readValues`).
    pub read_values: HashMap<String, Vec<(String, String)>>,
    pub fetched: Arc<Mutex<Option<Fetched>>>,
    pub busy: bool,
    pub message: Option<(&'static str, String)>,
    /// The window's lines as last read or saved: what "Unsaved changes" compares with (None
    /// until a character's file was looked for).
    pub baseline: Option<Vec<String>>,
    /// The character has a saved file (else the window shows CoreBots' defaults).
    pub has_file: bool,
    /// When Save last wrote the file: "Saved" shows next to the button for a moment.
    pub saved_at: Option<Instant>,
    /// The tab's data folder, from the bot the draft was read for.
    pub paths: Option<turu_bot::paths::Paths>,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            username: String::new(),
            current_class: None,
            items: Vec::new(),
            owned: Owned {
                classes: vec![CURRENT_CLASS.into()],
                ..Default::default()
            },
            sets: SkillSets::default(),
            roles: roles(),
            options: options(),
            other: other_options(),
            read_values: HashMap::new(),
            fetched: Arc::default(),
            busy: false,
            message: None,
            baseline: None,
            has_file: false,
            saved_at: None,
            paths: None,
        }
    }
}

impl Draft {
    fn cbo_path(&self) -> std::path::PathBuf {
        let paths = self.paths.clone().unwrap_or_else(|| crate::turu::bot().paths.clone());
        paths.cbo_file(&self.username)
    }

    /// Reads the username, the equipped class and the inventory on a bot thread; the result
    /// is applied on the next frame by [`Self::apply_fetched`].
    pub fn fetch(&mut self, bot: &std::sync::Arc<turu_bot::Bot>, purpose: Purpose) {
        self.busy = true;
        self.paths = Some(bot.paths.clone());
        let slot = self.fetched.clone();
        crate::turu::spawn_action_for(bot.clone(), "CoreBots Options", move |bot| {
            let username = if player::logged_in(bot).unwrap_or(false) {
                player::username(bot).unwrap_or_default()
            } else {
                String::new()
            };
            let (current_class, items) = if username.is_empty() {
                (None, Vec::new())
            } else {
                // Read straight from the engine: `inventory.items` serializes for scripts, with
                // other field names than `ItemBase` reads. A failure still answers, or the
                // window would stay busy.
                let items = inventory::items(bot, Container::Inventory).unwrap_or_default();
                (inventory::current_class(bot).ok().flatten(), items)
            };
            *lock(&slot) = Some(Fetched {
                purpose,
                username,
                current_class,
                items,
            });
            Ok(())
        });
    }

    pub fn apply_fetched(&mut self) {
        let Some(fetched) = lock(&self.fetched).take() else {
            return;
        };
        self.busy = false;
        let previous = std::mem::replace(&mut self.username, fetched.username);
        let switched = previous != self.username;
        let previous_was_empty = previous.is_empty();
        if switched && !previous_was_empty && self.has_changes() {
            // Another character now: the last one's changes are saved first, as closing would.
            let current = std::mem::replace(&mut self.username, previous);
            self.save(false);
            self.username = current;
        }
        self.current_class = fetched.current_class;
        if !self.username.is_empty() {
            self.items = fetched.items;
        }
        let logged_out = self.username.is_empty();
        match fetched.purpose {
            Purpose::Open => self.load(false),
            Purpose::Load => self.load(true),
            // Not over another character's file with the last one's values: its own first.
            Purpose::Save if switched && !previous_was_empty && !logged_out => {
                self.load(true);
                self.message = Some((
                    "Save",
                    format!(
                        "You're now on {}: the window shows its saved options. Change them and press Save again.",
                        self.username
                    ),
                ));
            }
            Purpose::Save => self.save(true),
            Purpose::LoadCharacter if logged_out => {
                self.message = Some((
                    "Refresh from game",
                    "Login first so that we can read your character's classes and equipment.".into(),
                ));
            }
            // A character the window hadn't loaded (logged in since, or another one): its file.
            Purpose::LoadCharacter if switched => self.load(true),
            // Not VibeSkua's, whose Reload Classes clears every role's class and mode: the lists
            // are read again and the picks kept.
            Purpose::LoadCharacter => self.refresh_keeping_picks(),
        }
    }

    /// Reads the class and gear lists again and keeps the window's picks (a pick no longer in
    /// the inventory stays, listed, as when a file names it).
    pub fn refresh_keeping_picks(&mut self) {
        let selections = turu_bot::cbo::parse(&self.lines());
        self.reload_classes();
        self.refresh_inventory();
        self.set_values(&selections);
    }

    /// VibeSkua `LoadInternal`.
    pub fn load(&mut self, show_dialog: bool) {
        if self.username.is_empty() {
            if show_dialog {
                self.message = Some((
                    "Load",
                    "Login first so that we can fetch your username to load the options file.".into(),
                ));
            }
            return;
        }
        // Not in VibeSkua (it fills the lists only when a file loads): the dropdowns list the
        // character's classes and gear as soon as the window knows them.
        self.reload_classes();
        self.refresh_inventory();
        if let Some(values) = self.read_values.get(&self.username).cloned() {
            self.set_values(&values);
            self.baseline = Some(self.lines());
            self.has_file = true;
            return;
        }
        let path = self.cbo_path();
        match turu_bot::cbo::read_lines(&path, |_, _| {}) {
            Ok(Some(lines)) => {
                let values = turu_bot::cbo::parse(&lines);
                self.set_values(&values);
                self.read_values.insert(self.username.clone(), values);
                self.baseline = Some(self.lines());
                self.has_file = true;
            }
            Ok(None) => {
                self.baseline = Some(self.lines());
                self.has_file = false;
            }
            Err(e) => {
                if show_dialog {
                    self.message = Some(("Load", format!("Couldn't read {}: {e}", path.display())));
                }
            }
        }
    }

    /// VibeSkua `SaveInternal`.
    pub fn save(&mut self, show_dialog: bool) {
        if self.username.is_empty() {
            if show_dialog {
                self.message = Some((
                    "Save",
                    "Login first so that we can fetch your username for the save file".into(),
                ));
            }
            return;
        }
        let path = self.cbo_path();
        // VibeSkua asks before saving public rooms; a save without the answer (closing the
        // window) keeps private rooms, in the window too so that it shows what was saved.
        if !self.public_confirmed() {
            for opt in self.options.iter_mut().filter(|o| o.tag == "PrivateRooms") {
                opt.value = Value::Bool(true);
            }
        }
        let lines = self.lines();
        let values = turu_bot::cbo::parse(&lines);
        let entries: Vec<(String, String)> = lines
            .iter()
            .map(|line| {
                let (k, v) = line.split_once(": ").unwrap_or((line, ""));
                (k.to_owned(), v.to_owned())
            })
            .collect();
        match turu_bot::cbo::write(&path, &entries) {
            Ok(()) => {
                // Said next to Save, not in a message box (VibeSkua's "Save Successful!").
                if show_dialog {
                    self.saved_at = Some(Instant::now());
                }
                self.read_values.insert(self.username.clone(), values);
                self.baseline = Some(self.lines());
                self.has_file = true;
            }
            Err(e) => self.message = Some(("Save", format!("Couldn't write {}: {e}", path.display()))),
        }
    }

    /// The file's lines, in VibeSkua's order: Loadout (classes, then gear), Options, Other.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for role in &self.roles {
            lines.push(format!(
                "{}ClassSelect: {}",
                role.name,
                role.class.as_deref().unwrap_or_default()
            ));
            lines.push(format!(
                "{}EquipCheck: {}",
                role.name,
                Value::Bool(role.use_equipment).to_file()
            ));
            lines.push(format!("{}ModeSelect: {}", role.name, role.use_mode()));
        }
        for (n, role) in self.roles.iter().enumerate() {
            for ((_, key), item) in SLOTS.iter().zip(&role.gear) {
                lines.push(format!("{key}{}Select: {item}", n + 1));
            }
        }
        for opt in &self.options {
            lines.push(format!("{}: {}", opt.tag, opt.value.to_file()));
        }
        for (_, opts) in &self.other {
            for opt in opts {
                lines.push(format!("{}: {}", opt.tag, opt.value.to_file()));
            }
        }
        lines
    }

    pub fn public_confirmed(&self) -> bool {
        lock(&PUBLIC_CONFIRMED).as_deref() == Some(self.username.as_str())
    }

    /// VibeSkua `SetValues` of every tab.
    pub fn set_values(&mut self, values: &[(String, String)]) {
        let get = |key: &str| values.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        let all = self
            .options
            .iter_mut()
            .chain(self.other.iter_mut().flat_map(|(_, opts)| opts.iter_mut()));
        for opt in all {
            opt.value = get(opt.tag)
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| opt.default.parse_like(v))
                .unwrap_or_else(|| opt.default.clone());
        }

        // CBOClassSelectViewModel.SetValues
        self.reload_classes();
        for i in 0..self.roles.len() {
            let name = self.roles[i].name;
            match get(&format!("{name}ClassSelect")) {
                Some(class) => {
                    if class != CURRENT_CLASS && !self.owned.classes.iter().any(|c| c == class) {
                        self.owned.classes.push(class.to_owned());
                        sort_classes(&mut self.owned.classes);
                    }
                    let role = &mut self.roles[i];
                    role.select_class(class.to_owned(), &mut self.sets, self.current_class.as_deref());
                    role.use_equipment =
                        get(&format!("{name}EquipCheck")).is_some_and(|v| v.eq_ignore_ascii_case("true"));
                    let mode = get(&format!("{name}ModeSelect"))
                        .and_then(|m| CLASS_USE_MODES.iter().find(|u| u.eq_ignore_ascii_case(m)))
                        .copied()
                        .unwrap_or("Base");
                    role.mode = Some(mode.to_owned());
                }
                None => {
                    let role = &mut self.roles[i];
                    role.class = Some(String::new());
                    role.modes.clear();
                    role.use_equipment = false;
                    role.mode = Some("Base".into());
                }
            }
        }

        // CBOClassEquipmentViewModel.SetValues
        for (n, role) in self.roles.iter_mut().enumerate() {
            for (slot, (_, key)) in SLOTS.iter().enumerate() {
                let item = get(&format!("{key}{}Select", n + 1)).unwrap_or_default().to_owned();
                if !item.is_empty() && !self.owned.gear[slot].contains(&item) {
                    self.owned.gear[slot].push(item.clone());
                }
                role.gear[slot] = item;
            }
        }
        for list in &mut self.owned.gear {
            list.sort_by_key(|name| name.to_lowercase());
        }
    }

    /// VibeSkua `ReloadClasses`: the enhanced classes in the inventory; clears the picks.
    pub fn reload_classes(&mut self) {
        let mut classes = vec![CURRENT_CLASS.to_owned()];
        classes.extend(
            self.items
                .iter()
                .filter(|i| i.category.eq_ignore_ascii_case("Class") && i.enhancement_level > 0)
                .map(|i| i.name.clone()),
        );
        sort_classes(&mut classes);
        self.owned.classes = classes;
        for role in &mut self.roles {
            role.class = None;
            role.mode = None;
            role.modes.clear();
        }
    }

    /// VibeSkua `RefreshInventory`: the gear each slot's dropdown offers.
    pub fn refresh_inventory(&mut self) {
        let names = |keep: &dyn Fn(&ItemBase) -> bool| {
            let mut names: Vec<String> = self.items.iter().filter(|i| keep(i)).map(|i| i.name.clone()).collect();
            names.sort_by_key(|name| name.to_lowercase());
            names
        };
        let category = |i: &ItemBase, c: &str| i.category.eq_ignore_ascii_case(c);
        self.owned.gear = [
            names(&|i| category(i, "Helm") && i.enhancement_level > 0),
            names(&|i| category(i, "Armor")),
            names(&|i| category(i, "Cape") && i.enhancement_level > 0),
            names(&|i| i.equip_slot == "Weapon" && i.enhancement_level > 0),
            names(&|i| category(i, "Pet")),
            names(&|i| category(i, "Misc")),
        ];
    }

    /// Whether the window differs from what was last read or saved.
    pub fn has_changes(&self) -> bool {
        self.baseline.as_ref().is_some_and(|saved| *saved != self.lines())
    }
}

/// The character whose public rooms were confirmed (None: not confirmed).
pub static PUBLIC_CONFIRMED: Mutex<Option<String>> = Mutex::new(None);

pub fn sort_classes(classes: &mut Vec<String>) {
    classes.retain(|c| c != CURRENT_CLASS);
    classes.sort_by_key(|c| c.to_lowercase());
    classes.insert(0, CURRENT_CLASS.to_owned());
}

/// The Options tab's groups: a heading and its options' tags, in the order shown (the file keeps
/// VibeSkua's order).
pub const OPTION_GROUPS: [(&str, &[&str]); 6] = [
    ("Rooms", &["PrivateRooms", "PrivateRoomNr", "PublicDifficult"]),
    ("Timing", &["ActionDelayNr", "ExitCombatNr", "HuntDelayNr"]),
    ("Quests", &["QuestTriesNr", "QuestMaxNr"]),
    (
        "Gear and inventory",
        &["DisableAutoEnhance", "DisableBestGear", "BankMiscAC"],
    ),
    (
        "While the bot runs",
        &[
            "AntiLag",
            "IncognitoMode",
            "LoggerInChat",
            "MessageBoxCheck",
            "RestCheck",
        ],
    ),
    ("When the bot stops", &["StopLocationSelect"]),
];

// A shell's page: one draft per tab, in the slot.

use super::form::{KindView, OptionValue, OptionView, SectionView};
use super::tab_state::PerTab;
use serde::{Deserialize, Serialize};

static DRAFTS: PerTab<Draft> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CoreBotsView {
    /// The character whose file this is ("" while logged out).
    pub username: String,
    /// How the page names it: `username`, or its tab's label ("Tab 1") in Streamer Mode.
    pub shown_name: String,
    /// Reading the game.
    pub busy: bool,
    /// The header's line: "Unsaved changes", "Saved", "CoreBots' defaults · not saved yet", ...
    pub state: String,
    pub unsaved: bool,
    /// A message for the user (a Load or Save that couldn't): (title, text).
    pub message: Option<(String, String)>,
    pub roles: Vec<RoleView>,
    /// The classes the roles can pick (`[Current]` first) and each gear slot's items.
    pub classes: Vec<String>,
    pub slots: Vec<SlotView>,
    /// The Options tab's groups and the Other tab's categories, as option rows (the tag is the
    /// row's label's key: [`OptionView::group`] holds it).
    pub sections: Vec<SectionView>,
    pub options: Vec<OptionView>,
    /// The sections that are the Other tab's (the rest are the Options tab's).
    pub other: Vec<String>,
    /// Private Rooms is off and its warning wasn't answered: Save asks first.
    pub ask_public: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct RoleView {
    pub name: String,
    pub purpose: String,
    /// Its dot, as 0xRRGGBB.
    pub color: u32,
    pub class: String,
    pub mode: String,
    pub modes: Vec<String>,
    pub use_equipment: bool,
    /// Per slot of [`SLOTS`].
    pub gear: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub name: String,
    pub items: Vec<String>,
}

fn option_view(o: &Opt, section: &'static str) -> OptionView {
    let value = match &o.value {
        Value::Bool(b) => OptionValue::Switch(*b),
        Value::Int(n) => OptionValue::Number(i64::from(*n)),
        Value::Text(t) => OptionValue::Text(t.clone()),
    };
    let kind = match &o.value {
        Value::Bool(_) => KindView::Switch,
        Value::Int(_) => KindView::Number {
            min: 0,
            max: None,
            unit: (!o.unit.is_empty()).then_some(o.unit),
        },
        Value::Text(_) => KindView::Text { hint: "" },
    };
    OptionView {
        label: o.label,
        description: o.description,
        section,
        group: o.tag,
        kind,
        saved: value.clone(),
        differs: o.value != o.default,
        value,
        applies: true,
        unused: false,
        restart: false,
    }
}

/// The page's view of `bot`'s draft (and what a bot thread read since).
/// The character as the page names it: its name, or its tab's label ("Tab 1") in Streamer Mode.
pub fn shown_name(bot: &Bot, username: &str) -> String {
    // Turu: VibeSkua's window names the character whatever Streamer Mode says.
    if bot.opts().streamer_mode && !username.trim().is_empty() {
        crate::turu::tab_label(bot)
    } else {
        username.to_owned()
    }
}

/// `text` naming the character as [`shown_name`] does.
pub fn shown_text(bot: &Bot, username: &str, text: &str) -> String {
    if bot.opts().streamer_mode && !username.trim().is_empty() {
        super::replace_word(text, username.trim(), &crate::turu::tab_label(bot))
    } else {
        text.to_owned()
    }
}

pub fn view(bot: &Arc<Bot>) -> CoreBotsView {
    let draft = DRAFTS.of(bot);
    let mut d = lock(&draft);
    d.apply_fetched();
    let unsaved = d.has_changes();
    let state = if d.busy {
        "Reading the game..."
    } else if d.username.is_empty() {
        "Not logged in · log in, then Refresh from game"
    } else if unsaved {
        "Unsaved changes · saved when you close this page"
    } else if d.has_file {
        "Saved"
    } else {
        "CoreBots' defaults · not saved yet"
    };
    let mut sections = Vec::new();
    let mut options = Vec::new();
    for (title, tags) in OPTION_GROUPS {
        sections.push(SectionView { chip: title, title });
        for tag in tags {
            if let Some(o) = d.options.iter().find(|o| o.tag == *tag) {
                options.push(option_view(o, title));
            }
        }
    }
    for (category, opts) in &d.other {
        sections.push(SectionView {
            chip: category,
            title: category,
        });
        options.extend(opts.iter().map(|o| option_view(o, category)));
    }
    let private = d
        .options
        .iter()
        .find(|o| o.tag == "PrivateRooms")
        .is_none_or(|o| o.value == Value::Bool(true));
    CoreBotsView {
        username: d.username.clone(),
        shown_name: shown_name(bot, &d.username),
        busy: d.busy,
        state: state.into(),
        unsaved,
        message: d
            .message
            .as_ref()
            .map(|(t, m)| ((*t).to_owned(), shown_text(bot, &d.username, m))),
        roles: d
            .roles
            .iter()
            .map(|r| RoleView {
                name: r.name.into(),
                purpose: r.purpose.into(),
                color: r.color,
                class: r.class.clone().unwrap_or_default(),
                mode: r.use_mode().into(),
                modes: r.modes.clone(),
                use_equipment: r.use_equipment,
                gear: r.gear.to_vec(),
            })
            .collect(),
        classes: d.owned.classes.clone(),
        slots: SLOTS
            .iter()
            .zip(&d.owned.gear)
            .map(|((name, _), items)| SlotView {
                name: (*name).into(),
                items: items.clone(),
            })
            .collect(),
        other: d.other.iter().map(|(c, _)| (*c).to_owned()).collect(),
        sections,
        options,
        ask_public: !private && !d.public_confirmed(),
    }
}

/// CoreBots' actions (`POST /models/core-bots/do`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum CoreBotsAct {
    /// Reads the game and the file.
    Open,
    /// Saves, as closing the egui window does.
    Close,
    Save,
    Revert,
    /// Reads the classes and gear again.
    Refresh,
    /// Private Rooms off, answered.
    ConfirmPublic,
    /// Dismisses the message.
    Dismiss,
    /// Sets the option tagged `tag`.
    Option {
        tag: String,
        value: OptionValue,
    },
    /// Changes the role at `position`: the fields given (`slot` with `item`).
    Role {
        position: u32,
        class: Option<String>,
        mode: Option<String>,
        equipment: Option<bool>,
        slot: Option<u32>,
        item: Option<String>,
    },
}

pub fn act(bot: &Arc<Bot>, act: CoreBotsAct) -> Result<(), String> {
    let draft = DRAFTS.of(bot);
    let mut d = lock(&draft);
    d.apply_fetched();
    let editable = !d.username.is_empty();
    match act {
        CoreBotsAct::Open => {
            d.sets = SkillSets::load_for(bot);
            d.fetch(bot, Purpose::Open);
        }
        CoreBotsAct::Close => {
            if editable {
                d.save(false);
            }
        }
        CoreBotsAct::Save => d.fetch(bot, Purpose::Save),
        CoreBotsAct::Refresh => d.fetch(bot, Purpose::LoadCharacter),
        CoreBotsAct::Revert => {
            let username = d.username.clone();
            d.read_values.remove(&username);
            d.load(true);
        }
        CoreBotsAct::ConfirmPublic => *lock(&PUBLIC_CONFIRMED) = Some(d.username.clone()),
        CoreBotsAct::Dismiss => d.message = None,
        CoreBotsAct::Option { .. } | CoreBotsAct::Role { .. } if !editable => {
            return Err("Log in first: the options are your character's".into());
        }
        CoreBotsAct::Option { tag, value } => {
            let d = &mut *d;
            let opt = d
                .options
                .iter_mut()
                .chain(d.other.iter_mut().flat_map(|(_, opts)| opts.iter_mut()))
                .find(|o| o.tag == tag)
                .ok_or_else(|| format!("no option {tag}"))?;
            opt.value = match (&opt.default, value) {
                (Value::Bool(_), OptionValue::Switch(b)) => Value::Bool(b),
                (Value::Int(_), OptionValue::Number(n)) => Value::Int(i32::try_from(n).map_err(|_| "not a number")?),
                (Value::Text(_), OptionValue::Text(t)) => Value::Text(t),
                (_, value) => return Err(format!("not a value for {tag}: {value:?}")),
            };
        }
        CoreBotsAct::Role {
            position,
            class,
            mode,
            equipment,
            slot,
            item,
        } => {
            let index = position as usize;
            let d = &mut *d;
            if index >= d.roles.len() {
                return Err("no such role".into());
            }
            if let Some(class) = class {
                d.roles[index].mode = None;
                let (sets, current) = (&mut d.sets, d.current_class.clone());
                d.roles[index].select_class(class, sets, current.as_deref());
            }
            if let Some(mode) = mode {
                d.roles[index].mode = Some(mode);
            }
            if let Some(on) = equipment {
                d.roles[index].use_equipment = on;
            }
            if let (Some(slot), Some(item)) = (slot, item) {
                let slot = slot as usize;
                if slot >= SLOTS.len() {
                    return Err("no such slot".into());
                }
                d.roles[index].gear[slot] = item;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Streamer Mode: the page names the character by its tab, and so do its messages.
    #[test]
    fn streamer_mode_hides_the_character() {
        let bot = Bot::new();
        let text = "You're now on Questwalker: the window shows its saved options.";
        assert_eq!(shown_name(&bot, "Questwalker"), "Questwalker");
        assert_eq!(shown_text(&bot, "Questwalker", text), text);
        bot.options.write().streamer_mode = true;
        assert!(shown_name(&bot, "Questwalker").starts_with("Tab"));
        assert!(!shown_text(&bot, "Questwalker", text).contains("Questwalker"));
        assert_eq!(shown_name(&bot, ""), "", "logged out: nothing to hide");
    }
}
