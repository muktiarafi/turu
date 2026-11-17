//! AQW build (Mugetsu): things the release player does on its own, without a launcher:
//! the window title, remembering that the Vulkan driver crashed on this PC (so the next start
//! uses DirectX 12) and the "running on the built-in graphics" tip.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use winit::window::Window;

/// Mugetsu's own version. Keep it equal to `ProductVersion` in desktop/Cargo.toml
/// ([package.metadata.winresource]) and to the GitHub release tag (`v1.1.1`).
pub const VERSION: &str = "2.0.0";

// --- Window title -------------------------------------------------------------------------

static TITLE_NOTES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// "Turu {version}" (Turu: was "Mugetsu"), plus notes such as "DirectX 12".
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

/// A file named `name` next to Mugetsu's program exists (`verbose-log`, `no-atlas`).
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
/// Mugetsu 2.0.0: the same while DirectX 12 is starting (see `save_dx12_crash`).
static DX12_STARTING: AtomicBool = AtomicBool::new(false);
static FIRST_PRESENT: OnceLock<Instant> = OnceLock::new();

/// Called before the graphics API is started (again when another one is tried after it).
pub fn graphics_startup_begin(vulkan_wanted: bool, dx12_wanted: bool) {
    VULKAN_STARTING.store(vulkan_wanted, Ordering::SeqCst);
    DX12_STARTING.store(dx12_wanted, Ordering::SeqCst);
}

/// Called after every presented frame.
pub fn frame_presented() {
    if (VULKAN_STARTING.load(Ordering::Relaxed) || DX12_STARTING.load(Ordering::Relaxed))
        && FIRST_PRESENT.get_or_init(Instant::now).elapsed() > VULKAN_STARTUP_WINDOW
    {
        VULKAN_STARTING.store(false, Ordering::Relaxed);
        DX12_STARTING.store(false, Ordering::Relaxed);
    }
}

#[cfg(windows)]
pub fn vulkan_starting() -> bool {
    VULKAN_STARTING.load(Ordering::SeqCst)
}

#[cfg(windows)]
pub fn dx12_starting() -> bool {
    DX12_STARTING.load(Ordering::SeqCst)
}

/// Mugetsu 2.0.0: opens the GPU with `open`, given the backends to try. DirectX 12 is the
/// default on Windows; where it is there but cannot be started (an old graphics chip or
/// driver), Vulkan is tried before giving up. `open` calls `graphics_startup_begin` again with
/// the backend it really starts (another is used where the wanted one isn't there at all).
/// The window, the shell and the clients all open theirs this way, so they end up on the same
/// backend and can share surfaces. (`RUFFLE_TEST_DX12_FAILS`, testing: DirectX 12 is taken as
/// not startable.)
pub fn open_gpu<T>(
    wanted: wgpu::Backends,
    mut open: impl FnMut(wgpu::Backends) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    graphics_startup_begin(
        wanted.contains(wgpu::Backends::VULKAN),
        wanted.contains(wgpu::Backends::DX12),
    );
    let first = if wanted == wgpu::Backends::DX12
        && std::env::var_os("RUFFLE_TEST_DX12_FAILS").is_some()
    {
        Err(anyhow::anyhow!("RUFFLE_TEST_DX12_FAILS is set"))
    } else {
        open(wanted)
    };
    match first {
        Err(error) if wanted == wgpu::Backends::DX12 => {
            tracing::warn!("DirectX 12 could not be started ({error}); trying Vulkan");
            graphics_startup_begin(true, false);
            open(wgpu::Backends::VULKAN)
        }
        opened => opened,
    }
}

/// Mugetsu 2.0.0: DirectX 12 was wanted and `backend` draws instead: logged and said in the
/// title (Vulkan runs as well, so nothing is asked). Whether it was.
pub fn note_dx12_fallback(wanted: wgpu::Backends, backend: wgpu::Backend) -> bool {
    if wanted != wgpu::Backends::DX12 || backend == wgpu::Backend::Dx12 {
        return false;
    }
    let name = match backend {
        wgpu::Backend::Vulkan => "Vulkan",
        wgpu::Backend::Gl => "OpenGL, slower",
        other => other.to_str(),
    };
    tracing::warn!("DirectX 12 is not available; using {name}");
    add_title_note(format!("{name}: DirectX 12 not available"));
    true
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
    write_crash_marker(VULKAN_CRASH_MARKER, module);
}

