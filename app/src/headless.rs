//! Turu: the windowless mode (`--headless`). No window, no GPU and no sound: the player draws
//! with Ruffle's null renderer and this loop on the main thread runs it, its network tasks and
//! the bot's calls. With `--user`/`--password` and `--run-script` the bot logs in and farms on
//! its own; its log goes to the terminal. Works without a display (a Linux server).

use crate::backends::ClientUiBackend;
use crate::cli::OpenUrlMode;
use crate::player::LaunchOptions;
use crate::preferences::GlobalPreferences;
use crate::turu;
use anyhow::Error;
use ruffle_core::backend::navigator::SocketMode;
use ruffle_core::backend::ui::NullUiBackend;
use ruffle_core::config::Letterbox;
use ruffle_core::{LoadBehavior, Player, PlayerBuilder};
use ruffle_frontend_utils::backends::audio::CpalAudioBackend;
use ruffle_frontend_utils::backends::navigator::ExternalNavigatorBackend;
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_render::backend::RenderBackend;
use ruffle_render::backend::ViewportDimensions;
use ruffle_render::backend::null::NullRenderer;
use ruffle_render::quality::StageQuality;
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_frontend::engine::{self, EngineEvent, Executor, NoDialogs, STAGE};
use turu_frontend::players::{Players, Slot};
use url::Url;

/// Runs `movie_url` (turu.swf) without a window until the process is stopped.
pub fn run(preferences: GlobalPreferences, movie_url: Url) -> Result<(), Error> {
    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let (sender, events) = channel();
    // One tab, through the loop that runs Android's tabs.
    let tab = turu::active_tab();
    let mut players = Players::new();
    let renderer = NullRenderer::new(ViewportDimensions {
        width: STAGE.0,
        height: STAGE.1,
        scale_factor: 1.0,
    });
    let player = build_player(
        &preferences,
        movie_url,
        sender.clone(),
        tab,
        renderer,
        None,
        StageQuality::Low,
    );
    players.add(Slot::new(Some(tab), player, ()));
    turu::bot().set_host(Some(Arc::new(engine::flash_host(sender, tab))));
    // Turu: nothing is drawn, so in game it runs at a hidden tab's 2 FPS (`api::options`; the
    // login screen keeps full speed). Halved the process's CPU on a farm, with the same kills.
    turu::bot().state.lock().hidden_tab = true;
    tracing::info!("Running without a window (--headless); stop with Ctrl+C");
    runtime.spawn(quit_on_signal());
    // Turu: Options > Application's launch updates (skill sets; the script it runs is updated on its own).
    turu::start_updates();
    let mut memory_log = turu::perf_log_every().map(|every| (every, Instant::now()));
    let mut last_collect = Instant::now();
    engine::run_loop_with(&mut players, &events, |player| {
        // Turu: hand the memory mimalloc keeps free back to the system (see `release_free_memory`).
        if last_collect.elapsed() >= Duration::from_secs(60) {
            last_collect = Instant::now();
            crate::release_free_memory();
        }
        if let Some((every, last)) = &mut memory_log
            && last.elapsed() >= *every
        {
            *last = Instant::now();
            log_memory(player);
        }
    });
    turu::bot().set_host(None);
    Ok(())
}

/// How long a signal's clean quit may take before the process quits anyway: under `docker stop`'s
/// 10 s, after which Docker kills it.
const QUIT_LIMIT: Duration = Duration::from_secs(8);

/// Turu: SIGTERM (`systemctl stop`, `docker stop`) or Ctrl+C quits cleanly (`turu::shut_down`:
/// the script stopped, logged out, Butler and the tapes closed). A second signal, or
/// [`QUIT_LIMIT`] without the clean quit ending, quits at once with status 1.
async fn quit_on_signal() {
    if let Err(e) = next_signal().await {
        tracing::warn!("Couldn't listen for Ctrl+C or SIGTERM: they end the process at once ({e})");
        return;
    }
    tracing::info!("Stopping: the script, then logging out (Ctrl+C again quits at once)");
    if let Err(e) = std::thread::Builder::new()
        .name("Turu Shutdown".into())
        .spawn(|| turu::shut_down())
    {
        tracing::error!("Couldn't start quitting cleanly: {e}");
    } else {
        tokio::select! {
            _ = next_signal() => tracing::warn!("Quitting at once"),
            () = tokio::time::sleep(QUIT_LIMIT) => {
                tracing::warn!("Couldn't quit cleanly in {} s: quitting at once", QUIT_LIMIT.as_secs());
            }
        }
    }
    turu_bot::tape::finish_all();
    std::process::exit(1);
}

