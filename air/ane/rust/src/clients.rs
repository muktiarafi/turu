//! Turu Mas's tabs: several game clients on AIR's one stage (docs/AIR_ARMY_SPIKE.md), each a
//! tab with its own bot, as Turu's Android app has them (`android/src/lib.rs`, `tab_request`).
//!
//! The host SWF numbers its clients (0 to [`MAX`] - 1): each client's turu.swf talks through its own
//! `TuruExternal<n>`, so its calls reach its tab's bot ([`bot`]) and its tab's bot reaches it
//! ([`AirFlashHost`]'s `client`). Client 0 is the first tab, which the host loads at start; tab
//! requests (the tab strip's "+", Army > Accounts, Army Control) open, select and close the
//! others, through status events "tab" to the host (`{"op", "client"}`). Requests are handled in
//! order on a thread of their own, as they may wait for a script to stop.
//!
//! Unlike Ruffle, AIR has one frame rate for all its clients, so a hidden tab's game isn't
//! slowed (that would slow the one on screen too); the host hides it, so it isn't drawn.

use crate::host::{AirFlashHost, BRIDGE};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard};
use turu_bot::Bot;
use turu_frontend::turu::{self, TabId, TabRequest};

/// The clients the host SWF holds at most: the highest Tab Limit (Options > Application), as in
/// Turu's Android app. The host SWF has the classes for this many (`TuruExternal0` ...).
pub(crate) const MAX: usize = 4;

/// Each client number's tab, while it's open.
static CLIENTS: Mutex<[Option<TabId>; MAX]> = Mutex::new([None; MAX]);

fn clients() -> MutexGuard<'static, [Option<TabId>; MAX]> {
    CLIENTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Client `client`'s bot, while its tab is open.
pub(crate) fn bot(client: u8) -> Option<Arc<Bot>> {
    let tab = (*clients().get(client as usize)?)?;
    turu::tab_bot(tab)
}

fn client_of(tab: TabId) -> Option<u8> {
    clients()
        .iter()
        .position(|t| *t == Some(tab))
        .and_then(|i| u8::try_from(i).ok())
}

/// The first tab as client 0, tab requests from here on, and the tabs open before Android
/// stopped the app (`android-tabs.json`).
pub(crate) fn start() {
    let tab = turu::active_tab();
    clients()[0] = Some(tab);
    turu::bot().set_host(Some(Arc::new(AirFlashHost { client: 0 })));
    turu::set_hidden_tabs_slowed(false);
    let (send, receive) = channel::<TabRequest>();
    let send = Mutex::new(send);
    turu::set_tab_requests(Box::new(move |request| {
        send.lock().is_ok_and(|send| send.send(request).is_ok())
    }));
    let spawned = std::thread::Builder::new()
        .name("Turu Tabs".into())
        .spawn(move || {
            for request in receive {
                handle(request);
            }
        });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start the tab thread; no tabs: {e}");
        return;
    }
    turu_frontend::saved_tabs::reopen();
}

fn handle(request: TabRequest) {
    match request {
        TabRequest::Open(startup) => {
            let limit = turu::bot().settings.read().tab_limit.clamp(1, MAX as i32) as usize;
            let open = clients().iter().flatten().count();
            if open >= limit {
                let message = format!(
                    "Not opening a tab: {limit} are open, the Tab Limit (Options > Application). Each one takes about 200 MB."
                );
                tracing::warn!("{message}");
                turu::bot().log(message);
                return;
            }
            let Some(client) = clients().iter().position(Option::is_none) else {
                return;
            };
            let (tab, bot) = turu::add_tab(*startup);
            clients()[client] = Some(tab);
            let client = client as u8;
            // Before the host loads its turu.swf, whose first call asks the bot to load the game.
            bot.set_host(Some(Arc::new(AirFlashHost { client })));
            send_host("open", client);
            select(tab);
            tracing::info!("Opened a tab, client {client} ({} open)", open + 1);
        }
        TabRequest::Select(tab) => {
            if turu::active_tab() != tab {
                select(tab);
            }
        }
        TabRequest::Close(tab) => {
            if turu::tabs().len() < 2 {
                return;
            }
            if let Some(client) = client_of(tab) {
                send_host("close", client);
                clients()[client as usize] = None;
            }
            if let Some(bot) = turu::remove_tab(tab) {
                bot.set_host(None);
            }
            select(turu::active_tab());
        }
        // No Grid View on a phone: the tiles would be too small.
        TabRequest::Grid(_) => return,
    }
    turu_frontend::saved_tabs::save();
}

/// Makes the tab the active one, and its client the one on screen.
fn select(tab: TabId) {
    if !turu::set_active_tab(tab) {
        return;
    }
    if let Some(client) = client_of(tab) {
        send_host("select", client);
    }
}

/// A status event "tab" to the host SWF: `{"op": op, "client": client}`.
fn send_host(op: &str, client: u8) {
    let Some((vm, class)) = BRIDGE.get() else {
        tracing::warn!("No AIR yet: can't {op} client {client}");
        return;
    };
    let level = serde_json::json!({ "op": op, "client": client }).to_string();
    let sent = crate::android::attach(vm, |env: &mut jni::Env| {
        crate::host::dispatch(env, class, "tab", &level)
    });
    if let Err(e) = sent {
        tracing::warn!("Couldn't {op} client {client}: {e}");
    }
}
