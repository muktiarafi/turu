//! Turu: the windowless mode (`--headless`). No window, no GPU and no sound: the player draws
//! with Ruffle's null renderer and this loop on the main thread runs it, its network tasks and
//! the bot's calls. With `--user`/`--password` and `--run-script` the bot logs in and farms on
//! its own; its log goes to the terminal. Works without a display (a Linux server).

use crate::player::LaunchOptions;
use crate::preferences::GlobalPreferences;
use crate::turu;
use anyhow::Error;
use ruffle_core::backend::navigator::SocketMode;
use ruffle_core::backend::ui::NullUiBackend;
use ruffle_core::{LoadBehavior, Player, PlayerBuilder};
use ruffle_frontend_utils::backends::navigator::ExternalNavigatorBackend;
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_render::backend::ViewportDimensions;
use ruffle_render::backend::null::NullRenderer;
use ruffle_render::quality::StageQuality;
use std::rc::Rc;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_frontend::engine::{self, EngineEvent, Executor, NoDialogs, STAGE};
use url::Url;

/// Runs `movie_url` (skua.swf) without a window until the process is stopped.
pub fn run(preferences: GlobalPreferences, movie_url: Url) -> Result<(), Error> {
    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let (sender, events) = channel();
    let player = build_player(&preferences, movie_url, sender.clone());
    turu::bot().set_host(Some(Arc::new(engine::flash_host(sender))));
    tracing::info!("Running without a window (--headless); stop with Ctrl+C");
    // Turu: Options > Application's launch updates (skill sets; the script it runs is updated on its own).
    turu::start_updates();
    let mut memory_log = turu::perf_log_every().map(|every| (every, Instant::now()));
    engine::run_loop_with(&player, &events, |player| {
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

/// With `TURU_PERF_LOG`: the windowed build's `memory:` line without its GPU parts, to see where
/// the process's memory goes (the allocator's resident and committed memory, the Rust heap that's
/// live, the SWFs the game loaded and the Flash objects it holds).
fn log_memory(player: &mut Player) {
    let mut info = [0usize; 8];
    // SAFETY: plain out-pointers.
    unsafe {
        let [elapsed, user, system, rss, peak_rss, commit, peak_commit, faults] = &mut info;
        libmimalloc_sys::mi_process_info(elapsed, user, system, rss, peak_rss, commit, peak_commit, faults);
    }
    let [_, _, _, rss, peak_rss, commit, peak_commit, _] = info.map(|bytes| bytes >> 20);
    tracing::info!(
        target: "turu_perf",
        "memory: process resident {rss} MB (peak {peak_rss}), commit {commit} MB (peak {peak_commit}), rust heap {} MB; {}",
        crate::HEAP_BYTES.load(std::sync::atomic::Ordering::Relaxed) >> 20,
        player.aqw_memory_report(),
    );
}

fn build_player(preferences: &GlobalPreferences, movie_url: Url, sender: Sender<EngineEvent>) -> Arc<Mutex<Player>> {
    let opt = LaunchOptions::from(preferences);
    let content = PlayingContent::DirectFile(ContentDescriptor {
        url: movie_url.clone(),
        root_content_path: None,
    });
    let navigator = ExternalNavigatorBackend::new(
        opt.player.base.clone().unwrap_or_else(|| movie_url.clone()),
        opt.player.referer.clone(),
        opt.player.cookie.clone(),
        Executor::new(sender),
        opt.proxy.clone(),
        opt.player.upgrade_to_https.unwrap_or_default(),
        opt.socket_allowed.clone(),
        // The game server is a TCP socket; there's nobody to ask.
        SocketMode::Allow,
        Rc::new(content),
        NoDialogs,
    );
    let navigator = crate::capture::CaptureNavigator::new(navigator, Some(turu::bot().capture.clone()));
    let player = PlayerBuilder::new()
        .with_navigator(navigator)
        .with_renderer(NullRenderer::new(ViewportDimensions {
            width: STAGE.0,
            height: STAGE.1,
            scale_factor: 1.0,
        }))
        .with_ui(NullUiBackend::new())
        .with_storage(preferences.storage_backend().create_backend(&opt))
        .with_external_interface(Box::new(turu::TuruExternalInterface { bot: turu::bot() }))
        .with_autoplay(true)
        .with_quality(StageQuality::Low)
        .with_max_execution_duration(opt.player.max_execution_duration.unwrap_or(Duration::MAX))
        .with_load_behavior(opt.player.load_behavior.unwrap_or(LoadBehavior::Streaming))
        .with_spoofed_url(opt.player.spoof_url.clone().map(|url| url.to_string()))
        .with_page_url(opt.player.spoof_url.clone().map(|url| url.to_string()))
        .with_player_version(opt.player.player_version)
        .with_player_runtime(opt.player.player_runtime.unwrap_or_default())
        .with_frame_rate(opt.player.frame_rate)
        .with_avm2_optimizer_enabled(opt.avm2_optimizer_enabled)
        .build();
    player.lock().expect("the player was just built").fetch_root_movie(
        movie_url.to_string(),
        opt.player.parameters,
        Box::new(|_| {}),
    );
    player
}
