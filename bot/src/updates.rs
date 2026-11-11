//! Client updates: is there a newer Turu, and installing it.
//!
//! VibeSkua's Manager checks its GitHub releases at launch through Velopack, asks whether to
//! download, then applies the update and restarts (`ClientUpdateService`). Velopack only updates
//! installed apps; Turu ships portable archives (`.github/workflows/release.yml`), so it has this
//! small updater instead:
//!
//! * [`check`] reads the GitHub releases ([`releases_url`]), skips drafts (and pre-releases,
//!   unless [`wants_prereleases`]) and returns the newest one above [`current_version`], with
//!   this platform's asset ([`Target`]) and `SHA256SUMS.txt`. The frontend checks at launch and
//!   every [`CHECK_EVERY`] (`turu_frontend::turu::start_client_updates`) and keeps the answer
//!   here ([`available`]), where the window, the control API's `/status` and the web monitor
//!   read it.
//! * [`download`] fetches the asset and checks its SHA-256 against `SHA256SUMS.txt`. That only
//!   proves the file arrived intact: anyone who can publish a release on the repository can
//!   publish matching sums (signing the sums, with minisign, is a later step: docs/STATUS.md).
//! * [`install`] (desktops): unpacks it ([`install::stage`]), swaps the program
//!   ([`install::apply`]) and starts the new one ([`install::Relaunch`]).
//!
//! Android only notifies: its button opens the APK's URL. Turu Mas ([`set_turu_mas`]) looks for
//! its own APK, not Turu's.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Turu's releases on GitHub (`TURU_RELEASES_URL` overrides it).
pub const DEFAULT_RELEASES_URL: &str = "https://api.github.com/repos/muktiarafi/turu/releases";
/// How often a running client checks again.
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// The release asset with every other asset's SHA-256 (`sha256sum`'s format).
pub const SUMS_NAME: &str = "SHA256SUMS.txt";

/// This build's version: the release workflow sets `TURU_VERSION` from the git tag (as
/// `turu_frontend::turu::VERSION` shows it); local builds have the crate's.
pub fn current_version() -> &'static str {
    match option_env!("TURU_VERSION") {
        Some(version) => version,
        None => env!("CARGO_PKG_VERSION"),
    }
}

/// [`DEFAULT_RELEASES_URL`], or `TURU_RELEASES_URL`.
pub fn releases_url() -> String {
    std::env::var("TURU_RELEASES_URL")
        .ok()
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_RELEASES_URL.to_owned())
}

/// Whether this is Turu Mas (docs/TURU_MAS.md): its engine says so at start ([`set_turu_mas`]).
static TURU_MAS: AtomicBool = AtomicBool::new(false);

/// Called by Turu Mas's engine before its first check: its package is
/// [`Target::TuruMasArm64`], and the windows name it [`app_name`].
pub fn set_turu_mas() {
    TURU_MAS.store(true, Ordering::Relaxed);
}

/// The app's name, as the update texts give it: "Turu", or "Turu Mas".
pub fn app_name() -> &'static str {
    if TURU_MAS.load(Ordering::Relaxed) {
        "Turu Mas"
    } else {
        "Turu"
    }
}

/// A release package, as `release.yml` names them: `<prefix><version>-<suffix>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    LinuxX64,
    WindowsX64,
    /// Both Macs: one universal `Turu.app`.
    MacOs,
    AndroidArm64,
    /// Turu Mas, Android's other app: `turumas-<version>-android-arm64.apk`.
    TuruMasArm64,
}

