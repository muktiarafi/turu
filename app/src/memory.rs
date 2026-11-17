//! AQW build: what Turu frees, and when: the memory watch (shapes not drawn for a while, the
//! graphics memory nearly full, Mugetsu 2.0.0's memory limit) and "Free memory now". Shared by
//! the window and the process tabs' clients (`client.rs`).

use std::time::{Duration, Instant};

/// AQW build: what the memory limit is compared with, in MB: the figure Task Manager shows
/// (see `mugetsu::task_manager_mb`; it was the committed memory until 2026-11-16).
fn process_commit_mb() -> u64 {
    crate::mugetsu::task_manager_mb()
}

/// What one `free_memory` did, for the log.
pub(crate) struct Freed {
    /// Shapes not drawn in the last 30 seconds, now gone.
    shapes: usize,
    /// (working set, committed) in MB, before and after.
    before: (u64, u64),
    after: (u64, u64),
    /// How long the three steps took on this thread, in ms: the shapes, the spare textures,
    /// the heap pages.
    took_ms: [f32; 3],
    /// Whether the heap's unused pages were returned on this thread (`after` has them) or
    /// are being returned in the background (a log line of its own says when and how much).
    heap_waited: bool,
    /// The thorough way only: (loaded SWFs nothing used any more, now freed; the
    /// milliseconds that took).
    libraries: Option<(usize, f64)>,
}

impl std::fmt::Display for Freed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [shapes, textures, heap] = self.took_ms;
        write!(
            f,
            "{} -> {} MB committed (working set {} -> {} MB); {} shapes not drawn in the last 30 s, the spare textures and unused heap pages freed in {:.1} ms (shapes {shapes:.1}, textures {textures:.1}, heap pages {heap:.1}){}",
            self.before.1,
            self.after.1,
            self.before.0,
            self.after.0,
            self.shapes,
            shapes + textures + heap,
            if self.heap_waited { "" } else { "; the heap's pages go back in the background" },
        )?;
        if let Some((libraries, ms)) = self.libraries {
            write!(f, "; before that, {libraries} loaded SWFs nothing used any more freed in {ms:.0} ms")?;
        }
        Ok(())
    }
}

/// TEST SWITCH: `RUFFLE_HEAP_RETURN=wait`: freeing memory returns the heap's unused pages on
/// the main thread and waits for every worker, as it did (for comparisons).
fn heap_return_waits() -> bool {
    static WAITS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *WAITS.get_or_init(|| std::env::var("RUFFLE_HEAP_RETURN").is_ok_and(|value| value == "wait"))
}

/// AQW build: frees everything that can be freed without the picture changing: the shapes
/// not drawn in the last 30 seconds (what is on screen is drawn in every picture), the spare
/// textures and the layer cache, and the allocator's free pages. For the memory limit and
/// "Free memory now".
///
/// The heap's pages were returned here, with a forced collect on this thread and on every
/// worker, waited for: 10-16 ms in a test room, but 27-33 ms of a 37-42 ms freeing in a real
/// session with a 3 GB limit (8 thousand pages, 5 thousand of them abandoned by their
/// threads): a game frame late every time the limit worked. Returning memory to the system
/// is the allocator's business for the whole process, not this thread's: now every thread
/// only hands its own free pages back to the allocator (cheap; the workers when they are next
/// free), and a thread started for it does the forced collect that gives them to the system.
///
/// That returns less: measured in a test room, the working set fell by 30-55 MB instead of
/// 110-140 (only a forced collect on the main thread takes back the pages other threads
/// abandoned), while the committed memory, which is mostly shapes and textures, fell as
/// before. So the memory limit, which works by itself during play, frees this way, and
/// "Free memory now", a click in the menu where a pause of 40 ms is not seen, frees the
/// `thorough` way: forced, on this thread and every worker, waited for.
///
/// Turu: for every player of the process, which `players` hands to the function it's given
/// (each holds its own libraries and renderer); the shapes and the heap's pages are the
/// process's.
pub(crate) fn free_memory(players: &mut dyn FnMut(&mut dyn FnMut(&mut ruffle_core::Player)), thorough: bool) -> Freed {
    let before = crate::mugetsu::process_memory_mb();
    // (A click only: every loaded SWF nothing uses any more goes now, not within the next
    // GC cycles: two whole cycles in one go, a pause the limit's own freeing must not make.)
    let libraries = thorough.then(|| {
        let mut freed = (0, 0.0);
        players(&mut |player| {
            let (more, took) = player.free_unused_libraries_now();
            freed = (freed.0 + more, freed.1 + took);
        });
        freed
    });
    let started = Instant::now();
    let shapes = ruffle_core::evict_unused_shapes(30);
    let shapes_done = Instant::now();
    players(&mut |player| trim_renderer(player));
    let textures_done = Instant::now();
    let heap_waited = thorough || heap_return_waits();
    // SAFETY: plain calls into the allocator, each on a thread that allocates with it.
    if heap_waited {
        ruffle_render_wgpu::on_every_worker(|| unsafe {
            libmimalloc_sys::mi_collect(true);
        });
        unsafe { libmimalloc_sys::mi_collect(true) };
    } else {
        unsafe { libmimalloc_sys::mi_collect(false) };
        ruffle_render_wgpu::on_every_worker_later(|| unsafe {
            libmimalloc_sys::mi_collect(false);
        });
        let spawned = std::thread::Builder::new()
            .name("free memory".to_string())
            .spawn(|| {
                // (The workers get to theirs first.)
                std::thread::sleep(Duration::from_millis(100));
                let started = Instant::now();
                unsafe { libmimalloc_sys::mi_collect(true) };
                let took = started.elapsed().as_secs_f32() * 1000.0;
                let (working_set, commit) = crate::mugetsu::process_memory_mb();
                tracing::info!(
                    target: "ruffle_aqw_perf",
                    "the heap's unused pages went back to the system in {took:.1} ms in the background: now {commit} MB committed (working set {working_set} MB)"
                );
            });
        if let Err(error) = spawned {
            tracing::warn!("free memory: no thread for the heap's pages ({error})");
        }
    }
    let heap_done = Instant::now();
    let ms = |from: Instant, to: Instant| (to - from).as_secs_f32() * 1000.0;
    Freed {
        shapes,
        before,
        after: crate::mugetsu::process_memory_mb(),
        took_ms: [
            ms(started, shapes_done),
            ms(shapes_done, textures_done),
            ms(textures_done, heap_done),
        ],
        heap_waited,
        libraries,
    }
}

