//! AQW build (Mugetsu): things the release player does on its own, without a launcher:
//! the window title, remembering that the Vulkan driver crashed on this PC (so the next start
//! uses DirectX 12), the "running on the built-in graphics" tip and the new-version notice.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use winit::window::Window;

/// Mugetsu's own version. Keep it equal to `ProductVersion` in desktop/Cargo.toml
/// ([package.metadata.winresource]) and to the GitHub release tag (`v1.1.1`).
pub const VERSION: &str = "1.6.0";

const RELEASES_API: &str = "https://api.github.com/repos/mugetsu-aqw/mugetsu/releases/latest";
const RELEASES_PAGE: &str = "https://github.com/mugetsu-aqw/mugetsu/releases/latest";

// --- Window title -------------------------------------------------------------------------

static TITLE_NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// "Turu {version}" (Turu: was "Mugetsu"), plus notes such as "DirectX 12" or
/// "update 1.0.2 available".
pub fn window_title() -> String {
    let notes = TITLE_NOTES.lock().unwrap_or_else(|e| e.into_inner());
    let name = format!("{} {}", crate::turu::APP_NAME, crate::turu::VERSION);
    if notes.is_empty() {
        name
    } else {
        format!("{name} – {}", notes.join(" · "))
    }
}

pub fn add_title_note(note: impl Into<String>) {
    TITLE_NOTES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(note.into());
}

/// Mugetsu's own files (markers) live next to Ruffle's settings: %LOCALAPPDATA%\ruffle.
/// Always the default directory, as `mugetsu_args` needs it before the options are parsed.
fn config_dir() -> Option<PathBuf> {
    dirs::config_local_dir().map(|dir| dir.join("ruffle"))
}

/// A file named `name` next to Mugetsu's program exists (`verbose-log`, `no-update-check`).
pub fn exe_flag(name: &str) -> bool {
    // Also `name.txt`: Windows hides extensions, so a "new text document" renamed to `name`
    // is really `name.txt`.
    let found = |dir: &Path| dir.join(name).exists() || dir.join(format!("{name}.txt")).exists();
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    if found(dir) {
        return true;
    }
    // In Mugetsu.app the program is Contents/MacOS/Mugetsu, where nobody would put a file:
    // also look next to the app itself.
    #[cfg(target_os = "macos")]
    if let Some(outside) = dir
        .parent()
        .and_then(Path::parent)
        .filter(|bundle| bundle.extension().and_then(|ext| ext.to_str()) == Some("app"))
        .and_then(Path::parent)
    {
        return found(outside);
    }
    false
}

// --- Vulkan driver crashes ----------------------------------------------------------------
//
// Some graphics drivers crash inside their Vulkan driver while Mugetsu starts (e.g. Intel's
// igvk64.dll on 6th-10th gen laptop graphics), which used to make Mugetsu vanish without a
// word. `windows::install_crash_handler` catches that crash; if it happened in a driver
// while Vulkan was starting, it saves which file crashed, and from then on Mugetsu starts on
// DirectX 12 on this PC. Vulkan is tried again after a graphics driver update (the crashed
// file's path or date changes: drivers live in a new folder per version).

const VULKAN_CRASH_MARKER: &str = "mugetsu-vulkan-crashed.txt";

/// How long after the first frame a crash still counts as "while Vulkan was starting".
const VULKAN_STARTUP_WINDOW: Duration = Duration::from_secs(10);

static VULKAN_STARTING: AtomicBool = AtomicBool::new(false);
static FIRST_PRESENT: OnceLock<Instant> = OnceLock::new();

/// Called before the graphics API is chosen.
pub fn vulkan_startup_begin(vulkan_wanted: bool) {
    VULKAN_STARTING.store(vulkan_wanted, Ordering::SeqCst);
}

