//! Parity mode (docs/HOST_SWF_PROPOSAL.md, R2): `TURU_HOST_PARITY=1` starts Turu's host SWF,
//! which loads VibeSkua's; VibeSkua's hosts the game and stays authoritative, Turu's answers
//! under `turu.` names. [`crate::flash::Flash::call`] asks both for each read and answers with
//! VibeSkua's; the events both send are matched per name, in order. Every difference not on
//! [`ALLOWED`] is a `parity:` line in the debug log.

use crate::repeat_log::RepeatLog;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::time::{Duration, Instant};

/// What Turu's host SWF puts before its names in parity mode.
pub const PREFIX: &str = "turu.";

/// The calls compared: reads, which change nothing. Writes go to VibeSkua's SWF only, or they
/// would happen twice.
const READS: &[&str] = &[
    "getGameObject",
    "getGameObjectS",
    "getGameObjectKey",
    "getArrayObject",
    "selectArrayObjects",
    "isNull",
    "isTrue",
];

/// The events compared. Not the start-up ones, which Turu's SWF doesn't send in parity mode,
/// nor `packetFromServer`, which it sends only on request.
const EVENTS: &[&str] = &["pext", "packet", "connectionLost", "userIntentionalAction"];

/// How long an event may wait for the other SWF's before it counts as missing.
const EVENT_WAIT: Duration = Duration::from_secs(5);
/// Events kept waiting per name at most.
const EVENTS_KEPT: usize = 1000;
/// How much of each answer a `parity:` line shows.
const SHOWN: usize = 200;

/// A known, deliberate difference: `same` says whether the two answers (VibeSkua's, then
/// Turu's) agree in what matters, for a call or event named `name` (`detail`: a call's
/// arguments).
struct Allowed {
    name: &'static str,
    /// Why they differ.
    #[allow(dead_code)]
    reason: &'static str,
    same: fn(detail: &str, vibeskua: &str, turu: &str) -> bool,
}

const ALLOWED: &[Allowed] = &[Allowed {
    name: "pext",
    reason: "Turu's SWF sends only the packet's `type` and `params`, what the bot and scripts \
             read; VibeSkua's stringifies the whole SmartFox event",
    same: |_, vibeskua, turu| pext_params(vibeskua) == pext_params(turu),
}];

/// `params` of the packet in a `pext` event's arguments (`["{json}"]`).
fn pext_params(args: &str) -> Option<Value> {
    let args: Value = serde_json::from_str(args).ok()?;
    let packet: Value = serde_json::from_str(args.get(0)?.as_str()?).ok()?;
    packet.get("params").cloned()
}

/// Which SWF an event came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    VibeSkua,
    Turu,
}

#[derive(Default)]
pub struct Parity {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// The callbacks Turu's SWF has (`turu.hostInfo`); `None` before it's asked.
    turu: Option<BTreeSet<String>>,
    counts: BTreeMap<String, Counts>,
    repeats: RepeatLog,
    events: HashMap<String, Waiting>,
}

#[derive(Default, Clone, Copy)]
struct Counts {
    compared: u64,
    /// Not on the allowlist.
    mismatched: u64,
    /// Different, but on the allowlist.
    allowed: u64,
}

/// One event's payloads from each SWF, waiting for the other's.
#[derive(Default)]
struct Waiting {
    vibeskua: VecDeque<(String, Instant)>,
    turu: VecDeque<(String, Instant)>,
}

impl Parity {
    /// On with `TURU_HOST_PARITY=1`.
    pub fn from_env() -> Option<Self> {
        enabled().then(Self::default)
    }

    /// Turu's host SWF said which callbacks it has (`None`: it didn't answer).
    pub fn set_turu_callbacks(&self, callbacks: Option<BTreeSet<String>>) {
        self.inner.lock().turu = callbacks;
    }

    /// The SWFs are starting again: forget Turu's callbacks and the events waiting.
    pub fn restart(&self) {
        let mut inner = self.inner.lock();
        inner.turu = None;
        inner.events.clear();
    }

    /// Whether `name` is compared: a read that Turu's SWF has.
    pub fn compares(&self, name: &str) -> bool {
        READS.contains(&name)
            && self
                .inner
                .lock()
                .turu
                .as_ref()
                .is_some_and(|c| c.contains(name))
    }

    /// Compares a call's answers; a line for the debug log when they differ and it's due.
    pub fn call(&self, name: &str, detail: &str, vibeskua: &str, turu: &str) -> Option<String> {
        self.inner
            .lock()
            .compare(name, detail, vibeskua, turu, Instant::now())
    }

