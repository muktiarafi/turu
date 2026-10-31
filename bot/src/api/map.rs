//! `bot.map` (VibeSkua `IScriptMap`, `ScriptMap.cs`).

use super::{Args, ok, player, send, wait};
use crate::Bot;
use crate::error::{BotError, BotResult};
use serde_json::Value;
use std::sync::Arc;

/// `world.strMapName` (empty without a world).
pub fn name(bot: &Bot) -> BotResult<String> {
    if bot.flash.is_null("world")? {
        return Ok(String::new());
    }
    bot.flash.get_or("world.strMapName", String::new())
}

/// `world.mapLoadInProgress`.
pub fn loading(bot: &Bot) -> BotResult<bool> {
    bot.flash.get_or("world.mapLoadInProgress", false)
}

/// Not loading and the connection screen (`mcConnDetail`) is gone.
pub fn loaded(bot: &Bot) -> BotResult<bool> {
    Ok(!loading(bot)? && bot.flash.is_null("mcConnDetail.stage")?)
}

/// `world.curRoom`.
pub fn room_id(bot: &Bot) -> BotResult<i64> {
    bot.flash.get_or("world.curRoom", 0)
}

/// The cells (frame labels) of the map.
pub fn cells(bot: &Bot) -> BotResult<Vec<String>> {
    bot.flash.select("world.map.currentScene.labels", "name")
}

/// The area name with its room number ("battleon-1234"), from the area list's title.
pub fn full_name(bot: &Bot) -> BotResult<String> {
    if !loaded(bot)? {
        return Ok(String::new());
    }
    let raw: Option<String> = bot.flash.get_opt("ui.mcInterface.areaList.title.t1.text")?;
    if let Some(raw) = raw {
        let raw = raw.replace('"', "");
        if let Some(last) = raw.split_whitespace().last() {
            let area = match last.get(..5) {
                Some(prefix) if prefix.eq_ignore_ascii_case("Area:") => last[5..].trim(),
                _ => last.trim(),
            };
            if !area.is_empty() && !area.eq_ignore_ascii_case("null") {
                return Ok(area.to_owned());
            }
        }
    }
    name(bot)
}

/// The map name `Join` uses: `map` itself, or `map-<number>` when private rooms are on and
/// no room was given, or the room is the `-1e9` placeholder (VibeSkua `_Join`).
pub fn private_room_name(map: &str, private_rooms: bool, private_number: i64) -> String {
    let map_name = map.split('-').next().unwrap_or_default();
    if (private_rooms && !map.contains('-')) || map.contains("-1e9") {
        let number = if private_number > 0 {
            private_number
        } else {
            100000
        };
        format!("{map_name}-{number}")
    } else {
        map.to_owned()
    }
}

/// VibeSkua `JoinPacket`: `%xt%zm%cmd%{room}%tfer%{username}%{map}%{cell}%{pad}%`.
pub fn join_packet(bot: &Bot, map: &str, cell: &str, pad: &str) -> BotResult<()> {
    let room = room_id(bot)?;
    let user = player::username(bot)?;
    send::packet(
        bot,
        &format!("%xt%zm%cmd%{room}%tfer%{user}%{map}%{cell}%{pad}%"),
        "String",
    )
}

/// VibeSkua `Join(map, cell, pad, ignoreCheck, autoCorrect)`: up to `JoinMapTries - 1`
/// transfers, each waiting for the `tfer` cooldown and the map load, then jumping to
/// `cell`/`pad` (or back to the current cell when the map didn't load).
pub fn join(
    bot: &Bot,
    map: &str,
    cell: &str,
    pad: &str,
    ignore_check: bool,
    auto_correct: bool,
) -> BotResult<()> {
    let map_name = map.split('-').next().unwrap_or_default().to_owned();
    bot.state.lock().last_map = Some(map_name.clone());
    if !player::playing(bot)? || !player::loaded(bot)? || (!ignore_check && name(bot)? == map) {
        return Ok(());
    }
    let (private_rooms, private_number, tries, delay) = {
        let o = bot.opts();
        (
            o.private_rooms,
            o.private_number,
            o.join_map_tries,
            o.action_delay,
        )
    };
    let mut map = map.to_owned();
    let mut i = 0;
    loop {
        if name(bot)?.eq_ignore_ascii_case(&map_name) || bot.should_exit() {
            break;
        }
        i += 1;
        if i >= tries {
            break;
        }
        map = private_room_name(&map, private_rooms, private_number.into());
        wait::for_action_cooldown(bot, "tfer", 40)?;
        join_packet(bot, &map, cell, pad)?;
        if !wait::for_map_load(bot, &map, 20)? && !bot.should_exit() {
            let (here, here_pad) = (player::cell(bot)?, player::pad(bot)?);
            jump(bot, &here, &here_pad, auto_correct, false)?;
        } else {
            jump(bot, cell, pad, auto_correct, false)?;
        }
        bot.sleep(delay.max(0) as u64)?;
    }
    Ok(())
}

