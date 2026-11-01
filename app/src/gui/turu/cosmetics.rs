//! Plugins > Cosmetics (VibeSkua's Cosmetics plugin window): the quick bar (copy a player's
//! look, reset to the real gear, gender), the item catalog, saved outfits, the color editor
//! and the manual SWF loader. Everything it changes is client side.
//!
//! The catalog, favorites and outfits are `turu_bot::cosmetics::Library`, shared by the tabs;
//! the game calls run off the UI thread and report back through `Shared`.

use super::theme::{ACCENT, DANGER, HOVER, INSET, MUTED, SUCCESS, TEXT, TRACK};
use super::widgets::{self, Kind};
use egui::{Align, Color32, ComboBox, Context, CornerRadius, Layout, RichText, ScrollArea, Sense, Shape, Ui, vec2};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::cosmetics::avatar::{self, ColorChange, Colors};
use turu_bot::cosmetics::{CosmeticItem, CosmeticSet, Library, Slot, WEAPON_KINDS, charpage, group_digits};

/// How often the room's players are read for the copy list.
const PLAYERS_REFRESH: Duration = Duration::from_secs(3);
const ROW_HEIGHT: f32 = 40.0;
const DETAIL_WIDTH: f32 = 250.0;
const PAGE_SIZES: [usize; 4] = [50, 100, 250, 500];

/// The color rows of the CC tab, in `Colors`' order: label and what it colors.
const COLOR_ROWS: [(&str, &str); 6] = [
    ("Base Color", "Primary Armor / Clothes"),
    ("Trim Color", "Secondary Highlights"),
    ("Accessory Color", "Gems, Accents, Glows"),
    ("Eye Color", "Pupils & Eyes"),
    ("Skin Color", "Body & Face Tone"),
    ("Hair Color", "Hair & Eyebrows"),
];

/// VibeSkua's themed presets: name, description, base, trim, accessory and eye.
const PRESETS: [(&str, &str, [&str; 4]); 8] = [
    (
        "Void Fiend",
        "Deep obsidian purple",
        ["#1A0826", "#8A2BE2", "#E0B0FF", "#FF00FF"],
    ),
    (
        "Legion Soul",
        "Underworld cyan glow",
        ["#0D1117", "#00E5FF", "#1DE9B6", "#00FFFF"],
    ),
    (
        "Shadow Assassin",
        "Pure stealth black & crimson",
        ["#0D0D0D", "#262626", "#404040", "#FF1744"],
    ),
    (
        "Celestial Light",
        "Pure white & heavenly gold",
        ["#F5F5F5", "#FFD700", "#FFB300", "#40C4FF"],
    ),
    (
        "Doom Crimson",
        "Blood vampire scarlet",
        ["#1F0000", "#800000", "#FF1A1A", "#FF3333"],
    ),
    (
        "Cyber Synth",
        "Neon magenta & cyan",
        ["#180033", "#FF007F", "#00F0FF", "#FFFF00"],
    ),
    (
        "Royal Gold",
        "Gilded imperial regalia",
        ["#1C1C1C", "#D4AF37", "#F3E5AB", "#FFD700"],
    ),
    (
        "Pure Frost",
        "Glacial crystalline ice",
        ["#E0F7FA", "#80DEEA", "#00BCD4", "#00E5FF"],
    ),
];

/// The Manual SWF tab's slots: label and `eqp` key.
const MANUAL_SLOTS: [(&str, &str); 6] = [
    ("Armor / Class (co)", "co"),
    ("Helm (he)", "he"),
    ("Cape (ba)", "ba"),
    ("Weapon (Weapon)", "Weapon"),
    ("Pet (pe)", "pe"),
    ("Ground Rune / Misc (mi)", "mi"),
];

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Catalog,
    Outfits,
    Colors,
    Manual,
}

/// The catalog filter chips.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum Category {
    #[default]
    All,
    Slot(Slot),
    Favorites,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum Sort {
    #[default]
    Newest,
    Oldest,
    NameAsc,
    NameDesc,
}

impl Sort {
    fn label(self) -> &'static str {
        match self {
            Sort::Newest => "Newest First (ID ↓)",
            Sort::Oldest => "Oldest First (ID ↑)",
            Sort::NameAsc => "Name (A → Z)",
            Sort::NameDesc => "Name (Z → A)",
        }
    }
}

/// What the filtered list was made from; the list is made again when it changes.
#[derive(PartialEq)]
struct FilterKey {
    revision: u64,
    query: String,
    category: Category,
    weapon_kind: Option<&'static str>,
    sort: Sort,
}

