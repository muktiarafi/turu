//! Tools > Grabber (VibeSkua `GrabberService` and the Grabber view models): reads game data
//! (shops, quests, items, monsters) into lists for writing scripts, the one-line and detailed
//! texts Copy and Export write, and the actions each list offers.

use crate::Bot;
use crate::api::inventory::{self, Container};
use crate::api::{NameOrId, combat, map, monsters, player, quests, shops, wait};
use crate::error::BotResult;
use crate::map_items::{self, Scanner};
use crate::models::{ItemBase, MapItem, Monster, Quest, ShopInfo, ShopItem};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A Grabber list (VibeSkua `GrabberTypes`), in the sidebar's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GrabKind {
    ShopItems,
    ShopIds,
    Quests,
    MapItems,
    Inventory,
    House,
    TempInventory,
    Bank,
    CellMonsters,
    MapMonsters,
}

impl GrabKind {
    pub const ALL: [Self; 10] = [
        Self::ShopItems,
        Self::ShopIds,
        Self::Quests,
        Self::MapItems,
        Self::Inventory,
        Self::House,
        Self::TempInventory,
        Self::Bank,
        Self::CellMonsters,
        Self::MapMonsters,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::ShopItems => "Shop Items",
            Self::ShopIds => "Shop IDs",
            Self::Quests => "Quests",
            Self::MapItems => "GetMap Item IDs",
            Self::Inventory => "Inventory",
            Self::House => "House Inventory",
            Self::TempInventory => "Temp Inventory",
            Self::Bank => "Bank Items",
            Self::CellMonsters => "Cell Monsters",
            Self::MapMonsters => "Map Monsters",
        }
    }

    /// The sidebar group the list sits under.
    pub fn group(self) -> &'static str {
        match self {
            Self::ShopItems | Self::ShopIds => "SHOPS",
            Self::Quests | Self::MapItems => "QUESTS",
            Self::Inventory | Self::House | Self::TempInventory | Self::Bank => "ITEMS",
            Self::CellMonsters | Self::MapMonsters => "MONSTERS",
        }
    }

    /// Whether the list offers Multi-select.
    pub fn select_multiple(self) -> bool {
        !matches!(self, Self::ShopIds | Self::TempInventory)
    }

    /// The list's action buttons, left to right.
    pub fn actions(self) -> &'static [GrabAction] {
        match self {
            Self::ShopItems => &[GrabAction::Buy],
            Self::ShopIds => &[GrabAction::LoadShop],
            Self::Quests => &[
                GrabAction::Open,
                GrabAction::Accept,
                GrabAction::Register,
                GrabAction::FakeComplete,
                GrabAction::UnregisterAll,
            ],
            Self::MapItems => &[GrabAction::Open, GrabAction::Accept, GrabAction::GetMapItem],
            Self::Inventory => &[
                GrabAction::Equip,
                GrabAction::Sell,
                GrabAction::SellAll,
                GrabAction::ToBank,
            ],
            Self::House => &[GrabAction::HouseToBank],
            Self::TempInventory => &[],
            Self::Bank => &[GrabAction::ToInventory],
            Self::CellMonsters => &[GrabAction::Kill],
            Self::MapMonsters => &[GrabAction::Kill, GrabAction::TeleportTo],
        }
    }
}

/// What a Grabber button does (VibeSkua `GrabberService`'s tasks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabAction {
    Buy,
    LoadShop,
    Open,
    Accept,
    Register,
    FakeComplete,
    UnregisterAll,
    GetMapItem,
    Equip,
    Sell,
    SellAll,
    ToBank,
    HouseToBank,
    ToInventory,
    Kill,
    TeleportTo,
}

