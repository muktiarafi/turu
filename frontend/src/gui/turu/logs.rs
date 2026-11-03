//! Diagnostics > Logs: the script log and the debug log, each with Copy / Save / Clear.

use super::files::SaveText;
use super::theme::MUTED;
use super::widgets::{self, Kind, LogView};
use egui::{Context, Ui, vec2};
use std::sync::Arc;
use turu_bot::log::{LogBuffer, LogLine};

/// How long Clear can be undone.
const UNDO_CLEAR_FOR: f64 = 8.0;

/// What a Clear took from a log, until it can't be undone any more (egui's memory, per buffer).
#[derive(Clone)]
struct ClearedLines {
    lines: Arc<Vec<LogLine>>,
    /// `Context` time when the undo ends.
    until: f64,
}

/// "Copy", "Save", "Clear" for one log buffer; call [`SaveText::poll`] each frame too. Clear is
/// apart from the others and turns into "Undo clear" for a few seconds: after a long run the log
/// is its only record.
pub fn log_buttons(ui: &mut Ui, buffer: &LogBuffer, save: &mut SaveText, file_name: &str) {
    // Finger-sized on a touch screen; Save only where it can ask where to save.
    let size = vec2(0.0, widgets::target(22.0, 40.0));
    let id = egui::Id::new(("turu_log_cleared", std::ptr::from_ref(buffer) as usize));
    let now = ui.input(|i| i.time);
    let cleared = ui.data(|d| d.get_temp::<ClearedLines>(id));
    // Past the undo, the lines are let go.
    let cleared = match cleared {
        Some(c) if c.until <= now => {
            ui.data_mut(|d| d.remove::<ClearedLines>(id));
            None
        }
        other => other,
    };
    match cleared {
        Some(cleared) => {
            if widgets::button(ui, "Undo clear", Kind::Primary, true, size, "")
                .on_hover_text(format!("Puts back the {} lines Clear took", cleared.lines.len()))
                .clicked()
            {
                buffer.restore(cleared.lines.as_ref().clone());
                ui.data_mut(|d| d.remove::<ClearedLines>(id));
            } else {
                ui.ctx().request_repaint_after_secs((cleared.until - now) as f32);
            }
        }
        None => {
            if widgets::button(ui, "Clear", Kind::Normal, !buffer.is_empty(), size, "").clicked() {
                let lines = Arc::new(buffer.take());
                ui.data_mut(|d| {
                    d.insert_temp(
                        id,
                        ClearedLines {
                            lines,
                            until: now + UNDO_CLEAR_FOR,
                        },
                    );
                });
            }
        }
    }
    // A gap between the one that removes and the ones that don't.
    ui.add_space(10.0);
    if crate::platform::platform().has_file_dialogs()
        && widgets::button(ui, "Save", Kind::Normal, !buffer.is_empty(), size, "").clicked()
    {
        save.ask(ui.ctx(), file_name, buffer.text());
    }
    if widgets::button(ui, "Copy", Kind::Normal, !buffer.is_empty(), size, "").clicked() {
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
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        self.save.poll();
        if !*open {
            return;
        }
        super::window(ctx, "Logs", [560.0, 380.0], 4)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        // New lines arrive from other threads.
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let (buffer, view, file_name) = match self.tab {
            Tab::Script => (&bot.logs.script, &mut self.script, "script-log.txt"),
            Tab::Debug => (&bot.logs.debug, &mut self.debug, "debug-log.txt"),
        };
        view.set_wrap(true);
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
                log_buttons(ui, buffer, &mut self.save, file_name);
            });
        });
        let height = ui.available_height();
        view.show(ui, "logs_view", height);
    }
}
