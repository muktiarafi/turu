//! `bot.inventory`, `bot.bank`, `bot.tempInv`, `bot.house`, `bot.invHelper` (VibeSkua
//! `ScriptInventory`, `ScriptBank`, `ScriptTempInv`, `ScriptHouseInv`, `ScriptInventoryHelper`
//! and the default members of `ICheckInventory`, `ICanBank`, `ICanEquip`, `ILimitedInventory`).
//!
//! Items are matched by ID, or by name trimmed and case-insensitively.

use super::{Args, NameOrId, map, ok, player, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::{ItemBase, ShopItem};
use serde_json::Value;
use std::sync::Arc;

/// `world.myAvatar.items`.
pub fn inventory_items(bot: &Bot) -> BotResult<Vec<ItemBase>> {
    items(bot, Container::Inventory)
}

/// The equipped class's name.
pub fn current_class(bot: &Bot) -> BotResult<Option<String>> {
    Ok(inventory_items(bot)?
        .into_iter()
        .find(|i| i.equipped && i.is_class())
        .map(|i| i.name))
}

/// Which item list an operation reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    Inventory,
    Bank,
    TempInv,
    House,
}

impl Container {
    /// The container behind a script module name (`"bank"` -> `Bank`).
    pub fn from_module(module: &str) -> Option<Self> {
        Some(match module {
            "inventory" => Self::Inventory,
            "bank" => Self::Bank,
            "tempInv" => Self::TempInv,
            "house" => Self::House,
            _ => return None,
        })
    }

    /// The game path of the container's item array.
    pub fn items_path(self) -> &'static str {
        match self {
            Self::Inventory => "world.myAvatar.items",
            Self::Bank => "world.bankinfo.items",
            Self::TempInv => "world.myAvatar.tempitems",
            Self::House => "world.myAvatar.houseitems",
        }
    }
}

// ---------------------------------------------------------------------------------------
// Matching (VibeSkua `ICheckInventory` default members), usable without the game
// ---------------------------------------------------------------------------------------

/// The item has this ID, or this name (trimmed, case-insensitive).
pub fn item_matches(item: &ItemBase, which: NameOrId) -> bool {
    match which {
        NameOrId::Name(name) => item.name_is(name),
        NameOrId::Id(id) => item.id == id,
    }
}

/// VibeSkua `GetItem`: the first matching item.
pub fn find_item<'a>(items: &'a [ItemBase], which: NameOrId) -> Option<&'a ItemBase> {
    items.iter().find(|i| item_matches(i, which))
}

/// VibeSkua `Contains(name|id, quantity)`: `quantity == 0` is always true; a class counts
/// at any quantity.
pub fn contains_in(items: &[ItemBase], which: NameOrId, quantity: i64) -> bool {
    quantity == 0
        || items
            .iter()
            .any(|i| item_matches(i, which) && (i.quantity >= quantity || i.is_class()))
}

/// VibeSkua `Contains(needAll, quantity, items...)`: all (`need_all`) or any of the items.
/// An empty list is `need_all`.
pub fn contains_many_in(
    items: &[ItemBase],
    which: &[NameOrId],
    quantity: i64,
    need_all: bool,
) -> bool {
    for &w in which {
        let has = contains_in(items, w, quantity);
        if has && !need_all {
            return true;
        }
        if !has && need_all {
            return false;
        }
    }
    need_all
}

/// VibeSkua `GetQuantity`: 0 when missing.
pub fn quantity_in(items: &[ItemBase], which: NameOrId) -> i64 {
    find_item(items, which).map_or(0, |i| i.quantity)
}

/// VibeSkua `IsMaxStack`: `quantity >= maxStack`.
pub fn is_max_stack_in(items: &[ItemBase], which: NameOrId) -> bool {
    find_item(items, which).is_some_and(|i| i.quantity >= i.max_stack)
}

/// Parses an item array leniently: entries that aren't items are skipped.
pub fn parse_items(values: Vec<Value>) -> Vec<ItemBase> {
    values
        .into_iter()
        .filter(Value::is_object)
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect()
}

