//! Shared by the fake games.
#![allow(dead_code)]

use serde_json::Value;

/// What skua.swf's `getGameObject` gives for a path inside a value a fake keeps whole
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
