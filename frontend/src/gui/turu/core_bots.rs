//! Options > CoreBots (VibeSkua `CoreBotsView`, 800x608): the per-character CoreBots Options
//! file (`options/CBO_Storage(<username>).txt`) that CoreBots' `setOptions()` reads. Tabs:
//! Loadout (classes, modes and gear per role), Options and Other. Opening the window loads the
//! file; closing it saves. The header says whether the window holds unsaved changes.

use super::skills::SkillSets;
use super::theme::{ACCENT, CARD, EDGE, HOVER, INSET, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{
    Align, Color32, ComboBox, CornerRadius, Frame, Label, Layout, Margin, Modal, RichText, ScrollArea, Sense, Stroke,
    StrokeKind, Ui, UiBuilder, vec2,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::api::inventory::{self, Container};
use turu_bot::api::player;
use turu_bot::models::ItemBase;

/// VibeSkua's `[Current]` class entry: the class equipped when the bot runs.
const CURRENT_CLASS: &str = "[Current]";

/// `ClassUseMode` (the values the file may hold for a mode).
const CLASS_USE_MODES: [&str; 8] = ["Base", "Atk", "Def", "Farm", "Solo", "Supp", "Dodge", "Ultra"];

#[derive(Clone, PartialEq)]
enum Value {
    Bool(bool),
    Int(i32),
    Text(String),
}

impl Value {
    fn to_file(&self) -> String {
        match self {
            // C# bool.ToString()
            Value::Bool(b) => if *b { "True" } else { "False" }.to_owned(),
            Value::Int(n) => n.to_string(),
            Value::Text(s) => s.clone(),
        }
    }

    /// The file's `text` as this kind of value (VibeSkua `SetValues`; ints out of range become
    /// 100000); `None` when it doesn't parse.
    fn parse_like(&self, text: &str) -> Option<Value> {
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

struct Opt {
    label: &'static str,
    description: &'static str,
    /// Shown after a number ("ms").
    unit: &'static str,
    tag: &'static str,
    value: Value,
    default: Value,
}

fn opt(label: &'static str, tag: &'static str, default: Value) -> Opt {
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
fn options() -> Vec<Opt> {
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
fn other_options() -> Vec<(&'static str, Vec<Opt>)> {
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
struct Role {
    name: &'static str,
    /// What the role is for, beside its name (VibeSkua's badge: "SINGLE TARGET", ...).
    purpose: &'static str,
    /// The role's dot.
    color: Color32,
    class: Option<String>,
    /// The selected skill set mode (VibeSkua keeps the mode string and the `ClassUseMode`
    /// in sync; every shipped mode is a `ClassUseMode`).
    mode: Option<String>,
    modes: Vec<String>,
    use_equipment: bool,
    /// Helm, Armor, Cape, Weapon, Pet, Ground item.
    gear: [String; 6],
}

const SLOTS: [(&str, &str); 6] = [
    ("Helm", "Helm"),
    ("Armor", "Armor"),
    ("Cape", "Cape"),
    ("Weapon", "Weapon"),
    ("Pet", "Pet"),
    ("Ground", "GroundItem"),
];

fn roles() -> [Role; 4] {
    let role = |name, purpose, color: u32| Role {
        name,
        purpose,
        color: Color32::from_rgb((color >> 16) as u8, (color >> 8) as u8, color as u8),
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
    /// none is picked.
    fn select_class(&mut self, class: String, sets: &SkillSets, current_class: Option<&str>) {
        let resolved = if class == CURRENT_CLASS {
            current_class.unwrap_or_default()
        } else {
            &class
        };
        let mut modes = sets.modes_for(resolved);
        modes.sort();
        if modes.is_empty() && class == CURRENT_CLASS {
            modes.push("Base".into());
        }
        if self.mode.is_none() {
            self.mode = modes.first().cloned();
        }
        self.modes = modes;
        self.class = Some(class);
    }

    /// The mode as the file stores it (a `ClassUseMode` name).
    fn use_mode(&self) -> &'static str {
        self.mode
            .as_deref()
            .and_then(|m| CLASS_USE_MODES.iter().find(|u| u.eq_ignore_ascii_case(m)))
            .copied()
            .unwrap_or("Base")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Loadout,
    Options,
    Other,
}

/// Why the game was read.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Purpose {
    /// The window opened: load quietly.
    Open,
    Load,
    LoadCharacter,
    Save,
}

/// What a bot thread read from the game for the window.
struct Fetched {
    purpose: Purpose,
    username: String,
    current_class: Option<String>,
    items: Vec<ItemBase>,
}

/// The inventory lists the dropdowns offer (VibeSkua `PlayerClasses`, `Helms`, ...).
#[derive(Default)]
struct Owned {
    /// Classes with `[Current]` first.
    classes: Vec<String>,
    /// Per slot of [`SLOTS`].
    gear: [Vec<String>; 6],
}

pub struct CoreBotsWindow {
    was_open: bool,
    tab: Tab,
    search: String,
    equipment_open: bool,
    username: String,
    current_class: Option<String>,
    /// The last inventory read, for the dropdown lists.
    items: Vec<ItemBase>,
    owned: Owned,
    sets: SkillSets,
    roles: [Role; 4],
    options: Vec<Opt>,
    other: Vec<(&'static str, Vec<Opt>)>,
    /// What was read from or saved to each character's file (VibeSkua `_readValues`).
    read_values: HashMap<String, Vec<(String, String)>>,
    fetched: Arc<Mutex<Option<Fetched>>>,
    busy: bool,
    message: Option<(&'static str, String)>,
    /// Save is waiting for the public room warning's answer.
    confirm_public: bool,
    /// Revert to saved is waiting for its question's answer.
    confirm_revert: bool,
    /// The window's lines as last read or saved: what "Unsaved changes" compares with (None
    /// until a character's file was looked for).
    baseline: Option<Vec<String>>,
    /// The character has a saved file (else the window shows CoreBots' defaults).
    has_file: bool,
    /// When Save last wrote the file: "Saved" shows next to the button for a moment.
    saved_at: Option<Instant>,
}

impl Default for CoreBotsWindow {
    fn default() -> Self {
        Self {
            was_open: false,
            tab: Tab::Loadout,
            search: String::new(),
            equipment_open: false,
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
            confirm_public: false,
            confirm_revert: false,
            baseline: None,
            has_file: false,
            saved_at: None,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl CoreBotsWindow {
    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        if *open && !self.was_open {
            // VibeSkua OnActivated: load without explaining why when logged out.
            self.sets = SkillSets::load();
            self.fetch(Purpose::Open);
        } else if !*open && self.was_open && !self.username.is_empty() {
            // VibeSkua OnDeactivated: save without a dialog.
            self.save(false);
        }
        self.was_open = *open;
        self.apply_fetched();
        if !*open {
            return;
        }
        super::window(ctx, "CoreBots Options", [800.0, 608.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        self.dialogs(ctx);
    }

    /// Reads the username, the equipped class and the inventory on a bot thread; the result
    /// is applied on the next frame by [`Self::apply_fetched`].
    fn fetch(&mut self, purpose: Purpose) {
        self.busy = true;
        let slot = self.fetched.clone();
        crate::turu::spawn_action("CoreBots Options", move |bot| {
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

    fn apply_fetched(&mut self) {
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
    fn refresh_keeping_picks(&mut self) {
        let selections = turu_bot::cbo::parse(&self.lines());
        self.reload_classes();
        self.refresh_inventory();
        self.set_values(&selections);
    }

    /// VibeSkua `LoadInternal`.
    fn load(&mut self, show_dialog: bool) {
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
        let path = crate::turu::bot().paths.cbo_file(&self.username);
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
    fn save(&mut self, show_dialog: bool) {
        if self.username.is_empty() {
            if show_dialog {
                self.message = Some((
                    "Save",
                    "Login first so that we can fetch your username for the save file".into(),
                ));
            }
            return;
        }
        let path = crate::turu::bot().paths.cbo_file(&self.username);
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
    fn lines(&self) -> Vec<String> {
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

    fn public_confirmed(&self) -> bool {
        lock(&PUBLIC_CONFIRMED).as_deref() == Some(self.username.as_str())
    }

    /// VibeSkua `SetValues` of every tab.
    fn set_values(&mut self, values: &[(String, String)]) {
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
                    role.select_class(class.to_owned(), &self.sets, self.current_class.as_deref());
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
    fn reload_classes(&mut self) {
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
    fn refresh_inventory(&mut self) {
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

    fn ui(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
        self.header(ui);
        self.tab_row(ui);
        // Logged out there is no character to save to: what you changed would be lost when the
        // window closes, so the tabs only show the values.
        let editable = !self.username.is_empty();
        ui.add_enabled_ui(editable, |ui| match self.tab {
            Tab::Loadout => self.loadout_tab(ui),
            Tab::Options => self.options_tab(ui),
            Tab::Other => self.other_tab(ui),
        });
    }

    /// Whether the window differs from what was last read or saved.
    fn has_changes(&self) -> bool {
        self.baseline.as_ref().is_some_and(|saved| *saved != self.lines())
    }

    /// The header's second line: whose file this is and whether it's saved.
    fn save_state(&self) -> (String, Color32) {
        if self.busy {
            ("Reading the game...".into(), MUTED)
        } else if self.username.is_empty() {
            ("Not logged in · log in, then press Refresh from game".into(), MUTED)
        } else if self.has_changes() {
            ("Unsaved changes · saved when you close this window".into(), WARNING)
        } else if self.has_file {
            ("Saved".into(), MUTED)
        } else {
            ("CoreBots' defaults · not saved yet".into(), MUTED)
        }
    }

    /// The character, whether its options are saved, and the actions. The name and state get
    /// the width the buttons leave (cut with "…" past it); in a narrow window (a small screen,
    /// a phone) the buttons go on a row of their own under them.
    /// Returns where the text and the buttons went.
    fn header(&mut self, ui: &mut Ui) -> (egui::Rect, egui::Rect) {
        let narrow = ui.available_width() < HEADER_BUTTONS_WIDTH + HEADER_TEXT_MIN_WIDTH;
        let buttons = |this: &mut Self, ui: &mut Ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                this.header_buttons(ui);
                ui.min_rect()
            })
            .inner
        };
        if narrow {
            let text = self.header_text(ui, ui.available_width());
            (text, buttons(self, ui))
        } else {
            ui.horizontal(|ui| {
                let width = ui.available_width() - HEADER_BUTTONS_WIDTH;
                let text = self.header_text(ui, width);
                (text, buttons(self, ui))
            })
            .inner
        }
    }

    /// The character's name and the save state, `width` wide.
    fn header_text(&self, ui: &mut Ui, width: f32) -> egui::Rect {
        ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 2.0;
            let name = if self.username.is_empty() {
                RichText::new("No character").size(15.0).strong().color(MUTED)
            } else {
                RichText::new(&self.username).size(15.0).strong().color(TEXT)
            };
            ui.add(Label::new(name).truncate());
            let (state, color) = self.save_state();
            ui.add(Label::new(RichText::new(&state).size(12.0).color(color)).truncate())
                .on_hover_text(&state);
        })
        .response
        .rect
    }

    /// Save, Refresh from game and Revert to saved, right to left, with the spinner or "Saved".
    fn header_buttons(&mut self, ui: &mut Ui) {
        let enabled = !self.busy;
        if widgets::button(ui, "Save", Kind::Primary, enabled, vec2(84.0, 28.0), "")
            .on_hover_text("Save current CoreBots options")
            .clicked()
        {
            let public = self
                .options
                .iter()
                .any(|o| o.tag == "PrivateRooms" && o.value == Value::Bool(false));
            if public {
                self.confirm_public = true;
            } else {
                self.fetch(Purpose::Save);
            }
        }
        if widgets::button(ui, "Refresh from game", Kind::Normal, enabled, vec2(140.0, 28.0), "")
                .on_hover_text(
                    "Reads your classes and gear again and keeps your picks; for a character the \
                     window hasn't loaded, loads its saved options (VibeSkua: Load character, Reload Classes, Refresh Inventory)",
                )
                .clicked()
            {
                self.fetch(Purpose::LoadCharacter);
            }
        let changed = self.has_changes();
        if widgets::button(
            ui,
            "Revert to saved",
            Kind::Normal,
            enabled && changed,
            vec2(120.0, 28.0),
            "Nothing to revert: the window shows the saved options",
        )
        .on_hover_text("Puts back the options as last saved (VibeSkua: Load)")
        .clicked()
        {
            self.confirm_revert = true;
        }
        if self.busy {
            ui.spinner();
        } else if let Some(at) = self.saved_at {
            if at.elapsed() < SAVED_SHOWN_FOR {
                ui.label(RichText::new("Saved").size(12.0).color(SUCCESS));
                ui.ctx().request_repaint_after(SAVED_SHOWN_FOR - at.elapsed());
            } else {
                self.saved_at = None;
            }
        }
    }

    /// The tabs; the Options tab's search sits on the right.
    fn tab_row(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Loadout, "Loadout"),
                (Tab::Options, "Options"),
                (Tab::Other, "Other"),
            ] {
                if widgets::chip(ui, label, self.tab == tab) {
                    self.tab = tab;
                }
            }
            if self.tab == Tab::Options {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::text_box(ui, &mut self.search, "Search options", 220.0);
                });
            }
        });
    }

    fn loadout_tab(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .id_salt("cbo_loadout")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // The lists come from the game with Refresh from game (VibeSkua's Reload Classes and
                // Refresh Inventory did a part of it each, here).
                section_title(ui, "Classes");
                // 2x2 cards; `columns` splits the width, so the window never grows to fit them.
                for pair in [0, 2] {
                    ui.columns(2, |columns| {
                        for (column, i) in columns.iter_mut().zip(pair..) {
                            self.class_card(column, i);
                        }
                    });
                }

                let header =
                    egui::CollapsingHeader::new(RichText::new("Gear per role").size(13.0).strong().color(TEXT))
                        .id_salt("cbo_equipment")
                        .open(Some(self.equipment_open))
                        .show_unindented(ui, |ui| {
                            for pair in [0, 2] {
                                ui.columns(2, |columns| {
                                    for (column, i) in columns.iter_mut().zip(pair..) {
                                        self.gear_card(column, i);
                                    }
                                });
                            }
                        });
                if header.header_response.clicked() {
                    self.equipment_open = !self.equipment_open;
                }
                header
                    .header_response
                    .on_hover_text("Optional: what each role wears when its \"Use Dedicated Equipment Loadout\" is on");
            });
    }

    fn class_card(&mut self, ui: &mut Ui, i: usize) {
        let (sets, current_class, classes) = (&self.sets, self.current_class.as_deref(), &self.owned.classes);
        let role = &mut self.roles[i];
        role_card(ui, role, true, |ui, role| {
            caption(ui, "Selected Class");
            let mut picked = None;
            ComboBox::from_id_salt(("cbo_class", i))
                .selected_text(role.class.as_deref().unwrap_or_default())
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for class in classes {
                        if ui.selectable_label(role.class.as_ref() == Some(class), class).clicked() {
                            picked = Some(class.clone());
                        }
                    }
                });
            if let Some(class) = picked {
                role.select_class(class, sets, current_class);
            }
            caption(ui, "Skill Rule / Preset");
            ComboBox::from_id_salt(("cbo_mode", i))
                .selected_text(role.mode.as_deref().unwrap_or_default())
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for mode in &role.modes {
                        if ui.selectable_label(role.mode.as_ref() == Some(mode), mode).clicked() {
                            role.mode = Some(mode.clone());
                        }
                    }
                });
            ui.checkbox(
                &mut role.use_equipment,
                RichText::new("Use Dedicated Equipment Loadout").size(11.0),
            );
        });
    }

    fn gear_card(&mut self, ui: &mut Ui, i: usize) {
        let lists = &self.owned.gear;
        let role = &mut self.roles[i];
        role_card(ui, role, false, |ui, role| {
            for (slot, (label, _)) in SLOTS.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(vec2(52.0, 20.0), egui::Sense::hover());
                    ui.painter().text(
                        rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        *label,
                        egui::FontId::proportional(11.0),
                        MUTED,
                    );
                    let current = &mut role.gear[slot];
                    ComboBox::from_id_salt(("cbo_gear", i, slot))
                        .selected_text(current.as_str())
                        .width(ui.available_width())
                        .truncate()
                        .show_ui(ui, |ui| {
                            // An empty entry clears the slot (VibeSkua leaves it blank).
                            if ui.selectable_label(current.is_empty(), "(none)").clicked() {
                                current.clear();
                            }
                            for name in &lists[slot] {
                                if ui.selectable_label(current == name, name).clicked() {
                                    *current = name.clone();
                                }
                            }
                        });
                });
            }
        });
    }

    /// Two columns of option tiles: the switches, then the numbers and text, each by label.
    fn options_tab(&mut self, ui: &mut Ui) {
        let needle = self.search.trim().to_lowercase();
        let mut shown: Vec<&mut Opt> = self
            .options
            .iter_mut()
            .filter(|o| {
                needle.is_empty()
                    || o.label.to_lowercase().contains(&needle)
                    || o.description.to_lowercase().contains(&needle)
            })
            .collect();
        let rank = |o: &Opt| match o.value {
            Value::Bool(_) => 0,
            Value::Int(_) => 1,
            Value::Text(_) => 2,
        };
        shown.sort_by_key(|o| (rank(o), o.label.to_lowercase()));
        let (switches, values): (Vec<_>, Vec<_>) = shown.into_iter().partition(|o| matches!(o.value, Value::Bool(_)));
        list_frame(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("cbo_options")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if switches.is_empty() && values.is_empty() {
                        caption(ui, "No options match the search.");
                    }
                    for (title, mut group) in [("SWITCHES", switches), ("VALUES", values)] {
                        if group.is_empty() {
                            continue;
                        }
                        widgets::section_label(ui, title);
                        let column = (ui.available_width() - TILE_GAP) / 2.0;
                        for pair in group.chunks_mut(2) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = TILE_GAP;
                                for opt in pair {
                                    option_tile(ui, opt, column);
                                }
                            });
                        }
                        ui.add_space(6.0);
                    }
                });
        });
    }

    fn other_tab(&mut self, ui: &mut Ui) {
        list_frame(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("cbo_other")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (category, opts) in &mut self.other {
                        widgets::section_label(ui, &category.to_uppercase());
                        for opt in opts {
                            let width = ui.available_width();
                            option_tile(ui, opt, width);
                        }
                        ui.add_space(6.0);
                    }
                });
        });
    }

    /// VibeSkua's message boxes: the public room warning and the Load/Save messages.
    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm_public {
            let mut answer = None;
            Modal::new(egui::Id::new("cbo_public_rooms")).show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.heading("Public Room Warning");
                ui.label(
                    "Whilst we do offer the option, we highly recommend staying in private rooms while botting. \
                     Bot in public at your own risk.\nConfirm the use of Public Rooms?",
                );
                ui.horizontal(|ui| {
                    if widgets::button(ui, "Yes", Kind::Danger, true, vec2(72.0, 28.0), "").clicked() {
                        answer = Some(true);
                    }
                    if widgets::button(ui, "No", Kind::Normal, true, vec2(72.0, 28.0), "").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(yes) = answer {
                self.confirm_public = false;
                // The answer holds for this character until the window saves a different one.
                *lock(&PUBLIC_CONFIRMED) = yes.then(|| self.username.clone());
                self.fetch(Purpose::Save);
            }
        }
        if self.confirm_revert {
            let mut answer = None;
            Modal::new(egui::Id::new("cbo_revert")).show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.heading("Revert to saved?");
                ui.label("Your unsaved changes to this character's options will be lost.");
                ui.horizontal(|ui| {
                    if widgets::button(ui, "Revert", Kind::Danger, true, vec2(84.0, 28.0), "").clicked() {
                        answer = Some(true);
                    }
                    if widgets::button(ui, "Keep editing", Kind::Normal, true, vec2(110.0, 28.0), "").clicked() {
                        answer = Some(false);
                    }
                });
            });
            if let Some(revert) = answer {
                self.confirm_revert = false;
                if revert {
                    self.fetch(Purpose::Load);
                }
            }
        }
        if let Some((title, text)) = &self.message {
            let mut close = false;
            let response = Modal::new(egui::Id::new("cbo_message")).show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.heading(*title);
                ui.label(text.as_str());
                if widgets::button(ui, "OK", Kind::Primary, true, vec2(72.0, 28.0), "").clicked() {
                    close = true;
                }
            });
            if close || response.should_close() {
                self.message = None;
            }
        }
    }
}

