//! AntiLag while following (VibeSkua ButlerV5 `ApplyAntiLag`): the bundle CoreBots' AntiLag
//! turns on (Lag Killer, the Lite client's animation cuts, hidden monsters). The user's nine
//! settings are saved first and put back exactly when the follow ends.

use super::{Level, trace};
use crate::Bot;
use crate::BotResult;
use crate::api::lite;
use serde_json::Value;

/// The Lite settings (`litePreference.data`): Freeze Monster Position, Disable Monster, Damage
/// Strobe, Self, Weapon, Skill and Aura Animations.
pub const LITE_KEYS: [&str; 7] = [
    "bFreezeMons",
    "bDisMonAnim",
    "bDisDmgStrobe",
    "bDisSelfMAnim",
    "bDisWepAnim",
    "bDisSkillAnim",
    "bDisAuraAnim",
];

/// True while the monsters are hidden (`world.toggleMonsters` shows the red X).
const MONSTERS_HIDDEN: &str = "ui.monsterIcon.redX.visible";

/// The nine settings before the follow turned AntiLag on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub lag_killer: bool,
    /// In [`LITE_KEYS`]' order.
    pub lite: [bool; 7],
    pub monsters_hidden: bool,
}

/// Reads the nine settings.
pub fn capture(bot: &Bot) -> BotResult<Snapshot> {
    let mut lite = [false; 7];
    for (value, key) in lite.iter_mut().zip(LITE_KEYS) {
        *value = lite::get(bot, key)?
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    }
    Ok(Snapshot {
        lag_killer: bot.opts().lag_killer,
        lite,
        monsters_hidden: monsters_hidden(bot)?,
    })
}

/// Turns the whole bundle on or off.
pub fn set(bot: &Bot, on: bool) -> BotResult<()> {
    apply(
        bot,
        &Snapshot {
            lag_killer: on,
            lite: [on; 7],
            monsters_hidden: on,
        },
    )
}

/// Puts the settings back as they were; without a snapshot (it couldn't be read), turns them
/// all off.
pub fn restore(bot: &Bot, snapshot: Option<&Snapshot>) {
    let result = match snapshot {
        Some(s) => apply(bot, s),
        None => set(bot, false),
    };
    if let Err(e) = result {
        trace(
            bot,
            "Follower",
            Level::Error,
            format!("AntiLag restore failed: {e}"),
        );
    }
}

fn apply(bot: &Bot, s: &Snapshot) -> BotResult<()> {
    // The options timer applies Lag Killer to the game.
    bot.options.write().lag_killer = s.lag_killer;
    for (key, on) in LITE_KEYS.iter().zip(s.lite) {
        lite::set(bot, key, on)?;
    }
    // A toggle: only when it differs.
    if monsters_hidden(bot)? != s.monsters_hidden {
        bot.flash
            .call_game_function("world.toggleMonsters", Vec::new())?;
    }
    Ok(())
}

fn monsters_hidden(bot: &Bot) -> BotResult<bool> {
    Ok(bot
        .flash
        .get_opt::<Value>(MONSTERS_HIDDEN)?
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}
