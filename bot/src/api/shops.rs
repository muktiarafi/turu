//! `bot.shops` (VibeSkua `ScriptShop`).

use super::inventory::{self, Container, item_matches};
use super::{Args, NameOrId, map, ok, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::ShopItem;
use serde_json::Value;
use std::sync::Arc;

/// `world.shopinfo` is set.
pub fn is_loaded(bot: &Bot) -> BotResult<bool> {
    Ok(!bot.flash.is_null("world.shopinfo")?)
}

/// `world.shopinfo.ShopID`.
pub fn id(bot: &Bot) -> BotResult<i64> {
    let value: Option<Value> = bot.flash.get_opt("world.shopinfo.ShopID")?;
    Ok(crate::api::packets::num(value.as_ref()).unwrap_or_default())
}

/// `world.shopinfo.sName`.
pub fn name(bot: &Bot) -> BotResult<String> {
    Ok(bot
        .flash
        .get_opt::<String>("world.shopinfo.sName")?
        .unwrap_or_default())
}

/// `world.shopinfo.items` (empty when no shop is loaded).
pub fn items(bot: &Bot) -> BotResult<Vec<ShopItem>> {
    let raw: Option<Vec<Value>> = bot.flash.get_opt("world.shopinfo.items")?;
    Ok(raw
        .unwrap_or_default()
        .into_iter()
        .filter(Value::is_object)
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

/// VibeSkua `Shops.Load(id)`: waits for the `loadShop` cooldown, requests the shop and
/// waits (20 tries) until it's the loaded one.
pub fn load(bot: &Bot, shop_id: i64) -> BotResult<bool> {
    wait::for_action_cooldown(bot, "loadShop", 40)?;
    bot.flash
        .call_game_function("world.sendLoadShopRequest", vec![shop_id.into()])?;
    wait::until(bot, || Ok(is_loaded(bot)? && id(bot)? == shop_id), 20)
}

/// VibeSkua `GetUnmetPurchaseRequirements(item, quantity)`: why the purchase would fail
/// (quest, materials, tokens); empty when it can be bought.
pub fn unmet_purchase_requirements(
    bot: &Bot,
    item: &ShopItem,
    quantity: i64,
) -> BotResult<Vec<String>> {
    let raw = bot.flash.call(
        "getUnmetPurchaseRequirements",
        vec![
            item.item.id.into(),
            item.shop_item_id.into(),
            quantity.into(),
        ],
    )?;
    if raw.trim().is_empty() {
        return Ok(vec!["Could not read purchase requirements.".into()]);
    }
    Ok(serde_json::from_str(raw.trim())
        .unwrap_or_else(|_| vec!["Could not read purchase requirements.".into()]))
}

/// VibeSkua `CanBuy`: a positive quantity (or -1), no unmet requirement and room for it.
fn can_buy(bot: &Bot, item: &ShopItem, quantity: i64) -> BotResult<bool> {
    if quantity != -1 && quantity <= 0 {
        return Ok(false);
    }
    let unmet = unmet_purchase_requirements(bot, item, quantity)?;
    if !unmet.is_empty() {
        bot.logs.debug.push(format!(
            "Can't buy {}: {}",
            item.item.name.trim(),
            unmet.join(" ")
        ));
        return Ok(false);
    }
    let space_for = if quantity == -1 {
        item.item.quantity.max(1)
    } else {
        quantity
    };
    inventory::has_space_for_purchase(bot, item, space_for)
}

/// VibeSkua `BuyItem(name | id, shopItemId, quantity)`: from the loaded shop, through the
/// `buyItemByName` / `buyItemByID` callbacks (`quantity` -1 buys one bundle), then waits
/// (10 tries) for the `buyItem` answer. `shop_item_id <= 0` matches any. True when the
/// purchase was confirmed.
pub fn buy_item(bot: &Bot, which: NameOrId, quantity: i64, shop_item_id: i64) -> BotResult<bool> {
    let shop_item_id = if shop_item_id <= 0 { -1 } else { shop_item_id };
    let item = items(bot)?.into_iter().find(|i| match which {
        NameOrId::Name(_) => item_matches(&i.item, which),
        NameOrId::Id(id) => {
            i.item.id == id && (shop_item_id == -1 || i.shop_item_id == shop_item_id)
        }
    });
    let Some(item) = item else {
        return Ok(false);
    };
    if !can_buy(bot, &item, quantity)? {
        return Ok(false);
    }
    wait::for_action_cooldown(bot, "buyItem", 40)?;
    // Forget an answer nobody waited for, so the wait below sees this purchase's.
    bot.state.lock().signals.item_bought = false;
    match which {
        NameOrId::Name(name) => bot
            .flash
            .call("buyItemByName", vec![name.into(), quantity.into()])?,
        NameOrId::Id(id) => bot.flash.call(
            "buyItemByID",
            vec![id.into(), shop_item_id.into(), quantity.into()],
        )?,
    };
    wait::for_item_buy(bot, 10)
}

/// VibeSkua `SellItem(name | id, quantity)`: sells up to `quantity` (-1 all) of an
/// inventory item, unless it's an AQW Lite favourite, then waits (10 tries) for the
/// `sellItem` answer. True when the sale was confirmed.
pub fn sell_item(bot: &Bot, which: NameOrId, quantity: i64) -> BotResult<bool> {
    let Some(item) = inventory::get_item(bot, Container::Inventory, which)? else {
        return Ok(false);
    };
    // VibeSkua `ProtectFavorites` (on by default).
    if inventory::is_favorited(bot, item.id)? {
        return Ok(false);
    }
    let count = if quantity == -1 {
        item.quantity
    } else {
        quantity.min(item.quantity)
    };
    wait::for_action_cooldown(bot, "sellItem", 40)?;
    bot.state.lock().signals.item_sold = false;
    let room = map::room_id(bot)?;
    bot.flash.send_packet(&format!(
        "%xt%zm%sellItem%{room}%{}%{count}%{}%",
        item.id, item.char_item_id
    ))?;
    wait::for_item_sell(bot, 10)
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "isLoaded" => ok(is_loaded(bot)?),
        "id" => ok(id(bot)?),
        "name" => ok(self::name(bot)?),
        "items" => ok(items(bot)?),
        "load" => {
            load(bot, args.i64(0)?)?;
            ok(())
        }
        "buyItem" => ok(buy_item(
            bot,
            args.name_or_id(0)?,
            args.i64_or(1, -1),
            args.i64_or(2, 0),
        )?),
        "sellItem" => ok(sell_item(bot, args.name_or_id(0)?, args.i64_or(1, 1))?),
        _ => Err(BotError::UnknownMethod(format!("shops.{name}"))),
    }
}
