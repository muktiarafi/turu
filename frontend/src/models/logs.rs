//! Logs (the Script Loader's script log, Diagnostics > Logs): line-fed. The view says which logs
//! there are and how far each has got; the lines come after a point, never the whole buffer each
//! time: `GET /log?type=<kind>&since=<next>` and `event: log` on `/events?logs=...`, which the
//! control API already serves (a full buffer is 5,000 lines, about 1 MB). Clear is the action.

use serde::{Deserialize, Serialize};
use turu_bot::Bot;
use turu_bot::log::LogKind;

/// The logs a user reads, in the Logs window's order: the script's and the client's (debug).
/// Packets are the Packet Logger's (`models::packets`).
pub const KINDS: [(LogKind, &str, &str); 2] = [
    (LogKind::Script, "script", "Script"),
    (LogKind::Debug, "debug", "Client"),
];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LogsView {
    pub logs: Vec<LogView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct LogView {
    /// Its name in the routes ("script").
    pub kind: &'static str,
    /// Its name for a user ("Script").
    pub title: &'static str,
    /// Lines held now.
    pub lines: usize,
    /// The number the next line will have: read `since` the last one seen.
    pub next: u64,
}

/// Logs' view of `bot` now.
pub fn view(bot: &Bot) -> LogsView {
    LogsView {
        logs: KINDS
            .iter()
            .map(|&(kind, name, title)| {
                let buffer = bot.logs.get(kind);
                LogView {
                    kind: name,
                    title,
                    lines: buffer.len(),
                    next: buffer.since(u64::MAX).1,
                }
            })
            .collect(),
    }
}

/// Clears the log named `kind` ("script", "debug").
pub fn clear(bot: &Bot, kind: &str) -> Result<(), String> {
    let (kind, _, _) = KINDS
        .iter()
        .find(|(_, name, _)| *name == kind)
        .ok_or_else(|| format!("no log named {kind}"))?;
    bot.logs.get(*kind).clear();
    Ok(())
}

/// The logs' actions (`POST /models/logs/do`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum LogsAct {
    /// Clears the log of `kind` ("script", "debug", ...).
    Clear { kind: String },
}

pub fn act(bot: &Bot, act: LogsAct) -> Result<(), String> {
    match act {
        LogsAct::Clear { kind } => clear(bot, &kind),
    }
}
