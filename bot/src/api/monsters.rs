//! `bot.monsters` (VibeSkua `IScriptMonster`, `ScriptMonster.cs`).

use super::{Args, NameOrId, ok, player};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::Monster;
use serde_json::Value;
use std::sync::Arc;

fn parse(raw: &str) -> Vec<Monster> {
    serde_json::from_str::<Option<Vec<Monster>>>(raw.trim())
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// `getMonsters`: every monster of the map.
pub fn map_monsters(bot: &Bot) -> BotResult<Vec<Monster>> {
    Ok(parse(&bot.flash.call("getMonsters", vec![])?))
}

/// `availableMonsters`: monsters in your cell that can be attacked (they have a `pMC`).
pub fn current_available(bot: &Bot) -> BotResult<Vec<Monster>> {
    Ok(parse(&bot.flash.call("availableMonsters", vec![])?))
}

/// Map monsters in your cell.
pub fn current_monsters(bot: &Bot) -> BotResult<Vec<Monster>> {
    let cell = player::cell(bot)?;
    Ok(map_monsters(bot)?
        .into_iter()
        .filter(|m| m.cell == cell)
        .collect())
}

/// VibeSkua `Exists`: an attackable monster in your cell by name (`*` for any) or by
/// monster/map ID.
pub fn exists(bot: &Bot, monster: NameOrId) -> BotResult<bool> {
    let available = current_available(bot)?;
    Ok(available.iter().any(|m| matches(m, monster, true)))
}

/// Name (`*` for any, case-insensitive) or `MonID`; with `map_id`, `MonMapID` too.
pub fn matches(m: &Monster, monster: NameOrId, map_id: bool) -> bool {
    match monster {
        NameOrId::Name(name) => m.name_matches(name),
        NameOrId::Id(id) => m.id == id || (map_id && m.map_id == id),
    }
}

fn distinct_cells<'a>(monsters: impl Iterator<Item = &'a Monster>) -> Vec<String> {
    let mut cells: Vec<String> = Vec::new();
    for m in monsters {
        if !cells.contains(&m.cell) {
            cells.push(m.cell.clone());
        }
    }
    cells
}

/// Cells with a living monster of this name or `MonID`.
pub fn living_cells_in(monsters: &[Monster], monster: NameOrId) -> Vec<String> {
    distinct_cells(
        monsters
            .iter()
            .filter(|m| m.alive() && matches(m, monster, false)),
    )
}

/// VibeSkua `GetLivingMonsterDataLeafCells`: like [`living_cells_in`], but a name is first
/// resolved to its `MonID` (so every monster of that type counts).
pub fn living_data_leaf_cells_in(monsters: &[Monster], monster: NameOrId) -> Vec<String> {
    match monster {
        NameOrId::Name(name) if name.trim() == "*" => {
            distinct_cells(monsters.iter().filter(|m| m.alive()))
        }
        NameOrId::Name(name) => match monsters.iter().find(|m| m.name_matches(name)) {
            Some(found) => living_cells_in(monsters, NameOrId::Id(found.id)),
            None => Vec::new(),
        },
        NameOrId::Id(_) => living_cells_in(monsters, monster),
    }
}

pub fn living_monster_cells(bot: &Bot, monster: NameOrId) -> BotResult<Vec<String>> {
    Ok(living_cells_in(&map_monsters(bot)?, monster))
}

pub fn living_monster_data_leaf_cells(bot: &Bot, monster: NameOrId) -> BotResult<Vec<String>> {
    Ok(living_data_leaf_cells_in(&map_monsters(bot)?, monster))
}

pub fn monsters_by_cell(bot: &Bot, cell: &str) -> BotResult<Vec<Monster>> {
    Ok(map_monsters(bot)?
        .into_iter()
        .filter(|m| m.cell == cell)
        .collect())
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "mapMonsters" => ok(map_monsters(bot)?),
        "currentMonsters" => ok(current_monsters(bot)?),
        "currentAvailableMonsters" => ok(current_available(bot)?),
        "exists" => ok(exists(bot, args.name_or_id(0)?)?),
        "getLivingMonsterCells" => ok(living_monster_cells(bot, args.name_or_id(0)?)?),
        "getMonstersByCell" => ok(monsters_by_cell(bot, args.str(0)?)?),
        _ => Err(BotError::UnknownMethod(format!("monsters.{name}"))),
    }
}
