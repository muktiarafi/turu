//! `bot.combat` (VibeSkua `IScriptCombat`, `ScriptCombat.cs`).
//!
//! TODO(turu): VibeSkua's `EnableCounterHandler`/`StopAttacking` (stop attacking while a
//! counter attack is up) isn't exposed in turu.ts yet; the `counterAttack` event is emitted.

use super::{Args, NameOrId, map, ok, player, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use serde_json::Value;
use std::sync::Arc;

/// Targets and approaches a monster in your cell: `attackMonsterName` (case-insensitive
/// substring, `*` for any; alive and lowest HP first) or `attackMonsterID` (map or
/// monster ID). False when there's no such monster.
pub fn attack(bot: &Bot, monster: NameOrId) -> BotResult<bool> {
    match monster {
        NameOrId::Name(name) => bot.flash.call_bool("attackMonsterName", vec![name.into()]),
        NameOrId::Id(id) => bot.flash.call_bool("attackMonsterID", vec![id.into()]),
    }
}

/// `attackPlayer(name)`: targets and approaches a player in your cell.
pub fn attack_player(bot: &Bot, name: &str) -> BotResult<bool> {
    bot.flash.call_bool("attackPlayer", vec![name.into()])
}

pub fn approach_target(bot: &Bot) -> BotResult<()> {
    bot.flash.call_game_function("world.approachTarget", vec![])
}

pub fn cancel_target(bot: &Bot) -> BotResult<()> {
    bot.flash.call_game_function("world.cancelTarget", vec![])
}

pub fn cancel_auto_attack(bot: &Bot) -> BotResult<()> {
    bot.flash
        .call_game_function("world.cancelAutoAttack", vec![])
}

/// `untargetSelf`: drops the target if it's yourself.
pub fn untarget_self(bot: &Bot) -> BotResult<()> {
    bot.flash.call("untargetSelf", vec![])?;
    Ok(())
}

/// VibeSkua `Exit`: unless idle, cancels the attack and target, jumps in place twice and
/// waits for the combat to end.
pub fn exit(bot: &Bot) -> BotResult<()> {
    if player::state(bot)? == 1 {
        return Ok(());
    }
    cancel_auto_attack(bot)?;
    cancel_target(bot)?;
    let (cell, pad) = (player::cell(bot)?, player::pad(bot)?);
    map::jump(bot, &cell, &pad, true, false)?;
    bot.sleep(300)?;
    map::jump(bot, &cell, &pad, true, false)?;
    wait::for_combat_exit(bot, 10)?;
    Ok(())
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "attack" => ok(attack(bot, args.name_or_id(0)?)?),
        "attackPlayer" => ok(attack_player(bot, args.str(0)?)?),
        "approachTarget" => ok(approach_target(bot)?),
        "cancelTarget" => ok(cancel_target(bot)?),
        "cancelAutoAttack" => ok(cancel_auto_attack(bot)?),
        "untargetSelf" => ok(untarget_self(bot)?),
        "exit" => ok(exit(bot)?),
        _ => Err(BotError::UnknownMethod(format!("combat.{name}"))),
    }
}
