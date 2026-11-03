//! The player's UI backend: Ruffle's null one, except that a text field taking focus shows the
//! soft keyboard and losing it hides the keyboard (while the activity is there to show it).

use ruffle_core::backend::ui::{
    DialogResultFuture, FileFilter, FontDefinition, FullscreenError, LanguageIdentifier, MouseCursor,
    MultiDialogResultFuture, NullUiBackend, UiBackend,
};
use ruffle_core::font::FontQuery;
use url::Url;

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
        if let Some(app) = crate::app() {
            app.show_soft_input(true);
        }
    }

    fn close_virtual_keyboard(&self) {
        if let Some(app) = crate::app() {
            app.hide_soft_input(true);
        }
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
