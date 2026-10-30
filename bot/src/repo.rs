//! Search Scripts: the script repository (VibeSkua `GetScriptsService`, and the Manager's
//! Update Scripts / Reset Scripts in `ScriptUpdaterViewModel`), for Turu's TypeScript scripts
//! in [`DEFAULT_REPO`] (`TURU_SCRIPTS_REPO` names another `owner/name`).
//!
//! The repository's CI publishes `index.json` on its `index` branch: each script's path, header
//! (name, description, tags), size, sha256, git dates and the files it imports. Turu keeps, in
//! the data folder:
//! * `scripts-index.json`: the last index and its ETag, so the list shows offline and a refresh
//!   that finds nothing new is an empty 304 (VibeSkua refetches its 1 MB `scripts.json`);
//! * `scripts-manifest.json`: the sha256 of each file as Turu wrote it. It tells a script that is
//!   only outdated from one you edited, which updates keep (VibeSkua overwrites those);
//! * `pinned-scripts.json`: the pinned scripts.
//!
//! Scripts are downloaded into [`Paths::scripts_dir`] with the files they import, pinned to the
//! index's commit and checked against its sha256. Many files at once come as one zip of the
//! repository instead of a request each. Everything here blocks on the network or the disk:
//! run it off the UI thread ([`ScriptRepo::spawn`]).

use crate::paths::Paths;
use crate::settings::AppSettings;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

/// Where Turu's scripts are published.
pub const DEFAULT_REPO: &str = "muktiarafi/turu-scripts";

/// The index format this client reads.
pub const INDEX_VERSION: u32 = 1;

/// Files downloaded at once (VibeSkua `DownloadAllWhereAsync`: 8).
const PARALLEL_DOWNLOADS: usize = 8;

/// From this many files, one zip of the repository is fetched instead of each file.
const ARCHIVE_THRESHOLD: usize = 25;

/// The repository's `index.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Index {
    pub version: u32,
    /// The commit the index was made from; files are downloaded from it.
    pub commit: String,
    /// When the index was made (RFC 3339).
    pub generated: String,
    pub scripts: Vec<ScriptInfo>,
}

/// VibeSkua `ScriptInfo`: one script of the index (or a local script).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScriptInfo {
    /// Relative to the repository and to the scripts folder, with `/`.
    pub path: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub size: u64,
    /// Lowercase hex.
    pub sha256: String,
    /// When the file was added to the repository (RFC 3339, UTC; VibeSkua `CreationDate`).
    pub added: String,
    /// When it last changed.
    pub updated: String,
    /// Every repository file it imports, directly or through others.
    pub deps: Vec<String>,
    /// No `export default`, so nothing to run (CoreBots, CoreFarms, ...): Search Scripts doesn't
    /// list it, and it comes with the scripts that import it. VibeSkua lists its Core files.
    pub library: bool,
}

impl ScriptInfo {
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The top folder (`farm`, `nation`, ...), or `None` for a file at the root.
    pub fn category(&self) -> Option<&str> {
        self.path.split_once('/').map(|(category, _)| category)
    }
}

/// A script's state against the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// In the repository, not in the scripts folder.
    NotDownloaded,
    UpToDate,
    /// Untouched since Turu wrote it, and the repository has another version (VibeSkua's
    /// "Update" badge).
    Outdated,
    /// Differs from what Turu wrote and from the repository: edited here (or put there by
    /// hand). Updates keep it; Download on the script itself overwrites it.
    Modified,
    /// A script of your own, not in the repository (VibeSkua's "Local").
    Local,
    /// Turu downloaded it, and the repository no longer has it.
    Removed,
}

impl Status {
    /// Whether the scripts folder has the file.
    pub fn is_downloaded(self) -> bool {
        self != Self::NotDownloaded
    }
}

/// A row of the Search Scripts list.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub info: ScriptInfo,
    pub status: Status,
    pub pinned: bool,
}

/// VibeSkua's search scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scope {
    #[default]
    All,
    Name,
    Tag,
    Description,
    File,
}

