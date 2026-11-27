//! `bot.options` and applying them in game (VibeSkua `ScriptOption`, `CheckOptions`).
//!
//! VibeSkua pushes most options into the game from their setters; Turu's options are plain
//! data, so [`apply`] (every 250 ms while playing) sends what changed since it last ran,
//! plus the options VibeSkua's `CheckOptions` re-applies on every tick.

use super::{Args, ok, player};
use crate::Bot;
use crate::error::{BotError, BotResult};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

/// Turu: the frame rate of a hidden tab (VibeSkua's tabbed host: 2). The app raises it for
/// Grid View's tiles (and `TURU_GRID_TEST`), and the next options pass applies it.
static HIDDEN_TAB_FPS: AtomicI32 = AtomicI32::new(2);

pub fn set_hidden_tab_fps(fps: i32) {
    HIDDEN_TAB_FPS.store(fps.max(1), Ordering::Relaxed);
}

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
    /// The frame rate a slowed client runs at (2 for Headless Mode, [`HIDDEN_TAB_FPS`] for a
    /// hidden tab), `None` when it isn't slowed.
    slowed: Option<Option<i32>>,
    /// LOW quality: Headless Mode only.
    low_quality: Option<bool>,
    /// The walk speed turu.swf keeps (`keepWalkSpeed`).
    walk_speed: Option<i32>,
    custom_name: Option<String>,
    custom_guild: Option<String>,
    /// turu.swf's Streamer Mode module, which keeps it applied itself.
    streamer: Option<bool>,
    /// turu.swf's Skip Cutscenes module, which it keeps applied itself.
    skip_cutscenes: Option<bool>,
    /// turu.swf's Infinite Range and Untarget Lite modules, kept applied by the SWF itself.
    infinite_range: Option<bool>,
    untarget_lite: Option<bool>,
    /// turu.swf's Magnetise, Aggro Monsters and Aggro All Monsters modules.
    magnetise: Option<bool>,
    aggro_monsters: Option<bool>,
    aggro_all_monsters: Option<bool>,
}

impl Applied {
    /// Headless Mode is on in game: the window shows a placeholder instead of the game.
    pub fn headless(&self) -> bool {
        self.headless == Some(true)
    }

