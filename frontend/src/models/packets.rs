//! Diagnostics' packet tools for a native shell, over the tab's engine (the egui windows use the
//! same, keeping their own search, selection and file dialogs):
//!
//! * **Logger** (VibeSkua's "Logger"): logging on or off and the "Show n / m" filters
//!   (`bot.logs.packet_logging`, `packet_hidden`); the lines are the control API's `packet` log,
//!   read from a point as the Logs page reads its own.
//! * **Spammer** (`PacketSpammerView`, `bot.spammer`): the queue, Start/Stop, the delay, Send to
//!   client, Add and Send now.
//! * **Interceptor** (`PacketInterceptorView`, `bot.capture`): Connect (relogging on a chosen
//!   server), Log packets, the captured packets with a page's search, category filters and
//!   selection, Copy and Clear.
//!
//! Every function here locks a page's state only briefly and never while calling the bot.

use super::Control;
use super::tab_state::{PerTab, lock};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use turu_bot::Bot;
use turu_bot::packet_filters::{self, LOGGER_FILTERS};

/// The filter flyout's ticks in an action's body (`shown`: one bool a filter).
fn ticks(shown: Vec<bool>, len: usize) -> Result<Vec<bool>, String> {
    if shown.len() != len {
        return Err(format!("{len} filters, not {}", shown.len()));
    }
    Ok(shown)
}

/// What each tool's last Clear took, for its Undo: the page offers it for a few seconds.
#[derive(Default)]
pub struct Cleared {
    log: Option<Vec<turu_bot::log::LogLine>>,
    queue: Option<Vec<String>>,
    capture: Option<Vec<turu_bot::capture::InterceptedPacket>>,
}

pub static CLEARED: PerTab<Cleared> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct PacketFilter {
    pub name: String,
    pub shown: bool,
}

// --- Logger ----------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct PacketLoggerView {
    pub logging: bool,
    pub filters: Vec<PacketFilter>,
    /// Lines held now.
    pub lines: u64,
    /// The number the next line will have: read the `packet` log `since` the last one seen.
    pub next: u64,
}

pub fn logger_view(bot: &Arc<Bot>) -> PacketLoggerView {
    let hidden = bot.logs.packet_hidden.load(Ordering::Relaxed);
    let buffer = &bot.logs.packets;
    PacketLoggerView {
        logging: bot.logs.packet_logging.load(Ordering::Relaxed),
        filters: LOGGER_FILTERS
            .iter()
            .enumerate()
            .map(|(i, name)| PacketFilter {
                name: (*name).to_owned(),
                shown: hidden & (1 << i) == 0,
            })
            .collect(),
        lines: buffer.len() as u64,
        next: buffer.since(u64::MAX).1,
    }
}

/// The Packet Logger's actions (`POST /models/packet-logger/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum PacketLoggerAct {
    Logging {
        on: bool,
    },
    /// Each filter's tick, in order: a filter that's off drops the packets it takes from then on.
    Filters {
        shown: Vec<bool>,
    },
    Clear,
    /// Puts the cleared lines back before those logged since.
    UndoClear,
}

pub fn logger_act(bot: &Arc<Bot>, act: PacketLoggerAct) -> Result<(), String> {
    match act {
        PacketLoggerAct::Logging { on } => bot.logs.packet_logging.store(on, Ordering::Relaxed),
        PacketLoggerAct::Filters { shown } => {
            let hidden = ticks(shown, LOGGER_FILTERS.len())?
                .iter()
                .enumerate()
                .filter(|(_, on)| !**on)
                .fold(0u32, |mask, (i, _)| mask | 1 << i);
            bot.logs.packet_hidden.store(hidden, Ordering::Relaxed);
        }
        PacketLoggerAct::Clear => lock(&CLEARED.of(bot)).log = Some(bot.logs.packets.take()),
        PacketLoggerAct::UndoClear => {
            let lines = lock(&CLEARED.of(bot)).log.take().ok_or("Nothing to put back")?;
            bot.logs.packets.restore(lines);
        }
    }
    Ok(())
}

// --- Spammer ---------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct SpammerView {
    pub running: bool,
    /// What it's sending ("Sending 2 of 5"); empty while stopped.
    pub status: String,
    /// The queue's row being sent; -1 while stopped.
    pub sending: i64,
    pub delay_ms: u64,
    pub to_client: bool,
    pub packets: Vec<String>,
    pub start: Control,
}