/// Called after every presented frame.
pub fn frame_presented() {
    if VULKAN_STARTING.load(Ordering::Relaxed)
        && FIRST_PRESENT.get_or_init(Instant::now).elapsed() > VULKAN_STARTUP_WINDOW
    {
        VULKAN_STARTING.store(false, Ordering::Relaxed);
    }
}

#[cfg(windows)]
pub fn vulkan_starting() -> bool {
    VULKAN_STARTING.load(Ordering::SeqCst)
}

/// When the driver file was last changed (installed), as seconds since 1970.
fn modified_secs(file: &Path) -> Option<u64> {
    let modified = std::fs::metadata(file).ok()?.modified().ok()?;
    Some(modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
}

/// Remembers that the Vulkan driver crashed in `module` (full path of the crashed file).
/// Only Windows has the crash handler that calls this (see `windows::install_crash_handler`).
#[cfg(windows)]
pub fn save_vulkan_crash(module: &Path) {
    // A chosen "Vulkan" would bring the crash straight back on the restart.
    if saved_graphics() == Some(Graphics::Vulkan) {
        save_graphics(None);
    }
    if let Some(dir) = config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join(VULKAN_CRASH_MARKER),
            format!(
                "{VERSION}\n{}\n{}\n",
                module.display(),
                modified_secs(module).unwrap_or(0)
            ),
        );
    }
}

/// The graphics API the player picked in the title-bar menu (`windows::add_graphics_menu`),
/// saved in this file as "vulkan" or "dx12"; no file means automatic. On macOS the file holds
/// "metal" (the only backend the AQW build offers there).
const GRAPHICS_CHOICE_FILE: &str = "mugetsu-graphics.txt";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Graphics {
    #[cfg(not(target_os = "macos"))]
    Vulkan,
    #[cfg(not(target_os = "macos"))]
    Dx12,
    #[cfg(target_os = "linux")]
    Gl,
    #[cfg(target_os = "macos")]
    Metal,
}

impl Graphics {
    pub fn arg(self) -> &'static str {
        match self {
            #[cfg(not(target_os = "macos"))]
            Graphics::Vulkan => "vulkan",
            #[cfg(not(target_os = "macos"))]
            Graphics::Dx12 => "dx12",
            #[cfg(target_os = "linux")]
            Graphics::Gl => "gl",
            #[cfg(target_os = "macos")]
            Graphics::Metal => "metal",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            #[cfg(not(target_os = "macos"))]
            Graphics::Vulkan => "Vulkan",
            #[cfg(not(target_os = "macos"))]
            Graphics::Dx12 => "DirectX 12",
            #[cfg(target_os = "linux")]
            Graphics::Gl => "OpenGL",
            #[cfg(target_os = "macos")]
            Graphics::Metal => "Metal",
        }
    }
}

/// Whether Mugetsu runs with its own AQW settings (no movie given on the command line), where
/// its graphics choice applies (see `main::mugetsu_args`).
static PLAYS_AQW: AtomicBool = AtomicBool::new(false);

pub fn set_plays_aqw() {
    PLAYS_AQW.store(true, Ordering::Relaxed);
}

pub fn plays_aqw() -> bool {
    PLAYS_AQW.load(Ordering::Relaxed)
}

pub fn saved_graphics() -> Option<Graphics> {
    let text = std::fs::read_to_string(config_dir()?.join(GRAPHICS_CHOICE_FILE)).ok()?;
    match text.trim() {
        #[cfg(not(target_os = "macos"))]
        "vulkan" => Some(Graphics::Vulkan),
        #[cfg(not(target_os = "macos"))]
        "dx12" => Some(Graphics::Dx12),
        #[cfg(target_os = "linux")]
        "gl" => Some(Graphics::Gl),
        #[cfg(target_os = "macos")]
        "metal" => Some(Graphics::Metal),
        _ => None,
    }
}