impl Target {
    /// This build's package; `None` where Turu ships none (Linux on ARM, say).
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "android") {
            cfg!(target_arch = "aarch64").then(|| {
                if TURU_MAS.load(Ordering::Relaxed) {
                    Self::TuruMasArm64
                } else {
                    Self::AndroidArm64
                }
            })
        } else if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(all(windows, target_arch = "x86_64")) {
            Some(Self::WindowsX64)
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self::LinuxX64)
        } else {
            None
        }
    }

    /// The package's name up to its version. Turu's own (`turu-`) never matches Turu Mas's
    /// (`turumas-`), nor the other way round.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::TuruMasArm64 => "turumas-",
            _ => "turu-",
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Self::LinuxX64 => "linux-x86_64.tar.gz",
            Self::WindowsX64 => "windows-x86_64.zip",
            Self::MacOs => "macos-universal.zip",
            Self::AndroidArm64 | Self::TuruMasArm64 => "android-arm64.apk",
        }
    }

    /// Whether Turu can replace itself with the package (else it only opens its URL).
    pub fn updates_in_place(self) -> bool {
        !matches!(self, Self::AndroidArm64 | Self::TuruMasArm64)
    }
}

/// A file of a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub name: String,
    /// Where to download it (`browser_download_url`).
    pub url: String,
    pub size: u64,
}

/// A newer release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// Without the tag's `v`: `0.2.0-alpha.7`.
    pub version: String,
    pub tag: String,
    /// The release's page: its notes.
    pub html_url: String,
    pub prerelease: bool,
    /// This platform's package; `None` when the release has none.
    pub asset: Option<Asset>,
    pub sums: Option<Asset>,
}

impl Release {
    /// Whether [`install`] can install it here: it has this platform's package and its sums.
    pub fn installable(&self) -> bool {
        self.asset.is_some() && self.sums.is_some()
    }
}

/// What the GitHub API lists (only the fields used).
#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Downloads a URL's body (GitHub, or a fake one in tests).
pub trait Fetch: Send + Sync {
    fn get(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// Over HTTPS, as Turu (GitHub's API wants a user agent).
pub struct Http {
    client: reqwest::blocking::Client,
}

impl Http {
    /// For the release list: a short timeout.
    pub fn for_checks() -> Self {
        Self::new(Duration::from_secs(30))
    }

    /// For packages (tens of MB): a long one.
    pub fn for_downloads() -> Self {
        Self::new(Duration::from_secs(30 * 60))
    }

    fn new(timeout: Duration) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(timeout)
            .user_agent(concat!("Turu/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        Self { client }
    }
}

impl Fetch for Http {
    fn get(&self, url: &str) -> Result<Vec<u8>, String> {
        self.client
            .get(url)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.bytes())
            .map(|b| b.to_vec())
            .map_err(|e| format!("{url}: {e}"))
    }
}

/// A tag's version (`v0.2.0-alpha.6` or `0.2.0`).
pub fn parse_version(tag: &str) -> Option<semver::Version> {
    let tag = tag.trim();
    semver::Version::parse(tag.strip_prefix(['v', 'V']).unwrap_or(tag)).ok()
}

/// Whether pre-releases count: Include Pre-releases (VibeSkua's Manager
/// `CheckClientPrereleases`), or this build is one itself (every Turu release so far is an
/// alpha: its users are on that channel already).
pub fn wants_prereleases(setting: bool, current: &str) -> bool {
    setting || parse_version(current).is_some_and(|v| !v.pre.is_empty())
}

/// The newest release in GitHub's list (`json`) above `current`: no drafts, and no
/// pre-releases (marked so, or with a `-` in the version) unless `prereleases`. `None` when
/// `current` is the newest, or isn't a version (a build from a branch).
pub fn newest(
    json: &[u8],
    current: &str,
    prereleases: bool,
    target: Option<Target>,
) -> Result<Option<Release>, String> {
    let list: Vec<GitHubRelease> =
        serde_json::from_slice(json).map_err(|e| format!("couldn't read the release list: {e}"))?;
    let Some(current) = parse_version(current) else {
        return Ok(None);
    };
    let best = list
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| parse_version(&r.tag_name).map(|v| (v, r)))
        .filter(|(v, r)| prereleases || !(r.prerelease || !v.pre.is_empty()))
        .filter(|(v, _)| *v > current)
        .max_by(|(a, _), (b, _)| a.cmp(b));
    Ok(best.map(|(version, r)| {
        let asset = |wanted: &dyn Fn(&str) -> bool| {
            r.assets.iter().find(|a| wanted(&a.name)).map(|a| Asset {
                name: a.name.clone(),
                url: a.browser_download_url.clone(),
                size: a.size,
            })
        };
        let package = target.and_then(|t| {
            let suffix = format!("-{}", t.suffix());
            asset(&|name| name.starts_with(t.prefix()) && name.ends_with(&suffix))
        });
        Release {
            version: version.to_string(),
            tag: r.tag_name.clone(),
            html_url: r.html_url.clone(),
            prerelease: r.prerelease || !version.pre.is_empty(),
            asset: package,
            sums: asset(&|name| name == SUMS_NAME),
        }
    }))
}

/// Reads the release list at `url` and returns [`newest`] for this platform.
pub fn check(
    fetch: &dyn Fetch,
    url: &str,
    current: &str,
    prereleases: bool,
) -> Result<Option<Release>, String> {
    newest(&fetch.get(url)?, current, prereleases, Target::current())
}

/// `sha256sum`'s lines (`<hex>  <name>`, or `<hex> *<name>`): name to lowercase hex.
pub fn parse_sums(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (hash, name) = line.trim().split_once(char::is_whitespace)?;
            let name = name.trim_start().trim_start_matches('*').trim();
            let valid = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
            (valid && !name.is_empty()).then(|| (name.to_owned(), hash.to_ascii_lowercase()))
        })
        .collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Checks `bytes` (the file `name`) against `SHA256SUMS.txt`'s text.
