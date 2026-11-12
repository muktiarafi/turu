//! Typed access to `skua.swf`'s ExternalInterface callbacks (VibeSkua: `IFlashUtil`).
//!
//! Paths are dotted and rooted at the game (`world.myAvatar.objData.intGold`); `get_static`
//! roots them at the game's class (`loginInfo.strUsername`). See docs/VIBESKUA_SPEC.md, section 2.

use crate::control::ScriptControl;
use crate::error::{BotError, BotResult};
use crate::host::{FlashArg, FlashHost};
use crate::perf::PerfStats;
use parking_lot::{Mutex, RwLock};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Flash {
    host: RwLock<Option<Arc<dyn FlashHost>>>,
    /// What the host SWF says of itself; `None` for VibeSkua's, or before it's asked.
    info: RwLock<Option<Arc<HostInfo>>>,
    errors: Mutex<ErrorLog>,
    control: Arc<ScriptControl>,
    perf: Arc<PerfStats>,
}

/// A host SWF's `hostInfo()` (Turu's, docs/HOST_SWF_PROPOSAL.md); VibeSkua's has none.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct HostInfo {
    /// `turu`.
    pub name: String,
    /// Raised when a callback changes what it answers.
    pub version: u32,
    /// What it does beyond VibeSkua's SWF (`strictErrors`, `packetEcho`, ...).
    #[serde(default)]
    pub features: BTreeSet<String>,
    /// Every callback it has: [`Flash::call`] refuses others.
    #[serde(default)]
    pub callbacks: BTreeSet<String>,
}

impl HostInfo {
    pub fn has(&self, feature: &str) -> bool {
        self.features.contains(feature)
    }
}

/// A callback a debug build may call though `hostInfo` doesn't list it (`turu.swf`'s
/// `throwForTest`, to check error reports by hand).
const UNLISTED_DEBUG_CALLBACKS: &[&str] = &["throwForTest"];

/// How often the same host SWF error is logged.
const ERROR_REPEAT: Duration = Duration::from_secs(60);
/// Errors remembered at most; the oldest are forgotten past it.
const ERRORS_KEPT: usize = 256;

/// When each host SWF error was last logged, and how often it came since.
#[derive(Default)]
struct ErrorLog {
    seen: HashMap<String, (Instant, u32)>,
}

impl ErrorLog {
    /// `Some(n)` when `message` should be logged now, `n` being the times it came since it last
    /// was; `None` while it was logged less than [`ERROR_REPEAT`] ago.
    fn note(&mut self, message: &str, now: Instant) -> Option<u32> {
        if let Some((logged, since)) = self.seen.get_mut(message) {
            if now.duration_since(*logged) < ERROR_REPEAT {
                *since += 1;
                return None;
            }
            let repeats = *since;
            *logged = now;
            *since = 0;
            return Some(repeats);
        }
        if self.seen.len() >= ERRORS_KEPT {
            self.seen
                .retain(|_, (logged, _)| now.duration_since(*logged) < ERROR_REPEAT);
            if self.seen.len() >= ERRORS_KEPT {
                self.seen.clear();
            }
        }
        self.seen.insert(message.to_owned(), (now, 0));
        Some(0)
    }
}

impl Flash {
    pub(crate) fn new(control: Arc<ScriptControl>) -> Self {
        Self {
            host: RwLock::new(None),
            info: RwLock::new(None),
            errors: Mutex::default(),
            control,
            perf: Arc::default(),
        }
    }

    /// The calls made and the player's ticks (see [`crate::perf`]).
    pub fn perf(&self) -> &Arc<PerfStats> {
        &self.perf
    }

    pub fn set_host(&self, host: Option<Arc<dyn FlashHost>>) {
        *self.host.write() = host;
    }

    /// [`FlashHost::loaded_swf`]; `None` without a host.
    pub fn loaded_swf(&self, path: &str) -> Option<Vec<u8>> {
        let host = self.host.read().clone();
        host?.loaded_swf(path)
    }

    pub fn has_host(&self) -> bool {
        self.host.read().is_some()
    }

    /// What the host SWF said of itself when the client was loaded; `None` for VibeSkua's.
    pub fn host_info(&self) -> Option<Arc<HostInfo>> {
        self.info.read().clone()
    }

    /// Asks the host SWF what it is (`hostInfo`) and keeps the answer. VibeSkua's has no such
    /// callback: its empty answer means `None`, and every call goes through as before.
    pub(crate) fn ask_host_info(&self) -> Option<Arc<HostInfo>> {
        let info = self
            .call_host("hostInfo", vec![])
            .ok()
            .and_then(|raw| serde_json::from_str::<Option<HostInfo>>(unquote_empty(&raw)).ok())
            .flatten()
            .map(Arc::new);
        *self.info.write() = info.clone();
        info
    }