/// A script argument holding one name/ID or an array of them.
pub(crate) fn name_or_ids(value: &Value) -> Vec<NameOrId<'_>> {
    fn one(v: &Value) -> Option<NameOrId<'_>> {
        match v {
            Value::String(s) => Some(NameOrId::Name(s.as_str())),
            Value::Number(n) => Some(NameOrId::Id(
                n.as_i64().or_else(|| n.as_f64().map(|f| f as i64))?,
            )),
            _ => None,
        }
    }
    match value {
        Value::Array(items) => items.iter().filter_map(one).collect(),
        other => one(other).into_iter().collect(),
    }
}

// ---------------------------------------------------------------------------------------
// Shared with other modules (kill, hunt, drops, quests). Signatures are fixed.
// ---------------------------------------------------------------------------------------

/// The container's items.
pub fn items(bot: &Bot, container: Container) -> BotResult<Vec<ItemBase>> {
    let raw: Option<Vec<Value>> = bot.flash.get_opt(container.items_path())?;
    Ok(parse_items(raw.unwrap_or_default()))
}

/// At least `quantity` of the item (VibeSkua `ICheckInventory.Contains`).
pub fn contains(bot: &Bot, container: Container, item: NameOrId, quantity: i64) -> BotResult<bool> {
    if quantity == 0 {
        return Ok(true);
    }
    Ok(contains_in(&items(bot, container)?, item, quantity))
}

/// 0 when missing.
pub fn quantity(bot: &Bot, container: Container, item: NameOrId) -> BotResult<i64> {
    Ok(quantity_in(&items(bot, container)?, item))
}

/// VibeSkua `GetItem` / `TryGetItem`.
pub fn get_item(bot: &Bot, container: Container, item: NameOrId) -> BotResult<Option<ItemBase>> {
    Ok(find_item(&items(bot, container)?, item).cloned())
}

// ---------------------------------------------------------------------------------------
// Slots (VibeSkua `ILimitedInventory`)
// ---------------------------------------------------------------------------------------

fn get_i64(bot: &Bot, path: &str) -> BotResult<i64> {
    let value: Option<Value> = bot.flash.get_opt(path)?;
    Ok(crate::api::packets::num(value.as_ref()).unwrap_or_default())
}

/// `iBagSlots`, `iBankSlots`, `iHouseSlots`; the temp inventory has no limit (0).
pub fn slots(bot: &Bot, container: Container) -> BotResult<i64> {
    match container {
        Container::Inventory => get_i64(bot, "world.myAvatar.objData.iBagSlots"),
        Container::Bank => get_i64(bot, "world.myAvatar.objData.iBankSlots"),
        Container::House => get_i64(bot, "world.myAvatar.objData.iHouseSlots"),
        Container::TempInv => Ok(0),
    }
}

/// Inventory: `inventoryBagUsedSlots` (bag pool only); bank: `iBankCount`; house and temp
/// inventory: the item count.
pub fn used_slots(bot: &Bot, container: Container) -> BotResult<i64> {
    match container {
        Container::Inventory => {
            let raw = bot.flash.call("inventoryBagUsedSlots", vec![])?;
            Ok(raw
                .trim()
                .trim_matches('"')
                .parse::<f64>()
                .map_or(0, |f| f as i64))
        }
        Container::Bank => get_i64(bot, "world.myAvatar.iBankCount"),
        Container::House => get_i64(bot, "world.myAvatar.houseitems.length"),
        Container::TempInv => Ok(items(bot, container)?.len() as i64),
    }
}

/// `Slots - UsedSlots` (never negative).
pub fn free_slots(bot: &Bot, container: Container) -> BotResult<i64> {
    Ok((slots(bot, container)? - used_slots(bot, container)?).max(0))
}

// ---------------------------------------------------------------------------------------
// Inventory (VibeSkua `ScriptInventory`)
// ---------------------------------------------------------------------------------------

/// `inventoryHasSpaceFor(itemJson, quantity, purchase, outgoingJson)`.
fn has_space_for_json(
    bot: &Bot,
    item: &Value,
    quantity: i64,
    purchase: bool,
    outgoing: Option<&Value>,
) -> BotResult<bool> {
    let mut args = vec![item.to_string().into(), quantity.into(), purchase.into()];
    if let Some(outgoing) = outgoing {
        args.push(outgoing.to_string().into());
    }
    bot.flash.call_bool("inventoryHasSpaceFor", args)
}

