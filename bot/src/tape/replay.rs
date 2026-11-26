//! Plays a tape back to the bot offline (docs/REPLAY_TESTS_PROPOSAL.md, phase 2): a
//! [`ReplayHost`] answers the script's calls into Flash from the tape and delivers the game's
//! events and packets as they came, and fails where the script's actions stop matching the
//! tape's.
//!
//! Each call is one of three kinds ([`kind`]):
//!
//! - **Reads** ask the game something and change nothing. They answer state, not a count: the
//!   latest answer recorded for the same name and arguments at or before the tape's position,
//!   else the first one after it. Polling waits and background threads repeat reads as often as
//!   their timing makes them, so matching reads one for one would diverge at once.
//! - **Sends** act on the game. Each must equal the next send recorded from the same thread. The
//!   script thread's sends move the position: between two of them it walks at the recorded pace
//!   (scaled), delivering the events and packets it passes, and stops before the next one until
//!   the script makes it.
//! - **Noise**: the background timer's levers and switches (`setLever`, `killLag`, ...) and the
//!   sends a tape's [`ReplayOptions::noise`] names. Any number, in either run, answered as reads.
//!
//! Anything not known to be a read or noise is a send, so a getter missing from the lists fails
//! loudly, naming itself, rather than an action passing unchecked.

use super::{Line, Tape};
use crate::Bot;
use crate::host::{FlashArg, FlashHost, HostError};
use crate::player_packets::Direction;
use crate::script::ScriptStatus;
use parking_lot::Mutex;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

/// Callbacks that only read the game (docs/VIBESKUA_SPEC.md §2, turu.swf's `hostInfo` list).
const READS: &[&str] = &[
    "availableMonsters",
    "canUseSkill",
    "gender",
    "GetAuraIcon",
    "GetAuraSnapshots",
    "GetAurasValue",
    "GetMonsterAuraByID",
    "GetMonsterAuraByName",
    "GetPlayerAura",
    "HasAnyActiveAura",
    "getActiveModalMessage",
    "getArrayObject",
    "getAvatar",
    "getGameObject",
    "getGameObjectKey",
    "getGameObjectS",
    "getMany",
    "getMonsters",
    "getTargetMonster",
    "getUnmetPurchaseRequirements",
    "hasInventoryCategories",
    "hostInfo",
    "inventoryBagUsedSlots",
    "inventoryHasSpaceFor",
    "isFavoriteItem",
    "isKicked",
    "isLoggedIn",
    "isNull",
    "questStates",
    "selectArrayObjects",
    "selectArrayObjectsWhere",
    "snapshot",
];

/// Callbacks that switch the client's own behaviour, which the background timer re-applies as
/// often as its timing makes it: never checked.
const NOISE: &[&str] = &[
    "catchPackets",
    "disableDeathAd",
    "infiniteRange",
    "keepWalkSpeed",
    "killLag",
    "killLoginModals",
    "loadPolicyFile",
    "loadPolicyFiles",
    "magnetize",
    "modDisable",
    "modEnable",
    "rejectExcept",
    "resetConnectionState",
    "setBackgroundValues",
    "setFrameReport",
    "setLever",
    "setPacketCapture",
    "skipCutscenes",
    "untargetSelf",
];

/// Game functions (`callGameFunction`'s path, its last part) that only read, beyond those named
/// `is…`, `has…` and `can…`.
const READ_FUNCTIONS: &[&str] = &[
    "artLoaded",
    "getAchievement",
    "getCPByID",
    "getQuestValue",
    "getRep",
    "maximumQuestTurnIns",
];

/// What a call is to the replay (see the module's documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Read,
    Send,
    Noise,
}

/// What `name(args)` is, before a tape's own noise table.
pub fn kind(name: &str, args: &[Value]) -> Kind {
    if READS.contains(&name) {
        return Kind::Read;
    }
    if NOISE.contains(&name) {
        return Kind::Noise;
    }
    if matches!(name, "callGameFunction" | "callGameFunction0") {
        let function = args.first().and_then(Value::as_str).unwrap_or_default();
        let last = function.rsplit('.').next().unwrap_or(function);
        if READ_FUNCTIONS.contains(&last) || is_getter(last) {
            return Kind::Read;
        }
    }
    Kind::Send
}

