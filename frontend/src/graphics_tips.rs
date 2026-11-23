//! Mugetsu 2.0.0's one-time tips for built-in graphics, in a tab (a desktop client, an Android
//! slot): Turu's UI shows them (`gui::turu::graphics_tips`).
//!
//! Built-in graphics run AQW several times faster on Quality Low (no edge smoothing): an Intel
//! UHD 620 went from ~8 to a locked 30 FPS in a crowded room. Many players never open AQW's
//! Options. So when the game runs on built-in graphics at Medium or High and its pictures stay
//! well below its frame rate for 15 seconds, the player is told once how to switch. Turu never
//! changes the setting itself: AQW keeps its own copy and puts it back on login and in its
//! Options. Where the game is at Quality Low already and still that slow, Performance mode (the
//! game drawn with fewer pixels and stretched) is offered once, with a button that turns it on:
//! Fast below 60 % of the frame rate, else Balanced. A phone's graphics are always built in.
//!
//! The frontend counts the game's pictures over 5-second windows and calls [`window`] with each;
//! a window in which the tab wasn't drawn all along (hidden, covered, a Grid View tile not
//! active, Headless Mode, no surface) is passed as `None`, which starts the count again. Each tip
//! is shown once per data folder (a marker there, made by the one tab that shows it); the test
//! switches (`RUFFLE_TEST_LOW_PROMPT`, `RUFFLE_TEST_PERFORMANCE_OFFER`) show theirs about 30 s
//! after the start on any graphics, every time.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long a window of pictures lasts.
pub const WINDOW: Duration = Duration::from_secs(5);
/// Slow windows in a row before a tip.
const SLOW_WINDOWS: u32 = 3;
/// Windows the test switches wait (time to log in first).
const TEST_WINDOWS: u32 = 6;

const LOW_TIP_MARKER: &str = "graphics-low-quality-tip-shown";
const PERFORMANCE_OFFER_MARKER: &str = "graphics-performance-offered";

/// AQW's Quality, as far as the tips care (the player's `StageQuality`, which this crate doesn't
/// see without its `ruffle` feature).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quality {
    Low,
    Medium,
    /// High or above.
    High,
}

/// A tip for Turu's UI to show.
#[derive(Debug, Clone, PartialEq)]
pub enum Tip {
    /// Slow on the built-in graphics `chip` at AQW's Quality `quality` ("High" or "Medium").
    QualityLow { chip: String, quality: &'static str },
    /// Still slow on the built-in graphics `chip` at Quality Low: Performance mode at `level`
    /// (`Host::set_performance`'s 1 balanced, 2 fast) is offered.
    PerformanceMode { chip: String, level: u8 },
}

/// Which tip a run of windows calls for ([`Watch::window`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Due {
    QualityLow,
    /// Performance mode at this level.
    PerformanceMode(u8),
}

/// One window of pictures, as [`Watch::window`] takes it.
#[derive(Debug, Clone, Copy)]
pub struct Measured {
    /// The game's pictures drawn a second.
    pub drawn_fps: f64,
    /// The game's frame rate.
    pub frame_rate: f64,
    /// AQW's Quality is Low.
    pub quality_low: bool,
    /// Performance mode can be turned on here and is off.
    pub performance_offerable: bool,
}

/// The tips' state over the windows: what has been said, and the slow windows in a row.
#[derive(Debug, Default)]
pub struct Watch {
    pub low_done: bool,
    pub offer_done: bool,
    low_slow: u32,
    offer_slow: u32,
    seen: u32,
}