/// The record of a graphics API that crashed while starting: the crashed file and when it
/// was installed.
#[cfg(windows)]
fn write_crash_marker(marker: &str, module: &Path) {
    if let Some(dir) = config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join(marker),
            format!(
                "{VERSION}\n{}\n{}\n",
                module.display(),
                modified_secs(module).unwrap_or(0)
            ),
        );
    }
}

// --- DirectX 12 crashes ---------------------------------------------------------------------
//
// Mugetsu 2.0.0: DirectX 12 is the default on Windows (it runs as fast or faster than Vulkan
// and its drivers hold far less memory). The same safety net as for Vulkan, the other way
// round: a crash in the graphics driver or in Direct3D's own files while DirectX 12 is starting
// is remembered, and from then on Turu starts on Vulkan on this PC, until the crashed file
// changes (a driver or Windows update).

const DX12_CRASH_MARKER: &str = "mugetsu-dx12-crashed.txt";

/// Remembers that DirectX 12 crashed in `module` (full path of the crashed file) while
/// starting. Only Windows has the crash handler that calls this.
#[cfg(windows)]
pub fn save_dx12_crash(module: &Path) {
    // A chosen "DirectX 12" would bring the crash straight back on the restart.
    if saved_graphics() == Some(Graphics::Dx12) {
        save_graphics(None);
    }
    write_crash_marker(DX12_CRASH_MARKER, module);
}

/// The file DirectX 12 crashed in on this PC while starting, if that still applies (see
/// `vulkan_crashed_before`).
pub fn dx12_crashed_before() -> Option<PathBuf> {
    crash_marker(DX12_CRASH_MARKER)
}

/// Why Automatic graphics chose what it did (`automatic_graphics`), for the log and the title.
#[derive(Debug, Clone, PartialEq)]
pub enum AutomaticGraphics {
    /// DirectX 12: the Vulkan driver crashed while starting before (the crashed file).
    VulkanCrashed(PathBuf),
    /// Vulkan: DirectX 12 crashed while starting before (the crashed file).
    Dx12Crashed(PathBuf),
    /// DirectX 12 on Intel or Qualcomm graphics (the chip's name).
    Intel(String),
    /// The system's default: DirectX 12 on Windows, Vulkan elsewhere.
    Default,
}

impl AutomaticGraphics {
    /// The `--graphics` value.
    pub fn arg(&self) -> &'static str {
        match self {
            Self::VulkanCrashed(_) | Self::Intel(_) => "dx12",
            Self::Dx12Crashed(_) => "vulkan",
            Self::Default if cfg!(windows) => "dx12",
            Self::Default => "vulkan",
        }
    }
}

