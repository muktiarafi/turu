//! The "Auto" popup's Auto Attack: a stoppable loop on a bot thread, ported from VibeSkua's
//! `ScriptAuto` (`_DoActionAuto` + `_Attack`).

use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use turu_bot::api::{NameOrId, combat, dispatch, inventory, kill, monsters, player};
use turu_bot::control::mark_script_thread;
use turu_bot::{Bot, BotError, BotResult};

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn running() -> bool {
    RUNNING.load(Ordering::Acquire)
}

/// Starts Auto Attack with `class`'s skill set in `mode` (VibeSkua passes both only when both
/// were picked; otherwise the equipped class in "Base"). `map_ids`, when given, are attacked
/// in that priority order.
pub fn start(class: Option<String>, mode: String, map_ids: Vec<i64>) {
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    crate::turu::spawn_action("Auto Attack", move |bot| {
        // Like the script thread: Stop (`request_stop`) makes the next bot call fail.
        mark_script_thread(true);
        bot.control.reset();
        let result = run(bot, class, &mode, &map_ids);
        mark_script_thread(false);
        if !bot.scripts.is_running() {
            bot.control.reset();
        }
        let _ = dispatch(bot, "skills.stop", &[]);
        bot.log("[Auto Attack] Stopped");
        RUNNING.store(false, Ordering::Release);
        match result {
            Err(BotError::Stopped) => Ok(()),
            other => other,
        }
    });
}

/// Stops Auto Attack (no-op when it isn't running).
pub fn stop() {
    if running() {
        crate::turu::bot().control.request_stop();
    }
}

/// "1, 2 3;4" -> [1, 2, 3, 4] (VibeSkua `ParseManualMapIDs`).
pub fn parse_map_ids(text: &str) -> Vec<i64> {
    text.split([',', ' ', ';'])
        .filter_map(|part| part.trim().parse().ok())
        .collect()
}

/// VibeSkua `_DoActionAuto(hunt: false)` then `_Attack`.
fn run(bot: &Arc<Bot>, class: Option<String>, mode: &str, map_ids: &[i64]) -> BotResult<()> {
    if !player::logged_in(bot)? {
        bot.log("[Auto Attack] Log in first");
        return Ok(());
    }
    bot.options.write().infinite_range = true;
    let class = match class {
        Some(class) => class,
        None => inventory::current_class(bot)?.unwrap_or_else(|| "Generic".into()),
    };
    dispatch(bot, "skills.startAdvanced", &[json!(class), json!(true), json!(mode)])?;
    player::set_spawn_point(bot, None, None)?;

    // What to attack: the manual map IDs, else the current target, else anything.
    let target = if !map_ids.is_empty() {
        None
    } else {
        player::target(bot)?.filter(|_| player::has_target(bot).unwrap_or(false))
    };
    let label = match (&target, map_ids.is_empty()) {
        (_, false) => format!("MapIDs:{map_ids:?}"),
        (Some(monster), true) => monster.name.clone(),
        (None, true) => "*".into(),
    };
    bot.log(format!("[Auto Attack] Attacking {label}"));

    loop {
        if !map_ids.is_empty() {
            // Always prefer the earliest ID that is alive.
            let living = monsters::map_monsters(bot)?;
            let next = map_ids
                .iter()
                .find(|id| living.iter().any(|m| m.map_id == **id && m.alive()));
            match next {
                Some(id) => {
                    combat::attack(bot, NameOrId::Id(*id))?;
                    bot.sleep(200)?;
                }
                None => bot.sleep(50)?,
            }
        } else if let Some(monster) = &target {
            let id = NameOrId::Id(monster.map_id);
            if monsters::exists(bot, id)? && combat::attack(bot, id)? {
                kill::monster(bot, id)?;
            }
            bot.sleep(bot.opts().action_delay.max(0) as u64)?;
        } else {
            // Any living monster: engage the first one that can be attacked.
            let mut engaged = false;
            for monster in monsters::map_monsters(bot)? {
                if !monster.alive() || !combat::attack(bot, NameOrId::Id(monster.map_id))? {
                    continue;
                }
                kill::monster(bot, NameOrId::Id(monster.map_id))?;
                engaged = true;
                break;
            }
            if !engaged {
                // Nothing to fight here right now (VibeSkua spins; a short wait is kinder).
                bot.sleep(100)?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_map_ids() {
        assert_eq!(super::parse_map_ids("1, 2 3;x;4"), [1, 2, 3, 4]);
        assert!(super::parse_map_ids("").is_empty());
    }
}
