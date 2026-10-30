//! `bot.quests` (VibeSkua `ScriptQuest`).

use super::inventory::{self, Container};
use super::{Args, NameOrId, map, ok, player, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::events::BotEvent;
use crate::models::Quest;
use crate::skills::{Worker, nap};
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};

/// VibeSkua `RegisterCompleteInterval`.
const REGISTER_COMPLETE_INTERVAL: u64 = 2000;

#[derive(Default)]
struct Registry {
    /// Registered quest IDs, in registration order.
    quests: Vec<i64>,
    /// The reward to pick per quest (-1: none).
    rewards: HashMap<i64, i64>,
    complete_cooldowns: HashMap<i64, Instant>,
    accept_cooldowns: HashMap<i64, Instant>,
}

#[derive(Default)]
struct QuestShared {
    registry: Mutex<Registry>,
    /// The "Quest Thread" (VibeSkua `_Poll`).
    poller: Worker,
    /// Loads the registered quests to add their items to the drop list.
    loader: Worker,
    /// The `Logout` subscription.
    subscribed: OnceLock<u64>,
}

/// The registered-quest auto turn-in loop (VibeSkua `RegisterQuests`).
#[derive(Default)]
pub struct QuestRunner {
    shared: Arc<QuestShared>,
}

impl QuestRunner {
    /// VibeSkua `Registered`.
    pub fn registered(&self) -> Vec<i64> {
        self.shared.registry.lock().quests.clone()
    }

    /// VibeSkua `RegisterQuests((questId, rewardId)...)`: registers the quests, adds the
    /// rewards (and, once loaded, the non-temporary requirements) to the drop list, starts
    /// the drop loop and the "Quest Thread", which every 2 s accepts the registered quests
    /// and turns in one that can be completed.
    pub fn register(&self, bot: &Arc<Bot>, quests: &[(i64, i64)]) {
        if quests.is_empty() {
            return;
        }
        self.subscribe(bot);
        {
            let mut registry = self.shared.registry.lock();
            for &(id, reward) in quests {
                if !registry.quests.contains(&id) {
                    registry.quests.push(id);
                }
                registry.rewards.insert(id, reward);
                if reward > 0 {
                    bot.drops.add(&[NameOrId::Id(reward)]);
                }
            }
        }
        if !bot.drops.running() {
            bot.drops.start(bot);
        }

        // Load the quests and add their requirements to the drop list in the background.
        self.shared.loader.stop(Duration::ZERO);
        let ids: Vec<i64> = quests.iter().map(|&(id, _)| id).collect();
        let weak = Arc::downgrade(bot);
        self.shared.loader.start("Quest Loader", move |cancel| {
            for id in ids {
                let Some(bot) = weak.upgrade() else { return };
                let quest = match ensure_load_cancellable(&bot, id, Some(cancel)) {
                    Ok(Some(q)) => q,
                    Ok(None) => continue,
                    Err(_) => return,
                };
                if cancel.load(Ordering::Acquire) {
                    return;
                }
                let items: Vec<NameOrId> = quest
                    .requirements
                    .iter()
                    .chain(&quest.accept_requirements)
                    .filter(|r| !r.temp)
                    .map(|r| NameOrId::Id(r.id))
                    .collect();
                bot.drops.add(&items);
            }
        });

        let weak = Arc::downgrade(bot);
        let shared = self.shared.clone();
        self.shared.poller.start("Quest Thread", move |cancel| {
            quest_loop(&weak, &shared, cancel)
        });
    }

    /// VibeSkua `UnregisterQuests`.
    pub fn unregister(&self, ids: &[i64]) {
        let mut registry = self.shared.registry.lock();
        registry.quests.retain(|q| !ids.contains(q));
        for id in ids {
            registry.rewards.remove(id);
            registry.complete_cooldowns.remove(id);
            registry.accept_cooldowns.remove(id);
        }
    }

    /// VibeSkua `UnregisterAllQuests`: clears the registry and stops the quest thread
    /// (waits up to 2 s). Called when a script ends.
    pub fn unregister_all(&self) {
        *self.shared.registry.lock() = Registry::default();
        self.shared.loader.stop(Duration::ZERO);
        self.shared.poller.stop(Duration::from_secs(2));
    }

    /// VibeSkua unregisters everything on logout. The handler only touches our own state.
    fn subscribe(&self, bot: &Bot) {
        self.shared.subscribed.get_or_init(|| {
            let shared = Arc::downgrade(&self.shared);
            bot.events.subscribe(move |event| {
                if let (BotEvent::Logout, Some(shared)) = (event, shared.upgrade()) {
                    *shared.registry.lock() = Registry::default();
                    shared.loader.cancel();
                    shared.poller.cancel();
                }
            })
        });
    }
}