    /// The host SWF is starting again (`requestLoadGame`): it is asked anew.
    pub(crate) fn forget_host_info(&self) {
        *self.info.write() = None;
    }

    /// An `error(message, stack)` from the host SWF, as a line for the debug log: `None` when
    /// the same message was logged less than a minute ago.
    pub(crate) fn host_error_line(&self, message: &str, stack: &str) -> Option<String> {
        let repeats = self.errors.lock().note(message, Instant::now())?;
        let mut line = format!("[host swf] {message}");
        if repeats > 0 {
            line.push_str(&format!(" (and {repeats} more since it was last logged)"));
        }
        let stack = stack.trim();
        if !stack.is_empty() {
            line.push('\n');
            line.push_str(stack);
        }
        Some(line)
    }

    /// Calls a callback and returns its raw string result.
    ///
    /// On the script thread this fails with [`BotError::Stopped`] once Stop was pressed.
    /// With Turu's host SWF, a name its `hostInfo` doesn't list fails with
    /// [`BotError::NotInHostSwf`], without calling Flash.
    pub fn call(&self, name: &str, args: Vec<FlashArg>) -> BotResult<String> {
        if let Some(info) = self.info.read().as_deref()
            && !info.callbacks.contains(name)
            && !(cfg!(debug_assertions) && UNLISTED_DEBUG_CALLBACKS.contains(&name))
        {
            self.control.check()?;
            return Err(BotError::NotInHostSwf(name.to_owned()));
        }
        self.call_host(name, args)
    }

    fn call_host(&self, name: &str, args: Vec<FlashArg>) -> BotResult<String> {
        self.control.check()?;
        let host = self.host.read().clone();
        let Some(host) = host else {
            return Err(BotError::Host(crate::host::HostError::NoPlayer));
        };
        let path = match args.first() {
            Some(FlashArg::String(path)) if crate::perf::takes_path(name) => Some(path.clone()),
            _ => None,
        };
        let asked = Instant::now();
        let result = host.call_timed(name, args);
        let (run, reply) = match &result {
            Ok((reply, run)) => (*run, Some(reply.len())),
            Err(_) => (None, None),
        };
        self.perf
            .record_call(name, path.as_deref(), asked.elapsed(), run, reply);
        let result = result.map(|(reply, _)| reply).map_err(BotError::from);
        if let Err(e) = &result {
            tracing::debug!("Flash call {name} failed: {e}");
        }
        result
    }

    pub fn call_str(&self, name: &str, args: Vec<FlashArg>) -> BotResult<String> {
        self.call(name, args)
    }

    pub fn call_bool(&self, name: &str, args: Vec<FlashArg>) -> BotResult<bool> {
        Ok(parse_bool(&self.call(name, args)?))
    }

    /// Calls a callback that returns JSON and deserializes it.
    pub fn call_json<T: DeserializeOwned>(&self, name: &str, args: Vec<FlashArg>) -> BotResult<T> {
        let raw = self.call(name, args)?;
        parse_json(name, &raw)
    }

    /// `getGameObject(path)`: a value of the game, e.g. `world.strMapName`.
    pub fn get<T: DeserializeOwned>(&self, path: &str) -> BotResult<T> {
        let raw = self.call("getGameObject", vec![path.into()])?;
        parse_json(path, &raw)
    }

    /// Like [`Flash::get`], but `None` when the value is missing or doesn't fit `T`.
    pub fn get_opt<T: DeserializeOwned>(&self, path: &str) -> BotResult<Option<T>> {
        let raw = self.call("getGameObject", vec![path.into()])?;
        Ok(serde_json::from_str::<Option<T>>(unquote_empty(&raw))
            .ok()
            .flatten())
    }

    /// Like [`Flash::get`], but `default` when the value is missing or doesn't fit `T`.
    pub fn get_or<T: DeserializeOwned>(&self, path: &str, default: T) -> BotResult<T> {
        Ok(self.get_opt(path)?.unwrap_or(default))
    }

    /// `getGameObjectS(path)`: a static value of the game's class, e.g. `serverIP`.
    pub fn get_static<T: DeserializeOwned>(&self, path: &str) -> BotResult<Option<T>> {
        let raw = self.call("getGameObjectS", vec![path.into()])?;
        Ok(serde_json::from_str::<Option<T>>(unquote_empty(&raw))
            .ok()
            .flatten())
    }

    /// `getGameObjectKey(path, key)`: `obj[key]`, for keys that aren't identifiers.
    pub fn get_key<T: DeserializeOwned>(&self, path: &str, key: &str) -> BotResult<Option<T>> {
        let raw = self.call("getGameObjectKey", vec![path.into(), key.into()])?;
        Ok(serde_json::from_str::<Option<T>>(unquote_empty(&raw))
            .ok()
            .flatten())
    }

