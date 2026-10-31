//! Cosmetics (VibeSkua's Cosmetics plugin, `Skua.Plugin.Cosmetics`): shows any item on your
//! avatar without owning it, client side only, and keeps saved outfits.
//!
//! [`Library`] is the item catalog (VibeSkua's community sheet, cached in
//! `cosmetics/catalog.json`), the favorites and the saved outfits; one per data folder, shared
//! by the tabs. [`Look`] is a bot's state: what it shows over its real gear and its real colors
//! and gender. The avatar actions are in [`avatar`].

pub mod avatar;
pub mod charpage;

use crate::models::{lenient_i64, lenient_string};
use crate::paths::Paths;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

pub use avatar::Look;

/// The catalog VibeSkua reads (`CosmeticsService.OpenSheetApiUrl`): a Google Sheet as JSON.
pub const CATALOG_URL: &str =
    "https://opensheet.elk.sh/14H2J2nd8Pp-6Mof6rDIPjCPsE-iHEzkO_CC9OA0O0Us/Data";

/// VibeSkua's `ItemCategory` names (`Enum.TryParse` ignoring case).
const CATEGORIES: &[&str] = &[
    "Unknown",
    "Sword",
    "Axe",
    "Dagger",
    "Gun",
    "HandGun",
    "Rifle",
    "Bow",
    "Mace",
    "Gauntlet",
    "Polearm",
    "Staff",
    "Wand",
    "Whip",
    "Class",
    "Armor",
    "Helm",
    "Cape",
    "Pet",
    "Amulet",
    "Necklace",
    "Note",
    "Resource",
    "Item",
    "Misc",
    "QuestItem",
    "ServerUse",
    "House",
    "WallItem",
    "FloorItem",
    "Enhancement",
];

/// The weapon kinds the catalog's Weapon filter offers (VibeSkua's subcategory list).
pub const WEAPON_KINDS: &[&str] = &[
    "Sword", "Axe", "Dagger", "Gun", "HandGun", "Rifle", "Bow", "Mace", "Gauntlet", "Polearm",
    "Staff", "Wand", "Whip",
];

/// An equipment slot the plugin dresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    Armor,
    Helm,
    Cape,
    Weapon,
    Pet,
    Rune,
}

impl Slot {
    /// In the order VibeSkua equips a set.
    pub const ALL: [Slot; 6] = [
        Slot::Armor,
        Slot::Helm,
        Slot::Cape,
        Slot::Weapon,
        Slot::Pet,
        Slot::Rune,
    ];

    /// The key in `objData.eqp` (the item's `sES`).
    pub fn group(self) -> &'static str {
        match self {
            Slot::Armor => "co",
            Slot::Helm => "he",
            Slot::Cape => "ba",
            Slot::Weapon => "Weapon",
            Slot::Pet => "pe",
            Slot::Rune => "mi",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Slot::Armor => "Armor / Class",
            Slot::Helm => "Helm",
            Slot::Cape => "Cape",
            Slot::Weapon => "Weapon",
            Slot::Pet => "Pet",
            Slot::Rune => "Ground Rune",
        }
    }

    /// The slot of an `eqp` key, ignoring case; the class (`ar`) shows as the armor.
    pub fn from_group(group: &str) -> Option<Slot> {
        match group.to_lowercase().as_str() {
            "co" | "ar" => Some(Slot::Armor),
            "he" => Some(Slot::Helm),
            "ba" => Some(Slot::Cape),
            "weapon" => Some(Slot::Weapon),
            "pe" => Some(Slot::Pet),
            "mi" => Some(Slot::Rune),
            _ => None,
        }
    }
}

/// A catalog entry (VibeSkua `CosmeticItem`); the JSON names are the catalog's.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CosmeticItem {
    #[serde(deserialize_with = "lenient_i64", default)]
    pub id: i64,
    #[serde(deserialize_with = "lenient_string", default)]
    pub name: String,
    /// The SWF (`sFile`).
    #[serde(deserialize_with = "lenient_string", default)]
    pub path: String,
    /// The linkage name (`sLink`).
    #[serde(deserialize_with = "lenient_string", default)]
    pub link: String,
    /// The `eqp` key (`sES`): `co`, `ar`, `he`, `ba`, `Weapon`, `pe` or `mi`.
    #[serde(rename = "itemGroup", deserialize_with = "lenient_string", default)]
    pub item_group: String,
    /// `sType`: `Armor`, `Sword`, `Helm`, ...
    #[serde(rename = "category", deserialize_with = "lenient_string", default)]
    pub category: String,
    #[serde(deserialize_with = "lenient_string", default)]
    pub meta: String,
}