impl Scope {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Name,
        Self::Tag,
        Self::Description,
        Self::File,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Name => "Name",
            Self::Tag => "Tag",
            Self::Description => "Description",
            Self::File => "File",
        }
    }
}

/// VibeSkua `ScriptInfoViewModel.SearchRank`: lower ranks first, `None` when it doesn't match.
/// With a scope: 0 equal, 1 starts with, 2 contains. With All: 0 name, 1 description, 2 path,
/// 3 tag.
pub fn search_rank(info: &ScriptInfo, query: &str, scope: Scope) -> Option<u8> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let contains = |text: &str| text.to_lowercase().contains(&query);
    match scope {
        Scope::Name => text_rank(&info.name, &query),
        Scope::Tag => info.tags.iter().filter_map(|t| text_rank(t, &query)).min(),
        Scope::Description => text_rank(&info.description, &query),
        Scope::File => [
            text_rank(&info.path, &query),
            text_rank(info.file_name(), &query),
        ]
        .into_iter()
        .flatten()
        .min(),
        Scope::All => {
            if contains(&info.name) {
                Some(0)
            } else if contains(&info.description) {
                Some(1)
            } else if contains(&info.path) {
                Some(2)
            } else if info.tags.iter().any(|t| contains(t)) {
                Some(3)
            } else {
                None
            }
        }
    }
}

/// `query` is lowercase.
fn text_rank(text: &str, query: &str) -> Option<u8> {
    let text = text.to_lowercase();
    if text == query {
        Some(0)
    } else if text.starts_with(query) {
        Some(1)
    } else if text.contains(query) {
        Some(2)
    } else {
        None
    }
}

/// VibeSkua's category filter: a folder of the path starts with `category`.
pub fn in_category(info: &ScriptInfo, category: &str) -> bool {
    let category = category.to_lowercase();
    let mut folders = info.path.split('/').collect::<Vec<_>>();
    folders.pop();
    folders
        .iter()
        .any(|f| f.to_lowercase().starts_with(&category))
}

/// A script's header block.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Header {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