    /// Turns the Lag Killer module off on the next tick even when it wasn't on, which shows the
    /// world again (VibeSkua's `ScriptStopped` toggles Lag Killer on and off for this).
    pub fn restore_world(&mut self) {
        self.lag_killer = Some(true);
    }
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

fn module(bot: &Bot, name: &'static str, enable: bool) -> BotResult<()> {
    let callback = if enable { "modEnable" } else { "modDisable" };
    bot.flash.call(callback, vec![name.into()])?;
    let mut state = bot.state.lock();
    if enable {
        state.swf_modules.insert(name);
    } else {
        state.swf_modules.remove(name);
    }
    Ok(())
}

/// A turu.swf module switched as [`changed`] says, and also off at the first pass of a login
/// (`applied` still `None`) when the SWF, which outlives a login, still has it on. Answers
/// whether it was sent.
fn switch(
    bot: &Bot,
    name: &'static str,
    value: bool,
    applied: &mut Option<bool>,
) -> BotResult<bool> {
    let stale = applied.is_none() && !value && bot.state.lock().swf_modules.contains(name);
    if changed(applied, value) || stale {
        module(bot, name, value)?;
        return Ok(true);
    }
    Ok(false)
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

/// Custom Name and Custom Guild, on change.
fn apply_custom_names(bot: &Bot, o: &crate::options::BotOptions, a: &mut Applied) -> BotResult<()> {
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

/// Out of game, once the game has loaded (the login screen): Streamer Mode alone, whose module
/// masks the login screen's fields there. Turu: VibeSkua's Streamer Mode works only in game.
pub fn apply_out_of_game(bot: &Bot) -> BotResult<()> {
    let on = bot.opts().streamer_mode;
    let mut applied = bot.state.lock().options_applied.clone();
    let result = switch(bot, "StreamerMode", on, &mut applied.streamer);
    bot.state.lock().options_applied = applied;
    result.map(|_| ())
}

fn apply_with(bot: &Bot, o: &crate::options::BotOptions, a: &mut Applied) -> BotResult<()> {
    // turu.swf keeps the world hidden while it's on (VibeSkua's CheckOptions called `killLag`
    // on every pass); turning it off shows the world again.
    switch(bot, "LagKiller", o.lag_killer, &mut a.lag_killer)?;

    if !player::playing(bot)? {
        return Ok(());
    }

    // Setters: only on change. Streamer Mode hides the other players too, and turned off
    // gives back Hide Players as it was (VibeSkua `_originalHidePlayers`).
    for (name, value, slot) in [
        (
            "HidePlayers",
            o.hide_players || o.streamer_mode,
            &mut a.hide_players,
        ),
        ("DisableFX", o.disable_fx, &mut a.disable_fx),
        (
            "DisableCollisions",
            o.disable_collisions,
            &mut a.disable_collisions,
        ),
    ] {
        switch(bot, name, value, slot)?;
    }
    if changed(&mut a.disable_death_ads, o.disable_death_ads) {
        bot.flash
            .call("disableDeathAd", vec![o.disable_death_ads.into()])?;
    }
    if changed(&mut a.show_fps, o.show_fps) {
        bot.flash.set("ui.mcFPS.visible", o.show_fps)?;
    }
    // VibeSkua `OnHeadlessModeChanged` and the tabbed host's `WM_SKUA_THROTTLE` (a hidden tab,
    // unless Headless Mode already slows it): 2 FPS. Leaving both restores the rate the game
    // had (VibeSkua's Headless Mode: Set FPS), unless Set FPS forces a rate.
    let headless_changed = changed(&mut a.headless, o.headless_mode);
    let slow_fps = if o.headless_mode {
        Some(2)
    } else if bot.state.lock().hidden_tab {
        Some(HIDDEN_TAB_FPS.load(Ordering::Relaxed))
    } else {
        None
    };
    let slowed = slow_fps.is_some();
    // As `changed`, but a change of the slowed rate counts too (Grid View in or out).
    let slow_changed = match a.slowed {
        Some(old) => old != slow_fps,
        None => slowed,
    };
    a.slowed = Some(slow_fps);
    if slow_changed {
        if let Some(fps) = slow_fps {
            if bot.state.lock().before_slowed.is_none() {
                let fps = bot.flash.get::<f64>("stage.frameRate")?.round() as i32;
                bot.state.lock().before_slowed = Some(fps);
            }
            bot.flash.set("stage.frameRate", fps)?;
        } else {
            let fps = match bot.state.lock().before_slowed.take() {
                _ if o.set_fps > 0 => o.set_fps,
                Some(fps) if fps > 2 => fps,
                _ => bot.settings.read().start_rate(),
            };
            bot.flash.set("stage.frameRate", fps)?;
        }
        a.set_fps = Some(o.set_fps);
    }
    // Headless Mode also drops to LOW quality, as VibeSkua's does. A hidden tab doesn't (unlike
    // VibeSkua's tabbed host, whose hidden clients still draw): Turu doesn't draw it, so LOW
    // would save nothing, and HIGH again re-tessellates every shape, a hitch on every switch.
    if changed(&mut a.low_quality, o.headless_mode) {
        if o.headless_mode {
            if bot.state.lock().before_low_quality.is_none() {
                let quality = bot.flash.get_or::<String>("stage.quality", "HIGH".into())?;
                bot.state.lock().before_low_quality = Some(quality);
            }
            bot.flash.set("stage.quality", "LOW")?;
        } else {
            let quality = bot
                .state
                .lock()
                .before_low_quality
                .take()
                .filter(|q| !q.is_empty() && !q.eq_ignore_ascii_case("low"))
                .unwrap_or_else(|| "HIGH".into());
            bot.flash.set("stage.quality", quality.as_str())?;
        }
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
    // Streamer Mode: turu.swf's module covers the names on every frame (VibeSkua's 500 ms loop)
    // and gives them back when it's turned off; then Custom Name and Guild apply again.
    if switch(bot, "StreamerMode", o.streamer_mode, &mut a.streamer)? && !o.streamer_mode {
        a.custom_name = None;
        a.custom_guild = None;
    }
    if !o.streamer_mode {
        apply_custom_names(bot, o, a)?;
    }

    // CheckOptions' per-tick options: turu.swf's modules keep them applied while you're logged in
    // and alive, the aggro ones every 250 ms as this pass did (VibeSkua calls them on every pass;
    // its "magnetise" isn't even registered).
    for (name, value, slot) in [
        ("Magnetise", o.magnetise, &mut a.magnetise),
        ("AggroMonsters", o.aggro_monsters, &mut a.aggro_monsters),
        (
            "AggroAllMonsters",
            o.aggro_all_monsters,
            &mut a.aggro_all_monsters,
        ),
    ] {
        switch(bot, name, value, slot)?;
    }
    // turu.swf keeps it applied (every 100 ms, as the game resets ranges on a class change);
    // VibeSkua's CheckOptions called `infiniteRange` on every pass.
    switch(
        bot,
        "InfiniteRange",
        o.infinite_range,
        &mut a.infinite_range,
    )?;
    // turu.swf keeps it applied every 100 ms: it takes a cutscene's overlay off, shows the
    // world again unless Lag Killer hides it, and cancels a cutscene the game is waiting to
    // show (lichwar's intro), which held back the cell's monsters (VibeSkua's CheckOptions
    // called `skipCutscenes` on every pass).
    switch(
        bot,
        "SkipCutscenes",
        o.skip_cutscenes,
        &mut a.skip_cutscenes,
    )?;
    // Walk Speed: turu.swf keeps `world.WALKSPEED` at it (the game sets it again on a new map);
    // 8, the game's own, leaves it alone (and sets 8 back after another).
    // Sent at a login's first pass too while the SWF still keeps another speed.
    let kept = bot.state.lock().swf_walk_speed;
    let speed_sent = match a.walk_speed {
        Some(old) => old != o.walk_speed,
        None => o.walk_speed != 8 || kept != 0,
    };
    if speed_sent {
        bot.flash
            .call("keepWalkSpeed", vec![i64::from(o.walk_speed).into()])?;
        bot.state.lock().swf_walk_speed = if o.walk_speed == 8 { 0 } else { o.walk_speed };
    }
    a.walk_speed = Some(o.walk_speed);
    // Lite's untarget-self and untarget-dead, always on (VibeSkua's CheckOptions sets them on
    // every pass): turu.swf keeps them so.
    switch(bot, "UntargetLite", true, &mut a.untarget_lite)?;
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
