//! Diagnostics > Logs: the script log and the debug log, each with Copy / Save / Clear.

use super::files::SaveText;
use super::theme::MUTED;
use super::widgets::{self, LogView};
use crate::gui::FilePicker;
use egui::{Context, Ui};
use turu_bot::log::LogBuffer;

/// "Copy", "Save", "Clear" for one log buffer; call [`SaveText::poll`] each frame too.
pub fn log_buttons(ui: &mut Ui, picker: &FilePicker, buffer: &LogBuffer, save: &mut SaveText, file_name: &str) {
    if widgets::small_button(ui, "Clear", true).clicked() {
        buffer.clear();
    }
    if widgets::small_button(ui, "Save", !buffer.is_empty()).clicked() {
        save.ask(picker, ui.ctx(), file_name, buffer.text());
    }
    if widgets::small_button(ui, "Copy", !buffer.is_empty()).clicked() {
        ui.ctx().copy_text(buffer.text());
    }
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Tab {
    #[default]
    Script,
    Debug,
}

#[derive(Default)]
pub struct LogsWindow {
    tab: Tab,
    script: LogView,
    debug: LogView,
    save: SaveText,
}

impl LogsWindow {
    pub fn show(&mut self, ctx: &Context, picker: &FilePicker, open: &mut bool) {
        self.save.poll();
        if !*open {
            return;
        }
        super::window(ctx, "Logs", [560.0, 380.0], 4)
            .open(open)
            .show(ctx, |ui| self.ui(ui, picker));
        // New lines arrive from other threads.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut Ui, picker: &FilePicker) {
        let bot = crate::turu::bot();
        let (buffer, view, file_name) = match self.tab {
            Tab::Script => (&bot.logs.script, &mut self.script, "script-log.txt"),
            Tab::Debug => (&bot.logs.debug, &mut self.debug, "debug-log.txt"),
        };
        view.refresh(buffer);
        ui.horizontal(|ui| {
            if widgets::chip(ui, "Script", self.tab == Tab::Script) {
                self.tab = Tab::Script;
            }
            if widgets::chip(ui, "Debug", self.tab == Tab::Debug) {
                self.tab = Tab::Debug;
            }
            ui.label(
                egui::RichText::new(format!("{} lines", buffer.len()))
                    .small()
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                log_buttons(ui, picker, buffer, &mut self.save, file_name);
            });
        });
        let height = ui.available_height();
        view.show(ui, "logs_view", height);
    }
}
