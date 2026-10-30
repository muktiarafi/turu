//! The web monitor's HTTP server: the page, the hub's entries, and commands for them.
//!
//! | Route | What |
//! |---|---|
//! | `GET /` | The page (`bot/assets/monitor/index.html`). |
//! | `GET /api/state` | Every entry (latest snapshot, recent events), Turu's version and uptime, and whether control is allowed. |
//! | `GET /api/events` | Server-Sent Events: the same state as an `event: state` every second. |
//! | `GET /api/bots/<id>` | One entry with all its events and its history. |
//! | `GET /api/bots/<id>/log?type=script\|debug[&since=]` | Its log lines from number `since` (the control API's `/log`). |
//! | `POST /api/bots/<id>/script/start\|stop\|pause\|resume`, `/login`, `/logout`, `/relogin[?server=]`, `/map/join?map=[&cell=][&pad=]`, `/house` | Control: the control API's route of that name, for a tab or an Army client. |
//! | `POST /api/bots/<id>/forget` | Control: drops an offline entry. |
//!
//! The `api` routes need the key (`Authorization: Bearer <key>`), except for a browser on this
//! computer: a loopback connection whose `Host` is a loopback name. Tailscale Serve and other
//! proxies connect from loopback too, but with their own host name, so they need the key; so
//! does a page that rebinds its DNS name to 127.0.0.1. A POST also needs Allow Control, and
//! either an `Origin` that is the page's own (no other site can send one) or the key.

use super::hub::{Control, Hub};
use super::{Watching, touch};
use crate::control_api::{authorized, saved_token};
use crate::paths::Paths;
use http_body_util::channel::Channel;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, HOST, HeaderMap, HeaderValue, ORIGIN};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use serde::Serialize;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

const PAGE: &str = include_str!("../../assets/monitor/index.html");
const STREAM_EVERY: Duration = Duration::from_secs(1);
/// How often the accept loop and the streams look at the stop flag.
const STOP_CHECK: Duration = Duration::from_millis(250);
/// The control API routes a POST may name, after `/api/bots/<id>`.
const COMMANDS: [&str; 9] = [
    "/script/start",
    "/script/stop",
    "/script/pause",
    "/script/resume",
    "/login",
    "/logout",
    "/relogin",
    "/map/join",
    "/house",
];

/// The key other devices need; it can change while the server runs.
#[derive(Default)]
pub struct Key(RwLock<String>);

impl Key {
    pub fn new(key: String) -> Arc<Self> {
        Arc::new(Self(RwLock::new(key)))
    }

    pub fn get(&self) -> String {
        self.0.read().clone()
    }