impl GrabAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Buy => "Buy",
            Self::LoadShop => "Load Shop",
            Self::Open => "Open",
            Self::Accept => "Accept",
            Self::Register => "Register",
            Self::FakeComplete => "Fake Complete",
            Self::UnregisterAll => "Unregister All",
            Self::GetMapItem => "Get Map Item",
            Self::Equip => "Equip",
            Self::Sell => "Sell",
            Self::SellAll => "Sell All",
            Self::ToBank | Self::HouseToBank => "To Bank",
            Self::ToInventory => "To Inventory",
            Self::Kill => "Kill",
            Self::TeleportTo => "Teleport To",
        }
    }

    /// Shown in the soft red style.
    pub fn destructive(self) -> bool {
        matches!(self, Self::Sell | Self::SellAll)
    }

    /// Fights: they run as a script thread and can't run beside a script.
    pub fn fights(self) -> bool {
        self == Self::Kill
    }
}

/// One grabbed row.
#[derive(Debug, Clone)]
pub enum Grabbed {
    Shop(ShopItem),
    ShopInfo(ShopInfo),
    Quest(Quest),
    /// An inventory, bank or house item (VibeSkua `InventoryItem`).
    Item(ItemBase),
    /// A temp inventory item (VibeSkua `ItemBase`: no category in its text).
    TempItem(ItemBase),
    Monster(Monster),
    MapItem(MapItem),
}

/// `1234567` as `1,234,567` (C# `N0`).
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// VibeSkua `ItemCategory`'s name for `sType` (ignoring case), else "Unknown".
fn category_name(kind: &str) -> String {
    let category = crate::quest_cache::category_of(Some(kind));
    category.replace(' ', "")
}

fn tags(coins: bool, upgrade: bool) -> String {
    format!(
        "{}{}",
        if coins { " [AC]" } else { "" },
        if upgrade { " [Member]" } else { "" }
    )
}

impl Grabbed {
    /// The ID in the list's ID column (a monster's `MonID`).
    pub fn id(&self) -> i64 {
        match self {
            Self::Shop(s) => s.item.id,
            Self::ShopInfo(s) => s.id,
            Self::Quest(q) => q.id,
            Self::Item(i) | Self::TempItem(i) => i.id,
            Self::Monster(m) => m.id,
            Self::MapItem(m) => m.id,
        }
    }

    /// The name searched and copied.
    pub fn name(&self) -> &str {
        match self {
            Self::Shop(s) => &s.item.name,
            Self::ShopInfo(s) => &s.name,
            Self::Quest(q) => &q.name,
            Self::Item(i) | Self::TempItem(i) => &i.name,
            Self::Monster(m) => &m.name,
            Self::MapItem(_) => "Map Item",
        }
    }

    /// VibeSkua `FormatGrabbedItem`: the one-line text Copy writes and the search matches.
    pub fn summary(&self) -> String {
        match self {
            Self::Quest(q) => format!(
                "[{}] {}{}{}",
                q.id,
                q.name,
                if q.slot > 0 {
                    format!(" (Slot: {})", q.slot)
                } else {
                    String::new()
                },
                if q.upgrade { " [Member]" } else { "" }
            ),
            Self::Item(i) => format!(
                "[{}] {} x{} | Category: {}{}",
                i.id,
                i.name.trim(),
                i.quantity,
                category_name(&i.category),
                tags(i.coins, i.upgrade)
            ),
            Self::Shop(s) => {
                let requirements = if s.turnin.is_empty() {
                    String::new()
                } else {
                    let list: Vec<String> = s
                        .turnin
                        .iter()
                        .map(|r| format!("[{}] {} x{}", r.id, r.name.trim(), r.quantity))
                        .collect();
                    format!(" Requirements: {}", list.join(", "))
                };
                format!(
                    "[{}] {} | Cost: {} {}{}{}",
                    s.item.id,
                    s.item.name.trim(),
                    thousands(s.cost),
                    if s.item.coins { "AC" } else { "Gold" },
                    if s.item.upgrade { " [Member]" } else { "" },
                    requirements
                )
            }
            Self::Monster(m) => format!(
                "[{}] {} | Cell: {} | HP: {}/{}",
                m.id, m.name, m.cell, m.hp, m.max_hp
            ),
            Self::ShopInfo(s) => format!("[{}] {}", s.id, s.name),
            Self::MapItem(m) => format!("[{}] Map Item (Quest: {})", m.id, m.quest_id),
            Self::TempItem(i) => format!(
                "[{}] {} x{}{}",
                i.id,
                i.name.trim(),
                i.quantity,
                tags(i.coins, i.upgrade)
            ),
        }
    }