/// AQW build: frees the renderer's spare textures and lets the GPU release the memory.
pub(crate) fn trim_renderer(player: &mut ruffle_core::Player) {
    if let Some(renderer) = <dyn std::any::Any>::downcast_mut::<
        ruffle_render_wgpu::backend::WgpuRenderBackend<crate::gui::MovieView>,
    >(player.renderer_mut())
    {
        renderer.aqw_trim_memory();
    }
}

/// AQW build (Mugetsu 2.0.0): the memory limit asks for a freeing (see `memory_watch`): the
/// limit in MB, whether the last freeing held, and the wait before the next one (s).
pub(crate) struct LimitTrim {
    pub limit_mb: u64,
    pub held: bool,
    pub next_wait_secs: u64,
}

/// How long after freeing memory for the player's limit the next freeing may come, at the
/// least, in seconds.
const LIMIT_WAIT_SECS: u64 = 20;
/// And at the most, while the freeing does not hold (see `memory_watch`).
const LIMIT_WAIT_MAX_SECS: u64 = 300;

///
/// Turu: of the active tab's player; a freeing for the memory limit is returned, for the
/// caller to make over every tab (`free_memory`) once this player is let go of.
pub(crate) fn memory_watch(player: &mut ruffle_core::Player) -> Option<LimitTrim> {
    struct Watch {
        pressure: bool,
        low_since: Option<Instant>,
        last_log: Option<Instant>,
        last_sweep: Option<Instant>,
        last_collect: Option<Instant>,
        /// When memory was last freed for the player's memory limit.
        last_limit_trim: Option<Instant>,
        /// How long to wait after that before freeing again, in seconds.
        limit_wait_secs: u64,
        /// The limit those two belong to: another limit starts afresh.
        limit_seen_mb: u64,
    }
    static WATCH: std::sync::Mutex<Watch> = std::sync::Mutex::new(Watch {
        pressure: false,
        low_since: None,
        last_log: None,
        last_sweep: None,
        last_collect: None,
        last_limit_trim: None,
        limit_wait_secs: LIMIT_WAIT_SECS,
        limit_seen_mb: 0,
    });
    let mut watch = WATCH.lock().unwrap_or_else(|e| e.into_inner());
    // Turu: once a minute, hand the memory mimalloc keeps free back to the system
    // (`release_free_memory`).
    if watch
        .last_collect
        .is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
    {
        watch.last_collect = Some(Instant::now());
        crate::release_free_memory();
    }
    // AQW build: on every GPU (and every OS), once a minute, free the tessellations of shapes
    // not drawn for a while: other maps, the gear of players who left. AQW keeps every SWF
    // it has loaded, so shape geometry otherwise grows for the whole session (800 MB in four
    // minutes of town hopping on a 32 GB card, where the memory-pressure path below never
    // runs). A shape drawn again later is tessellated again, as on its first draw. A
    // `no-shape-eviction` file next to the exe turns it off (A/B).
    //
    // "A while" was always 10 minutes, and ten minutes of going from room to room held 69
    // thousand shapes, 1 GB of geometry (which is the system's RAM on built-in graphics). So
    // the fuller the store, the sooner an unused shape goes: 10 minutes while the store is
    // small, 3 minutes above `mugetsu::shape_store_mb`, 2 minutes above twice that. What is
    // on screen is drawn every picture and is never among them.
    if crate::mugetsu::shape_eviction_enabled()
        && watch
            .last_sweep
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
    {
        watch.last_sweep = Some(Instant::now());
        let store_mb = (ruffle_render_wgpu::perf_counters::live().1.max(0) as u64) >> 20;
        let soft_mb = crate::mugetsu::shape_store_mb();
        let unused_secs = if soft_mb == 0 || store_mb <= soft_mb {
            600
        } else if store_mb <= soft_mb * 2 {
            180
        } else {
            120
        };
        let freed = ruffle_core::evict_unused_shapes(unused_secs);
        if freed > 0 {
            let now_mb = (ruffle_render_wgpu::perf_counters::live().1.max(0) as u64) >> 20;
            tracing::info!(
                target: "ruffle_aqw_perf",
                "freed {freed} shapes not drawn for {} minutes (shape geometry: {store_mb} -> {now_mb} MB)",
                unused_secs / 60
            );
        }
    }
    // AQW build: the player's own memory limit (see `mugetsu::memory_limit_mb`): over it,
    // everything that can be freed is, at most every 20 seconds (what is freed and needed
    // again is made again, and freeing in every window would do nothing else).
    //
    // A scene can need more than the limit: with 2 GB set, towns and boss rooms on DirectX 12
    // held 2.2-2.35 GB right after each freeing and 2.5 GB twenty seconds later, so the same
    // spare textures and heap pages were freed and made again every 20 seconds for as long as
    // the limit stood, each round a game frame arriving 15 ms late in the boss rooms (the
    // user's session of 2026-11-15). So when memory is over the limit again at the first
    // chance after a freeing, that freeing did not hold, and the wait before the next one
    // doubles, up to 5 minutes; a freeing that held for a while puts it back to 20 seconds.
    let limit_mb = u64::from(crate::mugetsu::memory_limit_mb());
    if limit_mb != watch.limit_seen_mb {
        watch.limit_seen_mb = limit_mb;
        watch.limit_wait_secs = LIMIT_WAIT_SECS;
        watch.last_limit_trim = None;
    }
    let mut limit_trim = None;
    if limit_mb != 0
        && watch
            .last_limit_trim
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(watch.limit_wait_secs))
        && process_commit_mb() > limit_mb
    {
        let held = watch
            .last_limit_trim
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(watch.limit_wait_secs * 2 + 10));
        watch.limit_wait_secs = if held {
            LIMIT_WAIT_SECS
        } else {
            (watch.limit_wait_secs * 2).min(LIMIT_WAIT_MAX_SECS)
        };
        watch.last_limit_trim = Some(Instant::now());
        limit_trim = Some(LimitTrim {
            limit_mb,
            held,
            next_wait_secs: watch.limit_wait_secs,
        });
    }
    let Some((budget, usage)) = crate::mugetsu::video_memory() else {
        return limit_trim;
    };
    let budget = crate::mugetsu::test_video_budget().map_or(budget, |test| test.min(budget));
    if usage > budget / 100 * 85 {
        // Shapes not drawn for 2 minutes (gear of players who left, other maps) are
        // tessellated again if they come back.
        let shapes = ruffle_core::evict_unused_shapes(120);
        if let Some(renderer) = <dyn std::any::Any>::downcast_mut::<
            ruffle_render_wgpu::backend::WgpuRenderBackend<crate::gui::MovieView>,
        >(player.renderer_mut())
        {
            renderer.aqw_trim_memory();
        }
        ruffle_render_wgpu::set_memory_pressure(true);
        let was = std::mem::replace(&mut watch.pressure, true);
        watch.low_since = None;
        if !was || watch.last_log.is_none_or(|t| t.elapsed() >= Duration::from_secs(60)) {
            watch.last_log = Some(Instant::now());
            let after = crate::mugetsu::video_memory().map_or(0, |(_, usage)| usage);
            tracing::info!(
                target: "ruffle_aqw_perf",
                "graphics memory nearly full: {} of {} MB; freed spare textures and {shapes} unused shapes, now {} MB",
                usage >> 20,
                budget >> 20,
                after >> 20
            );
        }
    } else if watch.pressure {
        if usage < budget / 100 * 65 {
            let low_since = *watch.low_since.get_or_insert_with(Instant::now);
            if low_since.elapsed() >= Duration::from_secs(60) {
                watch.pressure = false;
                watch.low_since = None;
                ruffle_render_wgpu::set_memory_pressure(false);
                tracing::info!(
                    target: "ruffle_aqw_perf",
                    "graphics memory back to normal: {} of {} MB",
                    usage >> 20,
                    budget >> 20
                );
            }
        } else {
            watch.low_since = None;
        }
    }
    limit_trim
}

