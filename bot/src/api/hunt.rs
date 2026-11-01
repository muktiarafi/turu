//! `bot.hunt` (VibeSkua `IScriptHunt`, `ScriptHunt.cs`): like `kill`, but jumps between
//! the cells where the monster lives, at most once per `HuntDelay`.

use super::inventory::{self, Container};
use super::kill::{self, ForItemToken};
use super::{Args, NameOrId, combat, map, monsters, ok, player};
use crate::Bot;
use crate::error::{BotError, BotResult};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Sleeps until `HuntDelay` passed since the last hunt jump.
fn throttle(bot: &Bot) -> BotResult<()> {
    let last = bot.state.lock().last_hunt_jump;
    let delay = Duration::from_millis(bot.opts().hunt_delay.max(0) as u64);
    if let Some(last) = last {
        let elapsed = last.elapsed();
        if elapsed < delay {
            bot.sleep((delay - elapsed).as_millis() as u64)?;
        }
    }
    Ok(())
}

/// Jumps to `cell` (pad Left) once `HuntDelay` passed since the last hunt jump.
pub(crate) fn hunt_jump(bot: &Bot, cell: &str) -> BotResult<()> {
    throttle(bot)?;
    map::jump(bot, cell, "Left", true, false)?;
    bot.state.lock().last_hunt_jump = Some(Instant::now());
    Ok(())
}

fn action_delay(bot: &Bot) -> u64 {
    bot.opts().action_delay.max(0) as u64
}

fn union(cells: &mut Vec<String>, more: Vec<String>) {
    for cell in more {
        if !cells.contains(&cell) {
            cells.push(cell);
        }
    }
}

/// VibeSkua `Hunt.Monster(name | id)`: jumps through the cells with a living monster
/// ("A|B" for several names) until one can be attacked, then kills it.
pub fn monster(bot: &Arc<Bot>, monster: NameOrId) -> BotResult<()> {
    monster_with(bot, monster, None)
}

/// [`monster`] that gives up when `token` is cancelled.
pub fn monster_with(
    bot: &Arc<Bot>,
    monster: NameOrId,
    token: Option<&ForItemToken>,
) -> BotResult<()> {
    let cancelled = || token.map_or(Ok(false), |t| t.poll());
    let delay = action_delay(bot);
    match monster {
        NameOrId::Name(name) => {
            let names: Vec<&str> = name.split('|').collect();
            while !cancelled()? && !bot.should_exit() {
                let all = monsters::map_monsters(bot)?;
                let mut cells = Vec::new();
                for n in &names {
                    union(
                        &mut cells,
                        monsters::living_cells_in(&all, NameOrId::Name(n)),
                    );
                }
                if cells.is_empty() {
                    for n in &names {
                        union(
                            &mut cells,
                            monsters::living_data_leaf_cells_in(&all, NameOrId::Name(n)),
                        );
                    }
                }
                if cells.is_empty() {
                    // Turu: VibeSkua spins here without sleeping.
                    bot.sleep(200)?;
                    continue;
                }
                for cell in &cells {
                    if cancelled()? {
                        break;
                    }
                    let here = player::cell(bot)?;
                    if (!cells.contains(&here) || *cell != here) && !cancelled()? {
                        hunt_jump(bot, cell)?;
                    }
                    for mon in &names {
                        if cancelled()? {
                            break;
                        }
                        if monsters::exists(bot, NameOrId::Name(mon))? && !cancelled()? {
                            if !combat::attack(bot, NameOrId::Name(mon))? {
                                continue;
                            }
                            bot.sleep(delay)?;
                            kill::monster_with(bot, NameOrId::Name(mon), token)?;
                            return Ok(());
                        }
                    }
                    bot.sleep(200)?;
                }
            }
        }
        NameOrId::Id(id) => {
            while !cancelled()? && !bot.should_exit() {
                let all = monsters::map_monsters(bot)?;
                let mut cells = monsters::living_cells_in(&all, monster);
                if cells.is_empty() {
                    cells = monsters::living_data_leaf_cells_in(&all, monster);
                }
                if cells.is_empty() {
                    bot.sleep(200)?;
                    continue;
                }
                for cell in &cells {
                    if cancelled()? {
                        break;
                    }
                    if !cells.contains(&player::cell(bot)?) && !cancelled()? {
                        hunt_jump(bot, cell)?;
                    }
                    if cancelled()? {
                        break;
                    }
                    if monsters::exists(bot, NameOrId::Id(id))? && !cancelled()? {
                        if !combat::attack(bot, NameOrId::Id(id))? {
                            continue;
                        }
                        bot.sleep(delay)?;
                        kill::monster_with(bot, NameOrId::Id(id), token)?;
                        return Ok(());
                    }
                    bot.sleep(200)?;
                }
            }
        }
    }
    Ok(())
}

/// VibeSkua `Hunt.ForItem`: hunts `monster` ("A|B" or "*") through the map until you have
/// `quantity` of `item`, picking it up from the drops.
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
    let token = ForItemToken::new(bot, item, quantity, temp, None);
    let names: Vec<&str> = monster.split('|').collect();
    let delay = action_delay(bot);
    let data_leaf_cells = |bot: &Bot| -> BotResult<Vec<String>> {
        let all = monsters::map_monsters(bot)?;
        let mut cells = Vec::new();
        for n in &names {
            union(
                &mut cells,
                monsters::living_data_leaf_cells_in(&all, NameOrId::Name(n)),
            );
        }
        Ok(cells)
    };
    let mut cells = data_leaf_cells(bot)?;
    while !token.poll()? && !bot.should_exit() {
        if cells.is_empty() {
            // Turu: VibeSkua keeps looping over an empty list here; look again instead.
            bot.sleep(200)?;
            token.check_quantity()?;
            cells = data_leaf_cells(bot)?;
            continue;
        }
        let mut i = cells.len();
        while i > 0 {
            i -= 1;
            if i >= cells.len() {
                continue;
            }
            if player::cell(bot)? != cells[i] && !token.poll()? {
                hunt_jump(bot, &cells[i].clone())?;
            }
            if token.poll()? {
                break;
            }
            // VibeSkua `cells.RemoveAt(i)` for each name that isn't here (or can't be
            // attacked); done once here, so several names can't remove other cells.
            let mut remove = false;
            for mon in &names {
                if token.poll()? {
                    break;
                }
                if monsters::exists(bot, NameOrId::Name(mon))? && !token.poll()? {
                    if !combat::attack(bot, NameOrId::Name(mon))? {
                        remove = true;
                        continue;
                    }
                    bot.sleep(delay)?;
                    kill::monster_with(bot, NameOrId::Name(mon), Some(&token))?;
                    break;
                } else {
                    remove = true;
                }
            }
            if remove && i < cells.len() {
                cells.remove(i);
            }
        }
        // Turu: also look at the inventory, in case the drop's packets were missed.
        token.check_quantity()?;
    }
    Ok(())
}

/// VibeSkua `Hunt.ForItems`: [`for_item`] for each item in turn.
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
        _ => Err(BotError::UnknownMethod(format!("hunt.{name}"))),
    }
}