/// Whether `source` is a library: no line starts with `export default` (the engine runs the
/// default export). The index generator (`tools/build-index.mjs`, `isLibrary`) checks the same.
pub fn is_library(source: &str) -> bool {
    !source.lines().any(|line| {
        line.trim_start()
            .strip_prefix("export")
            .and_then(|rest| rest.strip_prefix(char::is_whitespace))
            .is_some_and(|rest| {
                let rest = rest.trim_start();
                rest.strip_prefix("default").is_some_and(|after| {
                    !after.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            })
    })
}

/// The `/* name: ... description: ... tags: ... */` block a script starts with (kept from the
/// C# scripts). An indented line continues the key above it. The scripts repository's index
/// generator (`tools/build-index.mjs`) reads it the same way.
pub fn parse_header(source: &str) -> Header {
    let mut header = Header::default();
    let text = source.trim_start_matches('\u{feff}').trim_start();
    let Some((body, _)) = text.strip_prefix("/*").and_then(|t| t.split_once("*/")) else {
        return header;
    };
    let mut tags = String::new();
    let mut current = None;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.split_once(':').filter(|(k, _)| {
            !line.starts_with(char::is_whitespace) && matches!(*k, "name" | "description" | "tags")
        });
        let (field, value) = match key {
            Some((key, value)) => {
                current = Some(key);
                (key, value.trim())
            }
            None => match current {
                Some(key) => (key, trimmed),
                None => continue,
            },
        };
        let target = match field {
            "name" => &mut header.name,
            "description" => &mut header.description,
            _ => &mut tags,
        };
        if !target.is_empty() && !value.is_empty() {
            target.push(' ');
        }
        target.push_str(value);
    }
    header.tags = tags
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect();
    header
}

/// What the index request answered.
pub enum Fetched {
    /// Same as the ETag sent.
    Unchanged,
    Index {
        body: Vec<u8>,
        etag: Option<String>,
    },
}

/// Where the index and the files come from (GitHub, or a fake one in tests).
pub trait Remote: Send + Sync {
    /// The index, or [`Fetched::Unchanged`] when it still has `etag`.
    fn index(&self, etag: Option<&str>) -> Result<Fetched, String>;
    /// A file of the repository at `commit`.
    fn file(&self, commit: &str, path: &str) -> Result<Vec<u8>, String>;
    /// The repository at `commit` as a zip with one top folder (GitHub's zipball).
    fn archive(&self, commit: &str) -> Result<Vec<u8>, String>;
}

/// The repository on GitHub: the index from raw.githubusercontent.com (no API rate limit),
/// the zip from codeload.github.com.
pub struct GitHub {
    repo: String,
    client: reqwest::blocking::Client,
}

impl GitHub {
    /// `repo` is `owner/name`.
    pub fn new(repo: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent(concat!("Turu/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self {
            repo: repo.trim_matches('/').to_owned(),
            client,
        }
    }

    /// [`DEFAULT_REPO`], or the `owner/name` in `TURU_SCRIPTS_REPO`.
    pub fn from_env() -> Self {
        let repo = std::env::var("TURU_SCRIPTS_REPO")
            .ok()
            .filter(|r| r.trim().contains('/'))
            .unwrap_or_else(|| DEFAULT_REPO.to_owned());
        Self::new(repo.trim())
    }

    fn url(&self, host: &str, rest: &[&str]) -> reqwest::Url {
        let mut url = reqwest::Url::parse(host).expect("a valid URL");
        url.path_segments_mut()
            .expect("an http URL")
            .extend(self.repo.split('/'))
            .extend(rest.iter().flat_map(|part| part.split('/')));
        url
    }

    fn bytes(&self, url: reqwest::Url) -> Result<Vec<u8>, String> {
        self.client
            .get(url.clone())
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map(|b| b.to_vec())
            .map_err(|e| format!("{url}: {e}"))
    }
}

impl Remote for GitHub {
    fn index(&self, etag: Option<&str>) -> Result<Fetched, String> {
        let url = self.url(
            "https://raw.githubusercontent.com",
            &["index", "index.json"],
        );
        let mut request = self.client.get(url.clone());
        if let Some(etag) = etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let response = request.send().map_err(|e| format!("{url}: {e}"))?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(Fetched::Unchanged);
        }
        let response = response
            .error_for_status()
            .map_err(|e| format!("{url}: {e}"))?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response
            .bytes()
            .map_err(|e| format!("{url}: {e}"))?
            .to_vec();
        Ok(Fetched::Index { body, etag })
    }

    fn file(&self, commit: &str, path: &str) -> Result<Vec<u8>, String> {
        self.bytes(self.url("https://raw.githubusercontent.com", &[commit, path]))
    }

    fn archive(&self, commit: &str) -> Result<Vec<u8>, String> {
        self.bytes(self.url("https://codeload.github.com", &["zip", commit]))
    }
}

/// What a running task is doing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Progress {
    pub task: String,
    pub message: String,
    pub done: usize,
    pub total: usize,
}

