//! Tools & Helpers > Junk Items (VibeSkua `JunkService`): the items to sell, kept in
//! `JunkItems.json` in the data folder in VibeSkua's format (a VibeSkua file can be copied
//! over), and selling the ones in the inventory (`bot.sellJunk()`, Sell all junk).
//!
//! The file is read each time it is used, so every tab sees the latest list.

use crate::Bot;
use crate::api::inventory::{self, Container};
use crate::api::{NameOrId, shops};
use crate::error::BotResult;
use crate::models::{ItemBase, lenient_i64, lenient_string};
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    pub name: String,
    pub category: String,
    /// An AC item (`IsAc`: the item's `bCoins`).
    pub ac: bool,
    /// One of its inventory stacks is equipped (`IsEquipped`).
    pub equipped: bool,
    /// Favourited in AQW (`IsFavorite`).
    pub favorite: bool,
    /// The largest stack size of its inventory and bank stacks (`MaxStack`).
    pub max_stack: i64,
    /// One of its inventory stacks is at least 80% full (`IsNearStackLimit`).
    pub near_stack_limit: bool,
    /// The quantity in the inventory, every stack (`InventoryQuantity`).
    pub inventory_quantity: i64,
    /// The quantity in the bank, every stack (`BankQuantity`).
    pub bank_quantity: i64,
    pub junk: bool,
    /// The quantity goal, 0 for none (`GoalQuantity`, saved in `InventoryGoals`).
    pub goal: i64,
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

    /// VibeSkua `IsGoalTracked`.
    pub fn goal_tracked(&self) -> bool {
        self.goal > 0
    }

    /// VibeSkua `GoalProgressDisplay`: inventory and bank together, at most the goal
    /// ("1,200 / 2,000").
    pub fn goal_progress(&self) -> String {
        let have = (self.inventory_quantity + self.bank_quantity).min(self.goal);
        format!("{} / {}", group_digits(have), group_digits(self.goal))
    }

    /// VibeSkua `QuantityDisplay`.
    pub fn quantity_display(&self) -> String {
        format!(
            "Inv: {}  |  Bank: {}",
            self.inventory_quantity, self.bank_quantity
        )
    }
}

/// C#'s `N0`: 1200 -> "1,200".
fn group_digits(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The category filter's first option (VibeSkua `CategoryOptions`).
pub const ALL_CATEGORIES: &str = "All Categories";

/// VibeSkua `AcFilterOptions`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcFilter {
    #[default]
    All,
    Ac,
    NonAc,
}

impl AcFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Ac, Self::NonAc];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All Items",
            Self::Ac => "AC Items",
            Self::NonAc => "Non-AC Items",
        }
    }
}

/// VibeSkua `HighlightFilterOptions`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Highlight {
    #[default]
    All,
    Goal,
    NearStackLimit,
    Favorites,
    Equipped,
    Junk,
}

impl Highlight {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Goal,
        Self::NearStackLimit,
        Self::Favorites,
        Self::Equipped,
        Self::Junk,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All Highlights",
            Self::Goal => "Goal Items",
            Self::NearStackLimit => "Near Stack Limit",
            Self::Favorites => "Favorites",
            Self::Equipped => "Equipped",
            Self::Junk => "Junk",
        }
    }
}

/// What the window shows (VibeSkua `ApplySearch`): the search, the category, the AC filter and
/// the highlight filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    /// ID, name or category containing it, ignoring case.
    pub search: String,
    /// A category, or [`ALL_CATEGORIES`].
    pub category: String,
    pub ac: AcFilter,
    pub highlight: Highlight,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            search: String::new(),
            category: ALL_CATEGORIES.into(),
            ac: AcFilter::All,
            highlight: Highlight::All,
        }
    }
}

