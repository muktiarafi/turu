//! Turu: the disk cache for the files the game downloads (ruffle-aqw's `ruffle_http_cache`): one
//! for the process, in `<data>/cache/gamefiles`, which other clients (an army's) share too. AQW's
//! CDN says how long each file may be kept; see docs/STATUS.md, "Game files are cached".
//!
//! With Ruffle, every tab's navigator uses it (`ExternalNavigatorBackend::with_http_cache`).
//! Turu Mas, whose game runs in Adobe AIR, fetches the game's files through [`fetch`] instead of
//! AIR's own downloads, which keep nothing.

use ruffle_http_cache::{Entry, HttpCache, Policy};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use url::Url;

pub use ruffle_http_cache::CacheStats;

/// The folder's size limit unless `TURU_HTTP_CACHE_MB` says otherwise. AQW's own files are a
/// few hundred MB for the maps and gear of a long farm.
const DEFAULT_MB: u64 = 1024;

/// The cache to give a navigator (`ExternalNavigatorBackend::with_http_cache`). `None` when
/// `TURU_HTTP_CACHE=0` or the folder can't be made: every file then comes from the network.
pub fn cache() -> Option<Arc<HttpCache>> {
    static CACHE: OnceLock<Option<Arc<HttpCache>>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            if std::env::var("TURU_HTTP_CACHE").is_ok_and(|v| v.trim() == "0") {
                tracing::info!("Game file cache: off (TURU_HTTP_CACHE=0)");
                return None;
            }
            let megabytes = std::env::var("TURU_HTTP_CACHE_MB")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|mb| *mb > 0)
                .unwrap_or(DEFAULT_MB);
            let dir = turu_bot::paths::Paths::new().game_files_cache_dir();
            match HttpCache::new(&dir, megabytes * 1024 * 1024) {
                Ok(cache) => Some(cache),
                Err(error) => {
                    tracing::warn!("Game file cache: can't use {}: {error}", dir.display());
                    None
                }
            }
        })
        .clone()
}

/// What the cache has done since the start, for the perf log: `None` while it is off.
pub fn summary() -> Option<String> {
    let cache = cache()?;
    let stats = cache.stats();
    Some(format!(
        "{} from disk, {} more after asking the server, {} downloaded and kept; {:.1} MB not downloaded",
        stats.hits,
        stats.revalidated,
        stats.stored,
        stats.bytes_saved as f64 / (1024.0 * 1024.0)
    ))
}

/// Downloads `url` (a plain `GET`) through the cache, as Ruffle's navigator does: a fresh copy
/// comes from disk with no request; a stale one, or one kept with another query string, is
/// checked with the server (`If-None-Match`), which answers `304` without a body when nothing
/// changed; what the server allows keeping is kept. With the network down, a kept copy is used.
/// Blocks: call it off any thread that draws or runs the game.
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
    let client = CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .user_agent(turu_bot::GAME_USER_AGENT)
                .cookie_store(true)
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(180))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()?;
    let body = fetch_with(client, cache().as_deref(), url)?;
    if is_swf(&body)
        && let Ok(url) = Url::parse(url)
    {
        remember(&mut served(), url);
    }
    Ok(body)
}

/// How many of the SWFs [`fetch`] served are remembered (a map's own files are a handful).
const SERVED_KEPT: usize = 64;

/// The SWFs [`fetch`] served, by URL, newest last.
fn served() -> std::sync::MutexGuard<'static, VecDeque<Url>> {
    static SERVED: Mutex<VecDeque<Url>> = Mutex::new(VecDeque::new());
    SERVED.lock().unwrap_or_else(|e| e.into_inner())
}

fn is_swf(body: &[u8]) -> bool {
    matches!(body.get(..3), Some(b"FWS" | b"CWS" | b"ZWS"))
}

fn remember(served: &mut VecDeque<Url>, url: Url) {
    served.retain(|kept| *kept != url);
    if served.len() == SERVED_KEPT {
        served.pop_front();
    }
    served.push_back(url);
}

/// The newest URL in `served` whose path ends in `path` (a map file such as
/// `ChiralValley/town-Mobius-21Feb14.swf`), as the Ruffle host finds a loaded movie.
fn find_served(served: &VecDeque<Url>, path: &str) -> Option<Url> {
    let suffix = format!("/{}", path.trim_start_matches('/'));
    served
        .iter()
        .rev()
        .find(|url| url.path().ends_with(&suffix))
        .cloned()
}

