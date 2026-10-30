//! `bot.config`: the values of the loaded script's options (VibeSkua `Bot.Config`). The script
//! declares them (`script::options`), and the Script Options window edits the saved values.
//! `configure` opens that window from the script and waits for it (VibeSkua `Config.Configure()`).

use super::{Args, ok};
use crate::Bot;
use crate::error::{BotError, BotResult};
use crate::script::options::MAIN;
use serde_json::Value;
use std::sync::Arc;

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        // `get(name)` / `get(group, name)`, `set(name, value)` / `set(group, name, value)` and
        // `has(name)` / `has(group, name)`, as VibeSkua's overloads with a category.
        "get" => {
            let (category, name) = category_and_name(&args, 2)?;
            ok(bot
                .scripts
                .option_value(category, name)
                .map_err(BotError::Other)?)
        }
        "set" => {
            let (category, name) = category_and_name(&args, 3)?;
            let value = args.value(if args.is_missing(2) { 1 } else { 2 });
            bot.scripts
                .set_option_value(category, name, value.clone())
                .map_err(BotError::Other)?;
            ok(())
        }
        "has" => {
            let (category, name) = category_and_name(&args, 2)?;
            ok(bot.scripts.has_option(category, name))
        }
        "configure" => {
            bot.scripts.configure(&bot.control)?;
            ok(())
        }
        _ => Err(BotError::UnknownMethod(format!("config.{name}"))),
    }
}

/// The category and name of an option: `(group, name)` when the call has `full` arguments,
/// else `(MAIN, name)`.
fn category_and_name<'a>(args: &Args<'a>, full: usize) -> BotResult<(&'a str, &'a str)> {
    if args.is_missing(full - 1) {
        Ok((MAIN, args.str(0)?))
    } else {
        Ok((args.str(0)?, args.str(1)?))
    }
}