impl Filter {
    pub fn matches(&self, entry: &Entry) -> bool {
        let search = self.search.trim().to_lowercase();
        let matches_search = search.is_empty()
            || entry.name.to_lowercase().contains(&search)
            || entry.id.to_string().contains(&search)
            || entry.category.to_lowercase().contains(&search);
        let matches_category =
            self.category == ALL_CATEGORIES || entry.category.eq_ignore_ascii_case(&self.category);
        let matches_ac = match self.ac {
            AcFilter::All => true,
            AcFilter::Ac => entry.ac,
            AcFilter::NonAc => !entry.ac,
        };
        let matches_highlight = match self.highlight {
            Highlight::All => true,
            Highlight::Goal => entry.goal_tracked(),
            Highlight::NearStackLimit => entry.near_stack_limit,
            Highlight::Favorites => entry.favorite,
            Highlight::Equipped => entry.equipped,
            Highlight::Junk => entry.junk,
        };
        matches_search && matches_category && matches_ac && matches_highlight
    }
}

/// VibeSkua `CategoryOptions`: [`ALL_CATEGORIES`], then the entries' categories (once each,
/// ignoring case) in order.
pub fn categories(entries: &[Entry]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut categories: Vec<String> = entries
        .iter()
        .filter(|e| seen.insert(e.category.to_lowercase()))
        .map(|e| e.category.clone())
        .collect();
    categories.sort_by_key(|c| c.to_lowercase());
    categories.insert(0, ALL_CATEGORIES.into());
    categories
}

/// VibeSkua's Set goal input (`GoalQuantityInput`): a whole number above 0.
pub fn parse_goal(text: &str) -> Option<i64> {
    text.trim()
        .parse::<i32>()
        .ok()
        .filter(|&q| q > 0)
        .map(i64::from)
}

/// VibeSkua `SetGoalForSelected` (`Some`) / `ClearGoalForSelected` (`None`): sets or removes
/// the goal of `ids` in the application settings (`InventoryGoals`) and saves them.
pub fn set_goals(bot: &Bot, ids: &[i64], quantity: Option<i64>) {
    {
        let mut settings = bot.settings.write();
        for &id in ids {
            match quantity {
                Some(quantity) => settings.inventory_goals.insert(id, quantity),
                None => settings.inventory_goals.remove(&id),
            };
        }
    }
    bot.save_settings();
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
/// inventory and bank items (one entry per ID, from its first stack) by name, marking the junk
/// ones. Quantities add up every stack; equipped and near the stack limit look at each
/// inventory stack; favourites are asked from the game, one item at a time.
pub fn entries(bot: &Bot) -> BotResult<Vec<Entry>> {
    let junk: HashSet<i64> = load(&bot.paths.junk_items_file())
        .iter()
        .map(|j| j.id)
        .collect();
    let goals = bot.settings.read().inventory_goals.clone();
    if !bot.state.lock().bank_loaded {
        inventory::bank_load(bot, true)?;
    }
    let inventory_items = inventory::items(bot, Container::Inventory)?;
    let bank_items = inventory::items(bot, Container::Bank)?;
    let mut items: Vec<&ItemBase> = inventory_items.iter().chain(&bank_items).collect();
    let mut seen = HashSet::new();
    items.retain(|i| seen.insert(i.id));
    // C#'s `OrderBy(Name)`: ordinal, and stable.
    items.sort_by(|a, b| a.name.cmp(&b.name));
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let id = item.id;
        let in_inventory = || inventory_items.iter().filter(move |c| c.id == id);
        let in_bank = || bank_items.iter().filter(move |c| c.id == id);
        entries.push(Entry {
            id,
            name: item.name.clone(),
            category: category_name(&item.category),
            ac: item.coins,
            equipped: in_inventory().any(|c| c.equipped),
            // VibeSkua `IsFavorited`: false when the game can't say.
            favorite: inventory::is_favorited(bot, id).unwrap_or(false),
            max_stack: in_inventory()
                .chain(in_bank())
                .map(|c| c.max_stack)
                .max()
                .unwrap_or(0),
            near_stack_limit: in_inventory()
                .any(|c| c.max_stack > 0 && c.quantity as f64 >= c.max_stack as f64 * 0.8),
            inventory_quantity: in_inventory().map(|c| c.quantity).sum(),
            bank_quantity: in_bank().map(|c| c.quantity).sum(),
            junk: junk.contains(&id),
            goal: goals.get(&id).copied().unwrap_or(0),
        });
    }
    Ok(entries)
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
