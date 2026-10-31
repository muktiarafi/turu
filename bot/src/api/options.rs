//! `bot.options` and applying them in game (VibeSkua `ScriptOption`, `CheckOptions`).
//!
//! VibeSkua pushes most options into the game from their setters; Turu's options are plain
//! data, so [`apply`] (every 250 ms while playing) sends what changed since it last ran,
//! plus the options VibeSkua's `CheckOptions` re-applies on every tick.

use super::{Args, lite, monsters, ok, player, send};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::host::FlashArg;
use serde_json::Value;
use std::sync::Arc;

/// What [`apply`] last pushed into the game (`None`: nothing yet this login).
#[derive(Debug, Default, Clone)]
pub struct Applied {
    lag_killer: Option<bool>,
    hide_players: Option<bool>,
    disable_fx: Option<bool>,
    disable_collisions: Option<bool>,
    disable_death_ads: Option<bool>,
    show_fps: Option<bool>,
    set_fps: Option<i32>,
    headless: Option<bool>,
    /// 2 FPS and LOW quality: Headless Mode, or a hidden tab.
    slowed: Option<bool>,
    walk_speed: Option<i32>,
    custom_name: Option<String>,
    custom_guild: Option<String>,
}

/// True when a toggle must be sent: it changed, and "off" isn't sent before anything was
/// turned on (the game starts with everything off).
fn changed(applied: &mut Option<bool>, value: bool) -> bool {
    let send = match *applied {
        Some(old) => old != value,
        None => value,
    };
    *applied = Some(value);
    send
}

fn module(bot: &Bot, name: &str, enable: bool) -> BotResult<()> {
    let callback = if enable { "modEnable" } else { "modDisable" };
    bot.flash.call(callback, vec![name.into()])?;
    Ok(())
}

/// Writes the name tag / portrait name (VibeSkua `CustomName`).
fn write_name(bot: &Bot, name: &str) -> BotResult<()> {
    for path in [
        "world.myAvatar.objData.strUsername",
        "world.rootClass.ui.mcPortrait.strName.text",
        "world.myAvatar.pMC.pname.ti.text",
    ] {
        bot.flash.set(path, name)?;
    }
    Ok(())
}

/// Runs every 250 ms while playing (VibeSkua `ScriptInterface.CheckOptions` and the
/// options' setters).
pub fn apply(bot: &Bot) -> BotResult<()> {
    // Copies: neither lock may be held across Flash calls.
    let o = bot.opts().clone();
    let mut applied = bot.state.lock().options_applied.clone();
    let result = apply_with(bot, &o, &mut applied);
    bot.state.lock().options_applied = applied;
    result
}

