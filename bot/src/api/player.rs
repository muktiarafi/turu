//! `bot.player` (VibeSkua `IScriptPlayer`, `ScriptPlayer.cs`).

use super::{Args, ok, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::{Badge, Monster, lenient_bool, lenient_i64, lenient_string};
use crate::state::GameState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long a read of [`logged_in`] or [`state`] is reused (Turu). Nearly every wait and
/// background loop asks them (through `alive`, `playing`, ...), each on its own: about 25 times
/// a second per client in game. One [`snapshot`] refreshes both. One wait try
/// ([`wait::WAIT_SLEEP`]) is the most a change can go unseen.
const SHARED_READ: Duration = Duration::from_millis(100);

/// The value `slot` holds if it's younger than [`SHARED_READ`], else `read`'s (kept in `slot`).
/// The game is read without the state lock held.
fn shared<T: Copy>(
    bot: &Bot,
    slot: impl Fn(&mut GameState) -> &mut Option<(Instant, T)>,
    read: impl FnOnce() -> BotResult<T>,
) -> BotResult<T> {
    if let Some((at, value)) = *slot(&mut bot.state.lock())
        && at.elapsed() < SHARED_READ
    {
        return Ok(value);
    }
    let value = read()?;
    *slot(&mut bot.state.lock()) = Some((Instant::now(), value));
    Ok(value)
}

/// What the skill timer and the waits ask on every tick, read in one call (turu.swf's
/// `snapshot`, docs/HOST_SWF_PROPOSAL.md M4).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Snapshot {
    /// The SmartFox client is connected (`isLoggedIn`).
    #[serde(deserialize_with = "lenient_bool")]
    pub logged_in: bool,
    /// `world` isn't null.
    #[serde(deserialize_with = "lenient_bool")]
    pub world: bool,
    /// `world.myAvatar` isn't null.
    #[serde(deserialize_with = "lenient_bool")]
    pub avatar: bool,
    /// `dataLeaf.intState`: 0 dead, 1 idle, 2 in combat (0 without an avatar).
    #[serde(deserialize_with = "lenient_i64")]
    pub state: i64,
    /// [`loaded`]'s answer.
    #[serde(deserialize_with = "lenient_bool")]
    pub loaded: bool,
    /// `objData.iRank`, the class rank (0 without one).
    #[serde(deserialize_with = "lenient_i64")]
    pub rank: i64,
    target: Value,
}

impl Snapshot {
    pub fn alive(&self) -> bool {
        self.avatar && self.state > 0
    }

    /// Logged in and alive ([`playing`]).
    pub fn playing(&self) -> bool {
        self.logged_in && self.alive()
    }

    /// The target, when the snapshot was read with it ([`target`]'s answer).
    pub fn target(&self) -> Option<Monster> {
        monster_of(self.target.clone())
    }

    /// [`has_target`]'s answer, when the snapshot was read with the target.
    pub fn has_target(&self) -> bool {
        self.target().is_some_and(|m| m.alive())
    }
}

/// Reads the [`Snapshot`]. With `with_target` it also does what [`target`] does: drops a dead
/// or missing target. It refreshes the reads [`logged_in`] and [`state`] reuse.
pub fn snapshot(bot: &Bot, with_target: bool) -> BotResult<Snapshot> {
    let snap: Snapshot = bot.flash.call_json("snapshot", vec![with_target.into()])?;
    let now = Instant::now();
    let mut state = bot.state.lock();
    state.logged_in_read = Some((now, snap.logged_in));
    state.avatar_state_read = Some((now, if snap.avatar { snap.state } else { 0 }));
    Ok(snap)
}

/// `isLoggedIn`: the SmartFox client is connected.
pub fn logged_in(bot: &Bot) -> BotResult<bool> {
    shared(
        bot,
        |s| &mut s.logged_in_read,
        || Ok(snapshot(bot, false)?.logged_in),
    )
}

/// `dataLeaf.intState`: 0 dead, 1 idle, 2 in combat (0 without an avatar).
pub fn state(bot: &Bot) -> BotResult<i64> {
    shared(
        bot,
        |s| &mut s.avatar_state_read,
        || {
            let snap = snapshot(bot, false)?;
            Ok(if snap.avatar { snap.state } else { 0 })
        },
    )
}

pub fn alive(bot: &Bot) -> BotResult<bool> {
    Ok(state(bot)? > 0)
}

pub fn in_combat(bot: &Bot) -> BotResult<bool> {
    Ok(state(bot)? == 2)
}

/// Logged in and alive.
pub fn playing(bot: &Bot) -> BotResult<bool> {
    Ok(logged_in(bot)? && alive(bot)?)
}

/// VibeSkua `Flash.IsWorldLoaded`: `world` isn't null.
pub fn world_loaded(bot: &Bot) -> BotResult<bool> {
    Ok(!bot.flash.is_null("world")?)
}

