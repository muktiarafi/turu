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
//! drawn any more: [`gpu_lost`] says so once, and the slot's process ends, as a desktop tab's client
//! does; Android starts the slot again (`START_STICKY`), logged in from its record and with its
//! script (`restart.rs`), and the shell binds it again. At most [`GPU_RESTARTS`] times in
//! [`GPU_RESTART_WINDOW`] (a GPU that keeps failing would log the account in again and again):
//! after that the slot plays on without a picture, as every slot did before.
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

/// The exit status of a slot that lost its GPU ([`gpu_lost`]): `CrashReports.kt` doesn't report it
/// as a crash (the loss went to Sentry already). The desktop client's.
const EXIT_GPU_LOST: i32 = 86;
/// A slot starts again for a lost GPU at most this many times in this window.
const GPU_RESTARTS: usize = 3;
const GPU_RESTART_WINDOW: Duration = Duration::from_secs(600);

/// The least time between two trims for the memory limit (as Mugetsu's, and Options says).
const LIMIT_TRIM_EVERY: Duration = Duration::from_secs(20);
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
    /// The last trim for the memory limit (Options > Application > Memory Limit).
    last_limit_trim: Option<Instant>,
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
        last_limit_trim: None,
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
        check_memory_limit(watch, player, now);
    });
}