fn is_getter(function: &str) -> bool {
    ["is", "has", "can"].iter().any(|prefix| {
        function
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
    })
}

/// How a send is named in [`ReplayOptions::noise`]: a game function by its path
/// (`world.rest`), a packet by its command (`packet:restRequest`, from `sendClientPacket` or
/// `sfc.sendString`), any other callback by its name.
pub fn label(name: &str, args: &[Value]) -> String {
    let first = args.first().and_then(Value::as_str).unwrap_or_default();
    match name {
        "callGameFunction" | "callGameFunction0" if first == "sfc.sendString" => {
            packet_label(args.get(1).and_then(Value::as_str).unwrap_or_default())
        }
        "callGameFunction" | "callGameFunction0" => first.to_owned(),
        "sendClientPacket" => packet_label(first),
        _ => name.to_owned(),
    }
}

fn packet_label(packet: &str) -> String {
    // `%xt%zm%restRequest%1%...`; a JSON packet names its command in `cmd`.
    let cmd = packet.split('%').nth(3).map(str::to_owned).or_else(|| {
        let value: Value = serde_json::from_str(packet).ok()?;
        let cmd = value.pointer("/b/o/cmd").or_else(|| value.get("cmd"))?;
        cmd.as_str().map(str::to_owned)
    });
    format!("packet:{}", cmd.unwrap_or_default())
}

/// How strictly, and how fast, a tape is played.
#[derive(Debug, Clone)]
pub struct ReplayOptions {
    /// A read the tape never answered fails the replay (else it answers `"null"`, counted in
    /// [`ReplayReport::missing_reads`]). Also refuses a tape with lines missing or cut short.
    pub strict: bool,
    /// The recorded delays between events, multiplied: 1.0 replays them as they came, 0.0 at
    /// once.
    pub delay_scale: f64,
    /// Sends ([`label`]s) that may come any number of times in either run (`world.rest`,
    /// `packet:restRequest`).
    pub noise: Vec<String>,
    /// The thread whose sends move the position: the script's.
    pub script_thread: String,
    /// How long past the recorded time the script may take to make its next send before the
    /// replay counts it as stuck.
    pub stall: Duration,
    /// Plays a tape the scrubber hasn't renamed the players in (a test's own recording). Never
    /// for a committed tape.
    pub allow_unscrubbed: bool,
}

impl Default for ReplayOptions {
    fn default() -> Self {
        Self {
            strict: true,
            delay_scale: 1.0,
            noise: Vec::new(),
            script_thread: "Script Thread".into(),
            stall: Duration::from_secs(10),
            allow_unscrubbed: false,
        }
    }
}

/// How a replay that reached the tape's end went.
#[derive(Debug, Clone)]
pub struct ReplayReport {
    /// The script's sends matched.
    pub sends: usize,
    /// Other threads' sends matched.
    pub background_sends: usize,
    /// Reads the tape had no answer for (each once), answered `"null"`: always empty in strict
    /// mode.
    pub missing_reads: Vec<String>,
    /// The script's status when it ended.
    pub status: ScriptStatus,
}

/// Why a replay failed: what was expected, what came, and what led to it.
#[derive(Debug, Clone)]
pub struct ReplayFailure {
    pub message: String,
}

impl std::fmt::Display for ReplayFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ReplayFailure {}

/// A [`FlashHost`] that plays a tape back (see the module's documentation).
pub struct ReplayHost {
    tape: Tape,
    options: ReplayOptions,
    /// Each read's (and noise's) recorded answers by [`key`]: the lines' indexes, in order.
    answers: HashMap<String, Vec<usize>>,
    /// The script thread's sends, the lines' indexes.
    script_sends: Vec<usize>,
    /// Every other thread's sends.
    thread_sends: HashMap<String, Vec<usize>>,
    /// When the script thread first called on the tape.
    first_script_t: u64,
    /// The tape has the socket's messages (the player reported them).
    socket: bool,
    state: Mutex<State>,
    /// Held while lines are taken and delivered, so they reach the bot in order.
    delivering: Mutex<()>,
    bot: Mutex<Weak<Bot>>,
}