/// Mugetsu 2.0.0: the graphics API with no choice saved, on Windows and Linux. DirectX 12 is
/// the default on Windows (as fast or faster than Vulkan in the game, with far less memory held
/// by the graphics driver); Vulkan on Linux, and on Windows where DirectX 12 crashed while
/// starting (it can't be started at all: `open_gpu`). A Vulkan crash wins over a DirectX 12
/// one (with both, DirectX 12 as before), and a DirectX 12 crash over the Intel rule (a DirectX
/// 12 that crashes at start is no use there either). `intel` (`intel_graphics`) is asked last,
/// as it queries DXGI; the Intel rule stays for what it logs.
pub fn automatic_graphics(
    vulkan_crashed: Option<PathBuf>,
    dx12_crashed: impl FnOnce() -> Option<PathBuf>,
    intel: impl FnOnce() -> Option<String>,
) -> AutomaticGraphics {
    if let Some(module) = vulkan_crashed {
        AutomaticGraphics::VulkanCrashed(module)
    } else if let Some(module) = dx12_crashed() {
        AutomaticGraphics::Dx12Crashed(module)
    } else if let Some(name) = intel() {
        AutomaticGraphics::Intel(name)
    } else {
        AutomaticGraphics::Default
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
    /// Every choice this system offers.
    pub const ALL: &[Graphics] = &[
        #[cfg(not(target_os = "macos"))]
        Graphics::Vulkan,
        #[cfg(not(target_os = "macos"))]
        Graphics::Dx12,
        #[cfg(target_os = "linux")]
        Graphics::Gl,
        #[cfg(target_os = "macos")]
        Graphics::Metal,
    ];

    /// The choice `arg` names (`Graphics::arg`).
    pub fn from_arg(arg: &str) -> Option<Graphics> {
        Graphics::ALL.iter().copied().find(|graphics| graphics.arg() == arg)
    }

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
    Graphics::from_arg(text.trim())
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

/// A choice in Mugetsu's settings, in the game's right-click menu (and Options > Application).
/// (Turu: Mugetsu 2.0.0's, less its update reminders, its restart questions and the render
/// thread's restart: Turu has its own updater and asks about a graphics restart itself.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Interpolation(bool),
    Graphics(Option<Graphics>),
    ShowFps(bool),
    /// Uncapped frame pacing (see `uncapped`).
    Uncapped(bool),
    FpsLimit(FpsLimit),
    /// Performance mode (see `Performance`).
    Performance(Performance),
    /// The memory limit in MB, 0 for none (see `memory_limit_mb`).
    MemoryLimit(u32),
    /// Everything that can be freed is freed now (see `app::free_memory`).
    FreeMemory,
}

/// The settings as they are now, for the menu.
#[derive(Clone, Copy, Debug)]
pub struct MenuSettings {
    pub interpolation: bool,
    pub graphics: Option<Graphics>,
    /// Whether the graphics choice applies (Windows and Linux, playing AQW).
    pub graphics_choice: bool,
    pub show_fps: bool,
    pub uncapped: bool,
    pub fps_limit: FpsLimit,
    pub performance: Performance,
    pub memory_limit_mb: u32,
}

impl MenuSettings {
    pub fn now() -> Self {
        Self {
            interpolation: ruffle_core::frame_interpolation::enabled(),
            graphics: saved_graphics(),
            graphics_choice: (cfg!(windows) || cfg!(target_os = "linux")) && plays_aqw(),
            show_fps: show_fps(),
            uncapped: uncapped(),
            fps_limit: fps_limit(),
            performance: performance(),
            memory_limit_mb: memory_limit_mb(),
        }
    }
}

// --- Memory limit -------------------------------------------------------------------------
//
// Mugetsu keeps things it may need again: the shapes of rooms and players seen in the last
// minutes, spare textures, and memory its allocator holds for the next request. On a PC with
// little RAM a player may want less of that than Mugetsu would keep by itself. With a limit
// set (the menu's "Memory limit"), `app::memory_watch` frees all of it whenever the process's
// committed memory is over the limit. It is a target, not a cap: what is on screen and the
// game's own data stay, and what was freed is made again when it is needed, which can show as
// a short stutter. Off by default.

/// Holds the limit in MB while one is set; no file means none.
const MEMORY_LIMIT_FILE: &str = "mugetsu-memory-limit.txt";

static MEMORY_LIMIT_MB: AtomicU32 = AtomicU32::new(0);

/// Reads the saved limit. (`RUFFLE_TEST_MEMORY_LIMIT_MB=<n>`, testing: instead of it.)
pub fn load_memory_limit() {
    let saved = std::env::var("RUFFLE_TEST_MEMORY_LIMIT_MB").ok().or_else(|| {
        config_dir().and_then(|dir| std::fs::read_to_string(dir.join(MEMORY_LIMIT_FILE)).ok())
    });
    let mb = saved
        .and_then(|text| text.trim().parse::<u32>().ok())
        .unwrap_or(0);
    MEMORY_LIMIT_MB.store(mb, Ordering::Relaxed);
}

/// The memory limit in MB; 0 when there is none.
pub fn memory_limit_mb() -> u32 {
    MEMORY_LIMIT_MB.load(Ordering::Relaxed)
}

pub fn set_memory_limit_mb(mb: u32) {
    MEMORY_LIMIT_MB.store(mb, Ordering::Relaxed);
    let Some(dir) = config_dir() else {
        return;
    };
    let path = dir.join(MEMORY_LIMIT_FILE);
    if mb == 0 {
        let _ = std::fs::remove_file(path);
    } else {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(path, mb.to_string());
    }
}

/// The process's memory in MB: its working set (the RAM it holds right now; Task Manager's
/// "Memory" column is this without the pages shared with other processes) and its committed
/// memory (everything the system has promised it, which on Windows includes the backing of
/// its graphics memory: 2,500 MB against a working set of 1,100 in a town on DirectX 12).
///
/// The memory limit counted the committed memory at first (freeing moves it by 250-900 MB
/// and the working set by about 100, measured 2026-11-15). The user's decision of
/// 2026-11-16: it counts what Task Manager shows (`task_manager_mb`), so that the limit, the
/// menu's "Now:" line and the number people look at are the same one, with presets of 1, 2
/// and 3 GB. What brings that figure down in a long session is the freeing of unused movie
/// libraries (core's `Library::sweep_movie_libraries`), not this limit's own freeing.
pub fn process_memory_mb() -> (u64, u64) {
    let mut working_set = 0usize;
    let mut commit = 0usize;
    let mut zero = [0usize; 6];
    // SAFETY: plain out-pointers.
    unsafe {
        let [a, b, c, e, f, g] = &mut zero;
        libmimalloc_sys::mi_process_info(a, b, c, &mut working_set, e, &mut commit, f, g);
    }
    ((working_set as u64) >> 20, (commit as u64) >> 20)
}

/// The figure Task Manager's "Memory" column shows for the process, in MB: on Windows its
/// private working set (the RAM it holds that no other process shares), from the system; where
/// that cannot be read (Windows before 10 22H2, other systems) the whole working set, which
/// is a little more.
pub fn task_manager_mb() -> u64 {
    #[cfg(windows)]
    {
        // `PROCESS_MEMORY_COUNTERS_EX2` (psapi.h): the last two fields are what EX2 adds.
        #[repr(C)]
        #[derive(Default)]
        struct Counters {
            cb: u32,
            page_fault_count: u32,
            peak_working_set_size: usize,
            working_set_size: usize,
            quota_peak_paged_pool_usage: usize,
            quota_paged_pool_usage: usize,
            quota_peak_non_paged_pool_usage: usize,
            quota_non_paged_pool_usage: usize,
            pagefile_usage: usize,
            peak_pagefile_usage: usize,
            private_usage: usize,
            private_working_set_size: usize,
            shared_commit_usage: u64,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn K32GetProcessMemoryInfo(
                process: *mut std::ffi::c_void,
                counters: *mut Counters,
                cb: u32,
            ) -> i32;
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
        }
        let mut counters = Counters {
            cb: size_of::<Counters>() as u32,
            ..Default::default()
        };
        // SAFETY: `counters` is a valid, writable structure of the size given; the handle is
        // the pseudo-handle of this process.
        let read = unsafe {
            K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0
        };
        if read && counters.private_working_set_size > 0 {
            return (counters.private_working_set_size as u64) >> 20;
        }
    }
    process_memory_mb().0
}

/// What the memory limit is compared with right now, in MB, for the menu (which is drawn up
/// to a thousand times a second: read once a second at the most).
pub fn memory_now_mb() -> u64 {
    static LAST: std::sync::Mutex<Option<(std::time::Instant, u64)>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    match *last {
        Some((at, mb)) if at.elapsed() < std::time::Duration::from_secs(1) => mb,
        _ => {
            let mb = task_manager_mb();
            *last = Some((std::time::Instant::now(), mb));
            mb
        }
    }
}

// --- Performance mode ---------------------------------------------------------------------
//
// A weak graphics chip (a laptop's built-in one) spends its time filling pixels: the game is
// drawn with hundreds of blended layers over each other. In Performance mode the game is
// drawn at a share of the window's width and height and that picture is stretched over the
// window (the movie's picture is a texture drawn onto the window anyway, see `gui::movie`):
// at 70 % of each side the chip fills half the pixels, which was worth about 40 % more
// pictures a second on built-in graphics. The picture gets softer, the game's own text
// included; Mugetsu's menu and FPS counter stay sharp. Off by default: a graphics card that
// is not the limit gains nothing from it.

/// How much of the window's resolution the game is drawn at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Performance {
    #[default]
    Off,
    Balanced,
    Fast,
}

