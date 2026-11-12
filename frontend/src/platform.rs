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

    /// Whether Safe Drawing ([`SAFE_DRAWING`]) is saved on for the next start, or None where
    /// Options > Application doesn't offer it (the desktop: files next to the exe).
    fn safe_drawing(&self) -> Option<bool> {
        None
    }

    /// Saves Safe Drawing for the next start (the switches are read once, as they're first used).
    fn set_safe_drawing(&self, _on: bool) {}
}

/// Mugetsu's drawing speed-ups' switches (environment variables read when they're first used),
/// all set by Safe Drawing: Ruffle's plain ways of drawing, for graphics drivers that misdraw
/// with the speed-ups (a Snapdragon laptop lost parts of maps; a PowerVR phone drew streaks).
pub const SAFE_DRAWING: [&str; 5] = [
    "RUFFLE_NO_SHARED_SHAPES",
    "RUFFLE_NO_ATLAS",
    "RUFFLE_NO_CACHE_BATCH",
    "RUFFLE_NO_PASS_MERGE",
    "RUFFLE_SLOW_MULTIPLY",
];

/// The beep's pitch and length: .NET's `Console.Beep()` (800 Hz, 200 ms).
pub const BEEP_HZ: f32 = 800.0;
pub const BEEP_MS: u64 = 200;

/// The beep's sample `frame` at `sample_rate`, in -1..=1: a sine at [`BEEP_HZ`] for [`BEEP_MS`],
/// faded in and out over 5 ms so it doesn't click, then silence.
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