/// The next Ctrl+C (SIGINT), or SIGTERM on Unix.
async fn next_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = term.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await
}

/// With `TURU_PERF_LOG`: the windowed build's `memory:` line without its GPU parts, to see where
/// the process's memory goes (the allocator's resident and committed memory, the Rust heap that's
/// live, the SWFs the game loaded and the Flash objects it holds).
pub(crate) fn log_memory(player: &mut Player) {
    let mut info = [0usize; 8];
    // SAFETY: plain out-pointers.
    unsafe {
        let [elapsed, user, system, rss, peak_rss, commit, peak_commit, faults] = &mut info;
        libmimalloc_sys::mi_process_info(elapsed, user, system, rss, peak_rss, commit, peak_commit, faults);
    }
    let [_, _, _, rss, peak_rss, commit, peak_commit, _] = info.map(|bytes| bytes >> 20);
    tracing::info!(
        target: "turu_perf",
        "memory: process resident {rss} MB (peak {peak_rss}), commit {commit} MB (peak {peak_commit}), rust heap {} MB; {}; {}",
        crate::HEAP_BYTES.load(std::sync::atomic::Ordering::Relaxed) >> 20,
        player.aqw_memory_report(),
        // At the end: `tools/memrun` and `tools/farmrun` read the line up to the GC objects.
        crate::memory_split::report(),
    );
    // Turu: uncaught AS3 errors (a repeat is logged once, then only counted).
    tracing::info!(target: "turu_perf", "{}", ruffle_core::take_error_stats());
    // Turu: what's loaded most, to tell which URL's movies pile up.
    tracing::info!(target: "turu_perf", "memory census: {}", player.aqw_movie_census(8));
    // Turu: what costs the most while loaded (a map, a monster, gear; images with their pixels).
    tracing::info!(target: "turu_perf", "memory biggest: {}", player.aqw_movie_sizes(8));
    // Turu: what the disk cache spared the network, since the start.
    if let Some(summary) = turu_frontend::game_files::summary() {
        tracing::info!(target: "turu_perf", "game files: {summary}");
    }
    // Turu: `TURU_RETAIN_URL=<part of a URL>`: every 5th reading, what keeps the `LoaderInfo`s
    // of those movies alive (walks the whole heap: a short pause).
    static READINGS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    // Turu: TEMPORARY (leak investigation, uncommitted): `TURU_HEAP_CENSUS=<n>`: every nth
    // reading, the GC values reachable from the root by kind (AS3 objects by class).
    static CENSUS_READINGS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    if let Some(n) = std::env::var("TURU_HEAP_CENSUS")
        .ok()
        .and_then(|n| n.parse::<u32>().ok())
        && CENSUS_READINGS
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .is_multiple_of(n.max(1))
    {
        player.log_heap_census(150);
    }
    if let Ok(part) = std::env::var("TURU_RETAIN_URL")
        && READINGS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 5 == 4
    {
        player.log_loader_retention(&part, 3);
    }
}