    /// The row's text: the summary without its leading `[ID] ` (the ID has its own column).
    pub fn row(&self) -> String {
        let summary = self.summary();
        match summary
            .starts_with('[')
            .then(|| summary.find("] "))
            .flatten()
        {
            Some(end) => summary[end + 2..].to_owned(),
            None => summary,
        }
    }

    /// What the row holds, as scripts see it (`bot/js/turu.ts` names).
    pub fn json(&self) -> Value {
        let value = match self {
            Self::Shop(s) => serde_json::to_value(s),
            Self::ShopInfo(s) => serde_json::to_value(s),
            Self::Quest(q) => serde_json::to_value(q),
            Self::Item(i) | Self::TempItem(i) => serde_json::to_value(i),
            Self::Monster(m) => serde_json::to_value(m),
            Self::MapItem(m) => serde_json::to_value(m),
        };
        value.unwrap_or(Value::Null)
    }

    /// The DETAILS pane (VibeSkua's property grid): each field and its value; a list shows its
    /// length, then one `- …` line per entry.
    pub fn details(&self) -> Vec<(String, String)> {
        let mut rows = Vec::new();
        if let Value::Object(fields) = self.json() {
            for (key, value) in fields {
                match value {
                    Value::Array(entries) => {
                        let lines: Vec<String> = entries
                            .iter()
                            .map(|e| format!("- {}", entry_text(e)))
                            .collect();
                        let text = if lines.is_empty() {
                            "(none)".to_owned()
                        } else {
                            format!("{}\n{}", lines.len(), lines.join("\n"))
                        };
                        rows.push((key, text));
                    }
                    other => rows.push((key, scalar_text(&other))),
                }
            }
        }
        rows
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => "(none)".to_owned(),
        Value::String(s) => s.clone(),
        Value::Object(_) => "{…}".to_owned(),
        other => other.to_string(),
    }
}

/// One line for a list entry: `[id] name xqty` when it has those, else its JSON.
fn entry_text(value: &Value) -> String {
    match value {
        Value::Object(fields) => {
            let id = fields.get("id").map(scalar_text);
            let name = fields.get("name").and_then(Value::as_str).map(str::trim);
            let quantity = fields.get("quantity").and_then(Value::as_i64);
            match (id, name) {
                (Some(id), Some(name)) => match quantity {
                    Some(q) => format!("[{id}] {name} x{q}"),
                    None => format!("[{id}] {name}"),
                },
                _ => value.to_string(),
            }
        }
        other => scalar_text(other),
    }
}

/// Copy (VibeSkua `BuildExportText`): a heading, then each row's summary.
pub fn export_text(title: &str, items: &[&Grabbed]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut text = format!("=== {title} ({} items) ===\n", items.len());
    for item in items {
        text.push_str(&item.summary());
        text.push('\n');
    }
    text
}

/// Most levels of nested objects Export expands (VibeSkua `MaxExportDepth`).
const MAX_EXPORT_DEPTH: usize = 4;

fn append_fields(text: &mut String, value: &Value, depth: usize) {
    let Value::Object(fields) = value else {
        return;
    };
    let indent = " ".repeat(depth * 2);
    for (key, value) in fields {
        match value {
            Value::Array(entries) => {
                text.push_str(&format!("{indent}{key} ({}):\n", entries.len()));
                for entry in entries {
                    text.push_str(&format!("{indent}  - {}\n", entry_text(entry)));
                    if entry.is_object() && depth < MAX_EXPORT_DEPTH {
                        append_fields(text, entry, depth + 2);
                    }
                }
            }
            Value::Object(_) if depth < MAX_EXPORT_DEPTH => {
                text.push_str(&format!("{indent}{key}:\n"));
                append_fields(text, value, depth + 1);
            }
            other => text.push_str(&format!("{indent}{key}: {}\n", scalar_text(other))),
        }
    }
}

