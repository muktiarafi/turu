//! The control API: steer the bot over HTTP, mainly when it runs without a window
//! (`--headless`). Every request needs the token, as `Authorization: Bearer <token>` (32
//! characters at least): whoever can call it can run scripts as the bot.
//!
//! The routes act on the first tab (the only one without a window). `/tabs` lists the tabs, and
//! `/tabs/<id>/<route>` sends any route, `/events` included, to one of them.
//!
//! | Route | What |
//! |---|---|
//! | `GET /tabs` | The tabs: `id`, `label` and `default` (the one the routes without `/tabs/<id>` act on). |
//! | `GET /events[?logs=script,debug,packet]` | A stream of server-sent events, so a client needn't poll: `script` (as `/status`'s `script`) at the start and when it changes, `log` (`type`, `time`, `text`) for each new line of the logs asked for (`script` by default; `logs=` for none), `event` for each web monitor event (`seq`, `at`, `kind`, `text`), and a comment every 15 s. It reads the bot's state only, never the game. |
//! | `GET /status` | Logged in, player, map, cell, HP, the script and whether it runs, stats, and `update`: a newer Turu (`version`, `current`, `url` of its notes, `download`, `prerelease`) or `null`. Asks the game: prefer `/events` to polling it. |
//! | `GET /snapshot[?since=][&slow=1]` | The web monitor's `Snapshot` (the slow fields with `slow=1`), the events from number `since`, and `next`. Army polls it. |
//! | `POST /script/load?path=` | Load a script (relative to the scripts folder, or absolute). |
//! | `POST /script/start[?path=][&configure=1]` | Start the loaded script, or load and start `path`; with `configure=1` it shows the Script Options window first, as Start does in the window (Army Control sends it). Answers `started`, `options` (as `GET /script/options`) and `optionsWindow`: whether this run may show the window (not with `--headless` or without `configure=1`); without it the script uses `options` and its log says so. |
//! | `POST /script/stop`, `/script/pause`, `/script/resume` | |
//! | `GET /script/options` | The loaded script's Script Options: `options` (`none`, `saved` in `file`, or `defaults` when nothing was saved), `storage`, `file`, `dontPreconfigure`, and `categories` (`Options` first, then the groups), each with its `options`: `name`, `displayName`, `description`, `kind` (`bool`, `int`, `text`, `choice`), `default`, `choices`, `value`. |
//! | `POST /script/options[?reset=1]` | Body `{"name": value, "Group": {"name": value}}`: set the loaded script's options and save them (all or none), after the defaults with `reset=1`. Answers as `GET`. |
//! | `GET /scheduler/options?id=`, `POST /scheduler/options?id=[&reset=1]` | The same for a playlist entry's options (`options/Queue_<script>_<id>.cfg`). |
//! | `GET /log?type=script\|debug\|packet[&since=]` | Log lines from number `since`, and `next`. |
//! | `GET /game/options` | The bot options (Options > Game). |
//! | `POST /game/options` | Body `{"name": value, ...}`: set options (until the script stops, as scripts do). |
//! | `POST /login[?server=]` | Log in (the remembered login), on `server` or the best one. |
//! | `POST /logout`, `POST /relogin[?server=]` | |
//! | `POST /map/join?map=[&cell=][&pad=]` | Join `map` (or jump, when you're in it already), at `cell` (Enter) and `pad` (Spawn). |
//! | `POST /player/goto?name=` | Go to a player (`/goto`). |
//! | `POST /map/house` | Go to your house. |
//! | `POST /quest/register?id=[&reward=]` | Accept and turn in a quest in the background (`bot.quests.registerQuests`). |
//! | `GET /gear` | The account's gear file (`crate::gear`), read from the game now. |
//! | `POST /equip?item=[&stop=1]` | Put an item on (name or ID), from the bank if needed: the `EquipReport`. |
//! | `POST /bank/load[?stop=1]` | Load the bank into the gear file. |
//! | `POST /auto/attack[?class=][&mode=][&maps=]`, `/auto/hunt` (same) | Start Auto Attack or Auto Hunt (`crate::auto`): `class`'s skills in `mode` (Base), else the equipped class; `maps`: monster map IDs "1,2" in priority order. |
//! | `POST /auto/stop` | Stop Auto Attack or Auto Hunt. |
//! | `GET /scheduler` | The Scheduler's playlist: whether it runs, the summary, the current entry and each entry (id, name, path, status, seconds). |
//! | `POST /scheduler/add?path=` | Add a script at the end (found as `/script/load` finds it). |
//! | `POST /scheduler/remove?id=`, `/scheduler/move?id=&by=-1\|1`, `/scheduler/rename?id=&name=`, `/scheduler/clear` | Edit the playlist. |
//! | `POST /scheduler/load[?path=]` | Replace the playlist: the playlist's JSON in the body, or the file at `path`. |
//! | `POST /scheduler/save?path=` | Save the playlist to a file. |
//! | `POST /scheduler/start`, `/scheduler/stop` | Start queue, Stop queue. |
//! | `GET /butler` | Butler: whether it's on, this account, whom it follows and summons, the suspend question, and the other online accounts (each with why it can't be summoned now). |
//! | `POST /butler/summon?user=`, `/butler/release?user=` | Summon or release one account (`user` must be online with Butler to summon). |
//! | `POST /butler/summon-all`, `/butler/release-all` | Summon every available account; release them all. |
//! | `POST /butler/follow?master=`, `/butler/unfollow` | Follow an account from this client; stop following (declining a summon). |
//! | `POST /update[?stop=1]` | Download the newer Turu, check it against `SHA256SUMS.txt`, put it in place and exit with status 75, so a service manager restarts the new one (`--headless`; with a window it starts the new one itself). 409 while a script runs unless `stop=1`, or when there's no update. |
//! | `GET /debug/perf` | The Flash calls this client made and its player's ticks, since it started (`crate::perf::PerfSnapshot`). |
//!
//! Answers are JSON. A command answers with the state it changed: the `/script/*` commands with
//! the script (`path`, `status`, `running`, `scheduler`: as `/status`'s `script`), `/login`,
//! `/relogin` and `/logout` with `/status` (the logins add `playing`), the moves (`/map/join`,
//! `/map/house`, `/player/goto`) with where the player is now (`map`, `cell`, `pad`),
//! `/auto/*` with `auto` (the mode running now, or `null`; `/auto/stop` adds the mode it
//! `stopping`), the `/scheduler/*` and `/butler/*` commands with `/scheduler` and `/butler`.
//!
//! The yes/no parameters (`stop`, `configure`, `reset`, `slow`) take `1` or `true`, `0` or
//! `false`; anything else, like a number parameter that isn't one, answers 400.
//!
//! A failure is `{"error": "<for people>", "code": "<for programs>"}`:
//!
//! | Code | Status | When |
//! |---|---|---|
//! | `missing_parameter` | 400 | A parameter the route needs isn't there. |
//! | `invalid_parameter` | 400 | A parameter isn't a number, a log, a flag, ... |
//! | `invalid_body` | 400 | The body isn't the JSON the route takes, or a value in it is wrong. |
//! | `unauthorized` | 401 | No `Authorization: Bearer` token, or the wrong one. |
//! | `path_outside_data` | 403 | A playlist `path` outside the data folder. |
//! | `no_route` | 404 | No such route. |
//! | `unknown_tab` | 404 | No tab with that id. |
//! | `no_script` | 404 | No script at that path. |
//! | `no_entry` | 404 | No playlist entry with that id. |
//! | `wrong_method` | 405 | The route takes another method; `Allow` says which. |
//! | `not_logged_in` | 409 | The route needs a character in game. |
//! | `script_running` | 409 | A script runs (`stop=1` stops it first, where the route takes it). |
//! | `no_script_running` | 409 | Pause or resume with no script running. |
//! | `script_wont_stop` | 409 | `stop=1`, but the script didn't stop in time. |
//! | `script_refused` | 409 | The script couldn't start (none loaded, one runs, ...). |
//! | `login_under_way` | 409 | A login (yours, or an auto-relogin) is under way. |
//! | `options_refused` | 409 | The script's options couldn't be saved now. |
//! | `options_unreadable` | 409 | The script's options couldn't be read. |
//! | `scheduler_refused` | 409 | The playlist runs (it can't be edited), is empty, ... |
//! | `butler_refused` | 409 | Butler is off, this account follows someone, a script runs, ... |
//! | `auto_refused` | 409 | Auto Attack/Hunt can't start: a script, a Console snippet or Auto runs. |
//! | `no_update` | 409 | `/update` with no newer Turu. |
//! | `update_failed` | 409 | The update couldn't be downloaded, checked or put in place. |
//! | `body_too_large` | 413 | The body is over 64 KiB. |
//! | `game_error` | 500 | The game call failed. |
//! | `internal` | 500 | Something failed in Turu (a file it couldn't write, ...). |
//! | `not_supported` | 501 | This client can't do it (update itself). |
//! | `game_not_running` | 503 | The game isn't running: not loaded yet, or the app is closing. |
//! | `no_tabs` | 503 | There's no tab. |
//! | `game_timeout` | 504 | The game didn't answer in time. |
//!
//! `/equip` and `/bank/load` answer 409 while a script runs, unless `stop=1`: then the script
//! is stopped first and stays stopped. `/auto/attack` and `/auto/hunt` answer 409 while a
//! script, a Console snippet or Auto Attack/Hunt runs, or when not logged in.
//!
//! The playlist can't be changed while it runs (409; renaming can). `/script/stop` is Stop
//! Script: with the playlist running, the next entry starts; `/scheduler/stop` stops the
//! playlist. `/scheduler/load` and `/scheduler/save` take a `path` relative to the data
//! folder's `playlists` folder, or an absolute one inside the data folder.
//!
//! The Army window sends these to the clients it launched (`crate::army`), and to its own
//! client through [`local`].

