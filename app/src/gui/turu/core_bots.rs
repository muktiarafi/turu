//! Options > CoreBots (VibeSkua `CoreBotsView`, 800x608): the per-character CoreBots Options
//! file (`options/CBO_Storage(<username>).txt`) that CoreBots' `setOptions()` reads. Tabs:
//! Loadout (classes, modes and gear per role), Options and Other. Opening the window loads the
//! file; closing it saves.

use super::skills::SkillSets;
use super::theme::{EDGE, INSET, MUTED, TEXT};
use super::widgets::{self, Kind};
use egui::{
    Align, Color32, ComboBox, CornerRadius, Frame, Layout, Margin, Modal, RichText, ScrollArea, Stroke, Ui, vec2,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use turu_bot::api::{dispatch, inventory, player};
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
    tag: &'static str,
    value: Value,
    default: Value,
}

fn opt(label: &'static str, tag: &'static str, default: Value) -> Opt {
    Opt {
        label,
        description: "",
        tag,
        value: default.clone(),
        default,
    }
}

/// The Options tab (VibeSkua `CoreBots.CreateOptions`), in file order.
fn options() -> Vec<Opt> {
    use Value::{Bool, Int, Text};
    vec![
        opt("Private Rooms", "PrivateRooms", Bool(true)),
        opt("Public on Difficult Parts", "PublicDifficult", Bool(false)),
        opt("Bank Misc. AC items on Start-Up", "BankMiscAC", Bool(true)),
        opt("Logger in Chat", "LoggerInChat", Bool(true)),
        opt("Force Off MessageBoxes", "MessageBoxCheck", Bool(false)),
        opt("Should Rest", "RestCheck", Bool(false)),
        opt("Disable AutoEnhance", "DisableAutoEnhance", Bool(false)),
        opt("Disable BestGear", "DisableBestGear", Bool(false)),
        opt("Anti Lag", "AntiLag", Bool(true)),
        opt("Incognito Mode", "IncognitoMode", Bool(true)),
        opt("Room Number", "PrivateRoomNr", Int(100000)),
        opt("Action Delay", "ActionDelayNr", Int(700)),
        opt("Exit Combat Delay", "ExitCombatNr", Int(1600)),
        opt("Hunt Delay", "HuntDelayNr", Int(100)),
        opt("Accept and Complete Tries", "QuestTriesNr", Int(20)),
        opt("Loaded Quests Max.", "QuestMaxNr", Int(150)),
        opt("Map after stopping the Bot", "StopLocationSelect", Text("Home".into())),
    ]
}

/// The Other tab (VibeSkua `CoreBots.CreateOtherOptions`): categories of switches.
fn other_options() -> Vec<(&'static str, Vec<Opt>)> {
    let b = |label, tag, default| opt(label, tag, Value::Bool(default));
    let described = |label, description, tag| Opt {
        description,
        ..opt(label, tag, Value::Bool(false))
    };
    vec![
        (
            "Boosters",
            vec![
                b("Use Gold Boosts when farming gold", "doGoldBoost", false),
                b("Use Class Boosts when ranking up a class", "doClassBoost", false),
                b("Use Reputation Boosts when farming REP", "doRepBoost", false),
                b("Use Experience Boosts when farming EXP", "doExpBoost", false),
            ],
        ),
        (
            "Nation Farms",
            vec![
                b("Sell Voucher of Nulgath if not needed", "Nation_SellMemVoucher", true),
                b(
                    "Do Swindles Return during Supplies",
                    "Nation_ReturnPolicyDuringSupplies",
                    true,
                ),
                described(
                    "Ultra Alteon for Supplies",
                    "Use Ultra Alteon when farming Supplies instead of Escherion",
                    "UltraAlteonForSupplies",
                ),
            ],
        ),
        (
            "Bludrut Brawl (PvP)",
            vec![described(
                "Kill ads before boss",
                "Whether to kill brawlers and restorers in the PvP before the boss.",
                "PvP_SoloPvPBoss",
            )],
        ),
        (
            "Bot Creators Only",
            vec![described(
                "Story Bot Test Mode",
                "Toggles innate isCompleteBefore checks. It remains on the for Complete Once Quests",
                "BCO_Story_TestBot",
            )],
        ),
    ]
}