/// Only Windows and Linux have a graphics choice in the title-bar menu; macOS always uses Metal.
#[cfg(any(windows, target_os = "linux"))]
pub fn save_graphics(choice: Option<Graphics>) {
    let Some(dir) = config_dir() else {
        return;
    };
    let path = dir.join(GRAPHICS_CHOICE_FILE);
    match choice {
        Some(graphics) => {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(path, format!("{}\n", graphics.arg()));
        }
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Frame interpolation (title-bar menu, off by default; see `ruffle_core::frame_interpolation`):
/// this file exists while it's on.
const INTERPOLATION_FILE: &str = "mugetsu-interpolation.txt";

pub fn saved_interpolation() -> bool {
    config_dir().is_some_and(|dir| dir.join(INTERPOLATION_FILE).exists())
}

pub fn save_interpolation(on: bool) {
    let Some(dir) = config_dir() else {
        return;
    };
    let path = dir.join(INTERPOLATION_FILE);
    if on {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(path, "on\n");
    } else {
        let _ = std::fs::remove_file(path);
    }
}

/// Edge smoothing (Mugetsu's settings): "flash" for Flash-style, "msaa" for MSAA; no file means
/// the default (MSAA, or Flash-style with a `flash-aa` file next to the exe).
const EDGES_FILE: &str = "mugetsu-edges.txt";

pub fn saved_flash_edges() -> Option<bool> {
    let text = std::fs::read_to_string(config_dir()?.join(EDGES_FILE)).ok()?;
    match text.trim() {
        "flash" => Some(true),
        "msaa" => Some(false),
        _ => None,
    }
}

pub fn save_flash_edges(on: bool) {
    let Some(dir) = config_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(EDGES_FILE), if on { "flash\n" } else { "msaa\n" });
}

/// A choice in Mugetsu's settings, in the game's right-click menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Interpolation(bool),
    FlashEdges(bool),
    Graphics(Option<Graphics>),
    ShowFps(bool),
    OpenLogFolder,
}

/// The settings as they are now, for the menu.
#[derive(Clone, Copy, Debug)]
pub struct MenuSettings {
    pub interpolation: bool,
    pub flash_edges: bool,
    pub graphics: Option<Graphics>,
    /// Whether the graphics choice applies (Windows and Linux, playing AQW).
    pub graphics_choice: bool,
    pub show_fps: bool,
}

impl MenuSettings {
    pub fn now() -> Self {
        Self {
            interpolation: ruffle_core::frame_interpolation::enabled(),
            flash_edges: ruffle_render::tessellator::flash_aa_switch(),
            graphics: saved_graphics(),
            graphics_choice: (cfg!(windows) || cfg!(target_os = "linux")) && plays_aqw(),
            show_fps: show_fps(),
        }
    }
}

/// The FPS counter (Mugetsu's settings; off by default): this file exists while it's on.
const SHOW_FPS_FILE: &str = "mugetsu-show-fps.txt";

static SHOW_FPS: AtomicBool = AtomicBool::new(false);

/// Frames shown per second, for the counter (see `app::MainWindow::count_fps`), as f32 bits.
pub static SHOWN_FPS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn load_show_fps() {
    let on = config_dir().is_some_and(|dir| dir.join(SHOW_FPS_FILE).exists());
    SHOW_FPS.store(on, Ordering::Relaxed);
}

pub fn show_fps() -> bool {
    SHOW_FPS.load(Ordering::Relaxed)
}