    /// An event from `side`'s SWF (`args`: its arguments); lines for the debug log.
    pub fn event(&self, side: Side, name: &str, args: &[Value]) -> Vec<String> {
        self.event_at(side, name, args, Instant::now())
    }

    fn event_at(&self, side: Side, name: &str, args: &[Value], now: Instant) -> Vec<String> {
        if !EVENTS.contains(&name) {
            return Vec::new();
        }
        let payload = Value::Array(args.to_vec()).to_string();
        let mut inner = self.inner.lock();
        let waiting = inner.events.entry(name.to_owned()).or_default();
        let queue = match side {
            Side::VibeSkua => &mut waiting.vibeskua,
            Side::Turu => &mut waiting.turu,
        };
        if queue.len() >= EVENTS_KEPT {
            queue.pop_front();
        }
        queue.push_back((payload, now));

        let mut pairs = Vec::new();
        let mut missing = Vec::new();
        while let (Some(_), Some(_)) = (waiting.vibeskua.front(), waiting.turu.front()) {
            let (a, _) = waiting.vibeskua.pop_front().expect("checked");
            let (b, _) = waiting.turu.pop_front().expect("checked");
            pairs.push((a, b));
        }
        for (queue, side) in [
            (&mut waiting.vibeskua, Side::VibeSkua),
            (&mut waiting.turu, Side::Turu),
        ] {
            while queue
                .front()
                .is_some_and(|(_, at)| now.duration_since(*at) >= EVENT_WAIT)
            {
                let (payload, _) = queue.pop_front().expect("checked");
                missing.push((side, payload));
            }
        }

        let mut lines = Vec::new();
        for (a, b) in pairs {
            lines.extend(inner.compare(name, "", &a, &b, now));
        }
        for (side, payload) in missing {
            let (a, b) = match side {
                Side::VibeSkua => (payload, "(none)".to_owned()),
                Side::Turu => ("(none)".to_owned(), payload),
            };
            lines.extend(inner.compare(name, "", &a, &b, now));
        }
        lines
    }

    /// The counts so far, for the perf log; `None` before anything was compared.
    pub fn report(&self) -> Option<String> {
        let inner = self.inner.lock();
        if inner.counts.is_empty() {
            return None;
        }
        let rows: Vec<String> = inner
            .counts
            .iter()
            .map(|(name, c)| {
                format!(
                    "{name} {}/{} mismatched ({} allowed)",
                    c.mismatched, c.compared, c.allowed
                )
            })
            .collect();
        Some(format!("parity: {}", rows.join(", ")))
    }

    /// One line for the script log: everything compared and what differed.
    pub fn summary(&self) -> Option<String> {
        let inner = self.inner.lock();
        if inner.counts.is_empty() {
            return None;
        }
        let total = |f: fn(&Counts) -> u64| inner.counts.values().map(f).sum::<u64>();
        let different: Vec<String> = inner
            .counts
            .iter()
            .filter(|(_, c)| c.mismatched > 0)
            .map(|(name, c)| format!("{name} {}", c.mismatched))
            .collect();
        Some(format!(
            "Parity: {} compared, {} unlisted mismatches{}, {} allowed differences.",
            total(|c| c.compared),
            total(|c| c.mismatched),
            if different.is_empty() {
                String::new()
            } else {
                format!(" ({})", different.join(", "))
            },
            total(|c| c.allowed)
        ))
    }
}

impl Inner {
    fn compare(
        &mut self,
        name: &str,
        detail: &str,
        vibeskua: &str,
        turu: &str,
        now: Instant,
    ) -> Option<String> {
        let counts = self.counts.entry(name.to_owned()).or_default();
        counts.compared += 1;
        if vibeskua == turu {
            return None;
        }
        if ALLOWED
            .iter()
            .any(|a| a.name == name && (a.same)(detail, vibeskua, turu))
        {
            counts.allowed += 1;
            return None;
        }
        counts.mismatched += 1;
        let key = format!("{name} {detail}");
        let repeats = self.repeats.note(&key, now)?;
        let mut line = format!(
            "parity: {name}{}{detail}: VibeSkua {} / Turu {}",
            if detail.is_empty() { "" } else { " " },
            shorten(vibeskua),
            shorten(turu)
        );
        if repeats > 0 {
            line.push_str(&format!(" (and {repeats} more since last logged)"));
        }
        Some(line)
    }
}

