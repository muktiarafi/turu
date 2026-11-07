//! GPU memory on a phone. The GPU shares the phone's RAM (Mali, Adreno), and running out of it
//! aborts Turu: a Redmi 10 crashed logging in, a 64-byte buffer coming back invalid in
//! `Globals::new` (docs/ANDROID.md, "GPU memory"). What Mugetsu's memory watch does on a
//! desktop (`app/src/app.rs`, `memory_watch`), where it reads the GPU's own budget, is done here
//! from the phone's memory:
//!
//! * idle pooled textures are capped at Mugetsu's low-memory budget (64 MB) from the start
//!   ([`init`]), as if the GPU were always nearly full;
//! * every minute, the geometry of shapes not drawn for 5 minutes is freed (other maps, the gear
//!   of players who left; AQW keeps every SWF it loaded, so it otherwise grows all session);
//! * when the phone runs low on memory ([`MemAvailable`](read_meminfo) under 15%), when Android
//!   says so (`TuruService.onTrimMemory`), and when the window goes away (nothing is drawn then),
//!   the idle textures and the shapes not drawn for 2 minutes are freed too ([`trim`]).
//!
//! It also logs the process's own memory every minute ([`log_memory`]): RSS and its parts, PSS,
//! the `malloc` heap, the address space and the number of mappings, every 5 minutes with the
//! largest mappings by name. The last readings are kept in the data folder's
//! `crash/memory-<pid>.log`, which `CrashReports.kt` adds to that process's exit report, and every
//! 5 minutes one goes to Sentry as a breadcrumb. A Pixel 8 aborted on a failed allocation after 27
//! minutes in the background at 1.9 GB of RSS, with 3 GB of the phone's memory free; its report
//! couldn't say what had grown.
//!
//! All of it runs on the engine thread: the shape caches are the player's thread's.

use ruffle_core::Player;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use turu_frontend::process_memory::{self, Reading};
use turu_frontend::turu;

use crate::screen::MovieTexture;

/// How often the phone's memory is read.
const CHECK_EVERY: Duration = Duration::from_secs(10);
/// How often shapes not drawn for [`SWEEP_UNUSED_SECS`] are freed.
const SWEEP_EVERY: Duration = Duration::from_secs(60);
const SWEEP_UNUSED_SECS: u32 = 300;
/// Shapes not drawn for this long are freed by a [`trim`].
const TRIM_UNUSED_SECS: u32 = 120;
/// Low memory: less than this share of the phone's memory available.
const LOW_PERCENT: u64 = 15;
/// The least time between two trims for low memory (each waits for the GPU to finish).
const TRIM_EVERY: Duration = Duration::from_secs(30);
/// How often the process's memory is logged.
const LOG_EVERY: Duration = Duration::from_secs(60);
/// Every this many readings, the largest mappings are logged too and the reading goes to Sentry.
const DETAIL_EVERY: u32 = 5;
/// The readings kept in `crash/memory-<pid>.log` (2.5 hours).
const LOG_LINES: usize = 150;
/// The largest mappings logged.
const LARGEST_MAPPINGS: usize = 6;

struct Watch {
    last_check: Instant,
    last_sweep: Instant,
    last_trim: Option<Instant>,
    low: bool,
    log: MemoryLog,
}

/// The process's memory readings ([`log_memory`]).
struct MemoryLog {
    started: Instant,
    last: Option<Instant>,
    readings: u32,
    /// The file's first lines: the process, and the kernel's limit on mappings.
    header: String,
    lines: VecDeque<String>,
}

thread_local! {
    static WATCH: RefCell<Watch> = RefCell::new(Watch {
        last_check: Instant::now(),
        last_sweep: Instant::now(),
        last_trim: None,
        low: false,
        log: MemoryLog {
            started: Instant::now(),
            last: None,
            readings: 0,
            header: String::new(),
            lines: VecDeque::new(),
        },
    });
}

/// Caps the idle textures at Mugetsu's low-memory budget for the whole run, and takes the first
/// memory reading. Once, before the first frame.
pub fn init() {
    ruffle_render_wgpu::set_memory_pressure(true);
    WATCH.with_borrow_mut(|watch| {
        // Android's SELinux policy may not let apps read it.
        let max_maps = process_memory::max_map_count().map_or_else(String::new, |n| format!("; at most {n} maps"));
        watch.log.header = format!(
            "Turu {} memory, process {}, started {}{max_maps}\n",
            turu::VERSION,
            std::process::id(),
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S %z"),
        );
        log_memory(&mut watch.log, Instant::now());
    });
}

