//! Turu: connects the bot engine (`turu_bot`) to the player.
//!
//! * Tabs: every tab is a game client of its own, a player and its [`Bot`] (VibeSkua runs one
//!   process per tab instead). All the players run on the event loop thread; the window shows
//!   the active tab's, and [`bot()`] is the active tab's bot, which the UI shows.
//! * [`TuruExternalInterface`] receives a tab's `turu.swf`'s `ExternalInterface.call(...)`s.
//! * [`TuruFlashHost`] lets bot threads call `turu.swf`'s callbacks: each call is handed to the
//!   thread that runs the players (the window's event loop, the `headless` loop, Android's
//!   engine thread), names its tab, and is answered by [`answer_flash_call`] with that tab's
//!   player.
//!
//! Nothing here knows the window: the app hands in how to reach its loop
//! ([`set_tab_requests`], [`TuruFlashHost::new`]).
//!
//! See docs/ARCHITECTURE.md.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::Duration;
use turu_bot::Bot;
use turu_bot::events::BotEvent;
use turu_bot::shared_job::Ran;
use turu_bot::startup::Startup;

#[cfg(feature = "ruffle")]
mod ruffle_host;
#[cfg(feature = "ruffle")]
pub use ruffle_host::{
    TuruExternalInterface, TuruFlashCall, TuruFlashHost, TuruSocketObserver, answer_flash_call,
};

pub const APP_NAME: &str = "Turu";
/// The release workflow sets `TURU_VERSION` from the git tag; local builds show the crate version.
pub const VERSION: &str = match option_env!("TURU_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

static TABS: Mutex<Option<Tabs>> = Mutex::new(None);
/// Where tab requests go ([`request_tab`]); set when the window opens.
static TAB_REQUESTS: Mutex<Option<TabRequests>> = Mutex::new(None);
static ARMY: OnceLock<Arc<turu_bot::army::Army>> = OnceLock::new();
static MONITOR_HUB: OnceLock<Arc<turu_bot::monitor::Hub>> = OnceLock::new();
// Turu: Search Scripts' repository, and whether Army > Accounts launched this client.
static SCRIPT_REPO: OnceLock<Arc<turu_bot::repo::ScriptRepo>> = OnceLock::new();
static LAUNCHED_BY_ARMY: AtomicBool = AtomicBool::new(false);
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();
static HEADLESS: AtomicBool = AtomicBool::new(false);
/// This process is one of a shell's tabs: a desktop client or an Android slot (M3).
static SHELL_TAB: AtomicBool = AtomicBool::new(false);
/// `--client=<address>`: the shell this process is a client of (docs/TAB_PROCESSES_PROPOSAL.md).
static CLIENT_OF: Mutex<Option<String>> = Mutex::new(None);
/// `--shell`: this process is a shell over client processes.
static SHELL: AtomicBool = AtomicBool::new(false);
/// `--api`'s address, until `start_api`.
static API: Mutex<Option<String>> = Mutex::new(None);
/// The startup flags, until `configure_startup`.
static STARTUP: Mutex<Option<Startup>> = Mutex::new(None);
/// The web monitor (Options > Application > Web Monitor); [`apply_monitor`] starts and stops it.
static MONITOR: Mutex<Monitor> = Mutex::new(Monitor {
    server: None,
    running: None,
    error: None,
    key: None,
    push_key: None,
    reporter: None,
    reporting: None,
    report_error: None,
    generation: 0,
});
/// `--monitor[=address]` (empty without an address) and `--report-to`'s URL: they override
/// Options > Application > Monitor for this run.
static MONITOR_FLAG: Mutex<Option<String>> = Mutex::new(None);
static REPORT_FLAG: Mutex<Option<String>> = Mutex::new(None);

/// Ruffle's SharedObjects folder (`--save-directory`): what "Clear Flash Cache" empties.
static SAVE_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

/// A tab: one game client, with its own player and bot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(u64);

struct Tabs {
    list: Vec<(TabId, Arc<Bot>)>,
    active: TabId,
    next: u64,
}

fn tabs_lock() -> std::sync::MutexGuard<'static, Option<Tabs>> {
    let mut tabs = TABS.lock().unwrap_or_else(|e| e.into_inner());
    // The first tab: the one the window (or `--headless`) opens with.
    tabs.get_or_insert_with(|| {
        let bot = Bot::new();
        watch_alerts(&bot);
        Tabs {
            list: vec![(TabId(0), bot)],
            active: TabId(0),
            next: 1,
        }
    });
    tabs
}

/// The active tab's bot: what the UI shows and acts on. Threads that work for one tab keep
/// that tab's bot (`spawn_action` does) rather than calling this again.
pub fn bot() -> Arc<Bot> {
    let tabs = tabs_lock();
    let tabs = tabs.as_ref().expect("made by tabs_lock");
    tabs.list
        .iter()
        .find(|(id, _)| *id == tabs.active)
        .or(tabs.list.first())
        .map(|(_, bot)| bot.clone())
        .expect("there is always a tab")
}

/// The active tab.
pub fn active_tab() -> TabId {
    tabs_lock().as_ref().expect("made by tabs_lock").active
}

/// Every tab and its bot, in order.
pub fn tabs() -> Vec<(TabId, Arc<Bot>)> {
    tabs_lock()
        .as_ref()
        .expect("made by tabs_lock")
        .list
        .clone()
}

/// The tab's bot; `None` once it was closed.
pub fn tab_bot(tab: TabId) -> Option<Arc<Bot>> {
    tabs()
        .into_iter()
        .find(|(id, _)| *id == tab)
        .map(|(_, bot)| bot)
}

/// Adds a tab with a new bot (its player is the event loop's job: [`request_tab`]). With a
/// login, the bot logs in once its game has loaded, as with `--user`.
pub fn add_tab(startup: Startup) -> (TabId, Arc<Bot>) {
    let bot = Bot::new();
    watch_alerts(&bot);
    bot.scripts.set_interactive(!headless());
    turu_bot::startup::configure(&bot, startup);
    let mut tabs = tabs_lock();
    let tabs = tabs.as_mut().expect("made by tabs_lock");
    let id = TabId(tabs.next);
    tabs.next += 1;
    tabs.list.push((id, bot.clone()));
    (id, bot)
}

/// Where the platform posts alerts ([`crate::platform::Platform::posts_alerts`], Android's
/// notifications): a script that stops in `bot`'s tab alerts, 2 s later (as
/// [`exit_on_script_stop`] waits), unless it was the auto-relogin's stop, the script runs again, or
/// a playlist goes on to its next script. Notify Drop alerts its own drops (`notify_drop.rs`).
fn watch_alerts(bot: &Arc<Bot>) {
    let watched = Arc::downgrade(bot);
    // The first tab is made before the platform is installed: asked at each stop.
    bot.events.subscribe(move |event| {
        if !matches!(event, BotEvent::ScriptStopped)
            || !crate::platform::platform().posts_alerts()
        {
            return;
        }
        let watched = watched.clone();
        let _ = std::thread::Builder::new()
            .name("Turu Alert".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_secs(2));
                let Some(bot) = watched.upgrade() else { return };
                if bot.servers.is_relogging()
                    || bot.scripts.is_running()
                    || bot.scheduler.is_running()
                {
                    return;
                }
                let Some(end) = bot.scripts.last_end().1 else {
                    return;
                };
                let (title, text) = script_stopped_alert(&tab_label(&bot), &end);
                tracing::info!("Alert: {title}: {text}");
                let kind = crate::platform::Alert::ScriptStopped;
                crate::platform::platform().alert(kind, &title, &text);
            });
    });
}

/// "Script finished" / "Script failed" / "Script stopped", and "<tab>: <script>" (with the
/// error's first line).
fn script_stopped_alert(tab: &str, end: &turu_bot::script::RunEnd) -> (String, String) {
    let script = end
        .script
        .file_stem()
        .map_or_else(|| "The script".into(), |s| s.to_string_lossy().into_owned());
    let title = match (&end.error, end.finished) {
        (Some(_), _) => "Script failed",
        (None, true) => "Script finished",
        (None, false) => "Script stopped",
    };
    let mut text = format!("{tab}: {script}");
    if let Some(first) = end.error.as_deref().and_then(|e| e.lines().next()) {
        text.push_str(" (");
        text.push_str(first);
        text.push(')');
    }
    (title.to_owned(), text)
}

/// Removes the tab (never the last one): stops its script and Auto Attack/Hunt.
pub fn remove_tab(tab: TabId) -> Option<Arc<Bot>> {
    let removed = {
        let mut tabs = tabs_lock();
        let tabs = tabs.as_mut().expect("made by tabs_lock");
        if tabs.list.len() < 2 {
            return None;
        }
        let i = tabs.list.iter().position(|(id, _)| *id == tab)?;
        let (_, bot) = tabs.list.remove(i);
        if tabs.active == tab {
            tabs.active = tabs.list[i.min(tabs.list.len() - 1)].0;
        }
        bot
    };
    // Its playlist would start the next script once this one stops.
    removed.scheduler.stop(&removed);
    turu_bot::script::stop(&removed);
    removed.control.request_stop();
    // The other clients' Butler drops the account at once, not when its heartbeat ages out.
    turu_bot::butler::close(&removed);
    Some(removed)
}

/// Marks every tab's account offline for the other clients' Butler: the app quits.
pub fn close_butlers() {
    for (_, bot) in tabs() {
        turu_bot::butler::close(&bot);
    }
}

/// Whether hidden tabs' games are slowed ([`set_active_tab`]); see [`set_hidden_tabs_slowed`].
static HIDDEN_TABS_SLOWED: AtomicBool = AtomicBool::new(true);