impl Performance {
    /// The share of the window's width and height the game is drawn at.
    pub fn scale(self) -> f64 {
        match self {
            Performance::Off => 1.0,
            Performance::Balanced => 0.85,
            Performance::Fast => 0.7,
        }
    }

    fn from_saved(text: &str) -> Self {
        match text.trim() {
            "balanced" => Performance::Balanced,
            "fast" => Performance::Fast,
            _ => Performance::Off,
        }
    }

    pub fn from_index(index: u8) -> Self {
        match index {
            1 => Performance::Balanced,
            2 => Performance::Fast,
            _ => Performance::Off,
        }
    }
}

/// Holds "balanced" or "fast" while Performance mode is on; no file means off.
const PERFORMANCE_FILE: &str = "mugetsu-performance.txt";

static PERFORMANCE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Reads the saved choice. (`RUFFLE_TEST_PERFORMANCE=balanced|fast`, testing: instead of it.)
pub fn load_performance() {
    let saved = std::env::var("RUFFLE_TEST_PERFORMANCE").ok().or_else(|| {
        config_dir().and_then(|dir| std::fs::read_to_string(dir.join(PERFORMANCE_FILE)).ok())
    });
    let level = saved.as_deref().map_or(Performance::Off, Performance::from_saved);
    PERFORMANCE.store(level as u8, Ordering::Relaxed);
}