use crate::api::{map, player, send, servers};
use crate::auto::{self, AutoMode};
use crate::host::HostError;
use crate::log::LogKind;
use crate::monitor::hub::{Source, Sources};
use crate::script::options::ScriptOptions;
use crate::script::{OptionsSource, ScriptOptionsState};
use crate::{Bot, BotError, butler, gear};
use http_body_util::channel::Channel;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::header::{ALLOW, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, HeaderMap};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// The shortest token accepted: 128 bits of hex. Made ones are 64 characters ([`new_token`]).
pub const MIN_TOKEN: usize = 32;
/// Bodies larger than this are refused.
const MAX_BODY: usize = 64 * 1024;
/// How often `/events` looks for news.
const EVENTS_EVERY: Duration = Duration::from_millis(250);
/// How often `/events` sends a comment when nothing happens (so proxies keep it open, and a
/// client that left is noticed).
const EVENTS_PING: Duration = Duration::from_secs(15);

type Body = BoxBody<Bytes, Infallible>;

/// Listens on `addr` on its own thread, for `bot` alone; returns the address it listens on.
pub fn start(bot: &Arc<Bot>, addr: SocketAddr, token: String) -> Result<SocketAddr, String> {
    let bot = bot.clone();
    let tabs: Sources = Arc::new(move || {
        vec![Source {
            id: "tab:0".into(),
            label: "Tab 1".into(),
            bot: bot.clone(),
        }]
    });
    start_tabs(tabs, addr, token)
}