/// Whether [`set_active_tab`] slows the other tabs' games. Turu Mas turns it off: its tabs share
/// AIR's one stage, whose frame rate is every client's, so slowing a hidden one would slow the
/// one on screen too.
pub fn set_hidden_tabs_slowed(slowed: bool) {
    HIDDEN_TABS_SLOWED.store(slowed, Ordering::Relaxed);
}

/// Makes the tab the active one; false when there's no such tab. The others' games slow down
/// once in game (VibeSkua's tabbed host throttles hidden tabs: `options::apply`), unless
/// [`set_hidden_tabs_slowed`] says not to.
pub fn set_active_tab(tab: TabId) -> bool {
    let mut tabs = tabs_lock();
    let tabs = tabs.as_mut().expect("made by tabs_lock");
    if !tabs.list.iter().any(|(id, _)| *id == tab) {
        return false;
    }
    tabs.active = tab;
    let slowed = HIDDEN_TABS_SLOWED.load(Ordering::Relaxed);
    for (id, bot) in &tabs.list {
        bot.state.lock().hidden_tab = slowed && *id != tab;
    }
    true
}

/// One of the shell's tabs, as a client's tab strip shows it (docs/TAB_PROCESSES_PROPOSAL.md, M5):
/// in a client every tab but its own is another process, so the strip shows the shell's list.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellTab {
    pub id: u32,
    pub label: String,
    /// Its script or Auto runs (the strip's dot).
    pub busy: bool,
    /// Its process isn't running: it's starting again, or gave up ("closed").
    pub down: Option<String>,
    /// Its client's control API and token: this process's monitor hub polls the other tabs there,
    /// and Army Control sends them commands.
    pub api: Option<std::net::SocketAddr>,
    pub token: String,
    /// It's this client's own tab.
    pub me: bool,
    /// The login it was opened with (`--user`), if any.
    pub user: Option<String>,
}

impl ShellTab {
    /// Its id in the monitor hub (`tab:<n>`, the shell's number).
    pub fn hub_id(&self) -> String {
        format!("tab:{}", self.id)
    }
}

/// What a client's tab strip asks of the shell.
pub enum ShellTabAction {
    Select(u32),
    Close(u32),
    /// Open one of the shell's native screens.
    Open(crate::shell::NativeScreen),
}

/// The shell's tabs and the one shown, while this process is a client.
static SHELL_TABS: Mutex<Option<(Vec<ShellTab>, u32)>> = Mutex::new(None);
/// Where [`shell_tab_action`] sends, in a client.
static SHELL_ACTIONS: Mutex<Option<ShellActions>> = Mutex::new(None);

/// Where a client's tab strip actions go.
pub type ShellActions = Box<dyn Fn(ShellTabAction) + Send>;

/// The shell's tabs, from its latest message (a client only).
pub fn set_shell_tabs(tabs: Vec<ShellTab>, shown: u32) {
    *SHELL_TABS.lock().unwrap_or_else(|e| e.into_inner()) = Some((tabs, shown));
    // The other tabs, polled from now on (their processes, through their control APIs).
    install_peers();
}

/// Where this process's other tabs are (`set_peer_source`); the shell's list by default.
type PeerSource = Box<dyn Fn() -> Vec<turu_bot::monitor::hub::PeerSource> + Send>;
static PEER_SOURCE: Mutex<Option<PeerSource>> = Mutex::new(None);

/// Where this tab finds the others when there's no shell to list them (Android's slots read each
/// other's address files).
pub fn set_peer_source(source: PeerSource) {
    *PEER_SOURCE.lock().unwrap_or_else(|e| e.into_inner()) = Some(source);
}

/// The other tabs' processes, for the monitor hub.
fn peers() -> Vec<turu_bot::monitor::hub::PeerSource> {
    match PEER_SOURCE.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        Some(source) => source(),
        None => shell_peers(),
    }
}

/// The monitor hub polls the other tabs: while Army Control is open, and all along in the tab
/// that runs the web monitor. Not more: the shell (or that tab) polls every client already, and
/// each poll reads the client's game (and its CPU "since the last poll").
fn install_peers() {
    static PEERS: std::sync::Once = std::sync::Once::new();
    PEERS.call_once(|| {
        monitor_hub().set_peers(Arc::new(|| {
            if army_control_open() || runs_monitor() {
                peers()
            } else {
                Vec::new()
            }
        }))
    });
}

/// When Army Control last drew.
static ARMY_CONTROL_DRAWN: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Army Control is open: called each time it draws.
pub fn army_control_drawn() {
    *ARMY_CONTROL_DRAWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(std::time::Instant::now());
    // This process's own tabs are listed from their snapshots too, as the other tabs read them.
    turu_bot::monitor::touch();
}

/// Army Control drew in the last few seconds.
fn army_control_open() -> bool {
    ARMY_CONTROL_DRAWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_some_and(|at| at.elapsed() < Duration::from_secs(5))
}

/// The shell's other tabs, in a client, and the clients the shell's Army launched (under their
/// Army ids, `army:<name>`): what its monitor hub polls.
pub fn shell_peers() -> Vec<turu_bot::monitor::hub::PeerSource> {
    let tabs = shell_tabs()
        .map(|(tabs, _)| tabs)
        .unwrap_or_default()
        .into_iter()
        .filter(|tab| !tab.me)
        .filter_map(|tab| {
            Some(turu_bot::monitor::hub::PeerSource {
                id: tab.hub_id(),
                label: tab.label.clone(),
                api: tab.api?,
                token: tab.token,
            })
        });
    let clients = shell_clients()
        .into_iter()
        .filter(|client| client.exited.is_none())
        .filter_map(|client| {
            Some(turu_bot::monitor::hub::PeerSource {
                id: turu_bot::monitor::hub::army_id(&client.username),
                label: client.label,
                api: client.api?,
                token: client.token,
            })
        });
    tabs.chain(clients).collect()
}

/// One of the clients the shell's Army launched in a process of its own, in a client (its
/// Army Control lists them; `shell::ArmyClientInfo`).
#[derive(Debug, Clone)]
pub struct ShellClient {
    pub username: String,
    pub label: String,
    pub headless: bool,
    pub api: Option<std::net::SocketAddr>,
    pub token: String,
    /// How its process ended, if it has.
    pub exited: Option<String>,
}

static SHELL_CLIENTS: Mutex<Vec<ShellClient>> = Mutex::new(Vec::new());

/// The shell's Army clients, from its latest message (a client only).
pub fn set_shell_clients(clients: Vec<ShellClient>) {
    *SHELL_CLIENTS.lock().unwrap_or_else(|e| e.into_inner()) = clients;
    install_peers();
}

/// The shell's Army clients, in a client; none elsewhere.
pub fn shell_clients() -> Vec<ShellClient> {
    SHELL_CLIENTS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// What a client asks of the shell's Army.
pub enum ShellArmyRequest {
    /// Launch these accounts in processes of their own (the shell's children: they outlive this
    /// client).
    Launch(Vec<turu_bot::army::Launch>),
    Close(String),
    Restart(String),
}

/// Sends a client's Army requests to its shell.
pub type ShellArmy = Box<dyn Fn(ShellArmyRequest) + Send + Sync>;

static SHELL_ARMY: OnceLock<ShellArmy> = OnceLock::new();

/// A client's processes of their own are launched by its shell (`app/src/client.rs`).
pub fn set_shell_army(army: ShellArmy) {
    let _ = SHELL_ARMY.set(army);
}

/// Asks the shell's Army; false when there's no shell (the request isn't sent).
pub fn shell_army(request: ShellArmyRequest) -> bool {
    match SHELL_ARMY.get() {
        Some(send) => {
            send(request);
            true
        }
        None => false,
    }
}

/// Whether Army > Accounts' processes of their own are launched by a shell.
pub fn launches_through_shell() -> bool {
    SHELL_ARMY.get().is_some()
}

/// The shell's tabs and the one shown, in a client; `None` elsewhere.
pub fn shell_tabs() -> Option<(Vec<ShellTab>, u32)> {
    SHELL_TABS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Where a client's tab strip actions go (`app/src/client.rs`).
pub fn set_shell_actions(send: ShellActions) {
    *SHELL_ACTIONS.lock().unwrap_or_else(|e| e.into_inner()) = Some(send);
}

/// Asks the shell to show or close one of its tabs.
pub fn shell_tab_action(action: ShellTabAction) {
    if let Some(send) = SHELL_ACTIONS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        send(action);
    }
}

/// Where the client's rail starts when the shell draws the tabs itself (`ToClient::Tabs`'
/// `rail_top`, in points, as `f32` bits; `u32::MAX`: the client draws its tab strip).
static SHELL_RAIL_TOP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);

/// The shell's `rail_top`, from its latest tab list (a client only).
pub fn set_shell_rail_top(top: Option<f32>) {
    let bits = top.map_or(u32::MAX, |top| top.max(0.0).to_bits());
    SHELL_RAIL_TOP.store(bits, std::sync::atomic::Ordering::Relaxed);
}

/// Where the rail starts, in points, when the shell draws the tabs (Android's native Tabs item);
/// `None` while the client draws its own tab strip.
pub fn shell_rail_top() -> Option<f32> {
    match SHELL_RAIL_TOP.load(std::sync::atomic::Ordering::Relaxed) {
        u32::MAX => None,
        bits => Some(f32::from_bits(bits)),
    }
}