/// Mugetsu 2.0.0's memory limit (Options > Application > Memory Limit, `crate::memory_limit_mb`):
/// over it, this slot frees what it can, at most every [`LIMIT_TRIM_EVERY`]. On top of Turu's own budget
/// ([`check_own_budget`]), for a phone that kills Turu below that.
fn check_memory_limit(watch: &mut Watch, player: &mut Player, now: Instant) {
    let limit_mb = u64::from(crate::memory_limit_mb());
    if limit_mb == 0
        || watch
            .last_limit_trim
            .is_some_and(|t| now.duration_since(t) < LIMIT_TRIM_EVERY)
    {
        return;
    }
    let Some(reading) = Reading::now() else {
        return;
    };
    let rss_mb = reading.rss >> 20;
    if rss_mb <= limit_mb {
        return;
    }
    watch.last_limit_trim = Some(now);
    trim(player, &format!("over the memory limit: {rss_mb} of {limit_mb} MB"));
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

/// The GPU dropped the device: ruffle-aqw draws nothing from now on. Said once: in the log, the
/// debug log, the notification, and to Sentry as a warning of its own (a lost device used to abort
/// in `Device::poll`, Sentry TURU-ANDROID-7 on beta.6: a Redmi 10 2022). Then a slot's process ends
/// and Android starts it again (see the module), unless it did that [`GPU_RESTARTS`] times lately:
/// the game, its scripts and the notification's buttons go on without a picture then.
fn gpu_lost(why: &str) {
    let restart = crate::slots::slot().is_some_and(gpu_restart_allowed);
    if restart {
        tracing::error!("The GPU stopped working ({why}); this tab's process ends, and Android starts it again");
        turu::bot().logs.debug.push(format!(
            "Graphics: the GPU stopped working ({why}); the tab starts again"
        ));
    } else {
        tracing::error!(
            "The GPU stopped working ({why}); the game goes on without a picture until Turu restarts"
        );
        turu::bot().logs.debug.push(format!(
            "Graphics: the GPU stopped working ({why}); restart Turu to see the game"
        ));
    }
    crate::notify::gpu_lost(why);
    if restart {
        // Sentry keeps the warning on disk before sending it; a moment for the log to be written.
        std::thread::sleep(Duration::from_secs(1));
        std::process::exit(EXIT_GPU_LOST);
    }
}

/// Whether slot `slot` may start again for a lost GPU now (see [`GPU_RESTARTS`]); if so, this time
/// is kept with the others (`crash/gpu-lost-c<N>.log`, seconds since 1970, one a line).
fn gpu_restart_allowed(slot: u32) -> bool {
    let Some(dir) = crate::CRASH_DIR.get() else {
        return false;
    };
    let path = dir.join(format!("gpu-lost-c{slot}.log"));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let Some(times) = restart_times(&text, now) else {
        tracing::warn!(
            "The GPU was lost {GPU_RESTARTS} times in {} minutes: this tab doesn't start again for it",
            GPU_RESTART_WINDOW.as_secs() / 60
        );
        return false;
    };
    let _ = std::fs::create_dir_all(dir);
    let kept = times
        .iter()
        .map(|time| format!("{time}\n"))
        .collect::<String>();
    if let Err(e) = std::fs::write(&path, kept) {
        tracing::warn!("Couldn't keep the time of the GPU's loss ({e}): the tab doesn't start again");
        return false;
    }
    true
}

/// The times kept in `text` within [`GPU_RESTART_WINDOW`] of `now`, with `now`; `None` if that
/// would be more than [`GPU_RESTARTS`].
fn restart_times(text: &str, now: u64) -> Option<Vec<u64>> {
    let mut times: Vec<u64> = text
        .lines()
        .filter_map(|line| line.trim().parse::<u64>().ok())
        .filter(|time| *time <= now && now - *time < GPU_RESTART_WINDOW.as_secs())
        .collect();
    if times.len() >= GPU_RESTARTS {
        return None;
    }
    times.push(now);
    Some(times)
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

/// Options > Application > Free Memory Now (Mugetsu 2.0.0's "Free memory now", as a desktop tab
/// does it, `app/src/memory.rs`): every loaded SWF nothing uses any more, the shapes not drawn in
/// the last 30 seconds (what's on screen is drawn in every picture), and the spare textures. On
/// the engine thread.
pub fn free_now(player: &mut Player) {
    let before = Reading::now().map_or(0, |reading| reading.rss >> 20);
    let (libraries, took_ms) = player.free_unused_libraries_now();
    let shapes = ruffle_core::evict_unused_shapes(30);
    if let Some(renderer) =
        <dyn Any>::downcast_mut::<WgpuRenderBackend<MovieTexture>>(player.renderer_mut())
    {
        renderer.aqw_trim_memory();
    }
    let after = Reading::now().map_or(0, |reading| reading.rss >> 20);
    tracing::info!(
        target: "ruffle_aqw_perf",
        "free memory now: {before} -> {after} MB resident; {libraries} unused SWFs ({took_ms:.1} ms), {shapes} shapes not drawn in the last 30 s and the spare textures freed"
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
    // The tabs' states are in `crash/tabs-<pid>.log` (`turu_frontend::tabs_log`); the count here
    // ties each reading to it.
    let tabs = turu::tabs().len();
    let mut line = format!(
        "{} up {up} min, {tabs} tab{}: {cpu}{reading}",
        chrono::Local::now().format("%H:%M:%S"),
        if tabs == 1 { "" } else { "s" }
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

/// `SlotService.nativeTrimMemory(level)`: the same for a process tab's slot (each process gets its
/// own `onTrimMemory`: the activity's says nothing about the slots').
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeTrimMemory(
    _env: jni::EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    level: jni::sys::jint,
) {
    trim_asked(level);
}

/// Frees what the engine can spare, as Android asked at `level`.
fn trim_asked(level: jni::sys::jint) {
    crate::run_on_engine(move |player| {
        trim(player, &format!("Android asked (trim level {level})"))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_gpu_restarts_a_slot_three_times_in_ten_minutes() {
        assert_eq!(restart_times("", 1000), Some(vec![1000]));
        // Times older than the window, and from a clock set back, don't count.
        assert_eq!(restart_times("100\n1500\n2000\n", 1000 + 700), Some(vec![1500, 1700]));
        assert_eq!(restart_times("1000\n1100\n", 1200), Some(vec![1000, 1100, 1200]));
        assert_eq!(restart_times("1000\n1100\n1200\n", 1300), None);
        assert_eq!(restart_times("1000\n1100\n1200\n", 1650), Some(vec![1100, 1200, 1650]));
    }

    #[test]
    fn reads_available_and_total_memory() {
        let text = "MemTotal:        3798016 kB\nMemFree:          212340 kB\nMemAvailable:    1273856 kB\n";
        assert_eq!(parse_meminfo(text), Some((1273856 * 1024, 3798016 * 1024)));
        assert_eq!(parse_meminfo("MemTotal: 10 kB\n"), None);
    }
}