struct State {
    /// Lines before this one have been passed (delivered, or visible to reads).
    pos: usize,
    /// Where the position stops: the script's next send, else the tape's end.
    limit: usize,
    /// The script's next send, in `script_sends`.
    next_send: usize,
    /// Each other thread's next send, in its list.
    thread_next: HashMap<String, usize>,
    /// When the position last moved to a send, and that line's time on the tape.
    anchor: (Instant, u64),
    /// The script has made its first call: the stall check starts then, not while it compiles.
    script_began: bool,
    background_sends: usize,
    missing_reads: Vec<String>,
    failure: Option<String>,
}

impl ReplayHost {
    /// Reads the tape at `path` (see [`ReplayHost::new`]).
    pub fn open(path: &Path, options: ReplayOptions) -> Result<Arc<Self>, String> {
        let tape = super::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::new(tape, options)
    }

    /// Refuses a tape the scrubber hasn't marked (unless allowed), and in strict mode one with
    /// lines missing or cut short.
    pub fn new(tape: Tape, options: ReplayOptions) -> Result<Arc<Self>, String> {
        if !tape.header.scrubbed && !options.allow_unscrubbed {
            return Err(
                "the tape isn't scrubbed of player names: run `turu-tape scrub` on it first".into(),
            );
        }
        if options.strict && (!tape.is_complete() || tape.truncated) {
            return Err(format!(
                "the tape isn't whole (missing lines {:?}, end line {}, cut short {}): record it again",
                tape.missing,
                if tape.end.is_some() {
                    "present"
                } else {
                    "absent"
                },
                tape.truncated
            ));
        }
        let mut answers: HashMap<String, Vec<usize>> = HashMap::new();
        let mut script_sends = Vec::new();
        let mut thread_sends: HashMap<String, Vec<usize>> = HashMap::new();
        let mut socket = false;
        for (i, line) in tape.lines.iter().enumerate() {
            match line {
                Line::Call {
                    thread, name, args, ..
                } => match Self::kind_of(&options, name, args) {
                    Kind::Read | Kind::Noise => answers.entry(key(name, args)).or_default().push(i),
                    Kind::Send => {
                        let thread = thread.as_deref().unwrap_or_default();
                        if thread == options.script_thread {
                            script_sends.push(i);
                        } else {
                            thread_sends.entry(thread.to_owned()).or_default().push(i);
                        }
                    }
                },
                Line::Sent { .. } | Line::Received { .. } => socket = true,
                Line::Event { .. } | Line::Note { .. } => {}
            }
        }
        let first_script_t = tape
            .lines
            .iter()
            .find(
                |l| matches!(l, Line::Call { thread: Some(t), .. } if *t == options.script_thread),
            )
            .map_or(0, line_t);
        let limit = script_sends.first().copied().unwrap_or(tape.lines.len());
        let start_t = tape.lines.first().map_or(0, line_t);
        Ok(Arc::new(Self {
            state: Mutex::new(State {
                pos: 0,
                limit,
                next_send: 0,
                thread_next: HashMap::new(),
                anchor: (Instant::now(), start_t),
                script_began: false,
                background_sends: 0,
                missing_reads: Vec::new(),
                failure: None,
            }),
            tape,
            options,
            answers,
            script_sends,
            thread_sends,
            first_script_t,
            socket,
            delivering: Mutex::new(()),
            bot: Mutex::new(Weak::new()),
        }))
    }

    fn kind_of(options: &ReplayOptions, name: &str, args: &[Value]) -> Kind {
        match kind(name, args) {
            Kind::Send if options.noise.contains(&label(name, args)) => Kind::Noise,
            kind => kind,
        }
    }

