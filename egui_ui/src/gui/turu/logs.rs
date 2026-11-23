//! Diagnostics > Logs: the script log and the debug log, each with Copy / Save / Clear (and
//! Share on Android), and the crash reports earlier runs left (`crate::crash_reports`), with a
//! notice the first time Turu starts after a new one.

use super::files::SaveText;
use super::theme::MUTED;
use super::widgets::{self, Kind, LogView};
use crate::crash_reports;
use crate::models::crashes::{self, CrashRow, CrashText};
use egui::{Align, Context, Layout, RichText, ScrollArea, Ui, vec2};
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
    if crate::host::host().has_file_dialogs()
        && widgets::button(ui, "Save", Kind::Normal, !buffer.is_empty(), size, "").clicked()
    {
        save.ask(ui.ctx(), file_name, buffer.text());
    }
    if crate::host::host().shares_text()
        && widgets::button(ui, "Share", Kind::Normal, !buffer.is_empty(), size, "").clicked()
    {
        let text = crash_reports::shareable(&buffer.text(), crash_reports::SHARE_BYTES);
        crate::host::host().share_text(&format!("Turu {file_name}"), &text);
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
    Crashes,
}

#[derive(Default)]
pub struct LogsWindow {
    tab: Tab,
    script: LogView,
    debug: LogView,
    save: SaveText,
    crashes: Crashes,
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

    /// Opens on the Crashes tab with the report `name` chosen (the crash notice's Show report).
    pub fn show_report(&mut self, name: &str) {
        self.tab = Tab::Crashes;
        self.crashes.selected = Some(name.to_owned());
        self.crashes.listed_at = None;
    }

    fn ui(&mut self, ui: &mut Ui) {
        let now = ui.input(|i| i.time);
        self.crashes.refresh(now);
        let crashes = self.crashes.reports.len();
        ui.horizontal(|ui| {
            if widgets::chip(ui, "Script", self.tab == Tab::Script) {
                self.tab = Tab::Script;
            }
            if widgets::chip(ui, "Debug", self.tab == Tab::Debug) {
                self.tab = Tab::Debug;
            }
            // Only once there is one: most never crash.
            if (crashes > 0 || self.tab == Tab::Crashes)
                && widgets::chip(ui, &format!("Crashes ({crashes})"), self.tab == Tab::Crashes)
            {
                self.tab = Tab::Crashes;
                self.crashes.listed_at = None;
            }
            if self.tab == Tab::Crashes {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| self.crashes.buttons(ui));
            }
        });
        let bot = crate::turu::bot();
        let (buffer, view, file_name) = match self.tab {
            Tab::Script => (&bot.logs.script, &mut self.script, "script-log.txt"),
            Tab::Debug => (&bot.logs.debug, &mut self.debug, "debug-log.txt"),
            Tab::Crashes => {
                self.crashes.ui(ui);
                return;
            }
        };
        view.set_wrap(true);
        view.refresh(buffer);
        ui.horizontal(|ui| {
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

/// The Crashes tab: the reports, newest first, and the chosen one's text (`models::crashes`).
#[derive(Default)]
struct Crashes {
    reports: Vec<CrashRow>,
    /// `Context` time of the last listing (`None`: list again).
    listed_at: Option<f64>,
    /// The chosen report's name.
    selected: Option<String>,
    /// The chosen report's text, and which report it is.
    text: Option<(String, Result<CrashText, String>)>,
}

impl Crashes {
    /// Lists the folder again every few seconds (a report may arrive while Turu runs: a panic on
    /// another thread is written just before the abort, so not really, but deletions are).
    fn refresh(&mut self, now: f64) {
        if self.listed_at.is_some_and(|at| now - at < 3.0) {
            return;
        }
        self.listed_at = Some(now);
        self.reports = crashes::view().reports;
        if !self.reports.iter().any(|r| Some(&r.name) == self.selected.as_ref()) {
            self.selected = self.reports.first().map(|r| r.name.clone());
        }
        if self.text.as_ref().map(|(name, _)| name) != self.selected.as_ref() {
            self.text = self.selected.as_ref().map(|name| (name.clone(), crashes::text(name)));
        }
    }

    fn buttons(&mut self, ui: &mut Ui) {
        let size = vec2(0.0, widgets::target(22.0, 40.0));
        let has = self.text.as_ref().is_some_and(|(_, read)| read.is_ok());
        if widgets::button(ui, "Delete", Kind::Normal, self.text.is_some(), size, "").clicked()
            && let Some((name, _)) = self.text.take()
        {
            if let Err(e) = crashes::delete(&name) {
                tracing::warn!("{e}");
            }
            self.selected = None;
            self.listed_at = None;
            return;
        }
        let read = self.text.as_ref().and_then(|(_, read)| read.as_ref().ok());
        ui.add_space(10.0);
        let platform = crate::host::host();
        if platform.shares_text()
            && widgets::button(ui, "Share", Kind::Primary, has, size, "").clicked()
            && let Some(read) = read
        {
            platform.share_text(&read.subject, &read.text);
        }
        if widgets::button(ui, "Copy", Kind::Normal, has, size, "").clicked()
            && let Some(read) = read
        {
            ui.ctx().copy_text(read.text.clone());
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        if self.reports.is_empty() {
            ui.label(RichText::new("No crash reports.").color(MUTED));
            return;
        }
        let row = widgets::target(18.0, 40.0);
        let list_height = (ui.available_height() * 0.35).clamp(row, row * 4.0);
        ScrollArea::vertical()
            .id_salt("turu_crash_list")
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for report in &self.reports {
                    let chosen = self.selected.as_ref() == Some(&report.name);
                    let label = format!("{}  {}", report.written, report.summary);
                    let response = ui.add_sized(
                        [ui.available_width(), row],
                        egui::Button::selectable(chosen, RichText::new(label).small()).truncate(),
                    );
                    if response.clicked() && !chosen {
                        self.selected = Some(report.name.clone());
                        self.listed_at = None;
                    }
                }
            });
        ui.separator();
        if let Some((_, read)) = &self.text {
            let text = match read {
                Ok(read) => read.text.as_str(),
                Err(e) => e.as_str(),
            };
            ScrollArea::both()
                .id_salt("turu_crash_text")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(text).monospace().small()).extend());
                });
        }
    }
}

pub use crate::models::prompts::answer_crash_upload_question;
