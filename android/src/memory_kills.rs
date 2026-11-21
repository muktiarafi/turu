//! A slot that comes back after Android closed it for memory (`turu_frontend::memory_kills` has
//! the rules and why): how its process last ended comes from `SlotService` (Android's exit
//! record for `:c<N>`), [`at_start`] counts it in `crash/memory-kills-c<N>.log` and decides
//! whether the slot logs in and runs its script again, waits a little for memory first
//! ([`wait_for_memory`]), or is held: at the login screen with its script loaded and not started,
//! saying why in its notification and the shell's tab list, until Start again ([`start_again`]).

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use turu_frontend::memory_kills::{self, Kills, Start};

/// How this slot's process last ended (`SlotService`): Android's timestamp (ms), reason and
/// status; a time of 0 when there's no record.
static LAST_EXIT: OnceLock<(i64, i32, i32)> = OnceLock::new();
/// Held after this many kills for memory (0: not held).
static HELD: AtomicUsize = AtomicUsize::new(0);

/// The exit status of a slot started again by hand: Android starts it again (`START_STICKY`), and
/// `CrashReports.kt` doesn't report it.
const EXIT_START_AGAIN: i32 = 87;
/// After a kill for memory, the slot waits at most this long for the phone's memory to come back
/// over `memory.rs`'s low mark before it logs in: under the 30 s the shell gives a starting slot
/// to let go of the surface (`slots.rs`, `DETACH_LIMIT`).
const MEMORY_WAIT: Duration = Duration::from_secs(20);

/// How the slot's process last ended, from `SlotService`, before the engine starts.
pub(crate) fn set_last_exit(time: i64, reason: i32, status: i32) {
    let _ = LAST_EXIT.set((time, reason, status));
}

/// Held after this many kills for memory, if the slot is.
pub(crate) fn held() -> Option<usize> {
    Some(HELD.load(Ordering::Relaxed)).filter(|kills| *kills > 0)
}

/// What a held slot says (its notification, the shell's tab list), if it's held.
pub(crate) fn held_text() -> Option<String> {
    held().map(memory_kills::held_text)
}

/// How slot `slot` starts, counting a kill for memory that ended its process last.
/// `any_kill`: `--test-memory-kills`, every `SIGKILL` counts.
pub(crate) fn at_start(slot: u32, any_kill: bool) -> Start {
    let Some(dir) = crate::CRASH_DIR.get() else {
        return Start::Plain;
    };
    let (time, reason, status) = LAST_EXIT.get().copied().unwrap_or_default();
    let kill = (time > 0
        && memory_kills::memory_kill(reason, status, crate::memory::phone_low(), any_kill))
    .then_some(time as u64 / 1000);
    let path = memory_kills::file(dir, slot);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let before = Kills::parse(&text);
    let mut kills = before.clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let start = memory_kills::at_start(&mut kills, kill, now);
    if kills != before {
        let kept = if kills == Kills::default() {
            std::fs::remove_file(&path).or_else(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(e),
            })
        } else {
            std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&path, kills.to_text()))
        };
        if let Err(e) = kept {
            tracing::warn!("Slot {slot}: couldn't keep its kills for memory: {e}");
        }
    }
    let minutes = memory_kills::WINDOW.as_secs() / 60;
    match start {
        Start::Plain => {}
        Start::AfterKill { kills } => tracing::info!(
            "Slot {slot}: Android closed it for memory ({kills} of {} times in {minutes} minutes before it's held); it starts again",
            memory_kills::RESTARTS
        ),
        Start::Held { kills } => {
            HELD.store(kills, Ordering::Relaxed);
            tracing::warn!(
                "Slot {slot}: Android closed it for memory {kills} times in {minutes} minutes: it stays at the login screen, its script not started, until it's started again"
            );
            turu_frontend::client_link::set_held(Some(memory_kills::held_text(kills)));
        }
    }
    start
}

/// After a kill for memory: waits up to [`MEMORY_WAIT`] while the phone is still low on memory,
/// before the engine loads the game and logs in. On the engine thread, before the player exists.
pub(crate) fn wait_for_memory(slot: u32) {
    if !crate::memory::phone_low() {
        return;
    }
    tracing::info!(
        "Slot {slot}: the phone is still low on memory; waiting up to {} s before logging in",
        MEMORY_WAIT.as_secs()
    );
    let started = std::time::Instant::now();
    while crate::memory::phone_low() && started.elapsed() < MEMORY_WAIT {
        std::thread::sleep(Duration::from_secs(2));
    }
    tracing::info!(
        "Slot {slot}: waited {} s for memory (still low: {})",
        started.elapsed().as_secs(),
        crate::memory::phone_low()
    );
}

/// While the slot is held: once its tab is in game or runs a script anyway (logged in by hand on
/// the game's screen, say), the hold is let go, its kills forgotten, and its notification and tab
/// row go back to the usual. Checked every 2 s from the engine's start.
pub(crate) fn watch_hold() {
    let Some(slot) = crate::slots::slot().filter(|_| held().is_some()) else {
        return;
    };
    let spawned = std::thread::Builder::new()
        .name("Turu Memory Hold".into())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let bot = turu_frontend::turu::bot();
                let in_game = bot.state.lock().in_game;
                if in_game || bot.scripts.is_running() {
                    break;
                }
            }
            HELD.store(0, Ordering::Relaxed);
            turu_frontend::client_link::set_held(None);
            if let Some(dir) = crate::CRASH_DIR.get() {
                memory_kills::clear(dir, slot);
            }
            tracing::info!("Slot {slot}: in game or running a script again; no longer held after kills for memory");
        });
    if let Err(e) = spawned {
        tracing::warn!("Slot {slot}: couldn't watch its hold after kills for memory: {e}");
    }
}

/// Start again (the notification's, the shell's tab list's): a held slot forgets its kills and
/// its process ends, and Android starts it again as after any end it didn't ask for, logged in from
/// its record and with its script (`restart.rs`). Nothing while it isn't held.
pub(crate) fn start_again() {
    let Some(slot) = crate::slots::slot() else {
        return;
    };
    if held().is_none() {
        return;
    }
    if let Some(dir) = crate::CRASH_DIR.get() {
        memory_kills::clear(dir, slot);
    }
    tracing::info!(
        "Slot {slot}: started again by hand after kills for memory; its process ends to start again"
    );
    std::process::exit(EXIT_START_AGAIN);
}