/// Listens on `addr` on its own thread, for the tabs `tabs` gives (the first is the default);
/// returns the address it listens on.
pub fn start_tabs(tabs: Sources, addr: SocketAddr, token: String) -> Result<SocketAddr, String> {
    if token.len() < MIN_TOKEN {
        return Err(format!(
            "the API token must be at least {MIN_TOKEN} characters"
        ));
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
    std::thread::Builder::new()
        .name("Turu API".into())
        .spawn(move || runtime.block_on(serve(listener, tabs, token.into())))
        .map_err(|e| e.to_string())?;
    Ok(local)
}

async fn serve(listener: std::net::TcpListener, tabs: Sources, token: Arc<str>) {
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
        let (tabs, token) = (tabs.clone(), token.clone());
        tokio::spawn(async move {
            let service = service_fn(move |request| handle(tabs.clone(), token.clone(), request));
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
        && token.trim().len() >= MIN_TOKEN
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
    /// The `Allow` header of a 405.
    allow: Option<&'static str>,
}

fn ok(body: Value) -> Reply {
    Reply {
        status: 200,
        body,
        allow: None,
    }
}

/// Why a request failed: `code` in the error's body, for clients to act on (the `error` text is
/// for people), and its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Code {
    MissingParameter,
    InvalidParameter,
    InvalidBody,
    Unauthorized,
    PathOutsideData,
    NoRoute,
    UnknownTab,
    NoScript,
    NoEntry,
    WrongMethod,
    NotLoggedIn,
    ScriptRunning,
    NoScriptRunning,
    ScriptWontStop,
    ScriptRefused,
    LoginUnderWay,
    OptionsRefused,
    OptionsUnreadable,
    SchedulerRefused,
    ButlerRefused,
    AutoRefused,
    NoUpdate,
    UpdateFailed,
    BodyTooLarge,
    GameError,
    Internal,
    NotSupported,
    GameNotRunning,
    NoTabs,
    GameTimeout,
}

impl Code {
    /// Every code, for the test that the table above lists each.
    #[cfg(test)]
    const ALL: [Code; 30] = [
        Code::MissingParameter,
        Code::InvalidParameter,
        Code::InvalidBody,
        Code::Unauthorized,
        Code::PathOutsideData,
        Code::NoRoute,
        Code::UnknownTab,
        Code::NoScript,
        Code::NoEntry,
        Code::WrongMethod,
        Code::NotLoggedIn,
        Code::ScriptRunning,
        Code::NoScriptRunning,
        Code::ScriptWontStop,
        Code::ScriptRefused,
        Code::LoginUnderWay,
        Code::OptionsRefused,
        Code::OptionsUnreadable,
        Code::SchedulerRefused,
        Code::ButlerRefused,
        Code::AutoRefused,
        Code::NoUpdate,
        Code::UpdateFailed,
        Code::BodyTooLarge,
        Code::GameError,
        Code::Internal,
        Code::NotSupported,
        Code::GameNotRunning,
        Code::NoTabs,
        Code::GameTimeout,
    ];

    fn status(self) -> u16 {
        match self {
            Code::MissingParameter | Code::InvalidParameter | Code::InvalidBody => 400,
            Code::Unauthorized => 401,
            Code::PathOutsideData => 403,
            Code::NoRoute | Code::UnknownTab | Code::NoScript | Code::NoEntry => 404,
            Code::WrongMethod => 405,
            Code::NotLoggedIn
            | Code::ScriptRunning
            | Code::NoScriptRunning
            | Code::ScriptWontStop
            | Code::ScriptRefused
            | Code::LoginUnderWay
            | Code::OptionsRefused
            | Code::OptionsUnreadable
            | Code::SchedulerRefused
            | Code::ButlerRefused
            | Code::AutoRefused
            | Code::NoUpdate
            | Code::UpdateFailed => 409,
            Code::BodyTooLarge => 413,
            Code::GameError | Code::Internal => 500,
            Code::NotSupported => 501,
            Code::GameNotRunning | Code::NoTabs => 503,
            Code::GameTimeout => 504,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Code::MissingParameter => "missing_parameter",
            Code::InvalidParameter => "invalid_parameter",
            Code::InvalidBody => "invalid_body",
            Code::Unauthorized => "unauthorized",
            Code::PathOutsideData => "path_outside_data",
            Code::NoRoute => "no_route",
            Code::UnknownTab => "unknown_tab",
            Code::NoScript => "no_script",
            Code::NoEntry => "no_entry",
            Code::WrongMethod => "wrong_method",
            Code::NotLoggedIn => "not_logged_in",
            Code::ScriptRunning => "script_running",
            Code::NoScriptRunning => "no_script_running",
            Code::ScriptWontStop => "script_wont_stop",
            Code::ScriptRefused => "script_refused",
            Code::LoginUnderWay => "login_under_way",
            Code::OptionsRefused => "options_refused",
            Code::OptionsUnreadable => "options_unreadable",
            Code::SchedulerRefused => "scheduler_refused",
            Code::ButlerRefused => "butler_refused",
            Code::AutoRefused => "auto_refused",
            Code::NoUpdate => "no_update",
            Code::UpdateFailed => "update_failed",
            Code::BodyTooLarge => "body_too_large",
            Code::GameError => "game_error",
            Code::Internal => "internal",
            Code::NotSupported => "not_supported",
            Code::GameNotRunning => "game_not_running",
            Code::NoTabs => "no_tabs",
            Code::GameTimeout => "game_timeout",
        }
    }
}

fn error(code: Code, message: impl Into<String>) -> Reply {
    Reply {
        status: code.status(),
        body: json!({ "error": message.into(), "code": code.name() }),
        allow: None,
    }
}

fn wrong_method(allow: &'static str) -> Reply {
    Reply {
        allow: Some(allow),
        ..error(Code::WrongMethod, format!("this route takes {allow}"))
    }
}

/// A failed bot call: what went wrong with the game, not a bug here.
fn bot_error(e: BotError) -> Reply {
    let code = match &e {
        BotError::Host(HostError::Timeout) => Code::GameTimeout,
        BotError::Host(HostError::NoPlayer | HostError::Closed) => Code::GameNotRunning,
        BotError::InvalidArgument(_) => Code::InvalidParameter,
        BotError::Stopped => Code::ScriptRefused,
        _ => Code::GameError,
    };
    error(code, e.to_string())
}

fn gear_error(e: gear::GearError) -> Reply {
    let code = match &e {
        gear::GearError::ScriptRunning(_) => Code::ScriptRunning,
        gear::GearError::NotLoggedIn => Code::NotLoggedIn,
        gear::GearError::WontStop(_) => Code::ScriptWontStop,
        gear::GearError::Game(_) => Code::GameError,
    };
    error(code, e.to_string())
}

fn json_response(reply: Reply) -> Response<Body> {
    let mut builder = Response::builder()
        .status(reply.status)
        .header(CONTENT_TYPE, "application/json");
    if let Some(allow) = reply.allow {
        builder = builder.header(ALLOW, allow);
    }
    builder
        .body(Full::new(Bytes::from(reply.body.to_string())).boxed())
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from_static(b"{}")).boxed()))
}

async fn handle(
    tabs: Sources,
    token: Arc<str>,
    request: Request<Incoming>,
) -> Result<Response<Body>, Infallible> {
    if !authorized(request.headers(), &token) {
        return Ok(json_response(error(
            Code::Unauthorized,
            "missing or wrong API token",
        )));
    }
    let path_and_query = request
        .uri()
        .path_and_query()
        .map_or("/", |p| p.as_str())
        .to_owned();
    let (bot, path_and_query) = match pick_tab(&tabs, request.method(), &path_and_query) {
        Ok(picked) => picked,
        Err(reply) => return Ok(json_response(reply)),
    };
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((&path_and_query, ""));
    if path == "/events" {
        return Ok(match *request.method() {
            Method::GET => events(&bot, query),
            _ => json_response(wrong_method("GET")),
        });
    }
    let (parts, body) = request.into_parts();
    let reply = match Limited::new(body, MAX_BODY).collect().await {
        Err(_) => error(Code::BodyTooLarge, "the body is too large or was cut off"),
        // Bot calls block (a relogin for up to a minute): off the runtime's thread.
        Ok(body) => tokio::task::spawn_blocking(move || {
            route(
                &bot,
                parts.method.as_str(),
                &path_and_query,
                &body.to_bytes(),
            )
        })
        .await
        .unwrap_or_else(|e| error(Code::Internal, e.to_string())),
    };
    Ok(json_response(reply))
}