/// Called each turn of the engine loop; does something only every few seconds.
pub fn tick(player: &mut Player) {
    WATCH.with_borrow_mut(|watch| {
        let now = Instant::now();
        if watch.log.last.is_none_or(|last| now.duration_since(last) >= LOG_EVERY) {
            log_memory(&mut watch.log, now);
        }
        if now.duration_since(watch.last_sweep) >= SWEEP_EVERY {
            watch.last_sweep = now;
            let freed = ruffle_core::evict_unused_shapes(SWEEP_UNUSED_SECS);
            if freed > 0 {
                tracing::info!(
                    target: "ruffle_aqw_perf",
                    "freed {freed} shapes not drawn for {} minutes",
                    SWEEP_UNUSED_SECS / 60
                );
            }
        }
        if now.duration_since(watch.last_check) < CHECK_EVERY {
            return;
        }
        watch.last_check = now;
        let Some((available, total)) = read_meminfo() else {
            return;
        };
        let low = available * 100 < total * LOW_PERCENT;
        if low != watch.low {
            watch.low = low;
            tracing::info!(
                target: "ruffle_aqw_perf",
                "phone memory {}: {} of {} MB available",
                if low { "low" } else { "back to normal" },
                available >> 20,
                total >> 20
            );
        }
        if low && watch.last_trim.is_none_or(|t| now.duration_since(t) >= TRIM_EVERY) {
            watch.last_trim = Some(now);
            trim(player, "the phone is low on memory");
        }
    });
}

/// Frees the idle pooled textures (waiting for the GPU to let them go) and the shapes not drawn
/// for [`TRIM_UNUSED_SECS`]. On the engine thread.
pub fn trim(player: &mut Player, why: &str) {
    let shapes = ruffle_core::evict_unused_shapes(TRIM_UNUSED_SECS);
    if let Some(renderer) = <dyn Any>::downcast_mut::<WgpuRenderBackend<MovieTexture>>(player.renderer_mut()) {
        renderer.aqw_trim_memory();
    }
    tracing::info!(
        target: "ruffle_aqw_perf",
        "freed spare textures and {shapes} unused shapes: {why}"
    );
}

/// Logs the process's memory, keeps it in `crash/memory-<pid>.log` and, every [`DETAIL_EVERY`]
/// readings, logs the largest mappings and sends the reading to Sentry.
fn log_memory(log: &mut MemoryLog, now: Instant) {
    log.last = Some(now);
    let Some(mut reading) = Reading::now() else {
        return;
    };
    // SAFETY: `mallinfo` only reads the allocator's statistics.
    reading.heap = Some(unsafe { libc::mallinfo() }.uordblks as u64);
    let detail = log.readings.is_multiple_of(DETAIL_EVERY);
    log.readings += 1;
    let up = now.duration_since(log.started).as_secs() / 60;
    let mut line = format!("{} up {up} min: {reading}", chrono::Local::now().format("%H:%M:%S"));
    tracing::info!("Memory: {reading}");
    if detail {
        let largest = process_memory::format_mappings(&process_memory::largest_mappings(LARGEST_MAPPINGS));
        tracing::info!("Largest mappings: {largest}");
        crate::notify::breadcrumb("memory", &format!("up {up} min: {reading}; largest: {largest}"));
        line.push_str("\n    largest: ");
        line.push_str(&largest);
    }
    if log.lines.len() >= LOG_LINES {
        log.lines.pop_front();
    }
    log.lines.push_back(line);
    if let Some(path) = log_file() {
        let mut text = log.header.clone();
        for line in &log.lines {
            text.push_str(line);
            text.push('\n');
        }
        if let Err(e) = std::fs::write(&path, text) {
            tracing::warn!("Couldn't write {}: {e}", path.display());
        }
    }
}

/// `crash/memory-<pid>.log` (`.log`: the crash list only shows `.txt` reports).
fn log_file() -> Option<PathBuf> {
    let dir = crate::CRASH_DIR.get()?;
    std::fs::create_dir_all(dir).ok()?;
    Some(dir.join(format!("memory-{}.log", std::process::id())))
}

/// `MemAvailable` and `MemTotal` from `/proc/meminfo`, in bytes.
fn read_meminfo() -> Option<(u64, u64)> {
    parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
            .and_then(|rest| rest.split_whitespace().next()?.parse::<u64>().ok())
            .map(|kb| kb * 1024)
    };
    Some((field("MemAvailable")?, field("MemTotal")?))
}

/// `TuruService.nativeTrimMemory(level)`: Android's `onTrimMemory` at `TRIM_MEMORY_RUNNING_LOW`
/// or above (the service gets it as the activity does, and runs whenever the engine does).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruService_nativeTrimMemory(
    _env: jni::EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    level: jni::sys::jint,
) {
    crate::run_on_engine(move |player| trim(player, &format!("Android asked (trim level {level})")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_available_and_total_memory() {
        let text = "MemTotal:        3798016 kB\nMemFree:          212340 kB\nMemAvailable:    1273856 kB\n";
        assert_eq!(parse_meminfo(text), Some((1273856 * 1024, 3798016 * 1024)));
        assert_eq!(parse_meminfo("MemTotal: 10 kB\n"), None);
    }
}
