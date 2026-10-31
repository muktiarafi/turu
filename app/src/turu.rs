//! Turu: connects the bot engine (`turu_bot`) to the player.
//!
//! * Tabs: every tab is a game client of its own, a player and its [`Bot`] (VibeSkua runs one
//!   process per tab instead). All the players run on the event loop thread; the window shows
//!   the active tab's, and [`bot()`] is the active tab's bot, which the UI shows.
//! * [`TuruExternalInterface`] receives a tab's `skua.swf`'s `ExternalInterface.call(...)`s.
//! * [`DesktopFlashHost`] lets bot threads call `skua.swf`'s callbacks: each call is sent to
//!   the event loop as [`RuffleEvent::TuruFlashCall`] (without a window, to the `headless`
//!   loop), names its tab, and is answered by [`answer_flash_call`] with that tab's player.
//!
//! See docs/ARCHITECTURE.md.

use crate::custom_event::RuffleEvent;
use ruffle_core::Player;
use ruffle_core::context::UpdateContext;
use ruffle_core::external::{ExternalInterfaceProvider, Value as ExternalValue};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::Duration;
use turu_bot::events::BotEvent;
use turu_bot::startup::Startup;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};
use winit::event_loop::EventLoopProxy;

pub const APP_NAME: &str = "Turu";
/// The release workflow sets `TURU_VERSION` from the git tag; local builds show the crate version.
pub const VERSION: &str = match option_env!("TURU_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

/// How long a bot thread waits for the player to answer one call.
const CALL_TIMEOUT: Duration = Duration::from_secs(20);

static TABS: Mutex<Option<Tabs>> = Mutex::new(None);
/// Where tab requests go ([`request_tab`]); set when the window opens.
static EVENT_LOOP: Mutex<Option<EventLoopProxy<RuffleEvent>>> = Mutex::new(None);
static ARMY: OnceLock<Arc<turu_bot::army::Army>> = OnceLock::new();
static MONITOR_HUB: OnceLock<Arc<turu_bot::monitor::Hub>> = OnceLock::new();
// Turu: Search Scripts' repository, and whether Army > Accounts launched this client.
static SCRIPT_REPO: OnceLock<Arc<turu_bot::repo::ScriptRepo>> = OnceLock::new();
static LAUNCHED_BY_ARMY: AtomicBool = AtomicBool::new(false);
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();
static HEADLESS: AtomicBool = AtomicBool::new(false);
/// `--api`'s address, until `start_api`.
static API: Mutex<Option<String>> = Mutex::new(None);
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
    tabs.get_or_insert_with(|| Tabs {
        list: vec![(TabId(0), Bot::new())],
        active: TabId(0),
        next: 1,
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
    tabs_lock().as_ref().expect("made by tabs_lock").list.clone()
}

/// The tab's bot; `None` once it was closed.
pub fn tab_bot(tab: TabId) -> Option<Arc<Bot>> {
    tabs().into_iter().find(|(id, _)| *id == tab).map(|(_, bot)| bot)
}

/// Adds a tab with a new bot (its player is the event loop's job: [`request_tab`]). With a
/// login, the bot logs in once its game has loaded, as with `--user`.
pub(crate) fn add_tab(startup: Startup) -> (TabId, Arc<Bot>) {
    let bot = Bot::new();
    bot.scripts.set_interactive(!headless());
    turu_bot::startup::configure(&bot, startup);
    let mut tabs = tabs_lock();
    let tabs = tabs.as_mut().expect("made by tabs_lock");
    let id = TabId(tabs.next);
    tabs.next += 1;
    tabs.list.push((id, bot.clone()));
    (id, bot)
}

/// Removes the tab (never the last one): stops its script and Auto Attack.
pub(crate) fn remove_tab(tab: TabId) -> Option<Arc<Bot>> {
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
    turu_bot::script::stop(&removed);
    removed.control.request_stop();
    Some(removed)
}

/// Makes the tab the active one; false when there's no such tab. The others' games slow down
/// once in game (VibeSkua's tabbed host throttles hidden tabs: `options::apply`).
pub(crate) fn set_active_tab(tab: TabId) -> bool {
    let mut tabs = tabs_lock();
    let tabs = tabs.as_mut().expect("made by tabs_lock");
    if !tabs.list.iter().any(|(id, _)| *id == tab) {
        return false;
    }
    tabs.active = tab;
    for (id, bot) in &tabs.list {
        bot.state.lock().hidden_tab = *id != tab;
    }
    true
}

/// What the UI asks of the event loop, which owns the players.
pub enum TabRequest {
    /// Open a new tab (at the login screen, or logging in with `Startup`'s login).
    Open(Box<Startup>),
    Select(TabId),
    Close(TabId),
}

pub fn set_event_loop(event_loop: EventLoopProxy<RuffleEvent>) {
    *EVENT_LOOP.lock().unwrap_or_else(|e| e.into_inner()) = Some(event_loop);
}

/// Hands the request to the event loop; false without a window (`--headless` has one tab).
pub fn request_tab(request: TabRequest) -> bool {
    EVENT_LOOP
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|e| e.send_event(RuffleEvent::TuruTab(request)).is_ok())
}

/// A tab's name: the character once in game, else the login name it was opened with.
pub fn tab_label(bot: &Bot) -> String {
    let state = bot.state.lock();
    if let Some(name) = state.username.as_deref().filter(|n| state.in_game && !n.trim().is_empty()) {
        return name.to_owned();
    }
    drop(state);
    turu_bot::api::servers::login_name(bot).unwrap_or_else(|| "Login".into())
}

/// Search Scripts' repository and scripts folder, shared by the tabs (they share the data
/// folder).
pub fn script_repo() -> &'static Arc<turu_bot::repo::ScriptRepo> {
    SCRIPT_REPO.get_or_init(|| {
        turu_bot::repo::ScriptRepo::new(bot().paths.clone(), Box::new(turu_bot::repo::GitHub::from_env()))
    })
}