/// The world is loaded and we're logged in (the background timer's `inGame`).
pub fn is_in_game(bot: &Bot) -> BotResult<bool> {
    Ok(world_loaded(bot)? && logged_in(bot)?)
}

/// Items loaded, no map load in progress and the avatar's art is loaded.
pub fn loaded(bot: &Bot) -> BotResult<bool> {
    // The item count, as VibeSkua reads it: the skill timer asks this every tick, and reading
    // the items themselves sent the whole inventory as JSON each time.
    if bot.flash.get_or::<i64>("world.myAvatar.items.length", 0)? <= 0 {
        return Ok(false);
    }
    if bot.flash.get_or("world.mapLoadInProgress", true)? {
        return Ok(false);
    }
    let art: Option<bool> = bot
        .flash
        .call_game_function_json("world.myAvatar.pMC.artLoaded", vec![])?;
    Ok(art.unwrap_or(false))
}

pub fn username(bot: &Bot) -> BotResult<String> {
    Ok(bot
        .flash
        .get_static::<String>("loginInfo.strUsername")?
        .unwrap_or_default())
}

pub fn server_ip(bot: &Bot) -> BotResult<String> {
    Ok(bot
        .flash
        .get_static::<String>("serverIP")?
        .unwrap_or_default())
}

pub fn cell(bot: &Bot) -> BotResult<String> {
    bot.flash.get_or("world.strFrame", String::new())
}

pub fn pad(bot: &Bot) -> BotResult<String> {
    bot.flash.get_or("world.strPad", String::new())
}

pub fn health(bot: &Bot) -> BotResult<i64> {
    bot.flash.get_or("world.myAvatar.dataLeaf.intHP", 0)
}

pub fn max_health(bot: &Bot) -> BotResult<i64> {
    bot.flash.get_or("world.myAvatar.dataLeaf.intHPMax", 0)
}

pub fn mana(bot: &Bot) -> BotResult<i64> {
    bot.flash.get_or("world.myAvatar.dataLeaf.intMP", 0)
}

pub fn max_mana(bot: &Bot) -> BotResult<i64> {
    bot.flash.get_or("world.myAvatar.dataLeaf.intMPMax", 0)
}

pub fn x(bot: &Bot) -> BotResult<f64> {
    bot.flash.get_or("world.myAvatar.pMC.x", 0.0)
}

pub fn y(bot: &Bot) -> BotResult<f64> {
    bot.flash.get_or("world.myAvatar.pMC.y", 0.0)
}

/// `getTargetMonster`: the targeted monster, or `None` (the callback answers `{}` and
/// cancels the target when it's missing or dead).
pub fn target(bot: &Bot) -> BotResult<Option<Monster>> {
    let raw = bot.flash.call("getTargetMonster", vec![])?;
    Ok(monster_of(
        serde_json::from_str(raw.trim()).unwrap_or(Value::Null),
    ))
}

/// A monster's JSON, or `None` for nothing or `{}`.
fn monster_of(value: Value) -> Option<Monster> {
    if value.as_object().is_none_or(|o| o.is_empty()) {
        return None;
    }
    serde_json::from_value(value).ok()
}

/// VibeSkua `HasTarget`: the target exists and is alive.
pub fn has_target(bot: &Bot) -> BotResult<bool> {
    Ok(target(bot)?.is_some_and(|m| m.alive()))
}

/// `world.WALKSPEED`.
pub fn walk_speed(bot: &Bot) -> BotResult<i64> {
    bot.flash.get_or("world.WALKSPEED", 8)
}

pub fn set_walk_speed(bot: &Bot, speed: i64) -> BotResult<()> {
    bot.flash.set("world.WALKSPEED", speed)
}

/// A skill of the action bar (`world.actions.active[i]`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase"))]
pub struct SkillInfo {
    #[serde(skip_deserializing)]
    pub index: usize,
    #[serde(
        rename(deserialize = "nam"),
        deserialize_with = "lenient_string",
        default
    )]
    pub name: String,
    #[serde(
        rename(deserialize = "mp", serialize = "manaCost"),
        deserialize_with = "lenient_i64",
        default
    )]
    pub mana_cost: i64,
    #[serde(rename(deserialize = "cd"), deserialize_with = "lenient_i64", default)]
    pub cooldown: i64,
    #[serde(
        rename(deserialize = "isOK"),
        deserialize_with = "lenient_bool",
        default
    )]
    pub is_ok: bool,
}

/// `world.actions.active`.
pub fn skills(bot: &Bot) -> BotResult<Vec<SkillInfo>> {
    let raw: Vec<Value> = bot
        .flash
        .get_opt("world.actions.active")?
        .unwrap_or_default();
    Ok(raw
        .into_iter()
        .enumerate()
        .map(|(index, v)| SkillInfo {
            index,
            ..serde_json::from_value(v).unwrap_or_default()
        })
        .collect())
}