/// The tab a request is for and the route left for it: `/tabs/<id>/<route>`, else the first
/// tab. Answers `GET /tabs` itself.
fn pick_tab(
    tabs: &Sources,
    method: &Method,
    path_and_query: &str,
) -> Result<(Arc<Bot>, String), Reply> {
    let list = tabs();
    let path = path_and_query.split('?').next().unwrap_or_default();
    let short = |source: &Source| {
        source
            .id
            .strip_prefix("tab:")
            .unwrap_or(&source.id)
            .to_owned()
    };
    if path == "/tabs" {
        if method != Method::GET {
            return Err(wrong_method("GET"));
        }
        let tabs: Vec<Value> = list
            .iter()
            .enumerate()
            .map(|(i, source)| json!({ "id": short(source), "label": source.label, "default": i == 0 }))
            .collect();
        return Err(ok(json!({ "tabs": tabs })));
    }
    if let Some(rest) = path_and_query.strip_prefix("/tabs/") {
        let (id, route) = match rest.find(['/', '?']) {
            Some(at) if rest[at..].starts_with('/') => (&rest[..at], &rest[at..]),
            _ => {
                return Err(error(
                    Code::NoRoute,
                    "no route after the tab: /tabs/<id>/<route>",
                ));
            }
        };
        return list
            .into_iter()
            .find(|source| short(source) == id)
            .map(|source| (source.bot, route.to_owned()))
            .ok_or_else(|| error(Code::UnknownTab, format!("no tab {id}")));
    }
    match list.into_iter().next() {
        Some(source) => Ok((source.bot, path_and_query.to_owned())),
        None => Err(error(Code::NoTabs, "there's no tab")),
    }
}

/// `Authorization: Bearer <token>` (the scheme in any case, RFC 9110).
pub(crate) fn authorized(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get_all(AUTHORIZATION)
        .iter()
        .filter_map(|v| {
            let (scheme, given) = v.to_str().ok()?.trim().split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then_some(given)
        })
        .any(|given| same(given.trim().as_bytes(), token.as_bytes()))
}

/// Compares in a time that doesn't depend on where they differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `GET /events`: server-sent events until the client goes (or the tab's bot does).
fn events(bot: &Arc<Bot>, query: &str) -> Response<Body> {
    let mut kinds = vec![LogKind::Script];
    let url = reqwest::Url::parse(&format!("http://turu/?{query}"));
    for (name, value) in url.iter().flat_map(reqwest::Url::query_pairs) {
        if name != "logs" {
            continue;
        }
        kinds.clear();
        for kind in value.split(',').map(str::trim).filter(|k| !k.is_empty()) {
            match log_kind(kind) {
                Some(kind) => kinds.push(kind),
                None => {
                    return json_response(error(
                        Code::InvalidParameter,
                        format!("no log named {kind}"),
                    ));
                }
            }
        }
    }
    let bot = Arc::downgrade(bot);
    let (mut tx, body) = Channel::<Bytes, Infallible>::new(16);
    tokio::spawn(async move {
        let mut news = News::new(&bot, kinds);
        let mut quiet = Duration::ZERO;
        loop {
            let Some(frames) = news.read(&bot) else {
                return;
            };
            let frames = if !frames.is_empty() {
                quiet = Duration::ZERO;
                frames
            } else if quiet >= EVENTS_PING {
                quiet = Duration::ZERO;
                ": ping\n\n".to_owned()
            } else {
                String::new()
            };
            // Fails once the client has gone.
            if !frames.is_empty() && tx.send_data(Bytes::from(frames)).await.is_err() {
                return;
            }
            tokio::time::sleep(EVENTS_EVERY).await;
            quiet += EVENTS_EVERY;
        }
    });
    Response::builder()
        .status(200)
        .header(CONTENT_TYPE, "text/event-stream")
        .header(CACHE_CONTROL, "no-cache")
        .body(body.boxed())
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::new()).boxed()))
}

/// What `/events` has sent so far.
struct News {
    script: Option<Value>,
    logs: Vec<(LogKind, u64)>,
    events: u64,
}

impl News {
    /// From now on: no old log lines or events.
    fn new(bot: &Weak<Bot>, kinds: Vec<LogKind>) -> Self {
        let bot = bot.upgrade();
        let next_line = |kind| {
            bot.as_ref()
                .map_or(0, |b| b.logs.get(kind).since(u64::MAX).1)
        };
        Self {
            script: None,
            logs: kinds
                .into_iter()
                .map(|kind| (kind, next_line(kind)))
                .collect(),
            events: bot
                .as_ref()
                .map_or(0, |b| b.monitor.events_since(u64::MAX).1),
        }
    }

    /// The frames for what changed since the last call; `None` once the bot is gone.
    fn read(&mut self, bot: &Weak<Bot>) -> Option<String> {
        let bot = bot.upgrade()?;
        let mut frames = String::new();
        let script = script_state(&bot);
        if self.script.as_ref() != Some(&script) {
            push_frame(&mut frames, "script", &script);
            self.script = Some(script);
        }
        for (kind, next) in &mut self.logs {
            let (lines, after) = bot.logs.get(*kind).since(*next);
            *next = after;
            for line in lines {
                let line = json!({
                    "type": log_name(*kind),
                    "time": line.time.to_rfc3339(),
                    "text": line.text,
                });
                push_frame(&mut frames, "log", &line);
            }
        }
        let (events, next) = bot.monitor.events_since(self.events);
        self.events = next;
        for event in events {
            push_frame(
                &mut frames,
                "event",
                &serde_json::to_value(event).unwrap_or(Value::Null),
            );
        }
        Some(frames)
    }
}

/// One server-sent event (JSON has no raw newlines, so `data` is one line).
fn push_frame(frames: &mut String, event: &str, data: &Value) {
    frames.push_str(&format!("event: {event}\ndata: {data}\n\n"));
}

fn log_kind(name: &str) -> Option<LogKind> {
    match name {
        "script" => Some(LogKind::Script),
        "debug" => Some(LogKind::Debug),
        "packet" => Some(LogKind::Packet),
        _ => None,
    }
}

fn log_name(kind: LogKind) -> &'static str {
    match kind {
        LogKind::Script => "script",
        LogKind::Debug => "debug",
        LogKind::Packet => "packet",
    }
}

/// Answers a request without HTTP (for this process's own bot): the status and the body.
/// Blocks as long as the route does (a login up to a minute).
pub fn local(bot: &Arc<Bot>, post: bool, path_and_query: &str, body: &[u8]) -> (u16, Value) {
    let reply = route(bot, if post { "POST" } else { "GET" }, path_and_query, body);
    (reply.status, reply.body)
}

