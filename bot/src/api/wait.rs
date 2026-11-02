//! `bot.wait` (VibeSkua `IScriptWait`, `ScriptWait.cs`).
//!
//! A `timeout` counts tries: each try sleeps [`WAIT_SLEEP`] ms (or the given override), so
//! `10` is about a second. `timeout <= 0` waits forever (until Stop).

use super::{Args, NameOrId, ok};
use crate::Bot;
use crate::api::inventory::{self, Container};
use crate::api::{monsters, player, quests};
use crate::error::{BotError, BotResult};
use serde_json::Value;
use std::sync::Arc;

/// VibeSkua `WAIT_SLEEP`.
pub const WAIT_SLEEP: u64 = 100;

/// VibeSkua `ForTrue(predicate, loopFunction, timeout, sleepOverride)`.
///
/// Errors of `predicate` other than Stop count as false (VibeSkua `SafeEvaluate`). Returns
/// false after `timeout` tries; true when the predicate holds or the script is stopping
/// off the script thread (on it, Stop fails with [`BotError::Stopped`]).
pub fn for_true(
    bot: &Bot,
    mut predicate: impl FnMut() -> BotResult<bool>,
    mut loop_fn: Option<&mut dyn FnMut() -> BotResult<()>>,
    timeout: i64,
    sleep_ms: u64,
) -> BotResult<bool> {
    let mut counter = 0;
    loop {
        bot.control.check()?;
        if safe(predicate())? || bot.should_exit() {
            return Ok(true);
        }
        if timeout > 0 && counter >= timeout {
            return Ok(false);
        }
        if let Some(f) = loop_fn.as_mut() {
            swallow(f())?;
        }
        bot.sleep(sleep_ms)?;
        counter += 1;
    }
}

/// [`for_true`] without a loop function, sleeping [`WAIT_SLEEP`].
pub fn until(
    bot: &Bot,
    predicate: impl FnMut() -> BotResult<bool>,
    timeout: i64,
) -> BotResult<bool> {
    for_true(bot, predicate, None, timeout, WAIT_SLEEP)
}

/// A predicate's error is false, except Stop.
pub(crate) fn safe(result: BotResult<bool>) -> BotResult<bool> {
    match result {
        Ok(v) => Ok(v),
        Err(BotError::Stopped) => Err(BotError::Stopped),
        Err(e) => {
            tracing::trace!("wait predicate failed: {e}");
            Ok(false)
        }
    }
}

/// Ignores an error, except Stop (VibeSkua swallows exceptions of handlers and loops).
pub(crate) fn swallow<T>(result: BotResult<T>) -> BotResult<()> {
    match result {
        Ok(_) => Ok(()),
        Err(BotError::Stopped) => Err(BotError::Stopped),
        Err(e) => {
            tracing::debug!("ignored error: {e}");
            Ok(())
        }
    }
}

/// `world.lock.<action>` allows the action now: `now - ts >= cd` (VibeSkua `IsActionAvailable`).
pub fn is_action_available(bot: &Bot, action: &str) -> BotResult<bool> {
    if !player::playing(bot)? {
        return Ok(false);
    }
    #[derive(serde::Deserialize)]
    struct Lock {
        #[serde(default, deserialize_with = "crate::models::lenient_i64")]
        ts: i64,
        #[serde(default, deserialize_with = "crate::models::lenient_i64")]
        cd: i64,
    }
    let Some(lock) = bot.flash.get_opt::<Lock>(&format!("world.lock.{action}"))? else {
        return Ok(false);
    };
    let now = chrono::Utc::now().timestamp_millis();
    Ok(now - lock.ts >= lock.cd)
}

/// VibeSkua `ForActionCooldown(action, 40)`. Lock names: `loadShop`, `equipItem`, `buyItem`,
/// `sellItem`, `getMapItem`, `tryQuestComplete`, `acceptQuest`, `rest`, `tfer`, ...
pub fn for_action_cooldown(bot: &Bot, action: &str, timeout: i64) -> BotResult<bool> {
    until(bot, || is_action_available(bot, action), timeout)
}