/// Export (VibeSkua `BuildDetailedExportText`): a heading, then per row its summary and every
/// field, nested lists expanded underneath.
pub fn detailed_export_text(title: &str, items: &[&Grabbed]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut text = format!("=== {title} ({} items) ===\n", items.len());
    for item in items {
        text.push('\n');
        text.push_str(&item.summary());
        text.push('\n');
        append_fields(&mut text, &item.json(), 1);
    }
    text
}

/// VibeSkua `Grab(type)`: reads the list from the game. GetMap Item IDs scans the map's SWF
/// with `scan` ([`map_items::find`]).
pub fn grab(bot: &Bot, kind: GrabKind, scan: &Scanner) -> BotResult<Vec<Grabbed>> {
    Ok(match kind {
        GrabKind::ShopItems => shops::items(bot)?.into_iter().map(Grabbed::Shop).collect(),
        GrabKind::ShopIds => bot
            .state
            .lock()
            .loaded_shops
            .iter()
            .cloned()
            .map(Grabbed::ShopInfo)
            .collect(),
        GrabKind::Quests => quests::tree(bot)?.into_iter().map(Grabbed::Quest).collect(),
        GrabKind::MapItems => map_items::find(bot, scan, false)?
            .into_iter()
            .map(Grabbed::MapItem)
            .collect(),
        GrabKind::Inventory => inventory::items(bot, Container::Inventory)?
            .into_iter()
            .map(Grabbed::Item)
            .collect(),
        GrabKind::House => inventory::items(bot, Container::House)?
            .into_iter()
            .map(Grabbed::Item)
            .collect(),
        GrabKind::TempInventory => inventory::items(bot, Container::TempInv)?
            .into_iter()
            .map(Grabbed::TempItem)
            .collect(),
        GrabKind::Bank => inventory::items(bot, Container::Bank)?
            .into_iter()
            .map(Grabbed::Item)
            .collect(),
        GrabKind::CellMonsters => monsters::current_available(bot)?
            .into_iter()
            .map(Grabbed::Monster)
            .collect(),
        GrabKind::MapMonsters => monsters::map_monsters(bot)?
            .into_iter()
            .map(Grabbed::Monster)
            .collect(),
    })
}

/// What a button needs before it runs, worked out from the selection alone.
#[derive(Debug, Clone, PartialEq)]
pub enum Prepared {
    /// Run it now.
    Run,
    /// Ask for a quantity first (VibeSkua's input dialog), then run with it.
    AskQuantity { title: String, prompt: String },
    /// Don't run: show this message box, with `progress` as the status text.
    Warn {
        title: String,
        message: String,
        progress: String,
    },
    /// Don't run: only this status text.
    Report(String),
}

fn warn(title: &str, message: impl Into<String>, progress: &str) -> Prepared {
    Prepared::Warn {
        title: title.to_owned(),
        message: message.into(),
        progress: progress.to_owned(),
    }
}

fn ac_warning() -> Prepared {
    warn(
        "AC Item",
        "Don't use this to buy AC items that aren't 0 AC.",
        "AC item - Cancelled",
    )
}

/// The quantity a sale may take: one for a class, else what you have.
fn sell_max(item: &ItemBase) -> i64 {
    if item.is_class() { 1 } else { item.quantity }
}