fn shorten(text: &str) -> String {
    match text.char_indices().nth(SHOWN) {
        Some((i, _)) => format!("{}...", &text[..i]),
        None => text.to_owned(),
    }
}

/// `TURU_HOST_PARITY=1`.
pub fn enabled() -> bool {
    std::env::var("TURU_HOST_PARITY").is_ok_and(|v| v == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn with_turu(callbacks: &[&str]) -> Parity {
        let parity = Parity::default();
        parity.set_turu_callbacks(Some(callbacks.iter().map(|c| c.to_string()).collect()));
        parity
    }

    #[test]
    fn only_reads_turu_has_are_compared() {
        let parity = with_turu(&["getGameObject", "setGameObject"]);
        assert!(parity.compares("getGameObject"));
        assert!(!parity.compares("setGameObject"), "a write");
        assert!(!parity.compares("isNull"), "Turu's doesn't have it");
        assert!(
            !Parity::default().compares("getGameObject"),
            "not asked yet"
        );
    }

    #[test]
    fn a_difference_is_logged_and_counted() {
        let parity = with_turu(&["getGameObject"]);
        assert!(parity.call("getGameObject", "world.x", "1", "1").is_none());
        let line = parity.call("getGameObject", "world.x", "1", "2").unwrap();
        assert_eq!(line, "parity: getGameObject world.x: VibeSkua 1 / Turu 2");
        assert!(
            parity.call("getGameObject", "world.x", "1", "3").is_none(),
            "once a minute"
        );
        let summary = parity.summary().unwrap();
        assert!(
            summary.contains("3 compared, 2 unlisted mismatches (getGameObject 2)"),
            "{summary}"
        );
    }

    #[test]
    fn pext_compares_only_params() {
        let parity = Parity::default();
        let vibeskua = json!(
            r#"{"type":"onExtensionResponse","target":{},"params":{"type":"str","dataObj":["a"]}}"#
        );
        let turu =
            json!(r#"{"type":"onExtensionResponse","params":{"type":"str","dataObj":["a"]}}"#);
        assert!(parity.event(Side::VibeSkua, "pext", &[vibeskua]).is_empty());
        assert!(parity.event(Side::Turu, "pext", &[turu]).is_empty());
        assert!(parity.summary().unwrap().contains("1 allowed differences"));

        let other = json!(r#"{"params":{"type":"str","dataObj":["b"]}}"#);
        let mine = json!(r#"{"params":{"type":"str","dataObj":["c"]}}"#);
        parity.event(Side::Turu, "pext", &[mine]);
        let lines = parity.event(Side::VibeSkua, "pext", &[other]);
        assert_eq!(lines.len(), 1, "{lines:?}");
    }

    #[test]
    fn events_pair_in_order_and_a_late_one_is_missing() {
        let parity = Parity::default();
        let start = Instant::now();
        let packet = |p: &str| [json!(p)];
        assert!(
            parity
                .event_at(Side::VibeSkua, "packet", &packet("%a%"), start)
                .is_empty()
        );
        assert!(
            parity
                .event_at(Side::VibeSkua, "packet", &packet("%b%"), start)
                .is_empty()
        );
        assert!(
            parity
                .event_at(Side::Turu, "packet", &packet("%a%"), start)
                .is_empty()
        );
        let later = start + EVENT_WAIT;
        let lines = parity.event_at(Side::Turu, "packet", &packet("%c%"), later);
        assert_eq!(
            lines,
            [r#"parity: packet: VibeSkua ["%b%"] / Turu ["%c%"]"#]
        );

        // Turu's never sends this one: it counts as missing once the next comes (a new Parity:
        // the mismatch above keeps `packet` out of the log for a minute).
        let parity = Parity::default();
        let gone = later + Duration::from_secs(1);
        assert!(
            parity
                .event_at(Side::VibeSkua, "packet", &packet("%x%"), gone)
                .is_empty()
        );
        let lines = parity.event_at(Side::VibeSkua, "packet", &packet("%y%"), gone + EVENT_WAIT);
        assert_eq!(lines, [r#"parity: packet: VibeSkua ["%x%"] / Turu (none)"#]);
    }

    #[test]
    fn other_events_arent_compared() {
        let parity = Parity::default();
        assert!(parity.event(Side::VibeSkua, "loaded", &[]).is_empty());
        assert!(parity.summary().is_none());
    }
}