/// Whether the shell draws the whole rail (`ToClient::Tabs`' `native_rail`, Android).
static SHELL_NATIVE_RAIL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_shell_native_rail(native: bool) {
    SHELL_NATIVE_RAIL.store(native, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the shell draws the rail: this client draws none (but keeps its room).
pub fn shell_native_rail() -> bool {
    SHELL_NATIVE_RAIL.load(std::sync::atomic::Ordering::Relaxed)
}

/// Whether this process draws a tab strip: two tabs or more, and no shell drawing them natively.
pub fn tab_strip_shown() -> bool {
    tab_strip_count() > 1 && shell_rail_top().is_none()
}

/// Asks the shell to show its native `screen` in place of the egui window, where it has one
/// (Android's); false when the egui window is the one to show.
/// Turu: this client is the tab the shell shows (a client is one tab's process).
static TAB_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The shell shows (or hides) this client's tab (`client_screen`'s Show and Hide).
pub fn set_tab_shown(shown: bool) {
    TAB_SHOWN.store(shown, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the shell shows this client's tab: a window handed to the shell's native screens
/// (which show the tab on screen) waits in this tab's picture until then.
pub fn tab_shown() -> bool {
    TAB_SHOWN.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn open_shell_screen(screen: crate::shell::NativeScreen) -> bool {
    if shell_rail_top().is_none() {
        return false;
    }
    if let Some(send) = SHELL_ACTIONS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        send(ShellTabAction::Open(screen));
        return true;
    }
    false
}

/// The egui windows the shell's native screens asked to open (`ToClient::OpenWindow`).
static WINDOW_REQUESTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Opens the egui window named `window` ("script_repo", "script_options") at the next frame.
pub fn request_window(window: String) {
    WINDOW_REQUESTS.lock().unwrap_or_else(|e| e.into_inner()).push(window);
}

/// The windows asked for since the last call.
pub fn take_window_requests() -> Vec<String> {
    std::mem::take(&mut *WINDOW_REQUESTS.lock().unwrap_or_else(|e| e.into_inner()))
}

/// How many tabs the tab strip shows: the shell's in a client, else this window's.
pub fn tab_strip_count() -> usize {
    shell_tabs().map_or_else(|| tabs().len(), |(tabs, _)| tabs.len())
}

/// What the UI asks of the event loop, which owns the players.
pub enum TabRequest {
    /// Open a new tab (at the login screen, or logging in with `Startup`'s login).
    Open(Box<Startup>),
    Select(TabId),
    Close(TabId),
    /// Turn Grid View on or off (`crate::grid`).
    Grid(bool),
}

/// Hands a tab request to the loop that owns the players; false when it's gone.
pub type TabRequests = Box<dyn Fn(TabRequest) -> bool + Send>;

/// Where [`request_tab`] sends; the window sets it when it opens.
pub fn set_tab_requests(send: TabRequests) {
    *TAB_REQUESTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(send);
}

/// Hands the request to the event loop; false without a window (`--headless` has one tab).
pub fn request_tab(request: TabRequest) -> bool {
    TAB_REQUESTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|send| send(request))
}

/// A tab's name: the character once in game, else the login name it was opened with; "Tab 2"
/// while its Streamer Mode is on (in a client, which runs one tab, its place in the shell's tabs).
pub fn tab_label(bot: &Bot) -> String {
    if bot.opts().streamer_mode {
        let tab = shell_tabs()
            .and_then(|(tabs, _)| tabs.iter().position(|t| t.me))
            .or_else(|| {
                tabs()
                    .iter()
                    .position(|(_, b)| std::ptr::eq(Arc::as_ptr(b), bot))
            });
        return tab.map_or_else(|| "Tab".into(), |i| format!("Tab {}", i + 1));
    }
    let state = bot.state.lock();
    if let Some(name) = state
        .username
        .as_deref()
        .filter(|n| state.in_game && !n.trim().is_empty())
    {
        return name.to_owned();
    }
    drop(state);
    turu_bot::api::servers::login_name(bot).unwrap_or_else(|| "Login".into())
}

/// Search Scripts' repository and scripts folder, shared by the tabs (they share the data
/// folder).
pub fn script_repo() -> &'static Arc<turu_bot::repo::ScriptRepo> {
    SCRIPT_REPO.get_or_init(|| {
        turu_bot::repo::ScriptRepo::new(
            bot().paths.clone(),
            Box::new(turu_bot::repo::GitHub::from_env()),
        )
    })
}

/// At launch, Options > Application's "Auto Updates & Sync" switches: script updates (VibeSkua's
/// Manager does them) with a window, skill sets always ([`start_skill_set_updates`]). Without a
/// window (`--headless`) scripts aren't updated at launch: the script it runs is downloaded or
/// updated before the game starts. Clients launched by Army > Accounts skip both: their
/// launcher has the same data folder.
pub fn start_updates() {
    start_headless_mode_sync();
    start_app_updates();
}

/// The launch updates alone (skill sets, scripts), for a process without a game of its own too
/// (the desktop's shell, whose clients leave them to it).
pub fn start_app_updates() {
    start_skill_set_updates();
    if headless() || LAUNCHED_BY_ARMY.load(Ordering::Relaxed) {
        return;
    }
    let settings = bot().settings.read().clone();
    if !settings.check_bot_scripts_updates {
        return;
    }
    script_repo().spawn("Script update", move |repo| {
        let mut done = Ok(String::new());
        let message = match shared_update("script-update", || done = repo.startup(&settings)) {
            Ran::Here => done?,
            Ran::Busy => "Another tab is updating the scripts".to_owned(),
            Ran::Recent => format!(
                "Another tab updated the scripts in the last {} minutes",
                SHARED_UPDATE_EVERY.as_secs() / 60
            ),
        };
        tracing::info!("Scripts: {message}");
        Ok(message)
    });
}

/// How long a launch update done in one tab's process counts for the others (a shell's clients,
/// Android's slots, which start together and again after a kill).
const SHARED_UPDATE_EVERY: Duration = Duration::from_secs(10 * 60);

/// Runs a launch update once across the processes on this data folder
/// (`turu_bot::shared_job`): not while another runs it, and in a shell's tab not if one did it
/// lately. A lock that can't be taken (the folder can't be written) doesn't stop the update.
fn shared_update(name: &str, job: impl FnOnce()) -> Ran {
    let every = if shell_tab() {
        SHARED_UPDATE_EVERY
    } else {
        Duration::ZERO
    };
    let dir = turu_bot::shared_job::dir(&bot().paths.root);
    let mut job = Some(job);
    match turu_bot::shared_job::run_once(&dir, name, every, || {
        if let Some(job) = job.take() {
            job();
        }
    }) {
        Ok(ran) => ran,
        Err(e) => {
            tracing::warn!("{name}: no lock across Turu's processes ({e}); updating anyway");
            if let Some(job) = job.take() {
                job();
            }
            Ran::Here
        }
    }
}

/// How often [`start_headless_mode_sync`] looks at the tabs' options (the options pass's rate).
const HEADLESS_MODE_SYNC: Duration = Duration::from_millis(250);

/// Keeps Headless Mode (the game option) the same on every tab, as VibeSkua's tabbed host sends it
/// to every tab (`BroadcastOption`). A tab that turns it on or off (Game Options, a HotKey, a
/// script, Army Control, the control API) turns it on or off on all; a tab that opens gets it as
/// the others have it. On Turu Mas the tabs share AIR's one stage, whose 2 FPS and LOW quality are
/// every tab's anyway. Once, on a thread of its own.
pub fn start_headless_mode_sync() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let spawned = std::thread::Builder::new()
            .name("Turu Headless Mode".into())
            .spawn(|| {
                let mut seen = HashMap::new();
                let mut on = bot().opts().headless_mode;
                loop {
                    on = sync_headless_mode(&tabs(), &mut seen, on);
                    std::thread::sleep(HEADLESS_MODE_SYNC);
                }
            });
        if let Err(e) = spawned {
            tracing::error!("Couldn't start Headless Mode's thread; it stays each tab's: {e}");
        }
    });
}

/// One pass of [`start_headless_mode_sync`]: `seen` is each tab's Headless Mode as last passed,
/// `on` every tab's; returns every tab's now.
fn sync_headless_mode(
    tabs: &[(TabId, Arc<Bot>)],
    seen: &mut HashMap<TabId, bool>,
    mut on: bool,
) -> bool {
    let changed = tabs.iter().find_map(|(tab, bot)| {
        let value = bot.opts().headless_mode;
        seen.get(tab)
            .is_some_and(|old| *old != value)
            .then_some(value)
    });
    if let Some(value) = changed {
        on = value;
    }
    for (tab, bot) in tabs {
        if bot.opts().headless_mode != on {
            bot.options.write().headless_mode = on;
        }
        seen.insert(*tab, on);
    }
    seen.retain(|tab, _| tabs.iter().any(|(t, _)| t == tab));
    on
}

/// Client updates (`turu_bot::updates`): checks Turu's GitHub releases at launch and every 6
/// hours, on a thread of its own, while Options > Application > Check for Client Updates is on.
/// Started once, by the window (`TuruUi::new`), `--headless` or Android's engine; clients
/// launched by Army > Accounts skip it (their launcher checks), and so does a nightly build
/// (`turu_bot::updates::is_nightly`: testers download the next one by hand).
pub fn start_client_updates() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    if leaves_app_jobs() {
        return;
    }
    if turu_bot::updates::this_is_nightly() {
        tracing::info!("Turu {VERSION} is a nightly build: no update checks");
        return;
    }
    STARTED.call_once(|| {
        turu_bot::updates::set_installer(Box::new(|| install_update(false)));
        let spawned = std::thread::Builder::new()
            .name("Turu Client Updates".into())
            .spawn(|| {
                loop {
                    if bot().settings.read().check_client_updates {
                        check_client_updates_now_blocking();
                    }
                    std::thread::sleep(turu_bot::updates::CHECK_EVERY);
                }
            });
        if let Err(e) = spawned {
            tracing::error!("Couldn't start the client update checks: {e}");
        }
    });
}