/// A SWF [`fetch`] served whose URL path ends in `path`, read back from the disk cache and
/// written out uncompressed (`FlashHost::loaded_swf` for Turu Mas, whose game loads through
/// [`fetch`]). `None` when none was served, or the cache is off or no longer has it: the bot then
/// downloads the file itself.
pub fn served_swf(path: &str) -> Option<Vec<u8>> {
    let url = find_served(&served(), path)?;
    let entry = cache()?.lookup(&url)?;
    crate::map_pads::uncompressed_swf(&entry.body)
}

fn fetch_with(
    client: &reqwest::blocking::Client,
    cache: Option<&HttpCache>,
    url: &str,
) -> Result<Vec<u8>, String> {
    let url = Url::parse(url).map_err(|e| format!("{url}: {e}"))?;
    let Some(cache) = cache.filter(|_| matches!(url.scheme(), "http" | "https")) else {
        let response = client
            .get(url.clone())
            .send()
            .map_err(|e| format!("{url}: {e}"))?;
        return read_body(&url, response);
    };
    let cached = cache.lookup(&url);
    if let Some(entry) = cached.as_ref().filter(|entry| entry.meta.is_fresh(&url)) {
        cache.count_served(entry, false);
        return Ok(cached.map(|entry| entry.body).unwrap_or_default());
    }

    let mut request = client.get(url.clone());
    if let Some(entry) = &cached {
        if !entry.meta.etag.is_empty() {
            request = request.header(reqwest::header::IF_NONE_MATCH, &entry.meta.etag);
        }
        if !entry.meta.last_modified.is_empty() {
            request = request.header(
                reqwest::header::IF_MODIFIED_SINCE,
                &entry.meta.last_modified,
            );
        }
    }
    let served = |entry: Entry| {
        cache.count_served(&entry, true);
        entry.body
    };
    let response = match (request.send(), cached) {
        (Err(e), Some(entry)) => {
            tracing::warn!("Couldn't check {url} with the server ({e}); using the cached file");
            return Ok(served(entry));
        }
        (Ok(response), Some(entry)) if response.status() == reqwest::StatusCode::NOT_MODIFIED => {
            // Unchanged: fresh again, for this query string.
            if let Some(policy) = Policy::of_revalidation(response.headers()) {
                cache.refresh(&url, &entry.meta, &policy);
            }
            return Ok(served(entry));
        }
        (response, _) => response.map_err(|e| format!("{url}: {e}"))?,
    };
    let redirected = *response.url() != url;
    let policy = (response.status() == reqwest::StatusCode::OK && !redirected)
        .then(|| Policy::of_response(response.headers()))
        .flatten();
    let body = read_body(&url, response)?;
    // A connection that closed early gives a short body without an error.
    if let Some(policy) =
        policy.filter(|policy| policy.len.is_none_or(|len| len == body.len() as u64))
    {
        cache.store(&url, &policy, &body);
    }
    Ok(body)
}