pub fn set_show_fps(on: bool) {
    SHOW_FPS.store(on, Ordering::Relaxed);
    let Some(dir) = config_dir() else {
        return;
    };
    let path = dir.join(SHOW_FPS_FILE);
    if on {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(path, "on
");
    } else {
        let _ = std::fs::remove_file(path);
    }
}

/// Where the log is written (set at start), for "Open log folder".
static LOG_FOLDER: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn set_log_folder(path: PathBuf) {
    let _ = LOG_FOLDER.set(path);
}

/// Opens the folder holding ruffle.log in the file manager.
pub fn open_log_folder() {
    let Some(folder) = LOG_FOLDER.get() else {
        return;
    };
    #[cfg(windows)]
    let result = std::process::Command::new("explorer").arg(folder).spawn().map(|_| ());
    #[cfg(not(windows))]
    let result = webbrowser::open(&folder.to_string_lossy());
    if let Err(e) = result {
        tracing::warn!("Couldn't open the log folder {folder:?}: {e}");
    }
}

/// The file the Vulkan driver crashed in on this PC, if that still applies: the same driver
/// file (a driver update installs to a new folder, or at least replaces the file). A stale
/// record is deleted. Not retried for a new Mugetsu version: the crashes seen so far (Intel
/// UHD 620, also with the newest driver and in stock Ruffle) are driver bugs Mugetsu can't fix.
pub fn vulkan_crashed_before() -> Option<PathBuf> {
    let marker = config_dir()?.join(VULKAN_CRASH_MARKER);
    let text = std::fs::read_to_string(&marker).ok()?;
    let mut lines = text.lines();
    let _version = lines.next()?;
    let module = PathBuf::from(lines.next()?);
    let modified = lines.next().and_then(|secs| secs.parse::<u64>().ok());
    if modified.is_some() && modified_secs(&module) == modified {
        Some(module)
    } else {
        let _ = std::fs::remove_file(&marker);
        None
    }
}

// --- Intel graphics -----------------------------------------------------------------------

/// The name of the graphics chip Mugetsu will draw with (the one Windows lists first for
/// high performance, or for low power with `--power low`), if it is Intel's. Intel's Vulkan
/// driver crashed in different ways on several of its chips (UHD 620 when creating the
/// device, Iris Xe with "out of memory" and a lost device while the first room loaded), while
/// its DirectX 12 driver ran fine, and since Mugetsu 1.1 DirectX 12 runs about as well as
/// Vulkan, so Intel graphics default to DirectX 12 (see `mugetsu_args`). Asks DXGI only; no
/// Vulkan or DirectX device is created.
///
/// Also Qualcomm graphics (Snapdragon laptops on Windows on ARM): a player's Adreno showed only
/// a black window with Vulkan and works with DirectX 12 (2026-10, IdeaPad 5 2-in-1).
pub fn intel_graphics(low_power: bool) -> Option<String> {
    let (vendor, name) = preferred_adapter(low_power)?;
    // Intel; Qualcomm reports its PCI id 0x5143 or "QCOM" (0x4D4F4351).
    matches!(vendor, 0x8086 | 0x5143 | 0x4D4F_4351).then_some(name)
}

/// The PCI vendor id and name of the graphics chip Windows lists first for high performance
/// (or low power), skipping software adapters.
#[cfg(windows)]
fn preferred_adapter(low_power: bool) -> Option<(u32, String)> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE,
        DXGI_GPU_PREFERENCE_MINIMUM_POWER, IDXGIAdapter1, IDXGIFactory6,
    };
    let preference = if low_power {
        DXGI_GPU_PREFERENCE_MINIMUM_POWER
    } else {
        DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE
    };
    // SAFETY: plain DXGI queries; every returned interface is reference counted.
    unsafe {
        let factory: IDXGIFactory6 = CreateDXGIFactory1().ok()?;
        for index in 0.. {
            let adapter: IDXGIAdapter1 = factory.EnumAdapterByGpuPreference(index, preference).ok()?;
            let desc = adapter.GetDesc1().ok()?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let name = String::from_utf16_lossy(&desc.Description)
                .trim_end_matches('\0')
                .to_string();
            return Some((desc.VendorId, name));
        }
        None
    }
}

#[cfg(not(windows))]
fn preferred_adapter(_low_power: bool) -> Option<(u32, String)> {
    None
}

