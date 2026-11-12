//! The bridge between a tab's bot and its Ruffle player (the `ruffle` feature): bot threads call
//! `skua.swf`'s callbacks through [`TuruFlashHost`], which hands each call to the thread that runs
//! the players, where [`answer_flash_call`] runs it; [`TuruExternalInterface`] receives
//! `skua.swf`'s `ExternalInterface.call(...)`s.

use super::{MAIN_THREAD, TabId};
use ruffle_core::Player;
use ruffle_core::context::UpdateContext;
use ruffle_core::external::{ExternalInterfaceProvider, Value as ExternalValue};
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::{Duration, Instant};
use turu_bot::{Bot, FlashArg, FlashHost, HostError};

/// How long a bot thread waits for the player to answer one call.
const CALL_TIMEOUT: Duration = Duration::from_secs(20);

/// A call from a bot thread waiting to run on the event loop.
pub struct TuruFlashCall {
    tab: TabId,
    request: Request,
}

enum Request {
    /// An ExternalInterface callback.
    Call {
        name: String,
        args: Vec<ExternalValue>,
        /// The answer and how long the player took to run it.
        reply: SyncSender<(Option<ExternalValue>, Duration)>,
    },
    /// A loaded movie whose URL path ends in `path` ([`FlashHost::loaded_swf`]).
    LoadedMovie {
        path: String,
        reply: SyncSender<Option<Arc<ruffle_core::tag_utils::SwfMovie>>>,
    },
}

impl TuruFlashCall {
    /// The tab whose player must answer.
    pub fn tab(&self) -> TabId {
        self.tab
    }
}

/// Runs a bot call on the event loop thread. `player` is `None` when no movie is open.
pub fn answer_flash_call(call: TuruFlashCall, player: Option<&mut Player>) {
    match call.request {
        Request::Call { name, args, reply } => {
            let started = Instant::now();
            let result = player.map(|player| player.call_internal_interface(&name, args));
            let _ = reply.send((result, started.elapsed()));
        }
        Request::LoadedMovie { path, reply } => {
            let suffix = format!("/{}", path.trim_start_matches('/'));
            let movie = player.and_then(|player| {
                player.loaded_movie(|url| {
                    url::Url::parse(url).is_ok_and(|url| url.path().ends_with(&suffix))
                })
            });
            let _ = reply.send(movie);
        }
    }
}

pub struct TuruFlashHost {
    tab: TabId,
    /// Hands a call to the thread that runs the player; false when it's gone.
    send: Box<dyn Fn(TuruFlashCall) -> bool + Send + Sync>,
}

impl TuruFlashHost {
    /// The tab's host; `send` hands a call to the thread that runs its player (false when it's
    /// gone).
    pub fn new(tab: TabId, send: impl Fn(TuruFlashCall) -> bool + Send + Sync + 'static) -> Self {
        Self {
            tab,
            send: Box::new(send),
        }
    }
}

impl FlashHost for TuruFlashHost {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.call_timed(name, args).map(|(reply, _)| reply)
    }

    fn call_timed(
        &self,
        name: &str,
        args: Vec<FlashArg>,
    ) -> Result<(String, Option<Duration>), HostError> {
        if MAIN_THREAD.get() == Some(&std::thread::current().id()) {
            tracing::error!("Flash call {name} from the UI thread; use turu::spawn_action");
            return Err(HostError::Closed);
        }
        let (reply, answer) = sync_channel(1);
        let request = Request::Call {
            name: name.to_owned(),
            args: args.into_iter().map(to_external).collect(),
            reply,
        };
        if !(self.send)(TuruFlashCall {
            tab: self.tab,
            request,
        }) {
            return Err(HostError::Closed);
        }
        match answer.recv_timeout(CALL_TIMEOUT) {
            Ok((Some(value), run)) => Ok((from_external(value), Some(run))),
            Ok((None, _)) => Err(HostError::NoPlayer),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(HostError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(HostError::Closed),
        }
    }

    /// The movie from the player's library, written back out as an uncompressed SWF here, off
    /// the event loop (Ruffle keeps its tags decompressed, without the header).
    fn loaded_swf(&self, path: &str) -> Option<Vec<u8>> {
        if MAIN_THREAD.get() == Some(&std::thread::current().id()) {
            return None;
        }
        let (reply, answer) = sync_channel(1);
        let request = Request::LoadedMovie {
            path: path.to_owned(),
            reply,
        };
        if !(self.send)(TuruFlashCall {
            tab: self.tab,
            request,
        }) {
            return None;
        }
        let movie = answer.recv_timeout(CALL_TIMEOUT).ok()??;
        crate::map_pads::swf_file(movie.header(), movie.data())
    }
}

fn to_external(arg: FlashArg) -> ExternalValue {
    match arg {
        FlashArg::Null => ExternalValue::Null,
        FlashArg::Bool(b) => ExternalValue::Bool(b),
        FlashArg::Number(n) => ExternalValue::Number(n),
        FlashArg::String(s) => ExternalValue::String(s),
        FlashArg::List(items) => ExternalValue::List(items.into_iter().map(to_external).collect()),
        FlashArg::Object(map) => {
            ExternalValue::Object(map.into_iter().map(|(k, v)| (k, to_external(v))).collect())
        }
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
    fn call_method(
        &self,
        _context: &mut UpdateContext<'_>,
        name: &str,
        args: &[ExternalValue],
    ) -> ExternalValue {
        let args = args.iter().cloned().map(to_json).collect();
        self.bot.on_flash_call(name, args);
        ExternalValue::Undefined
    }

    fn on_callback_available(&self, _name: &str) {}

    fn get_id(&self) -> Option<String> {
        Some("turu".into())
    }
}