/// The header's buttons with "Saved" beside them: Save, Refresh from game, Revert to saved.
const HEADER_BUTTONS_WIDTH: f32 = 84.0 + 140.0 + 120.0 + 3.0 * 8.0 + 44.0;
/// Below this much room left for the name and state, the buttons go on their own row.
const HEADER_TEXT_MIN_WIDTH: f32 = 300.0;

/// How long "Saved" shows next to Save.
const SAVED_SHOWN_FOR: Duration = Duration::from_secs(3);

/// The character whose public rooms were confirmed (None: not confirmed).
static PUBLIC_CONFIRMED: Mutex<Option<String>> = Mutex::new(None);

fn sort_classes(classes: &mut Vec<String>) {
    classes.retain(|c| c != CURRENT_CLASS);
    classes.sort_by_key(|c| c.to_lowercase());
    classes.insert(0, CURRENT_CLASS.to_owned());
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(11.0).color(MUTED));
}

/// A heading over the role cards.
fn section_title(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).strong().color(TEXT));
}

/// A role's card: its dot and name (with what the role is for on the class card), then
/// `add_contents`.
fn role_card(ui: &mut Ui, role: &mut Role, with_purpose: bool, add_contents: impl FnOnce(&mut Ui, &mut Role)) {
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            widgets::dot(ui, role.color, 4.0);
            ui.label(RichText::new(role.name).strong().size(13.0).color(TEXT));
            if with_purpose {
                ui.label(RichText::new(role.purpose).size(11.0).color(MUTED));
            }
        });
        add_contents(ui, role);
    });
}