pub fn spammer_view(bot: &Arc<Bot>) -> SpammerView {
    let spammer = &bot.spammer;
    let running = spammer.is_running();
    let progress = spammer.progress().filter(|_| running);
    let packets = spammer.packets.lock().clone();
    SpammerView {
        running,
        status: progress.as_ref().map(|(_, s)| s.clone()).unwrap_or_default(),
        sending: progress.map_or(-1, |(i, _)| i as i64),
        delay_ms: spammer.delay_ms(),
        to_client: spammer.to_client(),
        start: Control::when(running || !packets.is_empty(), "Add packets to the queue first"),
        packets,
    }
}

/// The Spammer's actions (`POST /models/spammer/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum SpammerAct {
    Start,
    Stop,
    /// The delay between packets, at most 10 minutes.
    Delay {
        ms: u64,
    },
    ToClient {
        on: bool,
    },
    /// Adds `packet` to the queue.
    Add {
        packet: String,
    },
    /// Sends `packet` once, now.
    Send {
        packet: String,
    },
    /// Removes the queue's row at `position`, which must still be `packet` (a queue changed
    /// meanwhile isn't cut in the wrong place).
    Remove {
        position: u32,
        packet: String,
    },
    Clear,
    /// The cleared queue back, before what was added since.
    UndoClear,
}

pub fn spammer_act(bot: &Arc<Bot>, act: SpammerAct) -> Result<(), String> {
    let spammer = &bot.spammer;
    match act {
        SpammerAct::Start => {
            if spammer.packets.lock().is_empty() {
                return Err("Add packets to the queue first".into());
            }
            spammer.start(bot);
        }
        SpammerAct::Stop => spammer.stop(),
        SpammerAct::Delay { ms } => spammer.set_delay_ms(ms.min(600_000)),
        SpammerAct::ToClient { on } => spammer.set_to_client(on),
        SpammerAct::Add { packet } => {
            if !spammer.add(&packet) {
                return Err("Type a packet first".into());
            }
        }
        SpammerAct::Send { packet } => {
            if packet.trim().is_empty() {
                return Err("Type a packet first".into());
            }
            crate::turu::spawn_action_for(bot.clone(), "Spammer", move |bot| bot.spammer.send(bot, &packet));
        }
        SpammerAct::Remove { position, packet } => {
            let position = position as usize;
            let mut queue = spammer.packets.lock();
            if queue.get(position) != Some(&packet) {
                return Err("The queue changed: try again".into());
            }
            queue.remove(position);
        }
        SpammerAct::Clear => {
            let queue = std::mem::take(&mut *spammer.packets.lock());
            lock(&CLEARED.of(bot)).queue = Some(queue);
        }
        SpammerAct::UndoClear => {
            let mut earlier = lock(&CLEARED.of(bot)).queue.take().ok_or("Nothing to put back")?;
            let mut queue = spammer.packets.lock();
            earlier.append(&mut queue);
            *queue = earlier;
        }
    }
    Ok(())
}

// --- Interceptor -----------------------------------------------------------------------------

/// The most captured packets a native page is sent (the newest); the count says how many match.
pub const INTERCEPTOR_ROWS: usize = 300;

/// A native page's server, search, filters and selection (by packet number).
#[derive(Default)]
pub struct Interceptor {
    server: Option<String>,
    search: String,
    filters: Vec<bool>,
    selected: BTreeSet<u64>,
    /// Each packet's commands, by number, parsed once.
    commands: HashMap<u64, Vec<String>>,
}

pub static INTERCEPTOR: PerTab<Interceptor> = PerTab::new();

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct InterceptorView {
    pub running: bool,
    pub logging: bool,
    /// The server Connect relogs to; empty for the current one.
    pub server: String,
    pub servers: Vec<String>,
    pub refreshing: bool,
    pub search: String,
    pub filters: Vec<PacketFilter>,
    /// The packets the search and filters show.
    pub matches: u64,
    pub selected: u64,
    /// The newest [`INTERCEPTOR_ROWS`] of them, oldest first.
    pub rows: Vec<InterceptedRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "typegen", derive(facet::Facet), facet(rename_all = "camelCase"))]
#[serde(rename_all = "camelCase")]
pub struct InterceptedRow {
    /// Its number in the capture: a selection names it.
    pub number: u64,
    /// "OUT", "IN" or "BLOCK".
    pub direction: String,
    pub packet: String,
    pub selected: bool,
}

