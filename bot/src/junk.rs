//! Tools & Helpers > Junk Items (VibeSkua `JunkService`): the items to sell, kept in
//! `JunkItems.json` in the data folder in VibeSkua's format (a VibeSkua file can be copied
//! over), and selling the ones in the inventory (`bot.sellJunk()`, Sell all junk).
//!
//! The file is read each time it is used, so every tab sees the latest list.

use crate::Bot;
use crate::api::inventory::{self, Container};
use crate::api::{NameOrId, shops};
use crate::error::BotResult;
use crate::models::{lenient_i64, lenient_string};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;

/// The list's file name in the data folder (VibeSkua `ClientFileSources.SkuaJunkItemsFile`).
pub const FILE_NAME: &str = "JunkItems.json";

/// One junk item (VibeSkua `JunkItemConfig`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JunkItem {
    #[serde(rename = "ID", deserialize_with = "lenient_i64", default)]
    pub id: i64,
    #[serde(rename = "Name", deserialize_with = "lenient_string", default)]
    pub name: String,
    /// VibeSkua's `ItemCategory` name ("Item", "QuestItem", ...).
    #[serde(rename = "Category", deserialize_with = "lenient_string", default)]
    pub category: String,
    /// Always empty in VibeSkua.
    #[serde(rename = "Meta", deserialize_with = "lenient_string", default)]
    pub meta: String,
}

/// A row of the Junk Items window: an inventory or bank item and whether it is junk (VibeSkua
/// `JunkItemEntry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub junk: bool,
}

impl Entry {
    pub fn to_junk(&self) -> JunkItem {
        JunkItem {
            id: self.id,
            name: self.name.clone(),
            category: self.category.clone(),
            meta: String::new(),
        }
    }
}

/// VibeSkua `EnsureLoaded`: the list, empty when the file is missing, blank or unreadable.
pub fn load(path: &Path) -> Vec<JunkItem> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(&text).unwrap_or_default()
}

/// VibeSkua `Save`: the list as indented JSON.
pub fn save(path: &Path, items: &[JunkItem]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(items).map_err(io::Error::other)?;
    fs::write(path, text)
}

/// VibeSkua `SetJunk`: replaces the list (the first of each ID) and saves it.
pub fn set(path: &Path, items: impl IntoIterator<Item = JunkItem>) -> io::Result<()> {
    let mut seen = HashSet::new();
    let items: Vec<JunkItem> = items.into_iter().filter(|i| seen.insert(i.id)).collect();
    save(path, &items)
}

/// VibeSkua `ItemBase.Category.ToString()`: `sType` as an `ItemCategory` name.
fn category_name(kind: &str) -> String {
    crate::quest_cache::category_of(Some(kind)).replace(' ', "")
}

/// VibeSkua `JunkItemsViewModel.Refresh`: loads the bank if it isn't loaded, then lists the
/// inventory and bank items (the first of each ID) by name, marking the junk ones.
pub fn entries(bot: &Bot) -> BotResult<Vec<Entry>> {
    let junk: HashSet<i64> = load(&bot.paths.junk_items_file())
        .iter()
        .map(|j| j.id)
        .collect();
    if !bot.state.lock().bank_loaded {
        inventory::bank_load(bot, true)?;
    }
    let mut items = inventory::items(bot, Container::Inventory)?;
    items.extend(inventory::items(bot, Container::Bank)?);
    let mut seen = HashSet::new();
    items.retain(|i| seen.insert(i.id));
    // C#'s `OrderBy(Name)`: ordinal, and stable.
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items
        .into_iter()
        .map(|i| Entry {
            junk: junk.contains(&i.id),
            category: category_name(&i.category),
            id: i.id,
            name: i.name,
        })
        .collect())
}

/// VibeSkua `SellAllJunk` (`Bot.SellJunk`): sells every unequipped inventory item on the list,
/// the whole stack each (`Shops.SellItem(id)`: its cooldown wait, favourites kept, then the
/// wait for the sale). Bank items aren't sold. Returns how many items it tried to sell.
pub fn sell_all(bot: &Bot) -> BotResult<usize> {
    let junk: HashSet<i64> = load(&bot.paths.junk_items_file())
        .iter()
        .map(|j| j.id)
        .collect();
    if junk.is_empty() {
        return Ok(0);
    }
    let to_sell: Vec<i64> = inventory::items(bot, Container::Inventory)?
        .into_iter()
        .filter(|i| junk.contains(&i.id) && !i.equipped)
        .map(|i| i.id)
        .collect();
    for &id in &to_sell {
        shops::sell_item(bot, NameOrId::Id(id), -1)?;
    }
    Ok(to_sell.len())
}
