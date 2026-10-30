//! `bot.combat` (VibeSkua `IScriptCombat`, `ScriptCombat.cs`) and its counter-attack pause
//! (`EnableCounterHandler`, `StopAttacking`).

use super::{Args, NameOrId, map, ok, player, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::events::Work;
use parking_lot::Mutex;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// VibeSkua's counter-attack flags (`ScriptCombat`, `ScriptSkill._counterAttack`,
/// `UltraBossHelper`). Packet handlers change them, so they're atomics, never a Flash call.
#[derive(Default)]
pub struct CombatState {
    /// VibeSkua `Combat.EnableCounterHandler`: on a counter attack, stop attacking and
    /// attack the same target again once it faded.
    pub(crate) counter_handler: AtomicBool,
    /// VibeSkua `Combat.StopAttacking`: `attack` cancels the target instead, and the skill
    /// timer waits (up to 10 s) for the counter attack to fade.
    pub(crate) stop_attacking: AtomicBool,
    /// VibeSkua `UltraBossHelper.IsCounterAttackEnabled`.
    pub(crate) helper_enabled: AtomicBool,
    /// VibeSkua `UltraBossHelper.IsCounterAttackActive` (C# never sets it to true).
    pub(crate) helper_active: AtomicBool,
    /// The target's map ID saved when a counter attack began (VibeSkua `ScriptCombat._target`).
    target: Mutex<Option<i64>>,
    /// VibeSkua `ScriptSkill._counterAttack` (an `AutoResetEvent`): set when one faded,
    /// consumed by the skill timer's wait.
    faded: AtomicBool,
}

impl CombatState {
    pub fn stop_attacking(&self) -> bool {
        self.stop_attacking.load(Ordering::Relaxed)
    }

    pub fn set_stop_attacking(&self, stop: bool) {
        self.stop_attacking.store(stop, Ordering::Relaxed);
    }

    /// Takes the "counter attack faded" signal (true once per fade).
    pub(crate) fn take_faded(&self) -> bool {
        self.faded.swap(false, Ordering::AcqRel)
    }
}

/// VibeSkua `ScriptCombat.CounterAttack` and `ScriptSkill.CounterAttack`, from a `ct` packet.
/// Runs on the player's thread: the Flash part (saving the target, cancelling it, attacking
/// it again) is posted to the bot's work thread.
pub(crate) fn on_counter_attack(bot: &Bot, faded: bool) {
    let state = &bot.combat;
    if faded {
        state.faded.store(true, Ordering::Release);
    }
    if !state.counter_handler.load(Ordering::Relaxed) {
        return;
    }
    state.set_stop_attacking(!faded);
    bot.events.post(Work::CounterAttack { faded });
}

/// The work thread's half of [`on_counter_attack`].
pub(crate) fn counter_attack_work(bot: &Bot, faded: bool) -> BotResult<()> {
    if faded {
        if let Some(id) = bot.combat.target.lock().take() {
            attack(bot, NameOrId::Id(id))?;
        }
        return Ok(());
    }
    *bot.combat.target.lock() = player::target(bot)?.map(|m| m.map_id);
    cancel_auto_attack(bot)?;
    cancel_target(bot)
}

/// VibeSkua `UltraBossHelper.EnableCounterAttack` / `DisableCounterAttack`.
pub fn set_counter_attack(bot: &Bot, enabled: bool) {
    let state = &bot.combat;
    if state.helper_enabled.swap(enabled, Ordering::Relaxed) == enabled {
        return;
    }
    if !enabled {
        state.helper_active.store(false, Ordering::Relaxed);
    }
    state.counter_handler.store(enabled, Ordering::Relaxed);
}

/// VibeSkua `UltraBossHelper.SetAttacksStopped`.
pub fn set_attacks_stopped(bot: &Bot, stop: bool) {
    bot.combat.set_stop_attacking(stop);
    if !stop {
        bot.combat.helper_active.store(false, Ordering::Relaxed);
    }
}

/// Targets and approaches a monster in your cell: `attackMonsterName` (case-insensitive
/// substring, `*` for any; alive and lowest HP first) or `attackMonsterID` (map or
/// monster ID). False when there's no such monster, or while attacks are stopped (then it
/// cancels the target, as VibeSkua does).
pub fn attack(bot: &Bot, monster: NameOrId) -> BotResult<bool> {
    if bot.combat.stop_attacking() {
        cancel_target(bot)?;
        return Ok(false);
    }
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
        "stopAttacking" => ok(bot.combat.stop_attacking()),
        "setStopAttacking" => {
            bot.combat.set_stop_attacking(args.bool_or(0, false));
            ok(())
        }
        "enableCounterHandler" => ok(bot.combat.counter_handler.load(Ordering::Relaxed)),
        "setEnableCounterHandler" => {
            bot.combat
                .counter_handler
                .store(args.bool_or(0, false), Ordering::Relaxed);
            ok(())
        }
        "enableCounterAttack" => {
            set_counter_attack(bot, true);
            ok(())
        }
        "disableCounterAttack" => {
            set_counter_attack(bot, false);
            ok(())
        }
        "isCounterAttackEnabled" => ok(bot.combat.helper_enabled.load(Ordering::Relaxed)),
        "isCounterAttackActive" => ok(bot.combat.helper_active.load(Ordering::Relaxed)),
        "setAttacksStopped" => {
            set_attacks_stopped(bot, args.bool_or(0, false));
            ok(())
        }
        _ => Err(BotError::UnknownMethod(format!("combat.{name}"))),
    }
}