fn read_body(url: &Url, response: reqwest::blocking::Response) -> Result<Vec<u8>, String> {
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{url}: HTTP {status}"));
    }
    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|e| format!("{url}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// A server with one game file, as AQW's CDN serves them: `304` to a request with its
    /// `ETag`. Counts the requests and the bodies it sent; with `down` set it closes each
    /// connection without an answer.
    struct Server {
        port: u16,
        requests: Arc<Mutex<Vec<String>>>,
        bodies: Arc<AtomicUsize>,
        down: Arc<AtomicBool>,
    }

    impl Server {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let bodies = Arc::new(AtomicUsize::new(0));
            let down = Arc::new(AtomicBool::new(false));
            let (seen, sent, refuse) = (requests.clone(), bodies.clone(), down.clone());
            std::thread::spawn(move || {
                for socket in listener.incoming() {
                    let Ok(mut socket) = socket else { return };
                    if refuse.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut request = Vec::new();
                    let mut buffer = [0; 1024];
                    while !request.ends_with(b"\r\n\r\n") {
                        match socket.read(&mut buffer) {
                            Ok(n) if n > 0 => request.extend_from_slice(&buffer[..n]),
                            _ => break,
                        }
                    }
                    let request = String::from_utf8_lossy(&request).to_lowercase();
                    let first = request.lines().next().unwrap_or_default().to_string();
                    let headers = if first.contains("/api/") {
                        "cache-control: max-age=2\r\nconnection: close\r\n"
                    } else {
                        "cache-control: public, max-age=2678400\r\netag: \"v1\"\r\nconnection: close\r\n"
                    };
                    let response = if request.contains("if-none-match: \"v1\"") {
                        format!("HTTP/1.1 304 Not Modified\r\n{headers}\r\n")
                    } else {
                        sent.fetch_add(1, Ordering::Relaxed);
                        format!("HTTP/1.1 200 OK\r\n{headers}content-length: 5\r\n\r\nhello")
                    };
                    seen.lock().unwrap().push(first);
                    let _ = socket.write_all(response.as_bytes());
                }
            });
            Self {
                port,
                requests,
                bodies,
                down,
            }
        }

        fn counts(&self) -> (usize, usize) {
            (
                self.requests.lock().unwrap().len(),
                self.bodies.load(Ordering::Relaxed),
            )
        }
    }

    struct TempDir(std::path::PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn fetch_answers_from_disk_and_revalidates() {
        let server = Server::start();
        let dir =
            TempDir(std::env::temp_dir().join(format!("turu-game-files-{}", std::process::id())));
        let cache = HttpCache::new(&dir.0, 1 << 30).unwrap();
        let client = reqwest::blocking::Client::new();
        let file = format!(
            "http://127.0.0.1:{}/gamefiles/mon/Frogzard.swf",
            server.port
        );
        let get = |url: &str| fetch_with(&client, Some(&cache), url);

        // Downloaded and kept.
        assert_eq!(get(&format!("{file}?v=5.32")).unwrap(), b"hello");
        assert_eq!(server.counts(), (1, 1));
        assert_eq!(cache.stats().stored, 1);

        // The same URL: from disk, the server isn't asked.
        assert_eq!(get(&format!("{file}?v=5.32")).unwrap(), b"hello");
        assert_eq!(server.counts(), (1, 1));
        assert_eq!(cache.stats().hits, 1);

        // Another query (a game update): asked, `304`, no body; then fresh for that query.
        assert_eq!(get(&format!("{file}?v=5.33")).unwrap(), b"hello");
        assert_eq!(server.counts(), (2, 1));
        assert_eq!(get(&format!("{file}?v=5.33")).unwrap(), b"hello");
        assert_eq!(server.counts(), (2, 1));
        assert_eq!(cache.stats().revalidated, 1);

        // An answer the server doesn't let anyone keep is fetched every time.
        let api = format!("http://127.0.0.1:{}/api/data", server.port);
        assert_eq!(get(&api).unwrap(), b"hello");
        assert_eq!(get(&api).unwrap(), b"hello");
        assert_eq!(server.counts(), (4, 3));
        assert_eq!(cache.stats().stored, 1);

        // The server is down: a kept file that would be checked is used, others fail.
        server.down.store(true, Ordering::Relaxed);
        assert_eq!(get(&format!("{file}?v=5.34")).unwrap(), b"hello");
        assert_eq!(cache.stats().revalidated, 2);
        let other = format!("http://127.0.0.1:{}/gamefiles/mon/Other.swf", server.port);
        assert!(get(&other).is_err());
    }

    #[test]
    fn finds_the_newest_served_swf_by_its_path() {
        let url = |s: &str| Url::parse(s).unwrap();
        let mut served = VecDeque::new();
        remember(
            &mut served,
            url("https://game.aq.com/game/gamefiles/maps/Town/town-1.swf?v=1"),
        );
        remember(
            &mut served,
            url("https://cdn.aq.com/game/gamefiles/maps/Town/town-1.swf?v=2"),
        );
        remember(
            &mut served,
            url("https://game.aq.com/game/gamefiles/maps/Other/mytown-1.swf"),
        );
        let found = |path: &str| find_served(&served, path).map(|url| url.to_string());
        assert_eq!(
            found("Town/town-1.swf").as_deref(),
            Some("https://cdn.aq.com/game/gamefiles/maps/Town/town-1.swf?v=2")
        );
        assert_eq!(found("/Town/town-1.swf"), found("Town/town-1.swf"));
        // A whole file name, not the end of another's.
        assert_eq!(found("town-1.swf"), found("Town/town-1.swf"));
        assert_eq!(found("own-1.swf"), None);
        assert_eq!(found("Nowhere/town-1.swf"), None);

        // Served again: newest; the oldest go past the limit.
        remember(
            &mut served,
            url("https://game.aq.com/game/gamefiles/maps/Town/town-1.swf?v=1"),
        );
        assert_eq!(served.len(), 3);
        for n in 0..SERVED_KEPT {
            remember(&mut served, url(&format!("https://game.aq.com/x/{n}.swf")));
        }
        assert_eq!(served.len(), SERVED_KEPT);
        assert_eq!(find_served(&served, "Town/town-1.swf"), None);
    }

    #[test]
    fn without_a_cache_it_just_downloads() {
        let server = Server::start();
        let client = reqwest::blocking::Client::new();
        let file = format!("http://127.0.0.1:{}/gamefiles/a.swf", server.port);
        assert_eq!(fetch_with(&client, None, &file).unwrap(), b"hello");
        assert_eq!(fetch_with(&client, None, &file).unwrap(), b"hello");
        assert_eq!(server.counts(), (2, 2));
    }
}