/// A request, for a route's handler.
struct Req<'a> {
    bot: &'a Arc<Bot>,
    url: reqwest::Url,
    post: bool,
    body: &'a [u8],
}

impl Req<'_> {
    /// A query parameter; empty counts as missing.
    fn query(&self, name: &str) -> Option<String> {
        self.url
            .query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .filter(|v| !v.is_empty())
    }

    /// `name=1` or `name=true` ([`route`] refuses other values of [`FLAGS`]).
    fn flag(&self, name: &str) -> bool {
        matches!(self.query(name).as_deref(), Some("1" | "true"))
    }

    /// A number parameter: `None` when missing, 400 when it isn't one.
    fn number<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>, Reply> {
        match self.query(name) {
            None => Ok(None),
            Some(value) => value.parse().map(Some).map_err(|_| {
                error(
                    Code::InvalidParameter,
                    format!("{name} must be a number, not {value}"),
                )
            }),
        }
    }
}

type Handler = fn(&Req) -> Reply;

/// The yes/no parameters: `1` or `true`, `0` or `false`.
const FLAGS: [&str; 4] = ["stop", "configure", "reset", "slow"];

/// A path and what answers each method on it.
struct Route {
    path: &'static str,
    get: Option<Handler>,
    post: Option<Handler>,
}

impl Route {
    const fn get(path: &'static str, get: Handler) -> Self {
        Self {
            path,
            get: Some(get),
            post: None,
        }
    }

    const fn post(path: &'static str, post: Handler) -> Self {
        Self {
            path,
            get: None,
            post: Some(post),
        }
    }

    const fn both(path: &'static str, get: Handler, post: Handler) -> Self {
        Self {
            path,
            get: Some(get),
            post: Some(post),
        }
    }

    /// The `Allow` header.
    fn allow(&self) -> &'static str {
        match (self.get.is_some(), self.post.is_some()) {
            (true, true) => "GET, POST",
            (true, false) => "GET",
            _ => "POST",
        }
    }
}

/// Every route: what answers, and from that which methods a path takes.
static ROUTES: &[Route] = &[
    Route::get("/status", |r| ok(status(r.bot))),
    Route::get("/snapshot", |r| {
        let since = match r.number("since") {
            Ok(since) => since.unwrap_or(0),
            Err(e) => return e,
        };
        let snapshot = crate::monitor::sample(r.bot, r.flag("slow"));
        let (events, next) = r.bot.monitor.events_since(since);
        ok(json!({ "snapshot": snapshot, "events": events, "next": next }))
    }),
    Route::post("/update", update),
    Route::get("/debug/perf", |r| {
        ok(serde_json::to_value(r.bot.flash.perf().snapshot()).unwrap_or(Value::Null))
    }),
    Route::post("/script/load", |r| match r.query("path") {
        Some(path) => load(r.bot, &path).map_or_else(|e| e, |_| ok(script_state(r.bot))),
        None => error(Code::MissingParameter, "path is missing"),
    }),
    Route::post("/script/start", start_script),
    Route::both(
        "/script/options",
        |r| script_options(r.bot.scripts.loaded().as_deref(), r.bot.scripts.options()),
        set_script_options,
    ),
    Route::post("/script/stop", |r| {
        crate::script::stop(r.bot);
        ok(script_state(r.bot))
    }),
    Route::post("/script/pause", |r| pause(r, true)),
    Route::post("/script/resume", |r| pause(r, false)),
    Route::get("/log", log),
    Route::both(
        "/game/options",
        |r| ok(serde_json::to_value(&*r.bot.opts()).unwrap_or(Value::Null)),
        |r| set_options(r.bot, r.body),
    ),
    Route::post("/login", |r| login(r, true)),
    Route::post("/relogin", |r| login(r, false)),
    Route::post("/logout", |r| match servers::logout(r.bot) {
        Ok(()) => ok(status(r.bot)),
        Err(e) => bot_error(e),
    }),
    Route::post("/map/join", |r| {
        let Some(map) = r.query("map") else {
            return error(Code::MissingParameter, "map is missing");
        };
        let cell = r.query("cell").unwrap_or_else(|| "Enter".into());
        let pad = r.query("pad").unwrap_or_else(|| "Spawn".into());
        in_game(r.bot, |bot| join_or_jump(bot, &map, &cell, &pad))
    }),
    Route::post("/player/goto", |r| match r.query("name") {
        Some(name) => in_game(r.bot, |bot| player::goto(bot, &name)),
        None => error(Code::MissingParameter, "name is missing"),
    }),
    Route::post("/map/house", |r| in_game(r.bot, join_house)),
    Route::post("/quest/register", |r| {
        let (id, reward) = match (r.number::<i64>("id"), r.number("reward")) {
            (Ok(Some(id)), Ok(reward)) => (id, reward.unwrap_or(-1)),
            (Ok(None), _) => return error(Code::MissingParameter, "id is missing"),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        r.bot.quests.register(r.bot, &[(id, reward)]);
        ok(json!({ "registered": r.bot.quests.registered() }))
    }),
    Route::get("/gear", |r| match gear::refresh(r.bot) {
        Ok(Some(file)) => ok(serde_json::to_value(file).unwrap_or(Value::Null)),
        Ok(None) => error(Code::NotLoggedIn, "not logged in"),
        Err(e) => bot_error(e),
    }),
    Route::post("/equip", |r| {
        let Some(item) = r.query("item") else {
            return error(Code::MissingParameter, "item is missing");
        };
        match gear::equip(r.bot, &[gear::name_or_id(&item)], r.flag("stop")) {
            Ok(report) => ok(serde_json::to_value(report).unwrap_or(Value::Null)),
            Err(e) => gear_error(e),
        }
    }),
    Route::post("/bank/load", |r| {
        match gear::load_bank(r.bot, r.flag("stop")) {
            Ok((file, stopped)) => ok(json!({ "gear": file, "stoppedScript": stopped })),
            Err(e) => gear_error(e),
        }
    }),
    Route::post("/auto/attack", |r| start_auto(r, AutoMode::Attack)),
    Route::post("/auto/hunt", |r| start_auto(r, AutoMode::Hunt)),
    Route::post("/auto/stop", |r| {
        let stopping = r.bot.auto.mode().map(AutoMode::label);
        auto::stop(r.bot);
        ok(json!({ "auto": r.bot.auto.mode().map(AutoMode::label), "stopping": stopping }))
    }),
    Route::get("/scheduler", |r| ok(scheduler(r.bot))),
    Route::post("/scheduler/add", |r| {
        let Some(path) = r.query("path") else {
            return error(Code::MissingParameter, "path is missing");
        };
        let Some(found) = crate::script::resolve(&r.bot.paths.scripts_dir(), Path::new(&path))
        else {
            return error(Code::NoScript, format!("no script at {path}"));
        };
        edited(r.bot, r.bot.scheduler.add(&found))
    }),
    Route::post("/scheduler/remove", |r| match r.query("id") {
        Some(id) => edited(r.bot, r.bot.scheduler.remove(&id)),
        None => error(Code::MissingParameter, "id is missing"),
    }),
    Route::post("/scheduler/move", |r| {
        let (Some(id), Ok(Some(by))) = (r.query("id"), r.number::<isize>("by")) else {
            return error(
                Code::MissingParameter,
                "id or by is missing (by: -1 moves up, 1 down)",
            );
        };
        edited(r.bot, r.bot.scheduler.move_by(&id, by.signum()))
    }),
    Route::post("/scheduler/rename", |r| {
        let (Some(id), Some(name)) = (r.query("id"), r.query("name")) else {
            return error(Code::MissingParameter, "id or name is missing");
        };
        r.bot.scheduler.rename(&id, &name);
        ok(scheduler(r.bot))
    }),
    Route::post("/scheduler/clear", |r| {
        edited(r.bot, r.bot.scheduler.clear())
    }),
    Route::post("/scheduler/load", load_playlist),
    Route::post("/scheduler/save", |r| {
        let Some(path) = r.query("path") else {
            return error(Code::MissingParameter, "path is missing");
        };
        let file = match playlist_file(r.bot, &path) {
            Ok(file) => file,
            Err(e) => return e,
        };
        if let Some(dir) = file.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            return error(
                Code::Internal,
                format!("couldn't make {}: {e}", dir.display()),
            );
        }
        match r.bot.scheduler.save(&file, &r.bot.paths.scripts_dir()) {
            Ok(()) => {
                let mut state = scheduler(r.bot);
                state["saved"] = json!(file);
                ok(state)
            }
            Err(e) => error(Code::SchedulerRefused, e),
        }
    }),
    Route::both("/scheduler/options", entry_options, entry_options),
    Route::post("/scheduler/start", |r| match r.bot.scheduler.start(r.bot) {
        Ok(()) => ok(scheduler(r.bot)),
        Err(e) => error(Code::SchedulerRefused, e),
    }),
    Route::post("/scheduler/stop", |r| {
        r.bot.scheduler.stop(r.bot);
        ok(scheduler(r.bot))
    }),
    Route::get("/butler", |r| ok(butler_state(r.bot))),
    Route::post("/butler/summon", |r| match r.query("user") {
        Some(user) => butler_reply(r.bot, butler::summon(r.bot, &user)),
        None => error(Code::MissingParameter, "user is missing"),
    }),
    Route::post("/butler/release", |r| match r.query("user") {
        Some(user) => butler_reply(r.bot, butler::release(r.bot, &user)),
        None => error(Code::MissingParameter, "user is missing"),
    }),
    Route::post("/butler/summon-all", |r| {
        butler_reply(r.bot, butler::summon_all(r.bot))
    }),
    Route::post("/butler/release-all", |r| {
        butler::release_all(r.bot);
        butler_reply(r.bot, Ok("Released all followers.".into()))
    }),
    Route::post("/butler/follow", |r| match r.query("master") {
        Some(master) => {
            let result =
                butler::start_follow(r.bot, &master).map(|()| format!("Following {master}."));
            butler_reply(r.bot, result)
        }
        None => error(Code::MissingParameter, "master is missing"),
    }),
    Route::post("/butler/unfollow", |r| {
        butler::stop_follow(r.bot);
        butler_reply(r.bot, Ok("Stopped following.".into()))
    }),
];

