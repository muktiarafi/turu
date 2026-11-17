//! Turu: what the bot windows ask of the operating system (file dialogs, the browser, the file
//! manager), so the same windows run where those work differently (Android's system picker
//! answers through an `Intent`). The app installs its [`Platform`] once, with [`set`]; until then
//! (and without a window) nothing opens.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

/// What a file dialog asks: rfd's `AsyncFileDialog`, without the parent window.
#[derive(Debug, Clone, Default)]
pub struct FileDialog {
    pub title: Option<String>,
    pub directory: Option<PathBuf>,
    pub file_name: Option<String>,
    /// Name and extensions (without the dot; `*` for any).
    pub filters: Vec<(String, Vec<String>)>,
}

impl FileDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.filters.push((
            name.into(),
            extensions.iter().map(|e| (*e).to_owned()).collect(),
        ));
        self
    }

    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn set_directory(mut self, directory: impl AsRef<Path>) -> Self {
        self.directory = Some(directory.as_ref().to_owned());
        self
    }

    pub fn set_file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }
}

/// Gets a file dialog's answer: the chosen path, `None` when cancelled. Called on any thread.
pub type Answer = Box<dyn FnOnce(Option<PathBuf>) + Send>;

pub trait Platform: Send + Sync {
    /// Shows an "open file" dialog; false when it can't (one is open already). Called on the UI
    /// thread, and never waits for the answer.
    fn pick_file(&self, dialog: FileDialog, answer: Answer) -> bool;

    /// Shows a "save file" dialog, like [`Platform::pick_file`].
    fn save_file(&self, dialog: FileDialog, answer: Answer) -> bool;

    /// Opens a web page in the browser.
    fn open_url(&self, url: &str);

    /// Shows a folder in the file manager.
    fn open_folder(&self, folder: &Path);

    /// Opens a file with the program the system associates with it.
    fn open_file(&self, path: &Path);

    /// Whether [`Platform::pick_file`] and [`Platform::save_file`] show dialogs; the windows
    /// hide what needs one when they don't.
    fn has_file_dialogs(&self) -> bool {
        true
    }

    /// Whether [`Platform::open_file`] opens files in an app (Edit Script, Open VSCode).
    fn opens_files(&self) -> bool {
        true
    }

    /// Whether Army can start accounts in `turu` processes of their own (Army > Accounts' "In
    /// its own window" and "Without a window"); else only in tabs.
    fn launches_processes(&self) -> bool {
        true
    }

    /// Whether several clients work together: the Army menu (Accounts, Army Control, New Tab)
    /// and Plugins > Butler. Where it's false they're hidden.
    fn has_army(&self) -> bool {
        true
    }

    /// Whether the tabs are limited by Options > Application > Tab Limit (`AppSettings::tab_limit`):
    /// on a phone, whose memory holds two or three game clients.
    fn limits_tabs(&self) -> bool {
        false
    }

    /// Whether Grid View (every tab at once) is offered: the nav bar's Grid. Where it's false
    /// the button is hidden and the frontend ignores `TabRequest::Grid`.
    fn has_grid_view(&self) -> bool {
        true
    }

    /// Whether the screen is touched rather than pointed at: no hover, so tooltips never show,
    /// and controls need finger-sized targets (48 points, Android's minimum).
    fn is_touch(&self) -> bool {
        false
    }

    /// Whether keyboard shortcuts work: Options > HotKeys and its bindings (VibeSkua's F10, F9,
    /// ...). Android has them too, for a hardware keyboard (`android/src/input.rs` gives egui
    /// every key with its modifiers, as winit does on a desktop).
    fn has_hotkeys(&self) -> bool {
        true
    }

    /// Whether [`Platform::share_text`] opens the system's share sheet (Android): the log and
    /// crash report windows show Share then.
    fn shares_text(&self) -> bool {
        false
    }

    /// Hands a text to another app (mail, a chat, Drive) through the system's share sheet.
    fn share_text(&self, _subject: &str, _text: &str) {}

    /// Whether crash reports can be sent to Turu's developers automatically (builds with a Sentry
    /// DSN, Android and desktop): Turu then asks once, and Options > Application has a switch.
    fn uploads_crash_reports(&self) -> bool {
        false
    }

    /// Turns the automatic crash reports on or off now (the choice is kept in `settings.json`,
    /// `send_crash_reports`, which the app reads at start).
    fn set_crash_uploads(&self, _on: bool) {}

    /// Asks "Send crash reports?" with the system's own dialog (Android), which answers through
    /// `gui::turu::answer_crash_upload_question`; false where there is none, and the UI asks.
    fn ask_crash_uploads(&self, _question: &Question) -> bool {
        false
    }

    /// Whether [`Platform::alert`] tells the user anything (Android's notifications): the tabs
    /// are watched for alerts only then (`turu::watch_alerts`).
    fn posts_alerts(&self) -> bool {
        false
    }