/// At launch, Options > Application's Check for Script Updates / Auto Update Scripts (VibeSkua's
/// Manager does them). Clients launched by Army > Accounts skip it: their launcher has the same
/// scripts folder.
pub fn start_script_updates() {
    if LAUNCHED_BY_ARMY.load(Ordering::Relaxed) {
        return;
    }
    let settings = bot().settings.read().clone();
    if !settings.check_bot_scripts_updates {
        return;
    }
    script_repo().spawn("Script update", move |repo| {
        let message = repo.startup(&settings)?;
        tracing::info!("Scripts: {message}");
        Ok(message)
    });
}

/// The accounts this client launched (Army > Accounts), each in its own `turu` process.
pub fn army() -> &'static Arc<turu_bot::army::Army> {
    ARMY.get_or_init(|| {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from(APP_NAME.to_lowercase()));
        let army = turu_bot::army::Army::new(exe);
        army.set_hub(monitor_hub().clone());
        army
    })
}

/// The web monitor's hub: this window's tabs and the Army clients, kept even while the monitor
/// is off (Army Control reads the clients from it).
pub fn monitor_hub() -> &'static Arc<turu_bot::monitor::Hub> {
    MONITOR_HUB.get_or_init(|| {
        let hub = turu_bot::monitor::Hub::new(Some(bot().paths.monitor_file()));
        hub.set_tabs(Arc::new(|| {
            tabs()
                .into_iter()
                .enumerate()
                .map(|(i, (id, bot))| turu_bot::monitor::hub::Source {
                    id: format!("tab:{}", id.0),
                    label: format!("Tab {}", i + 1),
                    bot,
                })
                .collect()
        }));
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

/// Mugetsu's title, plus the character's name while you play when Options > Application >
/// "Show Username in Title" is on (VibeSkua names its tab after the character).
pub fn window_title() -> String {
    let title = crate::mugetsu::window_title();
    let bot = bot();
    if !bot.settings.read().show_username_in_title {
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

/// The URL Turu starts with: VibeSkua's `skua.swf`, written to the data directory.
pub fn skua_swf_url() -> anyhow::Result<String> {
    let path = bot().paths.write_skua_swf()?;
    let url =
        url::Url::from_file_path(&path).map_err(|()| anyhow::anyhow!("bad path for skua.swf: {}", path.display()))?;
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
///   only loads it.
/// * `--exit-on-script-stop` (or `--exit-on-finish`): log out and quit when the script stops
///   (not when auto-relogin stops it).
/// * `--headless`: no window (see `headless.rs`).
/// * `--exit-with-parent`: quit when stdin closes (Army > Accounts launches clients with it,
///   their stdin a pipe from the launcher: see `turu_bot::army`).
/// * `--api` (or `--api=<address>`, default `127.0.0.1:8791`): the control API (see
///   `turu_bot::control_api`), started by [`start_api`].
pub fn take_startup_args(args: &mut Vec<OsString>) {
    let Flags {
        mut startup,
        exit_on_stop,
        exit_with_parent,
        headless,
        api,
        monitor,
        report_to,
    } = parse_startup_args(args);
    if exit_with_parent {
        LAUNCHED_BY_ARMY.store(true, Ordering::Relaxed);
        turu_bot::army::exit_with_parent(|| std::process::exit(0));
    }
    HEADLESS.store(headless, Ordering::Relaxed);
    if let Ok(mut slot) = API.lock() {
        *slot = api;
    }
    if let Some(addr) = monitor.as_deref().filter(|a| !a.is_empty())
        && addr.parse::<std::net::SocketAddr>().is_err()
    {
        tracing::warn!("--monitor={addr}: expected an address like 0.0.0.0:8790; using the settings' port");
    }
    *MONITOR_FLAG.lock().unwrap_or_else(|e| e.into_inner()) = monitor;
    *REPORT_FLAG.lock().unwrap_or_else(|e| e.into_inner()) = report_to;
    if startup.password.is_none() {
        startup.password = std::env::var("TURU_PASSWORD").ok().filter(|p| !p.is_empty());
    }
    if startup.username.is_some() && !startup.logs_in() {
        tracing::warn!("--user without --password (or TURU_PASSWORD): log in yourself");
    }
    turu_bot::startup::configure(&bot(), startup);
    if exit_on_stop {
        exit_on_script_stop();
    }
}

/// Starts the control API if `--api` was given (once logging is up, so its messages show). The
/// token is `TURU_API_TOKEN`, else the one in the data folder's `api-token` (made the first time).
pub fn start_api() {
    let Some(addr) = API.lock().ok().and_then(|mut a| a.take()) else {
        return;
    };
    let addr: std::net::SocketAddr = match addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            tracing::error!("--api={addr}: {e} (expected an address like 127.0.0.1:8791)");
            return;
        }
    };
    let token = match std::env::var("TURU_API_TOKEN").ok().filter(|t| !t.trim().is_empty()) {
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
    match token.and_then(|token| turu_bot::control_api::start(&bot(), addr, token)) {
        Ok(local) => {
            tracing::info!("Control API on http://{local}");
            if !local.ip().is_loopback() {
                tracing::warn!("The control API is reachable from other computers: keep the token secret");
            }
        }
        Err(e) => tracing::error!("Control API: {e}"),
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
        (Some(reporter), Some(to), _) => ReportState::Reporting(to.machine.clone(), reporter.status()),
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
    let _ = std::thread::Builder::new().name("Turu Monitor Apply".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        apply_monitor_now(&settings, generation);
    });
}

/// The settings with `--monitor`, `--report-to` and `TURU_MONITOR_PUSH_KEY` applied.
fn monitor_settings(settings: &turu_bot::settings::AppSettings) -> turu_bot::settings::AppSettings {
    let mut settings = settings.clone();
    let flag = |slot: &Mutex<Option<String>>| slot.lock().unwrap_or_else(|e| e.into_inner()).clone();
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
    if let Some(key) = std::env::var("TURU_MONITOR_PUSH_KEY").ok().filter(|k| !k.trim().is_empty()) {
        settings.monitor_report_key = key;
    }
    settings
}

fn apply_monitor_now(settings: &turu_bot::settings::AppSettings, generation: u64) {
    use turu_bot::monitor::push::{ReportTo, Reporter};
    use turu_bot::monitor::server::{Server, load_key, load_push_key};
    if LAUNCHED_BY_ARMY.load(Ordering::Relaxed) {
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
            let _ = std::thread::Builder::new().name("Turu Monitor Report Stop".into()).spawn(move || drop(old));
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
    let wanted = settings.monitor_enabled.then_some((settings.monitor_allow_other_devices, port));
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
    match Server::start((ip, port).into(), key, push_key, hub, VERSION.into(), control) {
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
    loaded_key(&mut monitor.push_key, turu_bot::monitor::server::load_push_key).map(|k| k.get())
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

/// Turu's flags.
#[derive(Default)]
struct Flags {
    startup: Startup,
    exit_on_stop: bool,
    exit_with_parent: bool,
    headless: bool,
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
            "--exit-on-script-stop" | "--exit-on-finish" => flags.exit_on_stop = true,
            "--headless" => flags.headless = true,
            "--exit-with-parent" => flags.exit_with_parent = true,
            // The address only inline (`--api=...`): the next argument may be anything.
            "--api" => flags.api = Some(inline.unwrap_or_else(|| "127.0.0.1:8791".into())),
            // Likewise: `--monitor` alone takes Options > Application's port.
            "--monitor" => flags.monitor = Some(inline.unwrap_or_default()),
            "--report-to" => flags.report_to = value(),
            _ => rest.push(arg),
        }
    }
    *args = rest;
    flags
}

/// VibeSkua `--exit-on-script-stop`: 2 s after the script stops, log out; 1 s later, quit.
fn exit_on_script_stop() {
    let first = bot();
    let watched = Arc::downgrade(&first);
    first.events.subscribe(move |event| {
        let Some(bot) = watched.upgrade() else { return };
        if !matches!(event, BotEvent::ScriptStopped) || bot.servers.is_relogging() {
            return;
        }
        let _ = std::thread::Builder::new().name("Turu Exit".into()).spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            let _ = turu_bot::api::servers::logout(&bot);
            std::thread::sleep(Duration::from_secs(1));
            std::process::exit(0);
        });
    });
}

/// Runs a bot action for the active tab off the UI thread (bot calls block until the player
/// answers, and the player is driven by the UI thread).
pub fn spawn_action(name: &str, action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static) {
    spawn_action_for(bot(), name, action);
}

/// [`spawn_action`] for a given tab's bot.
pub fn spawn_action_for(
    bot: Arc<Bot>,
    name: &str,
    action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static,
) {
    let label = name.to_owned();
    let spawned = std::thread::Builder::new().name(format!("Turu {name}")).spawn(move || {
        if let Err(e) = action(&bot) {
            bot.logs.debug.push(format!("{label}: {e}"));
        }
    });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start {name}: {e}");
    }
}

/// A call from a bot thread waiting to run on the event loop.
pub struct TuruFlashCall {
    tab: TabId,
    name: String,
    args: Vec<ExternalValue>,
    reply: SyncSender<Option<ExternalValue>>,
}

impl TuruFlashCall {
    /// The tab whose player must answer.
    pub fn tab(&self) -> TabId {
        self.tab
    }
}

/// Runs a bot call on the event loop thread. `player` is `None` when no movie is open.
pub fn answer_flash_call(call: TuruFlashCall, player: Option<&mut Player>) {
    let result = player.map(|player| player.call_internal_interface(&call.name, call.args));
    let _ = call.reply.send(result);
}

pub struct DesktopFlashHost {
    tab: TabId,
    /// Hands a call to the thread that runs the player; false when it's gone.
    send: Box<dyn Fn(TuruFlashCall) -> bool + Send + Sync>,
}

impl DesktopFlashHost {
    pub fn new(event_loop: EventLoopProxy<RuffleEvent>, tab: TabId) -> Self {
        let event_loop = Mutex::new(event_loop);
        Self {
            tab,
            send: Box::new(move |call| {
                event_loop
                    .lock()
                    .is_ok_and(|e| e.send_event(RuffleEvent::TuruFlashCall(call)).is_ok())
            }),
        }
    }

    /// For the windowless loop.
    pub fn headless(sender: std::sync::mpsc::Sender<crate::headless::HeadlessEvent>) -> Self {
        Self {
            tab: active_tab(),
            send: Box::new(move |call| sender.send(crate::headless::HeadlessEvent::TuruFlashCall(call)).is_ok()),
        }
    }
}

impl FlashHost for DesktopFlashHost {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        if MAIN_THREAD.get() == Some(&std::thread::current().id()) {
            tracing::error!("Flash call {name} from the UI thread; use turu::spawn_action");
            return Err(HostError::Closed);
        }
        let (reply, answer) = sync_channel(1);
        let call = TuruFlashCall {
            tab: self.tab,
            name: name.to_owned(),
            args: args.into_iter().map(to_external).collect(),
            reply,
        };
        if !(self.send)(call) {
            return Err(HostError::Closed);
        }
        match answer.recv_timeout(CALL_TIMEOUT) {
            Ok(Some(value)) => Ok(from_external(value)),
            Ok(None) => Err(HostError::NoPlayer),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(HostError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(HostError::Closed),
        }
    }
}

fn to_external(arg: FlashArg) -> ExternalValue {
    match arg {
        FlashArg::Null => ExternalValue::Null,
        FlashArg::Bool(b) => ExternalValue::Bool(b),
        FlashArg::Number(n) => ExternalValue::Number(n),
        FlashArg::String(s) => ExternalValue::String(s),
        FlashArg::List(items) => ExternalValue::List(items.into_iter().map(to_external).collect()),
        FlashArg::Object(map) => ExternalValue::Object(map.into_iter().map(|(k, v)| (k, to_external(v))).collect()),
    }
}

/// skua.swf answers with strings; anything else is stringified like the OCX did.
fn from_external(value: ExternalValue) -> String {
    match value {
        ExternalValue::String(s) => s,
        ExternalValue::Bool(b) => b.to_string(),
        ExternalValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                (n as i64).to_string()
            } else {
                n.to_string()
            }
        }
        ExternalValue::Undefined | ExternalValue::Null => "null".into(),
        other => to_json(other).to_string(),
    }
}