/// What the Search Scripts window shows.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub entries: Arc<Vec<Entry>>,
    /// Changes whenever `entries` does.
    pub generation: u64,
    /// The index's commit and when it was fetched (empty before the first fetch).
    pub commit: String,
    pub fetched: String,
    pub busy: Option<Progress>,
    /// The last task's result.
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct CachedIndex {
    etag: Option<String>,
    /// When it was fetched (RFC 3339).
    fetched: String,
    index: Index,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Manifest {
    /// The index commit of the last download.
    commit: String,
    /// Path -> sha256 of the file as Turu wrote it.
    files: BTreeMap<String, String>,
}

#[derive(Default)]
struct Data {
    cache: CachedIndex,
    manifest: Manifest,
    pinned: BTreeSet<String>,
    /// Path -> (size, modified, sha256), so a rescan only hashes changed files.
    hashes: HashMap<PathBuf, (u64, SystemTime, String)>,
    entries: Arc<Vec<Entry>>,
    generation: u64,
}

/// The script repository and the scripts folder. One per app (the tabs share the data
/// folder).
pub struct ScriptRepo {
    paths: Paths,
    remote: Box<dyn Remote>,
    data: Mutex<Data>,
    busy: Mutex<Option<Progress>>,
    message: Mutex<String>,
    cancel: AtomicBool,
}

impl ScriptRepo {
    /// Reads the cached index, the manifest and the pins, and scans the scripts folder. No
    /// network.
    pub fn new(paths: Paths, remote: Box<dyn Remote>) -> Arc<Self> {
        let mut cache: CachedIndex =
            read_json(&paths.root.join("scripts-index.json")).unwrap_or_default();
        sanitize(&mut cache.index);
        let data = Data {
            cache,
            manifest: read_json(&paths.root.join("scripts-manifest.json")).unwrap_or_default(),
            pinned: read_json(&paths.root.join("pinned-scripts.json")).unwrap_or_default(),
            ..Default::default()
        };
        let repo = Arc::new(Self {
            paths,
            remote,
            data: Mutex::new(data),
            busy: Mutex::new(None),
            message: Mutex::new(String::new()),
            cancel: AtomicBool::new(false),
        });
        repo.rescan();
        repo
    }

    pub fn snapshot(&self) -> Snapshot {
        let data = self.data.lock();
        Snapshot {
            entries: data.entries.clone(),
            generation: data.generation,
            commit: data.cache.index.commit.clone(),
            fetched: data.cache.fetched.clone(),
            busy: self.busy.lock().clone(),
            message: self.message.lock().clone(),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy.lock().is_some()
    }

    /// Asks the running task to stop after the current file.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Where `path` (an index path) is in the scripts folder.
    pub fn local_path(&self, path: &str) -> Option<PathBuf> {
        safe_relative(path).map(|p| self.paths.scripts_dir().join(p))
    }

    /// Runs `task` on a thread of its own, unless a task is running (then `false`). Its result
    /// becomes the snapshot's `message`.
    pub fn spawn(
        self: &Arc<Self>,
        task: &str,
        work: impl FnOnce(&ScriptRepo) -> Result<String, String> + Send + 'static,
    ) -> bool {
        {
            let mut busy = self.busy.lock();
            if busy.is_some() {
                return false;
            }
            *busy = Some(Progress {
                task: task.to_owned(),
                message: format!("{task}..."),
                ..Default::default()
            });
        }
        self.cancel.store(false, Ordering::Relaxed);
        let repo = self.clone();
        let task = task.to_owned();
        let spawned = std::thread::Builder::new()
            .name(format!("turu-scripts: {task}"))
            .spawn(move || {
                let result = work(&repo);
                let message = match result {
                    Ok(message) => message,
                    Err(e) => {
                        tracing::warn!("{task}: {e}");
                        format!("{task} failed: {e}")
                    }
                };
                *repo.message.lock() = message;
                *repo.busy.lock() = None;
            });
        if spawned.is_err() {
            *self.busy.lock() = None;
            return false;
        }
        true
    }

    fn progress(&self, message: String, done: usize, total: usize) {
        if let Some(progress) = self.busy.lock().as_mut() {
            progress.message = message;
            progress.done = done;
            progress.total = total;
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Fetches the index (VibeSkua `RefreshScriptsAsync`). When the network fails the cached
    /// one stays, and the error is returned.
    pub fn refresh(&self) -> Result<String, String> {
        self.progress("Getting the script list...".into(), 0, 0);
        let etag = {
            let data = self.data.lock();
            // Without a cached index (or after a reset) ask for the whole file.
            (!data.cache.index.commit.is_empty())
                .then(|| data.cache.etag.clone())
                .flatten()
        };
        let fetched = self.remote.index(etag.as_deref());
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let changed = match fetched {
            Ok(Fetched::Unchanged) => {
                self.data.lock().cache.fetched = now;
                false
            }
            Ok(Fetched::Index { body, etag }) => {
                let mut index: Index = serde_json::from_slice(&body)
                    .map_err(|e| format!("the script list isn't valid: {e}"))?;
                if index.version > INDEX_VERSION {
                    return Err(format!(
                        "the script list is format {}, this Turu reads {INDEX_VERSION}: update Turu",
                        index.version
                    ));
                }
                sanitize(&mut index);
                let cache = CachedIndex {
                    etag,
                    fetched: now,
                    index,
                };
                write_json(&self.paths.root.join("scripts-index.json"), &cache)
                    .map_err(|e| format!("couldn't save the script list: {e}"))?;
                self.data.lock().cache = cache;
                true
            }
            Err(e) => {
                self.rescan();
                return Err(e);
            }
        };
        self.rescan();
        let entries = self.data.lock().entries.clone();
        let outdated = entries
            .iter()
            .filter(|e| e.status == Status::Outdated)
            .count();
        let total = entries.iter().filter(|e| e.status != Status::Local).count();
        let mut message = format!("{total} scripts");
        if outdated > 0 {
            message.push_str(&format!(", {outdated} to update"));
        }
        if !changed {
            message.push_str(" (no changes)");
        }
        Ok(message)
    }

    /// Downloads `paths` and every file they import (VibeSkua `DownloadScriptAsync`). The
    /// files they import are only fetched when missing or outdated; `paths` themselves also
    /// when modified if `overwrite` is set. Returns how many files were written.
    pub fn download(&self, paths: &[String], overwrite: bool) -> Result<usize, String> {
        let (index, entries) = {
            let data = self.data.lock();
            (data.cache.index.clone(), data.entries.clone())
        };
        let infos: HashMap<&str, &ScriptInfo> =
            index.scripts.iter().map(|s| (s.path.as_str(), s)).collect();
        let status: HashMap<&str, Status> = entries
            .iter()
            .map(|e| (e.info.path.as_str(), e.status))
            .collect();
        let needs = |path: &str, explicit: bool| match status.get(path) {
            Some(Status::NotDownloaded | Status::Outdated) => true,
            Some(Status::Modified) => explicit && overwrite,
            _ => false,
        };
        let mut wanted = BTreeSet::new();
        for path in paths {
            let info = infos
                .get(path.as_str())
                .ok_or_else(|| format!("{path} isn't in the script list"))?;
            if needs(path, true) {
                wanted.insert(path.as_str());
            }
            for dep in &info.deps {
                if infos.contains_key(dep.as_str()) && needs(dep, false) {
                    wanted.insert(dep.as_str());
                }
            }
        }
        let wanted: Vec<&ScriptInfo> = wanted.into_iter().map(|p| infos[p]).collect();
        self.fetch(&index.commit, &wanted)
    }

    /// Fetches `wanted` at `commit`, checks and writes them, and records them in the manifest.
    fn fetch(&self, commit: &str, wanted: &[&ScriptInfo]) -> Result<usize, String> {
        if wanted.is_empty() {
            return Ok(0);
        }
        let total = wanted.len();
        let mut written = 0;
        let mut errors = Vec::new();
        let mut save = |info: &ScriptInfo, bytes: Result<Vec<u8>, String>| {
            match bytes.and_then(|b| self.write(info, &b)) {
                Ok(()) => written += 1,
                Err(e) => errors.push(format!("{}: {e}", info.path)),
            }
            self.progress(
                format!("Downloading ({written}/{total}): {}", info.name),
                written,
                total,
            );
        };
        if total >= ARCHIVE_THRESHOLD {
            self.progress(format!("Downloading {total} scripts..."), 0, total);
            let archive = self.remote.archive(commit)?;
            let mut files = unzip(&archive, wanted)?;
            for info in wanted {
                if self.cancelled() {
                    break;
                }
                let bytes = files
                    .remove(&info.path)
                    .ok_or_else(|| "not in the downloaded zip".to_owned());
                save(info, bytes);
            }
        } else {
            let queue = Mutex::new(wanted.iter().copied());
            let (sender, receiver) = crossbeam_channel::unbounded();
            std::thread::scope(|scope| {
                for _ in 0..PARALLEL_DOWNLOADS.min(total) {
                    let sender = sender.clone();
                    let queue = &queue;
                    scope.spawn(move || {
                        loop {
                            if self.cancelled() {
                                break;
                            }
                            let Some(info) = queue.lock().next() else {
                                break;
                            };
                            let bytes = self.remote.file(commit, &info.path);
                            if sender.send((info, bytes)).is_err() {
                                break;
                            }
                        }
                    });
                }
                drop(sender);
                for (info, bytes) in receiver {
                    save(info, bytes);
                }
            });
        }
        {
            let mut data = self.data.lock();
            data.manifest.commit = commit.to_owned();
            write_json(
                &self.paths.root.join("scripts-manifest.json"),
                &data.manifest,
            )
            .map_err(|e| format!("couldn't save scripts-manifest.json: {e}"))?;
        }
        self.rescan();
        if self.cancelled() {
            return Err(format!("cancelled after {written} of {total} files"));
        }
        if !errors.is_empty() {
            for e in &errors {
                tracing::warn!("Script download: {e}");
            }
            return Err(format!(
                "{} of {total} files failed ({}{})",
                errors.len(),
                errors[0],
                if errors.len() > 1 { ", ..." } else { "" }
            ));
        }
        Ok(written)
    }

    /// Checks `bytes` against the index and writes them (a temporary file, then a rename, so
    /// a script is never half written).
    fn write(&self, info: &ScriptInfo, bytes: &[u8]) -> Result<(), String> {
        let hash = sha256(bytes);
        if !hash.eq_ignore_ascii_case(&info.sha256) {
            return Err("the download doesn't match the script list (sha256)".into());
        }
        let path = self.local_path(&info.path).ok_or("not a script path")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temp = path.with_extension("turu-download");
        fs::write(&temp, bytes)
            .and_then(|()| fs::rename(&temp, &path))
            .map_err(|e| {
                let _ = fs::remove_file(&temp);
                e.to_string()
            })?;
        self.data
            .lock()
            .manifest
            .files
            .insert(info.path.clone(), hash);
        Ok(())
    }

    /// VibeSkua's "Update all": the outdated scripts, and what the downloaded ones now import
    /// that's missing. Modified scripts are kept.
    pub fn update_all(&self) -> Result<String, String> {
        self.refresh()?;
        let paths =
            self.paths_where(|e| e.status == Status::Outdated || e.status == Status::UpToDate);
        let count = self.download(&paths, false)?;
        Ok(self.summary(format!("Updated {count} files.")))
    }

    /// VibeSkua's Update Scripts and "Download all": every script that's missing or outdated.
    /// Modified scripts are kept.
    pub fn download_all(&self) -> Result<String, String> {
        self.refresh()?;
        let paths =
            self.paths_where(|e| matches!(e.status, Status::NotDownloaded | Status::Outdated));
        let count = self.download(&paths, false)?;
        Ok(self.summary(format!("Downloaded {count} files.")))
    }

    fn paths_where(&self, keep: impl Fn(&Entry) -> bool) -> Vec<String> {
        let entries = self.data.lock().entries.clone();
        entries
            .iter()
            .filter(|e| keep(e))
            .map(|e| e.info.path.clone())
            .collect()
    }

    /// `message`, plus the modified scripts updates kept.
    fn summary(&self, mut message: String) -> String {
        let entries = self.data.lock().entries.clone();
        let modified = entries
            .iter()
            .filter(|e| e.status == Status::Modified)
            .count();
        if modified > 0 {
            message.push_str(&format!(
                " Kept {modified} modified script{} (Download on one replaces it).",
                if modified == 1 { "" } else { "s" }
            ));
        }
        message
    }

    /// VibeSkua's Reset Scripts, without deleting anything: the files Turu downloaded and the
    /// ones at a repository path move to `scripts-backup/<time>/` in the data folder, your own
    /// scripts stay, and everything is downloaded again.
    pub fn reset(&self) -> Result<String, String> {
        self.progress("Moving the scripts aside...".into(), 0, 0);
        let backup = self
            .paths
            .root
            .join("scripts-backup")
            .join(chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
        let paths: BTreeSet<String> = {
            let data = self.data.lock();
            data.manifest
                .files
                .keys()
                .chain(data.cache.index.scripts.iter().map(|s| &s.path))
                .cloned()
                .collect()
        };
        let mut moved = 0;
        for path in &paths {
            let (Some(from), Some(relative)) = (self.local_path(path), safe_relative(path)) else {
                continue;
            };
            if !from.is_file() {
                continue;
            }
            let to = backup.join(relative);
            move_file(&from, &to).map_err(|e| format!("couldn't move {path} aside: {e}"))?;
            moved += 1;
        }
        {
            let mut data = self.data.lock();
            data.manifest = Manifest::default();
            data.cache.etag = None;
            data.hashes.clear();
            write_json(
                &self.paths.root.join("scripts-manifest.json"),
                &data.manifest,
            )
            .map_err(|e| format!("couldn't save scripts-manifest.json: {e}"))?;
        }
        self.rescan();
        let downloaded = self.download_all()?;
        Ok(if moved > 0 {
            format!("Moved {moved} files to {}. {downloaded}", backup.display())
        } else {
            downloaded
        })
    }

    /// Deletes a script from the scripts folder (VibeSkua `DeleteScriptAsync`).
    pub fn delete(&self, path: &str) -> Result<(), String> {
        let local = self.local_path(path).ok_or("not a script path")?;
        match fs::remove_file(&local) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        {
            let mut data = self.data.lock();
            if data.manifest.files.remove(path).is_some() {
                write_json(
                    &self.paths.root.join("scripts-manifest.json"),
                    &data.manifest,
                )
                .map_err(|e| e.to_string())?;
            }
        }
        self.rescan();
        Ok(())
    }

    pub fn set_pinned(&self, path: &str, pinned: bool) {
        {
            let mut data = self.data.lock();
            if pinned {
                data.pinned.insert(path.to_owned());
            } else {
                data.pinned.remove(path);
            }
            if let Err(e) = write_json(&self.paths.root.join("pinned-scripts.json"), &data.pinned) {
                tracing::warn!("Couldn't save pinned-scripts.json: {e}");
            }
        }
        self.rescan();
    }

    /// The script at `path`, downloaded with what it imports if needed (Load and Start in the
    /// list). A modified script is used as it is.
    pub fn ensure(&self, path: &str) -> Result<PathBuf, String> {
        let local = self.local_path(path).ok_or("not a script path")?;
        let in_index = self
            .data
            .lock()
            .cache
            .index
            .scripts
            .iter()
            .any(|s| s.path == path);
        if in_index {
            self.download(&[path.to_owned()], false)?;
        }
        if !local.is_file() {
            return Err(format!("{path} isn't in the scripts folder"));
        }
        Ok(local)
    }

    /// At launch (VibeSkua Manager): with Check for Script Updates, fetch the index; with Auto
    /// Update Scripts too, download what's missing or outdated.
    pub fn startup(&self, settings: &AppSettings) -> Result<String, String> {
        if !settings.check_bot_scripts_updates {
            return Ok(String::new());
        }
        if settings.auto_update_bot_scripts {
            self.download_all()
        } else {
            self.refresh()
        }
    }

    /// Works out every script's status from the index, the manifest and the scripts folder.
    pub fn rescan(&self) {
        let (index, manifest, pinned, mut hashes) = {
            let mut data = self.data.lock();
            let hashes = std::mem::take(&mut data.hashes);
            (
                data.cache.index.clone(),
                data.manifest.clone(),
                data.pinned.clone(),
                hashes,
            )
        };
        let dir = self.paths.scripts_dir();
        let mut entries = Vec::with_capacity(index.scripts.len());
        let mut known = HashSet::new();
        for info in index.scripts {
            known.insert(info.path.clone());
            let local = dir.join(safe_relative(&info.path).unwrap_or_default());
            let status = match cached_hash(&mut hashes, &local) {
                None => Status::NotDownloaded,
                Some(hash) if hash.eq_ignore_ascii_case(&info.sha256) => Status::UpToDate,
                Some(hash) if manifest.files.get(&info.path) == Some(&hash) => Status::Outdated,
                Some(_) => Status::Modified,
            };
            entries.push(Entry {
                pinned: pinned.contains(&info.path),
                info,
                status,
            });
        }
        let mut local_files = Vec::new();
        list_scripts(&dir, &dir, &mut local_files);
        for (relative, path) in local_files {
            if known.contains(&relative) {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap_or_default();
            let header = parse_header(&source);
            let stem = Path::new(&relative)
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let status = if manifest.files.contains_key(&relative) {
                Status::Removed
            } else {
                Status::Local
            };
            entries.push(Entry {
                pinned: pinned.contains(&relative),
                info: ScriptInfo {
                    name: if header.name.is_empty() {
                        stem
                    } else {
                        header.name
                    },
                    description: header.description,
                    tags: header.tags,
                    size: source.len() as u64,
                    library: is_library(&source),
                    path: relative,
                    ..Default::default()
                },
                status,
            });
        }
        let mut data = self.data.lock();
        data.hashes = hashes;
        data.entries = Arc::new(entries);
        data.generation += 1;
    }
}

/// Drops entries whose path could leave the scripts folder or isn't a script.
fn sanitize(index: &mut Index) {
    index.scripts.retain(|s| {
        let ok = safe_relative(&s.path).is_some();
        if !ok {
            tracing::warn!("Script list: skipped {:?} (not a script path)", s.path);
        }
        ok
    });
    for script in &mut index.scripts {
        if script.name.trim().is_empty() {
            script.name = script
                .file_name()
                .trim_end_matches(".ts")
                .trim_end_matches(".js")
                .to_owned();
        }
    }
}

/// `path` as a relative path inside the scripts folder: `/`-separated folders and a `.ts` or
/// `.js` file, nothing absolute, no `..`.
fn safe_relative(path: &str) -> Option<PathBuf> {
    if path.is_empty() || path.contains('\\') || path.contains(':') {
        return None;
    }
    let relative = Path::new(path);
    if relative.is_absolute()
        || !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return None;
    }
    matches!(relative.extension()?.to_str()?, "ts" | "js").then(|| relative.to_path_buf())
}

/// The `.ts`/`.js` files under `dir`, as (`/`-separated path relative to `root`, path).
/// Hidden folders are skipped.
fn list_scripts(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            list_scripts(root, &path, out);
        } else if matches!(path.extension().and_then(|e| e.to_str()), Some("ts" | "js"))
            && let Ok(relative) = path.strip_prefix(root)
        {
            let relative = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        }
    }
}

fn cached_hash(
    hashes: &mut HashMap<PathBuf, (u64, SystemTime, String)>,
    path: &Path,
) -> Option<String> {
    let meta = fs::metadata(path).ok().filter(|m| m.is_file())?;
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    if let Some((size, time, hash)) = hashes.get(path)
        && *size == meta.len()
        && *time == modified
    {
        return Some(hash.clone());
    }
    let hash = sha256(&fs::read(path).ok()?);
    hashes.insert(path.to_path_buf(), (meta.len(), modified, hash.clone()));
    Some(hash)
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The `wanted` files of a GitHub zipball (paths below its top folder).
fn unzip(archive: &[u8], wanted: &[&ScriptInfo]) -> Result<HashMap<String, Vec<u8>>, String> {
    let wanted: HashSet<&str> = wanted.iter().map(|s| s.path.as_str()).collect();
    let mut zip =
        zip::ZipArchive::new(io::Cursor::new(archive)).map_err(|e| format!("bad zip: {e}"))?;
    let mut files = HashMap::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| format!("bad zip: {e}"))?;
        let Some((_, path)) = file.name().split_once('/') else {
            continue;
        };
        if !wanted.contains(path) {
            continue;
        }
        let path = path.to_owned();
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|e| format!("bad zip: {e}"))?;
        files.insert(path, bytes);
    }
    Ok(files)
}

fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::rename(from, to).is_err() {
        fs::copy(from, to)?;
        fs::remove_file(from)?;
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text)
        .inspect_err(|e| tracing::warn!("Couldn't read {}: {e}", path.display()))
        .ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, path)
}