    /// `setGameObject(path, value)`.
    pub fn set(&self, path: &str, value: impl Into<FlashArg>) -> BotResult<()> {
        self.call("setGameObject", vec![path.into(), value.into()])?;
        Ok(())
    }

    /// `setGameObjectKey(path, key, value)`.
    pub fn set_key(&self, path: &str, key: &str, value: impl Into<FlashArg>) -> BotResult<()> {
        self.call(
            "setGameObjectKey",
            vec![path.into(), key.into(), value.into()],
        )?;
        Ok(())
    }

    /// `isNull(path)`: true when the value is null or the path doesn't resolve.
    pub fn is_null(&self, path: &str) -> BotResult<bool> {
        self.call_bool("isNull", vec![path.into()])
    }

    /// `callGameFunction(path, ...args)` / `callGameFunction0(path)`; the raw JSON result.
    pub fn call_game_function_raw(&self, path: &str, args: Vec<FlashArg>) -> BotResult<String> {
        if args.is_empty() {
            self.call("callGameFunction0", vec![path.into()])
        } else {
            let mut all = Vec::with_capacity(args.len() + 1);
            all.push(path.into());
            all.extend(args);
            self.call("callGameFunction", all)
        }
    }

    /// Calls a game function and ignores its result, e.g. `world.cancelTarget`.
    pub fn call_game_function(&self, path: &str, args: Vec<FlashArg>) -> BotResult<()> {
        self.call_game_function_raw(path, args).map(|_| ())
    }

    /// Calls a game function and deserializes its JSON result.
    pub fn call_game_function_json<T: DeserializeOwned>(
        &self,
        path: &str,
        args: Vec<FlashArg>,
    ) -> BotResult<Option<T>> {
        let raw = self.call_game_function_raw(path, args)?;
        Ok(serde_json::from_str::<Option<T>>(unquote_empty(&raw))
            .ok()
            .flatten())
    }

    /// `selectArrayObjects(path, selector)`: `arr.map(x => x[selector])`.
    pub fn select<T: DeserializeOwned>(&self, path: &str, selector: &str) -> BotResult<Vec<T>> {
        let raw = self.call("selectArrayObjects", vec![path.into(), selector.into()])?;
        Ok(serde_json::from_str::<Option<Vec<T>>>(unquote_empty(&raw))
            .ok()
            .flatten()
            .unwrap_or_default())
    }

    /// `getArrayObject(path, index)`.
    pub fn get_array_item<T: DeserializeOwned>(
        &self,
        path: &str,
        index: usize,
    ) -> BotResult<Option<T>> {
        let raw = self.call("getArrayObject", vec![path.into(), index.into()])?;
        Ok(serde_json::from_str::<Option<T>>(unquote_empty(&raw))
            .ok()
            .flatten())
    }

    /// Sends a raw packet to the server: `sfc.sendString("%xt%zm%...%")`.
    pub fn send_packet(&self, packet: &str) -> BotResult<()> {
        self.call_game_function("sfc.sendString", vec![packet.into()])
    }
}

/// `"true"`, `"True"` and `"\"true\""` are true; everything else is false.
pub fn parse_bool(raw: &str) -> bool {
    raw.trim().trim_matches('"').eq_ignore_ascii_case("true")
}

fn parse_json<T: DeserializeOwned>(what: &str, raw: &str) -> BotResult<T> {
    serde_json::from_str(unquote_empty(raw)).map_err(|e| BotError::Json {
        path: what.to_owned(),
        message: format!("{e} (got {})", truncate(raw, 120)),
    })
}

/// An empty answer (an `undefined` value stringified to nothing) reads as `null`.
fn unquote_empty(raw: &str) -> &str {
    let raw = raw.trim();
    if raw.is_empty() || raw == "undefined" {
        "null"
    } else {
        raw
    }
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_error_is_logged_again_after_a_minute_with_its_count() {
        let mut log = ErrorLog::default();
        let start = Instant::now();
        assert_eq!(log.note("a", start), Some(0));
        assert_eq!(log.note("a", start + Duration::from_secs(1)), None);
        assert_eq!(log.note("a", start + Duration::from_secs(30)), None);
        assert_eq!(log.note("b", start + Duration::from_secs(30)), Some(0));
        assert_eq!(log.note("a", start + Duration::from_secs(61)), Some(2));
        assert_eq!(log.note("a", start + Duration::from_secs(62)), None);
    }

    #[test]
    fn old_errors_are_forgotten_past_the_limit() {
        let mut log = ErrorLog::default();
        let start = Instant::now();
        for i in 0..ERRORS_KEPT {
            log.note(&i.to_string(), start);
        }
        let later = start + ERROR_REPEAT;
        assert_eq!(log.note("new", later), Some(0));
        assert_eq!(log.seen.len(), 1);
    }
}
