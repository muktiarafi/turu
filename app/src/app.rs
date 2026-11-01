use crate::custom_event::{OpenType, RuffleEvent};
use crate::gui::{GuiController, MENU_HEIGHT};
use crate::player::{LaunchOptions, PlayerController};
use crate::preferences::GlobalPreferences;
use crate::util::{
    get_screen_size, gilrs_button_to_gamepad_button, winit_input_to_ruffle_key_descriptor,
    winit_to_ruffle_text_control,
};
use anyhow::Error;
use gilrs::{Event, EventType, Gilrs};
use ruffle_core::FloatDuration;
use ruffle_core::PlayerEvent;
use ruffle_core::events::{ImeEvent, ImeNotification, PlayerNotification};
use ruffle_core::swf::HeaderExt;
use ruffle_frontend_utils::content::ContentDescriptor;
use ruffle_render::backend::ViewportDimensions;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Size};
use winit::event::{ElementState, Ime, KeyEvent, Modifiers, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Icon, WindowAttributes, WindowId};

struct MainWindow {
    preferences: GlobalPreferences,
    gui: GuiController,
    player: PlayerController,
    minimized: bool,
    /// Set from `WindowEvent::Occluded`, which winit only reports on macOS and X11
    /// (and on X11 only without a compositor). Always false elsewhere.
    occluded: bool,
    mouse_pos: PhysicalPosition<f64>,
    /// Turu: whether the cursor is in the window (`CursorEntered`/`CursorLeft`), for the tab
    /// that becomes the active one (`PlayerController::give_mouse`).
    cursor_in_window: bool,
    /// Whether the cursor moved since the last hit-test. `CursorMoved` arrives at the
    /// mouse polling rate (1000+ Hz on gaming mice) and each `MouseMove` hit-tests the
    /// whole display list, which stalls crowded scenes. Moves are coalesced and applied
    /// at most once per movie frame, plus right before any click.
    mouse_move_pending: bool,
    modifiers: Modifiers,
    /// AQW build: frames shown since `fps_since`, for the FPS counter (see `count_fps`).
    fps_frames: u32,
    fps_since: Instant,
    min_window_size: LogicalSize<u32>,
    max_window_size: PhysicalSize<u32>,
    no_gui: bool,
    preferred_width: Option<f64>,
    preferred_height: Option<f64>,
    start_fullscreen: bool,
    loaded: LoadingState,
    time: Instant,
    next_frame_time: Option<Instant>,
    event_loop_proxy: EventLoopProxy<RuffleEvent>,
    perf: PerfStats,
    /// A SWF frame ran since the last draw; see `check_redraw`.
    frame_since_render: bool,
    last_redraw: Instant,
    /// With frame interpolation on: when the next in-between draw is due (see `pace_interpolation`).
    next_draw_time: Option<Instant>,
    /// When the last draw started.
    draw_started: Instant,
    /// Recent averages of a draw's and a SWF frame's time on the main thread (ms).
    draw_cost_ms: f64,
    frame_cost_ms: f64,
    /// Draw a new picture on every n-th refresh of the monitor.
    draw_divisor: u32,
    /// While interpolating: when the last new picture was due (see `pace_interpolation`).
    picture_due: Instant,
    /// Since when `draw_divisor` would have gone up / down (it changes only if that lasts).
    divisor_up_since: Option<Instant>,
    divisor_down_since: Option<Instant>,
    /// The monitor's refresh interval, and when it was last looked up.
    refresh_interval: Duration,
    refresh_checked: Option<Instant>,
}

/// Logs frame cost and pacing every few seconds (target `ruffle_aqw_perf`), for comparing builds.
#[derive(Default)]
struct PerfStats {
    window_start: Option<Instant>,
    last_frame: Option<Instant>,
    frames: u32,
    game_frames: u64,
    windows_logged: u32,
    /// Highest orphaned-avatar threshold a retention report was logged for.
    retention_reports_at: usize,
    frame_ms_sum: f64,
    frame_ms_max: f64,
    gap_ms_max: f64,
    /// Gaps between frames of 1.5x the frame time or more: long enough to see.
    hitches: u32,
    renders: u32,
    /// Pictures sent to the screen (movie renders plus GUI-only repaints).
    presents: u32,
    render_ms_sum: f64,
    render_ms_max: f64,
    /// `perf_counters::timers` at the start of this window.
    timers_at: Option<[u64; ruffle_render_wgpu::perf_counters::timers::LEN]>,
    /// `ruffle_core::render_walk_micros` at the start of this window.
    walk_at: Option<u64>,
    /// `ruffle_core::interpolation_replay::stats` at the start of this window.
    replay_at: Option<[u64; 5]>,
    /// Drawn frames per second of the last logged window.
    last_drawn_fps: f64,
    /// With frame interpolation on: the draws per second aimed for.
    interpolation_rate: f64,
}

impl PerfStats {
    /// `game_frames`: SWF frames this tick ran (more than 1 when catching up).
    fn frame(&mut self, now: Instant, frame_ms: f64, frame_time_ms: f64, game_frames: u64) {
        if let Some(last) = self.last_frame {
            let gap = now.duration_since(last).as_secs_f64() * 1000.0;
            self.gap_ms_max = self.gap_ms_max.max(gap);
            if gap >= frame_time_ms * 1.5 {
                self.hitches += 1;
            }
        }
        self.last_frame = Some(now);
        self.frames += 1;
        self.game_frames += game_frames;
        self.frame_ms_sum += frame_ms;
        self.frame_ms_max = self.frame_ms_max.max(frame_ms);
    }

    fn render(&mut self, render_ms: f64) {
        self.renders += 1;
        self.render_ms_sum += render_ms;
        self.render_ms_max = self.render_ms_max.max(render_ms);
    }