pub fn verify(name: &str, bytes: &[u8], sums: &str) -> Result<(), String> {
    let sums = parse_sums(sums);
    let expected = sums
        .get(name)
        .ok_or_else(|| format!("{SUMS_NAME} has no line for {name}"))?;
    let actual = sha256_hex(bytes);
    if *expected == actual {
        Ok(())
    } else {
        Err(format!(
            "{name} is damaged: its SHA-256 is {actual}, {SUMS_NAME} says {expected}"
        ))
    }
}

/// Downloads the release's package for this platform into `dir` (emptied first), checks it
/// against its sums, and returns where it is.
pub fn download(fetch: &dyn Fetch, release: &Release, dir: &Path) -> Result<PathBuf, String> {
    let asset = release
        .asset
        .as_ref()
        .ok_or_else(|| format!("Turu {} has no package for this platform", release.version))?;
    let sums = release
        .sums
        .as_ref()
        .ok_or_else(|| format!("Turu {} has no {SUMS_NAME}", release.version))?;
    // The asset's name becomes a file name: no folders in it.
    if asset.name.contains(['/', '\\']) || asset.name.starts_with('.') {
        return Err(format!("unexpected package name {}", asset.name));
    }
    let sums = String::from_utf8_lossy(&fetch.get(&sums.url)?).into_owned();
    let bytes = fetch.get(&asset.url)?;
    verify(&asset.name, &bytes, &sums)?;
    empty_dir(dir)?;
    let path = dir.join(&asset.name);
    std::fs::write(&path, &bytes).map_err(|e| format!("couldn't save {}: {e}", path.display()))?;
    Ok(path)
}

fn empty_dir(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("couldn't empty {}: {e}", dir.display())),
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't make {}: {e}", dir.display()))
}

// ---------------------------------------------------------------------------------------
// What this process knows: the UI, the control API and the monitor read it.

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The newer release, from the last check that worked.
    pub available: Option<Release>,
    /// When the last check ended, and its error when it failed.
    pub checked_at: Option<chrono::DateTime<chrono::Local>>,
    pub error: Option<String>,
    /// A check runs.
    pub checking: bool,
    pub install: Install,
}

/// Download & restart's progress.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Install {
    #[default]
    Idle,
    /// Downloading, checking or unpacking: what it does.
    Busy(String),
    /// Unpacked; installed when Turu quits (Update when I quit).
    OnQuit(String),
    Failed(String),
}

static STATE: Mutex<State> = Mutex::new(State {
    available: None,
    checked_at: None,
    error: None,
    checking: false,
    install: Install::Idle,
});