/// VibeSkua `ForMapLoad(name, 20)`: the map name matches, then the map finished loading.
pub fn for_map_load(bot: &Bot, name: &str, timeout: i64) -> BotResult<bool> {
    let clean = name.split('-').next().unwrap_or_default().to_lowercase();
    let named = until(
        bot,
        || Ok(super::map::name(bot)?.eq_ignore_ascii_case(&clean)),
        timeout,
    )?;
    Ok(named
        && until(
            bot,
            || Ok(!player::playing(bot)? || super::map::loaded(bot)?),
            timeout,
        )?)
}

/// VibeSkua `ForCellChange(name)`: 25 tries.
pub fn for_cell_change(bot: &Bot, cell: &str) -> BotResult<bool> {
    until(
        bot,
        || Ok(!player::playing(bot)? || player::cell(bot)?.eq_ignore_ascii_case(cell)),
        (WAIT_SLEEP / 4) as i64,
    )
}

pub fn for_combat_exit(bot: &Bot, timeout: i64) -> BotResult<bool> {
    until(
        bot,
        || Ok(!player::playing(bot)? || !player::in_combat(bot)?),
        timeout,
    )
}

/// VibeSkua `ForMonsterDeath(-1)`: until there's no target, untargeting yourself every 20 ms.
pub fn for_monster_death(bot: &Bot, timeout: i64) -> BotResult<bool> {
    let mut untarget = || super::combat::untarget_self(bot);
    for_true(
        bot,
        || Ok(!player::playing(bot)? || !player::has_target(bot)?),
        Some(&mut untarget),
        timeout,
        WAIT_SLEEP / 5,
    )
}

pub fn for_monster_spawn(bot: &Bot, monster: NameOrId, timeout: i64) -> BotResult<bool> {
    until(
        bot,
        || Ok(!player::playing(bot)? || monsters::exists(bot, monster)?),
        timeout,
    )
}

pub fn for_fully_rested(bot: &Bot, timeout: i64) -> BotResult<bool> {
    until(
        bot,
        || {
            Ok(!player::playing(bot)?
                || (player::health(bot)? >= player::max_health(bot)?
                    && player::mana(bot)? >= player::max_mana(bot)?))
        },
        timeout,
    )
}

/// Until the item is in the drop stack.
///
/// The drop stack is checked before `Playing` (VibeSkua checks `Playing` first): it's ours, so
/// a drop that's there costs no call to the game.
pub fn for_drop(bot: &Bot, item: NameOrId, timeout: i64) -> BotResult<bool> {
    until(
        bot,
        || Ok(bot.state.lock().drop_exists(item) || !player::playing(bot)?),
        timeout,
    )
}

/// Until the item left the drop stack.
///
/// The drop stack is checked before `Playing`, as in [`for_drop`]: the pickup loop waits on
/// every listed item, dropped or not, which read `Playing` (three calls to the game) for each.
pub fn for_pickup(bot: &Bot, item: NameOrId, timeout: i64) -> BotResult<bool> {
    until(
        bot,
        || Ok(!bot.state.lock().drop_exists(item) || !player::playing(bot)?),
        timeout,
    )
}

/// VibeSkua `ForItemEquip`: by ID, until it's equipped; by name, until it's equipped or gone.
pub fn for_item_equip(bot: &Bot, item: NameOrId, timeout: i64) -> BotResult<bool> {
    until(
        bot,
        || {
            if !player::playing(bot)? {
                return Ok(true);
            }
            let items = inventory::items(bot, Container::Inventory)?;
            Ok(match item {
                NameOrId::Id(id) => items.iter().any(|i| i.id == id && i.equipped),
                NameOrId::Name(name) => items
                    .iter()
                    .find(|i| i.name_is(name))
                    .is_none_or(|i| i.equipped),
            })
        },
        timeout,
    )
}

