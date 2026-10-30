//! `bot.reputation` (VibeSkua `IScriptFaction`, `ScriptFaction.cs`): your factions and ranks,
//! read from `world.myAvatar.factions`.

use super::{Args, NameOrId, ok};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::Faction;
use serde_json::Value;
use std::sync::Arc;

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "factionList" => ok(faction_list(bot)?),
        "getRank" => ok(rank(bot, args.name_or_id(0)?)?),
        "hasRank" => {
            let rank = args.i64_or(1, 1);
            ok(self::rank(bot, args.name_or_id(0)?)? >= rank)
        }
        _ => Err(BotError::UnknownMethod(format!("reputation.{name}"))),
    }
}

/// Every faction you have at least 1 point of reputation in (VibeSkua `FactionList`).
pub fn faction_list(bot: &Bot) -> BotResult<Vec<Faction>> {
    let mut factions: Vec<Faction> = bot
        .flash
        .get_opt::<Vec<Value>>("world.myAvatar.factions")?
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| serde_json::from_value(f).ok())
        .collect();
    for faction in &mut factions {
        faction.remaining_rep = faction.required_rep - faction.rep;
    }
    Ok(factions)
}

/// Your rank in a faction, by name (exact, as VibeSkua) or ID; 0 without reputation in it.
pub fn rank(bot: &Bot, faction: NameOrId) -> BotResult<i64> {
    Ok(faction_list(bot)?
        .into_iter()
        .find(|f| match faction {
            NameOrId::Name(name) => f.name == name,
            NameOrId::Id(id) => f.id == id,
        })
        .map_or(0, |f| f.rank))
}