// --- Drawing speed-up switches -----------------------------------------------------------
//
// On a Snapdragon laptop (Adreno X1-45, DirectX 12) parts of maps went missing (Yulgar's carpet
// and curtains, stars in the void maps) and lines of quest text flickered away, while the same
// maps drew fine on other graphics. Mugetsu's own drawing speed-ups are the likely cause, so
// each can be turned off with a file next to the exe, to find out which (not yet confirmed on
// Adreno, so nothing is turned off automatically).

/// The speed-ups' switches (environment variables read when they're first used).
const SAFE_DRAWING: [&str; 5] = [
    "RUFFLE_NO_SHARED_SHAPES",
    "RUFFLE_NO_ATLAS",
    "RUFFLE_NO_CACHE_BATCH",
    "RUFFLE_NO_PASS_MERGE",
    "RUFFLE_SLOW_MULTIPLY",
];

/// Called first thing in `main` (before any other thread exists): a `safe-drawing` file next to
/// the exe turns all the speed-ups off (Ruffle's plain ways of drawing); `no-shared-shapes`,
/// `no-pass-merge` and `slow-multiply` turn single ones off (`no-atlas`, `no-glow-batch` and
/// `no-cache-steps` are checked while running).
pub fn apply_drawing_switches() {
    let set = |var: &str| {
        // SAFETY: called first thing in `main`, before any other thread exists.
        unsafe { std::env::set_var(var, "1") };
    };
    if exe_flag("safe-drawing") {
        SAFE_DRAWING.iter().copied().for_each(set);
    }
    for (file, var) in [
        ("no-shared-shapes", "RUFFLE_NO_SHARED_SHAPES"),
        ("no-pass-merge", "RUFFLE_NO_PASS_MERGE"),
        ("slow-multiply", "RUFFLE_SLOW_MULTIPLY"),
        // Flash-style edge smoothing instead of MSAA (prototype).
        ("flash-aa", "RUFFLE_FLASH_AA"),
    ] {
        if exe_flag(file) {
            set(var);
        }
    }
}

/// Whether shapes not drawn for a while have their GPU geometry freed (see
/// `app::memory_watch`). A `no-shape-eviction` file next to the exe turns it off.
pub fn shape_eviction_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        let enabled = !exe_flag("no-shape-eviction");
        if !enabled {
            tracing::info!(target: "ruffle_aqw_perf", "Shape eviction switched off");
        }
        enabled
    })
}

/// How long a frame waits for the shapes it draws for the first time before it's drawn
/// without those still being tessellated (see `deferred_shapes` in ruffle_core): one very
/// detailed armor shape took 60-85 ms, freezing the frame a room was entered in. A
/// `wait-for-shapes` file next to the exe waits for all of them (as before, for A/B);
/// `RUFFLE_SHAPE_WAIT_MS` sets another wait (testing).
pub fn shape_wait() -> Option<std::time::Duration> {
    if exe_flag("wait-for-shapes") {
        tracing::info!(target: "ruffle_aqw_perf", "Waiting for every new shape (wait-for-shapes)");
        return None;
    }
    let ms = std::env::var("RUFFLE_SHAPE_WAIT_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .unwrap_or(12);
    Some(std::time::Duration::from_millis(ms))
}

/// The speed-ups that are switched off, for the log.
pub fn drawing_switches_off() -> Vec<&'static str> {
    SAFE_DRAWING
        .into_iter()
        .filter(|var| std::env::var_os(var).is_some())
        .collect()
}

// --- Graphics memory ------------------------------------------------------------------------

/// The graphics chip Mugetsu draws with (PCI vendor and device id, from wgpu), to find it in
/// DXGI for `video_memory`.
static ADAPTER_IDS: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();

#[cfg(not(windows))]
static VIDEO_ADAPTER: std::sync::OnceLock<wgpu::Adapter> = std::sync::OnceLock::new();

pub fn set_adapter_ids(vendor: u32, device: u32) {
    let _ = ADAPTER_IDS.set((vendor, device));
}

