//! `bot.send` (VibeSkua `IScriptSend`, `ScriptSend.cs`).

use super::{Args, ok};
use crate::Bot;
use crate::error::{BotError, BotResult};
use serde_json::Value;
use std::sync::Arc;

/// Sends a packet to the server: `sfc.send<type>(packet)`, type `String` (for
/// `%xt%zm%...%`) or `Xt`.
pub fn packet(bot: &Bot, packet: &str, kind: &str) -> BotResult<()> {
    bot.flash
        .call_game_function(&format!("sfc.send{kind}"), vec![packet.into()])
}

/// Makes the client handle `packet` as if the server sent it (`sendClientPacket`); type
/// `str`, `json` or `xml`.
pub fn client_packet(bot: &Bot, packet: &str, kind: &str) -> BotResult<()> {
    bot.flash
        .call("sendClientPacket", vec![packet.into(), kind.into()])?;
    Ok(())
}

/// A chat line shown only to you: `%xt%chatm%0%{kind}~{message}%{sentBy}%` (kind `server`,
/// `moderator`, `warning`, `event`, `guild`, `whisper`, `zone`).
pub fn client_message(bot: &Bot, kind: &str, message: &str, sent_by: &str) -> BotResult<()> {
    client_packet(
        bot,
        &format!("%xt%chatm%0%{kind}~{message}%{sent_by}%"),
        "str",
    )
}

/// `%xt%zm%whisper%1%{message}%{name}%`.
pub fn whisper(bot: &Bot, name: &str, message: &str) -> BotResult<()> {
    packet(
        bot,
        &format!("%xt%zm%whisper%1%{message}%{name}%"),
        "String",
    )
}

pub fn dispatch(bot: &Arc<Bot>, name: &str, args: Args) -> BotResult<Value> {
    match name {
        "packet" => ok(packet(bot, args.str(0)?, args.str_or(1, "String"))?),
        "clientPacket" => ok(client_packet(bot, args.str(0)?, args.str_or(1, "str"))?),
        "clientMessage" => ok(client_message(
            bot,
            args.str(0)?,
            args.str(1)?,
            args.str_or(2, "Turu"),
        )?),
        "whisper" => ok(whisper(bot, args.str(0)?, args.str(1)?)?),
        _ => Err(BotError::UnknownMethod(format!("send.{name}"))),
    }
}