/// VibeSkua `Inventory.HasSpaceFor(item, quantity)`: the bag (or house) has room for it.
pub fn has_space_for(bot: &Bot, item: &ItemBase, quantity: i64) -> BotResult<bool> {
    has_space_for_json(bot, &item.game_json(), quantity, false, None)
}

/// [`has_space_for`] for a shop item (a purchase: the items it consumes free space).
pub fn has_space_for_purchase(bot: &Bot, item: &ShopItem, quantity: i64) -> BotResult<bool> {
    has_space_for_json(bot, &item.game_json(), quantity, true, None)
}

/// `inventory.hasSpaceFor(name|id)`: looks the item up in the inventory, bank, drops and
/// loaded shop (in that order); an unknown item is checked by its name/ID alone.
fn has_space_for_any(bot: &Bot, which: NameOrId, quantity: i64) -> BotResult<bool> {
    for container in [Container::Inventory, Container::Bank] {
        if let Some(item) = get_item(bot, container, which)? {
            return has_space_for(bot, &item, quantity);
        }
    }
    let drop = bot.state.lock().find_drop(which).cloned();
    if let Some(item) = drop {
        return has_space_for(bot, &item, quantity);
    }
    if let Some(item) = super::shops::items(bot)?
        .into_iter()
        .find(|s| item_matches(&s.item, which))
    {
        return has_space_for_purchase(bot, &item, quantity);
    }
    let json = match which {
        NameOrId::Name(name) => serde_json::json!({ "sName": name }),
        NameOrId::Id(id) => serde_json::json!({ "ItemID": id }),
    };
    has_space_for_json(bot, &json, quantity, false, None)
}

/// `hasInventoryCategories`: the client sorts the inventory into pools (`InvCat`).
pub fn has_categories(bot: &Bot) -> BotResult<bool> {
    bot.flash.call_bool("hasInventoryCategories", vec![])
}

/// VibeSkua `CanBank`: classes can't be banked when the client has inventory pools.
pub fn can_bank(bot: &Bot, item: &ItemBase) -> BotResult<bool> {
    Ok(!item.is_class() || !has_categories(bot)?)
}

/// `isFavoriteItem(id)` (AQW Lite favourites).
pub fn is_favorited(bot: &Bot, item_id: i64) -> BotResult<bool> {
    bot.flash.call_bool("isFavoriteItem", vec![item_id.into()])
}

/// The `equipItem` packet. VibeSkua calls `world.sendEquipItemRequest({ItemID})`, but Flash
/// arguments can't be objects here; the packet is what that function sends.
fn send_equip(bot: &Bot, item_id: i64) -> BotResult<()> {
    wait::for_action_cooldown(bot, "equipItem", 40)?;
    let room = map::room_id(bot)?;
    bot.flash
        .send_packet(&format!("%xt%zm%equipItem%{room}%{item_id}%"))
}

/// VibeSkua `EquipItem(name|id)`: equips it and waits (10 tries) until it's equipped.
/// By name, an item that isn't in the inventory is skipped. True when it's equipped.
pub fn equip_item(bot: &Bot, which: NameOrId) -> BotResult<bool> {
    let id = match which {
        NameOrId::Id(id) => id,
        NameOrId::Name(_) => match get_item(bot, Container::Inventory, which)? {
            Some(item) => item.id,
            None => return Ok(false),
        },
    };
    send_equip(bot, id)?;
    wait::for_item_equip(bot, NameOrId::Id(id), 10)?;
    Ok(get_item(bot, Container::Inventory, NameOrId::Id(id))?.is_some_and(|i| i.equipped))
}