/// The checks VibeSkua makes before an action (and its input dialog), on `selected` (the
/// selected rows, in list order).
pub fn prepare(action: GrabAction, selected: &[&Grabbed]) -> Prepared {
    let none = |what: &str| Prepared::Report(format!("No {what} found/selected."));
    match action {
        GrabAction::UnregisterAll => Prepared::Run,
        _ if selected.is_empty() => match action {
            GrabAction::Open
            | GrabAction::Accept
            | GrabAction::Register
            | GrabAction::FakeComplete => none("quests"),
            GrabAction::GetMapItem => none("map items"),
            GrabAction::Kill | GrabAction::TeleportTo => none("monsters"),
            _ => none("items"),
        },
        GrabAction::Buy => {
            let items: Vec<&ShopItem> = selected
                .iter()
                .filter_map(|g| match g {
                    Grabbed::Shop(s) => Some(s),
                    _ => None,
                })
                .collect();
            if items.iter().any(|i| i.item.coins && i.cost > 0) {
                return ac_warning();
            }
            match items[..] {
                [item] => Prepared::AskQuantity {
                    title: format!("Buying {}", item.item.name.trim()),
                    prompt: format!(
                        "Buy quantity (Cost: {} {})",
                        item.cost,
                        if item.item.coins { "AC" } else { "Gold" }
                    ),
                },
                _ => Prepared::Run,
            }
        }
        GrabAction::Sell | GrabAction::SellAll => {
            if selected.len() > 1 {
                return warn(
                    "Selling item - Warning",
                    format!(
                        "ATTENTION - {} items selected!\nPlease sell 1 item at a time to prevent losses.",
                        selected.len()
                    ),
                    "Warning",
                );
            }
            let Grabbed::Item(item) = selected[0] else {
                return none("items");
            };
            if item.equipped {
                return warn("Sell item", "Cannot sell equipped item.", "");
            }
            if action == GrabAction::SellAll {
                return Prepared::Run;
            }
            Prepared::AskQuantity {
                title: format!("Selling {}", item.name.trim()),
                prompt: format!("Sell quantity (Currently has: {})", sell_max(item)),
            }
        }
        GrabAction::FakeComplete if selected.len() != 1 => {
            Prepared::Report("Please select exactly one quest to complete.".into())
        }
        GrabAction::GetMapItem => Prepared::AskQuantity {
            title: if selected.len() == 1 {
                format!("Getting {}", selected[0].id())
            } else {
                format!("Getting {} Map Items", selected.len())
            },
            prompt: "Quantity:".into(),
        },
        _ => Prepared::Run,
    }
}

/// A message box an action asks the UI to show (title, message).
pub type Notice = (String, String);

/// Sleeps `ms` in short steps; false when `cancel` was set meanwhile.
fn nap(bot: &Bot, ms: u64, cancel: &AtomicBool) -> BotResult<bool> {
    let mut left = ms;
    while left > 0 {
        if cancel.load(Ordering::Acquire) {
            return Ok(false);
        }
        let step = left.min(100);
        bot.sleep(step)?;
        left -= step;
    }
    Ok(!cancel.load(Ordering::Acquire))
}

/// VibeSkua `DefaultItemBaseTask`: the action on each item, a second apart.
fn each_item(
    bot: &Bot,
    identifier: &str,
    selected: &[&Grabbed],
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
    mut action: impl FnMut(i64) -> BotResult<bool>,
) -> BotResult<()> {
    progress(format!("{identifier} items..."));
    for (index, item) in selected.iter().enumerate() {
        progress(format!("{identifier} {}.", item.name().trim()));
        action(item.id())?;
        if index + 1 != selected.len() && !nap(bot, 1000, cancel)? {
            progress("Task cancelled.".into());
            return Ok(());
        }
    }
    Ok(())
}

/// The selected quests' IDs, or the selected map items' quests (VibeSkua `DefaultQuestTask`);
/// a map item with no known quest is left out.
fn quest_ids(selected: &[&Grabbed]) -> Vec<i64> {
    selected
        .iter()
        .filter_map(|g| match g {
            Grabbed::Quest(q) => Some(q.id),
            Grabbed::MapItem(m) if m.quest_id > 0 => Some(m.quest_id),
            _ => None,
        })
        .collect()
}