fn to_json(value: ExternalValue) -> serde_json::Value {
    match value {
        ExternalValue::Undefined | ExternalValue::Null => serde_json::Value::Null,
        ExternalValue::Bool(b) => b.into(),
        ExternalValue::Number(n) => serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        ExternalValue::String(s) => s.into(),
        ExternalValue::List(items) => items.into_iter().map(to_json).collect(),
        ExternalValue::Object(map) => map
            .into_iter()
            .map(|(k, v)| (k, to_json(v)))
            .collect::<serde_json::Map<_, _>>()
            .into(),
    }
}

/// Receives a tab's `skua.swf`'s calls to the host.
pub struct TuruExternalInterface {
    pub bot: Arc<Bot>,
}

impl ExternalInterfaceProvider for TuruExternalInterface {
    fn call_method(&self, _context: &mut UpdateContext<'_>, name: &str, args: &[ExternalValue]) -> ExternalValue {
        let args = args.iter().cloned().map(to_json).collect();
        self.bot.on_flash_call(name, args);
        ExternalValue::Undefined
    }

    fn on_callback_available(&self, _name: &str) {}

    fn get_id(&self) -> Option<String> {
        Some("turu".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_monitor_flags() {
        let mut args: Vec<OsString> = ["turu", "--monitor", "--report-to", "http://pc:8790", "x.swf"]
            .into_iter()
            .map(Into::into)
            .collect();
        let flags = parse_startup_args(&mut args);
        assert_eq!(args, ["turu", "x.swf"]);
        assert_eq!(flags.monitor.as_deref(), Some(""));
        assert_eq!(flags.report_to.as_deref(), Some("http://pc:8790"));
        let mut args: Vec<OsString> = ["turu", "--monitor=0.0.0.0:9000"].into_iter().map(Into::into).collect();
        assert_eq!(parse_startup_args(&mut args).monitor.as_deref(), Some("0.0.0.0:9000"));
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
        assert_eq!(api.as_deref(), Some("127.0.0.1:8791"));
        assert_eq!(startup.username.as_deref(), Some("Hero"));
        assert_eq!(startup.password.as_deref(), Some("a=b"));
        assert_eq!(startup.server.as_deref(), Some("Artix"));
        assert_eq!(startup.script, Some("farm/GoldFarm.ts".into()));
        assert!(startup.run_script && startup.logs_in() && exit);
    }
}
