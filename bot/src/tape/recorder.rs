//! Writes a tape: the callers hand over lines without waiting, a thread of its own redacts,
//! compresses and writes them.

use super::{EXTENSION, End, FORMAT, Header, Line, redact};
use crate::host::FlashArg;
use flate2::Compression;
use flate2::write::GzEncoder;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Lines waiting for the writer. A full queue drops lines (counted) rather than make the
/// player's thread wait: `on_flash_call` and `on_socket` run while the player is locked.
const QUEUE: usize = 65_536;

/// How often the writer flushes what it has, so a killed process leaves a readable tape.
const FLUSH_EVERY: Duration = Duration::from_secs(1);

/// How long a stop waits for calls into Flash still waiting for their answer. Short: at exit the
/// thread stopping may be the one that would answer them.
const STOP_WAITS: Duration = Duration::from_millis(500);

enum Msg {
    Line(Line),
    Stop,
}

/// Records one bot's tape. Cheap to call from any thread, including the player's.
pub struct Recorder {
    tx: SyncSender<Msg>,
    path: PathBuf,
    started: Instant,
    next: AtomicU64,
    dropped: Arc<AtomicU64>,
    /// The calls into Flash made but not answered yet: their lines come later.
    pending: Arc<Mutex<BTreeSet<u64>>>,
    /// Lines are taken once the player has joined a map (or from the start).
    open: AtomicBool,
    stopped: AtomicBool,
    writer: Mutex<Option<JoinHandle<()>>>,
}

/// A line's place and time, taken when its event begins.
#[derive(Debug, Clone, Copy)]
pub struct Ticket {
    n: u64,
    t: u64,
}

/// The recorders running in this process, for [`finish_all`].
static RUNNING: Mutex<Vec<Weak<Recorder>>> = Mutex::new(Vec::new());

/// Stops every recording in the process cleanly, before it exits.
pub fn finish_all() {
    let running = std::mem::take(&mut *RUNNING.lock());
    for recorder in running.iter().filter_map(Weak::upgrade) {
        recorder.stop();
    }
}

impl Recorder {
    /// Starts recording to `target`: a file, or a folder (existing, or named with a trailing
    /// separator) to make one in. An existing file is never written over: the name gets `-2`,
    /// `-3`, ... `options` goes in the header.
    pub fn start(target: &Path, from_start: bool, options: Value) -> io::Result<Arc<Self>> {
        let (path, file) = create(target)?;
        let started_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let header = Header {
            format: FORMAT,
            turu: crate::updates::current_version().to_owned(),
            started_unix_ms,
            from_world: !from_start,
            options,
            scrubbed: false,
        };
        let (tx, rx) = sync_channel(QUEUE);
        let dropped = Arc::new(AtomicU64::default());
        let pending = Arc::new(Mutex::new(BTreeSet::new()));
        let started = Instant::now();
        let writer = {
            let shared = Shared {
                dropped: dropped.clone(),
                pending: pending.clone(),
                started,
                path: path.clone(),
            };
            std::thread::Builder::new()
                .name("Turu Tape".into())
                .spawn(move || write_loop(rx, file, header, &shared))?
        };
        let recorder = Arc::new(Self {
            tx,
            path,
            started,
            next: AtomicU64::default(),
            dropped,
            pending,
            open: AtomicBool::new(from_start),
            stopped: AtomicBool::new(false),
            writer: Mutex::new(Some(writer)),
        });
        let mut running = RUNNING.lock();
        running.retain(|r| r.strong_count() > 0);
        running.push(Arc::downgrade(&recorder));
        Ok(recorder)
    }

    /// The file it writes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether lines are being taken yet (the player has joined a map, or it records from the
    /// start).
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    /// How many lines have been lost so far.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// A place for a call into Flash made now, whose line [`Self::call`] writes once it is
    /// answered; `None` while nothing is recorded.
    pub fn begin_call(&self) -> Option<Ticket> {
        let ticket = self.ticket()?;
        self.pending.lock().insert(ticket.n);
        Some(ticket)
    }

    /// A place for a line whose event begins now; `None` while nothing is recorded.
    fn ticket(&self) -> Option<Ticket> {
        if !self.is_open() || self.stopped.load(Ordering::Acquire) {
            return None;
        }
        Some(Ticket {
            n: self.next.fetch_add(1, Ordering::Relaxed),
            t: self.started.elapsed().as_millis() as u64,
        })
    }

    /// A call into Flash given `ticket` by [`Self::begin_call`], now answered.
    pub fn call(
        &self,
        ticket: Ticket,
        name: &str,
        args: &[FlashArg],
        result: Result<&str, String>,
        run: Option<Duration>,
    ) {
        let (answer, error) = match result {
            Ok(answer) => (Some(answer.to_owned()), None),
            Err(e) => (None, Some(e)),
        };
        self.pending.lock().remove(&ticket.n);
        self.send(Line::Call {
            n: ticket.n,
            t: ticket.t,
            thread: std::thread::current().name().map(str::to_owned),
            name: name.to_owned(),
            args: args.iter().map(arg_json).collect(),
            answer,
            error,
            run_ms: run.map(|d| d.as_secs_f64() * 1000.0),
        });
    }

    /// A call from `turu.swf`. Runs on the player's thread: it only queues. True when it opened
    /// the recording (the player joined a map).
    pub fn event(&self, name: &str, args: &[Value]) -> bool {
        let opened = !self.is_open()
            && args.iter().any(|a| a.as_str().is_some_and(joins_a_map))
            && self.open();
        let Some(Ticket { n, t }) = self.ticket() else {
            return opened;
        };
        self.send(Line::Event {
            n,
            t,
            name: name.to_owned(),
            args: args.to_vec(),
        });
        opened
    }

