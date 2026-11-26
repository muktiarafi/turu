//! What a tape never holds: passwords, session tokens, the account's email and IP address, the
//! socket's login, and chat (other people's words). Taken out as each line is written, wherever it shows: a call's arguments
//! or answer, an event, a socket message (docs/REPLAY_TESTS_PROPOSAL.md, "Secrets and names").
//! Player names stay: the scrubber (phase 2) replaces them before a tape becomes a fixture.

use super::Line;
use serde_json::Value;

/// What a redacted value is replaced with. A replay matches it with anything.
pub const REDACTED: &str = "<redacted>";
/// What a chat message is replaced with.
pub const REDACTED_CHAT: &str = "<redacted chat>";
/// What the socket's login message is replaced with.
pub const REDACTED_LOGIN: &str = "<redacted login>";

/// The callbacks whose first argument is a game path (`getGameObject`, ...).
const PATH_CALLS: &[&str] = &[
    "getGameObject",
    "getGameObjectS",
    "getGameObjectKey",
    "setGameObject",
    "setGameObjectKey",
    "callGameFunction",
    "callGameFunction0",
];

/// Takes the secrets and chat out of a line.
pub fn line(line: &mut Line) {
    match line {
        Line::Call {
            name, args, answer, ..
        } => {
            call(name, args, answer);
            args.iter_mut().for_each(|a| {
                value(a);
            });
            if let Some(answer) = answer {
                text(answer);
            }
        }
        Line::Event { args, .. } => args.iter_mut().for_each(|a| {
            value(a);
        }),
        Line::Sent { text: t, .. } | Line::Received { text: t, .. } => {
            text(t);
        }
        Line::Note { detail, .. } => {
            value(detail);
        }
    }
}

/// The calls whose arguments or answer are a secret by what they are, not by how they look.
fn call(name: &str, args: &mut [Value], answer: &mut Option<String>) {
    let path = args
        .first()
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    // The game's `login(username, password)`: `servers.login`, relogins, `--user`.
    if name.starts_with("callGameFunction") && path == "login" {
        args.iter_mut().skip(1).for_each(|a| *a = REDACTED.into());
        return;
    }
    // `getGameObjectKey(path, key)` / `setGameObjectKey(path, key, value)`: the field is the key.
    let keyed = name.ends_with("Key");
    let field = match (keyed, args.get(1).and_then(Value::as_str)) {
        (true, Some(key)) => format!("{path}.{key}"),
        _ => path.clone(),
    };
    if PATH_CALLS.contains(&name) && is_secret_path(&field) {
        // A set's value, a read's answer (the path and key stay).
        let kept = if keyed { 2 } else { 1 };
        args.iter_mut()
            .skip(kept)
            .for_each(|a| *a = REDACTED.into());
        if answer.is_some() {
            *answer = Some(REDACTED.into());
        }
        return;
    }
    // `getMany({key: path})`: the answer is keyed the same way.
    if name == "getMany"
        && let Some(Value::Object(paths)) = args.first()
        && let Some(raw) = answer.as_mut()
        && let Ok(Value::Object(mut answers)) = serde_json::from_str::<Value>(raw)
    {
        let mut changed = false;
        for (key, path) in paths {
            if path.as_str().is_some_and(is_secret_path)
                && let Some(slot) = answers.get_mut(key)
            {
                *slot = REDACTED.into();
                changed = true;
            }
        }
        if changed {
            *raw = Value::Object(answers).to_string();
        }
    }
}

/// A game path whose value is a secret: under `loginInfo` (all but the username), or ending in a
/// password, token, email or IP address field (`objLogin.sToken`).
pub fn is_secret_path(path: &str) -> bool {
    if let Some(field) = path.strip_prefix("loginInfo.") {
        return field != "strUsername";
    }
    let last = path.rsplit('.').next().unwrap_or(path);
    is_secret_key(last)
}

/// A field name that holds a secret: `strPassword`, `sToken`, `pword`, `strEmail`, `strIP` (the
/// account's address, in `initUserData`; not `ip0`, ..., quest progress).
pub(crate) fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.ends_with("password")
        || key.ends_with("token")
        || key.ends_with("pword")
        || key == "pass"
        || key.ends_with("email")
        || key == "strip"
        || key == "ip"
        || key.ends_with("ipaddress")
}

/// A JSON value: its strings are redacted as text, its secret fields replaced. True when it
/// changed.
pub(crate) fn value(v: &mut Value) -> bool {
    match v {
        Value::String(s) => text(s),
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |changed, item| value(item) | changed),
        Value::Object(map) => {
            let mut changed = false;
            for (key, field) in map.iter_mut() {
                if is_secret_key(key) {
                    *field = REDACTED.into();
                    changed = true;
                } else {
                    changed |= value(field);
                }
            }
            changed
        }
        _ => false,
    }
}