impl Watch {
    /// A window (`None`: the tab wasn't drawn all along). `test_low`, `test_offer`: the test
    /// switches, which make their tip due after [`TEST_WINDOWS`] windows on any graphics.
    pub fn window(&mut self, measured: Option<Measured>, test_low: bool, test_offer: bool) -> Vec<Due> {
        let mut due = Vec::new();
        let Some(measured) = measured else {
            self.low_slow = 0;
            self.offer_slow = 0;
            return due;
        };
        self.seen += 1;
        // Below 1 a second: not drawn, not slow.
        let slow = measured.drawn_fps >= 1.0 && measured.drawn_fps < 0.8 * measured.frame_rate;
        if !self.low_done {
            if test_low {
                if self.seen >= TEST_WINDOWS {
                    self.low_done = true;
                    due.push(Due::QualityLow);
                }
            } else if slow && !measured.quality_low {
                self.low_slow += 1;
                if self.low_slow >= SLOW_WINDOWS {
                    self.low_done = true;
                    due.push(Due::QualityLow);
                }
            } else {
                self.low_slow = 0;
            }
        }
        if !self.offer_done && measured.performance_offerable {
            let level = if measured.drawn_fps < 0.6 * measured.frame_rate {
                2
            } else {
                1
            };
            if test_offer {
                if self.seen >= TEST_WINDOWS {
                    self.offer_done = true;
                    due.push(Due::PerformanceMode(level));
                }
            } else if slow && measured.quality_low {
                self.offer_slow += 1;
                if self.offer_slow >= SLOW_WINDOWS {
                    self.offer_done = true;
                    due.push(Due::PerformanceMode(level));
                }
            } else {
                self.offer_slow = 0;
            }
        }
        due
    }
}

struct State {
    /// The built-in graphics chip the game is drawn on, if it is one.
    chip: Option<String>,
    watch: Watch,
    /// Tips for the UI to show, oldest first.
    pending: Vec<Tip>,
}

static STATE: Mutex<State> = Mutex::new(State {
    chip: None,
    watch: Watch {
        low_done: false,
        offer_done: false,
        low_slow: 0,
        offer_slow: 0,
        seen: 0,
    },
    pending: Vec::new(),
});

/// The game is drawn on built-in graphics, the chip `name` (the adapter's): the tips can apply.
pub fn set_built_in_chip(name: &str) {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).chip = Some(chip_name(name));
}

/// The chip in an adapter's `name`. On OpenGL ES that's the driver's renderer string, and a phone
/// whose OpenGL ES is Google's ANGLE (Samsung's Xclipse, an S24 FE's say) names the API ANGLE
/// runs on: `ANGLE (Samsung Xclipse 940) on Vulkan 1.3.279`, which made a tip say the game was
/// on Vulkan while it was drawn with OpenGL ES. ANGLE's other forms put the chip second:
/// `ANGLE (Vendor, Renderer, Driver)`, and SwiftShader's renderer is `Vulkan 1.3.0 (<device>)`.
fn chip_name(name: &str) -> String {
    let name = name.trim();
    let Some(rest) = name.strip_prefix("ANGLE (") else {
        return name.to_owned();
    };
    // The parenthesis that closes "ANGLE (", and the top-level commas inside.
    let (mut depth, mut close, mut commas) = (1, None, Vec::new());
    for (i, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            ',' if depth == 1 => commas.push(i),
            _ => {}
        }
    }
    let Some(close) = close else {
        return name.to_owned();
    };
    let inside = &rest[..close];
    let chip = match commas.as_slice() {
        [first, second, ..] => &inside[first + 1..*second],
        _ => inside,
    }
    .trim();
    // SwiftShader (and other renderers on Vulkan) inside ANGLE: `Vulkan 1.3.0 (<device>)`.
    let chip = match chip.strip_prefix("Vulkan ") {
        Some(rest) => match (rest.find('('), rest.rfind(')')) {
            (Some(open), Some(close)) if open < close => rest[open + 1..close].trim(),
            _ => chip,
        },
        None => chip,
    };
    if chip.is_empty() {
        name.to_owned()
    } else {
        chip.to_owned()
    }
}

