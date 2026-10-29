//! `bot.kill` (VibeSkua `IScriptKill`, `ScriptKill.cs`).

use super::inventory::{self, Container};
use super::wait::{self, WAIT_SLEEP, swallow};
use super::{Args, NameOrId, combat, drops, map, monsters, ok, player};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::events::BotEvent;
use parking_lot::Mutex;
use serde_json::Value;
use std::cell::Cell;
use std::sync::Arc;

/// A running `Kill.ForItem` / `Hunt.ForItem`: it is cancelled once you have the item
/// (VibeSkua's `_ctsKill` / `_ctsHunt` and their message handlers).
///
/// Event handlers only queue the events; [`ForItemToken::poll`] handles them on the
/// script's thread (picking the drop up, jumping back to the saved cell), because handlers
/// run on the player's thread and must not call Flash.
pub struct ForItemToken<'a> {
    bot: &'a Arc<Bot>,
    item: String,
    quantity: i64,
    temp: bool,
    /// `Kill.ForItem` jumps back here when you're moved (VibeSkua `_saveCell`/`_savePad`).
    save: Option<(String, String)>,
    queue: Arc<Mutex<Vec<BotEvent>>>,
    subscription: u64,
    cancelled: Cell<bool>,
}

impl<'a> ForItemToken<'a> {
    pub fn new(
        bot: &'a Arc<Bot>,
        item: &str,
        quantity: i64,
        temp: bool,
        save: Option<(String, String)>,
    ) -> Self {
        let queue: Arc<Mutex<Vec<BotEvent>>> = Arc::default();
        let watch_cells = save.is_some();
        let subscription = {
            let queue = queue.clone();
            bot.events.subscribe(move |event| {
                let wanted = match event {
                    BotEvent::ItemDropped { .. } | BotEvent::ItemAdded { .. } => true,
                    BotEvent::CellChanged { .. } => watch_cells,
                    _ => false,
                };
                if wanted {
                    queue.lock().push(event.clone());
                }
            })
        };
        Self {
            bot,
            item: item.to_owned(),
            quantity,
            temp,
            save,
            queue,
            subscription,
            cancelled: Cell::new(false),
        }
    }

    fn container(&self) -> Container {
        if self.temp {
            Container::TempInv
        } else {
            Container::Inventory
        }
    }

    pub fn cancel(&self) {
        self.cancelled.set(true);
    }

    /// Handles the queued events; true once cancelled.
    pub fn poll(&self) -> BotResult<bool> {
        if self.cancelled.get() {
            return Ok(true);
        }
        let events = std::mem::take(&mut *self.queue.lock());
        for event in events {
            match event {
                BotEvent::CellChanged { cell, .. } => {
                    if let Some((save_cell, save_pad)) = &self.save
                        && !cell.eq_ignore_ascii_case(save_cell)
                    {
                        swallow(map::jump(self.bot, save_cell, save_pad, true, false))?;
                    }
                }
                BotEvent::ItemDropped { item } => {
                    if !item.name_is(&self.item) {
                        continue;
                    }
                    swallow(drops::pickup(self.bot, &[NameOrId::Name(&item.name)]))?;
                    self.check_quantity()?;
                }
                BotEvent::ItemAdded { item, quantity_now } => {
                    if !item.name_is(&self.item) {
                        continue;
                    }
                    if !item.temp && quantity_now >= self.quantity {
                        self.cancel();
                    } else {
                        self.check_quantity()?;
                    }
                }
                _ => {}
            }
            if self.cancelled.get() {
                break;
            }
        }
        Ok(self.cancelled.get())
    }

    /// Cancels when the container has the quantity (VibeSkua `GetQuantity(...) >= quantity`).
    pub fn check_quantity(&self) -> BotResult<bool> {
        let have = match inventory::quantity(self.bot, self.container(), NameOrId::Name(&self.item))
        {
            Ok(q) => q,
            Err(BotError::Stopped) => return Err(BotError::Stopped),
            Err(e) => {
                tracing::debug!("ForItem quantity check failed: {e}");
                return Ok(false);
            }
        };
        if have >= self.quantity {
            self.cancel();
        }
        Ok(self.cancelled.get())
    }
}

impl Drop for ForItemToken<'_> {
    fn drop(&mut self) {
        self.bot.events.unsubscribe(self.subscription);
    }
}