/// The shown packets (number, packet): those the search finds (a search of spaces finds all)
/// in a ticked category (VibeSkua `MatchesPacket`).
fn shown(bot: &Arc<Bot>) -> Vec<(u64, turu_bot::capture::InterceptedPacket)> {
    let names = packet_filters::interceptor_filters();
    let (first, packets) = bot.capture.snapshot();
    let state = INTERCEPTOR.of(bot);
    let (search, filters, mut commands) = {
        let mut s = lock(&state);
        if s.filters.len() != names.len() {
            s.filters = vec![true; names.len()];
        }
        s.selected.retain(|n| *n >= first);
        s.commands.retain(|n, _| *n >= first);
        (s.search.clone(), s.filters.clone(), std::mem::take(&mut s.commands))
    };
    let needle = (!search.trim().is_empty()).then(|| search.to_lowercase());
    let mut rows = Vec::new();
    for (n, packet) in (first..).zip(packets) {
        if needle
            .as_ref()
            .is_some_and(|needle| !packet.packet.to_lowercase().contains(needle))
        {
            continue;
        }
        let parsed = commands
            .entry(n)
            .or_insert_with(|| packet_filters::commands(&packet.packet));
        if packet_filters::interceptor_shows(parsed, &filters) {
            rows.push((n, packet));
        }
    }
    lock(&state).commands = commands;
    rows
}

pub fn interceptor_view(bot: &Arc<Bot>) -> InterceptorView {
    let rows = shown(bot);
    let names = packet_filters::interceptor_filters();
    let state = INTERCEPTOR.of(bot);
    let s = lock(&state);
    let skip = rows.len().saturating_sub(INTERCEPTOR_ROWS);
    InterceptorView {
        running: bot.capture.is_running(),
        logging: bot.capture.is_logging(),
        server: s.server.clone().unwrap_or_default(),
        servers: super::server_names(bot),
        refreshing: bot.servers.is_refreshing(),
        search: s.search.clone(),
        filters: names
            .iter()
            .zip(&s.filters)
            .map(|(name, shown)| PacketFilter {
                name: (*name).to_owned(),
                shown: *shown,
            })
            .collect(),
        matches: rows.len() as u64,
        selected: rows.iter().filter(|(n, _)| s.selected.contains(n)).count() as u64,
        rows: rows
            .iter()
            .skip(skip)
            .map(|(n, p)| InterceptedRow {
                number: *n,
                direction: p.direction().to_owned(),
                packet: p.packet.clone(),
                selected: s.selected.contains(n),
            })
            .collect(),
    }
}

/// The Interceptor's actions (`POST /models/interceptor/do`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[cfg_attr(
    feature = "typegen",
    derive(facet::Facet),
    facet(tag = "action", rename_all = "kebab-case")
)]
#[serde(tag = "action", rename_all = "kebab-case")]
#[repr(C)]
pub enum InterceptorAct {
    /// The server to connect to; empty for the current one.
    Server {
        name: String,
    },
    RefreshServers,
    /// Starts capturing, relogging on the chosen server.
    Connect,
    Disconnect,
    Logging {
        on: bool,
    },
    Search {
        text: String,
    },
    /// Each filter's tick, in order.
    Filters {
        shown: Vec<bool>,
    },
    /// Selects or unselects the packet numbered `number`.
    Select {
        number: u64,
        on: bool,
    },
    Unselect,
    Clear,
    UndoClear,
    /// Answers `text`: the selected packets, or every shown one.
    Copy,
}

