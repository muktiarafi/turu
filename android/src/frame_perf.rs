//! Where the engine thread's time goes while there's a window, for `--perf-log` (the desktop's
//! `thread_perf` with Mugetsu's per-frame render timers): the game's frames, rendering the game,
//! Turu's UI, getting the window's next picture and presenting it. The game, its drawing, the UI
//! and every bot call share this one thread, so time spent drawing is time the game and the
//! bot's calls wait.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub enum Phase {
    /// A tick that ran game frames (`Player::tick`).
    Frames,
    /// A whole screen draw (the parts below and the rest: blit, egui's buffers).
    Draw,
    /// Rendering the game into its texture (`Player::render`, its GPU submit included).
    Render,
    /// Running and tessellating Turu's UI (egui).
    Ui,
    /// Getting the window's next picture (`get_current_texture`): blocks while every picture is
    /// on screen or queued for it.
    Acquire,
    /// Submitting the screen's commands and presenting.
    Present,
}

const PHASES: usize = 6;
static MICROS: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];
static COUNTS: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];
static MAX_MICROS: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];
/// Draws skipped because the game was behind (`Screen::behind`).
static SKIPPED: AtomicU64 = AtomicU64::new(0);
/// Game frames run (a tick can run more than one when it catches up).
static GAME_FRAMES: AtomicU64 = AtomicU64::new(0);
/// Gaps between ticks that ran frames of 1.5 frame times or more: a stutter one can see.
static HITCHES: AtomicU64 = AtomicU64::new(0);
static WORST_GAP_MICROS: AtomicU64 = AtomicU64::new(0);
static LAST_FRAMES_AT: Mutex<Option<Instant>> = Mutex::new(None);

pub fn skipped() {
    SKIPPED.fetch_add(1, Ordering::Relaxed);
}

pub fn add(phase: Phase, took: Duration) {
    let micros = took.as_micros() as u64;
    MICROS[phase as usize].fetch_add(micros, Ordering::Relaxed);
    COUNTS[phase as usize].fetch_add(1, Ordering::Relaxed);
    MAX_MICROS[phase as usize].fetch_max(micros, Ordering::Relaxed);
}

/// A tick ran `frames` game frames, taking `took`; `frame_time` is the game's frame interval.
pub fn frames(frames: u64, took: Duration, frame_time: Duration) {
    add(Phase::Frames, took);
    GAME_FRAMES.fetch_add(frames, Ordering::Relaxed);
    let now = Instant::now();
    let mut last = LAST_FRAMES_AT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(last) = last.replace(now) {
        let gap = now.duration_since(last);
        WORST_GAP_MICROS.fetch_max(gap.as_micros() as u64, Ordering::Relaxed);
        if gap >= frame_time * 3 / 2 {
            HITCHES.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub struct Snapshot {
    at: Instant,
    micros: [u64; PHASES],
    counts: [u64; PHASES],
    game_frames: u64,
    hitches: u64,
    skipped: u64,
    timers: [u64; ruffle_render_wgpu::perf_counters::timers::LEN],
    walk: u64,
}

pub fn snapshot() -> Snapshot {
    Snapshot {
        at: Instant::now(),
        micros: std::array::from_fn(|i| MICROS[i].load(Ordering::Relaxed)),
        counts: std::array::from_fn(|i| COUNTS[i].load(Ordering::Relaxed)),
        game_frames: GAME_FRAMES.load(Ordering::Relaxed),
        hitches: HITCHES.load(Ordering::Relaxed),
        skipped: SKIPPED.load(Ordering::Relaxed),
        timers: ruffle_render_wgpu::perf_counters::timers::snapshot(),
        walk: ruffle_core::render_walk_micros(),
    }
}

impl Snapshot {
    /// Two lines on the time since `earlier`: the engine thread per second (with the average and
    /// worst per draw or frame), then Mugetsu's render timers per rendered picture. The worst
    /// values cover the time since the last report.
    pub fn report(&self, earlier: &Snapshot) -> String {
        use ruffle_render_wgpu::perf_counters::timers;
        let secs = self.at.duration_since(earlier.at).as_secs_f64().max(0.001);
        let ms = |p: Phase| (self.micros[p as usize] - earlier.micros[p as usize]) as f64 / 1000.0;
        let count = |p: Phase| self.counts[p as usize] - earlier.counts[p as usize];
        let max = |p: Phase| MAX_MICROS[p as usize].swap(0, Ordering::Relaxed) as f64 / 1000.0;
        let part = |p: Phase| {
            let n = count(p);
            let avg = if n > 0 { ms(p) / n as f64 } else { 0.0 };
            format!("{:.0} ms/s [{avg:.2} avg, {:.1} max]", ms(p) / secs, max(p))
        };
        let renders = count(Phase::Render);
        let worst_gap = WORST_GAP_MICROS.swap(0, Ordering::Relaxed) as f64 / 1000.0;
        let per_render = |micros: u64| {
            if renders > 0 {
                micros as f64 / 1000.0 / renders as f64
            } else {
                0.0
            }
        };
        let timer_parts = timers::NAMES
            .iter()
            .zip(self.timers.iter().zip(earlier.timers))
            .filter(|(_, (a, b))| **a > *b)
            .map(|(name, (a, b))| format!("{name} {:.2}", per_render(a - b)))
            .collect::<Vec<_>>()
            .join(", ");
        let counts = ruffle_render_wgpu::perf_counters::take();
        let counters = ruffle_render_wgpu::perf_counters::NAMES
            .iter()
            .zip(counts)
            .filter(|(_, n)| *n > 0)
            .map(|(name, n)| {
                format!(
                    "{name} {:.1}",
                    if renders > 0 {
                        n as f64 / renders as f64
                    } else {
                        0.0
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "engine thread: game {:.1} fps, {} hitches, worst gap {worst_gap:.0} ms; frames {}; draws {:.1}/s ({:.1}/s skipped, game behind), \
             {:.1} rendered/s: draw {}, render {}, UI {}, next picture {}, submit+present {}\n\
             render per rendered picture (ms): build_commands {:.2}, {timer_parts}\n\
             per rendered picture: {counters}",
            (self.game_frames - earlier.game_frames) as f64 / secs,
            self.hitches - earlier.hitches,
            part(Phase::Frames),
            count(Phase::Draw) as f64 / secs,
            (self.skipped - earlier.skipped) as f64 / secs,
            renders as f64 / secs,
            part(Phase::Draw),
            part(Phase::Render),
            part(Phase::Ui),
            part(Phase::Acquire),
            part(Phase::Present),
            per_render(self.walk - earlier.walk),
        )
    }
}