    /// Logs object counts every 12th window (~1 min); counting walks the whole display tree.
    /// Every 12th window (~1 min): logs object counts, and the first time orphaned avatars pass
    /// 40 and again past 120, a retention report showing what keeps them alive.
    fn log_object_counts(&mut self, player: &mut ruffle_core::Player) {
        self.windows_logged += 1;
        if self.windows_logged % 12 != 1 {
            return;
        }
        let (orphans, on_stage, under_orphans, classes) = player.display_object_counts();
        tracing::info!(
            target: "ruffle_aqw_perf",
            "objects: {on_stage} on stage, {orphans} orphans holding {under_orphans} more; weak listeners registered: {:?} (method, anonymous); top orphan classes: {classes:?}",
            ruffle_core::Player::weak_listener_counts()
        );
        let renderer_report = <dyn std::any::Any>::downcast_ref::<
            ruffle_render_wgpu::backend::WgpuRenderBackend<crate::gui::MovieView>,
        >(player.renderer())
        .map(|r| r.aqw_memory_report())
        .unwrap_or_default();
        let (encoder_backlog, in_flight) = wgpu_core::aqw_stats();
        let mut commit = 0usize;
        let mut zero = [0usize; 7];
        // SAFETY: plain out-pointers.
        unsafe {
            let [a, b, c, d, e, f, g] = &mut zero;
            libmimalloc_sys::mi_process_info(a, b, c, d, e, &mut commit, f, g);
        }
        // AQW build: what Windows counts against the graphics chip's memory (see
        // `mugetsu::video_memory`).
        let video_memory = crate::mugetsu::video_memory()
            .map(|(budget, usage)| {
                format!(
                    "video memory {} / {} MB budget (transient textures apart: {}); ",
                    usage >> 20,
                    budget >> 20,
                    if ruffle_render_wgpu::transient_textures_enabled() { "on" } else { "off" }
                )
            })
            .unwrap_or_default();
        tracing::info!(
            target: "ruffle_aqw_perf",
            "memory: {video_memory}{}; {renderer_report}; process commit {} MB, rust heap {} MB, wgpu: {encoder_backlog} encoders waiting to drop, {in_flight} submissions in flight",
            player.aqw_memory_report(),
            commit / (1 << 20),
            crate::HEAP_BYTES.load(std::sync::atomic::Ordering::Relaxed) / (1 << 20),
        );
        crate::heap_census::report();
        // `RUFFLE_RETENTION_TEST=<class>` runs one report at startup, to check the tool itself.
        if self.windows_logged == 1
            && let Ok(class) = std::env::var("RUFFLE_RETENTION_TEST")
        {
            player.log_retention_paths(&class, 3);
        }
        let avatars = classes
            .iter()
            .find(|(class, _)| class == "AvatarMC")
            .map_or(0, |(_, n)| *n);
        // The report walks the whole heap (a noticeable pause), so it only runs when asked for
        // with `RUFFLE_RETENTION_REPORTS` (it found the avatar leak; kept for future leaks).
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let enabled =
            *ENABLED.get_or_init(|| std::env::var_os("RUFFLE_RETENTION_REPORTS").is_some());
        let threshold = [40, 120]
            .into_iter()
            .filter(|_| enabled)
            .find(|&t| self.retention_reports_at < t && avatars >= t);
        if let Some(threshold) = threshold {
            self.retention_reports_at = threshold;
            player.log_retention_paths("AvatarMC", 3);
        }
        // A `heap-census` file next to the exe: what's in memory, at 2 minutes and then every
        // 30 (a short pause each), to find what grows in long sessions.
        static CENSUS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *CENSUS.get_or_init(|| crate::mugetsu::exe_flag("heap-census"))
            && (self.windows_logged == 25 || self.windows_logged % 360 == 1 && self.windows_logged > 1)
        {
            player.log_heap_census(40);
        }
    }

    /// Returns whether a window was logged.
    fn maybe_log(&mut self, now: Instant) -> bool {
        use ruffle_render_wgpu::perf_counters::timers;
        let start = *self.window_start.get_or_insert(now);
        let timers_at = *self
            .timers_at
            .get_or_insert_with(timers::snapshot);
        let walk_at = *self.walk_at.get_or_insert_with(ruffle_core::render_walk_micros);
        let replay_at = *self
            .replay_at
            .get_or_insert_with(ruffle_core::interpolation_replay::stats);
        let secs = now.duration_since(start).as_secs_f64();
        if secs < 5.0 {
            return false;
        }
        let avg = |sum: f64, n: u32| if n == 0 { 0.0 } else { sum / n as f64 };
        tracing::info!(
            target: "ruffle_aqw_perf",
            "game fps {:.1}, drawn {:.1}, presented {:.1} | frame avg {:.1}ms max {:.1}ms | worst gap {:.0}ms, hitches {} | render avg {:.1}ms max {:.1}ms",
            self.game_frames as f64 / secs,
            self.renders as f64 / secs,
            self.presents as f64 / secs,
            avg(self.frame_ms_sum, self.frames),
            self.frame_ms_max,
            self.gap_ms_max,
            self.hitches,
            avg(self.render_ms_sum, self.renders),
            self.render_ms_max,
        );
        let counts = ruffle_render_wgpu::perf_counters::take();
        let per_render = ruffle_render_wgpu::perf_counters::NAMES
            .iter()
            .zip(counts)
            .enumerate()
            .filter(|&(i, (_, n))| i == 0 || n > 0)
            .map(|(_, (name, n))| format!("{name} {:.1}", avg(n as f64, self.renders)))
            .collect::<Vec<_>>()
            .join(", ");
        tracing::info!(target: "ruffle_aqw_perf", "per drawn frame: {per_render}");
        // Where the render time went, per drawn frame (the rest of "render avg" is recording
        // the frame's commands on the main thread).
        let timers_now = timers::snapshot();
        let parts = timers::NAMES
            .iter()
            .zip(timers_now.iter().zip(timers_at))
            .filter(|(_, (a, b))| *a > b)
            .map(|(name, (a, b))| {
                format!("{name} {:.2}", avg((a - b) as f64 / 1000.0, self.renders))
            })
            .collect::<Vec<_>>()
            .join(", ");
        // AQW build: a `no-atlas` file next to the exe turns the layer atlas off from the next
        // window on, for A/B comparisons in the same scene.
        let atlas_was_on = ruffle_render_wgpu::atlas_enabled_now();
        ruffle_render_wgpu::set_atlas_enabled(!crate::mugetsu::exe_flag("no-atlas"));
        // Same for the cacheAsBitmap size steps (`no-cache-steps`).
        let steps_were_on = ruffle_core::cache_size_steps();
        ruffle_core::set_cache_size_steps(!crate::mugetsu::exe_flag("no-cache-steps"));
        // And for batched cacheAsBitmap filters (`no-glow-batch`).
        let batch_was_on = ruffle_render_wgpu::cache_batch_enabled_now();
        ruffle_render_wgpu::set_cache_batch_enabled(!crate::mugetsu::exe_flag("no-glow-batch"));
        tracing::info!(
            target: "ruffle_aqw_perf",
            "render time per drawn frame (ms): build_commands {:.2}, {parts}; atlas {}; cache steps {}; glow batch {}; interpolation {}",
            avg((ruffle_core::render_walk_micros() - walk_at) as f64 / 1000.0, self.renders),
            if atlas_was_on { "on" } else { "off" },
            if steps_were_on { "on" } else { "off" },
            if batch_was_on { "on" } else { "off" },
            if ruffle_core::frame_interpolation::enabled() {
                format!("on, aiming for {:.0}/s", self.interpolation_rate)
            } else {
                "off".to_string()
            }
        );
        // In-between pictures drawn from the last recorded one (see `replay`); a `no-replay`
        // file next to the exe walks the stage for every picture instead (A/B comparisons).
        use ruffle_core::interpolation_replay as replay;
        let replay_now = replay::stats();
        if replay_now != replay_at {
            let counts = replay::STAT_NAMES
                .iter()
                .zip(replay_now.iter().zip(replay_at))
                .filter(|(_, (a, b))| *a > b)
                .map(|(name, (a, b))| format!("{name} {}", a - b))
                .collect::<Vec<_>>()
                .join(", ");
            let reason = replay::take_reason()
                .map(|why| format!("; last reason: {why}"))
                .unwrap_or_default();
            tracing::info!(target: "ruffle_aqw_perf", "interpolation replay: {counts}{reason}");
        }
        replay::set_enabled(!crate::mugetsu::exe_flag("no-replay"));
        *self = PerfStats {
            window_start: Some(now),
            last_frame: self.last_frame,
            windows_logged: self.windows_logged,
            retention_reports_at: self.retention_reports_at,
            timers_at: Some(timers_now),
            walk_at: Some(ruffle_core::render_walk_micros()),
            replay_at: Some(replay_now),
            last_drawn_fps: self.renders as f64 / secs,
            interpolation_rate: self.interpolation_rate,
            ..Default::default()
        };
        true
    }
}