/// What the background actions report.
#[derive(Default)]
struct Shared {
    /// The quick bar's message (VibeSkua `QuickActionBarStatus`).
    status: String,
    catalog_status: String,
    catalog_busy: bool,
    /// Colors read from the avatar for the CC tab; `true` also fills all-black ones.
    colors: Option<(Colors, bool)>,
    cc_status: String,
    room_players: Vec<String>,
    reading_players: bool,
    busy: bool,
}

pub struct CosmeticsWindow {
    shared: Arc<Mutex<Shared>>,
    was_open: bool,
    tab: Tab,
    players_read_at: Option<Instant>,
    room_player: String,
    /// A character name for Copy from Character Page.
    char_name: String,
    // Catalog.
    query: String,
    category: Category,
    weapon_kind: Option<&'static str>,
    sort: Sort,
    page: usize,
    page_size: usize,
    filter_key: Option<FilterKey>,
    /// The filtered items: `source[i]` for each `i`.
    source: Arc<Vec<CosmeticItem>>,
    filtered: Vec<u32>,
    selected: Option<CosmeticItem>,
    // Outfits.
    selected_set: Option<usize>,
    // Colors, as typed ("#RRGGBB").
    hex: [String; 6],
    // Manual SWF.
    manual_file: String,
    manual_link: String,
    manual_meta: String,
    manual_slot: usize,
    manual_status: String,
}

