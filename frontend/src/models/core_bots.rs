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