pub fn state() -> State {
    STATE.lock().clone()
}

pub fn available() -> Option<Release> {
    STATE.lock().available.clone()
}

/// Runs `change` on the state.
pub fn update_state<T>(change: impl FnOnce(&mut State) -> T) -> T {
    change(&mut STATE.lock())
}

/// Records a check's outcome; returns the release when it is new since the last check (to log
/// it once, not every 6 hours).
pub fn record_check(result: Result<Option<Release>, String>) -> Option<Release> {
    let mut state = STATE.lock();
    state.checking = false;
    state.checked_at = Some(chrono::Local::now());
    match result {
        Ok(release) => {
            state.error = None;
            let new = release
                .as_ref()
                .filter(|r| state.available.as_ref().map(|a| &a.version) != Some(&r.version));
            let new = new.cloned();
            state.available = release;
            new
        }
        Err(e) => {
            state.error = Some(e);
            None
        }
    }
}

/// The control API's and the monitor's `update` field: `null`, or the version and links.
pub fn status_json() -> serde_json::Value {
    let state = STATE.lock();
    match &state.available {
        None => serde_json::Value::Null,
        Some(release) => serde_json::json!({
            "version": release.version,
            "current": current_version(),
            "url": release.html_url,
            "download": release.asset.as_ref().map(|a| &a.url),
            "prerelease": release.prerelease,
        }),
    }
}

/// Installs the available update and exits (the control API's `POST /update`): set by the
/// frontend, which knows how this process quits. Answers what it started, or why it can't.
pub type Installer = Box<dyn Fn() -> Result<String, String> + Send + Sync>;
static INSTALLER: OnceLock<Installer> = OnceLock::new();

pub fn set_installer(installer: Installer) -> bool {
    INSTALLER.set(installer).is_ok()
}

pub fn installer() -> Option<&'static Installer> {
    INSTALLER.get()
}

#[cfg(not(target_os = "android"))]
pub mod install {
    //! Replacing the running Turu with a downloaded package.
    //!
    //! * Linux: the `.tar.gz` holds `turu`; Windows: the `.zip` holds `turu.exe`. The running
    //!   program is swapped with `self_replace` (Windows can't overwrite a running exe; it moves
    //!   it aside).
    //! * macOS: the `.zip` holds `Turu.app`. It is copied next to the running bundle with
    //!   `ditto`, the two are swapped by renames, and the new one starts with `open -n`.

    use super::Target;
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// An unpacked package.
    #[derive(Debug, Clone)]
    pub struct Staged {
        target: Target,
        /// The new program (Linux, Windows) or `Turu.app` (macOS).
        program: PathBuf,
    }

    /// How to start the installed Turu.
    #[derive(Debug, Clone)]
    pub enum Relaunch {
        Program(PathBuf),
        /// `open -n` this bundle.
        Bundle(PathBuf),
    }

    impl Relaunch {
        /// Starts it with `args` (the arguments this process got, without the program).
        pub fn spawn(&self, args: &[OsString]) -> Result<(), String> {
            let result = match self {
                Self::Program(program) => Command::new(program).args(args).spawn(),
                Self::Bundle(bundle) => Command::new("open")
                    .arg("-n")
                    .arg(bundle)
                    .arg("--args")
                    .args(args)
                    .spawn(),
            };
            result
                .map(drop)
                .map_err(|e| format!("couldn't start the new Turu: {e}"))
        }
    }

