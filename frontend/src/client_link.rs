//! Turu: a tab's side of its link to a shell (docs/TAB_PROCESSES_PROPOSAL.md, M5 and M3), shared
//! by the desktop's clients (`app/src/client.rs`, over a socket) and Android's slots (over Binder).
//! The tab strip shows the shell's tabs: its "+", Army > Accounts' "In a tab", selecting and
//! closing a tab and the bar's Grid toggle become messages to the shell; the shell's tab list
//! comes back to the strip, Army Control and the monitor hub; the tab's name and whether it's busy
//! go to the shell when they change, and so does Headless Mode, which the shell passes on to the
//! other tabs (each in a process of its own: `turu::start_headless_mode_sync` reaches only this
//! process's).

use crate::shell::{Answered, TabGame, TabInfo, ToShell};
use crate::turu;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What to do with a request's answer.
type Then = Box<dyn FnOnce(Answered) + Send>;

/// The requests made of the shell that wait for its answer (a file dialog, a picker, the
/// installer), by ID (docs/NATIVE_UI_PROPOSAL.md, L3).
static ASKED: Mutex<Option<HashMap<u32, Then>>> = Mutex::new(None);
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Asks the shell for something it answers ([`ToShell::FileDialog`], `PickFile`, `SaveFile`,
/// `Install`): `send` sends the request with the ID it's given, and `then` gets the answer
/// ([`answered`]). The ID.
pub fn ask(send: impl FnOnce(u32), then: impl FnOnce(Answered) + Send + 'static) -> u32 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    ASKED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .insert(id, Box::new(then));
    send(id);
    id
}

/// The shell's answer to request `id` ([`ToClient::Answer`](crate::shell::ToClient::Answer)):
/// handed to what asked. An answer to no request waiting (one answered already, or of a client
/// before a restart) does nothing; false then.
pub fn answered(id: u32, answer: Answered) -> bool {
    let then = ASKED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .and_then(|asked| asked.remove(&id));
    match then {
        Some(then) => {
            then(answer);
            true
        }
        None => {
            tracing::debug!("An answer to request {id}, which nothing waits for");
            false
        }
    }
}

/// Sends a message to the shell (dropped while there's none).
pub type Sender = Arc<dyn Fn(ToShell) + Send + Sync>;

/// Routes this process's tab requests to the shell through `send`, and reports the tab's name and
/// whether it's busy every second while `open` says the link is there.
pub fn install(send: Sender, open: impl Fn() -> bool + Send + 'static) {
    let tabs = send.clone();
    turu::set_tab_requests(Box::new(move |request| {
        match request {
            turu::TabRequest::Open(startup) => tabs(ToShell::OpenTab {
                username: startup.username,
                password: startup.password,
                server: startup.server,
                script: startup
                    .script
                    .map(|path| path.to_string_lossy().into_owned()),
                run_script: startup.run_script,
                playlist: startup
                    .playlist
                    .map(|path| path.to_string_lossy().into_owned()),
                run_playlist: startup.run_playlist,
                options_storage: startup.options_storage,
                exit_on_stop: startup.exit_on_stop,
            }),
            // The shell lays the tiles out in what Turu's UI leaves of the window.
            turu::TabRequest::Grid(on) => tabs(ToShell::Grid {
                on,
                left: crate::layout::left_width() as f32,
                top: crate::layout::top_height() as f32,
            }),
            _ => {}
        }
        true
    }));
    let actions = send.clone();
    turu::set_shell_actions(Box::new(move |action| {
        actions(match action {
            turu::ShellTabAction::Select(id) => ToShell::SelectTab { id },
            turu::ShellTabAction::Close(id) => ToShell::CloseTab { id },
            turu::ShellTabAction::Open(screen) => ToShell::OpenScreen { screen },
        });
    }));
    report_status(send, open);
}

/// The shell's Army clients, from its `ArmyClients` message.
pub fn set_army_clients(clients: Vec<crate::shell::ArmyClientInfo>) {
    turu::set_shell_clients(
        clients
            .into_iter()
            .map(|client| turu::ShellClient {
                username: client.username,
                label: client.label,
                headless: client.headless,
                api: client.api.parse().ok(),
                token: client.token,
                exited: client.exited,
            })
            .collect(),
    );
}

/// The shell's tab list, from its `Tabs` message: `you` is this tab.
pub fn set_tabs(tabs: Vec<TabInfo>, shown: u32, you: Option<u32>, rail_top: Option<f32>, native_rail: bool) {
    turu::set_shell_rail_top(rail_top);
    turu::set_shell_native_rail(native_rail);
    let tabs = tabs
        .into_iter()
        .map(|tab| turu::ShellTab {
            me: you == Some(tab.id),
            id: tab.id,
            label: tab.label,
            busy: tab.busy,
            down: tab.down,
            api: tab.api.and_then(|api| api.parse().ok()),
            token: tab.token.unwrap_or_default(),
            user: tab.user,
        })
        .collect();
    turu::set_shell_tabs(tabs, shown);
}

/// Headless Mode as this tab last sent it to the shell or got it from the shell (`None`: not
/// seen yet). A tab's own value at start isn't sent: the shell gives it the others' instead.
static HEADLESS_MODE: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

/// Another tab turned Headless Mode on or off (the shell's `HeadlessMode`): this process's tabs
/// take it, and it isn't sent back.
pub fn set_headless_mode(on: bool) {
    *HEADLESS_MODE.lock().unwrap_or_else(|e| e.into_inner()) = Some(on);
    for (_, bot) in turu::tabs() {
        if bot.opts().headless_mode != on {
            bot.options.write().headless_mode = on;
        }
    }
}

/// Headless Mode to the shell when this tab changed it (Game Options, a HotKey, a script, Army
/// Control, the control API) since it was last sent or got.
fn report_headless_mode(send: &Sender, on: bool) {
    let mut seen = HEADLESS_MODE.lock().unwrap_or_else(|e| e.into_inner());
    match *seen {
        Some(last) if last != on => {
            *seen = Some(on);
            drop(seen);
            send(ToShell::HeadlessMode { on });
        }
        Some(_) => {}
        None => *seen = Some(on),
    }
}

/// The tab's name, whether its script or Auto runs, where its game is (in game and the map, or
/// out) and its script, to the shell when any changes (checked every second), and again after
/// [`resend_status`] (a new shell); Headless Mode when it changes.
fn report_status(send: Sender, open: impl Fn() -> bool + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("Turu Shell Status".into())
        .spawn(move || {
            let mut sent = None;
            // Whether it has been in game: out of the game after that is a drop (or a logout), not
            // a first login.
            let mut was_in_game = false;
            while open() {
                let bot = turu::bot();
                report_headless_mode(&send, bot.opts().headless_mode);
                let (in_game, map) = {
                    let state = bot.state.lock();
                    (state.in_game, state.map_name.clone().filter(|_| state.in_game))
                };
                let game = if in_game {
                    was_in_game = true;
                    TabGame::InGame
                } else if bot.servers.is_relogging() {
                    TabGame::Relogging
                } else if was_in_game {
                    TabGame::Dropped
                } else {
                    TabGame::LoggedOut
                };
                let script = bot
                    .scripts
                    .loaded()
                    .and_then(|path| path.file_stem().map(|s| s.to_string_lossy().into_owned()));
                let now = (
                    turu::tab_label(&bot),
                    crate::turu::busy(&bot),
                    crate::turu::waits_for(&bot).map(str::to_owned),
                    crate::layout::rail_hidden(),
                    (game, map, script),
                    bot.opts().streamer_mode,
                    HELD.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                );
                let resend = std::mem::take(&mut *RESEND.lock().unwrap_or_else(|e| e.into_inner()));
                if resend || sent.as_ref() != Some(&now) {
                    send(ToShell::Status {
                        label: now.0.clone(),
                        busy: now.1,
                        waits: now.2.clone(),
                        rail_hidden: now.3,
                        game: now.4.0,
                        map: now.4.1.clone(),
                        script: now.4.2.clone(),
                        streamer: now.5,
                        held: now.6.clone(),
                    });
                    sent = Some(now);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Couldn't start reporting the tab's name to the shell: {e}");
    }
}

static RESEND: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

/// Why the tab is held after kills for memory (Android's process tabs, `memory_kills`), for the
/// shell's tab list.
static HELD: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The tab is held after kills for memory (`None`: it isn't): the shell's tab list says why.
pub fn set_held(why: Option<String>) {
    *HELD.lock().unwrap_or_else(|e| e.into_inner()) = why;
}

/// A new shell is listening: the next status report goes even if nothing changed.
pub fn resend_status() {
    *RESEND.lock().unwrap_or_else(|e| e.into_inner()) = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_reaches_its_request_once() {
        let (got, seen) = std::sync::mpsc::channel();
        let mut sent = None;
        let id = ask(|id| sent = Some(id), move |answer| got.send(answer).unwrap());
        assert_eq!(sent, Some(id));
        let answer = Answered::Path {
            path: Some("/scripts/Questwalker.ts".into()),
        };
        assert!(answered(id, answer.clone()));
        assert_eq!(seen.recv().unwrap(), answer);
        assert!(!answered(id, Answered::Path { path: None }), "answered once");
        assert!(!answered(u32::MAX, Answered::Path { path: None }), "no such request");
    }
}
