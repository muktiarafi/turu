//! The "Auto" popup's Auto Attack and Auto Hunt (VibeSkua `IScriptAuto`, `ScriptAuto.cs`): a
//! stoppable loop on its own thread, marked as a script thread so `control.request_stop()`
//! ends it.

use crate::Bot;
use crate::api::boosts::BoostType;
use crate::api::{NameOrId, combat, dispatch, hunt, inventory, kill, monsters, player};
use crate::control::mark_script_thread;
use crate::error::{BotError, BotResult};
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Auto Attack fights in your cell; Auto Hunt jumps to the cells where its targets live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoMode {
    Attack,
    Hunt,
}

impl AutoMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Attack => "Auto Attack",
            Self::Hunt => "Auto Hunt",
        }
    }
}

/// Whether Auto Attack/Hunt runs (VibeSkua `IsRunning`).
#[derive(Default)]
pub struct AutoState {
    running: AtomicBool,
    hunting: AtomicBool,
    /// VibeSkua `_target`: kept between runs, so a hunt started during a counter attack
    /// (`StopAttacking`) goes after the previous run's target.
    target: Mutex<String>,
}

impl AutoState {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// The running mode, if any.
    pub fn mode(&self) -> Option<AutoMode> {
        self.is_running().then(|| {
            if self.hunting.load(Ordering::Relaxed) {
                AutoMode::Hunt
            } else {
                AutoMode::Attack
            }
        })
    }
}

/// Starts Auto Attack or Auto Hunt on a new thread, with `class`'s skill set in `class_mode`
/// (VibeSkua passes both only when both were picked; otherwise the equipped class in "Base").
/// `map_ids`, when given, are fought in that priority order. Returns false when it already
/// runs.
pub fn start(
    bot: &Arc<Bot>,
    mode: AutoMode,
    class: Option<String>,
    class_mode: String,
    map_ids: Vec<i64>,
) -> bool {
    if bot.auto.running.swap(true, Ordering::AcqRel) {
        return false;
    }
    bot.auto
        .hunting
        .store(mode == AutoMode::Hunt, Ordering::Relaxed);
    let thread_bot = bot.clone();
    let spawned = std::thread::Builder::new()
        .name(format!("Turu {}", mode.label()))
        .spawn(move || {
            let bot = thread_bot;
            // Like the script thread: Stop (`request_stop`) makes the next bot call fail.
            mark_script_thread(true);
            bot.control.reset();
            let result = run(&bot, mode, class, &class_mode, &map_ids);
            mark_script_thread(false);
            if !bot.scripts.is_running() {
                bot.control.reset();
            }
            // VibeSkua's `finally`.
            bot.drops.stop();
            let _ = dispatch(&bot, "skills.stop", &[]);
            bot.boosts.stop();
            bot.options.write().infinite_range = false;
            bot.log(format!("[{}] Stopped", mode.label()));
            bot.auto.running.store(false, Ordering::Release);
            match result {
                Ok(()) | Err(BotError::Stopped) => {}
                Err(e) => bot.logs.debug.push(format!("{}: {e}", mode.label())),
            }
        });
    if let Err(e) = spawned {
        bot.logs
            .debug
            .push(format!("Couldn't start {}: {e}", mode.label()));
        bot.auto.running.store(false, Ordering::Release);
        return false;
    }
    true
}

/// Stops Auto Attack/Hunt (no-op when it isn't running).
pub fn stop(bot: &Bot) {
    if bot.auto.is_running() {
        bot.control.request_stop();
    }
}

/// "1, 2 3;4" -> [1, 2, 3, 4] (VibeSkua `ParseManualMapIDs`).
pub fn parse_map_ids(text: &str) -> Vec<i64> {
    text.split([',', ' ', ';'])
        .filter_map(|part| part.trim().parse().ok())
        .collect()
}

fn action_delay(bot: &Bot) -> u64 {
    bot.opts().action_delay.max(0) as u64
}

