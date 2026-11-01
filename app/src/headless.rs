//! Turu: the windowless mode (`--headless`). No window, no GPU and no sound: the player draws
//! with Ruffle's null renderer and this loop on the main thread runs it, its network tasks and
//! the bot's calls. With `--user`/`--password` and `--run-script` the bot logs in and farms on
//! its own; its log goes to the terminal. Works without a display (a Linux server).

use crate::player::LaunchOptions;
use crate::preferences::GlobalPreferences;
use crate::turu::{self, TuruFlashCall};
use anyhow::Error;
use ruffle_core::backend::navigator::{OwnedFuture, SocketMode};
use ruffle_core::backend::ui::NullUiBackend;
use ruffle_core::{FloatDuration, LoadBehavior, Player, PlayerBuilder};
use ruffle_frontend_utils::backends::navigator::{ExternalNavigatorBackend, FutureSpawner, NavigatorInterface};
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_render::backend::ViewportDimensions;
use ruffle_render::backend::null::NullRenderer;
use ruffle_render::quality::StageQuality;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use url::Url;

/// skua.swf's stage.
const STAGE: (u32, u32) = (958, 550);

/// Work for the loop from other threads.
pub enum HeadlessEvent {
    /// A player future is ready to be polled.
    TaskPoll(async_task::Runnable),
    /// A bot thread calls one of skua.swf's callbacks.
    TuruFlashCall(TuruFlashCall),
}

/// Runs `movie_url` (skua.swf) without a window until the process is stopped.
pub fn run(preferences: GlobalPreferences, movie_url: Url) -> Result<(), Error> {
    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();
    let (sender, events) = channel();
    let player = build_player(&preferences, movie_url, sender.clone());
    turu::bot().set_host(Some(Arc::new(turu::DesktopFlashHost::headless(sender))));
    tracing::info!("Running without a window (--headless); stop with Ctrl+C");
    run_loop(&player, &events);
    turu::bot().set_host(None);
    Ok(())
}

fn build_player(preferences: &GlobalPreferences, movie_url: Url, sender: Sender<HeadlessEvent>) -> Arc<Mutex<Player>> {
    let opt = LaunchOptions::from(preferences);
    let content = PlayingContent::DirectFile(ContentDescriptor {
        url: movie_url.clone(),
        root_content_path: None,
    });
    let navigator = ExternalNavigatorBackend::new(
        opt.player.base.clone().unwrap_or_else(|| movie_url.clone()),
        opt.player.referer.clone(),
        opt.player.cookie.clone(),
        HeadlessExecutor { sender },
        opt.proxy.clone(),
        opt.player.upgrade_to_https.unwrap_or_default(),
        opt.socket_allowed.clone(),
        // The game server is a TCP socket; there's nobody to ask.
        SocketMode::Allow,
        Rc::new(content),
        HeadlessNavigatorInterface,
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
        opt.player.parameters.clone(),
        Box::new(|_| {}),
    );
    player
}

/// Ticks the player when a frame is due and handles events in between.
fn run_loop(player: &Arc<Mutex<Player>>, events: &Receiver<HeadlessEvent>) {
    let mut time = Instant::now();
    loop {
        let next_frame = {
            let mut player = player.lock().expect("the player isn't poisoned");
            let now = Instant::now();
            player.tick(FloatDuration::from_std(now.duration_since(time)));
            time = now;
            now + player.time_til_next_frame()
        };
        loop {
            let wait = next_frame.saturating_duration_since(Instant::now());
            match events.recv_timeout(wait) {
                Ok(HeadlessEvent::TaskPoll(task)) => {
                    task.run();
                }
                Ok(HeadlessEvent::TuruFlashCall(call)) => {
                    let mut player = player.lock().expect("the player isn't poisoned");
                    turu::answer_flash_call(call, Some(&mut player));
                }
                Err(RecvTimeoutError::Timeout) => break,
                // Senders live as long as the player and the bot.
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

/// Runs the player's futures (downloads, sockets) on the loop's thread.
struct HeadlessExecutor {
    sender: Sender<HeadlessEvent>,
}

impl<E: std::error::Error + 'static> FutureSpawner<E> for HeadlessExecutor {
    fn spawn(&self, future: OwnedFuture<(), E>) {
        let future = async {
            if let Err(e) = future.await {
                tracing::error!("Async error: {}", e);
            }
        };
        let sender = self.sender.clone();
        let schedule = move |task| {
            if sender.send(HeadlessEvent::TaskPoll(task)).is_err() {
                tracing::error!("Couldn't schedule task - the loop has stopped");
            }
        };
        let (runnable, task) = async_task::spawn_local(future, schedule);
        task.detach();
        runnable.schedule();
    }
}

/// No dialogs: websites aren't opened, files and sockets are allowed.
#[derive(Clone)]
struct HeadlessNavigatorInterface;

impl NavigatorInterface for HeadlessNavigatorInterface {
    fn navigate_to_website(&self, url: Url) {
        tracing::info!("The game wants to open {url} (not opened without a window)");
    }

    async fn open_file(&self, path: &Path) -> std::io::Result<File> {
        File::open(path)
    }

    async fn confirm_socket(&self, _host: &str, _port: u16) -> bool {
        true
    }
}