fn route(bot: &Arc<Bot>, method: &str, path_and_query: &str, body: &[u8]) -> Reply {
    let Ok(url) = reqwest::Url::parse(&format!("http://turu{path_and_query}")) else {
        return error(Code::InvalidParameter, "bad URL");
    };
    let Some(route) = ROUTES.iter().find(|r| r.path == url.path()) else {
        return error(Code::NoRoute, "no such route");
    };
    if let Some((name, value)) = url.query_pairs().find(|(name, value)| {
        FLAGS.contains(&name.as_ref())
            && !matches!(value.as_ref(), "" | "0" | "1" | "false" | "true")
    }) {
        return error(
            Code::InvalidParameter,
            format!("{name} is 1 or true, 0 or false, not {value}"),
        );
    }
    let handler = match method {
        "GET" => route.get,
        "POST" => route.post,
        _ => None,
    };
    match handler {
        Some(handler) => handler(&Req {
            bot,
            url,
            post: method == "POST",
            body,
        }),
        None => wrong_method(route.allow()),
    }
}

fn update(r: &Req) -> Reply {
    if crate::updates::available().is_none() {
        return error(Code::NoUpdate, "no update available");
    }
    if r.bot.scripts.is_running() && !r.flag("stop") {
        return error(
            Code::ScriptRunning,
            "a script is running (stop=1 to update anyway)",
        );
    }
    match crate::updates::installer() {
        None => error(Code::NotSupported, "this client can't update itself"),
        Some(install) => match install() {
            Ok(message) => Reply {
                status: 202,
                ..ok(json!({ "updating": message }))
            },
            Err(e) => error(Code::UpdateFailed, e),
        },
    }
}

fn start_script(r: &Req) -> Reply {
    let bot = r.bot;
    if let Some(path) = r.query("path")
        && let Err(e) = load(bot, &path)
    {
        return e;
    }
    let started = if r.flag("configure") {
        crate::script::start(bot)
    } else {
        crate::script::start_silent(bot)
    };
    match started {
        Ok(()) => {
            let mut state = script_state(bot);
            state["options"] = json!(bot.scripts.options_source().label());
            state["optionsWindow"] =
                json!(bot.scripts.has_options() && bot.scripts.may_show_options());
            ok(state)
        }
        Err(e) => error(Code::ScriptRefused, e),
    }
}

fn set_script_options(r: &Req) -> Reply {
    let bot = r.bot;
    let ScriptOptionsState::Loaded(options) = bot.scripts.options() else {
        return script_options(bot.scripts.loaded().as_deref(), bot.scripts.options());
    };
    let edited = match edit_options(options, r.flag("reset"), r.body) {
        Ok(edited) => edited,
        Err(e) => return error(Code::InvalidBody, e),
    };
    match bot.scripts.save_options(edited) {
        Ok(()) => script_options(bot.scripts.loaded().as_deref(), bot.scripts.options()),
        Err(e) => error(Code::OptionsRefused, e),
    }
}

