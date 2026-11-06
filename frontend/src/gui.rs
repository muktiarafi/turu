//! The bot windows (`turu`, VibeSkua's layout) and what the app's window shares with them.

pub mod turu;

use std::sync::atomic::{AtomicU32, Ordering};

/// VibeSkua's top nav bar: the window shows the movie this far down.
pub const NAV_BAR_HEIGHT: u32 = 34;
/// The nav bar on a touch screen (`Platform::is_touch`): its buttons are 40 points high.
pub const TOUCH_NAV_BAR_HEIGHT: u32 = 48;

/// The nav bar's height here: [`NAV_BAR_HEIGHT`], or [`TOUCH_NAV_BAR_HEIGHT`] on a touch screen.
pub fn nav_bar_height() -> u32 {
    if crate::platform::platform().is_touch() {
        TOUCH_NAV_BAR_HEIGHT
    } else {
        NAV_BAR_HEIGHT
    }
}
/// The tab strip above the nav bar, shown once there are two tabs.
pub const TAB_STRIP_HEIGHT: u32 = 30;
/// The tab strip on a touch screen.
pub const TOUCH_TAB_STRIP_HEIGHT: u32 = 40;

/// The tab strip's height here: [`TAB_STRIP_HEIGHT`], or [`TOUCH_TAB_STRIP_HEIGHT`] on a touch
/// screen.
pub fn tab_strip_height() -> u32 {
    if crate::platform::platform().is_touch() {
        TOUCH_TAB_STRIP_HEIGHT
    } else {
        TAB_STRIP_HEIGHT
    }
}

/// Whether the nav bar is a rail down the left side instead of a bar across the top: on a
/// touch screen (Android, held sideways). The game is as tall as the screen allows, so a bar
/// above it would take its height; the rail sits in the room left and right of it instead.
pub fn nav_rail() -> bool {
    crate::platform::platform().is_touch()
}

/// The rail's width, without the display cutout's inset.
pub const RAIL_WIDTH: u32 = 64;

/// How far a display cutout reaches into the rail's side of the screen, in points (as an `f32`'s
/// bits); the rail grows by it so its content stays clear of the camera.
static RAIL_INSET: AtomicU32 = AtomicU32::new(0);

/// The display cutout on the rail's (left) side, in points: the frontend says, each frame.
pub fn set_rail_inset(points: f32) {
    RAIL_INSET.store(points.max(0.0).to_bits(), Ordering::Relaxed);
}

/// The display cutout on the rail's side, in points.
pub fn rail_inset() -> f32 {
    f32::from_bits(RAIL_INSET.load(Ordering::Relaxed))
}

/// The width left of the movie: the rail with its inset, or nothing with a top bar.
pub fn left_width() -> u32 {
    if nav_rail() {
        RAIL_WIDTH + rail_inset().ceil() as u32
    } else {
        0
    }
}

/// The height above the movie: the nav bar (unless it is a rail), and the tab strip when it
/// shows.
pub fn top_height() -> u32 {
    let bar = if nav_rail() { 0 } else { nav_bar_height() };
    if crate::turu::tabs().len() > 1 {
        bar + tab_strip_height()
    } else {
        bar
    }
}

/// Mugetsu's right-click panel colours (the app's `context_menu.rs`), which the bot windows'
/// theme builds on.
pub mod mugetsu {
    use egui::Color32;

    pub const EDGE: Color32 = Color32::from_rgba_premultiplied(58, 62, 78, 255);
    pub const ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(40, 44, 58, 255);
    pub const TRACK: Color32 = Color32::from_rgba_premultiplied(48, 52, 66, 255);
    pub const ACCENT: Color32 = Color32::from_rgb(124, 108, 240);
    pub const TEXT: Color32 = Color32::from_rgb(232, 234, 242);
    pub const MUTED: Color32 = Color32::from_rgb(140, 146, 166);
}
