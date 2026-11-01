//! `bot.drops` (VibeSkua `ScriptDrop`).
//!
//! The drop stack itself (`bot.state.drops`) is filled by the `dropItem` packet handler; this
//! module picks drops up, rejects them and runs the background pickup loop.

use super::inventory::{self, name_or_ids};
use super::{Args, NameOrId, map, ok, player, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::{ItemBase, game_name};
use crate::skills::{Worker, nap};
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// The drop pickup loop (VibeSkua `Drops.Start`) and its pickup list.
#[derive(Default)]
pub struct DropRunner {
    /// Names the loop picks up (VibeSkua `ToPickup`).
    to_pickup: Mutex<Vec<String>>,
    /// IDs the loop picks up (VibeSkua `ToPickupIDs`).
    to_pickup_ids: Mutex<Vec<i64>>,
    /// Reject everything the loop doesn't pick up (VibeSkua `RejectElse`).
    reject_else: AtomicBool,
    /// Milliseconds the loop waits on top of its 1 s tick (VibeSkua `Interval`).
    interval: AtomicU64,
    worker: Worker,
}

impl DropRunner {
    /// VibeSkua `Drops.Enabled`: the pickup loop is running.
    pub fn running(&self) -> bool {
        self.worker.running()
    }

    /// Stops the pickup loop (waits up to 200 ms for it). Called when a script ends.
    pub fn stop(&self) {
        self.worker.stop(Duration::from_millis(200));
    }

    /// The names the loop picks up.
    pub fn to_pickup(&self) -> Vec<String> {
        self.to_pickup.lock().clone()
    }

    /// The IDs the loop picks up.
    pub fn to_pickup_ids(&self) -> Vec<i64> {
        self.to_pickup_ids.lock().clone()
    }

    pub fn reject_else(&self) -> bool {
        self.reject_else.load(Ordering::Relaxed)
    }

    pub fn set_reject_else(&self, value: bool) {
        self.reject_else.store(value, Ordering::Relaxed);
    }

    /// VibeSkua `Drops.Interval`: added to the loop's 1 s tick.
    pub fn interval(&self) -> u64 {
        self.interval.load(Ordering::Relaxed)
    }

    pub fn set_interval(&self, ms: u64) {
        self.interval.store(ms, Ordering::Relaxed);
    }

    /// VibeSkua `Drops.Add(names|ids)`: adds what isn't in the list yet.
    pub fn add(&self, items: &[NameOrId]) {
        let mut names = self.to_pickup.lock();
        let mut ids = self.to_pickup_ids.lock();
        for item in items {
            match *item {
                NameOrId::Name(name) => {
                    let name = game_name(name);
                    if !names.iter().any(|n| *n == name) {
                        names.push(name.into_owned());
                    }
                }
                NameOrId::Id(id) => {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
        }
    }

    /// VibeSkua `Drops.Remove(names|ids)`.
    pub fn remove(&self, items: &[NameOrId]) {
        let mut names = self.to_pickup.lock();
        let mut ids = self.to_pickup_ids.lock();
        for item in items {
            match *item {
                NameOrId::Name(name) => {
                    let name = game_name(name);
                    names.retain(|n| *n != name)
                }
                NameOrId::Id(id) => ids.retain(|i| *i != id),
            }
        }
    }

    /// VibeSkua `Drops.Clear`: empties the pickup list.
    pub fn clear(&self) {
        self.to_pickup.lock().clear();
        self.to_pickup_ids.lock().clear();
    }

    /// VibeSkua `Drops.Start`: every second, while playing, applies the drop options
    /// (`acceptAllDrops`, `acceptACDrops`, `rejectAllDrops`) or picks up the list and, with
    /// `rejectElse`, rejects the rest. Does nothing when it already runs.
    pub fn start(&self, bot: &Arc<Bot>) {
        let weak = Arc::downgrade(bot);
        self.worker
            .start("Drops", move |cancel| drop_loop(&weak, cancel));
    }
}

fn drop_loop(weak: &Weak<Bot>, cancel: &AtomicBool) {
    loop {
        let Some(bot) = weak.upgrade() else { return };
        match nap(&bot, 1000, cancel) {
            Ok(true) => {}
            _ => return,
        }
        match drop_tick(&bot, cancel) {
            Ok(()) => {}
            Err(BotError::Stopped) => return,
            Err(e) => tracing::debug!("Error in drop handler: {e}"),
        }
    }
}

/// One pass of VibeSkua `HandleDrops`.
fn drop_tick(bot: &Bot, cancel: &AtomicBool) -> BotResult<()> {
    if !player::playing(bot)? {
        return Ok(());
    }
    let interval = bot.drops.interval();
    if interval > 0 && !nap(bot, interval, cancel)? {
        return Ok(());
    }
    let (accept_all, accept_ac, reject_all) = {
        let o = bot.opts();
        (o.accept_all_drops, o.accept_ac_drops, o.reject_all_drops)
    };
    if accept_all {
        return pickup_all(bot, false);
    }
    if accept_ac {
        pickup_ac_items(bot)?;
    }
    if reject_all {
        return reject_all_drops(bot);
    }
    let ids = bot.drops.to_pickup_ids();
    if !ids.is_empty() {
        let items: Vec<NameOrId> = ids.iter().map(|&id| NameOrId::Id(id)).collect();
        pickup(bot, &items)?;
    }
    let names = bot.drops.to_pickup();
    if !names.is_empty() {
        let items: Vec<NameOrId> = names.iter().map(|n| NameOrId::Name(n)).collect();
        pickup(bot, &items)?;
    }
    if bot.drops.reject_else() {
        // VibeSkua `BuildRejectElseWhitelist`: the names, plus the names of listed IDs.
        let mut keep = names;
        keep.extend(
            bot.state
                .lock()
                .drops
                .values()
                .filter(|d| ids.contains(&d.id))
                .map(|d| d.name.clone()),
        );
        reject_except_names(bot, keep)?;
    }
    Ok(())
}

/// The drops in the stack, sorted by name.
pub fn current(bot: &Bot) -> Vec<ItemBase> {
    let mut drops: Vec<ItemBase> = bot.state.lock().drops.values().cloned().collect();
    drops.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    drops
}

/// VibeSkua `Drops.Pickup(name|id)`: if the item is in the drop stack and fits (temporary
/// items always fit), sends `getDrop` and removes it from the stack.
pub fn pickup_one(bot: &Bot, item: NameOrId) -> BotResult<()> {
    let Some(drop) = bot.state.lock().find_drop(item).cloned() else {
        return Ok(());
    };
    if !drop.temp && !inventory::has_space_for(bot, &drop, 1)? {
        return Ok(());
    }
    let room = map::room_id(bot)?;
    bot.flash
        .send_packet(&format!("%xt%zm%getDrop%{room}%{}%", drop.id))?;
    bot.state.lock().drops.remove(&drop.id);
    Ok(())
}

// Shared with other modules (kill, hunt). Signatures are fixed.

/// Picks these items up if they're in the drop stack (VibeSkua `Drops.Pickup(params ...)`:
/// each pickup is followed by `Wait.ForPickup`).
pub fn pickup(bot: &Bot, items: &[NameOrId]) -> BotResult<()> {
    for &item in items {
        pickup_one(bot, item)?;
        wait::for_pickup(bot, item, 10)?;
    }
    Ok(())
}

/// VibeSkua `PickupAll(skipWait)`: picks up every drop, then waits (10 tries) for the stack
/// to empty.
pub fn pickup_all(bot: &Bot, skip_wait: bool) -> BotResult<()> {
    for drop in current(bot) {
        pickup_one(bot, NameOrId::Name(&drop.name))?;
    }
    if !skip_wait {
        wait::for_pickup(bot, NameOrId::Name("*"), 10)?;
    }
    Ok(())
}

/// VibeSkua `PickupACItems`: picks up every AC (`bCoins`) drop.
pub fn pickup_ac_items(bot: &Bot) -> BotResult<()> {
    let names: Vec<String> = current(bot)
        .into_iter()
        .filter(|d| d.coins)
        .map(|d| d.name)
        .collect();
    let items: Vec<NameOrId> = names.iter().map(|n| NameOrId::Name(n)).collect();
    pickup(bot, &items)
}

/// VibeSkua `RejectExceptCore`: `rejectExcept(csv of lowercase names)` (AC drops are kept
/// with `acceptACDrops`), then forgets the rejected drops.
fn reject_except_names(bot: &Bot, names: Vec<String>) -> BotResult<()> {
    let mut keep: HashSet<String> = names
        .iter()
        .map(|n| game_name(n.trim()).to_lowercase())
        .collect();
    if bot.opts().accept_ac_drops {
        keep.extend(
            current(bot)
                .iter()
                .filter(|d| d.coins)
                .map(|d| d.name.trim().to_lowercase()),
        );
    }
    let csv = keep.iter().cloned().collect::<Vec<_>>().join(",");
    if !bot.flash.call_bool("rejectExcept", vec![csv.into()])? {
        return Ok(());
    }
    bot.state
        .lock()
        .drops
        .retain(|_, d| keep.contains(&d.name.trim().to_lowercase()));
    Ok(())
}

/// VibeSkua `RejectExcept(names|ids)`: picks up AC drops first with `acceptACDrops`, then
/// rejects every drop that isn't listed.
pub fn reject_except(bot: &Bot, items: &[NameOrId]) -> BotResult<()> {
    if bot.opts().accept_ac_drops {
        pickup_ac_items(bot)?;
    }
    let mut names = Vec::new();
    let drops = current(bot);
    for item in items {
        match *item {
            NameOrId::Name(name) => names.push(name.to_owned()),
            NameOrId::Id(id) => {
                names.extend(drops.iter().filter(|d| d.id == id).map(|d| d.name.clone()))
            }
        }
    }
    reject_except_names(bot, names)
}

/// VibeSkua `RejectAll`.
pub fn reject_all_drops(bot: &Bot) -> BotResult<()> {
    reject_except(bot, &[])
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "current" => ok(current(bot)),
        "toPickup" => ok(bot.drops.to_pickup()),
        "toPickupIds" => ok(bot.drops.to_pickup_ids()),
        "running" => ok(bot.drops.running()),
        "rejectElse" => ok(bot.drops.reject_else()),
        "setRejectElse" => {
            bot.drops.set_reject_else(args.bool_or(0, false));
            ok(())
        }
        "interval" => ok(bot.drops.interval()),
        "setInterval" => {
            bot.drops.set_interval(args.i64_or(0, 0).max(0) as u64);
            ok(())
        }
        "exists" => ok(bot.state.lock().drop_exists(args.name_or_id(0)?)),
        "add" => {
            bot.drops.add(&name_or_ids(args.value(0)));
            ok(())
        }
        "remove" => {
            bot.drops.remove(&name_or_ids(args.value(0)));
            ok(())
        }
        "clear" => {
            bot.drops.clear();
            ok(())
        }
        "pickup" => ok(pickup(bot, &name_or_ids(args.value(0)))?),
        "pickupAll" => ok(pickup_all(bot, false)?),
        "pickupACItems" => ok(pickup_ac_items(bot)?),
        "rejectAll" => ok(reject_all_drops(bot)?),
        "rejectExcept" => ok(reject_except(bot, &name_or_ids(args.value(0)))?),
        "start" => {
            bot.drops.start(bot);
            ok(())
        }
        "stop" => {
            bot.drops.stop();
            ok(())
        }
        _ => Err(BotError::UnknownMethod(format!("drops.{name}"))),
    }
}