/// A string from the game: a message, a packet, or JSON (a `pext`, an answer). True when it
/// changed.
pub fn text(s: &mut String) -> bool {
    if is_login_message(s) {
        *s = REDACTED_LOGIN.into();
    } else if is_chat(s) {
        *s = REDACTED_CHAT.into();
    } else if (s.starts_with('{') || s.starts_with('['))
        // Most answers are JSON (monsters, quests, the avatar): only one that may name a secret
        // field is parsed, which keeps the writer's cost down (a farm run writes ~40 MB of it an hour).
        && names_a_secret(s)
        && let Ok(mut json) = serde_json::from_str::<Value>(s)
        && value(&mut json)
    {
        *s = json.to_string();
    } else {
        return false;
    }
    true
}

/// Whether `s` may hold a field [`is_secret_key`] takes: it has "pass", "token", "pword",
/// "email", "strip" or `"ip"` in it, in any case.
fn names_a_secret(s: &str) -> bool {
    const HINTS: &[&[u8]] = &[b"pass", b"token", b"pword", b"email", b"strip", b"\"ip\""];
    HINTS.iter().any(|hint| {
        s.as_bytes()
            .windows(hint.len())
            .any(|w| w.eq_ignore_ascii_case(hint))
    })
}

/// The socket's login (SmartFox `<msg t='sys'><body action='login'>`), with the session token.
fn is_login_message(s: &str) -> bool {
    s.contains("action='login'") || s.contains("action=\"login\"")
}

/// A chat message sent or received, a whisper included, as a packet or as a `pext`.
fn is_chat(s: &str) -> bool {
    const MARKS: &[&str] = &[
        "%xt%zm%message%",
        "%xt%zm%whisper%",
        "%xt%chatm%",
        "%xt%whisper%",
        "\"chatm\"",
        "\"whisper\"",
    ];
    MARKS.iter().any(|m| s.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secret_paths() {
        assert!(is_secret_path("loginInfo.strPassword"));
        assert!(is_secret_path("loginInfo.anything"));
        assert!(!is_secret_path("loginInfo.strUsername"));
        assert!(is_secret_path("objLogin.sToken"));
        assert!(!is_secret_path("objLogin.iUpgDays"));
        assert!(!is_secret_path("world.myAvatar.objData.intGold"));
    }

    fn call(name: &str, args: Value, answer: &str) -> Line {
        Line::Call {
            n: 0,
            t: 0,
            thread: None,
            name: name.into(),
            args: args.as_array().unwrap().clone(),
            answer: Some(answer.into()),
            error: None,
            run_ms: None,
        }
    }

    #[test]
    fn keyed_reads_and_sets_of_a_secret() {
        let mut read = call(
            "getGameObjectKey",
            json!(["loginInfo", "strPassword"]),
            r#""hunter2""#,
        );
        line(&mut read);
        assert!(
            matches!(&read, Line::Call { answer: Some(a), args, .. } if a == REDACTED && args[1] == "strPassword")
        );
        let mut set = call(
            "setGameObjectKey",
            json!(["objLogin", "sToken", "tok-123"]),
            "null",
        );
        line(&mut set);
        assert!(matches!(&set, Line::Call { args, .. } if args[2] == REDACTED));
        let mut gold = call(
            "getGameObjectKey",
            json!(["world.myAvatar.objData", "intGold"]),
            "1234",
        );
        line(&mut gold);
        assert!(matches!(&gold, Line::Call { answer: Some(a), .. } if a == "1234"));
    }

    #[test]
    fn notes_lose_their_secret_fields() {
        let mut note = Line::Note {
            n: 0,
            t: 0,
            what: "scriptStarted".into(),
            detail: json!({"options": {"password": "hunter2", "farm": "gold"}}),
        };
        line(&mut note);
        assert!(matches!(
            &note,
            Line::Note { detail, .. } if detail["options"]["password"] == REDACTED && detail["options"]["farm"] == "gold"
        ));
    }

    #[test]
    fn json_answers_lose_their_secret_fields() {
        let mut s = json!({"sToken": "abc", "servers": [{"sName": "Yorumi"}]}).to_string();
        text(&mut s);
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["sToken"], REDACTED);
        assert_eq!(v["servers"][0]["sName"], "Yorumi");
    }

    #[test]
    fn the_accounts_email_and_address_are_redacted() {
        let mut s = json!({"cmd": "initUserData", "data": {
            "strUsername": "Questwalker", "strEmail": "questwalker@example.com",
            "strIP": "192.0.2.7", "ip0": 5, "ip1": 0,
        }})
        .to_string();
        assert!(text(&mut s));
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["data"]["strEmail"], REDACTED);
        assert_eq!(v["data"]["strIP"], REDACTED);
        assert_eq!(v["data"]["ip0"], 5);
        assert_eq!(v["data"]["strUsername"], "Questwalker");
        assert!(is_secret_path("world.myAvatar.objData.strEmail"));
    }

    #[test]
    fn items_with_token_names_are_kept() {
        let mut s = json!({"TokID": 5, "TokName": "Gold Voucher", "iTokens": 3}).to_string();
        let before = s.clone();
        text(&mut s);
        assert_eq!(s, before);
    }
}