#[cfg(not(windows))]
pub fn set_video_memory_adapter(adapter: wgpu::Adapter) {
    let _ = VIDEO_ADAPTER.set(adapter);
}

/// How much of the graphics chip's own memory Windows lets this process use, and how much it
/// uses (bytes), whichever graphics API draws. Windows counts every GPU memory block Mugetsu
/// holds, used or not; past the budget it moves memory to system RAM, which is very slow.
#[cfg(windows)]
pub fn video_memory() -> Option<(u64, u64)> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
        IDXGIAdapter1, IDXGIAdapter3, IDXGIFactory1,
    };
    use windows::core::Interface as _;
    thread_local! {
        // The chip, found once (it's asked every few seconds).
        static ADAPTER: std::cell::RefCell<Option<IDXGIAdapter3>> =
            const { std::cell::RefCell::new(None) };
    }
    let find = || -> Option<IDXGIAdapter3> {
        let (vendor, device) = *ADAPTER_IDS.get()?;
        // SAFETY: plain DXGI queries; every returned interface is reference counted.
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
            for index in 0.. {
                let adapter: IDXGIAdapter1 = factory.EnumAdapters1(index).ok()?;
                let desc = adapter.GetDesc1().ok()?;
                if desc.VendorId == vendor && desc.DeviceId == device {
                    return adapter.cast().ok();
                }
            }
            None
        }
    };
    ADAPTER.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            *cell = find();
        }
        let adapter = cell.as_ref()?;
        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        // SAFETY: a plain query on a live adapter.
        unsafe { adapter.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info) }
            .ok()?;
        Some((info.Budget, info.CurrentUsage))
    })
}

#[cfg(not(windows))]
pub fn video_memory() -> Option<(u64, u64)> {
    VIDEO_ADAPTER
        .get()
        .and_then(ruffle_render_wgpu::backend::adapter_memory)
}

/// A smaller graphics memory budget to test the memory watch with on a big card: a file
/// `test-vram-mb.txt` next to the exe with a number of MB.
pub fn test_video_budget() -> Option<u64> {
    let exe = std::env::current_exe().ok()?;
    let text = std::fs::read_to_string(exe.parent()?.join("test-vram-mb.txt")).ok()?;
    Some(text.trim().parse::<u64>().ok()? << 20)
}

// --- "Quality Low" tip on built-in graphics -----------------------------------------------
//
// Built-in graphics run AQW several times faster on Quality Low (no edge smoothing): an Intel
// UHD 620 went from ~8 to a locked 30 FPS in a crowded room, and Iris Xe also drew white boxes
// at Medium/High. Many players never open AQW's Options. So when the game runs on built-in
// graphics at Medium/High and stays well below its frame rate for 15 seconds, Mugetsu tells the
// player once (per PC) how to switch. It never changes the setting itself: AQW keeps its own
// copy of it and re-applies that on login and in its Options, so only AQW's Options can change
// it cleanly.

/// The built-in graphics chip Mugetsu draws with, if it is one.
static BUILT_IN_GRAPHICS: std::sync::OnceLock<String> = std::sync::OnceLock::new();

