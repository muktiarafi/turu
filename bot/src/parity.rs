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
    // R3a: monsters, skills, auras.
    "availableMonsters",
    "getMonsters",
    "getTargetMonster",
    "canUseSkill",
    "GetPlayerAura",
    "GetMonsterAuraByID",
    "GetMonsterAuraByName",
    "GetAuraSnapshots",
    "GetAurasValue",
    "HasAnyActiveAura",
    // R3b: inventory, shop, player, world.
    "hasInventoryCategories",
    "inventoryBagUsedSlots",
    "inventoryHasSpaceFor",
    "isFavoriteItem",
    "getUnmetPurchaseRequirements",
    "isLoggedIn",
    "isKicked",
    "gender",
    "getAvatar",
    "getActiveModalMessage",
];

/// The events compared. Not the start-up ones, which Turu's SWF doesn't send in parity mode,
/// nor `packetFromServer`, which it sends only on request.
const EVENTS: &[&str] = &[
    "pext",
    "packet",
    "connectionLost",
    "userIntentionalAction",
    "openWebsite",
];

/// What a `parity:` line shows for the SWF that never sent an event.
const MISSING: &str = "(none)";

/// How long an event may wait for the other SWF's before it counts as missing.
const EVENT_WAIT: Duration = Duration::from_secs(5);
/// Events kept waiting per name at most.
const EVENTS_KEPT: usize = 1000;
/// How much of each answer a `parity:` line shows.
const SHOWN: usize = 200;
/// How much of what's the same a `parity:` line shows before the first difference.
const BEFORE_DIFFERENCE: usize = 60;

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

const ALLOWED: &[Allowed] = &[
    Allowed {
        name: "pext",
        reason: "Turu's SWF sends only the packet's `type` and `params`, what the bot and \
                 scripts read; VibeSkua's stringifies the whole SmartFox event",
        same: |_, vibeskua, turu| pext_params(vibeskua) == pext_params(turu),
    },
    Allowed {
        name: "GetAuraSnapshots",
        reason: "`remaining` is read from the player's clock in each SWF's call, which can be a \
                 millisecond or two apart in the same frame",
        same: |_, vibeskua, turu| same_snapshots(vibeskua, turu),
    },
    Allowed {
        name: "openWebsite",
        reason: "Turu's SWF has no monster-click wiki link: Turu doesn't act on `openWebsite` \
                 (it only logs it), and the game's own wiki buttons open the browser through \
                 Ruffle",
        same: |_, _, turu| turu == MISSING,
    },
    Allowed {
        name: "packet",
        reason: "only the player's socket saw it (R5): VibeSkua's SWF reads sent packets from \
                 SmartFox's debug messages, which the game's own logger keeps turning off, and \
                 missed about 5% of every kind in game; the player reports every flush",
        same: |_, vibeskua, _| vibeskua == MISSING,
    },
    Allowed {
        name: "pext",
        reason: "only the player's socket saw it (R5): VibeSkua's SWF missed about 4% of the \
                 received extension responses, of every kind, with no error of its own; the \
                 player reports every message the client handled",
        same: |_, vibeskua, _| vibeskua == MISSING,
    },
];

/// Aura snapshots that differ at most by a few milliseconds in `remaining`.
fn same_snapshots(a: &str, b: &str) -> bool {
    const SECONDS: f64 = 0.05;
    let (Ok(Value::Array(a)), Ok(Value::Array(b))) = (
        serde_json::from_str::<Value>(a),
        serde_json::from_str::<Value>(b),
    ) else {
        return false;
    };
    a.len() == b.len()
        && a.iter().zip(&b).all(|(a, b)| {
            let (Value::Object(a), Value::Object(b)) = (a, b) else {
                return false;
            };
            a.len() == b.len()
                && a.iter().all(|(key, va)| match (key.as_str(), b.get(key)) {
                    ("remaining", Some(vb)) => match (va.as_f64(), vb.as_f64()) {
                        (Some(x), Some(y)) => (x - y).abs() <= SECONDS,
                        _ => va == vb,
                    },
                    (_, Some(vb)) => va == vb,
                    (_, None) => false,
                })
        })
}