pub fn performance() -> Performance {
    Performance::from_index(PERFORMANCE.load(Ordering::Relaxed))
}

pub fn set_performance(level: Performance) {
    PERFORMANCE.store(level as u8, Ordering::Relaxed);
    let Some(dir) = config_dir() else {
        return;
    };
    let path = dir.join(PERFORMANCE_FILE);
    let text = match level {
        Performance::Off => {
            let _ = std::fs::remove_file(path);
            return;
        }
        Performance::Balanced => "balanced\n",
        Performance::Fast => "fast\n",
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(path, text);
}

/// The share of the window's width and height the game is drawn at (1 without Performance
/// mode).
pub fn render_scale() -> f64 {
    performance().scale()
}

/// A length of the window in the pixels the game is drawn at: the same without Performance
/// mode; never 0 for a length that is not.
pub fn drawn_len(len: u32) -> u32 {
    let scale = render_scale();
    if scale == 1.0 || len == 0 {
        len
    } else {
        ((f64::from(len) * scale).round() as u32).max(1)
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
        let _ = std::fs::write(path, "on\n");
    } else {
        let _ = std::fs::remove_file(path);
    }
}

/// Frame pacing (Mugetsu's settings). Normally pictures are sent in step with the monitor's
/// refreshes (v-sync): a new one on every refresh, or every second, third, ..., and the last
/// one again in between. "Uncapped" sends each new picture once, as soon as it is ready,
/// without waiting for a refresh: for monitors with a variable refresh rate (G-Sync,
/// FreeSync), which show every picture for as long as it took, at any rate. On a monitor
/// with a fixed rate that tears. Only with frame interpolation: without it there is one
/// picture per game frame either way. This file exists while it's on.
const UNCAPPED_FILE: &str = "mugetsu-uncapped.txt";

/// The most pictures a second while uncapped, if one was picked (the file holds `screen` or
/// a number).
const FPS_LIMIT_FILE: &str = "mugetsu-fps-limit.txt";

static UNCAPPED: AtomicBool = AtomicBool::new(false);

/// 0: no limit; 1: the screen's rate; else that many pictures a second.
static FPS_LIMIT: AtomicU32 = AtomicU32::new(0);

/// See `FPS_LIMIT_FILE`. `Screen`: a little under the monitor's refresh rate, where a
/// variable-refresh monitor still follows (at its top rate and above it stops, and tears).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FpsLimit {
    Off,
    Screen,
    Fixed(u32),
}

impl FpsLimit {
    fn parse(text: &str) -> Self {
        match text.trim() {
            "screen" => FpsLimit::Screen,
            number => number
                .parse::<u32>()
                .ok()
                .filter(|fps| (30..=1000).contains(fps))
                .map_or(FpsLimit::Off, FpsLimit::Fixed),
        }
    }

    fn bits(self) -> u32 {
        match self {
            FpsLimit::Off => 0,
            FpsLimit::Screen => 1,
            FpsLimit::Fixed(fps) => fps,
        }
    }
}

/// Reads the saved pacing. `RUFFLE_TEST_UNCAPPED=1` and `RUFFLE_TEST_FPS_LIMIT=screen|N`
/// set it for one run (testing).
pub fn load_pacing() {
    let dir = config_dir();
    let test = std::env::var_os("RUFFLE_TEST_UNCAPPED").is_some();
    let on = test || dir.as_ref().is_some_and(|dir| dir.join(UNCAPPED_FILE).exists());
    UNCAPPED.store(on, Ordering::Relaxed);
    let limit = std::env::var("RUFFLE_TEST_FPS_LIMIT")
        .ok()
        .or_else(|| dir.and_then(|dir| std::fs::read_to_string(dir.join(FPS_LIMIT_FILE)).ok()))
        .map_or(FpsLimit::Off, |text| FpsLimit::parse(&text));
    FPS_LIMIT.store(limit.bits(), Ordering::Relaxed);
    if on {
        tracing::info!("Frame pacing: uncapped (FPS limit: {limit:?})");
    }
}

/// Whether "uncapped" is picked in the settings.
pub fn uncapped() -> bool {
    UNCAPPED.load(Ordering::Relaxed)
}

/// Whether pictures are sent uncapped right now: picked, and frame interpolation is on.
pub fn uncapped_now() -> bool {
    uncapped() && ruffle_core::frame_interpolation::enabled()
}

pub fn set_uncapped(on: bool) {
    UNCAPPED.store(on, Ordering::Relaxed);
    let Some(dir) = config_dir() else {
        return;
    };
    let path = dir.join(UNCAPPED_FILE);
    if on {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(path, "on\n");
    } else {
        let _ = std::fs::remove_file(path);
    }
}

