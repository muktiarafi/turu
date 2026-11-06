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
//! All of it runs on the engine thread: the shape caches are the player's thread's.

use ruffle_core::Player;
use ruffle_render_wgpu::backend::WgpuRenderBackend;
use std::any::Any;
use std::cell::RefCell;
use std::time::{Duration, Instant};

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

struct Watch {
    last_check: Instant,
    last_sweep: Instant,
    last_trim: Option<Instant>,
    low: bool,
}

thread_local! {
    static WATCH: RefCell<Watch> = RefCell::new(Watch {
        last_check: Instant::now(),
        last_sweep: Instant::now(),
        last_trim: None,
        low: false,
    });
}

/// Caps the idle textures at Mugetsu's low-memory budget for the whole run. Once, before the
/// first frame.
pub fn init() {
    ruffle_render_wgpu::set_memory_pressure(true);
}

/// Called each turn of the engine loop; does something only every few seconds.
pub fn tick(player: &mut Player) {
    WATCH.with_borrow_mut(|watch| {
        let now = Instant::now();
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