impl Default for CosmeticsWindow {
    fn default() -> Self {
        Self {
            shared: Arc::default(),
            was_open: false,
            tab: Tab::default(),
            players_read_at: None,
            room_player: String::new(),
            char_name: String::new(),
            query: String::new(),
            category: Category::default(),
            weapon_kind: None,
            sort: Sort::default(),
            page: 0,
            page_size: PAGE_SIZES[0],
            filter_key: None,
            source: Arc::default(),
            filtered: Vec::new(),
            selected: None,
            selected_set: None,
            hex: Default::default(),
            manual_file: String::new(),
            manual_link: String::new(),
            manual_meta: String::new(),
            manual_slot: 0,
            manual_status: String::new(),
        }
    }
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// Runs a game action off the UI thread; a failure goes to the script log as VibeSkua's
/// "[Cosmetics] ... error" lines do.
fn act(what: &'static str, action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static) {
    crate::turu::spawn_action("Cosmetics", move |bot| {
        if let Err(e) = action(bot) {
            bot.log(format!("[Cosmetics] {what} error: {e}"));
        }
        Ok(())
    });
}

impl CosmeticsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            self.was_open = false;
            return;
        }
        let bot = crate::turu::bot();
        let library = Library::shared(&bot.paths);
        if !self.was_open {
            self.was_open = true;
            self.on_open(&library);
        }
        if self.players_read_at.is_none_or(|t| t.elapsed() >= PLAYERS_REFRESH) {
            self.read_players();
        }
        if let Some((colors, force)) = lock(&self.shared).colors.take()
            && (force || colors.to_array().iter().any(|&c| c != 0))
        {
            self.hex = colors.to_array().map(|c| format!("#{c:06X}"));
        }
        super::window(ctx, "Cosmetics & Outfits", [900.0, 620.0], 7)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &library));
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    /// VibeSkua `CosmeticsWindow_Loaded`: remember the real look, fill the CC tab, load the
    /// catalog.
    fn on_open(&self, library: &Arc<Library>) {
        self.read_colors(false);
        if library.is_loaded() {
            lock(&self.shared).catalog_status =
                format!("{} items (Local Machine)", group_digits(library.catalog().len()));
            return;
        }
        let mut shared = lock(&self.shared);
        if shared.catalog_busy {
            return;
        }
        shared.catalog_busy = true;
        shared.catalog_status = "Streaming local catalog...".into();
        drop(shared);
        let (shared, library) = (self.shared.clone(), library.clone());
        std::thread::spawn(move || {
            let result = library.ensure_loaded();
            let mut shared = lock(&shared);
            shared.catalog_busy = false;
            shared.catalog_status = result.unwrap_or_else(|e| e);
        });
    }

    /// Reads the avatar's colors into the CC tab (and remembers the real look first).
    fn read_colors(&self, force: bool) {
        let shared = self.shared.clone();
        act("Read colors", move |bot| {
            avatar::capture_original_state(bot)?;
            let colors = avatar::current_colors(bot)?;
            let mut shared = lock(&shared);
            shared.colors = Some((colors, force));
            if force {
                shared.cc_status = "Read colors from avatar.".into();
            }
            Ok(())
        });
    }

    fn read_players(&mut self) {
        self.players_read_at = Some(Instant::now());
        let mut shared = lock(&self.shared);
        if shared.reading_players {
            return;
        }
        shared.reading_players = true;
        drop(shared);
        let shared = self.shared.clone();
        crate::turu::spawn_action("Cosmetics players", move |bot| {
            let names = avatar::room_player_names(bot);
            let mut shared = lock(&shared);
            shared.reading_players = false;
            shared.room_players = names?;
            Ok(())
        });
    }

    /// A quick-bar action that reports a message and reads the colors after it worked.
    fn quick(&self, start: &str, action: impl FnOnce(&Arc<Bot>) -> Result<String, String> + Send + 'static) {
        let mut shared = lock(&self.shared);
        shared.status = start.into();
        shared.busy = true;
        drop(shared);
        let shared = self.shared.clone();
        crate::turu::spawn_action("Cosmetics", move |bot| {
            let result = action(bot);
            let colors = if result.is_ok() {
                avatar::current_colors(bot).ok()
            } else {
                None
            };
            let mut shared = lock(&shared);
            shared.busy = false;
            shared.status = result.unwrap_or_else(|e| e);
            if let Some(colors) = colors {
                shared.colors = Some((colors, false));
            }
            Ok(())
        });
    }

    fn ui(&mut self, ui: &mut Ui, library: &Arc<Library>) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        self.quick_bar(ui, library);
        ui.horizontal(|ui| {
            for (tab, label) in [
                (Tab::Catalog, "Catalog"),
                (Tab::Outfits, "Outfits & Inspector"),
                (Tab::Colors, "Color Custom (CC)"),
                (Tab::Manual, "Manual SWF"),
            ] {
                if widgets::chip(ui, label, self.tab == tab) {
                    self.tab = tab;
                }
            }
        });
        match self.tab {
            Tab::Catalog => self.catalog_tab(ui, library),
            Tab::Outfits => self.outfits_tab(ui, library),
            Tab::Colors => self.colors_tab(ui),
            Tab::Manual => self.manual_tab(ui),
        }
    }

    /// Copy Target Look, a room player to copy, Reset to Real Gear, and the gender buttons.
    fn quick_bar(&mut self, ui: &mut Ui, library: &Arc<Library>) {
        let (players, status, busy) = {
            let shared = lock(&self.shared);
            (shared.room_players.clone(), shared.status.clone(), shared.busy)
        };
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if widgets::button(ui, "Copy Target Look", Kind::Primary, !busy, vec2(0.0, 26.0), "")
                    .on_hover_text("Copy the equipment and colors of the player you target")
                    .clicked()
                {
                    let library = library.clone();
                    self.quick("Reading target avatar...", move |bot| {
                        avatar::copy_target_look(bot, &library)
                    });
                }
                if !self.room_player.is_empty() && !players.contains(&self.room_player) {
                    self.room_player.clear();
                }
                if self.room_player.is_empty()
                    && let Some(first) = players.first()
                {
                    self.room_player = first.clone();
                }
                let shown = if self.room_player.is_empty() {
                    "Room players..."
                } else {
                    &self.room_player
                };
                ComboBox::from_id_salt("turu_cosmetics_players")
                    .selected_text(shown)
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for name in &players {
                            ui.selectable_value(&mut self.room_player, name.clone(), name);
                        }
                    });
                if widgets::button(ui, "Copy", Kind::Normal, !busy, vec2(0.0, 26.0), "")
                    .on_hover_text("Copy the look of the selected room player")
                    .clicked()
                {
                    if self.room_player.is_empty() {
                        lock(&self.shared).status = "Please select a room player first.".into();
                    } else {
                        let (library, name) = (library.clone(), self.room_player.clone());
                        self.quick(&format!("Copying look from {name}..."), move |bot| {
                            avatar::copy_player_look_by_name(bot, &library, &name)
                        });
                    }
                }
                if widgets::button(ui, "Reset to Real Gear", Kind::Outline, !busy, vec2(0.0, 26.0), "")
                    .on_hover_text("Put back your equipped inventory items, colors and gender")
                    .clicked()
                {
                    self.quick("Reverting to real inventory gear & colors...", |bot| {
                        avatar::revert_to_real_gear(bot)
                            .map(|()| "Reverted to actual equipped gear & colors!".to_owned())
                            .map_err(|e| format!("Reset to real gear error: {e}"))
                    });
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for (label, tip, gender) in [
                        ("Swap", "Swap Gender", None),
                        ("F", "Morph Female", Some('F')),
                        ("M", "Morph Male", Some('M')),
                    ] {
                        if widgets::button(ui, label, Kind::Normal, true, vec2(26.0, 26.0), "")
                            .on_hover_text(tip)
                            .clicked()
                        {
                            act("Gender swap", move |bot| avatar::swap_gender(bot, gender, false));
                        }
                    }
                });
            });
            // Turu: any character's look from its account.aq.com page.
            ui.horizontal(|ui| {
                let typed = widgets::text_box(ui, &mut self.char_name, "Character name", 180.0);
                let entered = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let clicked = widgets::button(ui, "Copy from Character Page", Kind::Normal, !busy, vec2(0.0, 26.0), "")
                    .on_hover_text("Copy the gear, gender and colors shown on the character's account.aq.com page")
                    .clicked();
                if (clicked || entered) && !busy {
                    let name = self.char_name.trim().to_owned();
                    self.quick(&format!("Reading {name}'s character page..."), move |bot| {
                        let page = charpage::fetch(&name)?;
                        avatar::copy_char_page(bot, &page)
                    });
                }
            });
            if !status.is_empty() {
                ui.label(RichText::new(status).small().color(ACCENT));
            }
        });
    }

    // -------------------------------------------------------------------------------------
    // Catalog

    fn catalog_tab(&mut self, ui: &mut Ui, library: &Arc<Library>) {
        let (catalog_status, catalog_busy) = {
            let shared = lock(&self.shared);
            (shared.catalog_status.clone(), shared.catalog_busy)
        };
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 360.0).max(160.0);
            widgets::text_box(ui, &mut self.query, "Search cosmetics by name or ID...", width);
            if widgets::small_button(ui, "×", !self.query.is_empty()).clicked() {
                self.query.clear();
            }
            ComboBox::from_id_salt("turu_cosmetics_sort")
                .selected_text(self.sort.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    for sort in [Sort::Newest, Sort::Oldest, Sort::NameAsc, Sort::NameDesc] {
                        ui.selectable_value(&mut self.sort, sort, sort.label());
                    }
                });
            if widgets::button(ui, "Check Updates", Kind::Normal, !catalog_busy, vec2(0.0, 24.0), "")
                .on_hover_text("Download the catalog again and keep it if it has new items")
                .clicked()
            {
                self.check_updates(library);
            }
        });
        ui.label(RichText::new(catalog_status).small().color(MUTED));
        ui.horizontal_wrapped(|ui| {
            let chips = [
                (Category::All, "All"),
                (Category::Slot(Slot::Armor), "Armor"),
                (Category::Slot(Slot::Weapon), "Weapon"),
                (Category::Slot(Slot::Helm), "Helm"),
                (Category::Slot(Slot::Cape), "Cape"),
                (Category::Slot(Slot::Pet), "Pet"),
                (Category::Slot(Slot::Rune), "Ground Rune"),
                (Category::Favorites, "♥ Favorites"),
            ];
            for (category, label) in chips {
                if widgets::chip(ui, label, self.category == category) {
                    self.category = category;
                }
            }
            if self.category == Category::Slot(Slot::Weapon) {
                ComboBox::from_id_salt("turu_cosmetics_weapon")
                    .selected_text(self.weapon_kind.unwrap_or("All Weapons"))
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.weapon_kind, None, "All Weapons");
                        for &kind in WEAPON_KINDS {
                            ui.selectable_value(&mut self.weapon_kind, Some(kind), kind);
                        }
                    });
            }
        });
        self.filter(library);

        let height = (ui.available_height() - 36.0).max(160.0);
        ui.horizontal_top(|ui| {
            let list_width = (ui.available_width() - DETAIL_WIDTH - 8.0).max(240.0);
            ui.allocate_ui(vec2(list_width, height), |ui| {
                ui.set_min_size(vec2(list_width, height));
                self.item_list(ui, library);
            });
            ui.allocate_ui(vec2(DETAIL_WIDTH, height), |ui| self.detail_card(ui, library));
        });
        self.pagination(ui);
    }

    fn check_updates(&self, library: &Arc<Library>) {
        let mut shared = lock(&self.shared);
        shared.catalog_busy = true;
        shared.catalog_status = "Checking for updates...".into();
        drop(shared);
        let (shared, library) = (self.shared.clone(), library.clone());
        std::thread::spawn(move || {
            let result = library.check_for_updates();
            let mut shared = lock(&shared);
            shared.catalog_busy = false;
            shared.catalog_status = result.unwrap_or_else(|e| e);
        });
    }

    /// Makes the filtered list again when the catalog or the filter changed (VibeSkua
    /// `FilterCatalog`): category, search (by ID when it is a number, else by name), sort.
    fn filter(&mut self, library: &Library) {
        let key = FilterKey {
            revision: library.revision(),
            query: self.query.trim().to_owned(),
            category: self.category,
            weapon_kind: self.weapon_kind,
            sort: self.sort,
        };
        if self.filter_key.as_ref() == Some(&key) {
            return;
        }
        let favorites = key.category == Category::Favorites;
        self.source = if favorites {
            Arc::new(library.favorites())
        } else {
            library.catalog()
        };
        let query = key.query.to_lowercase();
        let id_query = key.query.parse::<i64>().ok();
        let matches = |item: &CosmeticItem| {
            let in_category = match key.category {
                Category::Slot(Slot::Weapon) => {
                    item.is_in(Slot::Weapon) && key.weapon_kind.is_none_or(|kind| item.kind() == kind)
                }
                Category::Slot(slot) => item.is_in(slot),
                Category::All | Category::Favorites => true,
            };
            in_category
                && match id_query {
                    _ if query.is_empty() => true,
                    Some(id) => item.id == id || item.id.to_string().contains(&query),
                    None => item.name.to_lowercase().contains(&query),
                }
        };
        let mut filtered: Vec<u32> = self
            .source
            .iter()
            .enumerate()
            .filter(|(_, item)| matches(item))
            .map(|(i, _)| i as u32)
            .collect();
        let source = &self.source;
        let item = |i: &u32| &source[*i as usize];
        match key.sort {
            // The catalog is already newest first.
            Sort::Newest if !favorites => {}
            Sort::Newest => filtered.sort_by_key(|i| std::cmp::Reverse(item(i).id)),
            Sort::Oldest => filtered.sort_by_key(|i| item(i).id),
            Sort::NameAsc => filtered.sort_by_cached_key(|i| item(i).name.to_lowercase()),
            Sort::NameDesc => {
                filtered.sort_by_cached_key(|i| item(i).name.to_lowercase());
                filtered.reverse();
            }
        }
        self.filtered = filtered;
        // A new search or category starts at page 1; a new favorite keeps the page.
        let same_view = self.filter_key.as_ref().is_some_and(|old| {
            old.query == key.query && old.category == key.category && old.weapon_kind == key.weapon_kind
        });
        if !same_view {
            self.page = 0;
        }
        self.filter_key = Some(key);
    }

    fn pages(&self) -> usize {
        self.filtered.len().div_ceil(self.page_size).max(1)
    }

    fn item_list(&mut self, ui: &mut Ui, library: &Library) {
        self.page = self.page.min(self.pages() - 1);
        let start = self.page * self.page_size;
        let rows: Vec<u32> = self.filtered.iter().skip(start).take(self.page_size).copied().collect();
        if rows.is_empty() {
            ui.label(RichText::new("No items.").color(MUTED));
            return;
        }
        let source = self.source.clone();
        ScrollArea::vertical()
            .id_salt((
                "turu_cosmetics_items",
                self.page,
                self.filter_key.as_ref().map(|k| k.revision),
            ))
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                for &index in &rows[range] {
                    let item = &source[index as usize];
                    let selected = self.selected.as_ref().is_some_and(|s| s.key() == item.key());
                    let response = item_row(ui, item, selected, library.is_favorite(item));
                    if response.clicked() {
                        self.selected = Some(item.clone());
                    }
                    if response.double_clicked() {
                        let item = item.clone();
                        act("Equip", move |bot| avatar::equip(bot, &item));
                    }
                }
            });
    }

    fn detail_card(&self, ui: &mut Ui, library: &Library) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "SELECTED COSMETIC");
            let Some(item) = self.selected.clone() else {
                ui.label(RichText::new("No Item Selected").size(15.0).strong().color(TEXT));
                return;
            };
            ui.label(RichText::new(&item.name).size(15.0).strong().color(TEXT));
            widgets::pill(ui, |ui| {
                ui.label(RichText::new(item.display_category()).size(11.0).color(ACCENT));
            });
            for line in [
                format!("ID: {}", item.id),
                format!("File: {}", item.path),
                format!("Link: {}", item.link),
            ] {
                ui.label(RichText::new(line).size(11.0).color(MUTED));
            }
            ui.add_space(4.0);
            let width = ui.available_width();
            if widgets::button(ui, "Equip Look", Kind::Primary, true, vec2(width, 30.0), "").clicked() {
                let item = item.clone();
                act("Equip", move |bot| avatar::equip(bot, &item));
            }
            let favorite = library.is_favorite(&item);
            let label = if favorite {
                "♥ Remove Favorite"
            } else {
                "♡ Add to Favorites"
            };
            if widgets::button(ui, label, Kind::Normal, true, vec2(width, 26.0), "").clicked() {
                library.toggle_favorite(&item);
            }
            if widgets::button(ui, "Open on AQW Wiki", Kind::Outline, true, vec2(width, 26.0), "").clicked() {
                let url = format!(
                    "http://aqwwiki.wikidot.com/{}",
                    turu_bot::cosmetics::escape_data(&item.name)
                );
                if let Err(e) = webbrowser::open(&url) {
                    crate::turu::bot().log(format!("[Cosmetics] Failed to open browser: {e}"));
                }
            }
        });
    }

    fn pagination(&mut self, ui: &mut Ui) {
        let pages = self.pages();
        ui.horizontal(|ui| {
            let (first, last) = (self.page == 0, self.page + 1 >= pages);
            if widgets::small_button(ui, "«", !first).clicked() {
                self.page = 0;
            }
            if widgets::small_button(ui, "‹", !first).clicked() {
                self.page -= 1;
            }
            ui.label(
                RichText::new(format!(
                    "Page {} of {pages} ({} items)",
                    self.page + 1,
                    group_digits(self.filtered.len())
                ))
                .small()
                .color(TEXT),
            );
            if widgets::small_button(ui, "›", !last).clicked() {
                self.page += 1;
            }
            if widgets::small_button(ui, "»", !last).clicked() {
                self.page = pages - 1;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let before = self.page_size;
                ComboBox::from_id_salt("turu_cosmetics_page_size")
                    .selected_text(self.page_size.to_string())
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for size in PAGE_SIZES {
                            ui.selectable_value(&mut self.page_size, size, size.to_string());
                        }
                    });
                if self.page_size != before {
                    self.page = 0;
                }
                ui.label(RichText::new("Per Page:").small().color(MUTED));
            });
        });
    }

    // -------------------------------------------------------------------------------------
    // Outfits & Inspector

    fn outfits_tab(&mut self, ui: &mut Ui, library: &Arc<Library>) {
        let sets = library.sets();
        if self.selected_set.is_some_and(|i| i >= sets.len()) {
            self.selected_set = None;
        }
        let height = ui.available_height().max(200.0);
        ui.horizontal_top(|ui| {
            let list_width = (ui.available_width() - DETAIL_WIDTH - 8.0).max(240.0);
            ui.allocate_ui(vec2(list_width, height), |ui| {
                ui.set_min_size(vec2(list_width, height));
                ui.horizontal(|ui| {
                    if widgets::button(ui, "Save Current Look", Kind::Primary, true, vec2(0.0, 28.0), "").clicked() {
                        let library = library.clone();
                        let name = format!("Outfit #{}", sets.len() + 1);
                        act("Capture look", move |bot| {
                            library.add_set(avatar::capture_current_look(bot, &library, &name)?);
                            Ok(())
                        });
                    }
                    ui.label(
                        RichText::new(format!("{} Outfits Saved", sets.len()))
                            .small()
                            .color(MUTED),
                    );
                });
                ScrollArea::vertical()
                    .id_salt("turu_cosmetics_sets")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, set) in sets.iter().enumerate() {
                            let response = set_row(ui, set, self.selected_set == Some(i));
                            if response.clicked() {
                                self.selected_set = Some(i);
                            }
                            if response.double_clicked() {
                                equip_set(set);
                            }
                        }
                    });
            });
            ui.allocate_ui(vec2(DETAIL_WIDTH, height), |ui| {
                let set = self.selected_set.and_then(|i| sets.get(i));
                self.inspector(ui, library, set);
            });
        });
    }

    fn inspector(&mut self, ui: &mut Ui, library: &Library, set: Option<&CosmeticSet>) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "OUTFIT INSPECTOR");
            ui.label(
                RichText::new(set.map_or("Select an Outfit", |s| &s.name))
                    .size(15.0)
                    .strong()
                    .color(TEXT),
            );
            let date = set.map_or("-".into(), |s| format!("Saved: {}", s.created_label()));
            ui.label(RichText::new(date).small().color(MUTED));
            for slot in Slot::ALL {
                let item = set.and_then(|s| s.get(slot));
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        // Never negative: egui panics on it, as in a narrow window.
                        ui.set_width((ui.available_width() - 60.0).max(0.0));
                        ui.label(RichText::new(slot.label()).size(10.0).strong().color(MUTED));
                        let name = item.map_or("None", |i| i.name.as_str());
                        ui.add(egui::Label::new(RichText::new(name).size(11.0).color(TEXT)).truncate());
                    });
                    if widgets::button(ui, "Equip", Kind::Normal, item.is_some(), vec2(52.0, 24.0), "").clicked()
                        && let Some(item) = item.cloned()
                    {
                        act("Equip", move |bot| avatar::equip(bot, &item));
                    }
                });
            }
            ui.add_space(4.0);
            let width = ui.available_width();
            if widgets::button(
                ui,
                "Equip Full Set",
                Kind::Primary,
                set.is_some(),
                vec2(width, 30.0),
                "",
            )
            .clicked()
                && let Some(set) = set
            {
                equip_set(set);
            }
            if widgets::button(ui, "Delete Set", Kind::Danger, set.is_some(), vec2(width, 26.0), "").clicked()
                && let Some(i) = self.selected_set.take()
            {
                library.remove_set(i);
            }
        });
    }

    // -------------------------------------------------------------------------------------
    // Color Custom (CC)

    fn colors_tab(&mut self, ui: &mut Ui) {
        let cc_status = lock(&self.shared).cc_status.clone();
        ui.horizontal_top(|ui| {
            let width = (ui.available_width() - DETAIL_WIDTH - 8.0).max(300.0);
            ui.allocate_ui(vec2(width, ui.available_height()), |ui| {
                widgets::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("Character Color Customizer")
                            .size(15.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.label(
                        RichText::new("Directly modify your client-side Base, Trim, Accessory, and Eye colors.")
                            .small()
                            .color(MUTED),
                    );
                    egui::Grid::new("turu_cosmetics_cc")
                        .num_columns(3)
                        .spacing(vec2(10.0, 8.0))
                        .show(ui, |ui| {
                            for ((label, what), hex) in COLOR_ROWS.iter().zip(self.hex.iter_mut()) {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(*label).strong().color(TEXT));
                                    ui.label(RichText::new(*what).size(10.0).color(MUTED));
                                });
                                widgets::text_box(ui, hex, "#RRGGBB", 110.0);
                                let mut rgb =
                                    parse_hex(hex).map_or([0, 0, 0], |c| [(c >> 16) as u8, (c >> 8) as u8, c as u8]);
                                if ui.color_edit_button_srgb(&mut rgb).changed() {
                                    *hex = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
                                }
                                ui.end_row();
                            }
                        });
                    ui.horizontal(|ui| {
                        if widgets::button(ui, "Apply Palette", Kind::Primary, true, vec2(0.0, 28.0), "").clicked() {
                            let colors = ColorChange(self.hex.each_ref().map(|h| parse_hex(h)));
                            act("SetColors", move |bot| avatar::set_colors(bot, colors, true));
                            lock(&self.shared).cc_status = "Applied palette colors to character!".into();
                        }
                        if widgets::button(ui, "Read Avatar Colors", Kind::Normal, true, vec2(0.0, 28.0), "").clicked()
                        {
                            self.read_colors(true);
                        }
                    });
                    if !cc_status.is_empty() {
                        ui.label(RichText::new(cc_status).small().color(SUCCESS));
                    }
                });
            });
            ui.allocate_ui(vec2(DETAIL_WIDTH, ui.available_height()), |ui| {
                ScrollArea::vertical()
                    .id_salt("turu_cosmetics_presets")
                    .show(ui, |ui| self.presets(ui));
            });
        });
    }

    /// VibeSkua's preset cards: set the first four colors and apply them with the skin and hair
    /// in the boxes.
    fn presets(&mut self, ui: &mut Ui) {
        widgets::section_label(ui, "THEMED PRESETS");
        for (name, description, colors) in PRESETS {
            let response = row(ui, false, 44.0, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(name).strong().color(TEXT));
                    ui.label(RichText::new(description).size(10.0).color(MUTED));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(6.0);
                    for hex in colors.iter().rev() {
                        swatch(ui, parse_hex(hex).unwrap_or(0));
                    }
                });
            });
            if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                for (box_, hex) in self.hex.iter_mut().zip(colors) {
                    *box_ = hex.to_owned();
                }
                let colors = ColorChange(self.hex.each_ref().map(|h| parse_hex(h)));
                act("SetColors", move |bot| avatar::set_colors(bot, colors, true));
                lock(&self.shared).cc_status = "Preset applied!".into();
            }
        }
    }

    // -------------------------------------------------------------------------------------
    // Manual SWF

    fn manual_tab(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Direct SWF Injection").size(15.0).strong().color(TEXT));
            ui.label(
                RichText::new("Visually loads any raw SWF file onto your avatar by path and linkage ID.")
                    .small()
                    .color(MUTED),
            );
            let width = ui.available_width();
            ui.label(RichText::new("SWF File Path (sFile)").strong().color(TEXT));
            widgets::text_box(
                ui,
                &mut self.manual_file,
                "e.g. classes/M/Warrior.swf or items/helms/MyHelm.swf",
                width,
            );
            ui.label(RichText::new("Linkage Identifier (sLink)").strong().color(TEXT));
            widgets::text_box(ui, &mut self.manual_link, "e.g. mWarrior or HelmLink", width);
            ui.label(RichText::new("Equipment Slot / Group").strong().color(TEXT));
            ComboBox::from_id_salt("turu_cosmetics_manual_slot")
                .selected_text(MANUAL_SLOTS[self.manual_slot].0)
                .width(width.min(260.0))
                .show_ui(ui, |ui| {
                    for (i, (label, _)) in MANUAL_SLOTS.iter().enumerate() {
                        ui.selectable_value(&mut self.manual_slot, i, *label);
                    }
                });
            ui.label(RichText::new("Meta Data (sMeta, Optional)").strong().color(TEXT));
            widgets::text_box(
                ui,
                &mut self.manual_meta,
                "Optional metadata or buff identifiers",
                width,
            );
            ui.add_space(4.0);
            if widgets::button(
                ui,
                "Inject SWF Onto Character",
                Kind::Primary,
                true,
                vec2(width, 32.0),
                "",
            )
            .clicked()
            {
                let (file, link, meta) = (
                    self.manual_file.trim().to_owned(),
                    self.manual_link.trim().to_owned(),
                    self.manual_meta.trim().to_owned(),
                );
                let group = MANUAL_SLOTS[self.manual_slot].1;
                if file.is_empty() || link.is_empty() {
                    self.manual_status = "Error: File path and Linkage ID cannot be empty.".into();
                } else {
                    self.manual_status = format!("Injected '{link}' into slot '{group}'!");
                    act("Equip", move |bot| avatar::inject(bot, group, &file, &link, &meta));
                }
            }
            if !self.manual_status.is_empty() {
                let color = if self.manual_status.starts_with("Error") {
                    DANGER
                } else {
                    SUCCESS
                };
                ui.label(RichText::new(&self.manual_status).small().color(color));
            }
        });
    }
}