impl CosmeticItem {
    /// An item worn in game (VibeSkua `new CosmeticItem(ItemBase)`).
    pub fn from_game(item: &crate::models::ItemBase) -> Self {
        Self {
            id: item.id,
            name: item.name.clone(),
            path: item.file_link.clone(),
            link: item.file_name.clone(),
            item_group: item.equip_slot.clone(),
            category: item.category.clone(),
            meta: item.meta.clone(),
        }
    }

    /// VibeSkua `Category`: the `ItemCategory` name, `Sword` for a weapon of an unknown kind,
    /// else `Unknown`.
    pub fn kind(&self) -> &'static str {
        if let Some(known) = CATEGORIES
            .iter()
            .find(|c| c.eq_ignore_ascii_case(self.category.trim()))
        {
            known
        } else if self.item_group == "Weapon" {
            "Sword"
        } else {
            "Unknown"
        }
    }

    /// The category shown in the list (VibeSkua `DisplayCategory`).
    pub fn display_category(&self) -> String {
        let kind = self.kind();
        if self.item_group != "Weapon" {
            if kind == "Misc" {
                "Ground Rune".into()
            } else if self.category.trim().is_empty() {
                self.item_group.clone()
            } else {
                self.category.clone()
            }
        } else if kind == "HandGun" {
            "Weapon (Hand Gun)".into()
        } else {
            format!("Weapon ({})", self.category)
        }
    }

    /// Whether the item is listed under the slot's catalog filter (VibeSkua
    /// `RebuildCategoryIndices`).
    pub fn is_in(&self, slot: Slot) -> bool {
        let (kind, group) = (self.kind(), self.item_group.as_str());
        match slot {
            Slot::Armor => kind == "Armor" || kind == "Class" || group == "co" || group == "ar",
            Slot::Weapon => group == "Weapon",
            Slot::Helm => kind == "Helm" || group == "he",
            Slot::Cape => kind == "Cape" || group == "ba",
            Slot::Pet => kind == "Pet" || group == "pe",
            Slot::Rune => kind == "Misc" || group == "mi",
        }
    }

    /// Identifies a favorite (VibeSkua `GetItemKey`).
    pub fn key(&self) -> String {
        format!("{}_{}", self.id, self.path.trim().to_lowercase())
    }
}

/// A saved outfit (VibeSkua `CosmeticSet`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CosmeticSet {
    pub name: String,
    /// RFC 3339.
    pub created_at: String,
    #[serde(default)]
    pub gender: Option<char>,
    #[serde(default)]
    pub helm: Option<CosmeticItem>,
    #[serde(default)]
    pub armor: Option<CosmeticItem>,
    #[serde(default)]
    pub cape: Option<CosmeticItem>,
    #[serde(default)]
    pub weapon: Option<CosmeticItem>,
    #[serde(default)]
    pub pet: Option<CosmeticItem>,
    #[serde(default)]
    pub misc: Option<CosmeticItem>,
}

impl Default for CosmeticSet {
    fn default() -> Self {
        Self::named("Default Outfit")
    }
}