/// Waits for a one-shot signal set by a packet (VibeSkua `AutoResetEvent.WaitOne`), polling
/// every 20 ms so Stop works.
fn for_signal(
    bot: &Bot,
    timeout: i64,
    take: impl Fn(&mut crate::state::Signals) -> &mut bool,
) -> BotResult<bool> {
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_millis(timeout.max(0) as u64 * WAIT_SLEEP);
    loop {
        {
            let mut state = bot.state.lock();
            let flag = take(&mut state.signals);
            if *flag {
                *flag = false;
                return Ok(true);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        bot.sleep(20)?;
    }
}

/// Until a `buyItem` packet with `bitSuccess` arrives.
pub fn for_item_buy(bot: &Bot, timeout: i64) -> BotResult<bool> {
    for_signal(bot, timeout, |s| &mut s.item_bought)
}

/// Until a `sellItem` packet arrives.
pub fn for_item_sell(bot: &Bot, timeout: i64) -> BotResult<bool> {
    for_signal(bot, timeout, |s| &mut s.item_sold)
}

/// Until a `loadBank` packet arrives.
pub fn for_bank_load(bot: &Bot, timeout: i64) -> BotResult<bool> {
    for_signal(bot, timeout, |s| &mut s.bank_loaded)
}

pub fn for_quest_accept(bot: &Bot, id: i64, timeout: i64) -> BotResult<bool> {
    for_true(
        bot,
        || Ok(!player::playing(bot)? || quests::is_in_progress(bot, id)?),
        None,
        timeout,
        WAIT_SLEEP / 2,
    )
}

pub fn for_quest_complete(bot: &Bot, id: i64, timeout: i64) -> BotResult<bool> {
    for_true(
        bot,
        || Ok(!player::playing(bot)? || !quests::is_in_progress(bot, id)?),
        None,
        timeout,
        WAIT_SLEEP / 2,
    )
}

/// VibeSkua `ForPlayerPosition(x, y, 10)`.
pub fn for_player_position(bot: &Bot, x: i64, y: i64, timeout: i64) -> BotResult<bool> {
    until(
        bot,
        || {
            Ok(!player::playing(bot)?
                || (player::x(bot)?.round() as i64 == x && player::y(bot)?.round() as i64 == y))
        },
        timeout,
    )
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "forActionCooldown" => ok(for_action_cooldown(bot, args.str(0)?, args.i64_or(1, 40))?),
        "forMapLoad" => ok(for_map_load(bot, args.str(0)?, args.i64_or(1, 20))?),
        "forCellChange" => ok(for_cell_change(bot, args.str(0)?)?),
        "forCombatExit" => ok(for_combat_exit(bot, args.i64_or(0, 10))?),
        "forMonsterSpawn" => ok(for_monster_spawn(
            bot,
            args.name_or_id(0)?,
            args.i64_or(1, 10),
        )?),
        "forMonsterDeath" => ok(for_monster_death(bot, args.i64_or(0, -1))?),
        "forDrop" => ok(for_drop(bot, args.name_or_id(0)?, args.i64_or(1, 10))?),
        "forPickup" => ok(for_pickup(bot, args.name_or_id(0)?, args.i64_or(1, 10))?),
        "forItemEquip" => ok(for_item_equip(
            bot,
            args.name_or_id(0)?,
            args.i64_or(1, 10),
        )?),
        "forItemBuy" => ok(for_item_buy(bot, args.i64_or(0, 10))?),
        "forItemSell" => ok(for_item_sell(bot, args.i64_or(0, 10))?),
        "forQuestAccept" => ok(for_quest_accept(bot, args.i64(0)?, args.i64_or(1, 14))?),
        "forQuestComplete" => ok(for_quest_complete(bot, args.i64(0)?, args.i64_or(1, 10))?),
        "forBankLoad" => ok(for_bank_load(bot, args.i64_or(0, 20))?),
        "forFullyRested" => ok(for_fully_rested(bot, args.i64_or(0, -1))?),
        _ => Err(BotError::UnknownMethod(format!("wait.{name}"))),
    }
}