/// VibeSkua `EquipUsableItem`: puts a consumable in the potion slot.
///
/// VibeSkua calls `world.equipUseableItem({ItemID, sDesc, sFile, sName})`; object arguments
/// aren't supported by the Flash bridge yet, so this sends the `equipItem` packet.
pub fn equip_usable_item(bot: &Bot, which: NameOrId) -> BotResult<()> {
    let Some(item) = get_item(bot, Container::Inventory, which)? else {
        return Ok(());
    };
    send_equip(bot, item.id)?;
    wait::for_item_equip(bot, NameOrId::Id(item.id), 10)?;
    Ok(())
}

/// Polls (`tries` x `sleep_ms`) until the item is or isn't in a container.
fn wait_contains(
    bot: &Bot,
    container: Container,
    which: NameOrId,
    present: bool,
    tries: i64,
    sleep_ms: u64,
) -> BotResult<bool> {
    wait::for_true(
        bot,
        || Ok(!player::playing(bot)? || contains(bot, container, which, 1)? == present),
        None,
        tries,
        sleep_ms,
    )
}

/// VibeSkua `ToBank(item)` of the inventory or the house: `bankFromInv`, then waits
/// (14 tries) until the item left. True when it's gone.
fn to_bank_item(bot: &Bot, from: Container, item: &ItemBase) -> BotResult<bool> {
    if !can_bank(bot, item)? {
        return Ok(false);
    }
    let room = map::room_id(bot)?;
    bot.flash.send_packet(&format!(
        "%xt%zm%bankFromInv%{room}%{}%{}%",
        item.id, item.char_item_id
    ))?;
    let name = NameOrId::Name(&item.name);
    // VibeSkua: ForInventoryToBank sleeps 50 ms per try, ForHouseInventoryToBank 100 ms.
    let sleep = if from == Container::House { 100 } else { 50 };
    wait_contains(bot, from, name, false, 14, sleep)?;
    Ok(!contains(bot, from, name, 1)?)
}

/// `inventory.toBank` / `house.toBank` (VibeSkua `ICanBank.ToBank(name|id)`).
pub fn to_bank(bot: &Bot, from: Container, which: NameOrId) -> BotResult<bool> {
    match get_item(bot, from, which)? {
        Some(item) => to_bank_item(bot, from, &item),
        None => Ok(false),
    }
}

/// VibeSkua `EnsureToBank`: retries up to `maximumTries`, sleeping `actionDelay`.
pub fn ensure_to_bank(bot: &Bot, from: Container, which: NameOrId) -> BotResult<bool> {
    let Some(item) = get_item(bot, from, which)? else {
        return Ok(false);
    };
    if !can_bank(bot, &item)? {
        return Ok(false);
    }
    let (tries, delay) = {
        let o = bot.opts();
        (o.maximum_tries.max(1) as i64, o.action_delay.max(0) as u64)
    };
    let mut i = 0;
    while !to_bank_item(bot, from, &item)? && !bot.should_exit() && player::playing(bot)? {
        i += 1;
        if i >= tries {
            break;
        }
        bot.sleep(delay)?;
    }
    Ok(!contains(bot, from, which, 1)?)
}

// ---------------------------------------------------------------------------------------
// Bank (VibeSkua `ScriptBank`)
// ---------------------------------------------------------------------------------------

fn bank_popup_open(bot: &Bot) -> BotResult<bool> {
    let label: Option<String> = bot.flash.get_opt("ui.mcPopup.currentLabel")?;
    Ok(label.is_some_and(|l| l.eq_ignore_ascii_case("Bank")))
}

/// VibeSkua `Bank.Open`: `world.toggleBank`, unless the bank window is open.
pub fn bank_open(bot: &Bot) -> BotResult<()> {
    if !player::world_loaded(bot)? || !player::logged_in(bot)? || bank_popup_open(bot)? {
        return Ok(());
    }
    bot.flash.call_game_function("world.toggleBank", vec![])
}

/// VibeSkua `Bank.Load(waitForLoad)`: sends `loadBank` (unless the bank window is open) and
/// waits (20 tries) for the `loadBank` answer.
pub fn bank_load(bot: &Bot, wait_for_load: bool) -> BotResult<()> {
    if bank_popup_open(bot)? {
        return Ok(());
    }
    // Forget an answer nobody waited for, so the wait below sees this one.
    bot.state.lock().signals.bank_loaded = false;
    let room = map::room_id(bot)?;
    bot.flash
        .send_packet(&format!("%xt%zm%loadBank%{room}%All%"))?;
    if wait_for_load {
        wait::for_bank_load(bot, 20)?;
    }
    Ok(())
}