static LOW_TIP_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SLOW_WINDOWS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Perf windows seen (the test switch waits a while, so there's time to log in first).
static WINDOWS_SEEN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

const LOW_TIP_MARKER: &str = "mugetsu-low-quality-tip-shown";

/// Called on the main thread after each 5-second perf window, with that window's drawn FPS.
/// `RUFFLE_TEST_LOW_PROMPT=1` shows the tip on any graphics, ~30 s after starting (testing).
pub fn low_quality_tick(player: &mut ruffle_core::Player, drawn_fps: f64) {
    use ruffle_render::quality::StageQuality;
    use std::sync::atomic::Ordering;
    if LOW_TIP_DONE.load(Ordering::Relaxed) {
        return;
    }
    let windows_seen = WINDOWS_SEEN.fetch_add(1, Ordering::Relaxed) + 1;
    let test = std::env::var_os("RUFFLE_TEST_LOW_PROMPT").is_some();
    if test && windows_seen < 6 {
        return;
    }
    let Some(chip) = BUILT_IN_GRAPHICS
        .get()
        .cloned()
        .or_else(|| test.then(|| "test".to_string()))
    else {
        return;
    };
    let quality = player.quality();
    // Below 1 FPS drawn: minimized or hidden, not slow.
    let slow = drawn_fps >= 1.0 && drawn_fps < 0.8 * player.frame_rate();
    if quality == StageQuality::Low || !(slow || test) {
        SLOW_WINDOWS.store(0, Ordering::Relaxed);
        return;
    }
    if SLOW_WINDOWS.fetch_add(1, Ordering::Relaxed) + 1 < 3 && !test {
        return;
    }
    LOW_TIP_DONE.store(true, Ordering::Relaxed);
    let Some(dir) = config_dir() else { return };
    let marker = dir.join(LOW_TIP_MARKER);
    if marker.exists() && !test {
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&marker, &chip);
    let quality_name = match quality {
        StageQuality::Medium => "Medium",
        _ => "High",
    };
    tracing::info!(
        "Suggesting Quality Low: {drawn_fps:.1} FPS drawn on {chip} at {quality_name}"
    );
    let _ = std::thread::Builder::new()
        .name("low quality tip".to_string())
        .spawn(move || {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Info)
                .set_title("Mugetsu")
                .set_description(format!(
                    "AQW is running on your built-in graphics ({chip}) with Quality set to \
                    {quality_name}, which is slow on built-in graphics.\n\n\
                    For much better FPS, set Quality to Low: open AQW's Options and change \
                    Quality to Low. Edges are a little less smooth, but built-in graphics \
                    usually run AQW several times faster.\n\n\
                    Mugetsu won't show this again."
                ))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        });
}

// --- Built-in graphics tip ----------------------------------------------------------------

/// Mugetsu asks for the fast graphics card, but on laptops the graphics driver's or Windows'
/// per-program setting can still put it on the built-in graphics, which is many times slower.
/// If that happened and the PC has a dedicated card, says so (once) and how to switch.
pub fn check_gpu_choice(window: Arc<Window>, chosen: &wgpu::AdapterInfo, power: wgpu::PowerPreference) {
    if chosen.device_type == wgpu::DeviceType::IntegratedGpu {
        let _ = BUILT_IN_GRAPHICS.set(chosen.name.clone());
    }
    // The tip below is about Windows' per-program graphics setting, which macOS doesn't have.
    // (The Quality tip for built-in graphics still applies on macOS; it uses the chip above.)
    if cfg!(target_os = "macos") {
        return;
    }
    // `RUFFLE_TEST_GPU_TIP=1` shows the tip whenever the PC has a dedicated card (testing).
    let test = std::env::var_os("RUFFLE_TEST_GPU_TIP").is_some();
    if !test
        && (chosen.device_type != wgpu::DeviceType::IntegratedGpu
            || power != wgpu::PowerPreference::HighPerformance)
    {
        return;
    }
    let chosen_name = chosen.name.clone();
    let _ = std::thread::Builder::new()
        .name("gpu check".to_string())
        .spawn(move || {
            // DirectX lists every graphics card; Vulkan can hide one a per-program setting
            // turned off for Mugetsu.
            let backends = if cfg!(windows) {
                wgpu::Backends::DX12
            } else {
                wgpu::Backends::VULKAN
            };
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let Some(dedicated) = futures::executor::block_on(instance.enumerate_adapters(backends))
                .into_iter()
                .map(|adapter| adapter.get_info())
                .find(|info| info.device_type == wgpu::DeviceType::DiscreteGpu)
            else {
                return;
            };
            tracing::warn!(
                "Running on the built-in graphics ({chosen_name}) although this PC has {}",
                dedicated.name
            );
            add_title_note("built-in graphics, slower");
            window.set_title(&window_title());

            let Some(dir) = config_dir() else { return };
            let marker = dir.join("mugetsu-gpu-tip-shown");
            if marker.exists() {
                return;
            }
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&marker, &dedicated.name);
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Mugetsu")
                .set_description(format!(
                    "Mugetsu is running on your built-in graphics ({chosen_name}) instead of \
                    your {}, which is much faster.\n\n\
                    To switch: open Windows Settings > System > Display > Graphics, add \
                    Mugetsu.exe (Browse), click it > Options > High performance > Save. On \
                    NVIDIA laptops also check NVIDIA Control Panel > Manage 3D settings > \
                    Program settings. Then restart Mugetsu.\n\n\
                    This message is only shown once.",
                    dedicated.name
                ))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        });
}

