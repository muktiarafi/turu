//! The bot windows (`turu`, VibeSkua's layout) and what the app's window shares with them.

pub mod turu;

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

/// The height above the movie: the nav bar, and the tab strip when it shows.
pub fn top_height() -> u32 {
    if crate::turu::tabs().len() > 1 {
        nav_bar_height() + tab_strip_height()
    } else {
        nav_bar_height()
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
