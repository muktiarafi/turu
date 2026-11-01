//! The control API (VibeSkua-Docker's, for the routes that apply): steer the bot over HTTP,
//! mainly when it runs without a window (`--headless`). Every request needs the token, as
//! `Authorization: Bearer <token>` or `X-Api-Token: <token>`: whoever can call it can run
//! scripts as the bot.
//!
//! | Route | What |
//! |---|---|
//! | `GET /status` | Logged in, player, map, cell, HP, the script and whether it runs, stats. |
//! | `GET /snapshot[?since=][&slow=1]` | The web monitor's `Snapshot` (the slow fields with `slow=1`), the events from number `since`, and `next`. Army polls it. |
//! | `POST /script/load?path=` | Load a script (relative to the scripts folder, or absolute). |
//! | `POST /script/start[?path=]` | Start the loaded script, or load and start `path`. |
//! | `POST /script/stop`, `/script/pause`, `/script/resume` | |
//! | `GET /log?type=script\|debug\|packet[&since=]` | Log lines from number `since`, and `next`. |
//! | `GET /options` | The bot options (Options > Game). |
//! | `POST /options` | Body `{"name": value, ...}`: set options (until the script stops, as scripts do). |
//! | `POST /login[?server=]` | Log in (the remembered login), on `server` or the best one. |
//! | `POST /logout`, `POST /relogin[?server=]` | |
//! | `POST /map/join?map=[&cell=][&pad=]` | Join `map` (or jump, when you're in it already), at `cell` (Enter) and `pad` (Spawn). |
//! | `POST /player/goto?name=` | Go to a player (`/goto`). |
//! | `POST /house` | Go to your house. |
//! | `POST /quests/register?id=[&reward=]` | Accept and turn in a quest in the background (`bot.quests.registerQuests`). |
//! | `GET /gear` | The account's gear file (`crate::gear`), read from the game now. |
//! | `POST /equip?item=[&stop=1]` | Put an item on (name or ID), from the bank if needed: the `EquipReport`. |
//! | `POST /bank/load[?stop=1]` | Load the bank into the gear file. |
//!
//! `/equip` and `/bank/load` answer 409 while a script runs, unless `stop=1`: then the script
//! is stopped first and stays stopped.
//!
//! The Army window sends these to the clients it launched (`crate::army`), and to its own
//! client through [`local`].

use crate::api::{map, player, send, servers};
use crate::log::LogKind;
use crate::{Bot, gear};
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Bodies larger than this are refused.
const MAX_BODY: usize = 64 * 1024;

/// Listens on `addr` on its own thread; returns the address it listens on.
pub fn start(bot: &Arc<Bot>, addr: SocketAddr, token: String) -> Result<SocketAddr, String> {
    if token.len() < 16 {
        return Err("the API token must be at least 16 characters".into());
    }
    // Bound here, so that a port in use is an error for the caller.
    let listener =
        std::net::TcpListener::bind(addr).map_err(|e| format!("couldn't listen on {addr}: {e}"))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let bot = bot.clone();
    std::thread::Builder::new()
        .name("Turu API".into())
        .spawn(move || runtime.block_on(serve(listener, bot, token.into())))
        .map_err(|e| e.to_string())?;
    Ok(local)
}

