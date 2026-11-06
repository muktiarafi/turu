//! Diagnostics > Logs: the script log and the debug log, each with Copy / Save / Clear (and
//! Share on Android), and the crash reports earlier runs left (`crate::crash_reports`), with a
//! notice the first time Turu starts after a new one.

use super::files::SaveText;
use super::theme::{MUTED, WARNING};
use super::widgets::{self, Kind, LogView};
use crate::crash_reports::{self, Report};
use egui::{Align, Context, Layout, Modal, RichText, ScrollArea, Ui, vec2};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};
use turu_bot::log::{LogBuffer, LogLine};

/// What a Share hands over at most (Android's binder limit is ~1 MB a transaction).
const SHARE_BYTES: usize = 400_000;

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
    if crate::platform::platform().shares_text()
        && widgets::button(ui, "Share", Kind::Normal, !buffer.is_empty(), size, "").clicked()
    {
        let text = crash_reports::shareable(&buffer.text(), SHARE_BYTES);
        crate::platform::platform().share_text(&format!("Turu {file_name}"), &text);
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

    /// Opens on the Crashes tab with `report` chosen.
    fn show_report(&mut self, report: &Report) {
        self.tab = Tab::Crashes;
        self.crashes.selected = Some(report.path.clone());
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

/// The Crashes tab: the reports, newest first, and the chosen one's text.
#[derive(Default)]
struct Crashes {
    reports: Vec<Report>,
    /// `Context` time of the last listing (`None`: list again).
    listed_at: Option<f64>,
    selected: Option<PathBuf>,
    /// The chosen report's text, and which report it is.
    text: Option<(PathBuf, String)>,
}

impl Crashes {
    /// Lists the folder again every few seconds (a report may arrive while Turu runs: a panic on
    /// another thread is written just before the abort, so not really, but deletions are).
    fn refresh(&mut self, now: f64) {
        if self.listed_at.is_some_and(|at| now - at < 3.0) {
            return;
        }
        self.listed_at = Some(now);
        self.reports = crash_reports::list();
        if !self.reports.iter().any(|r| Some(&r.path) == self.selected.as_ref()) {
            self.selected = self.reports.first().map(|r| r.path.clone());
        }
        if self.text.as_ref().map(|(path, _)| path) != self.selected.as_ref() {
            self.text = self.selected.as_ref().map(|path| {
                let text = std::fs::read_to_string(path).unwrap_or_else(|e| format!("Couldn't read it: {e}"));
                (path.clone(), text)
            });
        }
    }

    fn buttons(&mut self, ui: &mut Ui) {
        let size = vec2(0.0, widgets::target(22.0, 40.0));
        let has = self.text.is_some();
        if widgets::button(ui, "Delete", Kind::Normal, has, size, "").clicked()
            && let Some((path, _)) = self.text.take()
        {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("Couldn't delete {}: {e}", path.display());
            }
            self.selected = None;
            self.listed_at = None;
        }
        ui.add_space(10.0);
        let platform = crate::platform::platform();
        if platform.shares_text()
            && widgets::button(ui, "Share", Kind::Primary, has, size, "").clicked()
            && let Some((path, text)) = &self.text
        {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("crash report");
            platform.share_text(
                &format!("Turu {} crash report: {name}", crate::turu::VERSION),
                &crash_reports::shareable(text, SHARE_BYTES),
            );
        }
        if widgets::button(ui, "Copy", Kind::Normal, has, size, "").clicked()
            && let Some((_, text)) = &self.text
        {
            ui.ctx().copy_text(crash_reports::shareable(text, SHARE_BYTES));
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
                    let chosen = self.selected.as_ref() == Some(&report.path);
                    let when = chrono::DateTime::<chrono::Local>::from(report.modified).format("%b %-d, %H:%M");
                    let label = format!("{when}  {}", report.summary);
                    let response = ui.add_sized(
                        [ui.available_width(), row],
                        egui::Button::selectable(chosen, RichText::new(label).small()).truncate(),
                    );
                    if response.clicked() && !chosen {
                        self.selected = Some(report.path.clone());
                        self.listed_at = None;
                    }
                }
            });
        ui.separator();
        if let Some((_, text)) = &self.text {
            ScrollArea::both()
                .id_salt("turu_crash_text")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(text.as_str()).monospace().small()).extend());
                });
        }
    }
}