pub fn interceptor_act(bot: &Arc<Bot>, act: InterceptorAct) -> Result<Json, String> {
    let state = INTERCEPTOR.of(bot);
    let capture = &bot.capture;
    match act {
        InterceptorAct::Server { name } => {
            lock(&state).server = (!name.trim().is_empty()).then_some(name);
        }
        InterceptorAct::RefreshServers => {
            crate::turu::spawn_action_for(bot.clone(), "Servers", |bot| {
                turu_bot::api::servers::get_servers(bot, true);
                Ok(())
            });
        }
        InterceptorAct::Connect => {
            capture.start();
            if let Some(server) = lock(&state).server.clone() {
                crate::turu::spawn_action_for(bot.clone(), "Interceptor", move |bot| {
                    let ctx = turu_bot::api::servers::Ctx::new(bot);
                    turu_bot::api::servers::relogin_named(&ctx, &server).map(|_| ())
                });
            }
        }
        InterceptorAct::Disconnect => capture.stop(),
        InterceptorAct::Logging { on } => capture.set_logging(on),
        InterceptorAct::Search { text } => lock(&state).search = text,
        InterceptorAct::Filters { shown } => {
            let shown = ticks(shown, packet_filters::interceptor_filters().len())?;
            lock(&state).filters = shown;
        }
        InterceptorAct::Select { number, on } => {
            let mut s = lock(&state);
            if on {
                s.selected.insert(number);
            } else {
                s.selected.remove(&number);
            }
        }
        InterceptorAct::Unselect => lock(&state).selected.clear(),
        InterceptorAct::Clear => {
            let packets = capture.take();
            lock(&CLEARED.of(bot)).capture = Some(packets);
            lock(&state).selected.clear();
        }
        InterceptorAct::UndoClear => {
            let packets = lock(&CLEARED.of(bot)).capture.take().ok_or("Nothing to put back")?;
            capture.restore(packets);
        }
        InterceptorAct::Copy => {
            let rows = shown(bot);
            let selected = lock(&state).selected.clone();
            let picked: Vec<&str> = rows
                .iter()
                .filter(|(n, _)| selected.contains(n))
                .map(|(_, p)| p.packet.as_str())
                .collect();
            let text = if picked.is_empty() {
                rows.iter()
                    .map(|(_, p)| p.packet.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                picked.join("\n")
            };
            return super::to_json(&super::CopiedText { text: Some(text) });
        }
    }
    Ok(json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(packet: &str) -> SpammerAct {
        SpammerAct::Add { packet: packet.into() }
    }

    fn remove(packet: &str) -> SpammerAct {
        SpammerAct::Remove {
            position: 0,
            packet: packet.into(),
        }
    }

    #[test]
    fn logger_filters_and_spammer_queue() {
        let bot = Bot::new();
        let mut shown = vec![true; LOGGER_FILTERS.len()];
        shown[1] = false;
        logger_act(&bot, PacketLoggerAct::Filters { shown }).unwrap();
        let v = logger_view(&bot);
        assert!(!v.filters[1].shown && v.filters[0].shown);
        assert!(logger_act(&bot, PacketLoggerAct::Filters { shown: vec![true] }).is_err());

        assert!(spammer_act(&bot, SpammerAct::Start).is_err());
        spammer_act(&bot, add("%xt%zm%moveToCell%1%Enter%Spawn%")).unwrap();
        spammer_act(&bot, add("%xt%zm%afk%1%false%")).unwrap();
        let v = spammer_view(&bot);
        assert_eq!((v.packets.len(), v.start.enabled, v.sending), (2, true, -1));
        assert!(spammer_act(&bot, remove("nope")).is_err());
        spammer_act(&bot, remove("%xt%zm%moveToCell%1%Enter%Spawn%")).unwrap();
        assert_eq!(spammer_view(&bot).packets, ["%xt%zm%afk%1%false%"]);
        spammer_act(&bot, SpammerAct::Delay { ms: 999_999 }).unwrap();
        assert_eq!(spammer_view(&bot).delay_ms, 600_000);
        // Clear, add one, Undo: the cleared queue comes back first.
        spammer_act(&bot, SpammerAct::Clear).unwrap();
        spammer_act(&bot, add("%xt%zm%afk%1%true%")).unwrap();
        spammer_act(&bot, SpammerAct::UndoClear).unwrap();
        assert_eq!(
            spammer_view(&bot).packets,
            ["%xt%zm%afk%1%false%", "%xt%zm%afk%1%true%"]
        );
        assert!(spammer_act(&bot, SpammerAct::UndoClear).is_err());
    }

    #[test]
    fn interceptor_search_selection_and_copy() {
        let bot = Bot::new();
        let v = interceptor_view(&bot);
        assert_eq!((v.matches, v.rows.len(), v.server.as_str()), (0, 0, ""));
        interceptor_act(&bot, InterceptorAct::Server { name: "Yorumi".into() }).unwrap();
        interceptor_act(&bot, InterceptorAct::Search { text: "afk".into() }).unwrap();
        let v = interceptor_view(&bot);
        assert_eq!((v.server.as_str(), v.search.as_str()), ("Yorumi", "afk"));
        let copied = interceptor_act(&bot, InterceptorAct::Copy).unwrap();
        assert_eq!(copied, json!({ "text": "" }));
        assert!(interceptor_act(&bot, InterceptorAct::Filters { shown: vec![] }).is_err());
    }
}