fn pause(r: &Req, paused: bool) -> Reply {
    if !r.bot.scripts.is_running() {
        return error(Code::NoScriptRunning, "no script is running");
    }
    crate::script::set_paused(r.bot, paused);
    ok(script_state(r.bot))
}

fn log(r: &Req) -> Reply {
    let kind = match r.query("type") {
        None => LogKind::Script,
        Some(name) => match log_kind(&name) {
            Some(kind) => kind,
            None => return error(Code::InvalidParameter, format!("no log named {name}")),
        },
    };
    let since = match r.number("since") {
        Ok(since) => since.unwrap_or(0),
        Err(e) => return e,
    };
    let (lines, next) = r.bot.logs.get(kind).since(since);
    let lines: Vec<Value> = lines
        .iter()
        .map(|l| json!({ "time": l.time.to_rfc3339(), "text": l.text }))
        .collect();
    ok(json!({ "lines": lines, "next": next }))
}

/// `/login` (`fresh`) and `/relogin`; one login at a time, an auto-relogin's included.
fn login(r: &Req, fresh: bool) -> Reply {
    let bot = r.bot;
    if bot.servers.is_relogging() {
        return error(Code::LoginUnderWay, "a login is under way");
    }
    let ctx = servers::Ctx::new(bot);
    let server = r.query("server");
    let result = if fresh {
        servers::ensure_relogin(&ctx, server.as_deref())
    } else {
        match server {
            Some(name) => servers::relogin_named(&ctx, &name),
            None => servers::relogin(&ctx, None),
        }
    };
    match result {
        Ok(playing) => {
            let mut state = status(bot);
            state["playing"] = json!(playing);
            ok(state)
        }
        Err(e) => bot_error(e),
    }
}

fn load_playlist(r: &Req) -> Reply {
    let bot = r.bot;
    let scripts = bot.paths.scripts_dir();
    let loaded = match r.query("path") {
        Some(path) => match playlist_file(bot, &path) {
            Ok(file) => bot.scheduler.load(&file, &scripts),
            Err(e) => return e,
        },
        None => match std::str::from_utf8(r.body) {
            Ok(text) => bot.scheduler.load_json(text, &scripts),
            Err(e) => Err(e.to_string()),
        },
    };
    match loaded {
        Ok(_) => ok(scheduler(bot)),
        Err(e) if bot.scheduler.is_running() => error(Code::SchedulerRefused, e),
        Err(e) => error(Code::InvalidBody, e),
    }
}

/// A playlist file the API may read or write: relative to the `playlists` folder, or absolute
/// inside the data folder; no `..`. 403 otherwise.
fn playlist_file(bot: &Bot, path: &str) -> Result<PathBuf, Reply> {
    let path = Path::new(path);
    let outside = || {
        error(
            Code::PathOutsideData,
            "playlists are read and saved inside the data folder only",
        )
    };
    if path.components().any(|c| c == Component::ParentDir) {
        return Err(outside());
    }
    if path.is_relative() {
        return Ok(bot.paths.playlists_dir().join(path));
    }
    if path.starts_with(&bot.paths.root) {
        Ok(path.to_owned())
    } else {
        Err(outside())
    }
}

/// `/scheduler/options`: a playlist entry's options, read (`GET`) or set (`POST`).
fn entry_options(r: &Req) -> Reply {
    let bot = r.bot;
    let Some(id) = r.query("id") else {
        return error(Code::MissingParameter, "id is missing");
    };
    let Some(entry) = bot.scheduler.entries().into_iter().find(|e| e.id == id) else {
        return error(Code::NoEntry, format!("no playlist entry {id}"));
    };
    let state = bot
        .scripts
        .read_options(&entry.path, Some(&entry.storage()));
    if !r.post {
        return script_options(Some(&entry.path), state);
    }
    let ScriptOptionsState::Loaded(options) = state else {
        return script_options(Some(&entry.path), state);
    };
    let edited = match edit_options(options, r.flag("reset"), r.body) {
        Ok(edited) => edited,
        Err(e) => return error(Code::InvalidBody, e),
    };
    match edited.save() {
        Ok(()) => script_options(Some(&entry.path), ScriptOptionsState::Loaded(edited)),
        Err(e) => error(Code::Internal, format!("couldn't save the options: {e}")),
    }
}

/// Moves once logged in (409 before), and answers where the player is now: `map`, `cell`, `pad`.
fn in_game(bot: &Arc<Bot>, action: impl FnOnce(&Bot) -> crate::BotResult<()>) -> Reply {
    match player::logged_in(bot) {
        Ok(true) => {}
        Ok(false) => return error(Code::NotLoggedIn, "not logged in"),
        Err(e) => return bot_error(e),
    }
    let place = action(bot).and_then(|()| {
        Ok(json!({
            "map": map::name(bot)?,
            "cell": player::cell(bot)?,
            "pad": player::pad(bot)?,
        }))
    });
    place.map_or_else(bot_error, ok)
}

/// `/auto/attack` (VibeSkua's "Start Auto-Attack for All", `WM_SKUA_AUTO_ATTACK`) and
/// `/auto/hunt`: what the client's Auto popup's buttons do, refused where they're greyed out.
fn start_auto(r: &Req, auto_mode: AutoMode) -> Reply {
    let bot = r.bot;
    let (class, mode) = (r.query("class"), r.query("mode"));
    let map_ids = r
        .query("maps")
        .map_or_else(Vec::new, |ids| auto::parse_map_ids(&ids));
    if let Some(why) = auto::blocked(bot) {
        return error(Code::AutoRefused, why);
    }
    if !player::logged_in(bot).unwrap_or(false) {
        return error(Code::NotLoggedIn, "not logged in");
    }
    let mode = mode.unwrap_or_else(|| "Base".into());
    if !auto::start(bot, auto_mode, class, mode, map_ids) {
        return error(
            Code::AutoRefused,
            auto::blocked(bot).unwrap_or_else(|| "couldn't start".into()),
        );
    }
    ok(json!({ "auto": auto_mode.label() }))
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
    let name = player::username(bot)?;
    send::packet(bot, &format!("%xt%zm%house%1%{name}%"), "String")
}

/// The script: what `/status` has as `script`, `/events` sends as `script`, and the `/script/*`
/// commands answer. No game call.
fn script_state(bot: &Bot) -> Value {
    json!({
        "path": bot.scripts.loaded(),
        "status": bot.scripts.status().label(),
        "running": bot.scripts.is_running(),
        // Scripts > Scheduler: a stop here moves its playlist on to the next script.
        "scheduler": {
            "running": bot.scheduler.is_running(),
            "summary": bot.scheduler.summary(),
        },
    })
}