/// turu.swf's player without a window, drawing with `renderer` (the null renderer here; the
/// client mode's offscreen one, `client.rs`), with its mouse off the stage.
pub(crate) fn build_player(
    preferences: &GlobalPreferences,
    movie_url: Url,
    sender: Sender<EngineEvent>,
    tab: turu::TabId,
    renderer: impl RenderBackend + 'static,
    // A shell's client's (`client.rs`): its fonts and clipboard, sound, and where the player's
    // notifications go (a text field wanting an input method). `--headless` has nobody to hear
    // or see it.
    client: Option<(
        ClientUiBackend,
        async_channel::Sender<ruffle_core::events::PlayerNotification>,
    )>,
    quality: StageQuality,
) -> Arc<Mutex<Player>> {
    let opt = LaunchOptions::from(preferences);
    let content = PlayingContent::DirectFile(ContentDescriptor {
        url: movie_url.clone(),
        root_content_path: None,
    });
    let navigator = ExternalNavigatorBackend::new(
        opt.player.base.clone().unwrap_or_else(|| movie_url.clone()),
        opt.player.referer.clone(),
        opt.player.cookie.clone(),
        Executor::new(sender, tab),
        opt.proxy.clone(),
        opt.player.upgrade_to_https.unwrap_or_default(),
        opt.socket_allowed.clone(),
        // The game server is a TCP socket; there's nobody to ask.
        SocketMode::Allow,
        Rc::new(content),
        // A tab's client opens the game's links through the shell, unless Options deny it.
        NoDialogs {
            open_links: client.is_some() && preferences.open_url_mode() != OpenUrlMode::Deny,
        },
    )
    .with_http_cache(turu_frontend::game_files::cache());
    let navigator = crate::capture::CaptureNavigator::new(navigator, Some(turu::bot().capture.clone()));
    let mut builder = PlayerBuilder::new();
    match client {
        Some((ui, notifications)) => {
            builder = builder.with_ui(ui).with_notification_sender(notifications);
            match CpalAudioBackend::new(preferences.output_device_name().as_deref()) {
                Ok(audio) => builder = builder.with_audio(audio),
                Err(e) => tracing::error!("No sound: couldn't open the audio device: {e}"),
            }
        }
        None => builder = builder.with_ui(NullUiBackend::new()),
    }
    let player = builder
        .with_navigator(navigator)
        .with_renderer(renderer)
        .with_storage(preferences.storage_backend().create_backend(&opt))
        .with_external_interface(Box::new(turu::TuruExternalInterface { bot: turu::bot() }))
        // Turu: the game's packets, from its socket (R5).
        .with_socket_observer(Box::new(turu::TuruSocketObserver { bot: turu::bot() }))
        .with_autoplay(true)
        // Turu: as the window's player (`player.rs`): `--quality`, the letterbox, and the game
        // centred in a taller area unless `--align` says otherwise.
        .with_quality(opt.player.quality.unwrap_or(quality))
        .with_letterbox(opt.player.letterbox.unwrap_or(Letterbox::On))
        .with_align(
            opt.player.align.unwrap_or_default(),
            opt.player.align.is_none() || opt.player.force_align.unwrap_or_default(),
        )
        .with_scale_mode(
            opt.player.scale.unwrap_or_default(),
            opt.player.force_scale.unwrap_or_default(),
        )
        .with_gamepad_button_mapping(opt.gamepad_button_mapping.clone())
        .with_max_execution_duration(opt.player.max_execution_duration.unwrap_or(Duration::MAX))
        .with_load_behavior(opt.player.load_behavior.unwrap_or(LoadBehavior::Streaming))
        .with_spoofed_url(opt.player.spoof_url.clone().map(|url| url.to_string()))
        .with_page_url(opt.player.spoof_url.clone().map(|url| url.to_string()))
        .with_player_version(opt.player.player_version)
        .with_player_runtime(opt.player.player_runtime.unwrap_or_default())
        .with_frame_rate(opt.player.frame_rate)
        .with_avm2_early_binding_enabled(opt.avm2_early_binding_enabled)
        .with_avm2_dse_enabled(opt.avm2_dse_enabled)
        .build();
    {
        let mut locked = player.lock().expect("the player was just built");
        // Turu: there's no mouse. Ruffle starts with it on the stage, and then every update
        // hit-tests the whole display list for it (`run_mouse_pick`): 13% of the player's time at
        // 30 FPS, 75% at 2 FPS (updates run for timers and the socket whatever the frame rate).
        locked.set_mouse_in_stage(false);
        crate::player::set_default_fonts(&mut locked);
        locked.fetch_root_movie(movie_url.to_string(), opt.player.parameters, Box::new(|_| {}));
    }
    player
}
