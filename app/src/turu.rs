//! Turu: connects the bot engine (`turu_bot`) to the player.
//!
//! * [`TuruExternalInterface`] receives `skua.swf`'s `ExternalInterface.call(...)`s.
//! * [`DesktopFlashHost`] lets bot threads call `skua.swf`'s callbacks: each call is sent to
//!   the event loop as [`RuffleEvent::TuruFlashCall`] (without a window, to the `headless`
//!   loop) and answered by [`answer_flash_call`].
//!
//! See docs/turu/ARCHITECTURE.md.

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
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long a bot thread waits for the player to answer one call.
const CALL_TIMEOUT: Duration = Duration::from_secs(20);

static BOT: OnceLock<Arc<Bot>> = OnceLock::new();
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();
static HEADLESS: AtomicBool = AtomicBool::new(false);
/// `--api`'s address, until `start_api`.
static API: Mutex<Option<String>> = Mutex::new(None);

/// Ruffle's SharedObjects folder (`--save-directory`): what "Clear Flash Cache" empties.
static SAVE_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

/// The bot of this client (one per process, like VibeSkua).
pub fn bot() -> &'static Arc<Bot> {
    BOT.get_or_init(Bot::new)
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
    let Some(bot) = BOT.get() else { return title };
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
/// * `--api` (or `--api=<address>`, default `127.0.0.1:8791`): the control API (see
///   `turu_bot::control_api`), started by [`start_api`].
pub fn take_startup_args(args: &mut Vec<OsString>) {
    let Flags {
        mut startup,
        exit_on_stop,
        headless,
        api,
    } = parse_startup_args(args);
    HEADLESS.store(headless, Ordering::Relaxed);
    if let Ok(mut slot) = API.lock() {
        *slot = api;
    }
    if startup.password.is_none() {
        startup.password = std::env::var("TURU_PASSWORD").ok().filter(|p| !p.is_empty());
    }
    if startup.username.is_some() && !startup.logs_in() {
        tracing::warn!("--user without --password (or TURU_PASSWORD): log in yourself");
    }
    turu_bot::startup::configure(bot(), startup);
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
    match token.and_then(|token| turu_bot::control_api::start(bot(), addr, token)) {
        Ok(local) => {
            tracing::info!("Control API on http://{local}");
            if !local.ip().is_loopback() {
                tracing::warn!("The control API is reachable from other computers: keep the token secret");
            }
        }
        Err(e) => tracing::error!("Control API: {e}"),
    }
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
    headless: bool,
    /// `--api`'s address.
    api: Option<String>,
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
            // The address only inline (`--api=...`): the next argument may be anything.
            "--api" => flags.api = Some(inline.unwrap_or_else(|| "127.0.0.1:8791".into())),
            _ => rest.push(arg),
        }
    }
    *args = rest;
    flags
}

/// VibeSkua `--exit-on-script-stop`: 2 s after the script stops, log out; 1 s later, quit.
fn exit_on_script_stop() {
    bot().events.subscribe(|event| {
        if !matches!(event, BotEvent::ScriptStopped) || bot().servers.is_relogging() {
            return;
        }
        let _ = std::thread::Builder::new().name("Turu Exit".into()).spawn(|| {
            std::thread::sleep(Duration::from_secs(2));
            let _ = turu_bot::api::servers::logout(bot());
            std::thread::sleep(Duration::from_secs(1));
            std::process::exit(0);
        });
    });
}

/// Runs a bot action off the UI thread (bot calls block until the player answers, and the
/// player is driven by the UI thread).
pub fn spawn_action(name: &str, action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static) {
    let label = name.to_owned();
    let spawned = std::thread::Builder::new().name(format!("Turu {name}")).spawn(move || {
        let bot = bot();
        if let Err(e) = action(bot) {
            bot.logs.debug.push(format!("{label}: {e}"));
        }
    });
    if let Err(e) = spawned {
        tracing::error!("Couldn't start {name}: {e}");
    }
}

/// A call from a bot thread waiting to run on the event loop.
pub struct TuruFlashCall {
    name: String,
    args: Vec<ExternalValue>,
    reply: SyncSender<Option<ExternalValue>>,
}

/// Runs a bot call on the event loop thread. `player` is `None` when no movie is open.
pub fn answer_flash_call(call: TuruFlashCall, player: Option<&mut Player>) {
    let result = player.map(|player| player.call_internal_interface(&call.name, call.args));
    let _ = call.reply.send(result);
}

pub struct DesktopFlashHost {
    /// Hands a call to the thread that runs the player; false when it's gone.
    send: Box<dyn Fn(TuruFlashCall) -> bool + Send + Sync>,
}

impl DesktopFlashHost {
    pub fn new(event_loop: EventLoopProxy<RuffleEvent>) -> Self {
        let event_loop = Mutex::new(event_loop);
        Self {
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

/// Receives `skua.swf`'s calls to the host.
pub struct TuruExternalInterface;

impl ExternalInterfaceProvider for TuruExternalInterface {
    fn call_method(&self, _context: &mut UpdateContext<'_>, name: &str, args: &[ExternalValue]) -> ExternalValue {
        let args = args.iter().cloned().map(to_json).collect();
        bot().on_flash_call(name, args);
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