/// What a client asks of the shell that checks for updates and installs them for it (the
/// desktop's process tabs, `app/src/client.rs`).
pub enum UpdateRequest {
    Check,
    Install { on_quit: bool },
}

/// Sends a client's update requests to its shell.
pub type UpdateRequests = Box<dyn Fn(UpdateRequest) + Send + Sync>;

static UPDATE_REQUESTS: OnceLock<UpdateRequests> = OnceLock::new();

/// A client's updates are its shell's: Check Now and Download & restart go there, and the
/// state the UI shows comes from it (`shell::UpdateInfo`).
pub fn set_update_requests(requests: UpdateRequests) {
    let _ = UPDATE_REQUESTS.set(requests);
}

/// Whether another process (the shell) checks for and installs this one's updates, and takes
/// its requests.
pub fn updates_from_shell() -> bool {
    UPDATE_REQUESTS.get().is_some()
}

/// Options > Application's Check Now: one check, on a thread (by the shell, in its client).
pub fn check_client_updates_now() {
    if turu_bot::updates::this_is_nightly() {
        return;
    }
    if let Some(requests) = UPDATE_REQUESTS.get() {
        requests(UpdateRequest::Check);
        return;
    }
    let _ = std::thread::Builder::new()
        .name("Turu Client Update Check".into())
        .spawn(check_client_updates_now_blocking);
}

fn check_client_updates_now_blocking() {
    use turu_bot::updates;
    let started = updates::update_state(|s| !std::mem::replace(&mut s.checking, true));
    if !started {
        return;
    }
    let prereleases =
        updates::wants_prereleases(bot().settings.read().check_client_prereleases, VERSION);
    let result = updates::check(
        &updates::Http::for_checks(),
        &updates::releases_url(),
        VERSION,
        prereleases,
    );
    if let Err(e) = &result {
        tracing::warn!("Couldn't check for Turu updates: {e}");
    }
    if let Some(release) = updates::record_check(result) {
        let message = format!(
            "Update available: {} {} (you have {VERSION}): {}",
            updates::app_name(),
            release.version,
            release.html_url
        );
        tracing::info!("{message}");
        bot().log(format!("[Updates] {message}"));
    }
}

/// Whether Army > Accounts launched this client (it leaves updates and the monitor to its
/// launcher).
pub fn launched_by_army() -> bool {
    LAUNCHED_BY_ARMY.load(Ordering::Relaxed)
}

/// Whether a tab runs a script: Download & restart waits (Update when I quit doesn't).
pub fn any_script_running() -> bool {
    tabs()
        .iter()
        .any(|(_, bot)| bot.scripts.status().is_active())
}

/// An update unpacked by Update when I quit, put in place by [`apply_pending_update`].
#[cfg(not(target_os = "android"))]
static PENDING_UPDATE: Mutex<Option<turu_bot::updates::install::Staged>> = Mutex::new(None);