    /// Tells the user `title` and `text` the system's way, heard with the screen off (Android: a
    /// notification in the `kind`'s channel, while the app isn't on screen). Called on a thread
    /// of its own.
    fn alert(&self, _kind: Alert, _title: &str, _text: &str) {}

    /// Whether [`Platform::install_package`] installs a downloaded app package (Android's APK):
    /// the update window then downloads and installs an update instead of linking to it.
    fn installs_packages(&self) -> bool {
        false
    }

    /// Hands the package at `path` (downloaded and checked) to the system's installer, which asks
    /// the user to confirm; the outcome comes back through `turu::package_install_finished`.
    /// Err when it couldn't start. Called on a thread of its own.
    fn install_package(&self, _path: &Path) -> Result<(), String> {
        Err("this platform doesn't install packages".into())
    }

    /// Plays VibeSkua's `Console.Beep()` ([`beep_sample`]: 800 Hz for 200 ms) on the system's
    /// default sound output, apart from the game's sound (heard while the game is muted), and
    /// returns once it has played. Called on a thread of its own (Notify Drop); nothing plays
    /// without a sound device or a window.
    fn beep(&self) {}

    /// Turns Mugetsu's frame interpolation (`ruffle_core::frame_interpolation`) on or off from
    /// Options > Application, and saves the choice where the frontend reads it at start (the
    /// desktop: Mugetsu's `mugetsu-interpolation.txt`, also set by the game's right-click menu).
    fn set_frame_interpolation(&self, on: bool) {
        #[cfg(feature = "ruffle")]
        ruffle_core::frame_interpolation::set_enabled(on);
        // Ruffle's only: other clients draw the game themselves.
        #[cfg(not(feature = "ruffle"))]
        let _ = on;
    }

    /// The graphics APIs Options > Application offers, as (value, name), "" for Automatic, and
    /// the saved one's value; None where it doesn't offer them (the window has the game's
    /// right-click menu; a process tab's client has none: docs/TAB_PROCESSES_PROPOSAL.md, M5).
    fn graphics_choices(&self) -> Option<(Vec<(&'static str, &'static str)>, &'static str)> {
        None
    }

    /// Restarts Turu with the graphics API `value` names (from `graphics_choices`), after asking.
    fn set_graphics(&self, _value: &str) {}

    /// The rest of the game's right-click menu in the window (Mugetsu's), for Options > Application
    /// where there's no such menu (a process tab's client): whether the FPS counter is on, or
    /// None where Options doesn't offer it.
    fn show_fps(&self) -> Option<bool> {
        None
    }

    /// Turns the FPS counter on or off, and saves it.
    fn set_show_fps(&self, _on: bool) {}

    /// Mugetsu 2.0.0's Performance mode (0 off, 1 balanced, 2 fast: the game drawn at 85% or 70%
    /// of its area's width and height and stretched), or None where Options doesn't offer it.
    fn performance(&self) -> Option<u8> {
        None
    }

    /// Sets Performance mode, from the next picture on, and saves it.
    fn set_performance(&self, _level: u8) {}

    /// The share of the game's area it is drawn at now (see `performance`): 1.0 for all of it.
    fn render_scale(&self) -> f64 {
        1.0
    }

    /// Mugetsu 2.0.0's memory limit in MB (0: none), or None where Options doesn't offer it.
    fn memory_limit_mb(&self) -> Option<u32> {
        None
    }

    /// Sets the memory limit and saves it.
    fn set_memory_limit_mb(&self, _mb: u32) {}

    /// The process's memory as the limit counts it (Task Manager's figure), in MB.
    fn memory_now_mb(&self) -> Option<u64> {
        None
    }

    /// Frees now what the game isn't using (Mugetsu's "Free memory now").
    fn free_memory(&self) {}

    /// Mugetsu 2.0.0's frame pacing, with frame interpolation: whether it's uncapped (pictures go
    /// out without waiting for the display's refresh), and the FPS limit then (0: none, 1: just
    /// under the display's rate, else that many a second); None where Options doesn't offer it
    /// (as `show_fps`; the window has the game's right-click menu).
    fn frame_pacing(&self) -> Option<(bool, u32)> {
        None
    }

    /// Picks uncapped pacing or v-sync, and saves it.
    fn set_uncapped(&self, _on: bool) {}

    /// Sets the FPS limit (as `frame_pacing` gives it), and saves it.
    fn set_fps_limit(&self, _limit: u32) {}

    /// Whether the window is full screen, or None where Options doesn't offer it (as `show_fps`).
    fn full_screen(&self) -> Option<bool> {
        None
    }

    /// Makes the window full screen (Esc leaves it) or not.
    fn set_full_screen(&self, _on: bool) {}

    /// Whether Options offers Open Log Folder (as `show_fps`).
    fn opens_log_folder(&self) -> bool {
        false
    }