/// One of the four class roles (Solo = 1, Farm = 2, Dodge = 3, Boss = 4 in the gear keys).
struct Role {
    name: &'static str,
    badge: &'static str,
    /// Badge fill, border and text.
    colors: [Color32; 3],
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
    let role = |name, badge, colors: [u32; 3]| Role {
        name,
        badge,
        colors: colors.map(|c| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)),
        class: None,
        mode: None,
        modes: Vec::new(),
        use_equipment: false,
        gear: Default::default(),
    };
    [
        role("Solo", "SINGLE TARGET", [0x064E3B, 0x059669, 0x34D399]),
        role("Farm", "AOE FARMING", [0x451A03, 0xD97706, 0xFBBF24]),
        role("Dodge", "SURVIVABILITY", [0x1E3A8A, 0x3B82F6, 0x93C5FD]),
        role("Boss", "HEAVY BOSS", [0x2E1065, 0x8B5CF6, 0xC4B5FD]),
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
    ReloadClasses,
    RefreshInventory,
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
                // Always answer, or the window would stay busy.
                let items: Vec<ItemBase> = dispatch(bot, "inventory.items", &[])
                    .ok()
                    .and_then(|items| serde_json::from_value(items).ok())
                    .unwrap_or_default();
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
        self.username = fetched.username;
        self.current_class = fetched.current_class;
        if !self.username.is_empty() {
            self.items = fetched.items;
        }
        let logged_out = self.username.is_empty();
        match fetched.purpose {
            Purpose::Open => self.load(false),
            Purpose::Load => self.load(true),
            Purpose::Save => self.save(true),
            Purpose::LoadCharacter if logged_out => {
                self.message = Some((
                    "Load Character",
                    "Login first so that we can read your character's classes and equipment.".into(),
                ));
            }
            Purpose::LoadCharacter => {
                // Keep the window's selections, with the character's lists.
                let selections = turu_bot::cbo::parse(&self.lines());
                self.reload_classes();
                self.refresh_inventory();
                self.set_values(&selections);
            }
            Purpose::ReloadClasses => self.reload_classes(),
            Purpose::RefreshInventory => self.refresh_inventory(),
        }
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
            return;
        }
        let path = crate::turu::bot().paths.cbo_file(&self.username);
        match turu_bot::cbo::read_lines(&path, |_, _| {}) {
            Ok(Some(lines)) => {
                let values = turu_bot::cbo::parse(&lines);
                self.set_values(&values);
                self.read_values.insert(self.username.clone(), values);
            }
            Ok(None) => {}
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
        let public_confirmed = self.public_confirmed();
        let lines: Vec<String> = self
            .lines()
            .into_iter()
            // VibeSkua asks before saving public rooms; a save without the answer (closing the
            // window) keeps private rooms.
            .map(|line| {
                if line == "PrivateRooms: False" && !public_confirmed {
                    "PrivateRooms: True".into()
                } else {
                    line
                }
            })
            .collect();
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
                if show_dialog {
                    self.message = Some((
                        "Save Successful!",
                        format!("Saved to options/CBO_Storage({}).txt", self.username),
                    ));
                }
                self.read_values.insert(self.username.clone(), values);
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
        match self.tab {
            Tab::Loadout => self.loadout_tab(ui),
            Tab::Options => self.options_tab(ui),
            Tab::Other => self.other_tab(ui),
        }
    }

    /// The profile and its actions.
    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(32.0, 32.0), egui::Sense::hover());
            ui.painter().circle(rect.center(), 16.0, INSET, Stroke::new(1.0, EDGE));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.username
                    .chars()
                    .next()
                    .map_or("?".into(), |c| c.to_uppercase().to_string()),
                egui::FontId::proportional(14.0),
                super::theme::ACCENT,
            );
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("Profile").size(11.0).color(MUTED));
                if self.username.is_empty() {
                    ui.label(RichText::new("Not logged in").strong().color(MUTED));
                } else {
                    ui.label(RichText::new(&self.username).strong().color(TEXT));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
                if widgets::button(ui, "Load character", Kind::Normal, enabled, vec2(120.0, 28.0), "")
                    .on_hover_text("Load in-game character equipment and classes")
                    .clicked()
                {
                    self.fetch(Purpose::LoadCharacter);
                }
                if widgets::button(ui, "Load", Kind::Normal, enabled, vec2(84.0, 28.0), "")
                    .on_hover_text("Load saved CoreBots settings")
                    .clicked()
                {
                    self.fetch(Purpose::Load);
                }
                if self.busy {
                    ui.spinner();
                }
            });
        });
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
                let reload = "Refresh player classes from current inventory";
                if action_bar(
                    ui,
                    "Combat Class & Skill Configurations",
                    "Reload Classes",
                    reload,
                    self.busy,
                ) {
                    self.fetch(Purpose::ReloadClasses);
                }
                let width = ((ui.available_width() - 10.0) / 2.0).max(200.0);
                for pair in [0, 2] {
                    ui.horizontal_top(|ui| {
                        for i in pair..pair + 2 {
                            self.class_card(ui, i, width);
                        }
                    });
                }

                let header =
                    egui::CollapsingHeader::new(RichText::new("Detailed Equipment Slots  ").strong().color(TEXT))
                        .id_salt("cbo_equipment")
                        .open(Some(self.equipment_open))
                        .show(ui, |ui| {
                            let refresh = "Refresh items from player inventory";
                            if action_bar(
                                ui,
                                "Equipment Slots Per Combat Mode",
                                "Refresh Inventory",
                                refresh,
                                self.busy,
                            ) {
                                self.fetch(Purpose::RefreshInventory);
                            }
                            for pair in [0, 2] {
                                ui.horizontal_top(|ui| {
                                    for i in pair..pair + 2 {
                                        self.gear_card(ui, i, width);
                                    }
                                });
                            }
                        });
                if header.header_response.clicked() {
                    self.equipment_open = !self.equipment_open;
                }
                header
                    .header_response
                    .on_hover_text("(Optional per-mode gear overrides)");
            });
    }

    fn class_card(&mut self, ui: &mut Ui, i: usize, width: f32) {
        let (sets, current_class, classes) = (&self.sets, self.current_class.as_deref(), &self.owned.classes);
        let role = &mut self.roles[i];
        role_card(ui, role, width, "Class", |ui, role| {
            caption(ui, "Selected Class");
            let mut picked = None;
            ComboBox::from_id_salt(("cbo_class", i))
                .selected_text(role.class.as_deref().unwrap_or_default())
                .width(ui.available_width())
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

    fn gear_card(&mut self, ui: &mut Ui, i: usize, width: f32) {
        let lists = &self.owned.gear;
        let role = &mut self.roles[i];
        role_card(ui, role, width, "Equipment", |ui, role| {
            egui::Grid::new(("cbo_gear", i))
                .num_columns(2)
                .spacing(vec2(8.0, 6.0))
                .show(ui, |ui| {
                    for (slot, (label, _)) in SLOTS.iter().enumerate() {
                        ui.label(RichText::new(*label).size(10.0).color(MUTED));
                        let current = &mut role.gear[slot];
                        ComboBox::from_id_salt(("cbo_gear", i, slot))
                            .selected_text(current.as_str())
                            .width((width - 90.0).max(120.0))
                            .show_ui(ui, |ui| {
                                for name in &lists[slot] {
                                    if ui.selectable_label(current == name, name).clicked() {
                                        *current = name.clone();
                                    }
                                }
                            });
                        ui.end_row();
                    }
                });
        });
    }

    /// Two columns of options: switches, then numbers, then text, each by label.
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
        list_frame(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("cbo_options")
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let column = (ui.available_width() - 8.0) / 2.0;
                    for pair in shown.chunks_mut(2) {
                        ui.horizontal(|ui| {
                            for opt in pair {
                                ui.allocate_ui_with_layout(
                                    vec2(column, 30.0),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.set_width(column);
                                        option_row(ui, opt);
                                    },
                                );
                            }
                        });
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
                            ui.horizontal(|ui| {
                                ui.set_min_height(30.0);
                                option_row(ui, opt);
                            });
                        }
                        ui.add_space(4.0);
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

/// The character whose public rooms were confirmed (None: not confirmed).
static PUBLIC_CONFIRMED: Mutex<Option<String>> = Mutex::new(None);

fn sort_classes(classes: &mut Vec<String>) {
    classes.retain(|c| c != CURRENT_CLASS);
    classes.sort_by_key(|c| c.to_lowercase());
    classes.insert(0, CURRENT_CLASS.to_owned());
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).color(MUTED));
}

/// The bar above the class and gear cards: a title and a refresh button (true when clicked).
fn action_bar(ui: &mut Ui, title: &str, button: &str, tooltip: &str, busy: bool) -> bool {
    widgets::card(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::button(ui, button, Kind::Normal, !busy, vec2(0.0, 26.0), "")
                    .on_hover_text(tooltip)
                    .clicked()
            })
            .inner
        })
        .inner
    })
}