/// The command an event's arguments carry (`["%xt%zm%gar%..."]`: `gar`; a `pext`: its
/// `dataObj`'s `cmd`, or a string packet's first field), for telling one-sided events apart.
fn command_of(args: &str) -> String {
    let Ok(Value::Array(args)) = serde_json::from_str::<Value>(args) else {
        return String::new();
    };
    let Some(first) = args.first().and_then(Value::as_str) else {
        return String::new();
    };
    if let Some(rest) = first.strip_prefix("%xt%zm%") {
        return rest.split('%').next().unwrap_or_default().to_owned();
    }
    let Ok(packet) = serde_json::from_str::<Value>(first) else {
        return String::new();
    };
    let data = &packet["params"]["dataObj"];
    data.get("cmd")
        .or_else(|| data.get(0))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Whether [`ALLOWED`] accepts `vibeskua` and `turu` as the same for `name`.
fn allowed(name: &str, vibeskua: &str, turu: &str) -> bool {
    ALLOWED
        .iter()
        .any(|a| a.name == name && (a.same)("", vibeskua, turu))
}

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
    /// Events only VibeSkua's SWF sent (no partner in time).
    only_vibeskua: u64,
    /// Events only Turu's side sent.
    only_turu: u64,
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

    /// Pairs an event with an equal one (or one [`ALLOWED`] accepts) the other SWF sent, in
    /// either order: two captures can each see a few the other doesn't (VibeSkua's starts at
    /// login), and pairing by order would then compare every later event with the wrong one.
    /// An event left without a partner for [`EVENT_WAIT`] is a mismatch, seen on one side only.
    fn event_at(&self, side: Side, name: &str, args: &[Value], now: Instant) -> Vec<String> {
        if !EVENTS.contains(&name) {
            return Vec::new();
        }
        let payload = Value::Array(args.to_vec()).to_string();
        let mut inner = self.inner.lock();
        let waiting = inner.events.entry(name.to_owned()).or_default();
        let (mine, theirs) = match side {
            Side::VibeSkua => (&mut waiting.vibeskua, &mut waiting.turu),
            Side::Turu => (&mut waiting.turu, &mut waiting.vibeskua),
        };
        // VibeSkua's first, as `ALLOWED` takes them.
        let ordered = |other: &str| match side {
            Side::VibeSkua => (payload.clone(), other.to_owned()),
            Side::Turu => (other.to_owned(), payload.clone()),
        };
        let found = theirs.iter().position(|(other, _)| {
            let (a, b) = ordered(other);
            a == b || allowed(name, &a, &b)
        });
        let matched = match found {
            Some(i) => {
                let (other, _) = theirs.remove(i).expect("found");
                Some(ordered(&other))
            }
            None => {
                if mine.len() >= EVENTS_KEPT {
                    mine.pop_front();
                }
                mine.push_back((payload.clone(), now));
                None
            }
        };

        let mut missing = Vec::new();
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
        if let Some((a, b)) = matched {
            lines.extend(inner.compare(name, "", &a, &b, now));
        }
        for (side, payload) in missing {
            let (a, b) = match side {
                Side::VibeSkua => (payload, MISSING.to_owned()),
                Side::Turu => (MISSING.to_owned(), payload),
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
            .map(|(name, c)| {
                if c.only_vibeskua + c.only_turu > 0 {
                    format!(
                        "{name} {}: VibeSkua only {}, Turu only {}",
                        c.mismatched, c.only_vibeskua, c.only_turu
                    )
                } else {
                    format!("{name} {}", c.mismatched)
                }
            })
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
        // An event one side alone sent: counted by side, and logged once a minute per side and
        // command, so one kind doesn't hide another.
        let alone = if turu == MISSING {
            counts.only_vibeskua += 1;
            Some(("VibeSkua", vibeskua))
        } else if vibeskua == MISSING {
            counts.only_turu += 1;
            Some(("Turu", turu))
        } else {
            None
        };
        let key = match alone {
            Some((side, payload)) => format!("{name} {detail} {side} only {}", command_of(payload)),
            None => format!("{name} {detail}"),
        };
        let repeats = self.repeats.note(&key, now)?;
        let start = excerpt_start(vibeskua, turu);
        let mut line = format!(
            "parity: {name}{}{detail}: VibeSkua {} / Turu {}",
            if detail.is_empty() { "" } else { " " },
            excerpt(vibeskua, start),
            excerpt(turu, start)
        );
        if repeats > 0 {
            line.push_str(&format!(" (and {repeats} more since last logged)"));
        }
        Some(line)
    }
}

/// Where a line starts showing two answers: a little before the first character that differs,
/// so a difference deep in a long JSON answer shows.
fn excerpt_start(a: &str, b: &str) -> usize {
    let differs = a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count();
    differs.saturating_sub(BEFORE_DIFFERENCE)
}

/// `text` from character `start`, at most [`SHOWN`] characters, with `...` where it's cut.
fn excerpt(text: &str, start: usize) -> String {
    let shown: String = text.chars().skip(start).take(SHOWN).collect();
    let before = if start > 0 { "..." } else { "" };
    let after = if text.chars().count() > start + SHOWN {
        "..."
    } else {
        ""
    };
    format!("{before}{shown}{after}")
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
        let start = Instant::now();
        assert!(
            parity
                .event_at(Side::Turu, "pext", &[mine], start)
                .is_empty()
        );
        assert!(
            parity
                .event_at(Side::VibeSkua, "pext", &[other], start)
                .is_empty(),
            "different params don't pair"
        );
        let lines = parity.event_at(Side::VibeSkua, "pext", &[json!("{}")], start + EVENT_WAIT);
        // Turu's (the player's) alone is allowed; VibeSkua's alone isn't.
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(parity.summary().unwrap().contains("1 unlisted mismatches"));
    }

    #[test]
    fn events_pair_by_content_and_one_without_a_partner_is_missing() {
        let parity = Parity::default();
        let start = Instant::now();
        let packet = |p: &str| [json!(p)];
        // Only VibeSkua's saw `%a%`. The rest still pair, in any order.
        for (side, p) in [
            (Side::VibeSkua, "%a%"),
            (Side::Turu, "%b%"),
            (Side::VibeSkua, "%c%"),
            (Side::VibeSkua, "%b%"),
            (Side::Turu, "%c%"),
        ] {
            assert!(
                parity
                    .event_at(side, "packet", &packet(p), start)
                    .is_empty()
            );
        }
        let later = start + EVENT_WAIT;
        let lines = parity.event_at(Side::Turu, "packet", &packet("%d%"), later);
        assert_eq!(lines, [r#"parity: packet: VibeSkua ["%a%"] / Turu (none)"#]);
        let summary = parity.summary().unwrap();
        assert!(
            summary.contains(
                "3 compared, 1 unlisted mismatches (packet 1: VibeSkua only 1, Turu only 0)"
            ),
            "{summary}"
        );
    }

    #[test]
    fn a_long_answer_shows_where_it_differs() {
        let same = "x".repeat(500);
        let a = format!("{same}A{same}");
        let b = format!("{same}B{same}");
        let start = excerpt_start(&a, &b);
        assert_eq!(start, 500 - BEFORE_DIFFERENCE);
        let shown = excerpt(&a, start);
        assert!(
            shown.starts_with("...") && shown.ends_with("..."),
            "{shown}"
        );
        assert!(shown.contains('A'));
        assert_eq!(excerpt("short", 0), "short");
    }

    #[test]
    fn aura_snapshots_allow_a_few_milliseconds() {
        let a = r#"[{"dur":20,"nam":"Valiance","remaining":19.401,"n":1}]"#;
        assert!(same_snapshots(
            a,
            r#"[{"dur":20,"nam":"Valiance","remaining":19.399,"n":1}]"#
        ));
        assert!(!same_snapshots(
            a,
            r#"[{"dur":20,"nam":"Valiance","remaining":18.0,"n":1}]"#
        ));
        assert!(!same_snapshots(
            a,
            r#"[{"dur":20,"nam":"Other","remaining":19.401,"n":1}]"#
        ));
        assert!(!same_snapshots(a, "[]"));
    }

    #[test]
    fn a_wiki_link_only_vibeskua_opens_is_allowed() {
        let parity = Parity::default();
        let start = Instant::now();
        let url = [json!("https://aqwwiki.wikidot.com/FrogZard")];
        parity.event_at(Side::VibeSkua, "openWebsite", &url, start);
        let lines = parity.event_at(Side::VibeSkua, "openWebsite", &url, start + EVENT_WAIT);
        assert!(lines.is_empty(), "{lines:?}");
        assert!(parity.summary().unwrap().contains("0 unlisted mismatches"));
    }

    #[test]
    fn one_sided_events_are_counted_by_side_and_command() {
        assert_eq!(command_of(r#"["%xt%zm%gar%1%42%aa>m:1%wvz%"]"#), "gar");
        assert_eq!(
            command_of(
                &json!([r#"{"params":{"type":"json","dataObj":{"cmd":"ct"}}}"#]).to_string()
            ),
            "ct"
        );
        assert_eq!(
            command_of(
                &json!([r#"{"params":{"type":"str","dataObj":["warning","-1"]}}"#]).to_string()
            ),
            "warning"
        );
        let parity = Parity::default();
        let start = Instant::now();
        let gar = [json!("%xt%zm%gar%1%1%aa%")];
        let mv = [json!("%xt%zm%mv%1%2%3%")];
        parity.event_at(Side::VibeSkua, "packet", &gar, start);
        parity.event_at(Side::VibeSkua, "packet", &mv, start);
        let lines = parity.event_at(Side::Turu, "packet", &[json!("%x%")], start + EVENT_WAIT);
        assert_eq!(lines.len(), 2, "gar and mv each logged: {lines:?}");
        let summary = parity.summary().unwrap();
        assert!(
            summary.contains("VibeSkua only 2, Turu only 0"),
            "{summary}"
        );
    }

    #[test]
    fn packets_only_the_player_saw_are_allowed_but_not_the_reverse() {
        let parity = Parity::default();
        let start = Instant::now();
        let later = start + EVENT_WAIT;
        let pext = |d: &str| [json!(format!(r#"{{"params":{{"dataObj":["{d}"]}}}}"#))];
        parity.event_at(Side::Turu, "pext", &pext("player"), start);
        parity.event_at(Side::VibeSkua, "pext", &pext("vibeskua"), start);
        let lines = parity.event_at(Side::Turu, "pext", &pext("next"), later);
        assert_eq!(lines.len(), 1, "only VibeSkua's is reported: {lines:?}");
        assert!(lines[0].contains("Turu (none)"), "{lines:?}");
        assert!(parity.summary().unwrap().contains("1 allowed differences"));
    }

    #[test]
    fn other_events_arent_compared() {
        let parity = Parity::default();
        assert!(parity.event(Side::VibeSkua, "loaded", &[]).is_empty());
        assert!(parity.summary().is_none());
    }
}