    pub fn set(&self, key: String) {
        *self.0.write() = key;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct State {
    version: String,
    uptime_seconds: u64,
    /// The server's clock, for the page's "seconds ago".
    now: u64,
    allow_control: bool,
    bots: Vec<super::hub::View>,
}

struct Shared {
    key: Arc<Key>,
    hub: Arc<Hub>,
    version: String,
    allow_control: AtomicBool,
    stop: AtomicBool,
}

impl Shared {
    fn state(&self) -> String {
        let state = State {
            version: self.version.clone(),
            uptime_seconds: self.hub.uptime().as_secs(),
            now: super::snapshot::now_ms(),
            allow_control: self.allow_control.load(Ordering::Relaxed),
            bots: self.hub.views(),
        };
        serde_json::to_string(&state).unwrap_or_else(|_| "{}".into())
    }
}

/// The running server; stops when dropped.
pub struct Server {
    addr: SocketAddr,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Listens on `addr` on a thread of its own. `key` can change while it runs
    /// ([`new_key`]), and so can whether commands are allowed ([`Server::set_allow_control`]).
    pub fn start(
        addr: SocketAddr,
        key: Arc<Key>,
        hub: Arc<Hub>,
        version: String,
        allow_control: bool,
    ) -> Result<Self, String> {
        if key.get().len() < 16 {
            return Err("the monitor key must be at least 16 characters".into());
        }
        let listener = std::net::TcpListener::bind(addr)
            .map_err(|e| format!("couldn't listen on {addr}: {e}"))?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let shared = Arc::new(Shared {
            key,
            hub,
            version,
            allow_control: AtomicBool::new(allow_control),
            stop: AtomicBool::new(false),
        });
        let serving = shared.clone();
        let thread = std::thread::Builder::new()
            .name("Turu Monitor Server".into())
            .spawn(move || runtime.block_on(serve(listener, serving)))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            addr,
            shared,
            thread: Some(thread),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn set_allow_control(&self, allow: bool) {
        self.shared.allow_control.store(allow, Ordering::Relaxed);
    }
}

impl Drop for Server {
    /// Closes the listener and every stream before returning, so the port can be used again.
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The key in the data folder's `monitor-key`, made the first time.
pub fn load_key(paths: &Paths) -> Result<String, String> {
    saved_token(&paths.monitor_key_file())
}

/// Replaces the key: every device has to be given the new one.
pub fn new_key(paths: &Paths) -> Result<String, String> {
    let file = paths.monitor_key_file();
    match std::fs::remove_file(&file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("{}: {e}", file.display()));
        }
        _ => {}
    }
    saved_token(&file)
}

async fn serve(listener: std::net::TcpListener, shared: Arc<Shared>) {
    let listener = match tokio::net::TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Monitor: {e}");
            return;
        }
    };
    while !shared.stop.load(Ordering::Relaxed) {
        let (stream, peer) = match tokio::time::timeout(STOP_CHECK, listener.accept()).await {
            Err(_) => continue,
            Ok(Ok(accepted)) => accepted,
            Ok(Err(e)) => {
                tracing::debug!("Monitor accept: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request| {
                let shared = shared.clone();
                async move { Ok::<_, Infallible>(handle(shared, peer, request).await) }
            });
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}

type Body = BoxBody<Bytes, Infallible>;

fn full(body: impl Into<Bytes>) -> Body {
    Full::new(body.into()).boxed()
}

fn reply(status: StatusCode, content_type: &'static str, body: Body) -> Response<Body> {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response
}

fn error(status: StatusCode, message: &str) -> Response<Body> {
    let body = serde_json::json!({ "error": message }).to_string();
    reply(status, "application/json", full(body))
}

async fn handle(
    shared: Arc<Shared>,
    peer: SocketAddr,
    request: Request<Incoming>,
) -> Response<Body> {
    let path = request.uri().path().to_owned();
    let post = request.method() == Method::POST;
    if !post && request.method() != Method::GET {
        return error(StatusCode::METHOD_NOT_ALLOWED, "GET or POST only");
    }
    if !post && matches!(path.as_str(), "/" | "/index.html") {
        let mut response = reply(StatusCode::OK, "text/html; charset=utf-8", full(PAGE));
        let headers = response.headers_mut();
        headers.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
                 img-src data:; connect-src 'self'; frame-ancestors 'none'",
            ),
        );
        headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
        return response;
    }
    if !path.starts_with("/api/") {
        return error(StatusCode::NOT_FOUND, "no such route");
    }
    let headers = request.headers();
    let keyed = authorized(headers, &shared.key.get());
    let local = peer.ip().is_loopback() && loopback_host(headers);
    if !keyed && !local {
        return error(StatusCode::UNAUTHORIZED, "missing or wrong monitor key");
    }
    touch();
    if post {
        if !shared.allow_control.load(Ordering::Relaxed) {
            return error(
                StatusCode::FORBIDDEN,
                "Allow Control is off in Options > Application",
            );
        }
        // Another site's page can POST here, but not with our Origin or the key.
        let same_origin = match headers.get(ORIGIN) {
            Some(origin) => same_origin(origin, headers),
            None => keyed,
        };
        if !same_origin {
            return error(
                StatusCode::FORBIDDEN,
                "commands come from the monitor's own page",
            );
        }
    }
    let query = request.uri().query().unwrap_or_default().to_owned();
    match (post, path.as_str()) {
        (false, "/api/state") => reply(StatusCode::OK, "application/json", full(shared.state())),
        (false, "/api/events") => events(shared),
        (_, path) => match path.strip_prefix("/api/bots/") {
            Some(rest) => bot_route(&shared, post, rest, &query).await,
            None => error(StatusCode::NOT_FOUND, "no such route"),
        },
    }
}

/// `GET /api/events`: the state every second until the browser goes.
fn events(shared: Arc<Shared>) -> Response<Body> {
    let (mut tx, body) = Channel::<Bytes, Infallible>::new(2);
    tokio::spawn(async move {
        let _watching = Watching::new();
        if tx
            .send_data(Bytes::from_static(b"retry: 3000\n\n"))
            .await
            .is_err()
        {
            return;
        }
        let mut next = tokio::time::Instant::now();
        while !shared.stop.load(Ordering::Relaxed) {
            if tokio::time::Instant::now() >= next {
                next = tokio::time::Instant::now() + STREAM_EVERY;
                let frame = format!("event: state\ndata: {}\n\n", shared.state());
                // Fails once the browser has gone.
                if tx.send_data(Bytes::from(frame)).await.is_err() {
                    return;
                }
            }
            let wait = next.saturating_duration_since(tokio::time::Instant::now());
            tokio::time::sleep(STOP_CHECK.min(wait)).await;
        }
    });
    reply(StatusCode::OK, "text/event-stream", body.boxed())
}