pub fn fps_limit() -> FpsLimit {
    match FPS_LIMIT.load(Ordering::Relaxed) {
        0 => FpsLimit::Off,
        1 => FpsLimit::Screen,
        fps => FpsLimit::Fixed(fps),
    }
}

pub fn set_fps_limit(limit: FpsLimit) {
    FPS_LIMIT.store(limit.bits(), Ordering::Relaxed);
    let Some(dir) = config_dir() else {
        return;
    };
    let path = dir.join(FPS_LIMIT_FILE);
    match limit {
        FpsLimit::Off => {
            let _ = std::fs::remove_file(path);
        }
        FpsLimit::Screen => {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(path, "screen\n");
        }
        FpsLimit::Fixed(fps) => {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(path, format!("{fps}\n"));
        }
    }
}

/// Where the log is written (set at start), for "Open log folder".
static LOG_FOLDER: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn set_log_folder(path: PathBuf) {
    let _ = LOG_FOLDER.set(path);
}

/// The folder holding ruffle.log (and Turu's crash files), once known.
pub fn log_folder() -> Option<&'static std::path::Path> {
    LOG_FOLDER.get().map(PathBuf::as_path)
}

/// Opens the folder holding ruffle.log in the file manager.
pub fn open_log_folder() {
    if let Some(folder) = LOG_FOLDER.get() {
        open_folder(folder);
    }
}

/// Opens a folder in the file manager.
pub fn open_folder(folder: &std::path::Path) {
    #[cfg(windows)]
    let result = std::process::Command::new("explorer").arg(folder).spawn().map(|_| ());
    #[cfg(not(windows))]
    let result = webbrowser::open(&folder.to_string_lossy());
    if let Err(e) = result {
        tracing::warn!("Couldn't open the folder {folder:?}: {e}");
    }
}

/// The file the Vulkan driver crashed in on this PC, if that still applies: the same driver
/// file (a driver update installs to a new folder, or at least replaces the file). A stale
/// record is deleted. Not retried for a new Mugetsu version: the crashes seen so far (Intel
/// UHD 620, also with the newest driver and in stock Ruffle) are driver bugs Mugetsu can't fix.
pub fn vulkan_crashed_before() -> Option<PathBuf> {
    crash_marker(VULKAN_CRASH_MARKER)
}

