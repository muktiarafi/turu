//! Turu: connects the bot engine (`turu_bot`) to the player.
//!
//! * [`TuruExternalInterface`] receives `skua.swf`'s `ExternalInterface.call(...)`s.
//! * [`DesktopFlashHost`] lets bot threads call `skua.swf`'s callbacks: each call is sent to
//!   the event loop as [`RuffleEvent::TuruFlashCall`] and answered by [`answer_flash_call`].
//!
//! See docs/turu/ARCHITECTURE.md.

use crate::custom_event::RuffleEvent;
use ruffle_core::Player;
use ruffle_core::context::UpdateContext;
use ruffle_core::external::{ExternalInterfaceProvider, Value as ExternalValue};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::Duration;
use turu_bot::{Bot, FlashArg, FlashHost, HostError};
use winit::event_loop::EventLoopProxy;

pub const APP_NAME: &str = "Turu";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How long a bot thread waits for the player to answer one call.
const CALL_TIMEOUT: Duration = Duration::from_secs(20);

static BOT: OnceLock<Arc<Bot>> = OnceLock::new();
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// The bot of this client (one per process, like VibeSkua).
pub fn bot() -> &'static Arc<Bot> {
    BOT.get_or_init(Bot::new)
}

/// Remembers the event loop thread: calls from it would wait for themselves.
pub fn init_main_thread() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
}

/// The URL Turu starts with: VibeSkua's `skua.swf`, written to the data directory.
pub fn skua_swf_url() -> anyhow::Result<String> {
    let path = bot().paths.write_skua_swf()?;
    let url = url::Url::from_file_path(&path)
        .map_err(|()| anyhow::anyhow!("bad path for skua.swf: {}", path.display()))?;
    Ok(url.to_string())
}

/// Runs a bot action off the UI thread (bot calls block until the player answers, and the
/// player is driven by the UI thread).
pub fn spawn_action(name: &str, action: impl FnOnce(&Arc<Bot>) -> turu_bot::BotResult<()> + Send + 'static) {
    let label = name.to_owned();
    let spawned = std::thread::Builder::new()
        .name(format!("Turu {name}"))
        .spawn(move || {
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
    event_loop: Mutex<EventLoopProxy<RuffleEvent>>,
}

impl DesktopFlashHost {
    pub fn new(event_loop: EventLoopProxy<RuffleEvent>) -> Self {
        Self {
            event_loop: Mutex::new(event_loop),
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
        self.event_loop
            .lock()
            .map_err(|_| HostError::Closed)?
            .send_event(RuffleEvent::TuruFlashCall(call))
            .map_err(|_| HostError::Closed)?;
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