/// A window of the game's pictures has ended: `drawn_fps` of them a second, or `None` if the tab
/// wasn't drawn all along; the game's `frame_rate` and `quality` now. Makes a tip pending when one
/// is due (see the module's notes).
pub fn window(drawn_fps: Option<f64>, frame_rate: f64, quality: Quality) {
    let test_low = std::env::var_os("RUFFLE_TEST_LOW_PROMPT").is_some();
    let test_offer = std::env::var_os("RUFFLE_TEST_PERFORMANCE_OFFER").is_some();
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let chip = match (&state.chip, test_low || test_offer) {
        (Some(chip), _) => chip.clone(),
        (None, true) => "test".to_owned(),
        (None, false) => return,
    };
    if state.watch.low_done && state.watch.offer_done {
        return;
    }
    let measured = drawn_fps.map(|drawn_fps| Measured {
        drawn_fps,
        frame_rate,
        quality_low: quality == Quality::Low,
        performance_offerable: crate::host::host().performance() == Some(0),
    });
    let marker_dir = turu_bot::paths::Paths::new().root;
    for due in state.watch.window(measured, test_low, test_offer) {
        let (tip, marker, test) = match due {
            Due::QualityLow => {
                let quality = if quality == Quality::Medium { "Medium" } else { "High" };
                tracing::info!(
                    "Suggesting Quality Low: {:.1} FPS drawn on {chip} at {quality}",
                    drawn_fps.unwrap_or(0.0)
                );
                (
                    Tip::QualityLow {
                        chip: chip.clone(),
                        quality,
                    },
                    LOW_TIP_MARKER,
                    test_low,
                )
            }
            Due::PerformanceMode(level) => {
                tracing::info!(
                    "Offering Performance mode ({}): {:.1} FPS drawn on {chip} at Quality {quality:?}",
                    if level == 2 { "fast" } else { "balanced" },
                    drawn_fps.unwrap_or(0.0)
                );
                (
                    Tip::PerformanceMode {
                        chip: chip.clone(),
                        level,
                    },
                    PERFORMANCE_OFFER_MARKER,
                    test_offer,
                )
            }
        };
        // (The test switches show it every time: they don't use the marker.)
        if test || first_to_mark(&marker_dir, marker, &chip) {
            state.pending.push(tip);
        }
    }
}

/// Makes the marker `name` in `dir`, holding `chip`, if no tab has yet: whether this call made it.
/// Several tabs can come to the same tip at once (Grid View draws them all); one shows it.
fn first_to_mark(dir: &Path, name: &str, chip: &str) -> bool {
    use std::io::Write as _;
    let _ = std::fs::create_dir_all(dir);
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(name))
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{chip}");
            true
        }
        Err(_) => false,
    }
}

/// The oldest tip waiting for the UI, without taking it.
pub fn pending() -> Option<Tip> {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).pending.first().cloned()
}

/// The oldest tip was answered.
pub fn answered() {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !state.pending.is_empty() {
        state.pending.remove(0);
    }
}

/// Counts the game's pictures over [`WINDOW`]s for [`window`]: the frontend calls [`Self::drawn`]
/// for each picture drawn for a game frame (not an in-between one), [`Self::not_shown`] whenever
/// the tab isn't drawn, and [`Self::take`] after each step.
#[derive(Debug)]
pub struct PictureCount {
    since: Instant,
    pictures: u32,
    spoiled: bool,
}

impl Default for PictureCount {
    fn default() -> Self {
        Self {
            since: Instant::now(),
            pictures: 0,
            spoiled: false,
        }
    }
}

impl PictureCount {
    pub fn drawn(&mut self) {
        self.pictures += 1;
    }

    /// The tab wasn't drawn (for any part of the window): the window doesn't count.
    pub fn not_shown(&mut self) {
        self.spoiled = true;
    }

    /// A window that has ended: `Some(Some(fps))`, `Some(None)` if it doesn't count, or `None`
    /// while it lasts.
    pub fn take(&mut self) -> Option<Option<f64>> {
        let elapsed = self.since.elapsed();
        if elapsed < WINDOW {
            return None;
        }
        let fps = (!self.spoiled).then(|| f64::from(self.pictures) / elapsed.as_secs_f64());
        *self = Self::default();
        Some(fps)
    }
}

#[cfg(test)]
mod tests {
    use super::{Due, Measured, Watch, chip_name};