fn equip_set(set: &CosmeticSet) {
    let set = set.clone();
    act("Equip set", move |bot| avatar::equip_set(bot, &set));
}

/// A clickable row with a selection background behind what `add_contents` draws.
fn row(ui: &mut Ui, selected: bool, height: f32, add_contents: impl FnOnce(&mut Ui)) -> egui::Response {
    let background = ui.painter().add(Shape::Noop);
    let inner = ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.set_min_size(vec2(ui.available_width(), height));
            ui.add_space(6.0);
            add_contents(ui);
        },
    );
    let response = inner.response.interact(Sense::click());
    let fill = if selected {
        TRACK
    } else if response.hovered() {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().set(
        background,
        Shape::rect_filled(response.rect, CornerRadius::same(6), fill),
    );
    response
}

/// A catalog row: name, category and favorite mark; file and ID below.
fn item_row(ui: &mut Ui, item: &CosmeticItem, selected: bool, favorite: bool) -> egui::Response {
    row(ui, selected, ROW_HEIGHT - 2.0, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.horizontal(|ui| {
                if favorite {
                    ui.label(RichText::new("♥").color(DANGER));
                }
                ui.add(
                    egui::Label::new(RichText::new(&item.name).strong().color(TEXT))
                        .truncate()
                        .selectable(false),
                );
                ui.label(RichText::new(item.display_category()).size(10.0).color(ACCENT));
            });
            ui.add(
                egui::Label::new(
                    RichText::new(format!("ID: {} · {}", item.id, item.path))
                        .size(10.0)
                        .color(MUTED),
                )
                .truncate()
                .selectable(false),
            );
        });
    })
}