fn status(bot: &Bot) -> Value {
    let script = script_state(bot);
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
        "auto": bot.auto.mode().map(AutoMode::label),
        "stats": stats,
        // A newer Turu (`crate::updates`), or null.
        "update": crate::updates::status_json(),
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

/// `GET /scheduler`: the playlist.
fn scheduler(bot: &Bot) -> Value {
    let scheduler = &bot.scheduler;
    let entries: Vec<Value> = scheduler
        .entries()
        .iter()
        .map(|e| {
            json!({
                "id": e.id,
                "name": e.name,
                "path": e.path,
                "status": e.status.label(),
                "seconds": e.duration.map(|d| d.as_secs()),
            })
        })
        .collect();
    json!({
        "running": scheduler.is_running(),
        "summary": scheduler.summary(),
        "current": scheduler.current(),
        "entries": entries,
    })
}

/// `GET /butler`: this account's Butler and the other online accounts. Reads files and state
/// only, no game call.
fn butler_state(bot: &Bot) -> Value {
    let b = &bot.butler;
    let following = b.following().map(|(master, source)| {
        json!({
            "master": master,
            "summoned": source == butler::FollowSource::Ordered,
            "parked": b.is_parked(),
        })
    });
    let accounts: Vec<Value> = b
        .roster()
        .iter()
        .map(|d| {
            json!({
                "username": d.username,
                "server": d.server,
                "map": d.map_with_room(),
                "cell": d.cell,
                "class": d.class_name,
                "attacking": d.attacking,
                "following": d.following,
                "followers": d.followers,
                "script": d.script_on.then_some(&d.script_name),
                "parking": d.parking,
                "summoned": b.followers().iter().any(|f| f.eq_ignore_ascii_case(&d.username)),
                "unavailable": butler::unavailable(bot, d),
            })
        })
        .collect();
    json!({
        "enabled": b.enabled(),
        "username": b.username(),
        "server": b.server(),
        "following": following,
        "followers": b.followers(),
        "suspended": b.suspend_notice().map(|n| n.master),
        "accounts": accounts,
    })
}

/// Butler's message and state after a command, or 409 with its refusal.
fn butler_reply(bot: &Bot, result: Result<String, String>) -> Reply {
    match result {
        Ok(message) => {
            let mut state = butler_state(bot);
            state["message"] = json!(message);
            ok(state)
        }
        Err(e) => error(Code::ButlerRefused, e),
    }
}

/// The playlist after an edit, or 409 (it runs).
/// `/script/options`' answer: the script, where its values come from (`options`: `none`,
/// `saved`, `defaults`) and, when it declares options, `ScriptOptions::to_json`'s fields.
/// 409 when they couldn't be read.
fn script_options(script: Option<&Path>, state: ScriptOptionsState) -> Reply {
    let script = script.map(|p| p.to_string_lossy().into_owned());
    match state {
        ScriptOptionsState::NoOptions => ok(json!({
            "script": script,
            "options": OptionsSource::None.label(),
            "categories": [],
        })),
        ScriptOptionsState::Failed(e) => error(
            Code::OptionsUnreadable,
            format!("the script's options couldn't be read: {e}"),
        ),
        ScriptOptionsState::Loaded(options) => {
            let source = if options.saved() {
                OptionsSource::Saved
            } else {
                OptionsSource::Defaults
            };
            let mut body = options.to_json();
            body["script"] = json!(script);
            body["options"] = json!(source.label());
            ok(body)
        }
    }
}

/// The options with `body`'s values (`ScriptOptions::apply_json`), after the defaults with
/// `reset`. An empty body sets nothing.
fn edit_options(
    mut options: ScriptOptions,
    reset: bool,
    body: &[u8],
) -> Result<ScriptOptions, String> {
    if reset {
        options.reset();
    }
    if body.iter().any(|b| !b.is_ascii_whitespace()) {
        let values: Value =
            serde_json::from_slice(body).map_err(|e| format!("the body isn't JSON: {e}"))?;
        options.apply_json(&values)?;
    }
    Ok(options)
}

fn edited(bot: &Bot, result: Result<(), String>) -> Reply {
    match result {
        Ok(()) => ok(scheduler(bot)),
        Err(e) => error(Code::SchedulerRefused, e),
    }
}

/// Loads a script: absolute, or relative to the scripts folder (else to the working folder),
/// `.ts` optional.
fn load(bot: &Bot, path: &str) -> Result<PathBuf, Reply> {
    let Some(path) = crate::script::resolve(&bot.paths.scripts_dir(), Path::new(path)) else {
        return Err(error(Code::NoScript, format!("no script at {path}")));
    };
    if bot.scripts.is_running() {
        return Err(error(
            Code::ScriptRunning,
            "a script is running; stop it first",
        ));
    }
    bot.scripts.load(&path);
    Ok(path)
}

fn set_options(bot: &Bot, body: &[u8]) -> Reply {
    let Ok(Value::Object(values)) = serde_json::from_slice::<Value>(body) else {
        return error(
            Code::InvalidBody,
            "the body must be a JSON object of option names and values",
        );
    };
    let mut options = bot.opts().clone();
    for (name, value) in values {
        if let Err(e) = options.set(&name, value) {
            return error(Code::InvalidBody, e);
        }
    }
    *bot.options.write() = options;
    ok(serde_json::to_value(&*bot.opts()).unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module's table lists every code with its status, and nothing else.
    #[test]
    fn documents_every_error_code() {
        let source = include_str!("control_api.rs");
        let rows: Vec<&str> = source
            .lines()
            // `//! | `code` | status | when |`
            .filter(|l| l.starts_with("//! | `") && l.matches(" | ").count() == 3)
            .filter(|l| {
                l.split(" | ")
                    .nth(2)
                    .is_some_and(|s| s.parse::<u16>().is_ok())
            })
            .collect();
        assert_eq!(rows.len(), Code::ALL.len(), "{rows:#?}");
        for code in Code::ALL {
            let row = format!("//! | `{}` | {} |", code.name(), code.status());
            assert!(rows.iter().any(|r| r.starts_with(&row)), "{row}");
        }
    }

    #[test]
    fn compares_tokens() {
        assert!(same(b"abcdef", b"abcdef"));
        assert!(!same(b"abcdef", b"abcdeg"));
        assert!(!same(b"abc", b"abcdef"));
        assert_eq!(new_token().unwrap().len(), 64);
    }
}