    #[test]
    fn chip_names_leave_angle_out() {
        // Samsung's Xclipse (an S24 FE): OpenGL ES is ANGLE on Vulkan.
        assert_eq!(
            chip_name("ANGLE (Samsung Xclipse 940) on Vulkan 1.3.279"),
            "Samsung Xclipse 940"
        );
        // Vendor, renderer, driver.
        assert_eq!(
            chip_name("ANGLE (Intel, Intel(R) UHD Graphics 620 Direct3D11 vs_5_0 ps_5_0, D3D11)"),
            "Intel(R) UHD Graphics 620 Direct3D11 vs_5_0 ps_5_0"
        );
        // The emulator's SwiftShader.
        assert_eq!(
            chip_name(
                "ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (LLVM 10.0.0) (0x0000C0DE)), SwiftShader driver-5.0.0)"
            ),
            "SwiftShader Device (LLVM 10.0.0) (0x0000C0DE)"
        );
        // Not ANGLE: as it is.
        assert_eq!(chip_name("Mali-G52"), "Mali-G52");
        assert_eq!(chip_name("Adreno (TM) 740"), "Adreno (TM) 740");
        assert_eq!(
            chip_name("Android Emulator OpenGL ES Translator (Apple M5)"),
            "Android Emulator OpenGL ES Translator (Apple M5)"
        );
        // Malformed: as it is.
        assert_eq!(chip_name("ANGLE (broken"), "ANGLE (broken");
    }

    fn measured(drawn_fps: f64, quality_low: bool) -> Option<Measured> {
        Some(Measured {
            drawn_fps,
            frame_rate: 24.0,
            quality_low,
            performance_offerable: true,
        })
    }

    #[test]
    fn quality_low_is_suggested_after_three_slow_windows_in_a_row() {
        let mut watch = Watch::default();
        assert!(watch.window(measured(10.0, false), false, false).is_empty());
        assert!(watch.window(measured(10.0, false), false, false).is_empty());
        // A good window starts the count again, and so does one the tab wasn't drawn all along.
        assert!(watch.window(measured(24.0, false), false, false).is_empty());
        assert!(watch.window(measured(10.0, false), false, false).is_empty());
        assert!(watch.window(None, false, false).is_empty());
        assert!(watch.window(measured(10.0, false), false, false).is_empty());
        assert!(watch.window(measured(10.0, false), false, false).is_empty());
        assert_eq!(watch.window(measured(10.0, false), false, false), [Due::QualityLow]);
        // Once.
        for _ in 0..5 {
            assert!(watch.window(measured(10.0, false), false, false).is_empty());
        }
    }

    #[test]
    fn only_the_first_tab_to_mark_a_tip_shows_it() {
        let dir = std::env::temp_dir().join(format!("turu-graphics-tips-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(super::first_to_mark(&dir, "graphics-test-tip", "Test GPU"));
        assert!(!super::first_to_mark(&dir, "graphics-test-tip", "Test GPU"));
        assert_eq!(
            std::fs::read_to_string(dir.join("graphics-test-tip")).ok().as_deref(),
            Some("Test GPU\n")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hidden_tab_is_not_slow() {
        let mut watch = Watch::default();
        for _ in 0..5 {
            assert!(watch.window(measured(0.5, false), false, false).is_empty());
        }
    }

    #[test]
    fn performance_mode_is_offered_only_at_quality_low() {
        let mut watch = Watch::default();
        // Slow at High: the Quality tip, not the offer.
        for _ in 0..3 {
            watch.window(measured(18.0, false), false, false);
        }
        assert!(watch.low_done && !watch.offer_done);
        // Slow at Low: balanced above 60 % of the frame rate, fast below.
        for _ in 0..2 {
            assert!(watch.window(measured(18.0, true), false, false).is_empty());
        }
        assert_eq!(
            watch.window(measured(18.0, true), false, false),
            [Due::PerformanceMode(1)]
        );
        let mut watch = Watch::default();
        for _ in 0..2 {
            watch.window(measured(12.0, true), false, false);
        }
        assert_eq!(
            watch.window(measured(12.0, true), false, false),
            [Due::PerformanceMode(2)]
        );
    }

    #[test]
    fn performance_mode_is_not_offered_where_it_is_on_or_missing() {
        let mut watch = Watch::default();
        for _ in 0..5 {
            let window = Some(Measured {
                drawn_fps: 10.0,
                frame_rate: 24.0,
                quality_low: true,
                performance_offerable: false,
            });
            assert!(watch.window(window, false, false).is_empty());
        }
    }

    #[test]
    fn the_test_switches_wait_for_six_windows_on_any_graphics() {
        let mut watch = Watch::default();
        for _ in 0..5 {
            assert!(watch.window(measured(24.0, false), true, true).is_empty());
        }
        assert_eq!(
            watch.window(measured(24.0, false), true, true),
            [Due::QualityLow, Due::PerformanceMode(1)]
        );
    }
}
