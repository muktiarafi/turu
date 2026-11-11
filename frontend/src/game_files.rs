//! Turu: the disk cache for the files the game downloads (ruffle-aqw's `HttpCache`): one for
//! the process, shared by every tab's navigator, in `<data>/cache/gamefiles`, which other
//! clients (an army's) share too. AQW's CDN says how long each file may be kept; see
//! docs/STATUS.md, "Game files are cached".

use ruffle_frontend_utils::backends::navigator::HttpCache;
use std::sync::{Arc, OnceLock};

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
