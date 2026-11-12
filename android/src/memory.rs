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
//! * when Turu itself grows past its budget ([`process_memory::over_budget`]: RSS over the larger
//!   of 1 GB and 20% of the phone's memory, or 30 000 mappings), it trims too, at most every 2
//!   minutes: a Pixel 8 aborted at 1.9 GB with 3 GB of the phone free, so the phone's free memory
//!   alone never said so;
//! * every 10 s, finished GPU work is let go ([`release_finished_gpu_work`]), so its command
//!   buffers are freed though the game submits nothing new (wgpu only looks after a submit);
//! * when the phone runs low on memory ([`MemAvailable`](read_meminfo) under 15%), when Android
//!   says so (`TuruService.onTrimMemory`), and when the window goes away (nothing is drawn then),
//!   the idle textures and the shapes not drawn for 2 minutes are freed too ([`trim`]).
//!
//! It also logs the process's own memory every minute ([`log_memory`]), with its share of a CPU
//! core since the reading before and the busiest threads' shares (ColorOS closed Turu "for using
//! too much CPU" in the background: TURU-ANDROID-D's log showed 12-16% at 2 FPS and stretches of
//! ~94% for 10 minutes, and couldn't say whose): RSS and its parts, PSS,
//! the `malloc` heap, the address space and the number of mappings, every 5 minutes with the
//! largest mappings by name. The last readings are kept in the data folder's
//! `crash/memory-<pid>.log`, which `CrashReports.kt` adds to that process's exit report, and every
//! 5 minutes one goes to Sentry as a breadcrumb. A Pixel 8 aborted on a failed allocation after 27
//! minutes in the background at 1.9 GB of RSS, with 3 GB of the phone's memory free; its report
//! couldn't say what had grown.
//!
//! When the GPU drops the device (wgpu loses it when the driver runs out of memory), nothing can be
//! drawn any more, but the game and its scripts keep running: [`gpu_lost`] says so once.
//!
//! All of it runs on the engine thread: the shape caches are the player's thread's.

use ruffle_core::Player;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use turu_frontend::process_memory::{self, Reading, ThreadCpu};
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
/// The least time between two trims for Turu's own memory (it may stay over its budget).
const OWN_TRIM_EVERY: Duration = Duration::from_secs(120);
/// How often the process's memory is logged.
const LOG_EVERY: Duration = Duration::from_secs(60);
/// Every this many readings, the largest mappings are logged too and the reading goes to Sentry.
const DETAIL_EVERY: u32 = 5;
/// The readings kept in `crash/memory-<pid>.log` (2.5 hours).
const LOG_LINES: usize = 150;
/// The largest mappings logged.
const LARGEST_MAPPINGS: usize = 6;
/// The busiest threads named with the CPU share.
const BUSIEST_THREADS: usize = 4;

struct Watch {
    last_check: Instant,
    last_sweep: Instant,
    last_trim: Option<Instant>,
    low: bool,
    /// Turu over its own memory budget, and when it last trimmed for that.
    over: bool,
    last_own_trim: Option<Instant>,
    /// The GPU device was lost, and that was reported ([`gpu_lost`]).
    gpu_lost: bool,
    log: MemoryLog,
}

/// The process's memory readings ([`log_memory`]).
struct MemoryLog {
    started: Instant,
    last: Option<Instant>,
    readings: u32,
    /// The reading before, its threads' CPU times and when it was taken, for the CPU share in
    /// between.
    previous: Option<(Instant, Reading, Vec<ThreadCpu>)>,
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
        over: false,
        last_own_trim: None,
        gpu_lost: false,
        log: MemoryLog {
            started: Instant::now(),
            last: None,
            readings: 0,
            previous: None,
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
        let max_maps = process_memory::max_map_count()
            .map_or_else(String::new, |n| format!("; at most {n} maps"));
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
        if watch
            .log
            .last
            .is_none_or(|last| now.duration_since(last) >= LOG_EVERY)
        {
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
        if !watch.gpu_lost
            && let Some(why) = ruffle_render_wgpu::device_lost()
        {
            watch.gpu_lost = true;
            gpu_lost(&why);
        }
        release_finished_gpu_work(player);
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
        if low
            && watch
                .last_trim
                .is_none_or(|t| now.duration_since(t) >= TRIM_EVERY)
        {
            watch.last_trim = Some(now);
            trim(player, "the phone is low on memory");
        }
        check_own_budget(watch, player, total, now);
    });
}

