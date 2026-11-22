//! Plugins > Cosmetics (VibeSkua's Cosmetics plugin window): copy a look (a room player's, the
//! target's, a character page's), Reset to Real Gear and gender; the item catalog (search,
//! category, weapon kind, sort, favorites, Check Updates) with Equip; saved outfits; the color
//! editor and its themed presets. Everything it changes is client side.
//!
//! The catalog, favorites and outfits are `turu_bot::cosmetics::Library` (one per process).
//! What this adds per tab ([`State`]) is what the game actions report back (the quick bar's
//! outcome, the catalog's status, colors read, the room's players) and a native page's catalog
//! query with its filtered list. Every function locks the state briefly and never while calling
//! the game or the library's loading, so a shell may call them from its frame. The egui window
//! keeps its own filter (several pages of it, its selection) and filters with [`filter`].

use super::scheduler::{Note, Tint};
use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::cosmetics::avatar::{self, ColorChange, Colors};
use turu_bot::cosmetics::{CosmeticItem, CosmeticSet, Library, Slot, WEAPON_KINDS, charpage, group_digits};

/// How often the room's players are read for the copy list.
pub const PLAYERS_REFRESH: Duration = Duration::from_secs(3);
/// A native page's catalog page.
pub const PAGE_SIZE: usize = 50;

/// The catalog's categories.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Category {
    #[default]
    All,
    Slot(Slot),
    Favorites,
}