/// VibeSkua `_DoActionAuto` then `_Attack` or `_Hunt`.
fn run(
    bot: &Arc<Bot>,
    mode: AutoMode,
    class: Option<String>,
    class_mode: &str,
    map_ids: &[i64],
) -> BotResult<()> {
    if !player::logged_in(bot)? {
        bot.log(format!("[{}] Log in first", mode.label()));
        return Ok(());
    }
    // VibeSkua `CheckDropsandBoosts`.
    if !bot.drops.to_pickup().is_empty() || !bot.drops.to_pickup_ids().is_empty() {
        bot.drops.start(bot);
    }
    let boosts = [
        BoostType::Class,
        BoostType::Experience,
        BoostType::Gold,
        BoostType::Reputation,
    ];
    if boosts.iter().any(|b| bot.boosts.uses(*b)) {
        bot.boosts.start(bot);
    }
    bot.options.write().infinite_range = true;
    let class = match class {
        Some(class) => class,
        None => inventory::current_class(bot)?.unwrap_or_else(|| "Generic".into()),
    };
    dispatch(
        bot,
        "skills.startAdvanced",
        &[json!(class), json!(true), json!(class_mode)],
    )?;
    match mode {
        AutoMode::Attack => attack(bot, map_ids),
        AutoMode::Hunt => hunt(bot, map_ids),
    }
}

/// The highest-priority (earliest) map ID that is alive, and its monster.
fn first_alive(bot: &Bot, map_ids: &[i64]) -> BotResult<Option<crate::models::Monster>> {
    let all = monsters::map_monsters(bot)?;
    Ok(map_ids.iter().find_map(|id| {
        all.iter()
            .find(|m| m.map_id == *id)
            .filter(|m| m.alive())
            .cloned()
    }))
}