    /// Opens the folder of Turu's log in the file manager.
    fn open_log_folder(&self) {}
}

/// Mugetsu 2.0.0's speed-ups, which its player turns on when AQW is played: the switch, and
/// the name of a file that turns it off (`no-<name>`, next to the program on a computer).
///
/// Turu needs `lean-scripts` and `lean-frames` in particular: they replace Turu's own marks
/// for clips that stand still (dropped from ruffle-aqw for Mugetsu's).
pub const DEFAULT_ON: [(&str, &str); 8] = [
    // The per-frame walks skip the branches with nothing to do: frame construction and frame
    // scripts, and `enter_frame`.
    ("RUFFLE_LEAN_SCRIPTS", "lean-scripts"),
    ("RUFFLE_LEAN_FRAMES", "lean-frames"),
    // A recorded walk draws the branches that haven't changed from the last recording.
    ("RUFFLE_LEAN_RENDER", "lean-render"),
    // Overlay and hard light layers read straight from their atlas pages.
    ("RUFFLE_PAGE_BLENDS", "page-blends"),
    // New shapes are uploaded through an encoder of their own.
    ("RUFFLE_SHAPE_UPLOADS", "shape-uploads"),
    // The mouse pick skips plain branches the pointer is outside of.
    ("RUFFLE_PICK_BOUNDS", "pick-bounds"),
    // Small snapshots the game takes of its own art (darkened skill icons) are kept sharp.
    ("RUFFLE_SHARP_SNAPSHOTS", "sharp-snapshots"),
    // Clips put on the stage between two frames run their first frame scripts before the next
    // picture (colour-custom gear no longer shows in its drawn colours for a moment).
    ("RUFFLE_EARLY_SCRIPTS", "early-scripts"),
];

/// The switches of [`DEFAULT_ON`] to set before Ruffle reads them: less those whose `no-<name>`
/// file `off` says is there, and those already set from outside (`RUFFLE_LEAN_RENDER=every`,
/// for tests).
pub fn drawing_switches(off: impl Fn(&str) -> bool) -> Vec<&'static str> {
    DEFAULT_ON
        .iter()
        .filter(|(var, name)| !off(&format!("no-{name}")) && std::env::var_os(var).is_none())
        .map(|(var, _)| *var)
        .collect()
}

/// The beep's pitch and length: .NET's `Console.Beep()` (800 Hz, 200 ms).
pub const BEEP_HZ: f32 = 800.0;
pub const BEEP_MS: u64 = 200;

/// The beep's sample `frame` at `sample_rate`, in -1..=1: a sine at [`BEEP_HZ`] for [`BEEP_MS`],
/// faded in and out over 5 ms so it doesn't click, then silence.
/// What an alert ([`Platform::alert`]) is about; Android posts each kind in a channel of its own
/// (`Alerts.kt`, which numbers them the same).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alert {
    /// An item Notify Drop watches dropped.
    Drop = 0,
    /// A tab's script finished, failed or was stopped.
    ScriptStopped = 1,
}

/// A question for the system's own dialog ([`Platform::ask_crash_uploads`]): `message` and,
/// smaller, `note`; a link (`link`, opening `link_url`) that leaves it open; `no` and `yes`.
#[derive(Clone, Debug)]
pub struct Question {
    pub title: String,
    pub message: String,
    pub note: String,
    pub link: String,
    pub link_url: String,
    pub no: String,
    pub yes: String,
}

pub fn beep_sample(frame: u64, sample_rate: u32) -> f32 {
    let rate = sample_rate.max(1) as f32;
    let length = rate * BEEP_MS as f32 / 1000.0;
    let t = frame as f32;
    if t >= length {
        return 0.0;
    }
    let fade = rate * 0.005;
    let envelope = (t / fade).min((length - t) / fade).min(1.0);
    0.4 * envelope * (std::f32::consts::TAU * BEEP_HZ * t / rate).sin()
}

static PLATFORM: OnceLock<Arc<dyn Platform>> = OnceLock::new();

/// Installs the app's platform; false when one is already installed.
pub fn set(platform: Arc<dyn Platform>) -> bool {
    PLATFORM.set(platform).is_ok()
}

/// The installed platform, or one that opens nothing.
pub fn platform() -> &'static dyn Platform {
    PLATFORM.get().map_or(&NoPlatform, |p| p.as_ref())
}

/// Without a window: dialogs don't open, and what would open is logged.
struct NoPlatform;

impl Platform for NoPlatform {
    fn pick_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
    }

    fn save_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
    }

    fn open_url(&self, url: &str) {
        tracing::info!("Not opened without a window: {url}");
    }

    fn open_folder(&self, folder: &Path) {
        tracing::info!("Not opened without a window: {}", folder.display());
    }

    fn open_file(&self, path: &Path) {
        tracing::info!("Not opened without a window: {}", path.display());
    }
}