fn apply_with(bot: &Bot, o: &crate::options::BotOptions, a: &mut Applied) -> BotResult<()> {
    // Re-applied every tick (CheckOptions); turning it off restores the world once.
    if o.lag_killer {
        bot.flash.call("killLag", vec![true.into()])?;
        a.lag_killer = Some(true);
    } else if a.lag_killer == Some(true) {
        bot.flash.call("killLag", vec![false.into()])?;
        a.lag_killer = Some(false);
    }

    if !player::playing(bot)? {
        return Ok(());
    }

    // Setters: only on change.
    for (name, value, slot) in [
        ("HidePlayers", o.hide_players, &mut a.hide_players),
        ("DisableFX", o.disable_fx, &mut a.disable_fx),
        (
            "DisableCollisions",
            o.disable_collisions,
            &mut a.disable_collisions,
        ),
    ] {
        if changed(slot, value) {
            module(bot, name, value)?;
        }
    }
    if changed(&mut a.disable_death_ads, o.disable_death_ads) {
        bot.flash
            .call("disableDeathAd", vec![o.disable_death_ads.into()])?;
    }
    if changed(&mut a.show_fps, o.show_fps) {
        bot.flash.set("ui.mcFPS.visible", o.show_fps)?;
    }
    // VibeSkua `OnHeadlessModeChanged` and the tabbed host's `WM_SKUA_THROTTLE` (a hidden tab,
    // unless Headless Mode already slows it): 2 FPS and LOW quality. Leaving both restores the
    // rate and quality the game had (VibeSkua's Headless Mode: Set FPS), unless Set FPS forces
    // a rate.
    let headless_changed = changed(&mut a.headless, o.headless_mode);
    let slowed = o.headless_mode || bot.state.lock().hidden_tab;
    if changed(&mut a.slowed, slowed) {
        if slowed {
            if bot.state.lock().before_slowed.is_none() {
                let fps = bot.flash.get::<f64>("stage.frameRate")?.round() as i32;
                let quality = bot.flash.get_or::<String>("stage.quality", "HIGH".into())?;
                bot.state.lock().before_slowed = Some((fps, quality));
            }
            bot.flash.set("stage.frameRate", 2)?;
            bot.flash.set("stage.quality", "LOW")?;
        } else {
            let before = bot.state.lock().before_slowed.take();
            let fps = match &before {
                _ if o.set_fps > 0 => o.set_fps,
                Some((fps, _)) if *fps > 2 => *fps,
                _ => crate::settings::GAME_FRAME_RATE,
            };
            let quality = before
                .map(|(_, q)| q)
                .filter(|q| !q.is_empty() && !q.eq_ignore_ascii_case("low"))
                .unwrap_or_else(|| "HIGH".into());
            bot.flash.set("stage.frameRate", fps)?;
            bot.flash.set("stage.quality", quality.as_str())?;
        }
        a.set_fps = Some(o.set_fps);
    }
    if headless_changed {
        let volume = if o.headless_mode { 0 } else { 1 };
        bot.flash
            .set("world.rootClass.mixer.soundTransform.volume", volume)?;
    }
    if !slowed && a.set_fps != Some(o.set_fps) {
        // Set FPS 0 leaves the frame rate to the game's own setting.
        if o.set_fps > 0 {
            bot.flash.set("stage.frameRate", o.set_fps)?;
        }
        a.set_fps = Some(o.set_fps);
    }
    if a.custom_name.as_deref() != Some(o.custom_name.as_str()) {
        if !o.custom_name.is_empty() {
            write_name(bot, &o.custom_name)?;
        } else if a.custom_name.as_deref().is_some_and(|n| !n.is_empty()) {
            write_name(bot, &player::username(bot)?)?;
        }
        a.custom_name = Some(o.custom_name.clone());
    }
    if a.custom_guild.as_deref() != Some(o.custom_guild.as_str()) {
        if !o.custom_guild.is_empty() {
            bot.flash
                .set("world.myAvatar.pMC.pname.tg.text", o.custom_guild.as_str())?;
        } else if a.custom_guild.as_deref().is_some_and(|n| !n.is_empty()) {
            let guild: String = bot
                .flash
                .get_or("world.myAvatar.objData.guild.Name", String::new())?;
            bot.flash.set("world.myAvatar.pMC.pname.tg.text", guild)?;
        }
        a.custom_guild = Some(o.custom_guild.clone());
    }

    // CheckOptions: every tick while on.
    if o.magnetise {
        // VibeSkua calls "magnetise" here, which isn't registered; the callback is "magnetize".
        bot.flash.call("magnetize", vec![])?;
    }
    if o.infinite_range {
        bot.flash.call("infiniteRange", vec![])?;
    }
    if o.aggro_monsters {
        bot.flash.call_game_function("world.aggroAllMon", vec![])?;
    }
    if o.aggro_all_monsters {
        let room = super::map::room_id(bot)?;
        let ids: Vec<String> = monsters::map_monsters(bot)?
            .iter()
            .map(|m| m.map_id.to_string())
            .collect();
        send::packet(
            bot,
            &format!("%xt%zm%aggroMon%{room}%{}%", ids.join("%")),
            "String",
        )?;
    }
    if o.skip_cutscenes {
        bot.flash.call("skipCutscenes", vec![])?;
    }
    if o.walk_speed != 8 {
        player::set_walk_speed(bot, o.walk_speed.into())?;
        a.walk_speed = Some(o.walk_speed);
    } else if a.walk_speed.is_some_and(|s| s != 8) {
        player::set_walk_speed(bot, 8)?;
        a.walk_speed = Some(8);
    }
    for key in ["bUntargetSelf", "bUntargetDead"] {
        if lite::get(bot, key)?.and_then(|v| v.as_bool()) != Some(true) {
            lite::set(bot, key, FlashArg::Bool(true))?;
        }
    }
    Ok(())
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "get" => ok(bot.opts().get(args.str(0)?)),
        "set" => {
            bot.options
                .write()
                .set(args.str(0)?, args.value(1).clone())
                .map_err(BotError::InvalidArgument)?;
            ok(())
        }
        _ => Err(BotError::UnknownMethod(format!("options.{name}"))),
    }
}
