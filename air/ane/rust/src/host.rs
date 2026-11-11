//! [`AirFlashHost`]: the bot's calls into `skua.swf`, through AIR.
//!
//! A bot thread can't run AS3: AIR runs it on its own thread. So a call becomes a status event
//! ("call", with `{"id", "name", "args"}` as JSON) that Kotlin's `Bridge.dispatch` sends; the host
//! SWF runs the callback on AIR's thread and answers with the extension's `reply`, which lands in
//! [`answer`] here; the calling thread waits for it, as the Ruffle host waits on its event loop.

use jni::objects::{Global, JClass, JValue};
use jni::{Env, JavaVM, jni_sig, jni_str};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;
use std::time::Duration;
use turu_bot::{FlashArg, FlashHost, HostError};

/// How long a bot thread waits for AIR to answer one call (as the Ruffle host).
const CALL_TIMEOUT: Duration = Duration::from_secs(20);

/// The JVM and Kotlin's `Bridge` class: a native thread can't look the app's classes up itself.
pub(crate) static BRIDGE: OnceLock<(JavaVM, Global<JClass<'static>>)> = OnceLock::new();
/// AIR's thread, which runs the AS3: a call from it would wait for itself.
pub(crate) static AIR_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Calls waiting for AIR's answer: the answer and how long AS3 took to run the callback.
type Pending = HashMap<u64, SyncSender<(String, Duration)>>;
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn pending() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct AirFlashHost;

impl FlashHost for AirFlashHost {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError> {
        self.call_timed(name, args).map(|(reply, _)| reply)
    }

    fn call_timed(
        &self,
        name: &str,
        args: Vec<FlashArg>,
    ) -> Result<(String, Option<Duration>), HostError> {
        if AIR_THREAD.get() == Some(&std::thread::current().id()) {
            tracing::error!("Flash call {name} from AIR's thread; it would wait for itself");
            return Err(HostError::Closed);
        }
        let Some((vm, class)) = BRIDGE.get() else {
            return Err(HostError::NoPlayer);
        };
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = sync_channel(1);
        pending().get_or_insert_default().insert(id, reply);
        let request = serde_json::json!({
            "id": id,
            "name": name,
            "args": args.into_iter().map(to_json).collect::<Vec<_>>(),
        });
        let sent = crate::android::attach(vm, |env: &mut Env| {
            dispatch(env, class, "call", &request.to_string())
        });
        if let Err(e) = sent {
            pending().get_or_insert_default().remove(&id);
            tracing::warn!("Couldn't send {name} to AIR: {e}");
            return Err(HostError::Closed);
        }
        let result = answer.recv_timeout(CALL_TIMEOUT);
        pending().get_or_insert_default().remove(&id);
        match result {
            Ok((value, run)) => Ok((value, Some(run))),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(HostError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(HostError::Closed),
        }
    }
}

/// AIR's answer to call `id` (the callback's result as `ExternalInterface` would stringify it).
pub(crate) fn answer(id: u64, value: String, run: Duration) {
    if let Some(reply) = pending().get_or_insert_default().remove(&id) {
        let _ = reply.send((value, run));
    }
}

fn to_json(arg: FlashArg) -> Value {
    match arg {
        FlashArg::Null => Value::Null,
        FlashArg::Bool(b) => b.into(),
        FlashArg::Number(n) => serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        FlashArg::String(s) => s.into(),
        FlashArg::List(items) => items.into_iter().map(to_json).collect(),
        FlashArg::Object(map) => map
            .into_iter()
            .map(|(k, v)| (k, to_json(v)))
            .collect::<Map<_, _>>()
            .into(),
    }
}

/// `Bridge.dispatch(code, level)`: a status event to AS3.
pub(crate) fn dispatch(
    env: &mut Env,
    class: &JClass,
    code: &str,
    level: &str,
) -> jni::errors::Result<()> {
    let code = env.new_string(code)?;
    let level = env.new_string(level)?;
    env.call_static_method(
        class,
        jni_str!("dispatch"),
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
        &[JValue::Object(&code), JValue::Object(&level)],
    )?;
    Ok(())
}