/// VibeSkua `Kill(monster, token)`: jumps to its cell when you aren't there, then
/// `Kill.Monster(monster, token)`: waits for it (10 tries), attacks it by map ID and waits
/// until it's dead or `cancel` is set (then drops the attack and the target).
fn kill(bot: &Bot, monster: &Monster, cancel: &AtomicBool) -> BotResult<()> {
    if !monster.cell.eq_ignore_ascii_case(&player::cell(bot)?) {
        map::jump(bot, &monster.cell, "Left", true, false)?;
    }
    let target = NameOrId::Id(monster.map_id);
    wait::for_monster_spawn(bot, target, 10)?;
    if !combat::attack(bot, target)? {
        return Ok(());
    }
    bot.sleep(bot.opts().action_delay.max(0) as u64)?;
    wait::until(
        bot,
        || {
            Ok(cancel.load(Ordering::Acquire)
                || !player::playing(bot)?
                || !player::has_target(bot)?)
        },
        -1,
    )?;
    if cancel.load(Ordering::Acquire) {
        combat::cancel_auto_attack(bot)?;
        combat::cancel_target(bot)?;
    }
    Ok(())
}

/// Runs a button's action on the selected rows (after [`prepare`] said `Run` or the quantity
/// was entered) the way VibeSkua's `GrabberService` does, reporting VibeSkua's status texts
/// through `progress`. Returns a message box to show (Buy without enough gold). `cancel`
/// (Cancel) stops between items and ends a kill.
pub fn run(
    bot: &Arc<Bot>,
    action: GrabAction,
    selected: &[&Grabbed],
    quantity: Option<i64>,
    progress: &dyn Fn(String),
    cancel: &AtomicBool,
) -> BotResult<Option<Notice>> {
    match action {
        GrabAction::Buy => {
            let items: Vec<&ShopItem> = selected
                .iter()
                .filter_map(|g| match g {
                    Grabbed::Shop(s) => Some(s),
                    _ => None,
                })
                .collect();
            let gold = player_gold(bot)?;
            if let [item] = items[..] {
                let quantity = quantity.unwrap_or(1).clamp(1, item.item.max_stack.max(1));
                let total = item.cost * quantity;
                if !item.item.coins && total > gold {
                    progress(format!("Not enough gold. Total: {}", thousands(total)));
                    return Ok(Some((
                        "Not enough gold".into(),
                        format!(
                            "Not enough gold to buy {quantity} {}.\nTotal: {}\nNeeded: {}",
                            item.item.name.trim(),
                            thousands(total),
                            thousands(total - gold)
                        ),
                    )));
                }
                shops::buy_item(bot, NameOrId::Id(item.item.id), quantity, item.shop_item_id)?;
                progress(format!("Bought {quantity} {}", item.item.name.trim()));
                return Ok(None);
            }
            let total: i64 = items.iter().filter(|i| !i.item.coins).map(|i| i.cost).sum();
            if total > gold {
                progress(format!("Not enough gold. Total: {}", thousands(total)));
                return Ok(Some((
                    "Not enough gold".into(),
                    format!(
                        "Not enough gold to buy the {} items.\nTotal: {}\nNeeded: {}",
                        items.len(),
                        thousands(total),
                        thousands(total - gold)
                    ),
                )));
            }
            for (index, item) in items.iter().enumerate() {
                shops::buy_item(bot, NameOrId::Id(item.item.id), -1, -1)?;
                progress(format!("Bought {}", item.item.name.trim()));
                if index + 1 != items.len() && !nap(bot, 1000, cancel)? {
                    progress("Task cancelled.".into());
                    break;
                }
            }
        }
        GrabAction::LoadShop => {
            if let Some(Grabbed::ShopInfo(shop)) = selected.first() {
                shops::load(bot, shop.id)?;
                progress(format!("Shop {} [{}] loaded.", shop.name, shop.id));
            }
        }
        GrabAction::Open | GrabAction::Accept | GrabAction::Register => {
            let ids = quest_ids(selected);
            let identifier = match action {
                GrabAction::Open => "Showing",
                GrabAction::Accept => "Accepting",
                _ => "Registering",
            };
            progress(format!("{identifier} {} quests...", ids.len()));
            match action {
                GrabAction::Open => quests::load(bot, &ids)?,
                // VibeSkua `EnsureAccept(params int[])`: each, `ActionDelay` apart.
                GrabAction::Accept => {
                    for id in &ids {
                        quests::ensure_accept(bot, *id)?;
                        if !nap(bot, bot.opts().action_delay.max(0) as u64, cancel)? {
                            progress("Task cancelled.".into());
                            return Ok(None);
                        }
                    }
                }
                _ => {
                    let quests: Vec<(i64, i64)> = ids.iter().map(|&id| (id, -1)).collect();
                    bot.quests.register(bot, &quests);
                }
            }
            progress("Finished.".into());
        }
        GrabAction::FakeComplete => {
            if let Some(Grabbed::Quest(quest)) = selected.first() {
                progress(format!("Fake completing {}...", quest.name));
                if quests::update_quest(bot, quest.id)? {
                    progress(format!("Fake completed {}.", quest.name));
                } else {
                    progress("Failed to complete quest.".into());
                }
            }
        }
        GrabAction::UnregisterAll => {
            progress("Working...".into());
            bot.quests.unregister_all();
            progress("Finished.".into());
        }
        GrabAction::GetMapItem => {
            let quantity = quantity.unwrap_or(1);
            for (index, item) in selected.iter().enumerate() {
                progress(format!("Getting {} x{quantity}", item.id()));
                for _ in 0..quantity.max(1) {
                    map::get_map_item(bot, item.id())?;
                }
                if index + 1 != selected.len() && !nap(bot, 1000, cancel)? {
                    progress("Task cancelled.".into());
                    return Ok(None);
                }
            }
            progress("Map items acquired".into());
        }
        GrabAction::Equip => {
            each_item(bot, "Equipping", selected, progress, cancel, |id| {
                inventory::equip_item(bot, NameOrId::Id(id))
            })?;
        }
        GrabAction::Sell | GrabAction::SellAll => {
            if let Some(Grabbed::Item(item)) = selected.first() {
                let max = sell_max(item);
                if action == GrabAction::SellAll {
                    progress(format!("Selling all {max} {}", item.name.trim()));
                    shops::sell_item(bot, NameOrId::Id(item.id), -1)?;
                    progress(format!("Sold {max} {}", item.name.trim()));
                } else {
                    let quantity = quantity.unwrap_or(1).clamp(1, max.max(1));
                    shops::sell_item(bot, NameOrId::Id(item.id), quantity)?;
                    progress(format!("Sold {quantity} {}", item.name.trim()));
                }
            }
        }
        GrabAction::ToBank => {
            each_item(bot, "Banking", selected, progress, cancel, |id| {
                inventory::to_bank(bot, Container::Inventory, NameOrId::Id(id))
            })?;
        }
        GrabAction::HouseToBank => {
            each_item(bot, "Banking", selected, progress, cancel, |id| {
                inventory::to_bank(bot, Container::House, NameOrId::Id(id))
            })?;
        }
        GrabAction::ToInventory => {
            each_item(bot, "Unbanking", selected, progress, cancel, |id| {
                inventory::bank_to_inventory(bot, NameOrId::Id(id))
            })?;
        }
        GrabAction::Kill => {
            let list: Vec<&Monster> = selected
                .iter()
                .filter_map(|g| match g {
                    Grabbed::Monster(m) => Some(m),
                    _ => None,
                })
                .collect();
            for (index, monster) in list.iter().enumerate() {
                progress(format!("Killing {}.", monster.name));
                kill(bot, monster, cancel)?;
                if cancel.load(Ordering::Acquire) {
                    progress("Task cancelled.".into());
                    return Ok(None);
                }
                if list.len() > 1 && index + 1 != list.len() && !nap(bot, 1000, cancel)? {
                    progress("Task cancelled.".into());
                    return Ok(None);
                }
                progress(format!("Killed {}.", monster.name));
            }
        }
        GrabAction::TeleportTo => {
            if let Some(Grabbed::Monster(monster)) = selected.first() {
                map::jump(bot, &monster.cell, "Left", true, false)?;
            }
        }
    }
    Ok(None)
}

fn player_gold(bot: &Bot) -> BotResult<i64> {
    bot.flash.get_or::<i64>("world.myAvatar.objData.intGold", 0)
}
