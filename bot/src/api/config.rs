//! `bot.config`: the values of the loaded script's options (VibeSkua `Bot.Config`). The script
//! declares them (`script::options`), and the Script Options window edits the saved values.
//! `configure` opens that window from the script and waits for it (VibeSkua `Config.Configure()`).

use super::{Args, ok};
use crate::Bot;
use crate::error::{BotError, BotResult};
use serde_json::Value;
use std::sync::Arc;

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "get" => ok(bot
            .scripts
            .option_value(args.str(0)?)
            .map_err(BotError::Other)?),
        "set" => {
            bot.scripts
                .set_option_value(args.str(0)?, args.value(1).clone())
                .map_err(BotError::Other)?;
            ok(())
        }
        "has" => ok(bot.scripts.has_option(args.str(0)?)),
        "configure" => {
            bot.scripts.configure(&bot.control)?;
            ok(())
        }
        _ => Err(BotError::UnknownMethod(format!("config.{name}"))),
    }
}