/// VibeSkua `Rest(full)`: waits for the `rest` cooldown, rests, and with `full` waits
/// (20 tries) until HP and MP are full.
pub fn rest(bot: &Bot, full: bool) -> BotResult<()> {
    wait::for_action_cooldown(bot, "rest", 40)?;
    bot.flash.call_game_function("world.rest", vec![])?;
    if full {
        wait::until(
            bot,
            || Ok(health(bot)? >= max_health(bot)? && mana(bot)? >= max_mana(bot)?),
            20,
        )?;
    }
    Ok(())
}

/// `world.setSpawnPoint(cell, pad)`; the current cell/pad when not given.
pub fn set_spawn_point(bot: &Bot, cell: Option<&str>, pad: Option<&str>) -> BotResult<()> {
    let cell = match cell {
        Some(c) => c.to_owned(),
        None => self::cell(bot)?,
    };
    let pad = match pad {
        Some(p) => p.to_owned(),
        None => self::pad(bot)?,
    };
    bot.flash
        .call_game_function("world.setSpawnPoint", vec![cell.into(), pad.into()])
}

/// `walkTo(x, y, speed)` (speed 8 means `world.WALKSPEED`), then `Wait.ForPlayerPosition`.
pub fn walk_to(bot: &Bot, x: i64, y: i64, speed: i64) -> BotResult<()> {
    bot.flash
        .call("walkTo", vec![x.into(), y.into(), speed.into()])?;
    wait::for_player_position(bot, x, y, 10)?;
    Ok(())
}

/// `world.goto(name)`: teleports to a player in the same server.
pub fn goto(bot: &Bot, name: &str) -> BotResult<()> {
    bot.flash
        .call_game_function("world.goto", vec![name.into()])
}

/// Your character's badges from its character page (CoreBots `Badges`: a GET to
/// `account.aq.com/CharPage/Badges?ccid=<CharID>`). Empty without a character.
pub fn web_badges(bot: &Bot) -> BotResult<Vec<Badge>> {
    let id: i64 = bot.flash.get_or("world.myAvatar.objData.CharID", 0)?;
    if id <= 0 {
        return Ok(Vec::new());
    }
    let url = format!("https://account.aq.com/CharPage/Badges?ccid={id}");
    let failed = |e: reqwest::Error| BotError::Other(format!("couldn't read the badges: {e}"));
    let text = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0")
        .build()
        .map_err(failed)?
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(reqwest::blocking::Response::text)
        .map_err(failed)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let f = &bot.flash;
    match name {
        "loggedIn" => ok(logged_in(bot)?),
        "playing" => ok(playing(bot)?),
        "alive" => ok(alive(bot)?),
        "inCombat" => ok(in_combat(bot)?),
        "loaded" => ok(loaded(bot)?),
        "state" => ok(state(bot)?),
        "afk" => ok(f.get_or::<bool>("world.myAvatar.dataLeaf.afk", false)?),
        "username" => ok(username(bot)?),
        "serverIp" => ok(server_ip(bot)?),
        "kicked" => ok(super::servers::kicked(bot)?),
        "cell" => ok(cell(bot)?),
        "pad" => ok(pad(bot)?),
        "id" => ok(f.get_or::<i64>("world.myAvatar.uid", 0)?),
        "level" => ok(f.get_or::<i64>("world.myAvatar.dataLeaf.intLevel", 0)?),
        "health" => ok(health(bot)?),
        "maxHealth" => ok(max_health(bot)?),
        "mana" => ok(mana(bot)?),
        "maxMana" => ok(max_mana(bot)?),
        "gold" => ok(f.get_or::<i64>("world.myAvatar.objData.intGold", 0)?),
        "isMember" => ok(f.get_or::<i64>("world.myAvatar.objData.iUpgDays", -1)? >= 0),
        "webBadges" => ok(web_badges(bot)?),
        "currentClass" => {
            if !playing(bot)? {
                return ok(Value::Null);
            }
            ok(super::inventory::current_class(bot)?)
        }
        "currentClassRank" => ok(f.get_or::<i64>("world.myAvatar.objData.iRank", 0)?),
        "target" => ok(target(bot)?),
        "hasTarget" => ok(has_target(bot)?),
        "skills" => ok(skills(bot)?),
        "x" => ok(x(bot)?),
        "y" => ok(y(bot)?),
        "walkSpeed" => ok(walk_speed(bot)?),
        "setWalkSpeed" => ok(set_walk_speed(bot, args.i64(0)?)?),
        "rest" => ok(rest(bot, args.bool_or(0, false))?),
        "setSpawnPoint" => ok(set_spawn_point(bot, args.opt_str(0), args.opt_str(1))?),
        "walkTo" => ok(walk_to(bot, args.i64(0)?, args.i64(1)?, args.i64_or(2, 8))?),
        "goto" => ok(goto(bot, args.str(0)?)?),
        _ => Err(BotError::UnknownMethod(format!("player.{name}"))),
    }
}
