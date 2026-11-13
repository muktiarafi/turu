//! Typed access to `turu.swf`'s ExternalInterface callbacks (VibeSkua: `IFlashUtil`).
//!
//! Paths are dotted and rooted at the game (`world.myAvatar.objData.intGold`); `get_static`
//! roots them at the game's class (`loginInfo.strUsername`). See docs/VIBESKUA_SPEC.md, section 2.

use crate::control::ScriptControl;
use crate::error::{BotError, BotResult};
use crate::host::{FlashArg, FlashHost};
use crate::perf::PerfStats;
use crate::repeat_log::RepeatLog;
use parking_lot::{Mutex, RwLock};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

pub struct Flash {
    host: RwLock<Option<Arc<dyn FlashHost>>>,
    /// What turu.swf says of itself; `None` before it's asked.
    info: RwLock<Option<Arc<HostInfo>>>,
    errors: Mutex<RepeatLog>,
    control: Arc<ScriptControl>,
    perf: Arc<PerfStats>,
}

/// turu.swf's `hostInfo()` (docs/HOST_SWF_PROPOSAL.md).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct HostInfo {
    /// `turu`.
    pub name: String,
    /// Raised when a callback changes what it answers.
    pub version: u32,
    /// What it can do (`errors`, `packetCapture`, `events`, ...).
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

    /// The player reports the game's packets itself ([`FlashHost::player_packets`]).
    pub fn player_packets(&self) -> bool {
        self.host
            .read()
            .as_ref()
            .is_some_and(|h| h.player_packets())
    }

    /// What turu.swf said of itself when the client was loaded; `None` before, or for a host
    /// without `hostInfo` (the tests' fake games).
    pub fn host_info(&self) -> Option<Arc<HostInfo>> {
        self.info.read().clone()
    }

    /// Asks the host SWF what it is (`hostInfo`) and keeps the answer. A host without it (the
    /// tests' fake games) answers nothing: `None`, and no call is refused.
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

    /// `selectArrayObjectsWhere(path, filter)`: the whole items whose fields match every one of
    /// `filter`'s, loosely (strings trimmed and ignoring case, numbers by value, the game's
    /// booleans): check each again. Empty when `path` isn't an array.
    pub fn select_where<T: DeserializeOwned>(
        &self,
        path: &str,
        filter: BTreeMap<String, FlashArg>,
    ) -> BotResult<Vec<T>> {
        let raw = self.call(
            "selectArrayObjectsWhere",
            vec![path.into(), FlashArg::Object(filter)],
        )?;
        Ok(serde_json::from_str::<Option<Vec<T>>>(unquote_empty(&raw))
            .ok()
            .flatten()
            .unwrap_or_default())
    }

    /// `getMany({key: path})`: several values in one call. A key whose path doesn't resolve
    /// (or holds what JSON can't write) is `null`, or missing.
    pub fn get_many(&self, paths: &[(&str, &str)]) -> BotResult<serde_json::Map<String, Value>> {
        let paths = paths
            .iter()
            .map(|(key, path)| ((*key).to_owned(), FlashArg::from(*path)))
            .collect();
        let raw = self.call("getMany", vec![FlashArg::Object(paths)])?;
        Ok(
            serde_json::from_str::<Option<serde_json::Map<String, Value>>>(unquote_empty(&raw))
                .ok()
                .flatten()
                .unwrap_or_default(),
        )
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
