//! Tapes: what passed between the bot and the game during a run, recorded for tests that play
//! it back offline (docs/REPLAY_TESTS_PROPOSAL.md, phase 1).
//!
//! A tape is JSON Lines, gzip-compressed (`.tape.jsonl.gz`): a [`Header`], then one [`Line`] for
//! each call into Flash (with its answer), each call from `turu.swf` ([`crate::Bot::on_flash_call`])
//! and each message of the game's socket ([`crate::Bot::on_socket`]), and an [`End`] when the
//! recording stopped cleanly. The [`Recorder`] writes it on a thread of its own; secrets and chat
//! are taken out as each line is written ([`redact`]). [`read`] reads one back.
//!
//! Switched on with `TURU_TAPE=<file or folder>` (or the app's `--record-tape`): every bot made
//! afterwards records. By default a recording starts when the player first joins a map, so the
//! login screen isn't on it; `TURU_TAPE_FROM_START=1` records from the start (a relogin tape),
//! which redaction keeps free of the password.

mod recorder;
pub mod redact;

pub use recorder::{Recorder, finish_all};

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The tape format's version, in its [`Header`].
pub const FORMAT: u32 = 1;

/// A tape's file name ends in this.
pub const EXTENSION: &str = ".tape.jsonl.gz";

/// The first line of a tape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    /// [`FORMAT`] when it was written.
    pub format: u32,
    /// The engine's version (`turu_bot`).
    pub turu: String,
    /// When the recording started, in milliseconds since the Unix epoch.
    pub started_unix_ms: u64,
    /// Whether it waited for the player to join a map before recording.
    pub from_world: bool,
    /// The bot's options (`options.json`) when it started.
    pub options: Value,
    /// Set by the scrubber (phase 2) once player names are replaced; a recording never sets it.
    #[serde(default)]
    pub scrubbed: bool,
}

/// One line of a tape after the header. `n` orders them: it is taken when the line's event
/// begins (a call: when it is made, not when it answers), so sorting by it puts a call before
/// the packets it caused. `t` is the milliseconds since the recording started, at the same moment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k", rename_all = "camelCase")]
pub enum Line {
    /// A call into Flash ([`crate::flash::Flash::call`]).
    #[serde(rename_all = "camelCase")]
    Call {
        n: u64,
        t: u64,
        /// The calling thread's name ("Script Thread", ...), when it has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread: Option<String>,
        name: String,
        args: Vec<Value>,
        /// Its answer; `None` when it failed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        answer: Option<String>,
        /// Why it failed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// How long the player took to run it, in milliseconds, when the host says.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run_ms: Option<f64>,
    },
    /// A call from `turu.swf` (`pext`, `packet`, `event`, `loaded`, ...).
    Event {
        n: u64,
        t: u64,
        name: String,
        args: Vec<Value>,
    },
    /// A message the game's socket sent.
    Sent {
        n: u64,
        t: u64,
        socket: u64,
        text: String,
    },
    /// A message the game's socket received (unpacked when the server compressed it).
    Received {
        n: u64,
        t: u64,
        socket: u64,
        text: String,
    },
    /// Something the bot did that explains what follows (a script started or stopped).
    Note {
        n: u64,
        t: u64,
        what: String,
        detail: Value,
    },
}

impl Line {
    /// Its place in the tape's order.
    pub fn n(&self) -> u64 {
        match self {
            Self::Call { n, .. }
            | Self::Event { n, .. }
            | Self::Sent { n, .. }
            | Self::Received { n, .. }
            | Self::Note { n, .. } => *n,
        }
    }
}

/// The last line of a tape that was stopped cleanly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "k", rename = "end", rename_all = "camelCase")]
pub struct End {
    pub t: u64,
    /// How many lines were written.
    pub lines: u64,
    /// How many were lost because the writer fell behind (the queue was full) or failed.
    pub dropped: u64,
    /// The calls into Flash still waiting for their answer when it stopped: their numbers are
    /// taken, their lines never came.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unanswered: Vec<u64>,
}

/// A tape read back.
#[derive(Debug, Clone, PartialEq)]
pub struct Tape {
    pub header: Header,
    /// Sorted by [`Line::n`].
    pub lines: Vec<Line>,
    /// `None` when the recording didn't stop cleanly (the process was killed): the lines up to
    /// its last flush, about a second before, are there.
    pub end: Option<End>,
    /// Line numbers missing from the sequence: lines that were dropped (not the calls
    /// [`End::unanswered`] lists).
    pub missing: Vec<u64>,
    /// The file ended mid-stream (no gzip trailer).
    pub truncated: bool,
}