/// The crashed file a crash record names, if the record still applies; a stale record is
/// deleted.
fn crash_marker(marker: &str) -> Option<PathBuf> {
    let marker = config_dir()?.join(marker);
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

/// Called first thing in `main` (before any other thread exists): Mugetsu 2.0.0's speed-ups
/// (`platform::DEFAULT_ON`) are turned on, as Mugetsu's player does when AQW is played.
/// `no-lean-scripts`, `no-lean-frames`, `no-lean-render`, `no-page-blends`, `no-shape-uploads`,
/// `no-pick-bounds`, `no-sharp-snapshots`, `no-early-scripts`, `no-shared-shapes`,
/// `no-pass-merge` and `slow-multiply` files next to the exe turn single ones off (`no-atlas`,
/// `no-glow-batch` and `no-cache-steps` are checked while running). (Turu: Mugetsu's
/// `safe-drawing` file, all of them off at once, is gone with Turu's Safe Drawing.)
pub fn apply_drawing_switches() {
    let set = |var: &str| {
        // SAFETY: called first thing in `main`, before any other thread exists.
        unsafe { std::env::set_var(var, "1") };
    };
    crate::platform::drawing_switches(exe_flag)
        .into_iter()
        .for_each(set);
    for (file, var) in BY_FILE {
        if exe_flag(file) && std::env::var_os(var).is_none() {
            set(var);
        }
    }
}

/// Switches that are only on with their file next to the exe (Mugetsu 2.0.0's).
const BY_FILE: [(&str, &str); 10] = [
    // A room-entry picture is not held for its new cached objects' shapes (see core's
    // `deferred_shapes::entry_hold`): they are drawn blank until done, as from 2026-10-28.
    ("no-entry-hold", "RUFFLE_NO_ENTRY_HOLD"),
    // Finished shapes are uploaded within a few milliseconds per frame, the rest in the
    // next frames (see core's `deferred_shapes`): no hitch when a room's characters load.
    ("shape-budget", "RUFFLE_SHAPE_BUDGET"),
    ("no-shared-shapes", "RUFFLE_NO_SHARED_SHAPES"),
    ("no-pass-merge", "RUFFLE_NO_PASS_MERGE"),
    ("slow-multiply", "RUFFLE_SLOW_MULTIPLY"),
    // Flash-style edge smoothing (from the days it was a prototype).
    ("flash-aa", "RUFFLE_FLASH_AA"),
    // The lean walks' check: everything is still walked, and what the lean walks would have
    // skipped wrongly is reported in the log ("LEAN VERIFY", and the "lean walks" line).
    ("lean-verify", "RUFFLE_LEAN_VERIFY"),
    // Lean render's check: the unchanged branches are walked as well and compared.
    ("lean-render-verify", "RUFFLE_LEAN_RENDER_VERIFY"),
    // (Turu: no `render-thread`: see `player.rs`.)
    // The pick's check: every skipped branch is picked in full as well, and a difference is
    // reported in the log ("PICK MISMATCH", "PICK NOTE OUT OF DATE"); and its cost and counts.
    ("pick-verify", "RUFFLE_PICK_VERIFY"),
    ("pick-stats", "RUFFLE_PICK_STATS"),
];

/// Whether shapes not drawn for a while have their GPU geometry freed (see
/// `app::memory_watch`). A `no-shape-eviction` file next to the exe turns it off.
/// How much shape geometry (MB) is kept before unused shapes are freed sooner than after
/// 10 minutes (see `app::memory_watch`): 256 MB, 128 MB on built-in graphics, whose memory
/// is the system's RAM. `RUFFLE_SHAPE_STORE_MB` sets it (0: always 10 minutes, as it was).
pub fn shape_store_mb() -> u64 {
    static OVERRIDE: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    if let Some(mb) = OVERRIDE.get_or_init(|| {
        std::env::var("RUFFLE_SHAPE_STORE_MB")
            .ok()
            .and_then(|mb| mb.trim().parse().ok())
    }) {
        return *mb;
    }
    // (Known once the graphics chip is: this is asked a minute into the session at the
    // earliest.)
    if BUILT_IN_GRAPHICS.get().is_some() {
        128
    } else {
        256
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
    let single = [
        "RUFFLE_NO_SHARED_SHAPES",
        "RUFFLE_NO_ATLAS",
        "RUFFLE_NO_CACHE_BATCH",
        "RUFFLE_NO_PASS_MERGE",
        "RUFFLE_SLOW_MULTIPLY",
    ]
    .into_iter()
    .filter(|var| std::env::var_os(var).is_some());
    let new = crate::platform::DEFAULT_ON
        .into_iter()
        .filter(|(var, _)| std::env::var_os(var).is_none())
        .map(|(_, name)| name);
    single.chain(new).collect()
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

#[cfg(windows)]
thread_local! {
    /// The chip `video_memory` asks (every few seconds), found once. Let go of at shutdown
    /// (`release_video_memory_adapter`): this thread's storage is freed last of all, when the
    /// graphics driver is already gone.
    static VIDEO_MEMORY_ADAPTER: std::cell::RefCell<
        Option<windows::Win32::Graphics::Dxgi::IDXGIAdapter3>,
    > = const { std::cell::RefCell::new(None) };
}

/// Mugetsu 2.0.0: lets go of the DXGI adapter `video_memory` holds, while closing, on the
/// thread that asked (the window's event loop, a client's engine thread).
pub fn release_video_memory_adapter() {
    #[cfg(windows)]
    VIDEO_MEMORY_ADAPTER.with(|cell| drop(cell.borrow_mut().take()));
}

/// How far a normal exit has got (see `exit_phase`): the crash record says where a crash
/// during shutdown happened.
static EXIT_PHASE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
#[cfg(windows)]
const EXIT_PHASES: [&str; 7] = [
    "running",
    "closing: dropping the player and renderer",
    "closing: player and renderer dropped",
    "closing: done",
    "closing: event loop ended",
    "closing: window and app dropped",
    "closing: log closed",
];

/// Mugetsu 2.0.0: the closing step reached (an index into `EXIT_PHASES`).
pub fn set_exit_phase(phase: u8) {
    EXIT_PHASE.store(phase, Ordering::Relaxed);
}

/// What the program was doing, for the crash record: "running", or the last closing step
/// reached.
#[cfg(windows)]
pub fn exit_phase() -> &'static str {
    EXIT_PHASES[(EXIT_PHASE.load(Ordering::Relaxed) as usize).min(EXIT_PHASES.len() - 1)]
}

/// Whether a normal exit is under way.
#[cfg(windows)]
pub fn exiting() -> bool {
    EXIT_PHASE.load(Ordering::Relaxed) != 0
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
    // (Not found again while closing: `release_video_memory_adapter` has let go of it.)
    if exiting() {
        return None;
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
    VIDEO_MEMORY_ADAPTER.with(|cell| {
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

#[cfg(test)]
mod tests {
    use super::{AutomaticGraphics, FpsLimit, Performance, automatic_graphics, open_gpu};
    use std::path::PathBuf;

    #[test]
    fn directx_12_that_does_not_start_is_replaced_by_vulkan() {
        // DirectX 12 wanted and failing: Vulkan is tried next.
        let mut tried = Vec::new();
        let opened = open_gpu(wgpu::Backends::DX12, |backends| {
            tried.push(backends);
            if backends == wgpu::Backends::DX12 {
                anyhow::bail!("no DirectX 12 device")
            }
            Ok(backends)
        });
        assert_eq!(opened.ok(), Some(wgpu::Backends::VULKAN));
        assert_eq!(tried, [wgpu::Backends::DX12, wgpu::Backends::VULKAN]);
        // Anything else wanted: its error comes back, with nothing else tried.
        for wanted in [wgpu::Backends::VULKAN, wgpu::Backends::DX12 | wgpu::Backends::VULKAN] {
            let mut calls = 0;
            let opened = open_gpu(wanted, |_| -> anyhow::Result<()> {
                calls += 1;
                anyhow::bail!("no device")
            });
            assert!(opened.is_err());
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn automatic_graphics_takes_the_api_that_did_not_crash() {
        let file = || Some(PathBuf::from(r"C:\Windows\System32\igvk64.dll"));
        let intel = || Some("Intel(R) UHD Graphics 620".to_owned());
        // A Vulkan crash: DirectX 12, also when DirectX 12 crashed too (nothing to switch to).
        let chosen = automatic_graphics(file(), file, intel);
        assert_eq!(chosen.arg(), "dx12");
        assert!(matches!(chosen, AutomaticGraphics::VulkanCrashed(_)));
        // A DirectX 12 crash: Vulkan, before the Intel rule.
        let chosen = automatic_graphics(None, file, intel);
        assert_eq!(chosen.arg(), "vulkan");
        assert!(matches!(chosen, AutomaticGraphics::Dx12Crashed(_)));
        // Intel: DirectX 12, said so in the log.
        let chosen = automatic_graphics(None, || None, intel);
        assert_eq!(chosen, AutomaticGraphics::Intel("Intel(R) UHD Graphics 620".to_owned()));
        assert_eq!(chosen.arg(), "dx12");
        // Otherwise the system's default: DirectX 12 on Windows, Vulkan on Linux.
        let chosen = automatic_graphics(None, || None, || None);
        assert_eq!(chosen, AutomaticGraphics::Default);
        assert_eq!(chosen.arg(), if cfg!(windows) { "dx12" } else { "vulkan" });
    }

    #[test]
    fn performance_mode_is_read_back_as_it_was_saved() {
        assert_eq!(Performance::from_saved("fast\n"), Performance::Fast);
        assert_eq!(Performance::from_saved("balanced"), Performance::Balanced);
        // Anything else is off: an empty file, a level a later version might add.
        assert_eq!(Performance::from_saved(""), Performance::Off);
        assert_eq!(Performance::from_saved("fastest"), Performance::Off);
        for level in [Performance::Off, Performance::Balanced, Performance::Fast] {
            assert_eq!(Performance::from_index(level as u8), level);
        }
        // Off draws every pixel; each level draws fewer than the one before.
        assert_eq!(Performance::Off.scale(), 1.0);
        assert!(Performance::Balanced.scale() < 1.0);
        assert!(Performance::Fast.scale() < Performance::Balanced.scale());
    }

    #[test]
    fn an_fps_limit_is_read_back_as_it_was_saved() {
        assert_eq!(FpsLimit::parse("screen\n"), FpsLimit::Screen);
        assert_eq!(FpsLimit::parse("144"), FpsLimit::Fixed(144));
        // Out of range or not a number: no limit.
        assert_eq!(FpsLimit::parse("5"), FpsLimit::Off);
        assert_eq!(FpsLimit::parse("fast"), FpsLimit::Off);
        for limit in [FpsLimit::Off, FpsLimit::Screen, FpsLimit::Fixed(240)] {
            let bits = limit.bits();
            let back = match bits {
                0 => FpsLimit::Off,
                1 => FpsLimit::Screen,
                fps => FpsLimit::Fixed(fps),
            };
            assert_eq!(back, limit);
        }
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
