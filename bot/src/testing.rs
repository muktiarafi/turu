//! For fake games in tests: answers turu.swf's batched reads (`snapshot`, `getMany`,
//! `selectArrayObjectsWhere`, docs/HOST_SWF_PROPOSAL.md M4) from the single reads a fake already
//! answers, as turu.swf would from the game. A fake's `call` starts with
//!
//! ```ignore
//! if let Some(answer) = turu_bot::testing::batched(name, &args, |n, a| self.call(n, a).unwrap_or_default()) {
//!     return Ok(answer);
//! }
//! ```

use crate::flash::parse_bool;
use crate::host::FlashArg;
use serde_json::{Map, Value, json};

/// The answer to `name` when it's one of turu.swf's batched reads, built from `single`'s
/// answers to the reads it replaces; `None` for any other call.
pub fn batched(
    name: &str,
    args: &[FlashArg],
    single: impl Fn(&str, Vec<FlashArg>) -> String,
) -> Option<String> {
    let get = |path: &str| -> Value { parse(&single("getGameObject", vec![path.into()])) };
    Some(match name {
        "snapshot" => {
            let with_target = matches!(args.first(), Some(FlashArg::Bool(true)));
            let avatar = !parse_bool(&single("isNull", vec!["world.myAvatar".into()]));
            let num = |v: Value| crate::api::packets::num(Some(&v)).unwrap_or(0);
            let loaded = num(get("world.myAvatar.items.length")) > 0
                && get("world.mapLoadInProgress") == Value::Bool(false)
                && parse(&single(
                    "callGameFunction0",
                    vec!["world.myAvatar.pMC.artLoaded".into()],
                )) == Value::Bool(true);
            let target = if with_target {
                parse(&single("getTargetMonster", vec![]))
            } else {
                Value::Null
            };
            json!({
                "loggedIn": parse_bool(&single("isLoggedIn", vec![])),
                "world": !parse_bool(&single("isNull", vec!["world".into()])),
                "avatar": avatar,
                "state": if avatar { num(get("world.myAvatar.dataLeaf.intState")) } else { 0 },
                "loaded": loaded,
                "rank": get("world.myAvatar.objData.iRank"),
                "target": target,
            })
            .to_string()
        }
        "getMany" => {
            let Some(FlashArg::Object(paths)) = args.first() else {
                return Some("{}".into());
            };
            let out: Map<String, Value> = paths
                .iter()
                .map(|(key, path)| match path {
                    FlashArg::String(path) => (key.clone(), get(path)),
                    _ => (key.clone(), Value::Null),
                })
                .collect();
            Value::Object(out).to_string()
        }
        "selectArrayObjectsWhere" => {
            let (Some(FlashArg::String(path)), Some(FlashArg::Object(filter))) =
                (args.first(), args.get(1))
            else {
                return Some(String::new());
            };
            let Value::Array(items) = get(path) else {
                return Some(String::new());
            };
            let picked: Vec<Value> = items
                .into_iter()
                .filter(|item| {
                    filter
                        .iter()
                        .all(|(key, want)| loosely(item.get(key).unwrap_or(&Value::Null), want))
                })
                .collect();
            Value::Array(picked).to_string()
        }
        _ => return None,
    })
}

fn parse(raw: &str) -> Value {
    serde_json::from_str(raw.trim()).unwrap_or(Value::Null)
}

/// turu.swf's matching: strings trimmed and ignoring case, numbers by value, the game's
/// booleans.
fn loosely(have: &Value, want: &FlashArg) -> bool {
    match want {
        FlashArg::Bool(want) => {
            let truthy = match have {
                Value::Bool(b) => *b,
                Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                Value::String(s) => matches!(s.trim().to_lowercase().as_str(), "1" | "true"),
                _ => false,
            };
            truthy == *want
        }
        FlashArg::Number(want) => match have {
            Value::Number(n) => n.as_f64() == Some(*want),
            Value::String(s) => s.trim().parse::<f64>().ok() == Some(*want),
            _ => false,
        },
        FlashArg::String(want) => {
            let have = match have {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            loose_name(&have) == loose_name(want)
        }
        _ => false,
    }
}

fn loose_name(text: &str) -> String {
    crate::models::game_name(text.trim()).to_lowercase()
}