/// Download & restart (`on_quit` false) or Update when I quit (true), on a thread: downloads
/// the available release's package, checks it against `SHA256SUMS.txt` and unpacks it. Then
/// either puts it in place and restarts with the same arguments (`--headless` exits with
/// status 75 instead, for its service manager to restart it), or keeps it for
/// [`apply_pending_update`]. `turu_bot::updates::state().install` shows the progress.
pub fn install_update(on_quit: bool) -> Result<String, String> {
    if turu_bot::updates::this_is_nightly() {
        return Err(turu_bot::updates::NIGHTLY_NO_UPDATES.into());
    }
    // A shell's client: the shell installs it and starts every tab again.
    if let Some(requests) = UPDATE_REQUESTS.get() {
        requests(UpdateRequest::Install { on_quit });
        return Ok("the window installs it".into());
    }
    #[cfg(target_os = "android")]
    {
        let _ = on_quit;
        install_package()
    }
    #[cfg(not(target_os = "android"))]
    {
        use turu_bot::updates::{self, Install};
        let release = updates::available().ok_or("no update available")?;
        let target = updates::Target::current()
            .filter(|t| t.updates_in_place())
            .ok_or("Turu can't update itself on this platform")?;
        if !release.installable() {
            return Err(format!(
                "Turu {} has no package for this platform",
                release.version
            ));
        }
        let busy = updates::update_state(|s| {
            let busy = matches!(s.install, Install::Busy(_));
            if !busy {
                s.install = Install::Busy(format!("Downloading Turu {}…", release.version));
            }
            busy
        });
        if busy {
            return Err("an update is being installed already".into());
        }
        let version = release.version.clone();
        let spawned = std::thread::Builder::new()
            .name("Turu Client Update".into())
            .spawn(move || {
                let set = |install: Install| updates::update_state(|s| s.install = install);
                let dir = bot().paths.updates_dir();
                let staged = updates::download(&updates::Http::for_downloads(), &release, &dir)
                    .and_then(|archive| {
                        set(Install::Busy(format!(
                            "Unpacking Turu {}…",
                            release.version
                        )));
                        updates::install::stage(&archive, &dir.join("staged"), target)
                    });
                let staged = match staged {
                    Ok(staged) => staged,
                    Err(e) => {
                        tracing::error!("Couldn't update Turu: {e}");
                        set(Install::Failed(e));
                        return;
                    }
                };
                if on_quit {
                    tracing::info!("Turu {} will be installed when Turu quits", release.version);
                    *PENDING_UPDATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(staged);
                    set(Install::OnQuit(release.version));
                    return;
                }
                set(Install::Busy(format!(
                    "Installing Turu {}…",
                    release.version
                )));
                match updates::install::apply(&staged) {
                    Ok(relaunch) => {
                        tracing::info!("Installed Turu {}; restarting", release.version);
                        close_butlers();
                        if headless() {
                            // `Restart=on-failure` restarts it (docs/HEADLESS.md).
                            std::process::exit(75);
                        }
                        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
                        match relaunch.spawn(&args) {
                            Ok(()) => std::process::exit(0),
                            Err(e) => {
                                tracing::error!("{e}");
                                set(Install::Failed(format!(
                                    "Installed, but {e}: start Turu again"
                                )));
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Couldn't install Turu {}: {e}", release.version);
                        set(Install::Failed(e));
                    }
                }
            });
        if let Err(e) = spawned {
            updates::update_state(|s| s.install = Install::Failed(e.to_string()));
            return Err(e.to_string());
        }
        Ok(format!("installing Turu {version}"))
    }
}

/// Android's Download & install, on a thread: downloads the available release's APK, checks it
/// against `SHA256SUMS.txt` and hands it to the system's installer
/// ([`crate::platform::Platform::install_package`]), which asks the user, installs it over this
/// app and stops it; [`package_install_finished`] hears when it didn't.
#[cfg(target_os = "android")]
fn install_package() -> Result<String, String> {
    use turu_bot::updates::{self, Install};
    let platform = crate::platform::platform();
    if !platform.installs_packages() {
        return Err("this app can't install its updates".into());
    }
    let release = updates::available().ok_or("no update available")?;
    let name = updates::app_name();
    if !release.installable() {
        return Err(format!("{name} {} has no APK", release.version));
    }
    let busy = updates::update_state(|s| {
        let busy = matches!(s.install, Install::Busy(_));
        if !busy {
            s.install = Install::Busy(format!("Downloading {name} {}…", release.version));
        }
        busy
    });
    if busy {
        return Err("an update is being installed already".into());
    }
    let version = release.version.clone();
    let spawned = std::thread::Builder::new()
        .name("Turu Client Update".into())
        .spawn(move || {
            let set = |install: Install| updates::update_state(|s| s.install = install);
            let dir = bot().paths.updates_dir();
            let installed = updates::download(&updates::Http::for_downloads(), &release, &dir)
                .and_then(|apk| {
                    set(Install::Busy(format!("Installing {name} {}…", release.version)));
                    crate::platform::platform().install_package(&apk)
                });
            if let Err(e) = installed {
                tracing::error!("Couldn't update {name}: {e}");
                set(Install::Failed(e));
            }
        });
    if let Err(e) = spawned {
        updates::update_state(|s| s.install = Install::Failed(e.to_string()));
        return Err(e.to_string());
    }
    Ok(format!("installing {name} {version}"))
}

/// How Android's installer ended [`install_update`]'s install, as both apps' `PackageUpdate.kt`
/// reports it: `status` 0 installed (Android usually stops the app before), 1 cancelled by the
/// user, 2 failed (with Android's `message`). Called on any thread.
pub fn package_install_finished(status: i32, message: Option<String>) {
    use turu_bot::updates::{self, Install};
    let install = match status {
        0 => {
            tracing::info!("The update is installed");
            Install::Busy("Installed: open the app again…".into())
        }
        1 => {
            tracing::info!("The update's install was cancelled");
            Install::Idle
        }
        _ => {
            let why = message.unwrap_or_else(|| "Android's installer failed".into());
            tracing::error!("Android couldn't install the update: {why}");
            Install::Failed(why)
        }
    };
    updates::update_state(|s| s.install = install);
}

/// Puts an update kept by Update when I quit in place; the app calls it once its window closed.
pub fn apply_pending_update() {
    #[cfg(not(target_os = "android"))]
    if let Some(staged) = PENDING_UPDATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        match turu_bot::updates::install::apply(&staged) {
            Ok(_) => tracing::info!("Installed the update: it starts next time"),
            Err(e) => tracing::error!("Couldn't install the update: {e}"),
        }
    }
}

/// VibeSkua `AdvancedSkillContainer`'s launch refresh, on its own thread: with Check for
/// AdvanceSkill Sets Updates and Auto Update AdvanceSkill Sets on, adds the community's new skill
/// sets to the user's (`turu_bot::skills::startup_skill_sets`).
fn start_skill_set_updates() {
    if LAUNCHED_BY_ARMY.load(Ordering::Relaxed) {
        return;
    }
    let settings = bot().settings.read().clone();
    if !settings.check_advance_skill_sets_updates
        || !settings.auto_update_advance_skill_sets_updates
    {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("turu-skill-sets".into())
        .spawn(move || {
            let paths = &bot().paths;
            let mut done = Ok(None);
            let ran = shared_update("skill-sets", || {
                done = turu_bot::skills::startup_skill_sets(paths, &settings, turu_bot::skills::download_skill_sets);
            });
            if ran != Ran::Here {
                tracing::info!("Skill sets: another tab updates them");
                return;
            }
            match done {
                Ok(Some(0)) => tracing::info!("Skill sets: up to date"),
                Ok(Some(added)) => {
                    tracing::info!("Skill sets: added {added} from the community sets")
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("Skill sets: couldn't update them, keeping the local ones: {e}")
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Skill sets: couldn't start the update: {e}");
    }
}

/// The accounts this client launched (Army > Accounts), each in its own `turu` process.
pub fn army() -> &'static Arc<turu_bot::army::Army> {
    ARMY.get_or_init(|| {
        let exe =
            std::env::current_exe().unwrap_or_else(|_| PathBuf::from(APP_NAME.to_lowercase()));
        let army = turu_bot::army::Army::new(exe);
        army.set_hub(monitor_hub().clone());
        army
    })
}

/// This window's tabs as the web monitor and the control API name them (`tab:<n>`, "Tab 2").
fn tab_sources() -> turu_bot::monitor::hub::Sources {
    Arc::new(|| {
        tabs()
            .into_iter()
            .enumerate()
            .map(|(i, (id, bot))| turu_bot::monitor::hub::Source {
                id: format!("tab:{}", id.0),
                label: format!("Tab {}", i + 1),
                bot,
            })
            .collect()
    })
}

/// The web monitor's hub: this window's tabs and the Army clients, kept even while the monitor
/// is off (Army Control reads the clients from it).
pub fn monitor_hub() -> &'static Arc<turu_bot::monitor::Hub> {
    MONITOR_HUB.get_or_init(|| {
        let hub = turu_bot::monitor::Hub::new(Some(bot().paths.monitor_file()));
        // Only the process that runs the web monitor saves `monitor.json`.
        hub.set_saving(runs_monitor());
        // A shell has no game of its own: its tabs are its clients' processes (`set_peers`).
        if !shell() {
            hub.set_tabs(tab_sources());
        }
        hub.start();
        hub
    })
}

pub fn set_save_directory(dir: PathBuf) {
    let _ = SAVE_DIRECTORY.set(dir);
}

pub fn save_directory() -> Option<&'static Path> {
    SAVE_DIRECTORY.get().map(PathBuf::as_path)
}

/// The window's `title` (Mugetsu's), plus the character's name while you play when Options >
/// Application > "Show Username in Title" is on (VibeSkua names its tab after the character) and
/// Streamer Mode is off.
pub fn window_title(title: String) -> String {
    let bot = bot();
    if !bot.settings.read().show_username_in_title || bot.opts().streamer_mode {
        return title;
    }
    let state = bot.state.lock();
    match &state.username {
        Some(name) if state.in_game && !name.trim().is_empty() => format!("{title} – {name}"),
        _ => title,
    }
}

/// Remembers the event loop thread: calls from it would wait for themselves.
pub fn init_main_thread() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
}

/// The URL Turu starts with: its host SWF, `turu.swf`, written to the data directory.
pub fn host_swf_url() -> anyhow::Result<String> {
    let path = bot().paths.write_host_swf()?;
    let url = url::Url::from_file_path(&path)
        .map_err(|()| anyhow::anyhow!("bad path for turu.swf: {}", path.display()))?;
    Ok(url.to_string())
}

/// Takes Turu's own flags (VibeSkua's, long forms only: Ruffle has `-p` and `-s`) out of the
/// command line and sets the bot up with them:
///
/// * `--user <name>` and `--password <password>` (or the `TURU_PASSWORD` environment variable,
///   which other users can't read the way they can a command line): log in once the game has
///   loaded.
/// * `--server <name>`: the server to log in on (default Twilly).
/// * `--run-script <path>`: load the script and start it once logged in; `--load-script <path>`
///   only loads it. Absolute, or relative to the scripts folder (else to the working folder),
///   `.ts` optional; one Search Scripts lists is downloaded first (see [`configure_startup`]).
/// * `--run-playlist <file>` / `--load-playlist <file>`: the same for a Scheduler playlist
///   (VibeSkua's playlist JSON), started in place of the script.
/// * `--script-options-storage <name>`: the script's Script Options come from
///   `options/<name>.cfg`, and `--run-script` starts it without asking for them (Army Queue).
/// * `--exit-on-script-stop` (or `--exit-on-finish`): log out and quit when the script stops
///   (not when auto-relogin stops it), or with a playlist, when the playlist ends.
/// * `--headless`: no window (see `headless.rs`).
/// * `--client=<address>` / `--shell`: a tab in a process of its own, drawn for a shell / the
///   shell over such clients (docs/TAB_PROCESSES_PROPOSAL.md, M2; `app/src/client.rs`, `shell.rs`).
/// * `--in-process`: ignored (each tab is a process of its own since M6; it ran the window's tabs
///   in its own process).
/// * `--exit-with-parent`: quit when stdin closes (Army > Accounts launches clients with it,
///   their stdin a pipe from the launcher: see `turu_bot::army`).
/// * `--api` (or `--api=<address>`, default `127.0.0.1:9091`): the control API (see
///   `turu_bot::control_api`), started by [`start_api`].
pub fn take_startup_args(args: &mut Vec<OsString>) {
    let Flags {
        mut startup,
        exit_on_stop,
        exit_with_parent,
        headless,
        client,
        shell,
        api,
        monitor,
        report_to,
    } = parse_startup_args(args);
    if exit_with_parent {
        LAUNCHED_BY_ARMY.store(true, Ordering::Relaxed);
        turu_bot::army::exit_with_parent(|| {
            close_butlers();
            std::process::exit(0)
        });
    }
    HEADLESS.store(headless, Ordering::Relaxed);
    SHELL.store(shell, Ordering::Relaxed);
    *CLIENT_OF.lock().unwrap_or_else(|e| e.into_inner()) = client;
    if let Ok(mut slot) = API.lock() {
        *slot = api;
    }
    if let Some(addr) = monitor.as_deref().filter(|a| !a.is_empty())
        && addr.parse::<std::net::SocketAddr>().is_err()
    {
        tracing::warn!(
            "--monitor={addr}: expected an address like 0.0.0.0:9021; using the settings' port"
        );
    }
    *MONITOR_FLAG.lock().unwrap_or_else(|e| e.into_inner()) = monitor;
    *REPORT_FLAG.lock().unwrap_or_else(|e| e.into_inner()) = report_to;
    if startup.password.is_none() {
        startup.password = std::env::var("TURU_PASSWORD")
            .ok()
            .filter(|p| !p.is_empty());
    }
    if exit_on_stop {
        exit_on_script_stop(startup.playlist.is_some());
    }
    *STARTUP.lock().unwrap_or_else(|e| e.into_inner()) = Some(startup);
}

/// Hands the startup flags to the bot, once logging is up. A script or a playlist entry given
/// relative to the scripts folder that Search Scripts lists is downloaded (or updated) first,
/// with what it imports, so a new server needs no scripts folder. This blocks: the game isn't
/// loading yet, so the login can't start before the scripts are there.
pub fn configure_startup() {
    let Some(startup) = STARTUP.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        return;
    };
    if startup.username.is_some() && !startup.logs_in() {
        tracing::warn!("--user without --password (or TURU_PASSWORD): log in yourself");
    }
    if startup.script.is_some() && startup.playlist.is_some() {
        tracing::warn!("Both a script and a playlist were given: the playlist runs");
    }
    let bot = bot();
    let scripts = bot.paths.scripts_dir();
    let mut wanted: Vec<String> = startup
        .script
        .iter()
        .filter_map(|p| repo_path(&scripts, &p.to_string_lossy()))
        .collect();
    if let Some(playlist) = &startup.playlist {
        match turu_bot::scheduler::saved_paths(playlist) {
            Ok(paths) => wanted.extend(paths.iter().filter_map(|p| repo_path(&scripts, p))),
            Err(e) => tracing::error!("--run-playlist {}: {e}", playlist.display()),
        }
    }
    wanted.sort_unstable();
    wanted.dedup();
    if !wanted.is_empty() {
        let repo = script_repo();
        if let Err(e) = repo.refresh() {
            tracing::warn!("Couldn't get the script list ({e}); using the scripts folder as it is");
        }
        for path in &wanted {
            match repo.ensure(path) {
                Ok(local) => tracing::info!("Script {path}: {}", local.display()),
                Err(e) => tracing::warn!("Script {path}: {e}"),
            }
        }
    }
    // As `add_tab` does: the window shows the Script Options (and `startup::configure` warns
    // when nothing will).
    bot.scripts.set_interactive(!headless());
    turu_bot::startup::configure(&bot, startup);
}