/// The categories, with their words (the chips, a drop-down).
pub const CATEGORIES: [(Category, &str); 8] = [
    (Category::All, "All"),
    (Category::Slot(Slot::Armor), "Armor"),
    (Category::Slot(Slot::Weapon), "Weapon"),
    (Category::Slot(Slot::Helm), "Helm"),
    (Category::Slot(Slot::Cape), "Cape"),
    (Category::Slot(Slot::Pet), "Pet"),
    (Category::Slot(Slot::Rune), "Ground Rune"),
    (Category::Favorites, "Favorites"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sort {
    #[default]
    Newest,
    Oldest,
    NameAsc,
    NameDesc,
}

pub const SORTS: [(Sort, &str); 4] = [
    (Sort::Newest, "Newest first"),
    (Sort::Oldest, "Oldest first"),
    (Sort::NameAsc, "Name A to Z"),
    (Sort::NameDesc, "Name Z to A"),
];

/// What a catalog list is filtered by.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Filter {
    pub query: String,
    pub category: Category,
    /// Within Weapon: one kind ("Sword") or all.
    pub weapon_kind: Option<&'static str>,
    pub sort: Sort,
}

/// VibeSkua `FilterCatalog`: the list the filter shows (category, search by ID when it is a
/// number else by name, sort), as places in the source it returns beside them.
pub fn filter(library: &Library, f: &Filter) -> (Arc<Vec<CosmeticItem>>, Vec<u32>) {
    let favorites = f.category == Category::Favorites;
    let source = if favorites {
        Arc::new(library.favorites())
    } else {
        library.catalog()
    };
    let query = f.query.trim().to_lowercase();
    let id_query = f.query.trim().parse::<i64>().ok();
    let matches = |item: &CosmeticItem| {
        let in_category = match f.category {
            Category::Slot(Slot::Weapon) => {
                item.is_in(Slot::Weapon) && f.weapon_kind.is_none_or(|kind| item.kind() == kind)
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
    let mut filtered: Vec<u32> = source
        .iter()
        .enumerate()
        .filter(|(_, item)| matches(item))
        .map(|(i, _)| i as u32)
        .collect();
    let item = |i: &u32| &source[*i as usize];
    match f.sort {
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
    (source, filtered)
}

/// A native page's catalog: its filter, page, and the list as last filtered (with the library's
/// revision then).
#[derive(Default)]
struct Page {
    filter: Filter,
    page: usize,
    made: Option<(Filter, u64)>,
    source: Arc<Vec<CosmeticItem>>,
    filtered: Vec<u32>,
}

#[derive(Default)]
pub struct State {
    /// The quick bar's outcome (VibeSkua `QuickActionBarStatus`) and whether an action runs.
    pub status: String,
    pub busy: bool,
    pub catalog_status: String,
    pub catalog_busy: bool,
    /// Colors read from the avatar, a count that changes with each read, and whether the read
    /// asked to fill all-black ones too.
    pub colors: Option<Colors>,
    pub colors_count: u32,
    pub colors_force: bool,
    pub cc_status: String,
    pub room_players: Vec<String>,
    reading_players: bool,
    players_read_at: Option<Instant>,
    page: Page,
}

pub static STATE: PerTab<State> = PerTab::new();

/// How the room's players are shown: by name, or "Player 1", "Player 2"... in Streamer Mode
/// (Copy still goes by the name behind each).
pub fn player_labels(bot: &Bot, players: &[String]) -> Vec<String> {
    // Turu: VibeSkua's Cosmetics plugin lists the names whatever Streamer Mode says.
    if bot.opts().streamer_mode {
        (1..=players.len()).map(|n| format!("Player {n}")).collect()
    } else {
        players.to_vec()
    }
}

/// An outcome as the quick bar shows it: in Streamer Mode, without the player's name the copy
/// names ("Copied look from Hero!", "Player 'Hero' was not found ...").
fn shown_outcome(bot: &Bot, text: String) -> String {
    if !bot.opts().streamer_mode {
        text
    } else if text.starts_with("Copied look from ") {
        "Copied the look!".into()
    } else if text.starts_with("Player '") && text.contains("was not found") {
        "That player was not found in the current map.".into()
    } else {
        text
    }
}

/// Runs a game action off the caller's thread; a failure goes to the script log as VibeSkua's
/// "[Cosmetics] ... error" lines do.
pub fn act_in_game(
    bot: &Arc<Bot>,
    what: &'static str,
    action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static,
) {
    crate::turu::spawn_action_for(bot.clone(), "Cosmetics", move |bot| {
        if let Err(e) = action(bot) {
            bot.log(format!("[Cosmetics] {what} error: {e}"));
        }
        Ok(())
    });
}

/// VibeSkua `CosmeticsWindow_Loaded`: remember the real look, read the colors, load the
/// catalog (once per process).
pub fn on_open(bot: &Arc<Bot>, library: &Arc<Library>) {
    read_colors(bot, false);
    let state = STATE.of(bot);
    if library.is_loaded() {
        lock(&state).catalog_status = format!("{} items (Local Machine)", group_digits(library.catalog().len()));
        return;
    }
    {
        let mut s = lock(&state);
        if s.catalog_busy {
            return;
        }
        s.catalog_busy = true;
        s.catalog_status = "Streaming local catalog...".into();
    }
    let library = library.clone();
    std::thread::spawn(move || {
        let result = library.ensure_loaded();
        let mut s = lock(&state);
        s.catalog_busy = false;
        s.catalog_status = result.unwrap_or_else(|e| e);
    });
}

/// Check Updates: download the catalog again and keep it if it has new items.
pub fn check_updates(bot: &Arc<Bot>, library: &Arc<Library>) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        if s.catalog_busy {
            return;
        }
        s.catalog_busy = true;
        s.catalog_status = "Checking for updates...".into();
    }
    let library = library.clone();
    std::thread::spawn(move || {
        let result = library.check_for_updates();
        let mut s = lock(&state);
        s.catalog_busy = false;
        s.catalog_status = result.unwrap_or_else(|e| e);
    });
}

/// Reads the avatar's colors (and remembers the real look first); `force` fills all-black ones.
pub fn read_colors(bot: &Arc<Bot>, force: bool) {
    let state = STATE.of(bot);
    act_in_game(bot, "Read colors", move |bot| {
        avatar::capture_original_state(bot)?;
        let colors = avatar::current_colors(bot)?;
        let mut s = lock(&state);
        s.colors = Some(colors);
        s.colors_count = s.colors_count.wrapping_add(1);
        s.colors_force = force;
        if force {
            s.cc_status = "Read colors from avatar.".into();
        }
        Ok(())
    });
}

/// The room's players, read again every few seconds (or now, `fresh`).
pub fn read_players(bot: &Arc<Bot>, fresh: bool) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        let due = fresh || s.players_read_at.is_none_or(|t| t.elapsed() >= PLAYERS_REFRESH);
        if !due || s.reading_players {
            return;
        }
        s.players_read_at = Some(Instant::now());
        s.reading_players = true;
    }
    crate::turu::spawn_action_for(bot.clone(), "Cosmetics players", move |bot| {
        let names = avatar::room_player_names(bot);
        let mut s = lock(&state);
        s.reading_players = false;
        s.room_players = names?;
        Ok(())
    });
}

/// A quick-bar action that reports a message and reads the colors after it worked.
pub fn quick(bot: &Arc<Bot>, start: &str, action: impl FnOnce(&Arc<Bot>) -> Result<String, String> + Send + 'static) {
    let state = STATE.of(bot);
    {
        let mut s = lock(&state);
        if s.busy {
            return;
        }
        s.status = start.into();
        s.busy = true;
    }
    crate::turu::spawn_action_for(bot.clone(), "Cosmetics", move |bot| {
        let result = action(bot);
        let colors = if result.is_ok() {
            avatar::current_colors(bot).ok()
        } else {
            None
        };
        let mut s = lock(&state);
        s.busy = false;
        s.status = shown_outcome(bot, result.unwrap_or_else(|e| e));
        if let Some(colors) = colors {
            s.colors = Some(colors);
            s.colors_count = s.colors_count.wrapping_add(1);
            s.colors_force = false;
        }
        Ok(())
    });
}

pub fn copy_player(bot: &Arc<Bot>, library: &Arc<Library>, name: String) {
    let library = library.clone();
    let start = if bot.opts().streamer_mode {
        "Copying the look...".to_owned()
    } else {
        format!("Copying look from {name}...")
    };
    quick(bot, &start, move |bot| {
        avatar::copy_player_look_by_name(bot, &library, &name)
    });
}

pub fn copy_target(bot: &Arc<Bot>, library: &Arc<Library>) {
    let library = library.clone();
    quick(bot, "Reading target avatar...", move |bot| {
        avatar::copy_target_look(bot, &library)
    });
}

/// Turu: any character's look from its account.aq.com page.
pub fn copy_page(bot: &Arc<Bot>, name: String) {
    let start = if bot.opts().streamer_mode {
        "Reading the character page...".to_owned()
    } else {
        format!("Reading {name}'s character page...")
    };
    quick(bot, &start, move |bot| {
        let page = charpage::fetch(&name)?;
        avatar::copy_char_page(bot, &page)
    });
}

/// Reset to Real Gear: the equipped inventory items, colors and gender again.
pub fn reset_gear(bot: &Arc<Bot>) {
    quick(bot, "Reverting to real inventory gear & colors...", |bot| {
        avatar::revert_to_real_gear(bot)
            .map(|()| "Reverted to actual equipped gear & colors!".to_owned())
            .map_err(|e| format!("Reset to real gear error: {e}"))
    });
}

/// Female, male, or the other one (`None`).
pub fn gender(bot: &Arc<Bot>, gender: Option<char>) {
    act_in_game(bot, "Gender swap", move |bot| avatar::swap_gender(bot, gender, false));
}

pub fn equip(bot: &Arc<Bot>, item: CosmeticItem) {
    act_in_game(bot, "Equip", move |bot| avatar::equip(bot, &item));
}

pub fn equip_set(bot: &Arc<Bot>, set: CosmeticSet) {
    act_in_game(bot, "Equip set", move |bot| avatar::equip_set(bot, &set));
}

/// Save Current Look as an outfit called `name`.
pub fn save_look(bot: &Arc<Bot>, library: &Arc<Library>, name: String) {
    let library = library.clone();
    act_in_game(bot, "Capture look", move |bot| {
        library.add_set(avatar::capture_current_look(bot, &library, &name)?);
        Ok(())
    });
}

/// `#RRGGBB` (or without `#`), `None` for an empty box (that color stays).
pub fn parse_hex(text: &str) -> Option<i64> {
    let digits = text.trim().trim_start_matches('#');
    if digits.is_empty() {
        return None;
    }
    i64::from_str_radix(digits, 16).ok()
}

/// Apply Palette: the six boxes ("" leaves that color).
pub fn set_colors(bot: &Arc<Bot>, hex: &[String; 6]) {
    let colors = ColorChange(hex.each_ref().map(|h| parse_hex(h)));
    act_in_game(bot, "SetColors", move |bot| avatar::set_colors(bot, colors, true));
}

/// VibeSkua's themed presets: name, description, base, trim, accessory and eye.
pub const PRESETS: [(&str, &str, [&str; 4]); 8] = [
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

/// The color rows, in `Colors`' order: label and what it colors.
pub const COLOR_ROWS: [(&str, &str); 6] = [
    ("Base Color", "Primary Armor / Clothes"),
    ("Trim Color", "Secondary Highlights"),
    ("Accessory Color", "Gems, Accents, Glows"),
    ("Eye Color", "Pupils & Eyes"),
    ("Skin Color", "Body & Face Tone"),
    ("Hair Color", "Hair & Eyebrows"),
];

// --- the view a native page reads -----------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CosmeticsView {
    /// The room's other players, for Copy Player.
    pub players: Vec<String>,
    /// How each is shown: its name, or "Player n" in Streamer Mode.
    pub player_labels: Vec<String>,
    /// The quick bar's outcome ("" for none) and whether an action runs.
    pub status: String,
    pub busy: bool,
    pub catalog: CatalogView,
    pub outfits: Vec<OutfitView>,
    /// The six colors as last read ("#RRGGBB"; empty before a read), and a count that changes
    /// with each read.
    pub colors: Vec<String>,
    pub colors_count: u32,
    pub cc_status: String,
    pub color_rows: Vec<ColorRowView>,
    pub presets: Vec<PresetView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CatalogView {
    /// "45,210 items (Local Machine)", "Streaming local catalog...".
    pub status: String,
    pub busy: bool,
    /// The query as the slot has it, so the page knows its typing reached it.
    pub query: String,
    pub category: String,
    pub categories: Vec<String>,
    /// "" for all kinds; offered within Weapon.
    pub weapon_kind: String,
    pub weapon_kinds: Vec<String>,
    pub sort: String,
    pub sorts: Vec<String>,
    /// How many match, the page shown (from 0) and how many pages.
    pub matches: u32,
    pub page: u32,
    pub pages: u32,
    pub items: Vec<CosmeticItemView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CosmeticItemView {
    /// Its key for the actions.
    pub key: String,
    pub id: i64,
    pub name: String,
    /// "Armor", "Weapon · Sword", ...
    pub category: String,
    pub favorite: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct OutfitView {
    /// Its place in the list (the actions' `index`).
    pub position: u32,
    pub name: String,
    pub created: String,
    /// The slots it fills, each "Helm: Name".
    pub slots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct ColorRowView {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct PresetView {
    pub name: String,
    pub detail: String,
    /// Base, trim, accessory and eye ("#RRGGBB"); skin and hair stay.
    pub colors: Vec<String>,
}

fn category_words(c: Category) -> &'static str {
    CATEGORIES.iter().find(|(k, _)| *k == c).map_or("All", |(_, w)| w)
}

fn sort_words(s: Sort) -> &'static str {
    SORTS.iter().find(|(k, _)| *k == s).map_or("Newest first", |(_, w)| w)
}

fn outfit_view(i: usize, set: &CosmeticSet) -> OutfitView {
    let slots = [Slot::Helm, Slot::Armor, Slot::Cape, Slot::Weapon, Slot::Pet, Slot::Rune]
        .into_iter()
        .filter_map(|slot| set.get(slot).map(|item| format!("{}: {}", slot.label(), item.name)))
        .collect();
    OutfitView {
        position: i as u32,
        name: set.name.clone(),
        created: set.created_label(),
        slots,
    }
}

pub fn view(bot: &Arc<Bot>) -> CosmeticsView {
    let library = Library::shared(&bot.paths);
    read_players(bot, false);
    let state = STATE.of(bot);
    // Filter outside the lock when the catalog or the query changed (the catalog is large).
    let (want, made) = {
        let s = lock(&state);
        (s.page.filter.clone(), s.page.made.clone())
    };
    let revision = library.revision();
    if made.as_ref() != Some(&(want.clone(), revision)) {
        let (source, filtered) = filter(&library, &want);
        let mut s = lock(&state);
        if s.page.filter == want {
            s.page.source = source;
            s.page.filtered = filtered;
            s.page.made = Some((want, revision));
        }
    }
    let s = lock(&state);
    let p = &s.page;
    let pages = p.filtered.len().div_ceil(PAGE_SIZE).max(1);
    let page = p.page.min(pages - 1);
    let items = p
        .filtered
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .map(|&i| {
            let item = &p.source[i as usize];
            CosmeticItemView {
                key: item.key(),
                id: item.id,
                name: item.name.clone(),
                category: item.display_category(),
                favorite: library.is_favorite(item),
            }
        })
        .collect();
    CosmeticsView {
        players: s.room_players.clone(),
        player_labels: player_labels(bot, &s.room_players),
        status: s.status.clone(),
        busy: s.busy,
        catalog: CatalogView {
            status: s.catalog_status.clone(),
            busy: s.catalog_busy,
            query: p.filter.query.clone(),
            category: category_words(p.filter.category).to_owned(),
            categories: CATEGORIES.iter().map(|(_, w)| (*w).to_owned()).collect(),
            weapon_kind: p.filter.weapon_kind.unwrap_or("").to_owned(),
            weapon_kinds: WEAPON_KINDS.iter().map(|k| (*k).to_owned()).collect(),
            sort: sort_words(p.filter.sort).to_owned(),
            sorts: SORTS.iter().map(|(_, w)| (*w).to_owned()).collect(),
            matches: p.filtered.len() as u32,
            page: page as u32,
            pages: pages as u32,
            items,
        },
        outfits: library
            .sets()
            .iter()
            .enumerate()
            .map(|(i, set)| outfit_view(i, set))
            .collect(),
        colors: s
            .colors
            .map(|c| c.to_array().map(|c| format!("#{c:06X}")).to_vec())
            .unwrap_or_default(),
        colors_count: s.colors_count,
        cc_status: s.cc_status.clone(),
        color_rows: COLOR_ROWS
            .iter()
            .map(|(label, detail)| ColorRowView {
                label: (*label).to_owned(),
                detail: (*detail).to_owned(),
            })
            .collect(),
        presets: PRESETS
            .iter()
            .map(|(name, detail, colors)| PresetView {
                name: (*name).to_owned(),
                detail: (*detail).to_owned(),
                colors: colors.iter().map(|c| (*c).to_owned()).collect(),
            })
            .collect(),
    }
}

/// The catalog item with `key`, in the catalog or the favorites.
fn item(library: &Library, key: &str) -> Result<CosmeticItem, String> {
    library
        .catalog()
        .iter()
        .find(|i| i.key() == key)
        .cloned()
        .or_else(|| library.favorites().into_iter().find(|i| i.key() == key))
        .ok_or_else(|| "That item isn't in the catalog any more".to_owned())
}

/// The outfit at `index` if it's still called `name`.
fn outfit(library: &Library, index: usize, name: &str) -> Result<CosmeticSet, String> {
    match library.sets().get(index) {
        Some(set) if set.name == name => Ok(set.clone()),
        _ => Err(format!("{name} moved or went: the list changed")),
    }
}

/// A change to the item list's filter from a shell: the fields given (`category` and `sort` by
/// their words). A new search, category or weapon kind starts at page 1 unless `page` is given.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct CosmeticsQuery {
    pub query: Option<String>,
    pub category: Option<String>,
    pub weapon_kind: Option<String>,
    pub sort: Option<String>,
    pub page: Option<u32>,
}

/// Cosmetics' actions (`POST /models/cosmetics/do`). `key` is an item's; an outfit is named by
/// its `position` and its `name`, so a list changed meanwhile isn't changed in the wrong place.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum CosmeticsAct {
    Open,
    /// Reads the players in the map.
    Players,
    CopyPlayer {
        name: String,
    },
    CopyTarget,
    /// Copies the character page of `name`.
    CopyPage {
        name: String,
    },
    Reset,
    /// "F", "M", or "" to swap.
    Gender {
        gender: String,
    },
    Query(CosmeticsQuery),
    Equip {
        key: String,
    },
    Favorite {
        key: String,
    },
    CheckUpdates,
    /// Saves what's worn as the outfit `name`.
    SaveLook {
        name: String,
    },
    EquipSet {
        position: u32,
        name: String,
    },
    DeleteSet {
        position: u32,
        name: String,
    },
    /// The six colors, as #RRGGBB ("" for none).
    Colors {
        hex: Vec<String>,
    },
    ReadColors,
}

pub fn act(bot: &Arc<Bot>, act: CosmeticsAct) -> Result<(), String> {
    let library = Library::shared(&bot.paths);
    match act {
        CosmeticsAct::Open => on_open(bot, &library),
        CosmeticsAct::Players => read_players(bot, true),
        CosmeticsAct::CopyPlayer { name } => copy_player(bot, &library, name),
        CosmeticsAct::CopyTarget => copy_target(bot, &library),
        CosmeticsAct::CopyPage { name } => {
            if name.trim().is_empty() {
                return Err("Name the character".into());
            }
            copy_page(bot, name.trim().to_owned());
        }
        CosmeticsAct::Reset => reset_gear(bot),
        CosmeticsAct::Gender { gender: g } => gender(bot, g.chars().next()),
        CosmeticsAct::Query(change) => {
            let state = STATE.of(bot);
            let mut s = lock(&state);
            let before = s.page.filter.clone();
            let f = &mut s.page.filter;
            if let Some(q) = change.query {
                f.query = q;
            }
            if let Some(c) = change.category {
                f.category = CATEGORIES
                    .iter()
                    .find(|(_, w)| *w == c)
                    .map(|(k, _)| *k)
                    .ok_or_else(|| format!("no category {c}"))?;
                if f.category != Category::Slot(Slot::Weapon) {
                    f.weapon_kind = None;
                }
            }
            if let Some(k) = change.weapon_kind {
                f.weapon_kind = WEAPON_KINDS.iter().find(|w| **w == k).copied();
            }
            if let Some(o) = change.sort {
                f.sort = SORTS
                    .iter()
                    .find(|(_, w)| *w == o)
                    .map(|(k, _)| *k)
                    .ok_or_else(|| format!("no sort {o}"))?;
            }
            // A new search or category starts at page 1.
            let new_view =
                before.query != f.query || before.category != f.category || before.weapon_kind != f.weapon_kind;
            if let Some(p) = change.page {
                s.page.page = p as usize;
            } else if new_view {
                s.page.page = 0;
            }
        }
        CosmeticsAct::Equip { key } => equip(bot, item(&library, &key)?),
        CosmeticsAct::Favorite { key } => library.toggle_favorite(&item(&library, &key)?),
        CosmeticsAct::CheckUpdates => check_updates(bot, &library),
        CosmeticsAct::SaveLook { name } => {
            if name.trim().is_empty() {
                return Err("Name the outfit".into());
            }
            save_look(bot, &library, name.trim().to_owned());
        }
        CosmeticsAct::EquipSet { position, name } => equip_set(bot, outfit(&library, position as usize, &name)?),
        CosmeticsAct::DeleteSet { position, name } => {
            let i = position as usize;
            outfit(&library, i, &name)?;
            library.remove_set(i);
        }
        CosmeticsAct::Colors { hex } => {
            let hex: [String; 6] = hex.try_into().map_err(|_| "hex takes six colors".to_owned())?;
            if let Some(bad) = hex.iter().find(|h| !h.trim().is_empty() && parse_hex(h).is_none()) {
                return Err(format!("{bad} isn't a color (#RRGGBB)"));
            }
            set_colors(bot, &hex);
            lock(&STATE.of(bot)).cc_status = "Applied the palette.".into();
        }
        CosmeticsAct::ReadColors => read_colors(bot, true),
    }
    Ok(())
}

/// The quick bar's outcome as every shell shows it.
pub fn outcome(status: &str) -> Option<Note> {
    (!status.is_empty()).then(|| Note {
        text: status.to_owned(),
        tint: if status.contains("error") || status.starts_with("Couldn't") {
            Tint::Danger
        } else {
            Tint::Muted
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A query as a shell's JSON is read.
    fn query(mut body: serde_json::Value) -> CosmeticsAct {
        body["action"] = "query".into();
        serde_json::from_value(body).unwrap()
    }

    #[test]
    fn hex_reads_with_or_without_a_hash() {
        assert_eq!(parse_hex("#FF0000"), Some(0xFF0000));
        assert_eq!(parse_hex("00ff00"), Some(0x00FF00));
        assert_eq!(parse_hex("  "), None);
        assert_eq!(parse_hex("#zz"), None);
    }

    #[test]
    fn a_query_starts_at_page_one_unless_it_names_a_page() {
        let bot = Arc::new(Bot::new());
        act(&bot, query(serde_json::json!({ "page": 3 }))).unwrap();
        assert_eq!(lock(&STATE.of(&bot)).page.page, 3);
        act(&bot, query(serde_json::json!({ "query": "questwalker" }))).unwrap();
        assert_eq!(lock(&STATE.of(&bot)).page.page, 0);
        act(
            &bot,
            query(serde_json::json!({ "category": "Weapon", "weaponKind": "Sword" })),
        )
        .unwrap();
        act(&bot, query(serde_json::json!({ "category": "Helm" }))).unwrap();
        assert_eq!(
            lock(&STATE.of(&bot)).page.filter.weapon_kind,
            None,
            "a kind is Weapon's only"
        );
        assert!(act(&bot, query(serde_json::json!({ "sort": "Sideways" }))).is_err());
        assert!(
            act(
                &bot,
                CosmeticsAct::Colors {
                    hex: ["#12", "", "", "", "", "nope"].map(String::from).to_vec()
                }
            )
            .is_err()
        );
    }

    /// Streamer Mode: the room's players are numbered, and the outcome names nobody.
    #[test]
    fn streamer_mode_numbers_the_rooms_players() {
        let bot = Bot::new();
        let players = vec!["Questwalker".to_owned(), "Brambleox".to_owned()];
        assert_eq!(player_labels(&bot, &players), players);
        let copied = || "Copied look from Questwalker!".to_owned();
        assert_eq!(shown_outcome(&bot, copied()), copied());
        bot.options.write().streamer_mode = true;
        assert_eq!(player_labels(&bot, &players), ["Player 1", "Player 2"]);
        assert_eq!(shown_outcome(&bot, copied()), "Copied the look!");
        let missing = "Player 'Questwalker' was not found in the current map.".to_owned();
        assert!(!shown_outcome(&bot, missing).contains("Questwalker"));
        assert_eq!(
            shown_outcome(&bot, "You are not logged in.".into()),
            "You are not logged in."
        );
    }
}