/// `/api/bots/<id>[/...]`.
async fn bot_route(shared: &Arc<Shared>, post: bool, rest: &str, query: &str) -> Response<Body> {
    let (id, tail) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let id = percent_decode(id);
    let hub = &shared.hub;
    match (post, tail) {
        (false, "") => match hub.detail(&id) {
            Some(detail) => json_reply(StatusCode::OK, &detail),
            None => error(StatusCode::NOT_FOUND, "no such bot"),
        },
        (true, "/forget") => {
            if hub.forget(&id) {
                json_reply(StatusCode::OK, &json!({ "forgotten": id }))
            } else {
                error(StatusCode::CONFLICT, "only an offline bot can be forgotten")
            }
        }
        (false, "/log") => {
            let kind = query_value(query, "type").unwrap_or_else(|| "script".into());
            if kind != "script" && kind != "debug" {
                return error(StatusCode::BAD_REQUEST, "type is script or debug");
            }
            let since = query_value(query, "since")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let route = format!("/log?type={kind}&since={since}");
            forward(hub, &id, false, route).await
        }
        (true, command) if COMMANDS.contains(&command) => {
            let route = if query.is_empty() {
                command.to_owned()
            } else {
                format!("{command}?{query}")
            };
            forward(hub, &id, true, route).await
        }
        _ => error(StatusCode::NOT_FOUND, "no such route"),
    }
}

/// Runs a control API route for the entry: in this process for a tab, over HTTP for an Army
/// client. Off the runtime's thread: a login can take a minute.
async fn forward(hub: &Arc<Hub>, id: &str, post: bool, route: String) -> Response<Body> {
    let Some(control) = hub.control(id) else {
        return error(StatusCode::NOT_FOUND, "that bot isn't running");
    };
    let result = tokio::task::spawn_blocking(move || match control {
        Control::Tab(bot) => crate::control_api::local(&bot, post, &route, b""),
        Control::Army(army, client) => match army.forward(&client, post, &route, Vec::new()) {
            Ok(value) => (200, value),
            Err(e) => (502, json!({ "error": e })),
        },
    })
    .await;
    match result {
        Ok((status, value)) => json_reply(
            StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY),
            &value,
        ),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

fn json_reply(status: StatusCode, value: &impl Serialize) -> Response<Body> {
    let body = serde_json::to_string(value).unwrap_or_else(|_| "{}".into());
    reply(status, "application/json", full(body))
}

fn query_value(query: &str, name: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| percent_decode(&v.replace('+', " ")))
        .filter(|v| !v.is_empty())
}

/// `%xx` escapes, as the page's `encodeURIComponent` makes them.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((hi * 16 + lo) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The `Origin` is this server as the browser reached it: `http(s)://` and the `Host`.
fn same_origin(origin: &HeaderValue, headers: &HeaderMap) -> bool {
    let (Ok(origin), Some(host)) = (
        origin.to_str(),
        headers.get(HOST).and_then(|h| h.to_str().ok()),
    ) else {
        return false;
    };
    let origin = origin.trim_end_matches('/');
    let host = host.to_ascii_lowercase();
    let origin = origin.to_ascii_lowercase();
    origin == format!("http://{host}")
        || origin == format!("https://{host}")
        // Tailscale Serve answers https on 443 and may pass the host with its port.
        || host.strip_suffix(":443").is_some_and(|h| origin == format!("https://{h}"))
}

/// The request names this computer (`127.0.0.1`, `localhost`, `[::1]`, with or without the
/// port).
fn loopback_host(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or_default(),
        None => host.split(':').next().unwrap_or_default(),
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        "127.0.0.1" | "localhost" | "::1"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn checks_the_origin() {
        let origin = |o: &str| HeaderValue::from_str(o).unwrap();
        assert!(same_origin(
            &origin("http://127.0.0.1:8790"),
            &host("127.0.0.1:8790")
        ));
        assert!(same_origin(
            &origin("https://pc.tail1.ts.net"),
            &host("pc.tail1.ts.net")
        ));
        assert!(!same_origin(
            &origin("https://evil.example"),
            &host("127.0.0.1:8790")
        ));
        assert!(!same_origin(
            &origin("http://127.0.0.1:8791"),
            &host("127.0.0.1:8790")
        ));
        assert!(!same_origin(&origin("null"), &host("127.0.0.1:8790")));
    }

    #[test]
    fn decodes_ids_and_queries() {
        assert_eq!(percent_decode("army%3Amy%20alt"), "army:my alt");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(
            query_value("map=battleon-1&cell=Enter", "cell").as_deref(),
            Some("Enter")
        );
        assert_eq!(query_value("map=a+b", "map").as_deref(), Some("a b"));
        assert_eq!(query_value("map=", "map"), None);
    }

    #[test]
    fn knows_loopback_hosts() {
        assert!(loopback_host(&host("127.0.0.1:8790")));
        assert!(loopback_host(&host("localhost")));
        assert!(loopback_host(&host("LOCALHOST:8790")));
        assert!(loopback_host(&host("[::1]:8790")));
        assert!(!loopback_host(&host("my-pc.tail1234.ts.net")));
        assert!(!loopback_host(&host("127.0.0.1.evil.example:8790")));
        assert!(!loopback_host(&HeaderMap::new()));
    }
}