/// An outfit row: name, summary and date, with Equip.
fn set_row(ui: &mut Ui, set: &CosmeticSet, selected: bool) -> egui::Response {
    let response = row(ui, selected, 52.0, |ui| {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width() - 70.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            ui.add(
                egui::Label::new(RichText::new(&set.name).strong().color(TEXT))
                    .truncate()
                    .selectable(false),
            );
            ui.add(
                egui::Label::new(RichText::new(set.summary()).size(10.0).color(MUTED))
                    .truncate()
                    .selectable(false),
            );
            let date = format!("Saved: {}", set.created_label());
            ui.add(egui::Label::new(RichText::new(date).size(10.0).color(MUTED)).selectable(false));
        });
    });
    // The row's Equip button, over its right end.
    let rect = egui::Rect::from_min_size(response.rect.right_center() - vec2(64.0, 12.0), vec2(56.0, 24.0));
    if ui.put(rect, egui::Button::new("Equip")).clicked() {
        equip_set(set);
    }
    response
}

/// A small color square.
fn swatch(ui: &mut Ui, color: i64) {
    let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    let [r, g, b] = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
    ui.painter()
        .rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(r, g, b));
    ui.painter()
        .rect_stroke(rect, CornerRadius::same(3), (1.0, INSET), egui::StrokeKind::Inside);
}

/// "#1A0826" or "1a0826" (VibeSkua `ParseHexToInt`); `None` for anything else.
fn parse_hex(text: &str) -> Option<i64> {
    let digits = text.trim().trim_start_matches('#');
    if digits.is_empty() {
        return None;
    }
    i64::from_str_radix(digits, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_wiki_names() {
        assert_eq!(parse_hex("#1A0826"), Some(0x1A0826));
        assert_eq!(parse_hex(" ff "), Some(0xFF));
        assert_eq!(parse_hex("#"), None);
        assert_eq!(parse_hex("nope"), None);
    }
}