fn quest_loop(weak: &Weak<Bot>, shared: &QuestShared, cancel: &AtomicBool) {
    let mut last_complete = Instant::now();
    loop {
        let Some(bot) = weak.upgrade() else { return };
        let result = (|| -> BotResult<()> {
            if player::playing(&bot)? {
                let registered = shared.registry.lock().quests.clone();
                if !registered.is_empty() {
                    complete_registered(&bot, shared, &registered, &mut last_complete, cancel)?;
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {}
            Err(BotError::Stopped) => return,
            Err(e) => tracing::debug!("Quest processing iteration failed; it will retry: {e}"),
        }
        if !matches!(nap(&bot, REGISTER_COMPLETE_INTERVAL, cancel), Ok(true)) {
            return;
        }
    }
}

/// Waits (`tries` x 100 ms) for `predicate`, ending early when the loop is cancelled.
fn wait_cancellable(
    bot: &Bot,
    tries: i64,
    cancel: &AtomicBool,
    mut predicate: impl FnMut() -> BotResult<bool>,
) -> BotResult<bool> {
    for _ in 0..tries {
        if wait::safe(predicate())? {
            return Ok(true);
        }
        if !nap(bot, wait::WAIT_SLEEP, cancel)? {
            return Ok(false);
        }
    }
    wait::safe(predicate())
}

/// VibeSkua `_CompleteQuest`: accepts registered quests that aren't in progress (2 s
/// cooldown each) and turns in the first one that can be completed (3 s cooldown), with
/// `tryQuestComplete` as many times as the game allows.
fn complete_registered(
    bot: &Bot,
    shared: &QuestShared,
    registered: &[i64],
    last_complete: &mut Instant,
    cancel: &AtomicBool,
) -> BotResult<()> {
    if last_complete.elapsed() > Duration::from_secs(60) {
        let mut registry = shared.registry.lock();
        let quests = registry.quests.clone();
        registry
            .complete_cooldowns
            .retain(|k, _| quests.contains(k));
        registry.accept_cooldowns.retain(|k, _| quests.contains(k));
    }
    let delay = bot.opts().action_delay.max(0) as u64;
    let accept_again = |quest: i64| -> BotResult<()> {
        wait::for_action_cooldown(bot, "acceptQuest", 40)?;
        accept(bot, quest)?;
        shared
            .registry
            .lock()
            .accept_cooldowns
            .insert(quest, Instant::now());
        wait_cancellable(bot, 20, cancel, || is_in_progress(bot, quest))?;
        nap(bot, delay, cancel)?;
        Ok(())
    };

    for &quest in registered {
        if cancel.load(Ordering::Acquire) {
            return Ok(());
        }
        let recently_tried = shared
            .registry
            .lock()
            .complete_cooldowns
            .get(&quest)
            .is_some_and(|t| t.elapsed() < Duration::from_secs(3));
        if recently_tried {
            continue;
        }
        if !is_in_progress(bot, quest)? {
            let recently_accepted = shared
                .registry
                .lock()
                .accept_cooldowns
                .get(&quest)
                .is_some_and(|t| t.elapsed() < Duration::from_secs(2));
            if recently_accepted {
                continue;
            }
            accept_again(quest)?;
        }

        if !can_complete(bot, quest)? {
            continue;
        }
        let Some(data) = get(bot, quest)? else {
            continue;
        };
        let turn_ins = if data.once || !data.field.is_empty() {
            1
        } else {
            let max: Option<Value> = bot
                .flash
                .call_game_function_json("world.maximumQuestTurnIns", vec![quest.into()])?;
            crate::api::packets::num(max.as_ref()).unwrap_or(1)
        };
        let reward = shared
            .registry
            .lock()
            .rewards
            .get(&quest)
            .copied()
            .unwrap_or(-1);
        wait::for_action_cooldown(bot, "tryQuestComplete", 40)?;
        // The items may be gone by now (another turn-in, a bank move).
        if !can_complete(bot, quest)? {
            continue;
        }
        let room = map::room_id(bot)?;
        bot.flash.send_packet(&format!(
            "%xt%zm%tryQuestComplete%{room}%{quest}%{reward}%false%{turn_ins}%wvz%"
        ))?;
        shared
            .registry
            .lock()
            .complete_cooldowns
            .insert(quest, Instant::now());
        wait_cancellable(bot, 20, cancel, || Ok(!is_in_progress(bot, quest)?))?;
        nap(bot, delay, cancel)?;

        // The game re-accepts by itself when AQW Lite's "Reaccept quest" is on.
        if !bot.flash.get_or("litePreference.data.bReaccept", false)? {
            accept_again(quest)?;
        }
        *last_complete = Instant::now();
        // One turn-in per pass: the game handles one at a time.
        break;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Quest data
// ---------------------------------------------------------------------------------------

/// `world.questTree`: the quests the client has loaded.
pub fn tree(bot: &Bot) -> BotResult<Vec<Quest>> {
    let raw: Option<Value> = bot.flash.get_opt("world.questTree")?;
    Ok(raw.map(Quest::tree_from).unwrap_or_default())
}

/// The loaded quest with this ID (VibeSkua `TryGetQuest`).
pub fn get(bot: &Bot, id: i64) -> BotResult<Option<Quest>> {
    Ok(tree(bot)?.into_iter().find(|q| q.id == id))
}

/// VibeSkua `Load(ids)`: `world.showQuests("1,2,3", "q")`, in chunks of 30 (sleeping
/// `actionDelay` between chunks) for 30 or more.
pub fn load(bot: &Bot, ids: &[i64]) -> BotResult<()> {
    let join = |ids: &[i64]| ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    if ids.len() < 30 {
        return bot
            .flash
            .call_game_function("world.showQuests", vec![join(ids).into(), "q".into()]);
    }
    let delay = bot.opts().action_delay.max(0) as u64;
    for chunk in ids.chunks(30) {
        bot.flash
            .call_game_function("world.showQuests", vec![join(chunk).into(), "q".into()])?;
        bot.sleep(delay)?;
    }
    Ok(())
}

fn ensure_load_cancellable(
    bot: &Bot,
    id: i64,
    cancel: Option<&AtomicBool>,
) -> BotResult<Option<Quest>> {
    // VibeSkua `Wait.ForTrue(loaded, () => Load(id), 20, 1000)`.
    for i in 0..=20 {
        let found = match get(bot, id) {
            Ok(quest) => quest,
            Err(BotError::Stopped) => return Err(BotError::Stopped),
            Err(_) => None,
        };
        if found.is_some() {
            return Ok(found);
        }
        if i == 20 {
            break;
        }
        wait::swallow(load(bot, &[id]))?;
        match cancel {
            Some(cancel) => {
                if !nap(bot, 1000, cancel)? {
                    return Ok(None);
                }
            }
            None => bot.sleep(1000)?,
        }
    }
    get(bot, id)
}

/// VibeSkua `EnsureLoad(id)`: loads the quest and waits (20 tries of 1 s) until the client
/// has it.
pub fn ensure_load(bot: &Bot, id: i64) -> BotResult<Option<Quest>> {
    ensure_load_cancellable(bot, id, None)
}

// Shared with other modules (wait, kill). Signatures are fixed.

/// `world.isQuestInProgress(id)`.
pub fn is_in_progress(bot: &Bot, id: i64) -> BotResult<bool> {
    let raw = bot
        .flash
        .call_game_function_raw("world.isQuestInProgress", vec![id.into()])?;
    Ok(crate::flash::parse_bool(&raw))
}

/// The quest's turn-in requirements are met (`status == "c"` in `world.questTree`).
pub fn can_complete(bot: &Bot, id: i64) -> BotResult<bool> {
    Ok(tree(bot)?.iter().any(|q| q.id == id && q.completable()))
}

// ---------------------------------------------------------------------------------------
// Accept and complete
// ---------------------------------------------------------------------------------------

/// VibeSkua `Accept(id)`: waits for the `acceptQuest` cooldown, accepts and waits (14
/// tries) until it's in progress.
pub fn accept(bot: &Bot, id: i64) -> BotResult<bool> {
    wait::for_action_cooldown(bot, "acceptQuest", 40)?;
    bot.flash
        .call_game_function("world.acceptQuest", vec![id.into()])?;
    wait::for_quest_accept(bot, id, 14)?;
    is_in_progress(bot, id)
}

/// VibeSkua `Accept(params ids)`: each one, sleeping `actionDelay` after it.
pub fn accept_many(bot: &Bot, ids: &[i64]) -> BotResult<()> {
    let delay = bot.opts().action_delay.max(0) as u64;
    for &id in ids {
        accept(bot, id)?;
        bot.sleep(delay)?;
    }
    Ok(())
}

/// VibeSkua `EnsureAccept(id)`: up to `questAcceptAndCompleteTries` accepts.
pub fn ensure_accept(bot: &Bot, id: i64) -> BotResult<bool> {
    let (tries, delay) = {
        let o = bot.opts();
        (
            o.quest_accept_and_complete_tries,
            o.action_delay.max(0) as u64,
        )
    };
    for _ in 0..tries {
        accept(bot, id)?;
        if is_in_progress(bot, id)? {
            break;
        }
        bot.sleep(delay)?;
    }
    is_in_progress(bot, id)
}

/// VibeSkua `Complete(id, itemId, special)`: waits for the `tryQuestComplete` cooldown,
/// turns it in and waits (10 tries) until it's no longer in progress.
pub fn complete(bot: &Bot, id: i64, item_id: i64, special: bool) -> BotResult<bool> {
    wait::for_action_cooldown(bot, "tryQuestComplete", 40)?;
    bot.flash.call_game_function(
        "world.tryQuestComplete",
        vec![id.into(), item_id.into(), special.into()],
    )?;
    wait::for_quest_complete(bot, id, 10)?;
    Ok(!is_in_progress(bot, id)?)
}

/// VibeSkua `EnsureComplete(id, itemId, special)`: up to `questAcceptAndCompleteTries`.
pub fn ensure_complete(bot: &Bot, id: i64, item_id: i64, special: bool) -> BotResult<bool> {
    if id != 0 {
        let (tries, delay) = {
            let o = bot.opts();
            (
                o.quest_accept_and_complete_tries,
                o.action_delay.max(0) as u64,
            )
        };
        for _ in 0..tries {
            complete(bot, id, item_id, special)?;
            if !is_in_progress(bot, id)? {
                break;
            }
            bot.sleep(delay)?;
        }
    }
    Ok(!is_in_progress(bot, id)?)
}

// ---------------------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------------------

/// VibeSkua `CanCompleteFullCheck`: `canComplete`, or every turn-in and accept requirement
/// is in the temp inventory, inventory, bank or house (nothing is moved).
pub fn can_complete_full_check(bot: &Bot, id: i64) -> BotResult<bool> {
    if can_complete(bot, id)? {
        return Ok(true);
    }
    let Some(quest) = ensure_load(bot, id)? else {
        return Ok(false);
    };
    for item in quest.requirements.iter().chain(&quest.accept_requirements) {
        if !inventory::check(bot, NameOrId::Id(item.id), item.quantity, false)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn game_int(bot: &Bot, path: &str, args: Vec<crate::FlashArg>) -> BotResult<i64> {
    let value: Option<Value> = bot.flash.call_game_function_json(path, args)?;
    Ok(crate::api::packets::num(value.as_ref()).unwrap_or_default())
}

/// VibeSkua `IsDailyComplete`: `world.getAchievement(field, index) > 0`.
fn daily_complete(bot: &Bot, quest: &Quest) -> BotResult<bool> {
    Ok(game_int(
        bot,
        "world.getAchievement",
        vec![quest.field.clone().into(), quest.index.into()],
    )? > 0)
}

/// VibeSkua `IsUnlocked`: the chain value reached `value - 1` (or the quest has no slot).
fn unlocked(bot: &Bot, quest: &Quest) -> BotResult<bool> {
    Ok(quest.slot < 0
        || game_int(bot, "world.getQuestValue", vec![quest.slot.into()])? >= quest.value - 1)
}

/// VibeSkua `HasBeenCompleted`: the chain value reached `value`.
fn completed_before(bot: &Bot, quest: &Quest) -> BotResult<bool> {
    Ok(quest.slot < 0
        || game_int(bot, "world.getQuestValue", vec![quest.slot.into()])? >= quest.value)
}

/// VibeSkua `IsAvailable`: not done today, unlocked, membership, level, class points,
/// faction reputation and accept requirements.
pub fn is_available(bot: &Bot, id: i64) -> BotResult<bool> {
    let Some(quest) = ensure_load(bot, id)? else {
        return Ok(false);
    };
    if daily_complete(bot, &quest)? || !unlocked(bot, &quest)? {
        return Ok(false);
    }
    if quest.upgrade
        && bot
            .flash
            .get_or::<i64>("world.myAvatar.objData.iUpgDays", -1)?
            < 0
    {
        return Ok(false);
    }
    if bot
        .flash
        .get_or::<i64>("world.myAvatar.dataLeaf.intLevel", 0)?
        < quest.level
    {
        return Ok(false);
    }
    if quest.required_class_id > 0
        && game_int(
            bot,
            "world.myAvatar.getCPByID",
            vec![quest.required_class_id.into()],
        )? < quest.required_class_points
    {
        return Ok(false);
    }
    if quest.required_faction_id > 1
        && game_int(
            bot,
            "world.myAvatar.getRep",
            vec![quest.required_faction_id.into()],
        )? < quest.required_faction_rep
    {
        return Ok(false);
    }
    let inv = inventory::items(bot, Container::Inventory)?;
    Ok(quest
        .accept_requirements
        .iter()
        .all(|r| inventory::contains_in(&inv, NameOrId::Name(&r.name), r.quantity)))
}

/// VibeSkua `UpdateQuest(id)`: tells the client (a fake `updateQuest` packet) that the
/// quest's chain value is reached, fixing stale quest locks.
pub fn update_quest(bot: &Bot, id: i64) -> BotResult<bool> {
    let Some(quest) = ensure_load(bot, id)? else {
        return Ok(false);
    };
    update_quest_slot(bot, quest.value, quest.slot)?;
    Ok(true)
}

/// VibeSkua `UpdateQuest(value, slot)`: the same fake `updateQuest` packet with a chain value and
/// quest slot of your own.
pub fn update_quest_slot(bot: &Bot, value: i64, slot: i64) -> BotResult<()> {
    let packet = format!(
        "{{\"t\":\"xt\",\"b\":{{\"r\":-1,\"o\":{{\"cmd\":\"updateQuest\",\"iValue\":{value},\"iIndex\":{slot}}}}}}}"
    );
    bot.flash
        .call("sendClientPacket", vec![packet.into(), "json".into()])?;
    Ok(())
}

fn with_quest(bot: &Bot, id: i64, f: impl FnOnce(&Quest) -> BotResult<bool>) -> BotResult<bool> {
    match ensure_load(bot, id)? {
        Some(quest) => f(&quest),
        None => Ok(false),
    }
}

/// Quest IDs from `(...ids)` (an array argument).
fn ids(args: &Args, i: usize) -> Vec<i64> {
    match args.value(i) {
        Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .collect(),
        Value::Number(n) => n.as_i64().into_iter().collect(),
        _ => Vec::new(),
    }
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "tree" => ok(tree(bot)?),
        "active" => ok(tree(bot)?
            .into_iter()
            .filter(Quest::active)
            .collect::<Vec<_>>()),
        "registered" => ok(bot.quests.registered()),
        "load" => ok(load(bot, &ids(&args, 0))?),
        "ensureLoad" => ok(ensure_load(bot, args.i64(0)?)?),
        "get" => ok(get(bot, args.i64(0)?)?),
        "accept" => ok(accept(bot, args.i64(0)?)?),
        "acceptMany" => ok(accept_many(bot, &ids(&args, 0))?),
        "ensureAccept" => ok(ensure_accept(bot, args.i64(0)?)?),
        "complete" => ok(complete(
            bot,
            args.i64(0)?,
            args.i64_or(1, -1),
            args.bool_or(2, false),
        )?),
        "ensureComplete" => ok(ensure_complete(
            bot,
            args.i64(0)?,
            args.i64_or(1, -1),
            args.bool_or(2, false),
        )?),
        "isInProgress" => ok(is_in_progress(bot, args.i64(0)?)?),
        "canComplete" => ok(can_complete(bot, args.i64(0)?)?),
        "canCompleteFullCheck" => ok(can_complete_full_check(bot, args.i64(0)?)?),
        "isUnlocked" => ok(with_quest(bot, args.i64(0)?, |q| unlocked(bot, q))?),
        "hasBeenCompleted" => ok(with_quest(bot, args.i64(0)?, |q| completed_before(bot, q))?),
        "isDailyComplete" => ok(with_quest(bot, args.i64(0)?, |q| daily_complete(bot, q))?),
        "isAvailable" => ok(is_available(bot, args.i64(0)?)?),
        "updateQuest" => {
            if args.is_missing(1) {
                update_quest(bot, args.i64(0)?)?;
            } else {
                update_quest_slot(bot, args.i64(0)?, args.i64(1)?)?;
            }
            ok(())
        }
        "registerQuests" => {
            let quests: Vec<(i64, i64)> = ids(&args, 0).into_iter().map(|id| (id, -1)).collect();
            bot.quests.register(bot, &quests);
            ok(())
        }
        "unregisterQuests" => {
            bot.quests.unregister(&ids(&args, 0));
            ok(())
        }
        "unregisterAll" => {
            bot.quests.unregister_all();
            ok(())
        }
        _ => Err(BotError::UnknownMethod(format!("quests.{name}"))),
    }
}