impl CosmeticSet {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            created_at: chrono::Local::now().to_rfc3339(),
            gender: None,
            helm: None,
            armor: None,
            cape: None,
            weapon: None,
            pet: None,
            misc: None,
        }
    }

    pub fn get(&self, slot: Slot) -> Option<&CosmeticItem> {
        match slot {
            Slot::Armor => self.armor.as_ref(),
            Slot::Helm => self.helm.as_ref(),
            Slot::Cape => self.cape.as_ref(),
            Slot::Weapon => self.weapon.as_ref(),
            Slot::Pet => self.pet.as_ref(),
            Slot::Rune => self.misc.as_ref(),
        }
    }

    pub fn set(&mut self, slot: Slot, item: Option<CosmeticItem>) {
        let place = match slot {
            Slot::Armor => &mut self.armor,
            Slot::Helm => &mut self.helm,
            Slot::Cape => &mut self.cape,
            Slot::Weapon => &mut self.weapon,
            Slot::Pet => &mut self.pet,
            Slot::Rune => &mut self.misc,
        };
        *place = item;
    }

    pub fn has_any_item(&self) -> bool {
        Slot::ALL.iter().any(|&slot| self.get(slot).is_some())
    }

    pub fn clear(&mut self) {
        for slot in Slot::ALL {
            self.set(slot, None);
        }
    }

    /// "Armor: X • Helm: Y" (VibeSkua `Summary`, armor first).
    pub fn summary(&self) -> String {
        let order = [
            (Slot::Armor, "Armor"),
            (Slot::Helm, "Helm"),
            (Slot::Cape, "Cape"),
            (Slot::Weapon, "Weapon"),
            (Slot::Pet, "Pet"),
            (Slot::Rune, "Rune"),
        ];
        let parts: Vec<String> = order
            .iter()
            .filter_map(|&(slot, label)| {
                self.get(slot).map(|item| format!("{label}: {}", item.name))
            })
            .collect();
        if parts.is_empty() {
            "Empty Loadout".into()
        } else {
            parts.join(" • ")
        }
    }

    /// `created_at` as "2026-10-31 14:05", or as stored when it doesn't parse.
    pub fn created_label(&self) -> String {
        chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|_| self.created_at.clone())
    }
}

#[derive(Default)]
struct Favorites {
    items: Vec<CosmeticItem>,
    keys: HashSet<String>,
}

/// The catalog, favorites and saved outfits of a data folder (VibeSkua `CosmeticsService`'s
/// files). Its calls do file and network work: keep them off the UI thread, but
/// [`Library::is_loaded`], [`Library::catalog`] and the getters only read memory.
pub struct Library {
    dir: PathBuf,
    /// Newest (highest ID) first.
    catalog: RwLock<Arc<Vec<CosmeticItem>>>,
    loaded: AtomicBool,
    load_lock: Mutex<()>,
    favorites: RwLock<Favorites>,
    sets: RwLock<Vec<CosmeticSet>>,
    /// Bumped whenever the catalog, the favorites or the sets change.
    revision: AtomicU64,
}

impl Library {
    /// The library of the data folder `paths` points at, shared by every bot using it.
    pub fn shared(paths: &Paths) -> Arc<Library> {
        static LIBRARIES: OnceLock<Mutex<HashMap<PathBuf, Arc<Library>>>> = OnceLock::new();
        let dir = paths.cosmetics_dir();
        LIBRARIES
            .get_or_init(Mutex::default)
            .lock()
            .entry(dir.clone())
            .or_insert_with(|| Arc::new(Library::new(dir)))
            .clone()
    }

