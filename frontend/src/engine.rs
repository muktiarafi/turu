//! A player loop without a window: the desktop's `--headless` and Android's engine thread. One
//! thread owns the player and runs this loop: it ticks the player when a frame is due and, in
//! between, polls the player's futures (downloads, sockets) and answers the bot's Flash calls.
//! The frontend builds the player (with [`Executor`] and, when nobody can answer a dialog,
//! [`NoDialogs`]) and installs [`flash_host`] on the bot.

use crate::turu::{self, TuruFlashCall};
use ruffle_core::backend::navigator::OwnedFuture;
use ruffle_core::{FloatDuration, Player};
use ruffle_frontend_utils::backends::navigator::{FutureSpawner, NavigatorInterface};
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use url::Url;

/// skua.swf's stage.
pub const STAGE: (u32, u32) = (958, 550);

/// Work for the loop from other threads.
pub enum EngineEvent {
    /// A player future is ready to be polled.
    TaskPoll(async_task::Runnable),
    /// A bot thread calls one of skua.swf's callbacks.
    TuruFlashCall(TuruFlashCall),
    /// The frontend's work on the player (input, a new surface), run between ticks.
    Run(Box<dyn FnOnce(&mut Player) + Send>),
}

/// The active tab's Flash host: its calls go to the loop through `sender`.
pub fn flash_host(sender: Sender<EngineEvent>) -> turu::TuruFlashHost {
    turu::TuruFlashHost::new(turu::active_tab(), move |call| {
        sender.send(EngineEvent::TuruFlashCall(call)).is_ok()
    })
}

/// Ticks the player when a frame is due and handles events in between, until every sender is
/// gone.
pub fn run_loop(player: &Arc<Mutex<Player>>, events: &Receiver<EngineEvent>) {
    run_loop_with(player, events, |_| {});
}

/// [`run_loop`], calling `after_tick` with the player after each tick (to draw it where there's
/// a window).
pub fn run_loop_with(
    player: &Arc<Mutex<Player>>,
    events: &Receiver<EngineEvent>,
    mut after_tick: impl FnMut(&mut Player),
) {
    let perf = turu::bot().flash.perf().clone();
    let mut time = Instant::now();
    loop {
        let next_frame = {
            let mut player = player.lock().expect("the player isn't poisoned");
            let now = Instant::now();
            player.tick(FloatDuration::from_std(now.duration_since(time)));
            perf.record_tick(now.elapsed());
            time = now;
            after_tick(&mut player);
            now + player.time_til_next_frame()
        };
        loop {
            let wait = next_frame.saturating_duration_since(Instant::now());
            match events.recv_timeout(wait) {
                Ok(EngineEvent::TaskPoll(task)) => {
                    task.run();
                }
                Ok(EngineEvent::TuruFlashCall(call)) => {
                    let mut player = player.lock().expect("the player isn't poisoned");
                    turu::answer_flash_call(call, Some(&mut player));
                }
                Ok(EngineEvent::Run(work)) => {
                    work(&mut player.lock().expect("the player isn't poisoned"));
                }
                Err(RecvTimeoutError::Timeout) => break,
                // Senders live as long as the player and the bot.
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

/// Runs the player's futures (downloads, sockets) on the loop's thread.
pub struct Executor {
    sender: Sender<EngineEvent>,
}

impl Executor {
    pub fn new(sender: Sender<EngineEvent>) -> Self {
        Self { sender }
    }
}

impl<E: std::error::Error + 'static> FutureSpawner<E> for Executor {
    fn spawn(&self, future: OwnedFuture<(), E>) {
        let future = async {
            if let Err(e) = future.await {
                tracing::error!("Async error: {}", e);
            }
        };
        let sender = self.sender.clone();
        let schedule = move |task| {
            if sender.send(EngineEvent::TaskPoll(task)).is_err() {
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
pub struct NoDialogs;

impl NavigatorInterface for NoDialogs {
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