impl Tape {
    /// Whether every line taken was written: a test can rely on it as a whole.
    pub fn is_complete(&self) -> bool {
        self.missing.is_empty() && self.end.as_ref().is_some_and(|e| e.dropped == 0)
    }
}

/// Reads a tape. A file cut short (a killed process) reads up to where it ends.
pub fn read(path: &Path) -> io::Result<Tape> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(flate2::read::MultiGzDecoder::new(BufReader::new(file)));
    let mut header = None;
    let mut lines = Vec::new();
    let mut end = None;
    let mut truncated = false;
    let mut text = String::new();
    loop {
        text.clear();
        match reader.read_line(&mut text) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                truncated = true;
                break;
            }
            Err(e) => return Err(e),
        }
        if !text.ends_with('\n') {
            // The last line, cut off by the end of what was flushed.
            truncated = true;
            break;
        }
        let bad = |e: serde_json::Error| io::Error::new(io::ErrorKind::InvalidData, e);
        if header.is_none() {
            header = Some(serde_json::from_str::<Header>(&text).map_err(bad)?);
        } else if text.starts_with(r#"{"k":"end""#) {
            end = Some(serde_json::from_str::<End>(&text).map_err(bad)?);
        } else {
            lines.push(serde_json::from_str::<Line>(&text).map_err(bad)?);
        }
    }
    let header = header
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "a tape without its header"))?;
    lines.sort_by_key(Line::n);
    let unanswered = end
        .as_ref()
        .map_or(&[][..], |e: &End| e.unanswered.as_slice());
    let mut missing = Vec::new();
    let mut expected = 0;
    for line in &lines {
        missing.extend((expected..line.n()).filter(|n| !unanswered.contains(n)));
        expected = line.n() + 1;
    }
    Ok(Tape {
        header,
        lines,
        end,
        missing,
        truncated,
    })
}

/// Where a bot's recorder is kept: [`crate::flash::Flash`] holds it, as most lines are its calls.
#[derive(Default)]
pub struct Slot(RwLock<Option<Arc<Recorder>>>);

impl Slot {
    /// The recorder, while one records.
    pub fn get(&self) -> Option<Arc<Recorder>> {
        self.0.read().clone()
    }

    pub(crate) fn set(&self, recorder: Option<Arc<Recorder>>) -> Option<Arc<Recorder>> {
        std::mem::replace(&mut *self.0.write(), recorder)
    }
}

/// `--record-tape`'s path and whether to record from the start, for the bots made afterwards.
static STARTUP: Mutex<Option<(PathBuf, bool)>> = Mutex::new(None);

/// Every bot made from now on records to `path` (a file, or a folder for a file per bot), as
/// with `TURU_TAPE`. `from_start`: as `TURU_TAPE_FROM_START`.
pub fn set_startup_path(path: PathBuf, from_start: bool) {
    *STARTUP.lock() = Some((path, from_start));
}

/// Set in a process that plays no game (the window's shell): its bots don't record, even with
/// `TURU_TAPE` set (its tabs' clients, which inherit it, do).
static NO_STARTUP: AtomicBool = AtomicBool::new(false);

/// Bots made from now on don't start a tape by themselves.
pub fn disable_startup() {
    NO_STARTUP.store(true, Ordering::Relaxed);
}

/// What a new bot should record to: `--record-tape`, else `TURU_TAPE`.
pub(crate) fn startup_target() -> Option<(PathBuf, bool)> {
    if NO_STARTUP.load(Ordering::Relaxed) {
        return None;
    }
    if let Some(target) = STARTUP.lock().clone() {
        return Some(target);
    }
    let path = std::env::var_os("TURU_TAPE").filter(|p| !p.is_empty())?;
    let from_start = std::env::var("TURU_TAPE_FROM_START").is_ok_and(|v| !v.is_empty() && v != "0");
    Some((PathBuf::from(path), from_start))
}

/// On `tape`: what `turu.swf` said of itself (`hostInfo`), so a replay can check it plays the
/// tape to the same host. Asks nothing of the game (the answer kept when the client loaded): safe
/// on the player's thread.
pub(crate) fn note_host_info(tape: &Recorder, flash: &crate::flash::Flash) {
    let info = flash.host_info().map_or(Value::Null, |info| {
        serde_json::to_value(&*info).unwrap_or_default()
    });
    tape.note("hostInfo", info);
}
