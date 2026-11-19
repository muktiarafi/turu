//! Shared by the fake games.
#![allow(dead_code)]

use serde_json::Value;
use std::path::PathBuf;
use std::sync::Once;

/// The data folder (`TURU_DATA_DIR`) of every bot in this process, emptied the first time:
/// one per process, since `cargo test` runs every module's tests in one (cargo-nextest, each
/// test in its own). Call it before making a bot.
pub fn data_dir() -> PathBuf {
    static INIT: Once = Once::new();
    let dir = std::env::temp_dir().join(format!("turu-test-data-{}", std::process::id()));
    INIT.call_once(|| {
        let _ = std::fs::remove_dir_all(&dir);
        // SAFETY: set once, here only, before any bot of this process reads it.
        unsafe { std::env::set_var("TURU_DATA_DIR", &dir) };
    });
    dir
}

/// What turu.swf's `getGameObject` gives for a path inside a value a fake keeps whole
/// (`world.questTree.1234`, `world.myAvatar.items.length`): `bases` are such paths and their
/// JSON. `None` when `path` isn't below one of them.
pub fn sub_path(path: &str, bases: &[(&str, String)]) -> Option<String> {
    let (rest, raw) = bases.iter().find_map(|(base, raw)| {
        let rest = path.strip_prefix(base)?.strip_prefix('.')?;
        Some((rest, raw))
    })?;
    let mut value: Value = serde_json::from_str(raw).ok()?;
    for segment in rest.split('.') {
        value = match (&value, segment) {
            (Value::Array(items), "length") => Value::from(items.len()),
            (Value::Array(items), index) => index
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get(i).cloned())
                .unwrap_or(Value::Null),
            (Value::Object(fields), key) => fields.get(key).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        };
    }
    Some(value.to_string())
}
