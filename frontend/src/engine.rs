//! A player loop without a window: the desktop's `--headless` and Android's engine thread. One
//! thread owns the tabs' players ([`Players`]) and runs this loop: it ticks the active tab's
//! player when a frame is due and the others when theirs are (at the hidden tab's rate, as the
//! desktop's window does), and in between polls the players' futures (downloads, sockets),
//! answers the bots' Flash calls (each with its tab's player) and the frontend's tab requests.
//! The frontend builds each player (with [`Executor`] and, when nobody can answer a dialog,
//! [`NoDialogs`]) and installs [`flash_host`] on the tab's bot. `--headless` has one tab.

use crate::players::Players;
use crate::turu::{self, TabId, TabRequest, TuruFlashCall};
use ruffle_core::backend::navigator::OwnedFuture;
use ruffle_core::{FloatDuration, Player};
use ruffle_frontend_utils::backends::navigator::{FutureSpawner, NavigatorInterface};
use std::fs::File;
use std::path::Path;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Instant;
use url::Url;

/// skua.swf's stage.
pub const STAGE: (u32, u32) = (958, 550);

/// Work for the loop from other threads.
pub enum EngineEvent {
    /// A future of the tab's player is ready to be polled.
    TaskPoll(async_task::Runnable<TabId>),
    /// A bot thread calls one of skua.swf's callbacks (in its tab's player: `call.tab()`).
    TuruFlashCall(TuruFlashCall),
    /// The frontend's work on the active tab's player (input, a new surface), run between ticks.
    Run(Box<dyn FnOnce(&mut Player) + Send>),
    /// The UI opens, selects or closes a tab (`turu::request_tab`).
    Tab(TabRequest),
}

/// The tab's Flash host: its calls go to the loop through `sender`.
pub fn flash_host(sender: Sender<EngineEvent>, tab: TabId) -> turu::TuruFlashHost {
    turu::TuruFlashHost::new(tab, move |call| {
        sender.send(EngineEvent::TuruFlashCall(call)).is_ok()
    })
}

/// Ticks the players when a frame is due and handles events in between, until every sender is
/// gone, calling `after_tick` with the active player after each of its ticks. Tab requests are
/// ignored: `--headless` has one tab.
pub fn run_loop_with<E>(
    players: &mut Players<E>,
    events: &Receiver<EngineEvent>,
    mut after_tick: impl FnMut(&mut Player),
) {
    run_loop_paced(
        players,
        events,
        |player, step| {
            if let Step::Ticked { .. } = step {
                after_tick(player);
            }
            None
        },
        |_, _| tracing::warn!("This client has one tab: the request is ignored"),
    );
}

/// What [`run_loop_paced`]'s loop just did.
#[derive(Clone, Copy, Debug)]
pub enum Step {
    /// The player ticked; `frame_ms` is how long that took when it ran SWF frames.
    Ticked { frame_ms: Option<f64> },
    /// The time the callback asked for came, before the next frame.
    Due,
}

/// Runs the tabs' players until every sender is gone. The active tab's player ticks at its frame
/// rate, and `step` is called with it after each tick and at the time it returns (if that comes
/// before the next frame): the screen's in-between pictures with frame interpolation
/// (`crate::pacing`). The other tabs' players tick when their next frame or timer is due
/// ([`Players::tick_hidden`]). `on_tab` handles the UI's tab requests ([`EngineEvent::Tab`]).
pub fn run_loop_paced<E>(
    players: &mut Players<E>,
    events: &Receiver<EngineEvent>,
    mut step: impl FnMut(&mut Player, Step) -> Option<Instant>,
    mut on_tab: impl FnMut(&mut Players<E>, TabRequest),
) {
    let mut time = Instant::now();
    loop {
        let now = Instant::now();
        let (next_frame, mut next_step) = match players.get() {
            Some(mut player) => {
                let frames_before = player.frames_run();
                player.tick(FloatDuration::from_std(now.duration_since(time)));
                let took = now.elapsed();
                let frame_ms =
                    (player.frames_run() != frames_before).then_some(took.as_secs_f64() * 1000.0);
                let next_step = step(&mut player, Step::Ticked { frame_ms });
                let next_frame = now + player.time_til_next_frame();
                drop(player);
                players.record_tick(took);
                (next_frame, next_step)
            }
            // No player yet: only events.
            None => (now + crate::players::HIDDEN_TICK_MAX, None),
        };
        time = now;
        let mut next_hidden = players.tick_hidden(Instant::now(), |_| false).0;
        loop {
            let wake = [Some(next_frame), next_step, next_hidden]
                .into_iter()
                .flatten()
                .min()
                .unwrap_or(next_frame);
            let wait = wake.saturating_duration_since(Instant::now());
            match events.recv_timeout(wait) {
                Ok(EngineEvent::TaskPoll(task)) => {
                    // A closed tab's futures are dropped (which cancels them).
                    let tab = *task.metadata();
                    if let Some(i) = players
                        .slots()
                        .iter()
                        .position(|slot| slot.tab() == Some(tab))
                    {
                        players.run_for(i, || task.run());
                    }
                }
                Ok(EngineEvent::TuruFlashCall(call)) => {
                    let mut player = players.get_tab(call.tab());
                    turu::answer_flash_call(call, player.as_deref_mut());
                }
                Ok(EngineEvent::Run(work)) => {
                    if let Some(mut player) = players.get() {
                        work(&mut player);
                    }
                }
                Ok(EngineEvent::Tab(request)) => {
                    on_tab(players, request);
                    // Another player may be the active one: it ticks (and is drawn) now.
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {
                    let now = Instant::now();
                    if now >= next_frame {
                        break;
                    }
                    if next_hidden.is_some_and(|at| at <= now) {
                        next_hidden = players.tick_hidden(now, |_| false).0;
                    }
                    if next_step.is_some_and(|at| at <= now) {
                        next_step = match players.get() {
                            Some(mut player) => step(&mut player, Step::Due),
                            None => None,
                        };
                    }
                }
                // Senders live as long as the players and the bots.
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

/// Runs the player's futures (downloads, sockets) on the loop's thread.
pub struct Executor {
    sender: Sender<EngineEvent>,
    /// The tab whose player the futures belong to: they run with its thread-local state.
    tab: TabId,
}

impl Executor {
    /// The executor of `tab`'s player.
    pub fn new(sender: Sender<EngineEvent>, tab: TabId) -> Self {
        Self { sender, tab }
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
        let (runnable, task) = async_task::Builder::new()
            .metadata(self.tab)
            .spawn_local(|_| future, schedule);
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