/// AQW build: when Mugetsu uses most of the graphics memory Windows allows it (85%), it frees
/// its spare textures and keeps far fewer from then on, until usage has stayed under 65% for a
/// minute (going back sooner refilled the spares within seconds and needed freeing again: each
/// round made the spares be recreated, costing hitches). Past the budget, Windows moves
/// graphics memory to system RAM (2-3 FPS on a 4 GB card after a long session), and Vulkan runs
/// out of memory instead (gear of players arriving not drawn). Checked every 5 s; does nothing
/// while there's room, so big cards are unaffected.
fn memory_watch(player: &mut ruffle_core::Player) {
    struct Watch {
        pressure: bool,
        low_since: Option<Instant>,
        last_log: Option<Instant>,
        last_sweep: Option<Instant>,
    }
    static WATCH: std::sync::Mutex<Watch> = std::sync::Mutex::new(Watch {
        pressure: false,
        low_since: None,
        last_log: None,
        last_sweep: None,
    });
    let mut watch = WATCH.lock().unwrap_or_else(|e| e.into_inner());
    // AQW build: on every GPU (and every OS), once a minute, free the tessellations of shapes
    // not drawn for 10 minutes: other maps, the gear of players who left. AQW keeps every SWF
    // it has loaded, so shape geometry otherwise grows for the whole session (800 MB in four
    // minutes of town hopping on a 32 GB card, where the memory-pressure path below never
    // runs). A shape drawn again later is tessellated again, as on its first draw. A
    // `no-shape-eviction` file next to the exe turns it off (A/B).
    if crate::mugetsu::shape_eviction_enabled()
        && watch
            .last_sweep
            .is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
    {
        watch.last_sweep = Some(Instant::now());
        let freed = ruffle_core::evict_unused_shapes(600);
        if freed > 0 {
            tracing::info!(
                target: "ruffle_aqw_perf",
                "freed {freed} shapes not drawn for 10 minutes"
            );
        }
    }
    let Some((budget, usage)) = crate::mugetsu::video_memory() else {
        return;
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
}

/// AQW build: the time each `perf_counters::timers` timer added since `before`, in ms
/// (only the nonzero ones).
fn timer_parts(before: &[u64; ruffle_render_wgpu::perf_counters::timers::LEN]) -> String {
    use ruffle_render_wgpu::perf_counters::timers;
    let parts = timers::NAMES
        .iter()
        .zip(timers::snapshot().iter().zip(before))
        .filter(|(_, (a, b))| *a > *b)
        .map(|(name, (a, b))| format!("{name} {:.1}", (a - b) as f64 / 1000.0))
        .collect::<Vec<_>>()
        .join(", ");
    if parts.is_empty() { "none of the timed work".to_string() } else { parts }
}

impl MainWindow {
    pub fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            // Don't render when nobody can see the result: while minimized, the
            // surface also keeps the size it had before, as `GuiController::resize`
            // refuses to reconfigure it to a zero size.
            if !self.minimized && !self.occluded {
                let started = Instant::now();
                let timers_before = ruffle_render_wgpu::perf_counters::timers::snapshot();
                let walk_before = ruffle_core::render_walk_micros();
                let mut player = self.player.get();
                let frame_budget_ms = player
                    .as_ref()
                    .map_or(1000.0 / 24.0, |player| 1000.0 / player.frame_rate());
                let mut rendered_movie = false;
                if let Some(ref mut player) = player {
                    // AQW build: egui asks for a repaint on every mouse move, and each repaint
                    // used to re-render the whole movie too. The movie view keeps the last
                    // frame, so only re-render it when a new SWF frame ran (or it was resized).
                    // Even if the movie is paused, user interaction with debug tools can change the render output
                    // With frame interpolation on, a new in-between picture goes out on the
                    // first refresh after it's due, every n refreshes' time; the refreshes in
                    // between show the last one again (see `pace_interpolation`). Timed, not
                    // counted: a long draw (e.g. 11 ms in Yulgar) makes presents miss refreshes.
                    let interpolating = self.next_draw_time.is_some();
                    let new_picture_due = if interpolating {
                        let interval = self.refresh_interval * self.draw_divisor;
                        let due = self.picture_due + interval;
                        if started + self.refresh_interval / 2 >= due {
                            // Keep the rhythm, unless it fell more than a picture behind.
                            self.picture_due = if started < due + interval { due } else { started };
                            true
                        } else {
                            false
                        }
                    } else {
                        self.frame_since_render
                    };
                    // Turu: Headless Mode's overlay hides the movie, so it isn't drawn.
                    let hidden = crate::mugetsu::plays_aqw() && crate::turu::headless_mode();
                    if !hidden && (new_picture_due || !player.is_playing() || extra_renders_enabled())
                    {
                        player.render();
                        rendered_movie = true;
                    }
                }
                let movie_ms = started.elapsed().as_secs_f64() * 1000.0;

                self.gui.render(player);
                self.perf.presents += 1;
                self.count_fps();
                self.frame_since_render = false;
                self.last_redraw = Instant::now();
                self.draw_started = started;
                if rendered_movie {
                    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
                    // Drawing the movie only (presenting can include waiting for the refresh);
                    // one-off slow draws, e.g. of new shapes, count at most 3x the average.
                    let cost = movie_ms.min(self.draw_cost_ms * 3.0);
                    self.draw_cost_ms = self.draw_cost_ms * 0.9 + cost * 0.1;
                    self.perf.render(total_ms);
                    // AQW build: say what a slow render (longer than 1.2 frames) spent its
                    // time on.
                    if total_ms > frame_budget_ms * 0.9 {
                        let parts = timer_parts(&timers_before);
                        let walk_ms =
                            (ruffle_core::render_walk_micros() - walk_before) as f64 / 1000.0;
                        tracing::info!(
                            target: "ruffle_aqw_perf",
                            "slow render {total_ms:.0} ms: movie {movie_ms:.0} ms, window/present {:.0} ms; build_commands {walk_ms:.1}, {parts} (ms)",
                            total_ms - movie_ms
                        );
                    }
                }
            }

            // Important that we return here, or we'll get a feedback loop with egui
            // (winit says redraw, egui hears redraw and says redraw, we hear redraw and tell winit to redraw...)
            return;
        }

        if self.gui.handle_event(&event) {
            // Event consumed by GUI.
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                // AQW build: shutdown steps are logged, to find where a slow exit waits.
                tracing::info!("closing: window close requested");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                // The movie view must be re-rendered at the new size.
                self.frame_since_render = true;
                // How minimizing shows up on Windows; winit still has no
                // `WindowEvent::Minimize`. (`Window::is_minimized` does exist by
                // now, but it has to be polled, and it wouldn't tell us anything
                // new: macOS reports minimized windows as occluded instead, and
                // on Wayland it always returns `None`.)
                self.minimized = size.width == 0 && size.height == 0;

                // Turu: every tab's player, so a tab shows at the window's size when picked.
                self.set_viewports(size);
                self.gui.window().request_redraw();
                if matches!(self.loaded, LoadingState::WaitingForResize) {
                    self.loaded = LoadingState::Loaded;
                }
            }
            WindowEvent::Occluded(occluded) => {
                self.occluded = occluded;
                if !occluded {
                    // Nothing repaints us on its own if the movie is paused.
                    self.gui.window().request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.gui.is_context_menu_visible() {
                    return;
                }

                // This is needed because some platforms (like macOS)
                // will fire this event at the same time as WindowEvent::MouseInput,
                // which may cause inaccurate behavior in the movie
                if position == self.mouse_pos {
                    return;
                }

                self.mouse_pos = position;
                self.mouse_move_pending = true;
                // Test switch: hit-test every mouse move, like stock Ruffle.
                if unbatched_mouse_moves() {
                    self.flush_mouse_move();
                }
            }
            WindowEvent::DroppedFile(file) => {
                if let Some(content_descriptor) = ContentDescriptor::new_local(&file, None) {
                    self.gui.create_movie(
                        &mut self.player,
                        LaunchOptions::from(&self.preferences),
                        content_descriptor,
                    );
                }
            }
            WindowEvent::Focused(true) => {
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                {
                    self.modifiers = Default::default();
                }
                self.player.handle_event(PlayerEvent::FocusGained);
            }
            WindowEvent::Focused(false) => {
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                {
                    self.modifiers = Default::default();
                }
                self.player.handle_event(PlayerEvent::FocusLost);
            }
            WindowEvent::MouseInput { button, state, .. } => {
                if self.gui.is_context_menu_visible() {
                    return;
                }

                // Keep MouseMove -> MouseDown ordering and an up-to-date hover target.
                self.flush_mouse_move();

                use ruffle_core::events::MouseButton as RuffleMouseButton;
                use winit::event::MouseButton;
                let (x, y) = self.gui.window_to_movie_position(self.mouse_pos);
                let button = match button {
                    MouseButton::Left => RuffleMouseButton::Left,
                    MouseButton::Right => RuffleMouseButton::Right,
                    MouseButton::Middle => RuffleMouseButton::Middle,
                    _ => RuffleMouseButton::Unknown,
                };
                let event = match state {
                    // TODO We should get information about click index from the OS,
                    //   but winit does not support that yet.
                    ElementState::Pressed => PlayerEvent::MouseDown {
                        x,
                        y,
                        button,
                        index: None,
                    },
                    ElementState::Released => PlayerEvent::MouseUp { x, y, button },
                };
                let handled = self.player.handle_event(event);
                if !handled && state == ElementState::Pressed && button == RuffleMouseButton::Right
                {
                    // Show context menu.
                    if let Some(mut player) = self.player.get() {
                        let context_menu = player.prepare_context_menu();
                        // AQW build: Mugetsu's settings too (AQW turns Flash's menu off).
                        let settings = Some(crate::mugetsu::MenuSettings::now());

                        // MouseUp event will be ignored when the context menu is shown,
                        // but it has to be dispatched when the menu closes.
                        let close_event = PlayerEvent::MouseUp {
                            x,
                            y,
                            button: RuffleMouseButton::Right,
                        };
                        self.gui.show_context_menu(context_menu, settings, close_event);
                    }
                }
                self.check_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.gui.is_context_menu_visible() {
                    return;
                }

                use ruffle_core::events::MouseWheelDelta;
                use winit::event::MouseScrollDelta;
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, dy) => MouseWheelDelta::Lines(dy.into()),
                    MouseScrollDelta::PixelDelta(pos) => MouseWheelDelta::Pixels(pos.y),
                };
                let event = PlayerEvent::MouseWheel { delta };
                self.player.handle_event(event);
                self.check_redraw();
            }
            WindowEvent::CursorEntered { .. } => {
                self.cursor_in_window = true;
                if let Some(mut player) = self.player.get() {
                    player.set_mouse_in_stage(true);
                    if player.needs_render() {
                        self.gui.window().request_redraw();
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor_in_window = false;
                self.mouse_move_pending = false;
                if let Some(mut player) = self.player.get() {
                    player.set_mouse_in_stage(false);
                }
                self.player.handle_event(PlayerEvent::MouseLeave);
                self.check_redraw();
            }
            WindowEvent::ModifiersChanged(new_modifiers) => {
                self.modifiers = new_modifiers;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if self.gui.is_context_menu_visible() {
                    return;
                }

                // Handle escaping from fullscreen.
                if let KeyEvent {
                    state: ElementState::Pressed,
                    logical_key: Key::Named(NamedKey::Escape),
                    ..
                } = event
                {
                    let _ = self
                        .event_loop_proxy
                        .send_event(RuffleEvent::ExitFullScreen);
                }

                let key = winit_input_to_ruffle_key_descriptor(&event);
                match event.state {
                    ElementState::Pressed => {
                        self.player.handle_event(PlayerEvent::KeyDown { key });
                        if let Some(control_code) =
                            winit_to_ruffle_text_control(&event, self.modifiers)
                        {
                            self.player
                                .handle_event(PlayerEvent::TextControl { code: control_code });
                        } else if let Some(text) = event.text {
                            for codepoint in text.chars() {
                                self.player
                                    .handle_event(PlayerEvent::TextInput { codepoint });
                            }
                        }
                    }
                    ElementState::Released => {
                        self.player.handle_event(PlayerEvent::KeyUp { key });
                    }
                };
                self.check_redraw();
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Enabled => {}
                Ime::Preedit(text, cursor) => {
                    self.player
                        .handle_event(PlayerEvent::Ime(ImeEvent::Preedit(text, cursor)));
                }
                Ime::Commit(text) => {
                    self.player
                        .handle_event(PlayerEvent::Ime(ImeEvent::Commit(text)));
                }
                Ime::Disabled => {}
            },
            _ => (),
        }
    }

    /// AQW build: counts shown frames for the FPS counter (Mugetsu's settings), updated twice a
    /// second; it's drawn with the next frame, so it causes no extra redraws.
    fn count_fps(&mut self) {
        self.fps_frames += 1;
        let elapsed = self.fps_since.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let fps = self.fps_frames as f32 / elapsed;
            crate::mugetsu::SHOWN_FPS.store(fps.to_bits(), std::sync::atomic::Ordering::Relaxed);
            self.fps_frames = 0;
            self.fps_since = Instant::now();
        }
    }

    /// AQW build: a change in Mugetsu's settings (the game's right-click menu).
    fn apply_setting(&mut self, setting: crate::mugetsu::Setting) {
        use crate::mugetsu::{self, Setting};
        match setting {
            Setting::Interpolation(on) => {
                ruffle_core::frame_interpolation::set_enabled(on);
                mugetsu::save_interpolation(on);
                tracing::info!("Frame interpolation turned {}", if on { "on" } else { "off" });
                #[cfg(windows)]
                crate::windows::sync_menu_checks(self.gui.window());
            }
            Setting::FlashEdges(on) => {
                ruffle_render::tessellator::set_flash_aa(on);
                mugetsu::save_flash_edges(on);
                // The drawing surface is made again for it (and shapes tessellated again).
                for (_, mut player) in self.player.all() {
                    let quality = player.quality();
                    player.renderer_mut().set_quality(quality);
                }
                self.gui.window().request_redraw();
            }
            Setting::Graphics(choice) => {
                #[cfg(windows)]
                crate::windows::choose_graphics(self.gui.window(), choice);
                #[cfg(target_os = "linux")]
                crate::linux::choose_graphics(self.gui.window(), choice);
                #[cfg(not(any(windows, target_os = "linux")))]
                let _ = choice;
            }
            Setting::ShowFps(on) => {
                mugetsu::set_show_fps(on);
                self.gui.window().request_redraw();
            }
            Setting::OpenLogFolder => mugetsu::open_log_folder(),
        }
    }

    fn on_metadata(&mut self, swf_header: HeaderExt) {
        let height_offset = if self.gui.window().fullscreen().is_some() || self.no_gui {
            0.0
        } else {
            crate::gui::top_height() as f64
        };

        // To prevent issues like waiting on resize indefinitely (#11364) or desyncing the window state on Windows,
        // do not resize while window is maximized.
        // Turu: a new tab's movie doesn't resize the window it opens in.
        let should_resize = !self.gui.window().is_maximized() && self.player.len() < 2;

        let (viewport_size, state) = if should_resize {
            let movie_width = swf_header.stage_size().width().to_pixels();
            let movie_height = swf_header.stage_size().height().to_pixels();

            let window_size: Size = match (self.preferred_width, self.preferred_height) {
                (None, None) => LogicalSize::new(movie_width, movie_height + height_offset).into(),
                (Some(width), None) => {
                    let scale = width / movie_width;
                    let height = movie_height * scale;
                    PhysicalSize::new(
                        width.max(1.0),
                        height.max(1.0) + height_offset * self.gui.window().scale_factor(),
                    )
                    .into()
                }
                (None, Some(height)) => {
                    let scale = height / movie_height;
                    let width = movie_width * scale;
                    PhysicalSize::new(
                        width.max(1.0),
                        height.max(1.0) + height_offset * self.gui.window().scale_factor(),
                    )
                    .into()
                }
                (Some(width), Some(height)) => PhysicalSize::new(
                    width.max(1.0),
                    height.max(1.0) + height_offset * self.gui.window().scale_factor(),
                )
                .into(),
            };

            let window_size = Size::clamp(
                window_size,
                self.min_window_size.into(),
                self.max_window_size.into(),
                self.gui.window().scale_factor(),
            );

            let viewport_size = self.gui.window().inner_size();
            let mut window_resize_denied = false;

            if let Some(new_viewport_size) = self.gui.window().request_inner_size(window_size) {
                if new_viewport_size != viewport_size {
                    self.gui.resize(new_viewport_size);
                } else {
                    tracing::warn!("Unable to resize window");
                    window_resize_denied = true;
                }
            }

            let viewport_size = self.gui.window().inner_size();

            // On X11 (and possibly other platforms), the window size is not updated immediately.
            // On a successful resize request, wait for the window to be resized to the requested size
            // before we start running the SWF (which can observe the viewport size in "noScale" mode)
            let state = if !window_resize_denied && window_size != viewport_size.into() {
                LoadingState::WaitingForResize
            } else {
                LoadingState::Loaded
            };

            (viewport_size, state)
        } else {
            (self.gui.window().inner_size(), LoadingState::Loaded)
        };

        self.loaded = state;

        self.gui.window().set_fullscreen(if self.start_fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        });
        self.gui.window().set_visible(true);

        let viewport_scale_factor = self.gui.window().scale_factor();
        for (_, mut player) in self.player.all() {
            player.set_viewport_dimensions(ViewportDimensions {
                width: viewport_size.width,
                height: viewport_size
                    .height
                    .saturating_sub((height_offset * viewport_scale_factor) as u32),
                scale_factor: viewport_scale_factor,
            });
        }
    }

    /// Turu: gives every tab's player the room under the nav bar (and tab strip).
    fn set_viewports(&self, size: PhysicalSize<u32>) {
        let scale_factor = self.gui.window().scale_factor();
        let height = size.height.saturating_sub(self.gui.height_offset() as u32);
        for (_, mut player) in self.player.all() {
            player.set_viewport_dimensions(ViewportDimensions {
                width: size.width,
                height,
                scale_factor,
            });
        }
    }

    /// Turu: opens, selects or closes a tab.
    fn tab_request(&mut self, request: crate::turu::TabRequest) {
        use crate::turu::{self, TabRequest};
        let shown_before = crate::gui::top_height();
        match request {
            TabRequest::Open(startup) => {
                let Some(movie_url) = self.preferences.cli.movie_url.clone() else {
                    return;
                };
                let (tab, _) = turu::add_tab(*startup);
                self.gui.add_tab(
                    &mut self.player,
                    LaunchOptions::from(&self.preferences),
                    ContentDescriptor {
                        url: movie_url,
                        root_content_path: None,
                    },
                    tab,
                );
                turu::set_active_tab(tab);
            }
            TabRequest::Select(tab) => {
                if self.player.select_tab(tab) {
                    turu::set_active_tab(tab);
                    self.gui.set_tab_volumes(&self.player);
                }
            }
            TabRequest::Close(tab) => {
                if self.player.len() < 2 {
                    return;
                }
                // The player first: it detaches the tab's bot, which `remove_tab` stops.
                self.player.remove_tab(tab);
                turu::remove_tab(tab);
                let active = turu::active_tab();
                turu::set_active_tab(active);
                self.player.select_tab(active);
                self.gui.set_tab_volumes(&self.player);
            }
        }
        // Only the active tab has the mouse: the others' stale "mouse in stage" kept hit-testing
        // it on every tick, and a tab shown again ignored clicks if the cursor had left the
        // window while it was in front.
        self.player.give_mouse(self.cursor_in_window);
        if crate::gui::top_height() != shown_before {
            // The tab strip came or went: the movie moves up or down.
            self.gui.reconfigure_surface();
        }
        self.set_viewports(self.gui.window().inner_size());
        self.frame_since_render = true;
        self.gui.window().request_redraw();
    }

    fn about_to_wait(&mut self, gilrs: Option<&mut Gilrs>) {
        if let Some(Event { event, .. }) = gilrs.and_then(|gilrs| gilrs.next_event()) {
            match event {
                EventType::ButtonPressed(button, _) => {
                    if let Some(button) = gilrs_button_to_gamepad_button(button) {
                        self.player
                            .handle_event(PlayerEvent::GamepadButtonDown { button });
                        self.check_redraw();
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if let Some(button) = gilrs_button_to_gamepad_button(button) {
                        self.player
                            .handle_event(PlayerEvent::GamepadButtonUp { button });
                        self.check_redraw();
                    }
                }
                _ => {}
            }
        }

        // Core loop
        // [NA] This used to be called `MainEventsCleared`, but I think the behaviour is different now.
        // We should look at changing our tick to happen somewhere else if we see any behavioural problems.
        if matches!(self.loaded, LoadingState::Loaded) {
            let new_time = Instant::now();
            // This runs after every batch of OS events (so ~1000 times a second while a
            // gaming mouse moves); only hit-test when a movie frame is actually due.
            if self.next_frame_time.is_none_or(|t| new_time >= t) {
                self.flush_mouse_move();
            }
            let dt = FloatDuration::from_std(new_time.duration_since(self.time));
            if dt.as_millis() > 0.0 {
                self.time = new_time;
                let perf = &mut self.perf;
                let frame_since_render = &mut self.frame_since_render;
                let frame_cost_ms = &mut self.frame_cost_ms;
                // Turu: the other tabs' players run too (not drawn, muted), when due.
                let background_next = self.player.tick_hidden(new_time);
                let mut tick_took = None; // Turu: counted in the tab's bot's stats.
                self.next_frame_time = self.player.get().map(|mut player| {
                    let frames_before = player.frames_run();
                    let timers_before = ruffle_render_wgpu::perf_counters::timers::snapshot();
                    let tick_started = Instant::now();
                    player.tick(dt);
                    tick_took = Some(tick_started.elapsed());
                    if player.frames_run() != frames_before {
                        *frame_since_render = true;
                        let frame_ms = new_time.elapsed().as_secs_f64() * 1000.0;
                        // One-off slow frames, e.g. a room loading, count at most 3x the
                        // average (as draws do): one 300 ms frame made interpolation think every
                        // frame cost 30 ms and drop to once per frame for seconds.
                        let cost = frame_ms.min(*frame_cost_ms * 3.0);
                        *frame_cost_ms = *frame_cost_ms * 0.9 + cost * 0.1;
                        // AQW build: say what a slow frame (scripts etc., longer than 1.2
                        // frames) spent its time on, e.g. when a crowded room loads.
                        if frame_ms > 1.2 * 1000.0 / player.frame_rate() {
                            tracing::info!(
                                target: "ruffle_aqw_perf",
                                "slow frame {frame_ms:.0} ms ({} frames run); of that: {} (ms)",
                                player.frames_run() - frames_before,
                                timer_parts(&timers_before)
                            );
                        }
                        perf.frame(
                            new_time,
                            frame_ms,
                            1000.0 / player.frame_rate(),
                            player.frames_run() - frames_before,
                        );
                    }
                    if perf.maybe_log(new_time) {
                        memory_watch(&mut player);
                        perf.log_object_counts(&mut player);
                        crate::mugetsu::low_quality_tick(&mut player, perf.last_drawn_fps);
                    }
                    new_time + player.time_til_next_frame()
                });
                if let Some(took) = tick_took {
                    self.player.record_tick(took);
                }
                if let Some(next) = background_next {
                    self.next_frame_time = Some(self.next_frame_time.map_or(next, |t| t.min(next)));
                }
                self.check_redraw();
            }
            self.pace_interpolation();
        }
    }

    /// AQW build: with frame interpolation on (title-bar menu), sends a picture on every refresh
    /// of the monitor: vsync makes each one wait for its refresh, which paces this exactly (a
    /// timer is ~1-2 ms late, a lot at 360 Hz). A new in-between picture is drawn on every n-th
    /// of them, the most the main thread can afford, so each stays up the same time; the others
    /// show the last one again, which costs little. Needs at least 1.5 new pictures per SWF
    /// frame, else it draws once per frame as without interpolation.
    fn pace_interpolation(&mut self) {
        self.next_draw_time = None;
        if !ruffle_core::frame_interpolation::enabled() || self.minimized || self.occluded {
            return;
        }
        let now = Instant::now();
        if self
            .refresh_checked
            .is_none_or(|checked| now.duration_since(checked) > Duration::from_secs(2))
        {
            let first = self.refresh_checked.is_none();
            self.refresh_checked = Some(now);
            let millihertz = self
                .gui
                .window()
                .current_monitor()
                .and_then(|monitor| monitor.refresh_rate_millihertz())
                .unwrap_or(60_000)
                .clamp(24_000, 360_000);
            let interval = Duration::from_secs_f64(1000.0 / millihertz as f64);
            if interval != self.refresh_interval || first {
                tracing::info!(
                    target: "ruffle_aqw_perf",
                    "frame interpolation: monitor refreshes at {:.1} Hz",
                    millihertz as f64 / 1000.0
                );
            }
            self.refresh_interval = interval;
        }
        let frame_rate = match self.player.get() {
            Some(player) if player.is_playing() => player.frame_rate(),
            _ => return,
        };
        // Draw on every refresh if the main thread can afford it (keeping a third of its time
        // spare), else on every 2nd, 3rd, ... refresh: an even rate looks smoother than an
        // uneven higher one. Each change of rate is a visible change of rhythm, so it only steps
        // to fewer pictures after needing to for half a second, and back to more after 3 seconds
        // of clearly affording it; 1 second for a big step back (at least twice the pictures,
        // or out of drawing once per frame), as after a room has loaded.
        let refresh_hz = 1.0 / self.refresh_interval.as_secs_f64();
        let spare_ms = (650.0 - self.frame_cost_ms * frame_rate).max(100.0);
        let divisor_for = |cost_ms: f64| {
            (refresh_hz * cost_ms.max(0.5) / spare_ms).ceil().clamp(1.0, 16.0) as u32
        };
        let needed = divisor_for(self.draw_cost_ms);
        let lasted = |since: &mut Option<Instant>, wanted: bool, secs: f64| {
            if !wanted {
                *since = None;
                return false;
            }
            now.duration_since(*since.get_or_insert(now)).as_secs_f64() >= secs
        };
        let affordable = divisor_for(self.draw_cost_ms * 1.3);
        let once_per_frame =
            (self.refresh_interval * self.draw_divisor).as_secs_f64() * 1.5 > 1.0 / frame_rate;
        let big_step = once_per_frame || affordable * 2 <= self.draw_divisor;
        if lasted(&mut self.divisor_up_since, needed > self.draw_divisor, 0.5)
            || lasted(
                &mut self.divisor_down_since,
                affordable < self.draw_divisor,
                if big_step { 1.0 } else { 3.0 },
            )
        {
            self.draw_divisor = needed;
            self.divisor_up_since = None;
            self.divisor_down_since = None;
        }
        let interval = self.refresh_interval * self.draw_divisor;
        self.perf.interpolation_rate = refresh_hz / self.draw_divisor as f64;
        if interval.as_secs_f64() * 1.5 > 1.0 / frame_rate {
            // Not even 1.5 draws per SWF frame: draw once per frame, as without interpolation.
            return;
        }
        // Requested right away, keeping the screen's queue of pictures full so a busy moment on
        // the main thread doesn't miss a refresh; presenting waits for vsync. The timer only
        // limits this if presenting doesn't wait.
        let early = self.refresh_interval / 2;
        let mut due = self.draw_started + early;
        if now >= due {
            self.gui.window().request_redraw();
            due = now + early;
        }
        self.next_draw_time = Some(due);
    }

    fn flush_mouse_move(&mut self) {
        if !std::mem::take(&mut self.mouse_move_pending) {
            return;
        }
        let (x, y) = self.gui.window_to_movie_position(self.mouse_pos);
        self.player.handle_event(PlayerEvent::MouseMove { x, y });
        self.check_redraw();
    }

    fn check_redraw(&self) {
        let player = self.player.get();
        // AQW build: like Flash Player's normal pacing, only draw once a new SWF frame has run.
        // AQW calls `updateAfterEvent()` from timers and hovers change between frames, which
        // made Ruffle redraw the same frame 2-9x per frame interval; in combat those extra
        // draws cost the main thread the time the next frame needed. A paused movie still
        // redraws on demand.
        let player_needs_render = player
            .map(|p| {
                p.needs_render()
                    && (self.frame_since_render || !p.is_playing() || extra_renders_enabled())
            })
            .unwrap_or_default();
        // AQW build: egui asks for a repaint on every mouse move. Each repaint presents, and
        // presenting faster than the monitor refreshes blocks the main thread, so moving the
        // mouse made frames late. With the menu bar hidden there is usually nothing to update,
        // so GUI-only repaints are capped at ~60/s (popups and dialogs still work).
        let gui_needs_render = self.gui.needs_render()
            && self.gui.wants_redraws()
            && (!self.no_gui || self.last_redraw.elapsed() >= Duration::from_millis(16));
        if player_needs_render || gui_needs_render {
            self.gui.window().request_redraw();
        }
    }
}

