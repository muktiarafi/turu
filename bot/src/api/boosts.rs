//! `bot.boosts` (VibeSkua `ScriptBoost`): finds boost items and uses them on a 30 s timer while
//! their boost isn't active.

use super::inventory::{self, Container};
use super::{Args, NameOrId, map, ok, player};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::skills::{Worker, nap};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// VibeSkua `BoostType`, in the order the timer uses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoostType {
    Gold,
    Class,
    Experience,
    Reputation,
}

impl BoostType {
    const ALL: [BoostType; 4] = [Self::Gold, Self::Class, Self::Experience, Self::Reputation];

    pub fn parse(name: &str) -> BotResult<Self> {
        Ok(match name {
            "Gold" => Self::Gold,
            "Class" => Self::Class,
            "Experience" => Self::Experience,
            "Reputation" => Self::Reputation,
            other => {
                return Err(BotError::InvalidArgument(format!(
                    "unknown boost type `{other}`"
                )));
            }
        })
    }

    /// The `objData` counter that is above 0 while the boost is active.
    fn counter(self) -> &'static str {
        match self {
            Self::Gold => "iBoostG",
            Self::Class => "iBoostCP",
            Self::Reputation => "iBoostRep",
            Self::Experience => "iBoostXP",
        }
    }

    /// What the boost item's name contains.
    fn search(self) -> &'static str {
        match self {
            Self::Gold => "gold",
            Self::Class => "class",
            Self::Reputation => "rep",
            Self::Experience => "xp",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Which boosts to use, their item IDs and the timer.
#[derive(Default)]
pub struct BoostRunner {
    use_boost: [AtomicBool; 4],
    ids: [AtomicI64; 4],
    worker: Worker,
}

impl BoostRunner {
    /// VibeSkua `Boosts.Enabled`: the timer runs.
    pub fn enabled(&self) -> bool {
        self.worker.running()
    }

    /// `UseGoldBoost`, `UseClassBoost`, ...
    pub fn uses(&self, boost: BoostType) -> bool {
        self.use_boost[boost.index()].load(Ordering::Relaxed)
    }

    pub fn set_uses(&self, boost: BoostType, value: bool) {
        self.use_boost[boost.index()].store(value, Ordering::Relaxed);
    }

    /// `GoldBoostID`, `ClassBoostID`, ...
    pub fn id(&self, boost: BoostType) -> i64 {
        self.ids[boost.index()].load(Ordering::Relaxed)
    }

    pub fn set_id(&self, boost: BoostType, id: i64) {
        self.ids[boost.index()].store(id, Ordering::Relaxed);
    }

    /// Starts the timer: uses the boosts at once, then every 30 s.
    pub fn start(&self, bot: &Arc<Bot>) {
        let weak = Arc::downgrade(bot);
        self.worker
            .start("Boosts", move |cancel| boost_loop(&weak, cancel));
    }

    /// Stops the timer (waits up to 200 ms for it).
    pub fn stop(&self) {
        self.worker.stop(Duration::from_millis(200));
    }

    /// When a script ends: stops the timer and turns every boost off. (VibeSkua keeps both
    /// until a script turns them off, so a stopped script would leave the timer using boosts.)
    pub fn reset(&self) {
        self.stop();
        for boost in BoostType::ALL {
            self.set_uses(boost, false);
        }
    }
}

fn boost_loop(weak: &Weak<Bot>, cancel: &AtomicBool) {
    loop {
        let Some(bot) = weak.upgrade() else { return };
        match poll(&bot, cancel) {
            Ok(true) => {}
            Ok(false) | Err(BotError::Stopped) => return,
            Err(e) => tracing::debug!("Error in boost handler: {e}"),
        }
        match nap(&bot, 30_000, cancel) {
            Ok(true) => {}
            _ => return,
        }
    }
}

/// One pass of VibeSkua `PollBoosts`: uses each wanted boost that isn't active, 1 s apart.
/// `Ok(false)` when cancelled.
fn poll(bot: &Bot, cancel: &AtomicBool) -> BotResult<bool> {
    for boost in BoostType::ALL {
        let id = bot.boosts.id(boost);
        if !bot.boosts.uses(boost) || id == 0 || is_active(bot, boost)? {
            continue;
        }
        use_boost(bot, id)?;
        if !nap(bot, 1000, cancel)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// VibeSkua `IsBoostActive`.
pub fn is_active(bot: &Bot, boost: BoostType) -> BotResult<bool> {
    let path = format!("world.myAvatar.objData.{}", boost.counter());
    Ok(bot.flash.get_or::<f64>(&path, 0.0)? > 0.0)
}

/// VibeSkua `UseBoost`: uses the boost item `id`.
pub fn use_boost(bot: &Bot, id: i64) -> BotResult<()> {
    let room = map::room_id(bot)?;
    bot.flash
        .send_packet(&format!("%xt%zm%serverUseItem%{room}%+%{id}%"))
}

/// VibeSkua `GetBoostID`: the ID of the first boost item (category ServerUse) of `boost` in
/// the inventory, else, with `search_bank`, in the bank (moved to the inventory). 0 when there
/// is none. The name is matched ignoring case ("Gold Boost! (1 hr)" for "gold"); VibeSkua
/// matches it as is.
pub fn get_boost_id(bot: &Bot, boost: BoostType, search_bank: bool) -> BotResult<i64> {
    if !player::logged_in(bot)? {
        return Ok(0);
    }
    let find = |container| -> BotResult<i64> {
        Ok(inventory::items(bot, container)?
            .iter()
            .find(|i| {
                i.category.eq_ignore_ascii_case("ServerUse")
                    && i.name.to_lowercase().contains(boost.search())
            })
            .map_or(0, |i| i.id))
    };
    let mut id = find(Container::Inventory)?;
    if id == 0 && search_bank {
        if !bot.state.lock().bank_loaded {
            inventory::bank_load(bot, true)?;
        }
        id = find(Container::Bank)?;
        if id > 0 {
            inventory::bank_ensure_to_inventory(bot, NameOrId::Id(id), false)?;
        }
    }
    Ok(id)
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let boosts = &bot.boosts;
    match name {
        "enabled" => ok(boosts.enabled()),
        "start" => {
            boosts.start(bot);
            ok(())
        }
        "stop" => {
            boosts.stop();
            ok(())
        }
        "uses" => ok(boosts.uses(BoostType::parse(args.str(0)?)?)),
        "setUses" => {
            boosts.set_uses(BoostType::parse(args.str(0)?)?, args.bool_or(1, false));
            ok(())
        }
        "id" => ok(boosts.id(BoostType::parse(args.str(0)?)?)),
        "setId" => {
            boosts.set_id(BoostType::parse(args.str(0)?)?, args.i64(1)?);
            ok(())
        }
        "isActive" => ok(is_active(bot, BoostType::parse(args.str(0)?)?)?),
        "useBoost" => {
            use_boost(bot, args.i64(0)?)?;
            ok(())
        }
        "getBoostId" => ok(get_boost_id(
            bot,
            BoostType::parse(args.str(0)?)?,
            args.bool_or(1, true),
        )?),
        _ => Err(BotError::UnknownMethod(format!("boosts.{name}"))),
    }
}