// --- New version notice -------------------------------------------------------------------

/// Asks GitHub for the latest release once at startup. If it is newer, the window title says
/// so and a message (once per version) offers to open the download page. Nothing is
/// downloaded or installed; a `no-update-check` file next to Mugetsu.exe turns this off.
pub fn spawn_update_check(window: Arc<Window>) {
    if exe_flag("no-update-check") {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("update check".to_string())
        .spawn(move || {
            let latest = match latest_release() {
                Ok(latest) => latest,
                Err(error) => {
                    tracing::info!("Update check failed: {error}");
                    return;
                }
            };
            let Some(latest_number) = parse_version(&latest) else {
                return;
            };
            // `RUFFLE_TEST_UPDATE_FROM=0.9` pretends to be that version (testing).
            let current = std::env::var("RUFFLE_TEST_UPDATE_FROM").unwrap_or(VERSION.to_string());
            if Some(latest_number) <= parse_version(&current) {
                tracing::info!("Mugetsu {current} is up to date (latest release: {latest})");
                return;
            }
            let latest = latest.trim_start_matches('v').to_string();
            tracing::info!("Mugetsu {latest} is available");
            add_title_note(format!("update {latest} available"));
            window.set_title(&window_title());

            let Some(dir) = config_dir() else { return };
            let marker = dir.join("mugetsu-update-notified");
            if std::fs::read_to_string(&marker).is_ok_and(|seen| seen.trim() == latest) {
                return;
            }
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&marker, &latest);
            let answer = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Info)
                .set_title("Mugetsu")
                .set_description(format!(
                    "Mugetsu {latest} is available (you have {current}).\n\n\
                    Open the download page?"
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if answer == rfd::MessageDialogResult::Yes {
                let _ = webbrowser::open(RELEASES_PAGE);
            }
        });
}

fn latest_release() -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("Mugetsu/{VERSION}"))
        .build()?;
    let body = client
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .text()?;
    let release: serde_json::Value = serde_json::from_str(&body)?;
    let tag = release["tag_name"]
        .as_str()
        .ok_or("no tag_name in the release")?;
    Ok(tag.to_string())
}

/// "v1.0.2" / "1.0.2" -> (1, 0, 2); missing parts are 0.
fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let mut next = || -> Option<u32> { parts.next().map_or(Some(0), |p| p.parse().ok()) };
    Some((next()?, next()?, next()?))
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.0.2"), Some((1, 0, 2)));
        assert_eq!(parse_version("1.1"), Some((1, 1, 0)));
        assert_eq!(parse_version("v2"), Some((2, 0, 0)));
        assert_eq!(parse_version("v1.0-beta"), None);
        assert!(parse_version("v1.0.10") > parse_version("1.0.9"));
    }
}

#[cfg(all(test, windows))]
mod intel_tests {
    /// Prints what the Intel check sees on this PC (run with `--nocapture`).
    #[test]
    fn intel_graphics_on_this_pc() {
        for low_power in [false, true] {
            let start = std::time::Instant::now();
            let result = super::intel_graphics(low_power);
            println!(
                "low_power={low_power}: {result:?} ({:.1} ms)",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}
