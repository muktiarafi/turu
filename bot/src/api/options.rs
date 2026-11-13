//! `bot.options` and applying them in game (VibeSkua `ScriptOption`, `CheckOptions`).
//!
//! VibeSkua pushes most options into the game from their setters; Turu's options are plain
//! data, so [`apply`] (every 250 ms while playing) sends what changed since it last ran,
//! plus the options VibeSkua's `CheckOptions` re-applies on every tick.

use super::{Args, lite, monsters, ok, player, send};
use crate::Bot;
use crate::control::{Mark, Topics};
use crate::error::{BotError, BotResult};
use crate::host::FlashArg;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

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
    walk_speed: Option<i32>,
    custom_name: Option<String>,
    custom_guild: Option<String>,
    streamer: Option<bool>,
    /// Turu: the house sign's own text (`world.map.txtHouse`, "Name's House") while Streamer
    /// Mode covers it, to write back when it's turned off.
    house_title: Option<String>,
    /// When Skip Cutscenes last looked, and the events then: it looks again when turu.swf says
    /// a cutscene came or went, else every [`CUTSCENE_CHECK`].
    cutscene_check: Option<(Instant, Mark)>,
}

/// How often Skip Cutscenes looks without a `cutscene` event (M5 of HOST_SWF_PROPOSAL.md; it
/// was every options tick, 250 ms). The cutscene a map is still waiting to show has no event.
const CUTSCENE_CHECK: Duration = Duration::from_secs(1);

impl Applied {
    /// Headless Mode is on in game: the window shows a placeholder instead of the game.
    pub fn headless(&self) -> bool {
        self.headless == Some(true)
    }

