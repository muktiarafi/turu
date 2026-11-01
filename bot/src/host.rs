//! The boundary between the bot and the Flash player.
//!
//! The desktop frontend implements [`FlashHost`] by marshalling each call to the winit event
//! loop thread (where Ruffle's `Player` lives) and blocking until it answers.

use std::collections::BTreeMap;
use std::fmt;

/// An argument passed to one of `skua.swf`'s ExternalInterface callbacks.
#[derive(Debug, Clone, PartialEq)]
pub enum FlashArg {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    /// An AS3 `Array` (VibeSkua passes a .NET array or list as one).
    List(Vec<FlashArg>),
    /// An AS3 `Object` (VibeSkua passes any other .NET object, e.g. `new ExpandoObject()`, as one).
    Object(BTreeMap<String, FlashArg>),
}

impl From<&str> for FlashArg {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for FlashArg {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&String> for FlashArg {
    fn from(value: &String) -> Self {
        Self::String(value.clone())
    }
}

impl From<bool> for FlashArg {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

macro_rules! number_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for FlashArg {
            fn from(value: $t) -> Self {
                Self::Number(value as f64)
            }
        }
    )*};
}
number_arg!(i32, i64, u32, u64, usize, f32, f64);

impl From<serde_json::Value> for FlashArg {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => Self::Number(n.as_f64().unwrap_or_default()),
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(items) => {
                Self::List(items.into_iter().map(Self::from).collect())
            }
            serde_json::Value::Object(map) => {
                Self::Object(map.into_iter().map(|(k, v)| (k, Self::from(v))).collect())
            }
        }
    }
}

/// Why a call into Flash failed.
#[derive(Debug, Clone)]
pub enum HostError {
    /// No player is running (not started yet, or it was closed).
    NoPlayer,
    /// The event loop is gone (the app is quitting).
    Closed,
    /// The player didn't answer in time.
    Timeout,
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPlayer => write!(f, "the game isn't running"),
            Self::Closed => write!(f, "the app is closing"),
            Self::Timeout => write!(f, "the game didn't answer in time"),
        }
    }
}

/// Calls an ExternalInterface callback registered by `skua.swf`.
///
/// Implementations must be callable from any thread except the one that runs the player,
/// and return the callback's result as a string (`ExternalInterface` results other than
/// strings are converted: booleans to `"true"`/`"false"`, null/undefined to `"null"`).
pub trait FlashHost: Send + Sync {
    fn call(&self, name: &str, args: Vec<FlashArg>) -> Result<String, HostError>;

    /// A SWF the game has loaded from a URL whose path ends in `path` (a map file such as
    /// `ChiralValley/town-Mobius-21Feb14.swf`), as an uncompressed SWF file; `None` when
    /// none is loaded or the host can't tell.
    fn loaded_swf(&self, _path: &str) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_arrays_and_objects_become_flash_lists_and_objects() {
        let arg = FlashArg::from(json!({"a": [1, "x"], "b": {}}));
        let expected = FlashArg::Object(BTreeMap::from([
            (
                "a".to_owned(),
                FlashArg::List(vec![FlashArg::Number(1.0), FlashArg::String("x".into())]),
            ),
            ("b".to_owned(), FlashArg::Object(BTreeMap::new())),
        ]));
        assert_eq!(arg, expected);
    }
}