/// Turu: shows Turu's icon in the Dock. A bundled `.app` gets it from its `Info.plist`, but a
/// bare binary (`cargo run`) would show the generic executable icon.
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let png = include_bytes!("../assets/Assets.xcassets/RuffleMacIcon.iconset/icon_512x512@2x.png");
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png)) {
        // SAFETY: on the main thread, with a valid image.
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

pub struct App {
    main_window: Option<MainWindow>,
    runtime: Option<tokio::runtime::Runtime>,
    gilrs: Option<Gilrs>,
    event_loop_proxy: EventLoopProxy<RuffleEvent>,
    preferences: GlobalPreferences,
    font_database: fontdb::Database,
}

/// Enters the tokio runtime context.
/// This cannot be a method, as the borrow-checker would complain.
macro_rules! enter_runtime {
    ($this:expr) => {
        let _guard = $this.runtime.as_ref().map(|runtime| runtime.enter());
    };
}

impl App {
    pub fn new(preferences: GlobalPreferences) -> Result<(Self, EventLoop<RuffleEvent>), Error> {
        let event_loop = EventLoop::with_user_event().build()?;
        // AQW build: Ruffle never reads raw `DeviceEvent`s, but winit registers for raw input
        // while focused, so a 1000 Hz mouse woke the loop (and ran a player tick) ~1000x/s.
        event_loop.listen_device_events(winit::event_loop::DeviceEvents::Never);

        let mut font_database = fontdb::Database::default();
        font_database.load_system_fonts();

        let gilrs = Gilrs::new()
            .inspect_err(|err| {
                tracing::warn!("Gamepad support could not be initialized: {err}");
            })
            .ok();
        let event_loop_proxy = event_loop.create_proxy();
        let runtime = tokio::runtime::Runtime::new()?;

        Ok((
            Self {
                main_window: None,
                runtime: Some(runtime),
                gilrs,
                event_loop_proxy,
                font_database,
                preferences,
            },
            event_loop,
        ))
    }
}

impl ApplicationHandler<RuffleEvent> for App {
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        enter_runtime!(self);

        if cause == StartCause::Init {
            let movie_url = self.preferences.cli.movie_url.clone();
            // Turu: Turu's icon (64 px so the taskbar copy stays sharp; source in docs/turu/logo).
            let icon_bytes = include_bytes!("../assets/turu-icon-64.rgba");
            let icon =
                Icon::from_rgba(icon_bytes.to_vec(), 64, 64).expect("App icon should be correct");
            // Turu: macOS ignores the window icon; the Dock takes the app's.
            #[cfg(target_os = "macos")]
            set_dock_icon();

            let no_gui = self.preferences.cli.no_gui;
            let min_window_size = if no_gui {
                (16, 16)
            } else {
                (350, MENU_HEIGHT + 16)
            }
            .into();
            let preferred_width = self.preferences.cli.width;
            let preferred_height = self.preferences.cli.height;
            let start_fullscreen = self.preferences.cli.fullscreen;

            #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
            let mut window_attributes = WindowAttributes::default()
                .with_visible(false)
                .with_title(crate::window_title()) // AQW build
                .with_window_icon(Some(icon))
                .with_min_inner_size(min_window_size);

            #[cfg(target_os = "linux")]
            {
                use winit::platform::startup_notify::{
                    self, EventLoopExtStartupNotify, WindowAttributesExtStartupNotify,
                };
                use winit::platform::wayland::WindowAttributesExtWayland;
                window_attributes = window_attributes.with_name("rs.ruffle.Ruffle", "main");
                if let Some(token) = event_loop.read_token_from_env() {
                    startup_notify::reset_activation_token_env();
                    window_attributes = window_attributes.with_activation_token(token);
                }
            }

            let event_loop_proxy = self.event_loop_proxy.clone();
            let preferences = self.preferences.clone();
            let window = event_loop
                .create_window(window_attributes)
                .expect("Window should be created");
            let max_window_size = get_screen_size(&window);
            window.set_max_inner_size(Some(max_window_size));
            let window = Arc::new(window);
            let font_database = self.font_database.clone();

            let mut gui = GuiController::new(
                window.clone(),
                event_loop_proxy.clone(),
                preferences.clone(),
                &font_database,
                movie_url.clone(),
                no_gui,
            )
            .expect("GUI controller should be created");

            let mut player = PlayerController::new(
                event_loop_proxy.clone(),
                window.clone(),
                gui.descriptors().clone(),
                font_database,
                preferences.clone(),
                gui.file_picker(),
            );

            if let Some(movie_url) = &movie_url {
                gui.create_movie(
                    &mut player,
                    LaunchOptions::from(&preferences),
                    ContentDescriptor {
                        url: movie_url.clone(),
                        root_content_path: None,
                    },
                );
            } else {
                gui.show_open_dialog();
            }

            let mut loaded = LoadingState::Loading;

            if movie_url.is_none() {
                // No SWF provided on command line; show window with dummy movie immediately.
                window.set_visible(true);
                loaded = LoadingState::Loaded;
            }

            self.main_window = Some(MainWindow {
                preferences,
                gui,
                player,
                min_window_size,
                max_window_size,
                no_gui,
                preferred_width,
                preferred_height,
                start_fullscreen,
                loaded,
                minimized: false,
                occluded: false,
                mouse_pos: PhysicalPosition::new(0.0, 0.0),
                cursor_in_window: true,
                mouse_move_pending: false,
                fps_frames: 0,
                fps_since: Instant::now(),
                modifiers: Modifiers::default(),
                time: Instant::now(),
                next_frame_time: None,
                event_loop_proxy,
                perf: PerfStats::default(),
                frame_since_render: true,
                last_redraw: Instant::now(),
                next_draw_time: None,
                draw_started: Instant::now(),
                draw_cost_ms: 2.0,
                frame_cost_ms: 3.0,
                draw_divisor: 1,
                picture_due: Instant::now(),
                divisor_up_since: None,
                divisor_down_since: None,
                refresh_interval: Duration::from_secs_f64(1.0 / 60.0),
                refresh_checked: None,
            });
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: RuffleEvent) {
        enter_runtime!(self);

        match (&mut self.main_window, event) {
            (Some(main_window), RuffleEvent::TaskPoll(task)) => main_window.player.poll(task),
            (Some(main_window), RuffleEvent::TuruFlashCall(call)) => {
                let mut player = main_window.player.get_tab(call.tab());
                crate::turu::answer_flash_call(call, player.as_deref_mut());
            }
            (Some(main_window), RuffleEvent::TuruTab(request)) => main_window.tab_request(request),
            (None, RuffleEvent::TuruFlashCall(call)) => crate::turu::answer_flash_call(call, None),

            (Some(main_window), RuffleEvent::OnMetadata(swf_header)) => {
                main_window.on_metadata(swf_header)
            }

            (Some(main_window), RuffleEvent::ContextMenuItemClicked(index)) => {
                if let Some(mut player) = main_window.player.get() {
                    player.run_context_menu_callback(index);
                }
            }

            (Some(main_window), RuffleEvent::MugetsuSetting(setting)) => {
                main_window.apply_setting(setting);
            }

            (Some(main_window), RuffleEvent::BrowseAndOpen(options, open_type)) => {
                let event_loop = main_window.event_loop_proxy.clone();
                let picker = main_window.gui.file_picker();
                tokio::spawn(async move {
                    let picked = match open_type {
                        OpenType::File => {
                            picker.pick_ruffle_file(None).await.map(|file| (file, None))
                        }
                        OpenType::Directory => picker
                            .pick_ruffle_directory_and_content(None)
                            .await
                            .map(|(dir, file)| (file, Some(dir))),
                    };

                    if let Some(desc) =
                        picked.and_then(|(file, dir)| ContentDescriptor::new_local(&file, dir))
                    {
                        let _ = event_loop.send_event(RuffleEvent::Open(desc, options));
                    }
                });
            }

            (Some(main_window), RuffleEvent::Open(desc, options)) => {
                main_window
                    .gui
                    .create_movie(&mut main_window.player, *options, desc);
            }

            (Some(main_window), RuffleEvent::OpenDialog(descriptor)) => {
                main_window.gui.open_dialog(descriptor);
            }

            (Some(main_window), RuffleEvent::CloseFile) => {
                main_window.gui.window().set_title(&crate::window_title()); // Reset title since file has been closed.
                main_window.gui.close_movie(&mut main_window.player);
            }

            (Some(main_window), RuffleEvent::ExportBundle) => {
                main_window.gui.export_bundle();
            }

            (Some(main_window), RuffleEvent::EnterFullScreen) => {
                if let Some(mut player) = main_window.player.get()
                    && player.is_playing()
                {
                    player.set_fullscreen(true);
                }
            }

            (Some(main_window), RuffleEvent::ExitFullScreen) => {
                if let Some(mut player) = main_window.player.get()
                    && player.is_playing()
                {
                    player.set_fullscreen(false);
                }
            }

            (Some(main_window), RuffleEvent::PlayerNotification(notification)) => {
                match notification {
                    PlayerNotification::ImeNotification(ImeNotification::ImeReady {
                        purpose,
                        cursor_area,
                    }) => {
                        let ime_enabled = main_window.preferences.ime_enabled().unwrap_or(false);
                        main_window.gui.set_ime_allowed(ime_enabled);
                        main_window.gui.set_ime_purpose(purpose);
                        main_window.gui.set_ime_cursor_area(cursor_area);
                    }
                    PlayerNotification::ImeNotification(ImeNotification::ImePurposeUpdated(
                        purpose,
                    )) => {
                        main_window.gui.set_ime_purpose(purpose);
                    }
                    PlayerNotification::ImeNotification(ImeNotification::ImeCursorAreaUpdated(
                        cursor_area,
                    )) => {
                        main_window.gui.set_ime_cursor_area(cursor_area);
                    }
                    PlayerNotification::ImeNotification(ImeNotification::ImeNotReady) => {
                        main_window.gui.set_ime_allowed(false);
                    }
                }
            }

            (_, RuffleEvent::ExitRequested) => {
                event_loop.exit();
            }

            _ => {}
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        enter_runtime!(self);

        if let Some(main_window) = &mut self.main_window {
            main_window.window_event(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        enter_runtime!(self);

        if let Some(main_window) = &mut self.main_window {
            main_window.about_to_wait(self.gilrs.as_mut());

            // The event loop is finished; let's find out how long we need to wait for.
            // We don't need to worry about earlier update requests, as it's the
            // only place where we're setting control flow, and events cancel wait.
            // Note: the control flow might be set to `ControlFlow::WaitUntil` with a
            // timestamp in the past! Take that into consideration when changing this code.
            let wake = match (main_window.next_frame_time, main_window.next_draw_time) {
                (Some(frame), Some(draw)) => Some(frame.min(draw)),
                (frame, draw) => frame.or(draw),
            };
            if let Some(wake) = wake {
                event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        let started = Instant::now();
        tracing::info!("closing: dropping the player and renderer");
        // Turu: detach the bot first, or its watchdog takes the closed game for a disconnect.
        if crate::mugetsu::plays_aqw() {
            for (_, bot) in crate::turu::tabs() {
                bot.set_host(None);
            }
            crate::turu::close_butlers();
        }
        // `MainWindow` needs a tokio context to properly drop.
        {
            enter_runtime!(self);
            let _ = self.main_window.take();
        }
        tracing::info!(
            "closing: player and renderer dropped after {} ms",
            started.elapsed().as_millis()
        );

        // Manually stop the tokio runtime: this makes sure that any pending Player-bound futures
        // are properly cancelled and put back on the winit event loop before it closes, preventing
        // them from being dropped on the wrong thread and causing a panic.
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(std::time::Duration::from_secs(1));
        }
        tracing::info!("closing: done after {} ms", started.elapsed().as_millis());
    }
}

enum LoadingState {
    Loading,
    WaitingForResize,
    Loaded,
}

/// `RUFFLE_EXTRA_RENDERS` restores redrawing between SWF frames (on `updateAfterEvent`, hover
/// changes, ...), for A/B comparisons.
fn extra_renders_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("RUFFLE_EXTRA_RENDERS").is_some())
}

/// `RUFFLE_UNBATCHED_MOUSE` (test switch): send every mouse move to the movie as it arrives
/// instead of at most once per frame.
fn unbatched_mouse_moves() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("RUFFLE_UNBATCHED_MOUSE").is_some())
}