    /// Reads the favorites and saved outfits in `dir`; the catalog waits for
    /// [`Library::ensure_loaded`].
    pub fn new(dir: PathBuf) -> Self {
        let favorites: Vec<CosmeticItem> = read_json(&dir.join(FAVORITES_FILE)).unwrap_or_default();
        let sets: Vec<CosmeticSet> = read_json(&dir.join(SETS_FILE)).unwrap_or_default();
        Self {
            dir,
            catalog: RwLock::default(),
            loaded: AtomicBool::new(false),
            load_lock: Mutex::new(()),
            favorites: RwLock::new(Favorites {
                keys: favorites.iter().map(CosmeticItem::key).collect(),
                items: favorites,
            }),
            sets: RwLock::new(sets),
            revision: AtomicU64::new(0),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.loaded.load(Ordering::Acquire)
    }

    pub fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    fn changed(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
    }

    /// Every item, newest first (empty until loaded).
    pub fn catalog(&self) -> Arc<Vec<CosmeticItem>> {
        self.catalog.read().clone()
    }

    /// Loads the saved catalog, or downloads it when there is none (VibeSkua
    /// `EnsureCatalogLoadedAsync`). Blocks; the message says what happened.
    pub fn ensure_loaded(&self) -> Result<String, String> {
        let _guard = self.load_lock.lock();
        if self.is_loaded() {
            return Ok(self.loaded_message());
        }
        match read_json::<Vec<CosmeticItem>>(&self.dir.join(CATALOG_FILE)) {
            Some(items) if !items.is_empty() => {
                self.replace_catalog(items);
                Ok(self.loaded_message())
            }
            _ => self.check_for_updates_locked(),
        }
    }

    fn loaded_message(&self) -> String {
        format!(
            "{} items (Local Machine)",
            group_digits(self.catalog.read().len())
        )
    }

    /// Downloads the catalog and keeps it when it has more items than the saved one (VibeSkua
    /// `CheckAndUpdateCatalogAsync`).
    pub fn check_for_updates(&self) -> Result<String, String> {
        let _guard = self.load_lock.lock();
        self.check_for_updates_locked()
    }

    fn check_for_updates_locked(&self) -> Result<String, String> {
        let remote = fetch_catalog().map_err(|e| format!("Update check failed: {e}"))?;
        self.update_from(remote)
    }

    /// Takes a downloaded catalog: kept (and saved) when it has more items than the current one
    /// or there is none yet.
    pub fn update_from(&self, remote: Vec<CosmeticItem>) -> Result<String, String> {
        if remote.is_empty() {
            return Err("Invalid catalog format.".into());
        }
        let current = self.catalog.read().len();
        let new = remote.len();
        if new <= current && current != 0 {
            return Ok(format!(
                "Catalog is up to date ({} items).",
                group_digits(current)
            ));
        }
        self.replace_catalog(remote);
        let catalog = self.catalog();
        if let Err(e) = write_json(&self.dir.join(CATALOG_FILE), &*catalog, false) {
            tracing::warn!("Couldn't save the cosmetics catalog: {e}");
        }
        Ok(if current == 0 {
            format!(
                "Loaded {} cosmetics to local catalog.",
                group_digits(catalog.len())
            )
        } else {
            format!(
                "Catalog updated! (+{} new items, Total: {})",
                group_digits(new - current),
                group_digits(catalog.len())
            )
        })
    }

    fn replace_catalog(&self, mut items: Vec<CosmeticItem>) {
        items.sort_by_key(|item| std::cmp::Reverse(item.id));
        *self.catalog.write() = Arc::new(items);
        self.loaded.store(true, Ordering::Release);
        self.changed();
    }

    /// The catalog's name for an SWF, ignoring case.
    pub fn name_of(&self, path: &str) -> Option<String> {
        self.catalog
            .read()
            .iter()
            .find(|item| item.path.eq_ignore_ascii_case(path))
            .map(|item| item.name.clone())
    }

    pub fn favorites(&self) -> Vec<CosmeticItem> {
        self.favorites.read().items.clone()
    }

    pub fn is_favorite(&self, item: &CosmeticItem) -> bool {
        self.favorites.read().keys.contains(&item.key())
    }

    /// Adds or removes a favorite and saves the list.
    pub fn toggle_favorite(&self, item: &CosmeticItem) {
        let key = item.key();
        let mut favorites = self.favorites.write();
        if favorites.keys.remove(&key) {
            favorites.items.retain(|x| x.key() != key);
        } else {
            favorites.keys.insert(key);
            favorites.items.push(item.clone());
        }
        if let Err(e) = write_json(&self.dir.join(FAVORITES_FILE), &favorites.items, true) {
            tracing::warn!("Couldn't save the cosmetics favorites: {e}");
        }
        drop(favorites);
        self.changed();
    }

    pub fn sets(&self) -> Vec<CosmeticSet> {
        self.sets.read().clone()
    }

    pub fn add_set(&self, set: CosmeticSet) {
        let mut sets = self.sets.write();
        sets.push(set);
        self.save_sets(&sets);
        drop(sets);
        self.changed();
    }

    /// Removes the outfit at `index`.
    pub fn remove_set(&self, index: usize) {
        let mut sets = self.sets.write();
        if index < sets.len() {
            sets.remove(index);
            self.save_sets(&sets);
        }
        drop(sets);
        self.changed();
    }

    fn save_sets(&self, sets: &[CosmeticSet]) {
        if let Err(e) = write_json(&self.dir.join(SETS_FILE), sets, true) {
            tracing::warn!("Couldn't save the cosmetics outfits: {e}");
        }
    }
}

const CATALOG_FILE: &str = "catalog.json";
const FAVORITES_FILE: &str = "favorites.json";
const SETS_FILE: &str = "outfits.json";

fn fetch_catalog() -> Result<Vec<CosmeticItem>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(35))
        .user_agent(concat!("Turu/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(CATALOG_URL)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let text = response.text().map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(e) => {
            tracing::warn!("Couldn't read {}: {e}", path.display());
            None
        }
    }
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T, pretty: bool) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .map_err(std::io::Error::other)?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, text)?;
    fs::rename(temp, path)
}

