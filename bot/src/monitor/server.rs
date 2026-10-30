//! The web monitor's HTTP server: the page and the bots' snapshots, read only.
//!
//! | Route | What |
//! |---|---|
//! | `GET /` | The page (`bot/assets/monitor/index.html`). |
//! | `GET /api/state` | Every bot's latest snapshot, plus Turu's version and uptime. |
//! | `GET /api/events` | Server-Sent Events: the same state as an `event: state` every second. |
//!
//! The `api` routes need the key (`Authorization: Bearer <key>`), except for a browser on this
//! computer: a loopback connection whose `Host` is a loopback name. Tailscale Serve and other
//! proxies connect from loopback too, but with their own host name, so they need the key; so
//! does a page that rebinds its DNS name to 127.0.0.1.

use super::{Snapshot, Watching, touch};
use crate::Bot;
use crate::control_api::{authorized, saved_token};
use crate::paths::Paths;
use http_body_util::channel::Channel;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::{CACHE_CONTROL, CONTENT_TYPE, HOST, HeaderMap, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use parking_lot::RwLock;
use serde::Serialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const PAGE: &str = include_str!("../../assets/monitor/index.html");
const STREAM_EVERY: Duration = Duration::from_secs(1);
/// How often the accept loop and the streams look at the stop flag.
const STOP_CHECK: Duration = Duration::from_millis(250);

/// A bot the page shows.
pub struct Source {
    /// Stays the same while the bot exists (`tab:3`).
    pub id: String,
    /// What the page calls it ("Tab 2").
    pub label: String,
    pub bot: Arc<Bot>,
}

/// The bots there are now; the app answers (its tabs).
pub type Sources = Arc<dyn Fn() -> Vec<Source> + Send + Sync>;

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
    bots: Vec<BotEntry>,
}

#[derive(Serialize)]
struct BotEntry {
    id: String,
    label: String,
    /// `None` until the first sample.
    snapshot: Option<Snapshot>,
}

struct Shared {
    key: Arc<Key>,
    sources: Sources,
    version: String,
    started: Instant,
    stop: AtomicBool,
}

impl Shared {
    fn state(&self) -> String {
        let bots = (self.sources)()
            .into_iter()
            .map(|s| BotEntry {
                id: s.id,
                label: s.label,
                snapshot: s.bot.monitor.latest(),
            })
            .collect();
        let state = State {
            version: self.version.clone(),
            uptime_seconds: self.started.elapsed().as_secs(),
            now: super::snapshot::now_ms(),
            bots,
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
    /// ([`new_key`]).
    pub fn start(
        addr: SocketAddr,
        key: Arc<Key>,
        sources: Sources,
        version: String,
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
            sources,
            version,
            started: Instant::now(),
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
                std::future::ready(Ok::<_, Infallible>(handle(shared.clone(), peer, request)))
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

fn handle(shared: Arc<Shared>, peer: SocketAddr, request: Request<Incoming>) -> Response<Body> {
    if request.method() != Method::GET {
        return error(StatusCode::METHOD_NOT_ALLOWED, "the monitor is read only");
    }
    let path = request.uri().path();
    if matches!(path, "/" | "/index.html") {
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
    if !matches!(path, "/api/state" | "/api/events") {
        return error(StatusCode::NOT_FOUND, "no such route");
    }
    let local = peer.ip().is_loopback() && loopback_host(request.headers());
    if !local && !authorized(request.headers(), &shared.key.get()) {
        return error(StatusCode::UNAUTHORIZED, "missing or wrong monitor key");
    }
    touch();
    if path == "/api/state" {
        return reply(StatusCode::OK, "application/json", full(shared.state()));
    }
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
        let mut next = Instant::now();
        while !shared.stop.load(Ordering::Relaxed) {
            if Instant::now() >= next {
                next = Instant::now() + STREAM_EVERY;
                let frame = format!("event: state\ndata: {}\n\n", shared.state());
                // Fails once the browser has gone.
                if tx.send_data(Bytes::from(frame)).await.is_err() {
                    return;
                }
            }
            tokio::time::sleep(STOP_CHECK.min(next.saturating_duration_since(Instant::now())))
                .await;
        }
    });
    reply(StatusCode::OK, "text/event-stream", body.boxed())
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