    /// A message of the game's socket. Runs on the player's thread: it only queues. True when
    /// it opened the recording.
    pub fn socket(&self, socket: u64, sent: bool, text: &str) -> bool {
        let opened = !sent && !self.is_open() && joins_a_map(text) && self.open();
        let Some(Ticket { n, t }) = self.ticket() else {
            return opened;
        };
        let text = text.to_owned();
        self.send(if sent {
            Line::Sent { n, t, socket, text }
        } else {
            Line::Received { n, t, socket, text }
        });
        opened
    }

    /// Opens the recording; true for the caller that did.
    fn open(&self) -> bool {
        self.open
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Something the bot did (`what`: "scriptStarted", ...).
    pub fn note(&self, what: &str, detail: Value) {
        let Some(Ticket { n, t }) = self.ticket() else {
            return;
        };
        self.send(Line::Note {
            n,
            t,
            what: what.to_owned(),
            detail,
        });
    }

    fn send(&self, line: Line) {
        match self.tx.try_send(Msg::Line(line)) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Stops recording: what is queued is written, then the end line, and the file is closed.
    /// Waits for the writer. Calling it again does nothing.
    pub fn stop(&self) {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        // Calls made before the stop get a moment to be answered; the end line lists the rest.
        let waited = Instant::now();
        while !self.pending.lock().is_empty() && waited.elapsed() < STOP_WAITS {
            std::thread::sleep(Duration::from_millis(5));
        }
        // Blocking is fine here (never the player's thread): the stop must not be dropped.
        let _ = self.tx.send(Msg::Stop);
        if let Some(writer) = self.writer.lock().take() {
            let _ = writer.join();
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A received message (or `pext`) that puts the player on a map: recording starts there.
fn joins_a_map(text: &str) -> bool {
    text.contains("\"moveToArea\"")
}

/// A Flash argument as JSON.
pub(crate) fn arg_json(arg: &FlashArg) -> Value {
    match arg {
        FlashArg::Null => Value::Null,
        FlashArg::Bool(b) => Value::Bool(*b),
        FlashArg::Number(n) => serde_json::Number::from_f64(*n).map_or(Value::Null, Value::Number),
        FlashArg::String(s) => Value::String(s.clone()),
        FlashArg::List(items) => Value::Array(items.iter().map(arg_json).collect()),
        FlashArg::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), arg_json(v))).collect())
        }
    }
}

/// Creates the tape's file, never over an existing one.
fn create(target: &Path) -> io::Result<(PathBuf, File)> {
    let folder = target.is_dir()
        || target
            .as_os_str()
            .to_string_lossy()
            .ends_with(std::path::is_separator);
    let first = if folder {
        std::fs::create_dir_all(target)?;
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        target.join(format!("turu-{secs}-{}{EXTENSION}", std::process::id()))
    } else {
        if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        target.to_owned()
    };
    for attempt in 1..1000 {
        let path = if attempt == 1 {
            first.clone()
        } else {
            numbered(&first, attempt)
        };
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no free tape file name",
    ))
}

/// `name.tape.jsonl.gz` -> `name-<n>.tape.jsonl.gz` (the number before the first dot).
fn numbered(path: &Path, n: u32) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let renamed = match name.split_once('.') {
        Some((stem, rest)) => format!("{stem}-{n}.{rest}"),
        None => format!("{name}-{n}"),
    };
    path.with_file_name(renamed)
}

/// What the writer shares with its recorder.
struct Shared {
    dropped: Arc<AtomicU64>,
    pending: Arc<Mutex<BTreeSet<u64>>>,
    started: Instant,
    path: PathBuf,
}

fn write_loop(rx: Receiver<Msg>, file: File, header: Header, shared: &Shared) {
    let Shared {
        dropped,
        pending,
        started,
        path,
    } = shared;
    let mut out = GzEncoder::new(BufWriter::new(file), Compression::default());
    let mut lines = 0;
    let mut last_flush = Instant::now();
    let mut result = write_json(&mut out, &header);
    while result.is_ok() {
        match rx.recv_timeout(FLUSH_EVERY) {
            Ok(Msg::Line(mut line)) => {
                redact::line(&mut line);
                result = write_json(&mut out, &line);
                lines += 1;
            }
            Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => {
                // What was queued behind the stop (a call answered meanwhile).
                while result.is_ok()
                    && let Ok(Msg::Line(mut line)) = rx.try_recv()
                {
                    redact::line(&mut line);
                    result = write_json(&mut out, &line);
                    lines += 1;
                }
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        if result.is_ok() && last_flush.elapsed() >= FLUSH_EVERY {
            // A sync flush: what is written so far can be read even if the process is killed.
            result = out.flush();
            last_flush = Instant::now();
        }
    }
    if let Err(e) = result {
        tracing::warn!(
            "Tape {}: couldn't write it ({e}); recording stopped",
            path.display()
        );
        // What arrives from now on is lost; the tape has no end line, so a reader knows.
        drop(rx);
        return;
    }
    let end = End {
        t: started.elapsed().as_millis() as u64,
        lines,
        dropped: dropped.load(Ordering::Relaxed),
        unanswered: pending.lock().iter().copied().collect(),
    };
    let finished = write_json(&mut out, &end)
        .and_then(|()| out.try_finish())
        .and_then(|()| out.get_mut().flush());
    match finished {
        Ok(()) => tracing::info!(
            "Tape {}: {lines} lines, {} dropped",
            path.display(),
            end.dropped
        ),
        Err(e) => tracing::warn!("Tape {}: couldn't finish it ({e})", path.display()),
    }
}

pub(super) fn write_json(out: &mut impl Write, value: &impl serde::Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *out, value).map_err(io::Error::other)?;
    out.write_all(b"\n")
}