fn list_frame(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(INSET)
        .stroke(Stroke::new(1.0, EDGE))
        .corner_radius(CornerRadius::same(super::theme::SMALL_RADIUS))
        .inner_margin(Margin::symmetric(12, 6))
        .show(ui, add_contents);
}

/// The space between two option tiles side by side.
const TILE_GAP: f32 = 10.0;

/// One option as a tile `width` wide: its label (and description) on the left, its control on
/// the right. The tile lights up under the pointer, and a click anywhere on a switch's tile
/// flips it, so each control plainly belongs to the label in its own tile. A number or text
/// that isn't the default gets a Reset beside it (a switch's other value is one click away).
fn option_tile(ui: &mut Ui, opt: &mut Opt, width: f32) {
    let height = if opt.description.is_empty() { 34.0 } else { 46.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let is_switch = matches!(opt.value, Value::Bool(_));
    let tile = ui.interact(
        rect,
        ui.make_persistent_id(("cbo_tile", opt.tag)),
        if is_switch { Sense::click() } else { Sense::hover() },
    );
    let hot = ui.rect_contains_pointer(rect);
    ui.painter().rect(
        rect,
        CornerRadius::same(super::theme::SMALL_RADIUS),
        if hot { HOVER } else { CARD },
        Stroke::new(1.0, if hot { ACCENT.gamma_multiply(0.6) } else { EDGE }),
        StrokeKind::Inside,
    );
    if is_switch && tile.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    // The description is cut to the tile's width: the whole of it under the pointer.
    let tile = if opt.description.is_empty() {
        tile
    } else {
        tile.on_hover_text(opt.description)
    };
    let inner = UiBuilder::new()
        .id_salt(opt.tag)
        .max_rect(rect.shrink2(vec2(12.0, 0.0)))
        .layout(Layout::right_to_left(Align::Center));
    let ui = &mut ui.new_child(inner);
    match &mut opt.value {
        Value::Bool(on) => {
            if tile.clicked() {
                *on = !*on;
            }
            widgets::switch(ui, on);
        }
        Value::Int(n) => {
            let speed = (n.unsigned_abs() as f64 / 200.0).max(1.0);
            let mut value = egui::DragValue::new(n).range(0..=i32::MAX).speed(speed);
            if !opt.unit.is_empty() {
                value = value.suffix(format!(" {}", opt.unit));
            }
            ui.add(value);
        }
        Value::Text(text) => {
            ui.add(
                egui::TextEdit::singleline(text)
                    .desired_width(140.0)
                    .margin(Margin::symmetric(8, 4)),
            );
        }
    }
    if !is_switch && opt.value != opt.default {
        let default = match &opt.default {
            Value::Int(n) if !opt.unit.is_empty() => format!("{n} {}", opt.unit),
            other => other.to_file(),
        };
        if widgets::small_button(ui, "Reset", true)
            .on_hover_text(format!("Back to the default, {default}"))
            .clicked()
        {
            opt.value = opt.default.clone();
        }
    }
    ui.add_space(10.0);
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        // Centre the text block vertically (top-down layouts start at the top).
        let lines = if opt.description.is_empty() { 1.0 } else { 2.0 };
        ui.add_space(((ui.available_height() - lines * 16.0) / 2.0).max(0.0));
        ui.add(
            Label::new(RichText::new(opt.label).color(TEXT))
                .truncate()
                .selectable(false),
        );
        if !opt.description.is_empty() {
            ui.add(
                Label::new(RichText::new(opt.description).size(11.0).color(MUTED))
                    .truncate()
                    .selectable(false),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The header tells a fresh file, saved values and unsaved changes apart, and logged out
    /// asks for a login.
    #[test]
    fn header_says_whether_the_options_are_saved() {
        let mut window = CoreBotsWindow::default();
        assert!(window.save_state().0.starts_with("Not logged in"));
        window.username = "Hero".into();
        window.baseline = Some(window.lines());
        assert_eq!(window.save_state().0, "CoreBots' defaults · not saved yet");
        window.has_file = true;
        assert_eq!(window.save_state().0, "Saved");
        window.options[0].value = Value::Bool(false);
        assert!(window.has_changes());
        assert!(window.save_state().0.starts_with("Unsaved changes"));
    }

    /// Every option says what it does, and the delays say they're milliseconds.
    #[test]
    fn every_option_is_described() {
        let window = CoreBotsWindow::default();
        let all = window
            .options
            .iter()
            .chain(window.other.iter().flat_map(|(_, opts)| opts));
        for opt in all {
            assert!(!opt.description.is_empty(), "{} has no description", opt.label);
            if opt.tag.ends_with("DelayNr") || opt.tag == "ExitCombatNr" {
                assert_eq!(opt.unit, "ms", "{}", opt.label);
            }
        }
    }

    /// Refreshing the class and gear lists keeps each role's picks (VibeSkua's Reload Classes
    /// cleared them), so it changes nothing to save.
    #[test]
    fn refresh_keeps_the_picks() {
        let mut window = CoreBotsWindow::default();
        window.username = "Hero".into();
        window.roles[1].select_class("Legion Revenant".into(), &SkillSets::default(), None);
        window.roles[1].mode = Some("Farm".into());
        window.roles[1].gear[3] = "Necrotic Sword of Doom".into();
        window.baseline = Some(window.lines());
        window.refresh_keeping_picks();
        assert_eq!(window.roles[1].class.as_deref(), Some("Legion Revenant"));
        assert_eq!(window.roles[1].mode.as_deref(), Some("Farm"));
        assert_eq!(window.roles[1].gear[3], "Necrotic Sword of Doom");
        assert!(!window.has_changes());
    }

    /// A long name and state are cut, and in a narrow window (a phone's) the buttons go on a row
    /// of their own: the text never runs under the buttons, and keeps room to be read.
    #[test]
    fn header_fits_narrow_windows() {
        for width in [800.0, 440.0] {
            let ctx = egui::Context::default();
            let mut window = CoreBotsWindow::default();
            window.username = "AVeryLongCharacterNameThatGoesOnAndOnForeverAndEver".into();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 1000.0))),
                ..Default::default()
            };
            let mut placed = None;
            let mut output = ctx.run_ui(input, |ui| {
                ui.allocate_ui(vec2(width, 200.0), |ui| {
                    ui.set_width(width);
                    placed = Some((window.header(ui), ui.max_rect()));
                });
            });
            output.textures_delta.clear();
            let ((text, buttons), area) = placed.unwrap();
            assert!(
                !text.intersects(buttons),
                "at {width}: text {text:?} under buttons {buttons:?}"
            );
            assert!(
                text.width() >= 200.0,
                "at {width}: the text has only {} px",
                text.width()
            );
            assert!(buttons.right() <= area.right() + 1.0 && text.right() <= area.right() + 1.0);
        }
    }

    /// The window keeps its width with long class and item names (egui windows grow to fit
    /// content that asks for more than it was given).
    #[test]
    fn loadout_fits_the_window() {
        let ctx = egui::Context::default();
        let mut window = CoreBotsWindow::default();
        let long = "Legendary Elemental Warrior of the Very Long Item Name Showcase".to_owned();
        window.owned.classes.push(long.clone());
        for list in &mut window.owned.gear {
            list.push(long.clone());
        }
        for role in &mut window.roles {
            role.class = Some(long.clone());
            role.mode = Some("Base".into());
            role.gear = std::array::from_fn(|_| long.clone());
        }
        window.equipment_open = true;
        let input = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 1000.0))),
            ..Default::default()
        };
        let mut widths = Vec::new();
        for tab in [Tab::Loadout, Tab::Options, Tab::Other] {
            window.tab = tab;
            for _ in 0..8 {
                let mut output = ctx.run_ui(input(), |ui| {
                    let ctx = ui.ctx().clone();
                    let shown = egui::Window::new("CoreBots Options")
                        .default_size(vec2(800.0, 608.0))
                        .resizable(true)
                        .show(&ctx, |ui| window.ui(ui));
                    widths.push(shown.unwrap().response.rect.width());
                });
                output.textures_delta.clear();
            }
        }
        let first = widths[0];
        assert!(widths.iter().all(|w| *w <= first + 1.0), "the window grew: {widths:?}");
        assert!(first < 840.0, "the window is wider than its default: {first}");
    }
}