/// 67334 -> "67,334" (C#'s `N0`).
pub fn group_digits(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// .NET `Uri.EscapeDataString`: everything but `A-Z a-z 0-9 - _ . ~` as `%XX` (UTF-8).
pub fn escape_data(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64, name: &str, group: &str, category: &str) -> CosmeticItem {
        CosmeticItem {
            id,
            name: name.into(),
            path: format!("items/{name}.swf"),
            item_group: group.into(),
            category: category.into(),
            ..Default::default()
        }
    }

    #[test]
    fn catalog_rows_parse_with_string_ids() {
        let rows = r#"[{"Timestamp":"01/02/2023","name":"Default Sword","id":"1","path":"items/swords/sword01.swf","category":"Sword","itemGroup":"Weapon"},
            {"name":"No Class","id":"4","path":"peasant_skin.swf","category":"Armor","itemGroup":"ar","link":"Peasant","meta":"24"}]"#;
        let items: Vec<CosmeticItem> = serde_json::from_str(rows).unwrap();
        assert_eq!(items[0].id, 1);
        assert_eq!(items[1].link, "Peasant");
        assert_eq!(items[1].meta, "24");
        assert!(items[1].is_in(Slot::Armor));
        assert_eq!(items[0].display_category(), "Weapon (Sword)");
    }

    #[test]
    fn categories_follow_vibeskua() {
        assert_eq!(item(1, "a", "Weapon", "Odd").kind(), "Sword");
        assert_eq!(
            item(1, "a", "Weapon", "HandGun").display_category(),
            "Weapon (Hand Gun)"
        );
        assert_eq!(item(1, "a", "mi", "Misc").display_category(), "Ground Rune");
        assert_eq!(item(1, "a", "he", "").display_category(), "he");
        assert!(item(1, "a", "co", "Armor").is_in(Slot::Armor));
        assert!(!item(1, "a", "co", "Armor").is_in(Slot::Helm));
        assert_eq!(Slot::from_group("WEAPON"), Some(Slot::Weapon));
        assert_eq!(Slot::from_group("ar"), Some(Slot::Armor));
        assert_eq!(group_digits(67334), "67,334");
        assert_eq!(group_digits(100), "100");
        assert_eq!(escape_data("Horatio's Staff"), "Horatio%27s%20Staff");
    }

    #[test]
    fn library_keeps_the_bigger_catalog_and_saves_favorites_and_outfits() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path().to_owned());
        assert!(library.update_from(vec![]).is_err());
        let first = library
            .update_from(vec![item(1, "a", "he", "Helm"), item(5, "b", "ba", "Cape")])
            .unwrap();
        assert_eq!(first, "Loaded 2 cosmetics to local catalog.");
        assert_eq!(library.catalog()[0].id, 5, "newest first");
        let same = library
            .update_from(vec![item(1, "a", "he", "Helm")])
            .unwrap();
        assert_eq!(same, "Catalog is up to date (2 items).");
        let more = library
            .update_from(vec![
                item(1, "a", "he", "Helm"),
                item(5, "b", "ba", "Cape"),
                item(9, "c", "pe", "Pet"),
            ])
            .unwrap();
        assert_eq!(more, "Catalog updated! (+1 new items, Total: 3)");
        assert_eq!(library.name_of("ITEMS/c.swf").as_deref(), Some("c"));

        let helm = library.catalog()[2].clone();
        library.toggle_favorite(&helm);
        let mut outfit = CosmeticSet::named("Mine");
        outfit.set(Slot::Helm, Some(helm.clone()));
        outfit.gender = Some('F');
        library.add_set(outfit);

        let again = Library::new(dir.path().to_owned());
        assert!(!again.is_loaded());
        assert_eq!(again.ensure_loaded().unwrap(), "3 items (Local Machine)");
        assert!(again.is_favorite(&helm));
        let sets = again.sets();
        assert_eq!(sets[0].summary(), "Helm: a");
        assert_eq!(sets[0].gender, Some('F'));
        again.toggle_favorite(&helm);
        assert!(again.favorites().is_empty());
        again.remove_set(0);
        assert!(Library::new(dir.path().to_owned()).sets().is_empty());
    }
}