/// A role's card: its name and badge, then `add_contents`.
fn role_card(ui: &mut Ui, role: &mut Role, width: f32, kind: &str, add_contents: impl FnOnce(&mut Ui, &mut Role)) {
    widgets::card(ui, |ui| {
        ui.set_width(width - 22.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            widgets::dot(ui, role.colors[2], 4.0);
            ui.label(
                RichText::new(format!("{} {kind}", role.name))
                    .strong()
                    .size(13.0)
                    .color(TEXT),
            );
            if kind == "Class" {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    Frame::new()
                        .fill(role.colors[0])
                        .stroke(Stroke::new(1.0, role.colors[1]))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.label(RichText::new(role.badge).size(9.0).strong().color(role.colors[2]));
                        });
                });
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

/// One option: its label (with the description as a tooltip) and its control on the right.
fn option_row(ui: &mut Ui, opt: &mut Opt) {
    let label = ui.label(RichText::new(opt.label).color(TEXT));
    if !opt.description.is_empty() {
        label.on_hover_text(opt.description);
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| match &mut opt.value {
        Value::Bool(on) => {
            widgets::switch(ui, on);
        }
        Value::Int(n) => {
            let speed = (n.unsigned_abs() as f64 / 200.0).max(1.0);
            ui.add(egui::DragValue::new(n).range(0..=i32::MAX).speed(speed));
        }
        Value::Text(text) => {
            ui.add(
                egui::TextEdit::singleline(text)
                    .desired_width(140.0)
                    .margin(Margin::symmetric(8, 4)),
            );
        }
    });
}