async fn serve(listener: std::net::TcpListener, bot: Arc<Bot>, token: Arc<str>) {
    let listener = match tokio::net::TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Control API: {e}");
            return;
        }
    };
    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(e) => {
                // Out of file descriptors and the like: don't spin.
                tracing::debug!("Control API accept: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let (bot, token) = (bot.clone(), token.clone());
        tokio::spawn(async move {
            let service = service_fn(move |request| handle(bot.clone(), token.clone(), request));
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}

/// A random token, 32 bytes as hex.
pub fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The token in `path`, or a new one written there, readable by its owner only.
pub fn saved_token(path: &Path) -> Result<String, String> {
    if let Ok(token) = std::fs::read_to_string(path)
        && token.trim().len() >= 16
    {
        return Ok(token.trim().to_owned());
    }
    let token = new_token()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    std::io::Write::write_all(&mut file, token.as_bytes()).map_err(|e| e.to_string())?;
    Ok(token)
}

struct Reply {
    status: u16,
    body: Value,
}

fn ok(body: Value) -> Reply {
    Reply { status: 200, body }
}

fn error(status: u16, message: impl Into<String>) -> Reply {
    Reply {
        status,
        body: json!({ "error": message.into() }),
    }
}

async fn handle(
    bot: Arc<Bot>,
    token: Arc<str>,
    request: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let reply = if !authorized(request.headers(), &token) {
        error(401, "missing or wrong API token")
    } else {
        let (parts, body) = request.into_parts();
        match Limited::new(body, MAX_BODY).collect().await {
            Err(_) => error(413, "the body is too large or was cut off"),
            // Bot calls block (a relogin for up to a minute): off the runtime's thread.
            Ok(body) => tokio::task::spawn_blocking(move || {
                let path = parts.uri.path_and_query().map_or("/", |p| p.as_str());
                route(&bot, parts.method == Method::POST, path, &body.to_bytes())
            })
            .await
            .unwrap_or_else(|e| error(500, e.to_string())),
        }
    };
    let response = Response::builder()
        .status(reply.status)
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(reply.body.to_string())))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from_static(b"{}"))));
    Ok(response)
}

pub(crate) fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let bearer = headers
        .get_all(AUTHORIZATION)
        .iter()
        .filter_map(|v| v.to_str().ok()?.trim().strip_prefix("Bearer "));
    let api_token = headers
        .get_all("x-api-token")
        .iter()
        .filter_map(|v| v.to_str().ok());
    bearer
        .chain(api_token)
        .any(|given| same(given.trim().as_bytes(), token.as_bytes()))
}

/// Compares in a time that doesn't depend on where they differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Answers a request without HTTP (for this process's own bot): the status and the body.
/// Blocks as long as the route does (a login up to a minute).
pub fn local(bot: &Arc<Bot>, post: bool, path_and_query: &str, body: &[u8]) -> (u16, Value) {
    let reply = route(bot, post, path_and_query, body);
    (reply.status, reply.body)
}