/// The repository path of a script given relative to the scripts folder (`farm/GoldFarm`,
/// `.ts` optional); `None` for an absolute path or a file in the working folder.
fn repo_path(scripts: &Path, given: &str) -> Option<String> {
    let given = Path::new(given);
    if given.is_absolute() || given.as_os_str().is_empty() {
        return None;
    }
    let with_ts = match given.extension() {
        Some(_) => given.to_owned(),
        None => given.with_extension("ts"),
    };
    if !scripts.join(&with_ts).is_file() && (given.is_file() || with_ts.is_file()) {
        return None;
    }
    let path = turu_bot::script::normalize(&with_ts)
        .to_string_lossy()
        .replace('\\', "/");
    Some(path.trim_start_matches('/').to_owned())
}

/// Starts the control API if `--api` was given (once logging is up, so its messages show). The
/// token is `TURU_API_TOKEN`, else the one in the data folder's `api-token` (made the first time).
/// A process tabs' shell starts its own instead ([`start_shell_api`]).
pub fn start_api() {
    // The view models a native shell's screens read (`crate::models`).
    crate::models::register();
    if let Some((addr, token)) = api_settings() {
        let started = token.and_then(|token| turu_bot::control_api::start_tabs(tab_sources(), addr, token));
        report_api(started);
    }
}

/// A process tabs' shell's control API, if `--api` was given: the same routes over its tabs
/// (`peers`, each a client process with a control API of its own), passed on to each tab's
/// (`control_api::start_forwarding`), with the token [`start_api`] would use.
pub fn start_shell_api(peers: turu_bot::monitor::hub::Peers) {
    if let Some((addr, token)) = api_settings() {
        let started = token.and_then(|token| turu_bot::control_api::start_forwarding(peers, addr, token));
        report_api(started);
    }
}

/// `--api`'s address and the token, taken once.
fn api_settings() -> Option<(std::net::SocketAddr, Result<String, String>)> {
    let addr = API.lock().ok().and_then(|mut a| a.take())?;
    let addr: std::net::SocketAddr = match addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            tracing::error!("--api={addr}: {e} (expected an address like 127.0.0.1:9091)");
            return None;
        }
    };
    let token = match std::env::var("TURU_API_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty())
    {
        Some(token) => {
            tracing::info!("Control API token: from TURU_API_TOKEN");
            Ok(token.trim().to_owned())
        }
        None => {
            let file = bot().paths.api_token_file();
            let token = turu_bot::control_api::saved_token(&file);
            tracing::info!("Control API token: in {}", file.display());
            token
        }
    };
    Some((addr, token))
}

/// Logs where the control API listens, or why it doesn't.
fn report_api(started: Result<std::net::SocketAddr, String>) {
    match started {
        Ok(local) => {
            tracing::info!("Control API on http://{local}");
            if !local.ip().is_loopback() {
                tracing::warn!(
                    "The control API is reachable from other computers: keep the token secret"
                );
            }
        }
        Err(e) => tracing::error!("Control API: {e}"),
    }
}

/// Reports on the thread that runs the players as a whole, given the milliseconds the games took
/// over the last period ([`start_perf_log`]).
pub type ThreadReport = Box<dyn FnMut(f64) -> String + Send>;

/// `TURU_PERF_LOG`'s period (1 s at least), when it's set.
pub fn perf_log_every() -> Option<Duration> {
    let seconds = std::env::var("TURU_PERF_LOG")
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()?;
    Some(Duration::from_secs_f64(seconds.max(1.0)))
}

/// With `TURU_PERF_LOG=<seconds>`, logs each tab's Flash calls and player ticks over the last
/// `seconds` (`turu_bot::perf`), with several tabs the player thread's time for all of them, and
/// `main_thread`'s report (the window's event loop).
pub fn start_perf_log(mut main_thread: Option<ThreadReport>) {
    let Some(every) = perf_log_every() else {
        return;
    };
    // Turu: the 12 busiest calls hid half a stuck script's calls.
    let rows = std::env::var("TURU_PERF_LOG_ROWS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(12);
    tracing::info!(
        "Logging Flash calls and ticks every {} s (TURU_PERF_LOG)",
        every.as_secs_f64()
    );
    let spawned = std::thread::Builder::new().name("Turu Perf Log".into()).spawn(move || {
        let mut last: HashMap<TabId, turu_bot::perf::PerfSnapshot> = HashMap::new();
        loop {
            std::thread::sleep(every);
            let tabs = tabs();
            last.retain(|tab, _| tabs.iter().any(|(t, _)| t == tab));
            let mut busy = 0.0;
            let mut seconds: f64 = 0.0;
            for (tab, bot) in &tabs {
                let now = bot.flash.perf().snapshot();
                let window = last.get(tab).map_or_else(|| now.clone(), |before| now.since(before));
                busy += window.busy_ms();
                seconds = seconds.max(window.seconds);
                tracing::info!(target: "turu_perf", "{}", window.report(&format!("tab {}", tab_label(bot)), rows));
                last.insert(*tab, now);
            }
            if tabs.len() > 1 {
                tracing::info!(
                    target: "turu_perf",
                    "all {} tabs: player thread {:.0} ms/s",
                    tabs.len(),
                    busy / seconds.max(0.001)
                );
            }
            if let Some(report) = &mut main_thread {
                tracing::info!(target: "turu_perf", "{}", report(busy));
            }
        }
    });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start the perf log: {e}");
    }
}

struct Monitor {
    server: Option<turu_bot::monitor::server::Server>,
    /// What `server` was started with: allow other devices, port.
    running: Option<(bool, u16)>,
    /// Why it isn't running.
    error: Option<String>,
    key: Option<Arc<turu_bot::monitor::server::Key>>,
    push_key: Option<Arc<turu_bot::monitor::server::Key>>,
    /// Reporting to another machine's monitor, and what to.
    reporter: Option<turu_bot::monitor::push::Reporter>,
    reporting: Option<turu_bot::monitor::push::ReportTo>,
    /// Why Report To didn't start.
    report_error: Option<String>,
    /// Bumped by every `apply_monitor`; only the latest one applies.
    generation: u64,
}

fn monitor_lock() -> std::sync::MutexGuard<'static, Monitor> {
    MONITOR.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the web monitor is doing, for Options > Application.
pub enum MonitorStatus {
    Off,
    /// Served here; for this computer and, with Allow Other Devices, for others.
    Serving(std::net::SocketAddr),
    Failed(String),
    /// Army > Accounts launched this client: its launcher's monitor shows it.
    LaunchedByArmy,
}

pub fn monitor_status() -> MonitorStatus {
    if LAUNCHED_BY_ARMY.load(Ordering::Relaxed) {
        return MonitorStatus::LaunchedByArmy;
    }
    let monitor = monitor_lock();
    match (&monitor.server, &monitor.error) {
        (Some(server), _) => MonitorStatus::Serving(server.addr()),
        (None, Some(error)) => MonitorStatus::Failed(error.clone()),
        (None, None) => MonitorStatus::Off,
    }
}

/// How reporting to another machine's monitor goes, for Options > Application.
pub enum ReportState {
    Off,
    Failed(String),
    /// As this machine name, with its last report.
    Reporting(String, turu_bot::monitor::push::ReportStatus),
}

pub fn report_state() -> ReportState {
    let monitor = monitor_lock();
    match (&monitor.reporter, &monitor.reporting, &monitor.report_error) {
        (Some(reporter), Some(to), _) => {
            ReportState::Reporting(to.machine.clone(), reporter.status())
        }
        (_, _, Some(error)) => ReportState::Failed(error.clone()),
        _ => ReportState::Off,
    }
}

/// This computer's name, as reports use it when Machine Name is empty (asked once).
pub fn default_machine_name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(turu_bot::monitor::push::default_machine_name)
}

/// Starts the web monitor and Report To at launch when Options > Application (or `--monitor`,
/// `--report-to`) has them on. Only this process does: clients launched by Army > Accounts
/// don't.
pub fn start_monitor() {
    let settings = bot().settings.read().clone();
    let generation = monitor_lock().generation;
    apply_monitor_now(&settings, generation);
}

/// Starts, restarts or stops the web monitor and Report To for these settings, half a second
/// later (dragging the port changes it every frame), off the UI thread (stopping waits for the
/// server's thread).
pub fn apply_monitor(settings: &turu_bot::settings::AppSettings) {
    let generation = {
        let mut monitor = monitor_lock();
        monitor.generation += 1;
        monitor.generation
    };
    let settings = settings.clone();
    let _ = std::thread::Builder::new()
        .name("Turu Monitor Apply".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            apply_monitor_now(&settings, generation);
        });
}