/// The reports added since Turu last showed the notice, looked up once per process.
static UNSEEN: Mutex<Vec<Report>> = Mutex::new(Vec::new());
static LOOKED: Once = Once::new();

/// "Turu closed unexpectedly" the first time Turu starts after a new crash report, with a way
/// to the report (Logs > Crashes, where Share is).
pub fn show_crash_notice(ctx: &Context, logs_open: &mut bool, logs: &mut LogsWindow) {
    LOOKED.call_once(|| *UNSEEN.lock().unwrap_or_else(|e| e.into_inner()) = crash_reports::take_unseen());
    let mut unseen = UNSEEN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(newest) = unseen.first().cloned() else {
        return;
    };
    // One box at a time: the script's own first.
    if crate::turu::bot().scripts.message_box().is_some() {
        return;
    }
    let mut show = None;
    let size = vec2(0.0, widgets::target(28.0, 44.0));
    let response = Modal::new(egui::Id::new("turu_crash_notice")).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.set_max_width(420.0);
        ui.heading("Turu closed unexpectedly");
        ui.label(RichText::new(&newest.summary).color(WARNING));
        if unseen.len() > 1 {
            ui.label(RichText::new(format!("And {} more since the last notice.", unseen.len() - 1)).color(MUTED));
        }
        ui.label(
            RichText::new("Diagnostics > Logs > Crashes has the report. Share it with whoever helps you with Turu.")
                .small()
                .color(MUTED),
        );
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, "Show report", Kind::Primary, true, size, "").clicked() {
                show = Some(true);
            }
            if widgets::button(ui, "Close", Kind::Normal, true, size, "").clicked() {
                show = Some(false);
            }
        });
    });
    if show.is_none() && response.should_close() {
        show = Some(false);
    }
    if let Some(show) = show {
        unseen.clear();
        if show {
            *logs_open = true;
            logs.show_report(&newest);
        }
    }
}

/// "Send crash reports automatically?", once: where the build can send them
/// (`Platform::uploads_crash_reports`) and the user hasn't answered (`send_crash_reports`). After
/// the crash notice, and never over a script's message box. Closing it without an answer asks
/// again at the next start; Options > Application has the switch.
pub fn show_crash_upload_question(ctx: &Context) {
    static DISMISSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DISMISSED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let bot = crate::turu::bot();
    if bot.settings.read().send_crash_reports.is_some()
        || !UNSEEN.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
        || bot.scripts.message_box().is_some()
        || !crate::platform::platform().uploads_crash_reports()
    {
        return;
    }
    let mut answer = None;
    let size = vec2(0.0, widgets::target(28.0, 44.0));
    let response = Modal::new(egui::Id::new("turu_crash_upload_question")).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.set_max_width(420.0);
        ui.heading("Send crash reports?");
        ui.label(
            "When Turu crashes, it can send a report to its developers so they can fix it: what crashed, \
             the phone's model and Android version, and Turu's version.",
        );
        ui.label(
            RichText::new("No account names, passwords or game data. You can change this in Options > Application.")
                .small()
                .color(MUTED),
        );
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, "Send reports", Kind::Primary, true, size, "").clicked() {
                answer = Some(true);
            }
            if widgets::button(ui, "Don't send", Kind::Normal, true, size, "").clicked() {
                answer = Some(false);
            }
        });
    });
    match answer {
        Some(on) => {
            let mut settings = bot.settings.read().clone();
            settings.send_crash_reports = Some(on);
            *bot.settings.write() = settings;
            bot.save_settings();
            crate::platform::platform().set_crash_uploads(on);
        }
        None if response.should_close() => DISMISSED.store(true, std::sync::atomic::Ordering::Relaxed),
        None => {}
    }
}