fn route(bot: &Arc<Bot>, post: bool, path_and_query: &str, body: &[u8]) -> Reply {
    let Ok(url) = reqwest::Url::parse(&format!("http://turu{path_and_query}")) else {
        return error(400, "bad URL");
    };
    let query = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .filter(|v| !v.is_empty())
    };
    match (url.path(), post) {
        ("/status", false) => ok(status(bot)),
        ("/snapshot", false) => {
            let since = query("since").and_then(|s| s.parse().ok()).unwrap_or(0);
            let snapshot = crate::monitor::sample(bot, query("slow").as_deref() == Some("1"));
            let (events, next) = bot.monitor.events_since(since);
            ok(json!({ "snapshot": snapshot, "events": events, "next": next }))
        }
        ("/script/load", true) => match query("path") {
            Some(path) => load(bot, &path).map_or_else(|e| e, |path| ok(json!({ "loaded": path }))),
            None => error(400, "path is missing"),
        },
        ("/script/start", true) => {
            if let Some(path) = query("path")
                && let Err(e) = load(bot, &path)
            {
                return e;
            }
            match crate::script::start_silent(bot) {
                Ok(()) => ok(json!({ "started": bot.scripts.loaded() })),
                Err(e) => error(409, e),
            }
        }
        ("/script/stop", true) => {
            crate::script::stop(bot);
            ok(json!({ "stopping": bot.scripts.is_running() }))
        }
        ("/script/pause", true) | ("/script/resume", true) => {
            if !bot.scripts.is_running() {
                return error(409, "no script is running");
            }
            crate::script::set_paused(bot, url.path() == "/script/pause");
            ok(json!({ "status": bot.scripts.status().label() }))
        }
        ("/log", false) => {
            let kind = match query("type").as_deref() {
                None | Some("script") => LogKind::Script,
                Some("debug") => LogKind::Debug,
                Some("packet") => LogKind::Packet,
                Some(other) => return error(400, format!("no log named {other}")),
            };
            let since = query("since").and_then(|s| s.parse().ok()).unwrap_or(0);
            let (lines, next) = bot.logs.get(kind).since(since);
            let lines: Vec<Value> = lines
                .iter()
                .map(|l| json!({ "time": l.time.to_rfc3339(), "text": l.text }))
                .collect();
            ok(json!({ "lines": lines, "next": next }))
        }
        ("/options", false) => ok(serde_json::to_value(&*bot.opts()).unwrap_or(Value::Null)),
        ("/options", true) => set_options(bot, body),
        ("/login", true) | ("/relogin", true) => {
            let ctx = servers::Ctx::new(bot);
            let server = query("server");
            let result = if url.path() == "/login" {
                servers::ensure_relogin(&ctx, server.as_deref())
            } else {
                match server {
                    Some(name) => servers::relogin_named(&ctx, &name),
                    None => servers::relogin(&ctx, None),
                }
            };
            match result {
                Ok(playing) => ok(json!({ "playing": playing })),
                Err(e) => error(500, e.to_string()),
            }
        }
        ("/logout", true) => match servers::logout(bot) {
            Ok(()) => ok(json!({ "loggedOut": true })),
            Err(e) => error(500, e.to_string()),
        },
        ("/map/join", true) => {
            let Some(map) = query("map") else {
                return error(400, "map is missing");
            };
            let cell = query("cell").unwrap_or_else(|| "Enter".into());
            let pad = query("pad").unwrap_or_else(|| "Spawn".into());
            game_action(join_or_jump(bot, &map, &cell, &pad))
        }
        ("/player/goto", true) => match query("name") {
            Some(name) => game_action(player::goto(bot, &name)),
            None => error(400, "name is missing"),
        },
        ("/house", true) => game_action(join_house(bot)),
        ("/quests/register", true) => {
            let Some(id) = query("id").and_then(|id| id.parse::<i64>().ok()) else {
                return error(400, "id is missing or isn't a number");
            };
            let reward = query("reward").and_then(|r| r.parse().ok()).unwrap_or(-1);
            bot.quests.register(bot, &[(id, reward)]);
            ok(json!({ "registered": bot.quests.registered() }))
        }
        ("/gear", false) => match gear::refresh(bot) {
            Ok(Some(file)) => ok(serde_json::to_value(file).unwrap_or(Value::Null)),
            Ok(None) => error(409, "not logged in"),
            Err(e) => error(500, e.to_string()),
        },
        ("/equip", true) => {
            let Some(item) = query("item") else {
                return error(400, "item is missing");
            };
            let stop = query("stop").as_deref() == Some("1");
            match gear::equip(bot, &[gear::name_or_id(&item)], stop) {
                Ok(report) => ok(serde_json::to_value(report).unwrap_or(Value::Null)),
                Err(e) => error(e.status(), e.to_string()),
            }
        }
        ("/bank/load", true) => match gear::load_bank(bot, query("stop").as_deref() == Some("1")) {
            Ok((file, stopped)) => ok(json!({ "gear": file, "stoppedScript": stopped })),
            Err(e) => error(e.status(), e.to_string()),
        },
        (
            "/status" | "/snapshot" | "/script/load" | "/script/start" | "/script/stop"
            | "/script/pause" | "/script/resume" | "/log" | "/login" | "/relogin" | "/logout"
            | "/map/join" | "/player/goto" | "/house" | "/quests/register" | "/gear" | "/equip"
            | "/bank/load",
            _,
        ) => error(405, "wrong method"),
        _ => error(404, "no such route"),
    }
}

fn game_action(result: crate::BotResult<()>) -> Reply {
    match result {
        Ok(()) => ok(json!({ "done": true })),
        Err(e) => error(500, e.to_string()),
    }
}

/// VibeSkua's "Jump All to Map": a jump when you're in that map already, else a join.
fn join_or_jump(bot: &Bot, map: &str, cell: &str, pad: &str) -> crate::BotResult<()> {
    let here = map::name(bot)?;
    if here.eq_ignore_ascii_case(map.split('-').next().unwrap_or_default()) {
        map::jump(bot, cell, pad, false, false)
    } else {
        map::join(bot, map, cell, pad, true, false)
    }
}