/// For the process that runs the app's once-only jobs while Options > Application is in other
/// processes too (a desktop shell, Android's slot 1): reads `settings.json` again every 2 s and
/// applies a change to the web monitor or Report To. With `adopt` (a process without Turu's UI)
/// the settings it holds follow the file too.
pub fn follow_settings_file(adopt: bool) {
    let spawned = std::thread::Builder::new()
        .name("Turu Settings Watch".into())
        .spawn(move || {
            let bot = bot();
            let mut seen = bot.settings.read().clone();
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let Some(settings) = bot.paths.load_settings() else {
                    continue;
                };
                if settings != seen {
                    apply_monitor(&settings);
                    if adopt {
                        *bot.settings.write() = settings.clone();
                    }
                    seen = settings;
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Couldn't watch the settings file: {e}");
    }
}

/// The settings with `--monitor`, `--report-to` and `TURU_MONITOR_PUSH_KEY` applied.
fn monitor_settings(settings: &turu_bot::settings::AppSettings) -> turu_bot::settings::AppSettings {
    let mut settings = settings.clone();
    let flag =
        |slot: &Mutex<Option<String>>| slot.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let Some(addr) = flag(&MONITOR_FLAG) {
        settings.monitor_enabled = true;
        if let Ok(addr) = addr.parse::<std::net::SocketAddr>() {
            settings.monitor_port = i32::from(addr.port());
            settings.monitor_allow_other_devices = !addr.ip().is_loopback();
        }
    }
    if let Some(url) = flag(&REPORT_FLAG) {
        settings.monitor_report_to = url;
    }
    if let Some(key) = std::env::var("TURU_MONITOR_PUSH_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
    {
        settings.monitor_report_key = key;
    }
    settings
}

fn apply_monitor_now(settings: &turu_bot::settings::AppSettings, generation: u64) {
    use turu_bot::monitor::push::{ReportTo, Reporter};
    use turu_bot::monitor::server::{Server, load_key, load_push_key};
    if !runs_monitor() {
        return;
    }
    let settings = monitor_settings(settings);
    let mut monitor = monitor_lock();
    if monitor.generation != generation {
        return;
    }

    // Report To.
    let url = settings.monitor_report_to.trim();
    let report = (!url.is_empty()).then(|| ReportTo {
        url: url.to_owned(),
        push_key: settings.monitor_report_key.trim().to_owned(),
        machine: match settings.monitor_machine_name.trim() {
            "" => default_machine_name().to_owned(),
            name => turu_bot::monitor::hub::machine_name(name),
        },
        accept_commands: settings.monitor_accept_commands,
        version: VERSION.into(),
    });
    if report != monitor.reporting {
        // Stopping waits for the report in flight: not while holding the lock.
        if let Some(old) = monitor.reporter.take() {
            let _ = std::thread::Builder::new()
                .name("Turu Monitor Report Stop".into())
                .spawn(move || drop(old));
        }
        monitor.report_error = None;
        monitor.reporting.clone_from(&report);
        if let Some(to) = report {
            match Reporter::start(monitor_hub().clone(), to.clone()) {
                Ok(reporter) => {
                    tracing::info!("Reporting to the monitor at {} as {}", to.url, to.machine);
                    monitor.reporter = Some(reporter);
                }
                Err(e) => {
                    tracing::error!("Report To: {e}");
                    monitor.report_error = Some(e);
                }
            }
        }
    }

    // The server.
    let port = u16::try_from(settings.monitor_port).unwrap_or(turu_bot::monitor::DEFAULT_PORT);
    let wanted = settings
        .monitor_enabled
        .then_some((settings.monitor_allow_other_devices, port));
    if wanted == monitor.running && (monitor.server.is_some() || wanted.is_none()) {
        if let Some(server) = &monitor.server {
            server.set_allow_control(settings.monitor_allow_control);
        }
        return;
    }
    // Closes the old listener before binding the new one (the same port, maybe).
    monitor.server = None;
    monitor.running = None;
    monitor.error = None;
    let Some((others, port)) = wanted else {
        tracing::info!("Web monitor stopped");
        return;
    };
    let keys = loaded_key(&mut monitor.key, load_key)
        .and_then(|key| Ok((key, loaded_key(&mut monitor.push_key, load_push_key)?)));
    let (key, push_key) = match keys {
        Ok(keys) => keys,
        Err(e) => {
            monitor.error = Some(format!("couldn't read the monitor keys: {e}"));
            return;
        }
    };
    let ip = if others {
        std::net::Ipv4Addr::UNSPECIFIED
    } else {
        std::net::Ipv4Addr::LOCALHOST
    };
    let hub = monitor_hub().clone();
    let control = settings.monitor_allow_control;
    match Server::start(
        (ip, port).into(),
        key,
        push_key,
        hub,
        VERSION.into(),
        control,
    ) {
        Ok(server) => {
            tracing::info!("Web monitor on http://{}", server.addr());
            monitor.server = Some(server);
            monitor.running = wanted;
        }
        Err(e) => {
            tracing::error!("Web monitor: {e}");
            monitor.error = Some(e);
        }
    }
}

type KeyLoader = fn(&turu_bot::paths::Paths) -> Result<String, String>;

/// The key in `slot`, read from its file (made the first time) when it's not there yet.
fn loaded_key(
    slot: &mut Option<Arc<turu_bot::monitor::server::Key>>,
    load: KeyLoader,
) -> Result<Arc<turu_bot::monitor::server::Key>, String> {
    if let Some(key) = slot {
        return Ok(key.clone());
    }
    let key = turu_bot::monitor::server::Key::new(load(&bot().paths)?);
    *slot = Some(key.clone());
    Ok(key)
}

/// The monitor's key, made the first time it's asked for.
pub fn monitor_key() -> Result<String, String> {
    let mut monitor = monitor_lock();
    loaded_key(&mut monitor.key, turu_bot::monitor::server::load_key).map(|k| k.get())
}

/// The key other machines report with (Copy Report Settings).
pub fn monitor_push_key() -> Result<String, String> {
    let mut monitor = monitor_lock();
    loaded_key(
        &mut monitor.push_key,
        turu_bot::monitor::server::load_push_key,
    )
    .map(|k| k.get())
}

/// A new key: devices given the old one are cut off at their next request.
pub fn regenerate_monitor_key() -> Result<(), String> {
    let key = turu_bot::monitor::server::new_key(&bot().paths)?;
    let mut monitor = monitor_lock();
    loaded_key(&mut monitor.key, |_| Ok(String::new()))?.set(key);
    Ok(())
}

/// A new push key: machines reporting with the old one are refused from their next report.
pub fn regenerate_push_key() -> Result<(), String> {
    let key = turu_bot::monitor::server::new_push_key(&bot().paths)?;
    let mut monitor = monitor_lock();
    loaded_key(&mut monitor.push_key, |_| Ok(String::new()))?.set(key);
    Ok(())
}

/// This computer's address on its network (the interface its default route uses), for links
/// other devices open. Nothing is sent: connecting a UDP socket only picks the route.
pub fn lan_address() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

/// `--headless` was given: run without a window.
pub fn headless() -> bool {
    HEADLESS.load(Ordering::Relaxed)
}

/// Marks this process as one of a shell's tabs (an Android slot; a desktop client is one by
/// `--client`).
pub fn mark_shell_tab() {
    SHELL_TAB.store(true, Ordering::Relaxed);
}

/// This process is one of a shell's tabs: a desktop client or an Android slot. The app's
/// once-only jobs (update checks, the web monitor, `monitor.json`) aren't its.
pub fn shell_tab() -> bool {
    SHELL_TAB.load(Ordering::Relaxed) || client_of().is_some()
}

/// Leaves the app's once-only jobs to another process: launched by Army > Accounts (its launcher
/// runs them), or a shell's tab that doesn't hold the monitor's lock.
fn leaves_app_jobs() -> bool {
    !runs_monitor()
}

/// The lock of the one tab that runs the web monitor among a shell's tabs that have no shell
/// running it (Android's slots: `start_monitor_election`).
static MONITOR_LOCK: Mutex<Option<turu_bot::shared_job::Held>> = Mutex::new(None);

/// This process runs the web monitor, Report To and the client update checks: it isn't Army's
/// launched client, and isn't a shell's tab or holds the monitor's lock.
fn runs_monitor() -> bool {
    !LAUNCHED_BY_ARMY.load(Ordering::Relaxed)
        && (!shell_tab() || MONITOR_LOCK.lock().unwrap_or_else(|e| e.into_inner()).is_some())
}

/// Among tabs in processes of their own with no shell that stays (Android's slots): the first to
/// take the monitor's lock runs the web monitor, Report To and the update checks, and keeps them
/// until its process ends; the system then lets the lock go, and another tab takes it (tried every
/// 3 s). That tab polls the others all along and follows Options changed in theirs.
pub fn start_monitor_election() {
    // Turu: only the tab that wins runs the update checks; the others show what it found.
    turu_bot::updates::share_through(bot().paths.root.join("update-available.json"));
    let spawned = std::thread::Builder::new()
        .name("Turu Monitor Election".into())
        .spawn(|| {
            let dir = turu_bot::shared_job::dir(&bot().paths.root);
            loop {
                match turu_bot::shared_job::try_hold(&dir, "monitor") {
                    Ok(Some(held)) => {
                        *MONITOR_LOCK.lock().unwrap_or_else(|e| e.into_inner()) = Some(held);
                        tracing::info!("This tab runs the web monitor and the update checks");
                        monitor_hub().set_saving(true);
                        install_peers();
                        start_monitor();
                        start_client_updates();
                        follow_settings_file(false);
                        return;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!("The monitor's lock: {e}");
                        return;
                    }
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("Couldn't start the monitor's election: {e}");
    }
}

/// `--client=<address>`: the shell to connect to; this process runs one tab without a window and
/// draws it for the shell (`app/src/client.rs`).
pub fn client_of() -> Option<String> {
    CLIENT_OF.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `--shell`: this process owns the window and runs each tab in a client process
/// (`app/src/shell.rs`).
pub fn shell() -> bool {
    SHELL.load(Ordering::Relaxed)
}

/// The active tab's game is in Headless Mode (the option, not `--headless`): the window covers
/// it with VibeSkua's headless overlay and doesn't draw it.
pub fn headless_mode() -> bool {
    bot().state.lock().options_applied.headless()
}

/// Turu's flags.
#[derive(Default)]
struct Flags {
    startup: Startup,
    exit_on_stop: bool,
    exit_with_parent: bool,
    headless: bool,
    /// `--client`'s address.
    client: Option<String>,
    shell: bool,
    /// `--api`'s address.
    api: Option<String>,
    /// `--monitor[=address]`: empty without an address.
    monitor: Option<String>,
    /// `--report-to`'s URL.
    report_to: Option<String>,
}

/// Removes Turu's flags from `args` and returns them.
fn parse_startup_args(args: &mut Vec<OsString>) -> Flags {
    let mut flags = Flags::default();
    let startup = &mut flags.startup;
    let mut rest = Vec::with_capacity(args.len());
    let mut iter = std::mem::take(args).into_iter();
    while let Some(arg) = iter.next() {
        let Some(text) = arg.to_str() else {
            rest.push(arg);
            continue;
        };
        let (flag, inline) = match text.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_owned())),
            _ => (text, None),
        };
        let mut value = || {
            inline
                .clone()
                .or_else(|| iter.next().and_then(|v| v.into_string().ok()))
        };
        match flag {
            "--user" => startup.username = value(),
            "--password" => startup.password = value(),
            "--server" => startup.server = value(),
            "--run-script" => {
                startup.script = value().map(Into::into);
                startup.run_script = true;
            }
            "--load-script" => {
                startup.script = value().map(Into::into);
                startup.run_script = false;
            }
            "--run-playlist" => {
                startup.playlist = value().map(Into::into);
                startup.run_playlist = true;
            }
            "--load-playlist" => {
                startup.playlist = value().map(Into::into);
                startup.run_playlist = false;
            }
            "--script-options-storage" => startup.options_storage = value(),
            "--exit-on-script-stop" | "--exit-on-finish" => flags.exit_on_stop = true,
            "--headless" => flags.headless = true,
            "--exit-with-parent" => flags.exit_with_parent = true,
            // The address only inline (`--client=127.0.0.1:port`).
            "--client" => flags.client = inline,
            "--shell" => flags.shell = true,
            // (Ignored since M6: an old shortcut with it still starts.)
            "--in-process" => {}
            // The address only inline (`--api=...`): the next argument may be anything.
            "--api" => flags.api = Some(inline.unwrap_or_else(|| "127.0.0.1:9091".into())),
            // Likewise: `--monitor` alone takes Options > Application's port.
            "--monitor" => flags.monitor = Some(inline.unwrap_or_default()),
            "--report-to" => flags.report_to = value(),
            _ => rest.push(arg),
        }
    }
    *args = rest;
    flags
}

/// VibeSkua `--exit-on-script-stop`: 2 s after the script stops, log out; 1 s later, quit. With
/// a playlist (`--run-playlist`), once the playlist has run and ended (finished or Stop queue)
/// and no script runs: a script stopping only moves the playlist on.
fn exit_on_script_stop(playlist: bool) {
    let first = bot();
    if playlist {
        let watched = Arc::downgrade(&first);
        let _ = std::thread::Builder::new()
            .name("Turu Exit".into())
            .spawn(move || {
                let mut started = false;
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let Some(bot) = watched.upgrade() else { return };
                    if bot.scheduler.is_running() {
                        started = true;
                    } else if started && !bot.scripts.is_running() && !bot.servers.is_relogging() {
                        exit_after_stop(&bot);
                    }
                }
            });
        return;
    }
    let watched = Arc::downgrade(&first);
    first.events.subscribe(move |event| {
        let Some(bot) = watched.upgrade() else { return };
        if !matches!(event, BotEvent::ScriptStopped) || bot.servers.is_relogging() {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("Turu Exit".into())
            .spawn(move || exit_after_stop(&bot));
    });
}

fn exit_after_stop(bot: &Arc<Bot>) {
    std::thread::sleep(Duration::from_secs(2));
    // Turu: a script that found itself logged out may stop just before the drop starts the
    // auto-relogin, which restarts it: the run goes on.
    if bot.servers.is_relogging() || bot.scripts.is_running() {
        return;
    }
    let _ = turu_bot::api::servers::logout(bot);
    std::thread::sleep(Duration::from_secs(1));
    // Turu: a shell's tab asks the shell to close it instead of quitting. The shell starts a tab
    // whose client quit again, with its flags: the script would run once more.
    if client_of().is_some() {
        match shell_tabs().and_then(|(tabs, _)| tabs.into_iter().find(|tab| tab.me)) {
            Some(me) => shell_tab_action(ShellTabAction::Close(me.id)),
            None => tracing::warn!("--exit-on-script-stop: the shell's tab list hasn't come; the tab stays open"),
        }
        return;
    }
    close_butlers();
    std::process::exit(0);
}

/// Runs a bot action for the active tab off the UI thread (bot calls block until the player
/// answers, and the player is driven by the UI thread).
pub fn spawn_action(
    name: &str,
    action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static,
) {
    spawn_action_for(bot(), name, action);
}

/// [`spawn_action`] for a given tab's bot.
pub fn spawn_action_for(
    bot: Arc<Bot>,
    name: &str,
    action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static,
) {
    let label = name.to_owned();
    let spawned = std::thread::Builder::new()
        .name(format!("Turu {name}"))
        .spawn(move || {
            if let Err(e) = action(&bot) {
                bot.logs.debug.push(format!("{label}: {e}"));
            }
        });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start {name}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_mode_follows_the_tab_that_changed_it() {
        let bots: Vec<_> = (0..3).map(|i| (TabId(100 + i), Bot::new())).collect();
        for (_, bot) in &bots {
            bot.options.write().headless_mode = false;
        }
        let modes = |tabs: &[(TabId, Arc<Bot>)]| {
            tabs.iter()
                .map(|(_, b)| b.opts().headless_mode)
                .collect::<Vec<_>>()
        };
        let mut seen = HashMap::new();
        let mut on = sync_headless_mode(&bots, &mut seen, false);
        assert!(!on);

        // One tab turns it on: every tab has it.
        bots[1].1.options.write().headless_mode = true;
        on = sync_headless_mode(&bots, &mut seen, on);
        assert!(on);
        assert_eq!(modes(&bots), [true, true, true]);

        // A new tab, off in its options.json, gets it rather than turning it off for the others.
        let mut tabs = bots.clone();
        let new = Bot::new();
        new.options.write().headless_mode = false;
        tabs.push((TabId(200), new));
        on = sync_headless_mode(&tabs, &mut seen, on);
        assert!(on);
        assert_eq!(modes(&tabs), [true, true, true, true]);

        // Another tab turns it off: off on every tab.
        tabs[3].1.options.write().headless_mode = false;
        on = sync_headless_mode(&tabs, &mut seen, on);
        assert!(!on);
        assert_eq!(modes(&tabs), [false, false, false, false]);

        // A closed tab is forgotten.
        on = sync_headless_mode(&tabs[..2], &mut seen, on);
        assert!(!on);
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn script_stopped_alert_says_how_it_ended() {
        let end = |finished, error: Option<&str>| turu_bot::script::RunEnd {
            script: PathBuf::from("/data/scripts/farm/GoldFarm.ts"),
            started: true,
            finished,
            error: error.map(str::to_owned),
            ran: Duration::from_secs(60),
            ended: chrono::Local::now(),
        };
        assert_eq!(
            script_stopped_alert("Hero", &end(true, None)),
            ("Script finished".into(), "Hero: GoldFarm".into())
        );
        assert_eq!(
            script_stopped_alert("Hero", &end(false, None)),
            ("Script stopped".into(), "Hero: GoldFarm".into())
        );
        assert_eq!(
            script_stopped_alert("Hero", &end(false, Some("BotError: map.join\n  at main"))),
            ("Script failed".into(), "Hero: GoldFarm (BotError: map.join)".into())
        );
    }

    #[test]
    fn takes_the_monitor_flags() {
        let mut args: Vec<OsString> = [
            "turu",
            "--monitor",
            "--report-to",
            "http://pc:9021",
            "x.swf",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let flags = parse_startup_args(&mut args);
        assert_eq!(args, ["turu", "x.swf"]);
        assert_eq!(flags.monitor.as_deref(), Some(""));
        assert_eq!(flags.report_to.as_deref(), Some("http://pc:9021"));
        let mut args: Vec<OsString> = ["turu", "--monitor=0.0.0.0:9000"]
            .into_iter()
            .map(Into::into)
            .collect();
        assert_eq!(
            parse_startup_args(&mut args).monitor.as_deref(),
            Some("0.0.0.0:9000")
        );
    }

    #[test]
    fn takes_the_old_in_process_flag_and_ignores_it() {
        let mut args: Vec<OsString> = ["turu", "--in-process", "-g", "metal"]
            .into_iter()
            .map(Into::into)
            .collect();
        let flags = parse_startup_args(&mut args);
        assert_eq!(args, ["turu", "-g", "metal"]);
        assert!(!flags.shell);
    }

    #[test]
    fn takes_turu_flags_and_leaves_ruffle_ones() {
        let mut args: Vec<OsString> = [
            "turu",
            "-p",
            "low",
            "--user",
            "Hero",
            "--password=a=b",
            "--server",
            "Artix",
        ]
        .into_iter()
        .chain([
            "--run-script",
            "farm/GoldFarm.ts",
            "--exit-on-finish",
            "--headless",
            "--api",
            "-g",
            "metal",
        ])
        .map(Into::into)
        .collect();
        let Flags {
            startup,
            exit_on_stop: exit,
            headless,
            api,
            ..
        } = parse_startup_args(&mut args);
        assert_eq!(args, ["turu", "-p", "low", "-g", "metal"]);
        assert!(headless);
        assert_eq!(api.as_deref(), Some("127.0.0.1:9091"));
        assert_eq!(startup.username.as_deref(), Some("Hero"));
        assert_eq!(startup.password.as_deref(), Some("a=b"));
        assert_eq!(startup.server.as_deref(), Some("Artix"));
        assert_eq!(startup.script, Some("farm/GoldFarm.ts".into()));
        assert!(startup.run_script && startup.logs_in() && exit);
    }
}