    fn run(command: &mut Command) -> Result<(), String> {
        let name = command.get_program().to_string_lossy().into_owned();
        let output = command
            .output()
            .map_err(|e| format!("couldn't run {name}: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "{name} failed ({}): {}",
                output.status,
                stderr.trim()
            ))
        }
    }

    /// Unpacks `archive` into `dir` (next to it, emptied first).
    pub fn stage(archive: &Path, dir: &Path, target: Target) -> Result<Staged, String> {
        super::empty_dir(dir)?;
        let program = match target {
            Target::LinuxX64 => {
                run(Command::new("tar")
                    .arg("-xzf")
                    .arg(archive)
                    .arg("-C")
                    .arg(dir))?;
                dir.join("turu")
            }
            Target::WindowsX64 => {
                let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
                let mut zip = zip::ZipArchive::new(file)
                    .map_err(|e| format!("couldn't open the package: {e}"))?;
                let mut exe = zip
                    .by_name("turu.exe")
                    .map_err(|e| format!("the package has no turu.exe: {e}"))?;
                let path = dir.join("turu.exe");
                let mut out = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                std::io::copy(&mut exe, &mut out)
                    .map_err(|e| format!("couldn't unpack turu.exe: {e}"))?;
                path
            }
            Target::MacOs => {
                run(Command::new("ditto")
                    .arg("-x")
                    .arg("-k")
                    .arg(archive)
                    .arg(dir))?;
                dir.join("Turu.app")
            }
            Target::AndroidArm64 | Target::TuruMasArm64 => {
                return Err("Android installs the APK itself".into());
            }
        };
        if !program.exists() {
            return Err(format!(
                "the package has no {}",
                program.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
        Ok(Staged { target, program })
    }

    /// The `.app` bundle the running program is in (`Turu.app/Contents/MacOS/turu`).
    fn running_bundle(exe: &Path) -> Option<PathBuf> {
        let bundle = exe.parent()?.parent()?.parent()?;
        bundle
            .extension()
            .is_some_and(|e| e == "app")
            .then(|| bundle.to_path_buf())
    }

    /// Puts the staged Turu in the running one's place; returns how to start it.
    pub fn apply(staged: &Staged) -> Result<Relaunch, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("couldn't find the running program: {e}"))?;
        if staged.target == Target::MacOs {
            let bundle = running_bundle(&exe).ok_or_else(|| {
                format!("{} isn't inside Turu.app: update it by hand", exe.display())
            })?;
            replace_bundle(&bundle, &staged.program)?;
            return Ok(Relaunch::Bundle(bundle));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&staged.program, std::fs::Permissions::from_mode(0o755));
        }
        self_replace::self_replace(&staged.program)
            .map_err(|e| format!("couldn't replace {}: {e}", exe.display()))?;
        // The path, not `current_exe` again: on Linux that now names the replaced file.
        Ok(Relaunch::Program(exe))
    }

    /// Copies `new` next to `bundle` with `ditto`, then swaps them by renames (the running
    /// program keeps its open files), and deletes the old one.
    fn replace_bundle(bundle: &Path, new: &Path) -> Result<(), String> {
        let name = bundle
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let incoming = bundle.with_file_name(format!(".{name}.update"));
        let old = bundle.with_file_name(format!(".{name}.old"));
        for leftover in [&incoming, &old] {
            if leftover.exists() {
                std::fs::remove_dir_all(leftover)
                    .map_err(|e| format!("couldn't remove {}: {e}", leftover.display()))?;
            }
        }
        run(Command::new("ditto").arg(new).arg(&incoming))?;
        std::fs::rename(bundle, &old)
            .map_err(|e| format!("couldn't move {} aside: {e}", bundle.display()))?;
        if let Err(e) = std::fs::rename(&incoming, bundle) {
            let _ = std::fs::rename(&old, bundle);
            return Err(format!("couldn't put the new Turu.app in place: {e}"));
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::Write;

        #[cfg(unix)]
        #[test]
        fn stages_the_linux_tarball() {
            let dir = tempfile::tempdir().unwrap();
            let package = dir.path().join("package");
            std::fs::create_dir_all(&package).unwrap();
            std::fs::write(package.join("turu"), b"new").unwrap();
            std::fs::write(package.join("README.txt"), b"readme").unwrap();
            let archive = dir.path().join("turu-9.0.0-linux-x86_64.tar.gz");
            // As release.yml packs it: `-C package .`, so the entries are `./turu`.
            run(Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(&package)
                .arg("."))
            .unwrap();
            let staged = stage(&archive, &dir.path().join("staged"), Target::LinuxX64).unwrap();
            assert_eq!(std::fs::read(&staged.program).unwrap(), b"new");
            let empty = dir.path().join("empty.tar.gz");
            run(Command::new("tar")
                .arg("-czf")
                .arg(&empty)
                .arg("-C")
                .arg(dir.path().join("staged"))
                .arg("README.txt"))
            .unwrap();
            assert!(stage(&empty, &dir.path().join("staged2"), Target::LinuxX64).is_err());
        }

        #[test]
        fn stages_the_windows_zip() {
            let dir = tempfile::tempdir().unwrap();
            let archive = dir.path().join("turu-9.0.0-windows-x86_64.zip");
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
            for (name, body) in [("README.txt", &b"readme"[..]), ("turu.exe", b"new")] {
                zip.start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(body).unwrap();
            }
            zip.finish().unwrap();
            let staged = stage(&archive, &dir.path().join("staged"), Target::WindowsX64).unwrap();
            assert_eq!(std::fs::read(&staged.program).unwrap(), b"new");
            assert!(stage(&archive, &dir.path().join("x"), Target::AndroidArm64).is_err());
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn stages_the_mac_zip_and_swaps_the_bundle() {
            let dir = tempfile::tempdir().unwrap();
            let make_app = |root: &Path, body: &[u8]| {
                let macos = root.join("Turu.app/Contents/MacOS");
                std::fs::create_dir_all(&macos).unwrap();
                std::fs::write(macos.join("turu"), body).unwrap();
            };
            let package = dir.path().join("package");
            make_app(&package, b"new");
            std::fs::write(package.join("README.txt"), b"readme").unwrap();
            let archive = dir.path().join("turu-9.0.0-macos-universal.zip");
            // As release.yml packs it.
            run(Command::new("ditto")
                .args(["-c", "-k", "--norsrc"])
                .arg(&package)
                .arg(&archive))
            .unwrap();
            let staged = stage(&archive, &dir.path().join("staged"), Target::MacOs).unwrap();
            assert!(staged.program.ends_with("Turu.app"));

            let installed = dir.path().join("Applications");
            make_app(&installed, b"old");
            let bundle = installed.join("Turu.app");
            assert_eq!(
                running_bundle(&bundle.join("Contents/MacOS/turu")),
                Some(bundle.clone())
            );
            assert_eq!(running_bundle(Path::new("/usr/local/bin/turu")), None);
            replace_bundle(&bundle, &staged.program).unwrap();
            assert_eq!(
                std::fs::read(bundle.join("Contents/MacOS/turu")).unwrap(),
                b"new"
            );
            let left: Vec<_> = std::fs::read_dir(&installed)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            assert_eq!(left, ["Turu.app"], "no leftovers");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serves canned bodies by URL.
    struct Fake(HashMap<String, Vec<u8>>);

    impl Fetch for Fake {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.0
                .get(url)
                .cloned()
                .ok_or_else(|| format!("{url}: 404"))
        }
    }

    fn release(tag: &str, prerelease: bool, draft: bool) -> serde_json::Value {
        let v = tag.trim_start_matches('v');
        let assets: Vec<serde_json::Value> = [
            "linux-x86_64.tar.gz",
            "windows-x86_64.zip",
            "macos-universal.zip",
            "android-arm64.apk",
        ]
        .iter()
        .map(|suffix| {
            let name = format!("turu-{v}-{suffix}");
            serde_json::json!({ "name": name, "browser_download_url": format!("https://dl/{tag}/{name}"), "size": 10 })
        })
        .chain([serde_json::json!({ "name": SUMS_NAME, "browser_download_url": format!("https://dl/{tag}/sums"), "size": 1 })])
        .collect();
        serde_json::json!({
            "tag_name": tag,
            "html_url": format!("https://github.com/muktiarafi/turu/releases/tag/{tag}"),
            "draft": draft,
            "prerelease": prerelease,
            "assets": assets,
        })
    }

    fn list(releases: &[serde_json::Value]) -> Vec<u8> {
        serde_json::to_vec(releases).unwrap()
    }

    #[test]
    fn compares_versions_with_semver() {
        let json = list(&[
            release("v0.2.0-alpha.10", true, false),
            release("v0.2.0-alpha.9", true, false),
            release("v0.1.5", false, false),
        ]);
        // alpha.10 > alpha.9: numeric, not textual.
        let newest = newest(&json, "0.2.0-alpha.9", true, None).unwrap().unwrap();
        assert_eq!(newest.version, "0.2.0-alpha.10");
        assert_eq!(newest.tag, "v0.2.0-alpha.10");
        assert!(newest.prerelease);
        assert!(
            super::newest(&json, "0.2.0-alpha.10", true, None)
                .unwrap()
                .is_none()
        );
        // 0.2.0 is above its alphas.
        assert!(super::newest(&json, "0.2.0", true, None).unwrap().is_none());
        // A build from a branch isn't a version: nothing to offer.
        assert!(
            super::newest(&json, "0.0.0-dev.abc1234", true, None)
                .unwrap()
                .is_some()
        );
        assert!(super::newest(&json, "dev", true, None).unwrap().is_none());
        assert_eq!(
            parse_version("v1.2.3"),
            semver::Version::parse("1.2.3").ok()
        );
        assert!(super::newest(b"not json", "0.1.0", true, None).is_err());
    }

    #[test]
    fn skips_drafts_and_chooses_the_channel() {
        let json = list(&[
            release("v0.4.0", false, true),
            release("v0.3.1-beta.1", true, false),
            // Not marked a pre-release, but its version says it is one.
            release("v0.3.1-rc.1", false, false),
            release("v0.3.0", false, false),
        ]);
        let stable = newest(&json, "0.2.0", false, None).unwrap().unwrap();
        assert_eq!(stable.version, "0.3.0", "no drafts, no pre-releases");
        let any = newest(&json, "0.2.0", true, None).unwrap().unwrap();
        assert_eq!(any.version, "0.3.1-rc.1", "rc > beta; never the draft");
        // A pre-release build is on the pre-release channel even with the switch off.
        assert!(wants_prereleases(false, "0.2.0-alpha.6"));
        assert!(!wants_prereleases(false, "0.2.0"));
        assert!(wants_prereleases(true, "0.2.0"));
    }

    #[test]
    fn picks_this_platforms_asset_and_the_sums() {
        let json = list(&[release("v0.3.0", false, false)]);
        for (target, name) in [
            (Target::LinuxX64, "turu-0.3.0-linux-x86_64.tar.gz"),
            (Target::WindowsX64, "turu-0.3.0-windows-x86_64.zip"),
            (Target::MacOs, "turu-0.3.0-macos-universal.zip"),
            (Target::AndroidArm64, "turu-0.3.0-android-arm64.apk"),
        ] {
            let release = newest(&json, "0.2.0", false, Some(target))
                .unwrap()
                .unwrap();
            let asset = release.asset.as_ref().expect("an asset");
            assert_eq!(asset.name, name);
            assert_eq!(asset.url, format!("https://dl/v0.3.0/{name}"));
            assert_eq!(release.sums.as_ref().unwrap().name, SUMS_NAME);
            assert!(release.installable());
            assert_eq!(target.updates_in_place(), target != Target::AndroidArm64);
        }
        let none = newest(&json, "0.2.0", false, None).unwrap().unwrap();
        assert!(none.asset.is_none() && !none.installable());
        // Turu and Turu Mas (two apps, release.yml's `android` and `turu-mas` jobs) each take
        // their own APK, whichever is listed first.
        for mas_first in [true, false] {
            let mut with_mas = release("v0.3.0", false, false);
            let mas = serde_json::json!({ "name": "turumas-0.3.0-android-arm64.apk", "browser_download_url": "https://dl/mas", "size": 10 });
            let assets = with_mas["assets"].as_array_mut().unwrap();
            if mas_first {
                assets.insert(0, mas);
            } else {
                assets.push(mas);
            }
            let json = list(&[with_mas]);
            for (target, name) in [
                (Target::AndroidArm64, "turu-0.3.0-android-arm64.apk"),
                (Target::TuruMasArm64, "turumas-0.3.0-android-arm64.apk"),
            ] {
                let release = newest(&json, "0.2.0", false, Some(target))
                    .unwrap()
                    .unwrap();
                assert_eq!(release.asset.unwrap().name, name);
                assert!(!target.updates_in_place());
            }
        }
        // A release made before Turu Mas has no APK of its own: a link to the release page.
        let old = newest(&json, "0.2.0", false, Some(Target::TuruMasArm64))
            .unwrap()
            .unwrap();
        assert!(old.asset.is_none());
        // A release without this platform's package is still offered, as a link.
        let bare = serde_json::json!([{ "tag_name": "v0.3.0", "html_url": "u", "assets": [] }]);
        let bare = newest(
            &serde_json::to_vec(&bare).unwrap(),
            "0.2.0",
            false,
            Some(Target::MacOs),
        );
        let bare = bare.unwrap().unwrap();
        assert!(bare.asset.is_none() && bare.sums.is_none());
    }

    #[test]
    fn check_reads_the_list_through_the_fetch() {
        let fake = Fake(HashMap::from([(
            "https://api/releases".to_owned(),
            list(&[release("v9.0.0", false, false)]),
        )]));
        let found = check(&fake, "https://api/releases", "0.1.0", false)
            .unwrap()
            .unwrap();
        assert_eq!(found.version, "9.0.0");
        assert_eq!(found.asset.is_some(), Target::current().is_some());
        assert!(check(&fake, "https://api/other", "0.1.0", false).is_err());
    }

    #[test]
    fn downloads_and_checks_the_sums() {
        let package = b"the package".to_vec();
        let name = "turu-9.0.0-linux-x86_64.tar.gz";
        let sums = format!(
            "{}  {name}\n{} *turu-9.0.0-windows-x86_64.zip\nnot a line\n",
            sha256_hex(&package),
            "0".repeat(64)
        );
        let release = Release {
            version: "9.0.0".into(),
            tag: "v9.0.0".into(),
            html_url: String::new(),
            prerelease: false,
            asset: Some(Asset {
                name: name.into(),
                url: "https://dl/pkg".into(),
                size: 11,
            }),
            sums: Some(Asset {
                name: SUMS_NAME.into(),
                url: "https://dl/sums".into(),
                size: 1,
            }),
        };
        let parsed = parse_sums(&sums);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed["turu-9.0.0-windows-x86_64.zip"], "0".repeat(64));

        let dir = tempfile::tempdir().unwrap();
        let mut files = HashMap::from([
            ("https://dl/pkg".to_owned(), package.clone()),
            ("https://dl/sums".to_owned(), sums.into_bytes()),
        ]);
        let path = download(&Fake(files.clone()), &release, &dir.path().join("updates")).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), package);

        files.insert("https://dl/pkg".to_owned(), b"tampered".to_vec());
        let err = download(&Fake(files), &release, &dir.path().join("updates")).unwrap_err();
        assert!(err.contains("damaged"), "{err}");
        assert!(verify("missing", b"", "").unwrap_err().contains("no line"));
    }

    #[test]
    fn records_checks_and_reports_new_releases_once() {
        let release = newest(
            &list(&[release("v9.0.0", false, false)]),
            "0.1.0",
            false,
            None,
        )
        .unwrap();
        assert!(record_check(Ok(release.clone())).is_some(), "new");
        assert!(record_check(Ok(release.clone())).is_none(), "seen already");
        assert!(record_check(Err("offline".into())).is_none());
        let state = state();
        assert_eq!(state.error.as_deref(), Some("offline"));
        assert_eq!(
            state.available, release,
            "a failed check keeps what was found"
        );
        assert_eq!(status_json()["version"], "9.0.0");
        record_check(Ok(None));
        assert!(status_json().is_null());
    }
}