/// VibeSkua `Jump`: `jumpCorrectRoom(cell, pad, autoCorrect, clientOnly)`, sleeps
/// `ActionDelay`, then waits for the cell change.
pub fn jump(
    bot: &Bot,
    cell: &str,
    pad: &str,
    auto_correct: bool,
    client_only: bool,
) -> BotResult<()> {
    bot.flash.call(
        "jumpCorrectRoom",
        vec![
            cell.into(),
            pad.into(),
            auto_correct.into(),
            client_only.into(),
        ],
    )?;
    let delay = bot.opts().action_delay.max(0) as u64;
    bot.sleep(delay)?;
    wait::for_cell_change(bot, cell)?;
    Ok(())
}

/// `world.reloadCurrentMap`.
pub fn reload(bot: &Bot) -> BotResult<()> {
    bot.flash
        .call_game_function("world.reloadCurrentMap", vec![])
}

/// VibeSkua `GetMapItem`: waits for the `getMapItem` cooldown, sleeps `ActionDelay`, then
/// `world.getMapItem(id)`.
pub fn get_map_item(bot: &Bot, id: i64) -> BotResult<()> {
    wait::for_action_cooldown(bot, "getMapItem", 40)?;
    let delay = bot.opts().action_delay.max(0) as u64;
    bot.sleep(delay)?;
    bot.flash
        .call_game_function("world.getMapItem", vec![id.into()])
}

/// The players in your cell, you included (VibeSkua `Map.CellPlayers`, by name): each of
/// `world.areaUsers` whose `world.uoTree` entry is in your `strFrame`.
pub fn cell_players(bot: &Bot) -> BotResult<Vec<String>> {
    let f = &bot.flash;
    let mine = player::cell(bot)?;
    let names = f
        .get_opt::<Vec<String>>("world.areaUsers")?
        .unwrap_or_default();
    let mut out = Vec::new();
    for name in names {
        let info: Option<Value> = f.get_key("world.uoTree", &name.to_lowercase())?;
        let cell = info
            .as_ref()
            .and_then(|i| i.get("strFrame"))
            .and_then(Value::as_str);
        if cell.is_some_and(|c| c.eq_ignore_ascii_case(&mine)) {
            out.push(name);
        }
    }
    Ok(out)
}

/// Another player in your room (VibeSkua `Map.GetPlayer`): their `world.uoTree` entry, with
/// VibeSkua `PlayerInfo`'s names; `None` when they aren't in the room. Tries three times, 50 ms
/// apart, as VibeSkua does.
pub fn get_player(bot: &Bot, username: &str) -> BotResult<Option<Value>> {
    let key = username.to_lowercase();
    for attempt in 0..3 {
        let info: Option<Value> = bot.flash.get_key("world.uoTree", &key)?;
        if let Some(info) = info.filter(Value::is_object) {
            let text = |k: &str| info.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
            let num = |k: &str| {
                info.get(k)
                    .and_then(|v| {
                        v.as_f64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })
                    .unwrap_or(0.0)
            };
            return Ok(Some(serde_json::json!({
                "name": text("uoName"),
                "hp": num("intHP"),
                "maxHp": num("intHPMax"),
                "mp": num("intMP"),
                "afk": info.get("afk").and_then(Value::as_bool).unwrap_or(false),
                "entId": num("entID"),
                "level": num("intLevel"),
                "cell": text("strFrame"),
                "pad": text("strPad"),
                "x": num("tx"),
                "y": num("ty"),
                "state": num("intState"),
            })));
        }
        if attempt < 2 {
            bot.sleep(50)?;
        }
    }
    Ok(None)
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    let f = &bot.flash;
    match name {
        "name" => ok(self::name(bot)?),
        "fullName" => ok(full_name(bot)?),
        "roomId" => ok(room_id(bot)?),
        "loading" => ok(loading(bot)?),
        "loaded" => ok(loaded(bot)?),
        "cells" => ok(cells(bot)?),
        "playerNames" => ok(f
            .get_opt::<Vec<String>>("world.areaUsers")?
            .unwrap_or_default()),
        "playerCount" => ok(f.get_or::<i64>("world.areaUsers.length", 0)?),
        "cellPlayers" => ok(cell_players(bot)?),
        "getPlayer" => ok(get_player(bot, args.str(0)?)?),
        "filePath" => ok(bot.state.lock().map_file_path.clone()),
        "lastMap" => ok(bot.state.lock().last_map.clone()),
        "join" => ok(join(
            bot,
            args.str(0)?,
            args.str_or(1, "Enter"),
            args.str_or(2, "Spawn"),
            args.bool_or(3, false),
            args.bool_or(4, true),
        )?),
        "jump" => ok(jump(
            bot,
            args.str(0)?,
            args.str_or(1, "Left"),
            args.bool_or(2, true),
            args.bool_or(3, false),
        )?),
        "reload" => ok(reload(bot)?),
        "getMapItem" => ok(get_map_item(bot, args.i64(0)?)?),
        _ => Err(BotError::UnknownMethod(format!("map.{name}"))),
    }
}