/// VibeSkua `_Attack`.
fn attack(bot: &Arc<Bot>, map_ids: &[i64]) -> BotResult<()> {
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
    *bot.auto.target.lock() = label.clone();
    bot.log(format!("[Auto Attack] Attacking {label}"));

    loop {
        if !map_ids.is_empty() {
            // Always prefer the earliest ID that is alive.
            match first_alive(bot, map_ids)? {
                Some(monster) => {
                    combat::attack(bot, NameOrId::Id(monster.map_id))?;
                    bot.sleep(200)?;
                }
                None => bot.sleep(50)?,
            }
        } else if let Some(monster) = &target {
            let id = NameOrId::Id(monster.map_id);
            if monsters::exists(bot, id)? && combat::attack(bot, id)? {
                kill::monster(bot, id)?;
            }
            bot.sleep(action_delay(bot))?;
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

/// VibeSkua's hunt jump: to `cell` (pad Left) unless you're there, at most once per
/// `HuntDelay`.
fn go_to(bot: &Bot, cell: &str) -> BotResult<()> {
    if player::cell(bot)? != cell {
        hunt::hunt_jump(bot, cell)?;
    }
    Ok(())
}

/// Attacks `monster` every `ActionDelay` while `hp` (looked up again each time) reads alive.
fn attack_while_alive(
    bot: &Arc<Bot>,
    monster: NameOrId,
    hp: impl Fn(&Bot) -> BotResult<Option<i64>>,
) -> BotResult<()> {
    while hp(bot)?.is_some_and(|hp| hp > 0) {
        combat::attack(bot, monster)?;
        bot.sleep(action_delay(bot))?;
    }
    Ok(())
}

/// VibeSkua `_Hunt`: the manual map IDs in priority order, else the current target's name
/// throughout the map, else every monster of your cell throughout the map.
fn hunt(bot: &Arc<Bot>, map_ids: &[i64]) -> BotResult<()> {
    let target = if !map_ids.is_empty() {
        let ids: Vec<String> = map_ids.iter().map(i64::to_string).collect();
        format!("MapIDs:[{}]", ids.join(", "))
    } else if bot.combat.stop_attacking() {
        bot.auto.target.lock().clone()
    } else if player::has_target(bot)? {
        player::target(bot)?.map_or_else(|| "*".into(), |m| m.name)
    } else {
        let mut names: Vec<String> = Vec::new();
        for monster in monsters::current_monsters(bot)? {
            if !names.contains(&monster.name) {
                names.push(monster.name);
            }
        }
        names.join("|")
    };
    *bot.auto.target.lock() = target.clone();
    bot.log(format!("[Auto Hunt] Hunting for {target}"));

    if !map_ids.is_empty() {
        return hunt_priority(bot, map_ids);
    }
    // Turu: VibeSkua sends "A|B" to the one-name hunt too, which finds no monster named
    // "A|B" and idles; its multi-name loop below only ever got "" or "*".
    if !target.is_empty() && target != "*" && !target.contains('|') {
        return hunt_name(bot, &target);
    }
    hunt_names(bot, &target)
}

/// Goes to the earliest living map ID's cell and attacks it until it dies.
fn hunt_priority(bot: &Arc<Bot>, map_ids: &[i64]) -> BotResult<()> {
    loop {
        let Some(monster) = first_alive(bot, map_ids)? else {
            bot.sleep(500)?;
            continue;
        };
        let id = monster.map_id;
        go_to(bot, &monster.cell)?;
        if monsters::exists(bot, NameOrId::Id(id))? {
            attack_while_alive(bot, NameOrId::Id(id), |bot| {
                Ok(monsters::map_monsters(bot)?
                    .into_iter()
                    .find(|m| m.map_id == id)
                    .map(|m| m.hp))
            })?;
        }
        bot.sleep(200)?;
    }
}

/// Goes through the cells where monsters of `name`'s type live, attacking it in each.
fn hunt_name(bot: &Arc<Bot>, name: &str) -> BotResult<()> {
    loop {
        let cells = monsters::living_monster_data_leaf_cells(bot, NameOrId::Name(name))?;
        if cells.is_empty() {
            bot.sleep(200)?;
            continue;
        }
        for cell in &cells {
            go_to(bot, cell)?;
            if monsters::exists(bot, NameOrId::Name(name))? {
                attack_while_alive(bot, NameOrId::Name(name), |bot| {
                    Ok(monsters::current_monsters(bot)?
                        .into_iter()
                        .find(|m| m.name_matches(name))
                        .map(|m| m.hp))
                })?;
            }
            bot.sleep(200)?;
        }
    }
}

/// `names` ("A|B"): walks the cells where they live from the last one, killing one of them
/// in each and dropping cells with none of them left.
fn hunt_names(bot: &Arc<Bot>, names: &str) -> BotResult<()> {
    let names: Vec<&str> = names.split('|').collect();
    let living_cells = |bot: &Bot| -> BotResult<Vec<String>> {
        let all = monsters::map_monsters(bot)?;
        let mut cells: Vec<String> = Vec::new();
        for name in &names {
            for cell in monsters::living_data_leaf_cells_in(&all, NameOrId::Name(name)) {
                if !cells.contains(&cell) {
                    cells.push(cell);
                }
            }
        }
        Ok(cells)
    };
    let mut cells = living_cells(bot)?;
    loop {
        if cells.is_empty() {
            // Turu: VibeSkua keeps looping over the emptied list (doing nothing until
            // stopped); look again instead, like `hunt::for_item`.
            bot.sleep(200)?;
            cells = living_cells(bot)?;
            continue;
        }
        let mut i = cells.len();
        while i > 0 {
            i -= 1;
            go_to(bot, &cells[i].clone())?;
            // VibeSkua `cells.RemoveAt(i)` for each name that isn't here (or can't be
            // attacked); done once here, so several names can't remove other cells (or
            // throw, which ended VibeSkua's hunt).
            let mut remove = false;
            for name in &names {
                if monsters::exists(bot, NameOrId::Name(name))? {
                    if !combat::attack(bot, NameOrId::Name(name))? {
                        remove = true;
                        continue;
                    }
                    bot.sleep(action_delay(bot))?;
                    kill::monster(bot, NameOrId::Name(name))?;
                    break;
                }
                remove = true;
            }
            if remove {
                cells.remove(i);
            }
        }
    }
}
