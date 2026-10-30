//! `bot.player` (VibeSkua `IScriptPlayer`, `ScriptPlayer.cs`).

use super::{Args, ok, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::models::{Badge, Monster, lenient_bool, lenient_i64, lenient_string};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

const AVATAR: &str = "world.myAvatar";

/// `isLoggedIn`: the SmartFox client is connected.
pub fn logged_in(bot: &Bot) -> BotResult<bool> {
    bot.flash.call_bool("isLoggedIn", vec![])
}

/// `dataLeaf.intState`: 0 dead, 1 idle, 2 in combat (0 without an avatar).
pub fn state(bot: &Bot) -> BotResult<i64> {
    if bot.flash.is_null(AVATAR)? {
        return Ok(0);
    }
    bot.flash.get_or("world.myAvatar.dataLeaf.intState", 0)
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
    let items: Option<Vec<Value>> = bot.flash.get_opt("world.myAvatar.items")?;
    if items.is_none_or(|i| i.is_empty()) {
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
    let value: Value = serde_json::from_str(raw.trim()).unwrap_or(Value::Null);
    if value.as_object().is_none_or(|o| o.is_empty()) {
        return Ok(None);
    }
    Ok(serde_json::from_value(value).ok())
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