    /// Makes this `bot`'s game: the tape's options (`ActionDelay`, ...) and host SWF, and the
    /// lines before its script started delivered. Then load the script and start it.
    pub fn attach(self: &Arc<Self>, bot: &Arc<Bot>) {
        if let Ok(options) = serde_json::from_value(self.tape.header.options.clone()) {
            *bot.options.write() = options;
        }
        *self.bot.lock() = Arc::downgrade(bot);
        bot.set_host(Some(self.clone()));
        if self.host_info().is_some() {
            bot.flash.ask_host_info();
        }
        // What happened before the script started is the world it starts in.
        let started = self
            .tape
            .lines
            .iter()
            .position(|l| matches!(l, Line::Note { what, .. } if what == "scriptStarted"));
        if let Some(started) = started {
            let _order = self.delivering.lock();
            let lines = {
                let mut state = self.state.lock();
                let to = started.min(state.limit);
                let lines = self.pass(&mut state, to);
                state.anchor = (Instant::now(), line_t(&self.tape.lines[started]));
                lines
            };
            self.deliver(bot, &lines);
        }
        let weak = Arc::downgrade(self);
        let spawned = std::thread::Builder::new()
            .name("Turu Replay".into())
            .spawn(move || {
                while let Some(host) = weak.upgrade() {
                    let Some(bot) = host.bot.lock().upgrade() else {
                        return;
                    };
                    if !host.walk(&bot) {
                        return;
                    }
                    drop((host, bot));
                    std::thread::sleep(Duration::from_millis(5));
                }
            });
        if let Err(e) = spawned {
            self.fail(format!("couldn't start the replay's thread: {e}"));
        }
    }