    /// Sends `killLag(false)` on the next tick even when Lag Killer wasn't on, which shows the
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

/// The name tag and portrait name while Streamer Mode is on (VibeSkua: "VibeSkuaUser").
pub const STREAMER_NAME: &str = "TuruUser";
/// The guild tag and target portrait name while Streamer Mode is on.
const STREAMER_GUILD: &str = "Hidden";

/// The house sign while Streamer Mode is on, in place of "Name's House".
const STREAMER_HOUSE: &str = "TuruUser's House";
/// The sign over a house map (`house_fla`), with its owner's name.
const HOUSE_TITLE: &str = "world.map.txtHouse.text";

/// What Streamer Mode hides: the gold, coins and chat (VibeSkua tries every spelling the
/// interface has had), and, Turu's, the gold as the interface has it now (`mcGold`, its icon
/// and `strGold`) and the portrait's name. The game rewrites the portrait's name whenever the
/// avatar's bars change (in combat, about one frame in eight showed it under the 250 ms
/// rewrite), but never its visibility, so it is hidden rather than rewritten.
const STREAMER_HIDDEN: [&str; 10] = [
    "world.rootClass.ui.mcInterface.mcGold.visible",
    "world.rootClass.ui.mcPortrait.strName.visible",
    "world.rootClass.ui.mcInterface.teGold.visible",
    "world.rootClass.ui.mcInterface.teCoins.visible",
    "world.rootClass.ui.mcInterface.strGold.visible",
    "world.rootClass.ui.mcInterface.strCoins.visible",
    "world.rootClass.ui.mcInterface.txtGold.visible",
    "world.rootClass.ui.mcInterface.txtCoins.visible",
    "world.rootClass.ui.mcInterface.t1.visible",
    "world.rootClass.ui.mcInterface.te.visible",
];

/// VibeSkua `ApplyStreamerMode`'s loop: the drawn names only (never `objData`, which shops and
/// the inventory read), the gold, coins and chat hidden, and the area title without the room
/// number. Each write may fail on its own, as VibeSkua ignores them.
fn write_streamer(bot: &Bot, a: &mut Applied) {
    let f = &bot.flash;
    let _ = f.set("world.myAvatar.pMC.pname.ti.text", STREAMER_NAME);
    let _ = f.set("world.myAvatar.pMC.pname.tg.text", STREAMER_GUILD);
    let _ = f.set("world.rootClass.ui.mcPortrait.strName.text", STREAMER_NAME);
    let _ = f.set(
        "world.rootClass.ui.mcPortraitTarget.strName.text",
        STREAMER_GUILD,
    );
    for path in STREAMER_HIDDEN {
        let _ = f.set(path, false);
    }
    let map: String = f
        .get_or("world.strMapName", String::new())
        .unwrap_or_default();
    if !map.is_empty() && map != "null" {
        let mut chars = map.chars();
        let title: String = chars
            .next()
            .into_iter()
            .flat_map(char::to_uppercase)
            .chain(chars.flat_map(char::to_lowercase))
            .collect();
        let _ = f.set("ui.mcInterface.areaList.title.t1.text", title);
    }
    // Turu: the sign in a house names its owner. Kept to write back unless it's already ours.
    if let Ok(Some(title)) = f.get_opt::<String>(HOUSE_TITLE)
        && title != STREAMER_HOUSE
    {
        a.house_title = Some(title);
        let _ = f.set(HOUSE_TITLE, STREAMER_HOUSE);
    }
}

/// Streamer Mode turned off: the gold, coins and chat shown again, and the name and guild
/// written back from `objData`, which Streamer Mode leaves alone. VibeSkua only calls
/// `world.setUserData`, which leaves the name tag as it is. As in [`write_streamer`], one failed
/// write doesn't stop the others, and it isn't tried again.
fn clear_streamer(bot: &Bot, a: &mut Applied) {
    let f = &bot.flash;
    if let Some(title) = a.house_title.take()
        && f.get_opt::<String>(HOUSE_TITLE).ok().flatten().as_deref() == Some(STREAMER_HOUSE)
    {
        let _ = f.set(HOUSE_TITLE, title);
    }
    for path in STREAMER_HIDDEN {
        let _ = f.set(path, true);
    }
    let name: String = f
        .get_or("world.myAvatar.objData.strUsername", String::new())
        .unwrap_or_default();
    if !name.is_empty() {
        let _ = f.set("world.myAvatar.pMC.pname.ti.text", name.as_str());
        let _ = f.set("world.rootClass.ui.mcPortrait.strName.text", name.as_str());
    }
    let guild: String = f
        .get_or("world.myAvatar.objData.guild.Name", String::new())
        .unwrap_or_default();
    let _ = f.set("world.myAvatar.pMC.pname.tg.text", guild);
    let _ = f.call_game_function("world.setUserData", vec![]);
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
                _ => crate::settings::GAME_FRAME_RATE,
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
    // Streamer Mode: rewritten on every pass (VibeSkua's loop), over Custom Name and Guild,
    // which apply again once it's off.
    if o.streamer_mode {
        write_streamer(bot, a);
        a.streamer = Some(true);
    } else {
        if a.streamer == Some(true) {
            clear_streamer(bot, a);
            a.custom_name = None;
            a.custom_guild = None;
        }
        a.streamer = Some(false);
        apply_custom_names(bot, o, a)?;
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
    let cutscene_due = a.cutscene_check.is_none_or(|(at, mark)| {
        at.elapsed() >= CUTSCENE_CHECK || bot.control.fired_since(&mark, Topics::CUTSCENE)
    });
    if o.skip_cutscenes && cutscene_due {
        a.cutscene_check = Some((Instant::now(), bot.control.mark()));
        // `skipCutscenes` removes the cutscene but leaves the world as the cutscene left it,
        // hidden: show it again unless Lag Killer hides it (VibeSkua's black screen until Lag
        // Killer is toggled).
        let cutscene = bot.flash.get_or::<i64>("mcExtSWF.numChildren", 0)? > 0;
        bot.flash.call("skipCutscenes", vec![])?;
        if cutscene && !o.lag_killer {
            bot.flash.set("world.visible", true)?;
        }
        cancel_pending_cutscene(bot)?;
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

/// Turu: Skip Cutscenes also cancels the cutscene the game is waiting to show. A map asks for
/// one with `world.cHandle.showCutscene` (lichwar's intro, for one), and while `isPending` the
/// game holds back the monsters of the cells you enter (`World.placeMissingCellMonsters`)
/// until the cutscene's overlay appears, for up to 450 frames (~225 s at a hidden tab's
/// 2 FPS). `skipCutscenes` removes the overlay, often before the game sees it, so the cell's
/// monsters stayed unattackable for that long. `cancel` is what the cutscene's own end
/// (`transfer`) calls. The game's cell settle gave up while the cutscene was pending (and its
/// `scheduleCellSettle` is private), so entering the current cell again places its monsters.
fn cancel_pending_cutscene(bot: &Bot) -> BotResult<()> {
    if bot.flash.is_null("world.cHandle")? {
        return Ok(());
    }
    let pending = bot
        .flash
        .call_game_function_json::<bool>("world.cHandle.isPending", vec![])?;
    if pending != Some(true) {
        return Ok(());
    }
    bot.flash
        .call_game_function("world.cHandle.cancel", vec![])?;
    let frame = bot.flash.get_opt::<String>("world.strFrame")?;
    let pad = bot.flash.get_opt::<String>("world.strPad")?;
    if let (Some(frame), Some(pad)) = (frame, pad) {
        bot.flash.call_game_function(
            "world.moveToCell",
            vec![frame.as_str().into(), pad.as_str().into()],
        )?;
    }
    bot.note("Skip Cutscenes: cancelled the cutscene the game was waiting to show.");
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