/// The packet CoreBots joins your own house with.
fn join_house(bot: &Bot) -> crate::BotResult<()> {
    if !player::logged_in(bot)? {
        return Err(crate::BotError::Other("not logged in".into()));
    }
    let name = player::username(bot)?;
    send::packet(bot, &format!("%xt%zm%house%1%{name}%"), "String")
}

fn status(bot: &Bot) -> Value {
    let script = json!({
        "path": bot.scripts.loaded(),
        "status": bot.scripts.status().label(),
        "running": bot.scripts.is_running(),
        // Scripts > Scheduler: a stop here moves its playlist on to the next script.
        "scheduler": {
            "running": bot.scheduler.is_running(),
            "summary": bot.scheduler.summary(),
        },
    });
    let (stats, loaded, connected) = {
        let state = bot.state.lock();
        let s = &state.stats;
        let stats = json!({
            "kills": s.kills, "deaths": s.deaths, "drops": s.drops,
            "questsAccepted": s.quests_accepted, "questsCompleted": s.quests_completed,
            "relogins": s.relogins,
            "runSeconds": s.started.map(|t| t.elapsed().as_secs()),
        });
        (stats, state.game_loaded, bot.flash.has_host())
    };
    let mut status = json!({
        "connected": connected,
        "gameLoaded": loaded,
        "relogging": bot.servers.is_relogging(),
        "script": script,
        "stats": stats,
    });
    if !(connected && loaded) {
        return status;
    }
    let logged_in = player::logged_in(bot).unwrap_or(false);
    status["loggedIn"] = json!(logged_in);
    if logged_in && player::world_loaded(bot).unwrap_or(false) {
        let game = json!({
            "playing": player::playing(bot).unwrap_or(false),
            "username": player::username(bot).unwrap_or_default(),
            "server": bot.servers.last_name(),
            "map": map::name(bot).unwrap_or_default(),
            "cell": player::cell(bot).unwrap_or_default(),
            "pad": player::pad(bot).unwrap_or_default(),
            "health": player::health(bot).unwrap_or_default(),
            "maxHealth": player::max_health(bot).unwrap_or_default(),
            "level": bot.flash.get_or::<i64>("world.myAvatar.dataLeaf.intLevel", 0).unwrap_or_default(),
        });
        if let (Value::Object(status), Value::Object(game)) = (&mut status, game) {
            status.extend(game);
        }
    }
    status
}

/// Loads a script: absolute, or relative to the scripts folder (else to the working folder),
/// `.ts` optional.
fn load(bot: &Bot, path: &str) -> Result<PathBuf, Reply> {
    let Some(path) = resolve(&bot.paths.scripts_dir(), Path::new(path)) else {
        return Err(error(404, format!("no script at {path}")));
    };
    if bot.scripts.is_running() {
        return Err(error(409, "a script is running; stop it first"));
    }
    bot.scripts.load(&path);
    Ok(path)
}

fn resolve(scripts: &Path, path: &Path) -> Option<PathBuf> {
    let with_ts = path.with_extension("ts");
    let bases: &[&Path] = if path.is_absolute() {
        &[Path::new("")]
    } else {
        &[scripts, Path::new(".")]
    };
    bases
        .iter()
        .flat_map(|base| [base.join(path), base.join(&with_ts)])
        .find(|p| p.is_file())
}

fn set_options(bot: &Bot, body: &[u8]) -> Reply {
    let Ok(Value::Object(values)) = serde_json::from_slice::<Value>(body) else {
        return error(
            400,
            "the body must be a JSON object of option names and values",
        );
    };
    let mut options = bot.opts().clone();
    for (name, value) in values {
        if let Err(e) = options.set(&name, value) {
            return error(400, e);
        }
    }
    *bot.options.write() = options;
    ok(serde_json::to_value(&*bot.opts()).unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_tokens() {
        assert!(same(b"abcdef", b"abcdef"));
        assert!(!same(b"abcdef", b"abcdeg"));
        assert!(!same(b"abc", b"abcdef"));
        assert_eq!(new_token().unwrap().len(), 64);
    }
}
