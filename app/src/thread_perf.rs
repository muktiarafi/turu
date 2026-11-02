//! Turu: where the event loop thread's time goes, for `TURU_PERF_LOG`: waiting for work, and
//! drawing the window in its parts. Every tab's game runs and every bot call is answered on
//! this thread, so a call that waits far longer than it runs (`turu_bot::perf`) waits behind
//! these.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum Phase {
    /// Nothing to do: from `about_to_wait` until the next event.
    Idle,
    /// A whole redraw: the movie and the window.
    Draw,
    /// Drawing the movie (`Player::render`, its GPU submit included).
    Movie,
    /// Getting the window's next picture to draw into (`get_current_texture`): blocks while
    /// every picture is still on screen or queued for it.
    Acquire,
    /// Building the window's UI (egui: the tool windows, Turu's included).
    Ui,
    /// Submitting the window's commands and presenting the picture.
    Present,
    /// A hidden tab drawing its movie, which only `TURU_GRID_TEST` does.
    HiddenDraw,
}

const PHASES: usize = 7;
static MICROS: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];
static COUNTS: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];

thread_local! {
    static IDLE_SINCE: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub fn add(phase: Phase, took: Duration) {
    MICROS[phase as usize].fetch_add(took.as_micros() as u64, Ordering::Relaxed);
    COUNTS[phase as usize].fetch_add(1, Ordering::Relaxed);
}

/// The event loop goes to sleep (end of `about_to_wait`).
pub fn going_idle() {
    IDLE_SINCE.set(Some(Instant::now()));
}

/// The event loop woke up (start of `new_events`).
pub fn woke() {
    if let Some(since) = IDLE_SINCE.take() {
        add(Phase::Idle, since.elapsed());
    }
}

pub struct Snapshot {
    at: Instant,
    micros: [u64; PHASES],
    counts: [u64; PHASES],
    /// The renderer's wait for the GPU, part of drawing the movie (ruffle-aqw keeps at most two
    /// frames queued for the GPU).
    gpu_wait_micros: u64,
}

pub fn snapshot() -> Snapshot {
    use ruffle_render_wgpu::perf_counters::timers;
    Snapshot {
        at: Instant::now(),
        micros: std::array::from_fn(|i| MICROS[i].load(Ordering::Relaxed)),
        counts: std::array::from_fn(|i| COUNTS[i].load(Ordering::Relaxed)),
        gpu_wait_micros: timers::snapshot()[timers::GPU_WAIT],
    }
}

impl Snapshot {
    /// One line on the time since `earlier`, per second: idle, drawing and its parts (with the
    /// average per draw for getting the picture and presenting it), the tabs' games and bot
    /// calls (`game_ms`, from the bots' stats), and the rest (events, the UI's own work).
    pub fn report(&self, earlier: &Snapshot, game_ms: f64) -> String {
        let secs = self.at.duration_since(earlier.at).as_secs_f64().max(0.001);
        let ms = |phase: Phase| (self.micros[phase as usize] - earlier.micros[phase as usize]) as f64 / 1000.0;
        let per_sec = |phase: Phase| ms(phase) / secs;
        let count = |phase: Phase| self.counts[phase as usize] - earlier.counts[phase as usize];
        let draws = count(Phase::Draw);
        let per_draw = |phase: Phase| if draws > 0 { ms(phase) / draws as f64 } else { 0.0 };
        let rest = (1000.0 - per_sec(Phase::Idle) - per_sec(Phase::Draw) - per_sec(Phase::HiddenDraw) - game_ms / secs)
            .max(0.0);
        let hidden_draws = count(Phase::HiddenDraw);
        let hidden = if hidden_draws > 0 {
            format!(
                "; hidden tabs drawing {:.0} ms/s, {:.1} draws/s [{:.2} ms/draw]",
                per_sec(Phase::HiddenDraw),
                hidden_draws as f64 / secs,
                ms(Phase::HiddenDraw) / hidden_draws as f64
            )
        } else {
            String::new()
        };
        format!(
            "main thread: idle {:.0} ms/s; drawing {:.0} ms/s, {:.1} draws/s (movie {:.0} of which GPU wait {:.0}, \
             next picture {:.0} [{:.2} ms/draw], UI {:.0}, submit+present {:.0} [{:.2} ms/draw]){hidden}; games \
             and bot calls {:.0} ms/s; rest {:.0} ms/s",
            per_sec(Phase::Idle),
            per_sec(Phase::Draw),
            draws as f64 / secs,
            per_sec(Phase::Movie),
            (self.gpu_wait_micros - earlier.gpu_wait_micros) as f64 / 1000.0 / secs,
            per_sec(Phase::Acquire),
            per_draw(Phase::Acquire),
            per_sec(Phase::Ui),
            per_sec(Phase::Present),
            per_draw(Phase::Present),
            game_ms / secs,
            rest,
        )
    }
}