fn action_delay(bot: &Bot) -> u64 {
    bot.opts().action_delay.max(0) as u64
}

/// VibeSkua `Kill.Monster(name | id)`: waits for it to spawn (10 tries), attacks it, sleeps
/// `ActionDelay` and waits until it's dead.
pub fn monster(bot: &Arc<Bot>, monster: NameOrId) -> BotResult<()> {
    monster_with(bot, monster, None)
}

/// [`monster`] that also stops when `token` is cancelled; then it cancels the attack and
/// the target (VibeSkua `WaitMonsterDeathOrCancellation`).
pub fn monster_with(
    bot: &Arc<Bot>,
    monster: NameOrId,
    token: Option<&ForItemToken>,
) -> BotResult<()> {
    wait::for_monster_spawn(bot, monster, 10)?;
    if !combat::attack(bot, monster)? {
        return Ok(());
    }
    bot.sleep(action_delay(bot))?;
    match token {
        None => {
            wait::for_monster_death(bot, -1)?;
        }
        Some(token) => {
            wait::until(
                bot,
                || Ok(token.poll()? || !player::playing(bot)? || !player::has_target(bot)?),
                -1,
            )?;
            combat::cancel_auto_attack(bot)?;
            combat::cancel_target(bot)?;
        }
    }
    Ok(())
}

/// VibeSkua `Kill.Player(name)`.
pub fn player(bot: &Arc<Bot>, name: &str) -> BotResult<()> {
    combat::attack_player(bot, name)?;
    wait::for_monster_death(bot, -1)?;
    Ok(())
}

/// VibeSkua `Kill.ForItem`: kills `monster` ("A|B" or "*") in your cell until you have
/// `quantity` of `item` (in the temp inventory with `temp`), picking it up from the drops
/// and jumping back to this cell when something moves you.
pub fn for_item(
    bot: &Arc<Bot>,
    monster: &str,
    item: &str,
    quantity: i64,
    temp: bool,
) -> BotResult<()> {
    let container = if temp {
        Container::TempInv
    } else {
        Container::Inventory
    };
    if inventory::contains(bot, container, NameOrId::Name(item), quantity)? {
        return Ok(());
    }
    let save = (player::cell(bot)?, player::pad(bot)?);
    let token = ForItemToken::new(bot, item, quantity, temp, Some(save));
    let names: Vec<&str> = monster.split('|').collect();
    let delay = action_delay(bot);
    while !token.poll()? && !bot.should_exit() {
        if monster.trim() == "*" {
            let available = monsters::current_available(bot)?;
            if available.is_empty() {
                // Turu: VibeSkua spins here without sleeping.
                bot.sleep(WAIT_SLEEP)?;
            }
            for mon in available {
                if token.poll()? {
                    break;
                }
                monster_with(bot, NameOrId::Id(mon.map_id), Some(&token))?;
                bot.sleep(delay)?;
            }
        } else {
            for name in &names {
                if token.poll()? {
                    break;
                }
                monster_with(bot, NameOrId::Name(name), Some(&token))?;
                bot.sleep(delay)?;
            }
        }
        // Turu: also look at the inventory, in case the drop's packets were missed.
        token.check_quantity()?;
    }
    Ok(())
}

/// VibeSkua `Kill.ForItems`: [`for_item`] for each item in turn.
pub fn for_items(
    bot: &Arc<Bot>,
    monster: &str,
    items: &[String],
    quantities: &[i64],
    temp: bool,
) -> BotResult<()> {
    if items.len() != quantities.len() {
        bot.log("Item count does not match quantity/temp item count.");
        return Ok(());
    }
    for (item, quantity) in items.iter().zip(quantities) {
        for_item(bot, monster, item, *quantity, temp)?;
    }
    Ok(())
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "monster" => ok(monster(bot, args.name_or_id(0)?)?),
        "player" => ok(player(bot, args.str(0)?)?),
        "forItem" => ok(for_item(
            bot,
            args.str(0)?,
            args.str(1)?,
            args.i64(2)?,
            args.bool_or(3, false),
        )?),
        "forItems" => ok(for_items(
            bot,
            args.str(0)?,
            &args.strings(1),
            &args.i64s(2),
            args.bool_or(3, false),
        )?),
        _ => Err(BotError::UnknownMethod(format!("kill.{name}"))),
    }
}
