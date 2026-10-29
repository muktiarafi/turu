//! `bot.lite`: the AQW Lite client's settings, `litePreference.data` (VibeSkua `ScriptLite`).

use super::{Args, ok};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::host::FlashArg;
use serde_json::Value;
use std::sync::Arc;

const DATA: &str = "litePreference.data";

/// `litePreference.data.<key>` (VibeSkua `Lite.Get<T>`), e.g. `bUntargetSelf`.
pub fn get(bot: &Bot, key: &str) -> BotResult<Option<Value>> {
    bot.flash.get_opt(&format!("{DATA}.{key}"))
}

/// Writes `litePreference.data.<key>` (VibeSkua `Lite.Set<T>`).
pub fn set(bot: &Bot, key: &str, value: impl Into<FlashArg>) -> BotResult<()> {
    bot.flash.set(&format!("{DATA}.{key}"), value)
}

/// Writes `litePreference.data.dOptions["<key>"]`, e.g. `disRed`, `termsAgree`.
pub fn set_option(bot: &Bot, key: &str, value: impl Into<FlashArg>) -> BotResult<()> {
    bot.flash.set_key(&format!("{DATA}.dOptions"), key, value)
}

/// `litePreference.data.dOptions["<key>"]`.
pub fn get_option(bot: &Bot, key: &str) -> BotResult<Option<Value>> {
    bot.flash.get_key(&format!("{DATA}.dOptions"), key)
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "get" => ok(get(bot, args.str(0)?)?),
        "set" => ok(set(bot, args.str(0)?, args.value(1).clone())?),
        "getOption" => ok(get_option(bot, args.str(0)?)?),
        "setOption" => ok(set_option(bot, args.str(0)?, args.value(1).clone())?),
        _ => Err(BotError::UnknownMethod(format!("lite.{name}"))),
    }
}