/// VibeSkua `Bank.ToInventory(item)`: `bankToInv` if the inventory has space, then waits
/// (14 tries of 50 ms) until it arrived. True when it's in the inventory.
fn bank_to_inventory_item(bot: &Bot, item: &ItemBase) -> BotResult<bool> {
    if !has_space_for(bot, item, 1)? {
        return Ok(false);
    }
    let room = map::room_id(bot)?;
    bot.flash.send_packet(&format!(
        "%xt%zm%bankToInv%{room}%{}%{}%",
        item.id, item.char_item_id
    ))?;
    wait_contains(
        bot,
        Container::Inventory,
        NameOrId::Name(&item.name),
        true,
        14,
        50,
    )?;
    contains(bot, Container::Inventory, NameOrId::Id(item.id), 1)
}

/// `bank.toInventory(name|id)`. (VibeSkua's by-name/ID overloads always return false; this
/// returns whether the item arrived.)
pub fn bank_to_inventory(bot: &Bot, which: NameOrId) -> BotResult<bool> {
    match get_item(bot, Container::Bank, which)? {
        Some(item) => bank_to_inventory_item(bot, &item),
        None => Ok(false),
    }
}

/// VibeSkua `EnsureToInventory(item, loadBank)`: loads the bank first, then retries up to
/// `maximumTries`.
pub fn bank_ensure_to_inventory(bot: &Bot, which: NameOrId, load_bank: bool) -> BotResult<bool> {
    if load_bank {
        bank_load(bot, true)?;
    }
    let Some(item) = get_item(bot, Container::Bank, which)? else {
        return Ok(false);
    };
    if !has_space_for(bot, &item, 1)? {
        return Ok(false);
    }
    let (tries, delay) = {
        let o = bot.opts();
        (o.maximum_tries.max(1) as i64, o.action_delay.max(0) as u64)
    };
    let mut i = 0;
    while !bank_to_inventory_item(bot, &item)? && !bot.should_exit() && player::playing(bot)? {
        i += 1;
        if i >= tries {
            break;
        }
        bot.sleep(delay)?;
    }
    contains(bot, Container::Inventory, which, 1)
}

/// VibeSkua `Bank.Swap(invItem, bankItem)`: `bankSwapInv` when the inventory item can be
/// banked and the bank item fits in its place. True when the bank item is in the inventory.
pub fn bank_swap(bot: &Bot, inventory_item: NameOrId, bank_item: NameOrId) -> BotResult<bool> {
    let Some(bank) = get_item(bot, Container::Bank, bank_item)? else {
        return Ok(false);
    };
    let Some(inv) = get_item(bot, Container::Inventory, inventory_item)? else {
        return Ok(false);
    };
    if !can_bank(bot, &inv)?
        || !has_space_for_json(bot, &bank.game_json(), 1, false, Some(&inv.game_json()))?
    {
        return Ok(false);
    }
    let room = map::room_id(bot)?;
    bot.flash.send_packet(&format!(
        "%xt%zm%bankSwapInv%{room}%{}%{}%{}%{}%",
        inv.id, inv.char_item_id, bank.id, bank.char_item_id
    ))?;
    wait_contains(
        bot,
        Container::Inventory,
        NameOrId::Id(inv.id),
        false,
        14,
        50,
    )?;
    contains(bot, Container::Inventory, NameOrId::Id(bank.id), 1)
}

// ---------------------------------------------------------------------------------------
// InvHelper (VibeSkua `ScriptInventoryHelper`)
// ---------------------------------------------------------------------------------------