    /// Waits for the script to end, at most `timeout`, then checks it reached the tape's end.
    /// Stops the script on a failure.
    pub fn wait(&self, bot: &Arc<Bot>, timeout: Duration) -> Result<ReplayReport, ReplayFailure> {
        let started = Instant::now();
        loop {
            if let Some(failure) = self.failure() {
                return Err(self.stop_with(bot, failure));
            }
            if !bot.scripts.is_running() {
                break;
            }
            if let Some(stuck) = self.stuck() {
                self.fail(stuck);
            } else if started.elapsed() > timeout {
                self.fail(format!("the script ran past the test's {timeout:?}"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(failure) = self.failure() {
            return Err(self.failure_with_log(bot, failure));
        }
        let state = self.state.lock();
        if let Some(&next) = self.script_sends.get(state.next_send) {
            let message = format!(
                "the script ended ({:?}) before the tape: its send {} of {} never came\n  expected: {}",
                bot.scripts.status(),
                state.next_send + 1,
                self.script_sends.len(),
                render(&self.tape.lines[next]),
            );
            let message = self.context(&state, next, message);
            drop(state);
            return Err(self.failure_with_log(bot, message));
        }
        Ok(ReplayReport {
            sends: state.next_send,
            background_sends: state.background_sends,
            missing_reads: state.missing_reads.clone(),
            status: bot.scripts.status(),
        })
    }

    /// The first failure, if the replay has failed.
    pub fn failure(&self) -> Option<String> {
        self.state.lock().failure.clone()
    }

    /// What `hostInfo` answered when it was recorded: its call's answer, else the recording's
    /// note.
    fn host_info(&self) -> Option<String> {
        self.tape.lines.iter().find_map(|line| match line {
            Line::Call {
                name,
                answer: Some(answer),
                ..
            } if name == "hostInfo" => Some(answer.clone()),
            Line::Note { what, detail, .. } if what == "hostInfo" && !detail.is_null() => {
                Some(detail.to_string())
            }
            _ => None,
        })
    }

    /// A message when the script is past the time it took on the tape to make its next send,
    /// and the stall's margin. That time isn't scaled: `ActionDelay` and the script's sleeps
    /// aren't.
    fn stuck(&self) -> Option<String> {
        let state = self.state.lock();
        if !state.script_began {
            return None;
        }
        let &next = self.script_sends.get(state.next_send)?;
        let (at, from) = state.anchor;
        let gap = line_t(&self.tape.lines[next]).saturating_sub(from);
        let due = at + Duration::from_millis(gap) + self.options.stall;
        if Instant::now() < due {
            return None;
        }
        let message = format!(
            "the script is stuck: its send {} of {} didn't come within {:?} of when it came on \
             the tape\n  expected: {}",
            state.next_send + 1,
            self.script_sends.len(),
            self.options.stall,
            render(&self.tape.lines[next]),
        );
        Some(self.context(&state, next, message))
    }

    fn stop_with(&self, bot: &Arc<Bot>, failure: String) -> ReplayFailure {
        crate::script::stop(bot);
        let until = Instant::now() + Duration::from_secs(5);
        while bot.scripts.is_running() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        self.failure_with_log(bot, failure)
    }

    fn failure_with_log(&self, bot: &Bot, message: String) -> ReplayFailure {
        let log = bot.logs.script.text();
        let lines: Vec<&str> = log.lines().collect();
        let tail = &lines[lines.len().saturating_sub(20)..];
        let mut message = message;
        message.push_str("\nscript log (its end):\n");
        for line in tail {
            let _ = writeln!(message, "  {line}");
        }
        ReplayFailure { message }
    }

    /// Records the replay's first failure; every call fails from then on.
    fn fail(&self, message: String) {
        let mut state = self.state.lock();
        state.failure.get_or_insert(message);
    }

    /// When the line recorded at `t` comes in this replay.
    fn paced(&self, state: &State, t: u64) -> Instant {
        let (at, from) = state.anchor;
        let gap = t.saturating_sub(from) as f64 * self.options.delay_scale.max(0.0);
        at + Duration::from_secs_f64(gap / 1000.0)
    }

    /// Moves the position up to `to`, returning the lines to deliver.
    fn pass(&self, state: &mut State, to: usize) -> Vec<Line> {
        let from = state.pos.min(to);
        state.pos = state.pos.max(to);
        self.tape.lines[from..to]
            .iter()
            .filter(|l| {
                matches!(
                    l,
                    Line::Event { .. } | Line::Sent { .. } | Line::Received { .. }
                )
            })
            .cloned()
            .collect()
    }

    /// Walks the position at the recorded pace up to where it stops, delivering what it passes.
    /// False once there is nothing more to walk: the replay failed or passed the tape's end.
    fn walk(&self, bot: &Arc<Bot>) -> bool {
        let _order = self.delivering.lock();
        let lines = {
            let mut state = self.state.lock();
            if state.failure.is_some() || state.pos >= self.tape.lines.len() {
                return false;
            }
            let now = Instant::now();
            let mut to = state.pos;
            while to < state.limit && self.paced(&state, line_t(&self.tape.lines[to])) <= now {
                to += 1;
            }
            self.pass(&mut state, to)
        };
        self.deliver(bot, &lines);
        true
    }

    fn deliver(&self, bot: &Arc<Bot>, lines: &[Line]) {
        for line in lines {
            match line {
                Line::Event { name, args, .. } => bot.on_flash_call(name, args.clone()),
                Line::Sent { socket, text, .. } => {
                    bot.on_socket(*socket, Direction::Sent, &nul_ended(text));
                }
                Line::Received { socket, text, .. } => {
                    bot.on_socket(*socket, Direction::Received, &nul_ended(text));
                }
                Line::Call { .. } | Line::Note { .. } => {}
            }
        }
    }

    /// A read's (or noise's) answer: the latest recorded at or before the position, else the
    /// first after it.
    fn read(&self, name: &str, args: &[Value]) -> Result<String, HostError> {
        let key = key(name, args);
        let at = self.state.lock().pos;
        let Some(lines) = self.answers.get(&key) else {
            if name == "hostInfo"
                && let Some(info) = self.host_info()
            {
                return Ok(info);
            }
            return self.missing(name, args, key);
        };
        let before = lines.partition_point(|&i| i < at);
        let i = if before > 0 {
            lines[before - 1]
        } else {
            lines[0]
        };
        answer_of(&self.tape.lines[i])
    }

    fn missing(&self, name: &str, args: &[Value], key: String) -> Result<String, HostError> {
        if Self::kind_of(&self.options, name, args) == Kind::Noise {
            return Ok(String::new());
        }
        let mut state = self.state.lock();
        if self.options.strict {
            let message = format!(
                "a read the tape never answered: {}",
                render_call(None, name, args)
            );
            let pos = state.pos;
            let message = self.context(&state, pos, message);
            state.failure.get_or_insert(message);
            return Err(HostError::Closed);
        }
        if !state.missing_reads.contains(&key) {
            state.missing_reads.push(key);
        }
        Ok("null".into())
    }

    /// A send: checked against the next one recorded from its thread.
    fn send(&self, name: &str, args: &[Value]) -> Result<String, HostError> {
        let thread = std::thread::current().name().unwrap_or_default().to_owned();
        let _order = self.delivering.lock();
        let mut state = self.state.lock();
        if thread != self.options.script_thread {
            let next = state.thread_next.get(&thread).copied().unwrap_or(0);
            let expected = self.thread_sends.get(&thread).and_then(|s| s.get(next));
            return match expected {
                Some(&i) if same_send(&self.tape.lines[i], name, args) => {
                    state.thread_next.insert(thread, next + 1);
                    state.background_sends += 1;
                    answer_of(&self.tape.lines[i])
                }
                expected => {
                    let message =
                        mismatch(&thread, expected.map(|&i| &self.tape.lines[i]), name, args);
                    let pos = state.pos;
                    let message = self.context(&state, pos, message);
                    state.failure.get_or_insert(message);
                    Err(HostError::Closed)
                }
            };
        }
        let expected = self.script_sends.get(state.next_send).copied();
        match expected {
            Some(i) if same_send(&self.tape.lines[i], name, args) => {
                // What came before the send on the tape reaches the bot first.
                let lines = self.pass(&mut state, i);
                state.pos = i + 1;
                state.next_send += 1;
                state.limit = self
                    .script_sends
                    .get(state.next_send)
                    .copied()
                    .unwrap_or(self.tape.lines.len());
                state.anchor = (Instant::now(), line_t(&self.tape.lines[i]));
                drop(state);
                if let Some(bot) = self.bot.lock().upgrade() {
                    self.deliver(&bot, &lines);
                }
                answer_of(&self.tape.lines[i])
            }
            expected => {
                let at = expected.unwrap_or(self.tape.lines.len());
                let mut message =
                    mismatch(&thread, expected.map(|i| &self.tape.lines[i]), name, args);
                let _ = write!(
                    message,
                    "\n  (the script's send {} of {})",
                    state.next_send + 1,
                    self.script_sends.len()
                );
                let message = self.context(&state, at, message);
                state.failure.get_or_insert(message);
                Err(HostError::Closed)
            }
        }
    }

    /// `message`, with where on the tape it happened and the ten lines before it.
    fn context(&self, state: &State, at: usize, message: String) -> String {
        let mut out = format!(
            "replay failed at tape line {} of {} (position {}):\n{message}\nthe tape before it:\n",
            at,
            self.tape.lines.len(),
            state.pos
        );
        let from = at.saturating_sub(10);
        for line in &self.tape.lines[from..at.min(self.tape.lines.len())] {
            let _ = writeln!(out, "  {}", render(line));
        }
        out
    }
}

impl FlashHost for ReplayHost {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        {
            let mut state = self.state.lock();
            if state.failure.is_some() {
                return Err(HostError::Closed);
            }
            if !state.script_began
                && std::thread::current().name() == Some(self.options.script_thread.as_str())
            {
                state.script_began = true;
                state.anchor = (Instant::now(), self.first_script_t);
            }
        }
        let args: Vec<Value> = args.iter().map(super::recorder::arg_json).collect();
        match Self::kind_of(&self.options, name, &args) {
            Kind::Read | Kind::Noise => self.read(name, &args),
            Kind::Send => self.send(name, &args),
        }
    }

    fn player_packets(&self) -> bool {
        self.socket
    }
}

/// A call's key among the recorded answers: its name and arguments.
fn key(name: &str, args: &[Value]) -> String {
    format!("{name}{}", Value::from(args.to_vec()))
}

fn line_t(line: &Line) -> u64 {
    match line {
        Line::Call { t, .. }
        | Line::Event { t, .. }
        | Line::Sent { t, .. }
        | Line::Received { t, .. }
        | Line::Note { t, .. } => *t,
    }
}

fn answer_of(line: &Line) -> Result<String, HostError> {
    match line {
        Line::Call {
            answer: Some(answer),
            ..
        } => Ok(answer.clone()),
        // It failed when it was recorded.
        Line::Call { .. } => Err(HostError::Timeout),
        _ => Ok(String::new()),
    }
}

/// Whether `name(args)` is the send recorded on `line`. A redacted argument matches anything.
fn same_send(line: &Line, name: &str, args: &[Value]) -> bool {
    let Line::Call {
        name: recorded,
        args: recorded_args,
        ..
    } = line
    else {
        return false;
    };
    recorded == name
        && recorded_args.len() == args.len()
        && recorded_args
            .iter()
            .zip(args)
            .all(|(want, have)| want == have || want == super::redact::REDACTED)
}

fn mismatch(thread: &str, expected: Option<&Line>, name: &str, args: &[Value]) -> String {
    let expected = expected.map_or_else(|| "nothing: the tape has no more".into(), render);
    format!(
        "a send that doesn't match the tape's\n  expected: {expected}\n  actual:   {}",
        render_call(Some(thread), name, args)
    )
}

fn render_call(thread: Option<&str>, name: &str, args: &[Value]) -> String {
    let args: Vec<String> = args.iter().map(show).collect();
    match thread {
        Some(thread) => format!("[{thread}] {name}({})", args.join(", ")),
        None => format!("{name}({})", args.join(", ")),
    }
}

/// An argument as a script would write it: whole numbers without `.0` (Flash's numbers are
/// doubles, so the tape has `4777.0`).
fn show(arg: &Value) -> String {
    match arg.as_f64() {
        Some(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", n as i64),
        _ => arg.to_string(),
    }
}

/// A tape line on one line, for failure messages.
fn render(line: &Line) -> String {
    let cut = |s: &str| -> String {
        let mut chars = s.chars();
        let head: String = chars.by_ref().take(160).collect();
        if chars.next().is_some() {
            format!("{head}…")
        } else {
            head
        }
    };
    match line {
        Line::Call {
            n,
            t,
            thread,
            name,
            args,
            answer,
            error,
            ..
        } => {
            let call = render_call(Some(thread.as_deref().unwrap_or("?")), name, args);
            let answer = answer.as_deref().map_or_else(
                || format!("failed: {}", error.as_deref().unwrap_or_default()),
                cut,
            );
            format!("#{n} {t}ms call {} -> {answer}", cut(&call))
        }
        Line::Event { n, t, name, args } => {
            format!(
                "#{n} {t}ms event {name} {}",
                cut(&Value::from(args.clone()).to_string())
            )
        }
        Line::Sent { n, t, text, .. } => format!("#{n} {t}ms out {}", cut(text)),
        Line::Received { n, t, text, .. } => format!("#{n} {t}ms in {}", cut(text)),
        Line::Note { n, t, what, .. } => format!("#{n} {t}ms note {what}"),
    }
}

fn nul_ended(text: &str) -> Vec<u8> {
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_sends_and_noise() {
        assert_eq!(
            kind("getGameObject", &[json!("world.strMapName")]),
            Kind::Read
        );
        assert_eq!(kind("setLever", &[json!("x"), json!(true)]), Kind::Noise);
        assert_eq!(kind("buyItemByID", &[json!(1)]), Kind::Send);
        assert_eq!(
            kind(
                "callGameFunction",
                &[json!("world.isQuestInProgress"), json!(1)]
            ),
            Kind::Read
        );
        assert_eq!(
            kind(
                "callGameFunction0",
                &[json!("world.myAvatar.pMC.artLoaded")]
            ),
            Kind::Read
        );
        assert_eq!(
            kind("callGameFunction", &[json!("world.getMapItem"), json!(1)]),
            Kind::Send
        );
        assert_eq!(
            kind("callGameFunction", &[json!("world.island"), json!(1)]),
            Kind::Send,
            "`is` must start a word"
        );
        // An unknown callback is a send: it fails loudly if it was a getter.
        assert_eq!(kind("somethingNew", &[]), Kind::Send);
    }

    #[test]
    fn labels_name_game_functions_and_packet_commands() {
        assert_eq!(
            label("callGameFunction0", &[json!("world.rest")]),
            "world.rest"
        );
        assert_eq!(
            label(
                "sendClientPacket",
                &[json!("%xt%zm%restRequest%1%%"), json!("str")]
            ),
            "packet:restRequest"
        );
        assert_eq!(
            label(
                "callGameFunction",
                &[
                    json!("sfc.sendString"),
                    json!("%xt%zm%moveToCell%1%Enter%Spawn%")
                ]
            ),
            "packet:moveToCell"
        );
        assert_eq!(label("walkTo", &[json!(1), json!(2)]), "walkTo");
    }
}