/// Trims when Turu itself is over its memory budget ([`process_memory::over_budget`]), at most
/// every [`OWN_TRIM_EVERY`]. Going over and back are warnings, so they reach Sentry as breadcrumbs.
fn check_own_budget(watch: &mut Watch, player: &mut Player, phone_memory: u64, now: Instant) {
    let Some(reading) = Reading::now() else {
        return;
    };
    let over = process_memory::over_budget(&reading, phone_memory);
    if over.is_some() != watch.over {
        watch.over = over.is_some();
        match &over {
            Some(why) => {
                tracing::warn!(target: "ruffle_aqw_perf", "Turu is over its memory budget: {why}")
            }
            None => {
                tracing::warn!(target: "ruffle_aqw_perf", "Turu is back within its memory budget: {reading}")
            }
        }
    }
    if let Some(why) = over
        && watch
            .last_own_trim
            .is_none_or(|t| now.duration_since(t) >= OWN_TRIM_EVERY)
    {
        watch.last_own_trim = Some(now);
        trim(player, &why);
    }
}

/// Lets wgpu release the command buffers of GPU work that has finished. It only looks after the
/// next submit or a poll, and without a window the game may submit nothing for minutes: on the
/// emulator, the last submission (and its command buffers) then stayed held all that time. A
/// poll that doesn't wait.
fn release_finished_gpu_work(player: &mut Player) {
    // Nothing finishes on a lost device (its poll no longer panics, but there's no use).
    if ruffle_render_wgpu::device_lost().is_some() {
        return;
    }
    if let Some(renderer) =
        <dyn Any>::downcast_mut::<WgpuRenderBackend<MovieTexture>>(player.renderer_mut())
    {
        let _ = renderer.descriptors().device.poll(wgpu::PollType::Poll);
    }
}

/// The GPU dropped the device: ruffle-aqw draws nothing from now on and the screen stops drawing
/// (`Screen::draw`), while the game, its scripts and the notification's buttons go on. Said once:
/// in the log, the debug log, the notification, and to Sentry as a warning of its own (a lost
/// device used to abort in `Device::poll`, Sentry TURU-ANDROID-7 on beta.6: a Redmi 10 2022).
fn gpu_lost(why: &str) {
    tracing::error!(
        "The GPU stopped working ({why}); the game goes on without a picture until Turu restarts"
    );
    turu::bot().logs.debug.push(format!(
        "Graphics: the GPU stopped working ({why}); restart Turu to see the game"
    ));
    crate::notify::gpu_lost(why);
}

/// Frees the idle pooled textures (waiting for the GPU to let them go) and the shapes not drawn
/// for [`TRIM_UNUSED_SECS`]. On the engine thread.
pub fn trim(player: &mut Player, why: &str) {
    let shapes = ruffle_core::evict_unused_shapes(TRIM_UNUSED_SECS);
    if let Some(renderer) =
        <dyn Any>::downcast_mut::<WgpuRenderBackend<MovieTexture>>(player.renderer_mut())
    {
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
    let threads = process_memory::thread_cpu_times();
    let cpu = match &log.previous {
        Some((then, before, before_threads)) => {
            let elapsed = now.duration_since(*then);
            process_memory::cpu_percent(before, &reading, elapsed).map(|percent| {
                let busiest = process_memory::busiest_threads(
                    before_threads,
                    &threads,
                    elapsed,
                    BUSIEST_THREADS,
                );
                if busiest.is_empty() {
                    format!("CPU {percent:.0}% of a core, ")
                } else {
                    format!(
                        "CPU {percent:.0}% of a core ({}), ",
                        process_memory::format_threads(&busiest)
                    )
                }
            })
        }
        None => reading
            .cpu_time
            .map(|time| format!("CPU {:.1} s so far, ", time.as_secs_f64())),
    }
    .unwrap_or_default();
    let mut line = format!(
        "{} up {up} min: {cpu}{reading}",
        chrono::Local::now().format("%H:%M:%S")
    );
    tracing::info!("Memory: {cpu}{reading}");
    if detail {
        let largest =
            process_memory::format_mappings(&process_memory::largest_mappings(LARGEST_MAPPINGS));
        tracing::info!("Largest mappings: {largest}");
        crate::notify::breadcrumb(
            "memory",
            &format!("up {up} min: {cpu}{reading}; largest: {largest}"),
        );
        line.push_str("\n    largest: ");
        line.push_str(&largest);
    }
    log.previous = Some((now, reading, threads));
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
    crate::run_on_engine(move |player| {
        trim(player, &format!("Android asked (trim level {level})"))
    });
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