/// VibeSkua `InvHelper.Check(item, quantity, moveToInventory)`: temp inventory, inventory,
/// bank (moved to the inventory if asked), then house.
pub fn check(
    bot: &Bot,
    which: NameOrId,
    quantity: i64,
    move_to_inventory: bool,
) -> BotResult<bool> {
    if contains(bot, Container::TempInv, which, quantity)?
        || contains(bot, Container::Inventory, which, quantity)?
    {
        return Ok(true);
    }
    if contains(bot, Container::Bank, which, 1)? {
        if move_to_inventory {
            bank_to_inventory(bot, which)?;
        }
        let from = if move_to_inventory {
            Container::Inventory
        } else {
            Container::Bank
        };
        let have = self::quantity(bot, from, which)?;
        if have >= quantity {
            return Ok(true);
        }
    }
    contains(bot, Container::House, which, 1)
}

// ---------------------------------------------------------------------------------------
// Script API
// ---------------------------------------------------------------------------------------

pub fn dispatch(bot: &Arc<Bot>, module: &str, name: &str, args: Args) -> BotResult<Value> {
    if module == "invHelper" {
        return match name {
            "check" => ok(check(
                bot,
                args.name_or_id(0)?,
                args.i64_or(1, 1),
                args.bool_or(2, true),
            )?),
            _ => Err(BotError::UnknownMethod(format!("{module}.{name}"))),
        };
    }
    let Some(container) = Container::from_module(module) else {
        return Err(BotError::UnknownMethod(format!("{module}.{name}")));
    };
    // Members of every container (VibeSkua `ICheckInventory`, `ILimitedInventory`).
    match name {
        "items" => return ok(items(bot, container)?),
        "slots" => return ok(slots(bot, container)?),
        "usedSlots" => return ok(used_slots(bot, container)?),
        "freeSlots" => return ok(free_slots(bot, container)?),
        "contains" => {
            return ok(contains(
                bot,
                container,
                args.name_or_id(0)?,
                args.i64_or(1, 1),
            )?);
        }
        "containsMany" => {
            let which = name_or_ids(args.value(0));
            let quantity = args.i64_or(1, 1);
            let need_all = args.bool_or(2, true);
            let list = if quantity == 0 {
                Vec::new()
            } else {
                items(bot, container)?
            };
            return ok(contains_many_in(&list, &which, quantity, need_all));
        }
        "getItem" => return ok(get_item(bot, container, args.name_or_id(0)?)?),
        "getQuantity" => return ok(quantity(bot, container, args.name_or_id(0)?)?),
        "isMaxStack" => {
            return ok(is_max_stack_in(
                &items(bot, container)?,
                args.name_or_id(0)?,
            ));
        }
        "isEquipped" => {
            return ok(get_item(bot, container, args.name_or_id(0)?)?.is_some_and(|i| i.equipped));
        }
        _ => {}
    }
    match (container, name) {
        (Container::Inventory, "hasSpaceFor") => ok(has_space_for_any(
            bot,
            args.name_or_id(0)?,
            args.i64_or(1, 1),
        )?),
        (Container::Inventory, "equipItem") => ok(equip_item(bot, args.name_or_id(0)?)?),
        (Container::Inventory, "equipUsableItem") => {
            ok(equip_usable_item(bot, args.name_or_id(0)?)?)
        }
        (Container::Inventory | Container::House, "toBank") => {
            ok(to_bank(bot, container, args.name_or_id(0)?)?)
        }
        (Container::Inventory | Container::House, "ensureToBank") => {
            ok(ensure_to_bank(bot, container, args.name_or_id(0)?)?)
        }
        (Container::Bank, "loaded") => ok(bot.state.lock().bank_loaded),
        (Container::Bank, "setLoaded") => {
            bot.state.lock().bank_loaded = args.bool_or(0, false);
            ok(())
        }
        (Container::Bank, "open") => ok(bank_open(bot)?),
        (Container::Bank, "load") => ok(bank_load(bot, args.bool_or(0, true))?),
        (Container::Bank, "toInventory") => ok(bank_to_inventory(bot, args.name_or_id(0)?)?),
        (Container::Bank, "ensureToInventory") => ok(bank_ensure_to_inventory(
            bot,
            args.name_or_id(0)?,
            args.bool_or(1, true),
        )?),
        (Container::Bank, "swap") => ok(bank_swap(bot, args.name_or_id(0)?, args.name_or_id(1)?)?),
        _ => Err(BotError::UnknownMethod(format!("{module}.{name}"))),
    }
}
