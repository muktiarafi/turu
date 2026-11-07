//! The player's UI backend: Ruffle's null one, except that a text field taking focus shows the
//! soft keyboard and losing it hides the keyboard (while the activity is there to show it). Also
//! the keyboard's height and the display cutout's sides, which the activity reads from the
//! window's insets, and the display's refresh rate.

use jni::EnvUnowned;
use jni::objects::JClass;
use jni::sys::{jfloat, jint};
use ruffle_core::backend::ui::{
    DialogResultFuture, FileFilter, FontDefinition, FullscreenError, LanguageIdentifier, MouseCursor,
    MultiDialogResultFuture, NullUiBackend, UiBackend,
};
use ruffle_core::font::FontQuery;
use std::sync::atomic::{AtomicU32, Ordering};
use url::Url;

/// The soft keyboard's height in window pixels, 0 while it's down.
static KEYBOARD_HEIGHT: AtomicU32 = AtomicU32::new(0);

pub fn keyboard_height() -> u32 {
    KEYBOARD_HEIGHT.load(Ordering::Relaxed)
}

/// `TuruActivity.nativeKeyboardHeight(pixels)`: the window's insets changed.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeKeyboardHeight<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    pixels: jint,
) {
    KEYBOARD_HEIGHT.store(pixels.max(0) as u32, Ordering::Relaxed);
}

/// How far the display cutout reaches in from the window's left and right edges, in window
/// pixels (0 without one): a punch-hole camera sits there while the phone is held sideways.
static CUTOUT_LEFT: AtomicU32 = AtomicU32::new(0);
static CUTOUT_RIGHT: AtomicU32 = AtomicU32::new(0);

/// Shows or hides the soft keyboard: `TuruActivity.softKeyboard(show)`, which does it on the UI
/// thread (android-activity's `show_soft_input` calls the input method on this one, which Android
/// 15 refuses with a `CalledFromWrongThreadException`, so the keyboard never came up).
pub fn soft_keyboard(show: bool) {
    let Some(app) = crate::app() else {
        return;
    };
    let result = crate::platform::with_activity(&app, |env, activity| {
        env.call_method(
            activity,
            jni::jni_str!("softKeyboard"),
            jni::jni_sig!("(Z)V"),
            &[jni::objects::JValue::Bool(show)],
        )?;
        Ok(())
    });
    if let Err(e) = result {
        tracing::warn!("Couldn't {} the soft keyboard: {e}", if show { "show" } else { "hide" });
    }
}

/// The display cutout's (left, right) insets in window pixels.
pub fn cutout() -> (u32, u32) {
    (CUTOUT_LEFT.load(Ordering::Relaxed), CUTOUT_RIGHT.load(Ordering::Relaxed))
}

/// `TuruActivity.nativeCutout(left, right)`: the window's insets changed (Android 9+).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeCutout<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    left: jint,
    right: jint,
) {
    CUTOUT_LEFT.store(left.max(0) as u32, Ordering::Relaxed);
    CUTOUT_RIGHT.store(right.max(0) as u32, Ordering::Relaxed);
}

/// The display's refresh rate in millihertz (60 Hz until the activity says).
static REFRESH_MILLIHERTZ: AtomicU32 = AtomicU32::new(60_000);

/// The display's refresh rate in millihertz, for frame interpolation's pacing (`screen.rs`).
pub fn refresh_millihertz() -> u32 {
    REFRESH_MILLIHERTZ.load(Ordering::Relaxed)
}

/// `TuruActivity.nativeRefreshRate(hz)`: the activity started, or its display changed mode.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeRefreshRate<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    hz: jfloat,
) {
    if hz.is_finite() && hz > 0.0 {
        REFRESH_MILLIHERTZ.store((hz * 1000.0).round() as u32, Ordering::Relaxed);
    }
}

pub struct AndroidUiBackend {
    inner: NullUiBackend,
}

impl AndroidUiBackend {
    pub fn new() -> Self {
        Self {
            inner: NullUiBackend::new(),
        }
    }
}

impl UiBackend for AndroidUiBackend {
    fn open_virtual_keyboard(&self) {
        soft_keyboard(true);
    }

    fn close_virtual_keyboard(&self) {
        soft_keyboard(false);
    }

    fn mouse_visible(&self) -> bool {
        self.inner.mouse_visible()
    }

    fn set_mouse_visible(&mut self, visible: bool) {
        self.inner.set_mouse_visible(visible);
    }

    fn set_mouse_cursor(&mut self, cursor: MouseCursor) {
        self.inner.set_mouse_cursor(cursor);
    }

    fn clipboard_content(&mut self) -> String {
        self.inner.clipboard_content()
    }

    fn set_clipboard_content(&mut self, content: String) {
        self.inner.set_clipboard_content(content);
    }

    fn set_fullscreen(&mut self, is_full: bool) -> Result<(), FullscreenError> {
        self.inner.set_fullscreen(is_full)
    }

    fn display_root_movie_download_failed_message(&self, invalid_swf: bool, fetched_error: String) {
        tracing::error!("skua.swf failed to load (invalid SWF: {invalid_swf}): {fetched_error}");
    }

    /// AQW's dash button and keys listen for the fingers that aren't the mouse
    /// (`Screen::touch`).
    fn supports_touch_events(&self) -> bool {
        true
    }

    fn message(&self, message: &str) {
        tracing::info!("{message}");
    }

    fn language(&self) -> LanguageIdentifier {
        self.inner.language()
    }

    fn display_unsupported_video(&self, url: Url) {
        self.inner.display_unsupported_video(url);
    }

    fn load_device_font(&self, query: &FontQuery, register: &mut dyn FnMut(FontDefinition)) {
        self.inner.load_device_font(query, register);
    }

    fn sort_device_fonts(&self, query: &FontQuery, register: &mut dyn FnMut(FontDefinition)) -> Vec<FontQuery> {
        self.inner.sort_device_fonts(query, register)
    }

    fn display_file_open_dialog(&mut self, filters: Vec<FileFilter>) -> Option<DialogResultFuture> {
        self.inner.display_file_open_dialog(filters)
    }

    fn display_file_open_dialog_multiple(&mut self, filters: Vec<FileFilter>) -> Option<MultiDialogResultFuture> {
        self.inner.display_file_open_dialog_multiple(filters)
    }

    fn display_file_save_dialog(&mut self, file_name: String, title: String) -> Option<DialogResultFuture> {
        self.inner.display_file_save_dialog(file_name, title)
    }

    fn close_file_dialog(&mut self) {
        self.inner.close_file_dialog();
    }
}
