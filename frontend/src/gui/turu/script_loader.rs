//! Scripts > Script Loader (VibeSkua's 370x500 window): loaded script and status, the run
//! controls, the setup buttons, and the script log.

use super::files::{self, Pending, SaveText};
use super::theme::{DANGER, MUTED, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::platform::FileDialog;
use egui::{Align, Context, CursorIcon, Key, Label, Layout, RichText, Sense, Ui, vec2};
use std::path::PathBuf;
use std::time::Duration;
use turu_bot::script::{self, ScriptOptionsState, ScriptStatus};

/// A running script whose log has been quiet this long gets its "last log" line in amber: it
/// may be stuck.
const QUIET_FOR: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
pub struct ScriptLoader {
    pending_load: Pending,
    save: SaveText,
    log: widgets::LogView,
    vscode: Option<Option<PathBuf>>,
    /// The whole error shows in the card: its line is no longer in the log to reveal.
    error_open: bool,
}

impl ScriptLoader {
    pub fn show(&mut self, ctx: &Context, open: &mut bool, options_open: &mut bool, repo_open: &mut bool) {
        let bot = crate::turu::bot();
        self.save.poll();
        if let Some(path) = self.pending_load.poll() {
            bot.scripts.load(&path);
            bot.log(format!("Loaded script {}", path.display()));
        }
        if !*open {
            return;
        }
        super::window(ctx, "Script Loader", [370.0, 500.0], 0)
            .open(open)
            .show(ctx, |ui| self.ui(ui, options_open, repo_open));
        // The script writes its log and changes status from its own thread.
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    /// Load Script's file dialog (also VibeSkua's LoadScript hotkey, F9), unless a script runs or
    /// a dialog is open already.
    pub fn pick_script(&mut self, ctx: &Context) {
        let bot = crate::turu::bot();
        if bot.scripts.status().is_active() || self.pending_load.is_open() {
            return;
        }
        let start = files::scripts_start_dir(bot.paths.scripts_dir());
        let dialog = FileDialog::new()
            .set_title("Load Script")
            .add_filter("Scripts", &["ts", "js"])
            .set_directory(start);
        self.pending_load.pick_file(ctx, dialog);
    }

    fn ui(&mut self, ui: &mut Ui, options_open: &mut bool, repo_open: &mut bool) {
        let bot = crate::turu::bot();
        let status = bot.scripts.status();
        let loaded = bot.scripts.loaded();
        let options = bot.scripts.options();
        let active = status.is_active();
        let platform = crate::platform::platform();
        let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
        self.log.set_wrap(true);
        self.log.refresh(&bot.logs.script);

        ui.spacing_mut().item_spacing = vec2(6.0, 10.0);

        // Status card: what a check-in looks for. The state leads, then the script and, after a
        // failure, the error's first line (clicking it shows the error in the log).
        let elapsed = status
            .is_active()
            .then(|| bot.state.lock().stats.started)
            .flatten()
            .map(|started| started.elapsed());
        let error = (status == ScriptStatus::Error)
            .then(|| bot.scripts.last_end().1.and_then(|end| end.error))
            .flatten();
        if error.is_none() {
            self.error_open = false;
        }
        let last_run = last_run(&bot);
        // While it runs: how long ago the script last logged, the answer to "is it stuck?".
        let last_log = active.then(|| bot.logs.script.last_time()).flatten();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                widgets::dot(ui, widgets::status_color(status), 5.0);
                let mut state = state_text(status).to_owned();
                if let Some(elapsed) = elapsed {
                    state.push_str(" · ");
                    state.push_str(&duration_text(elapsed));
                }
                // After a run: how it ended and how long it ran, until the next start or load.
                if let Some(run) = &last_run {
                    state = match status {
                        ScriptStatus::Error => format!("{state} · ran {}", run.ran),
                        _ => format!("{} · ran {}", run.how, run.ran),
                    };
                }
                let label = ui.label(RichText::new(state).size(16.0).strong().color(TEXT));
                if let Some(run) = &last_run {
                    label.on_hover_text(format!("Ended at {}", run.at));
                }
                if let Some((time, quiet)) = last_log {
                    let text = if quiet < Duration::from_secs(5) {
                        "last log just now".to_owned()
                    } else {
                        format!("last log {} ago", duration_text(quiet))
                    };
                    let color = if status == ScriptStatus::Running && quiet >= QUIET_FOR {
                        WARNING
                    } else {
                        MUTED
                    };
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(text).size(12.0).color(color))
                            .on_hover_text(format!("The script log's newest line, at {}", time.format("%H:%M:%S")));
                    });
                }
            });
            match &loaded {
                Some(path) => {
                    let name = path
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    ui.horizontal(|ui| {
                        // For script authors: one way to open the file, in VS Code when there is
                        // one. Small and beside the name, so it stays out of a runner's way.
                        if platform.opens_files() {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (label, tip) = match &code {
                                    Some(_) => ("Open in VS Code", "Opens the script in VS Code"),
                                    None => (
                                        "Open in editor",
                                        "Opens the script in the app your system opens it with",
                                    ),
                                };
                                let size = vec2(0.0, widgets::target(22.0, 40.0));
                                if widgets::button(ui, label, Kind::Normal, true, size, "")
                                    .on_hover_text(tip)
                                    .clicked()
                                {
                                    match &code {
                                        Some(code) => files::open_in_vscode(code, path),
                                        None => files::open_with_default(path),
                                    }
                                }
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    script_name(ui, &name, path);
                                });
                            });
                        } else {
                            script_name(ui, &name, path);
                        }
                    });
                }
                None => {
                    // Only the buttons this platform shows (Android has no file dialogs).
                    let hint = if platform.has_file_dialogs() {
                        "Load Script or Search Scripts to pick one"
                    } else {
                        "Search Scripts to pick one"
                    };
                    ui.label(RichText::new(hint).color(MUTED));
                }
            }
            if let Some(error) = &error {
                let first = error.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim();
                if self.error_open {
                    // Its line is gone from the log (cleared, or pushed out): the whole error here.
                    let text = ui.add(
                        Label::new(RichText::new(error.trim()).size(12.0).color(DANGER))
                            .wrap()
                            .selectable(true),
                    );
                    if text.clicked() {
                        self.error_open = false;
                    }
                } else {
                    let line = ui
                        .add(
                            Label::new(RichText::new(first).size(12.0).color(DANGER))
                                .truncate()
                                .sense(Sense::click()),
                        )
                        .on_hover_text("Show the error in the script log");
                    if line.clicked() && !self.log.reveal(first) {
                        self.error_open = true;
                    }
                    if line.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                }
            }
        });

        // The run controls lead, right under the status: they are what a check-in reaches for.
        // With nothing running, Start takes the row; while it runs, Pause/Resume joins it, and
        // while it stops, Force Stop takes that place (never Stop's own, so a double press of
        // Stop doesn't skip the stopping steps). The setup buttons follow, smaller and outlined.
        // 6 px inside a group, 14 px between. On a touch screen every button is 48 points high
        // and the reasons a button is disabled (tooltips elsewhere) show as a line under its group.
        let touch = widgets::touch();
        let full = ui.available_width();
        let half = (full - 6.0) / 2.0;
        let run = ((full - 6.0) * 0.62).floor();
        let run_height = widgets::target(38.0, 48.0);
        let setup_height = widgets::target(28.0, 48.0);
        let auto = super::auto::running(&bot);
        let snippet = bot.scripts.is_snippet_running();
        let stopping = status == ScriptStatus::Stopping;
        let second = matches!(status, ScriptStatus::Running | ScriptStatus::Paused) || stopping;
        let main_width = if second { run } else { full };
        let force_tip = "Interrupts the script's stopping steps (such as going to the stop location)";
        let mut run_hint = None;
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            if active {
                let stop = widgets::button(
                    ui,
                    if stopping { "Stopping..." } else { "Stop Script" },
                    Kind::Danger,
                    !stopping,
                    vec2(main_width, run_height),
                    "The script is running its stopping steps",
                )
                .on_hover_text(format!("Stops the script ({TOGGLE_KEY_NAME})"));
                if stop.clicked() {
                    script::stop(&bot);
                }
            } else {
                let tip = if auto {
                    "Stop Auto Attack/Hunt first"
                } else if snippet {
                    "Stop the Console snippet first"
                } else {
                    "Load a script first"
                };
                // Without a script the status card already says how to get one.
                if loaded.is_some() && (auto || snippet) {
                    run_hint = Some(tip);
                }
                if widgets::button(
                    ui,
                    "Start Script",
                    Kind::Primary,
                    loaded.is_some() && !auto && !snippet,
                    vec2(main_width, run_height),
                    tip,
                )
                .on_hover_text(format!("Starts the loaded script ({TOGGLE_KEY_NAME})"))
                .clicked()
                    && let Err(e) = script::start(&bot)
                {
                    bot.log(format!("Couldn't start the script: {e}"));
                }
            }
            let size = vec2(full - 6.0 - run, run_height);
            if stopping {
                run_hint = Some(force_tip);
                let ready = bot.scripts.can_force_stop();
                if widgets::button(ui, "Force Stop", Kind::Danger, ready, size, "Available in a moment")
                    .on_hover_text(format!("{force_tip} ({TOGGLE_KEY_NAME})"))
                    .clicked()
                {
                    script::stop(&bot);
                }
            } else if second {
                let paused = status == ScriptStatus::Paused;
                let label = if paused { "Resume Script" } else { "Pause Script" };
                if widgets::button(ui, label, Kind::Normal, true, size, "").clicked() {
                    script::set_paused(&bot, !paused);
                }
            }
        });
        if touch && let Some(hint) = run_hint {
            hint_line(ui, hint);
        }

        // Getting a script, then setting it up. What the platform can't do isn't offered:
        // Android has no file dialogs, so it keeps Search Scripts and Edit Script Options.
        let (has_options, options_tip) = match (&loaded, &options) {
            (None, _) => (false, "Load a script first"),
            (Some(_), ScriptOptionsState::Loaded(_)) => (true, ""),
            (Some(_), ScriptOptionsState::Failed(_)) => (false, "The script's options couldn't be read"),
            (Some(_), ScriptOptionsState::NoOptions) => (false, "This script declares no options"),
        };
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let width = if platform.has_file_dialogs() { half } else { full };
            if platform.has_file_dialogs() {
                let enabled = !active && !self.pending_load.is_open();
                let size = vec2(width, setup_height);
                if widgets::button(ui, "Load Script", Kind::Outline, enabled, size, "Stop the script first")
                    .on_hover_text(format!("Opens a script file ({LOAD_KEY_NAME})"))
                    .clicked()
                {
                    self.pick_script(ui.ctx());
                }
            }
            if widgets::button(ui, "Search Scripts", Kind::Outline, true, vec2(width, setup_height), "").clicked() {
                *repo_open = true;
            }
        });
        let size = vec2(full, setup_height);
        if widgets::button(ui, "Edit Script Options", Kind::Outline, has_options, size, options_tip).clicked() {
            *options_open = true;
        }
        if touch && loaded.is_some() && !has_options {
            hint_line(ui, options_tip);
        }

        // Script log, with a find box: the lines that contain its text (Escape clears it).
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            widgets::section_label(ui, "SCRIPT LOG");
            if !self.log.filter_mut().is_empty() {
                widgets::count_badge(ui, self.log.matches());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                super::logs::log_buttons(ui, &bot.logs.script, &mut self.save, "script-log.txt");
                let width = (ui.available_width() - 4.0).max(60.0);
                let find = widgets::text_box(ui, self.log.filter_mut(), "Find", width);
                if find.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.log.filter_mut().clear();
                }
            });
        });
        let height = ui.available_height();
        self.log.show(ui, "script_log", height);
    }
}

/// The loaded script's file name, cut to fit; its whole path on hover.
fn script_name(ui: &mut Ui, name: &str, path: &std::path::Path) {
    ui.add(Label::new(RichText::new(name).monospace().color(TEXT)).truncate())
        .on_hover_text(path.display().to_string());
}

/// How the loaded script's last run ended, for the status card and the nav bar's chip.
pub(super) struct LastRun {
    /// "Finished" or "Stopped".
    pub how: &'static str,
    /// How long it ran ("6h 12m").
    pub ran: String,
    /// When it ended ("04:12").
    pub at: String,
}

/// The last run's end while it's what the window shows: the script stopped (Ready or Error)
/// and it's the one that ran (loading another one starts afresh).
pub(super) fn last_run(bot: &turu_bot::Bot) -> Option<LastRun> {
    let status = bot.scripts.status();
    if !matches!(status, ScriptStatus::Loaded | ScriptStatus::Error) {
        return None;
    }
    let end = bot.scripts.last_end().1?;
    if bot.scripts.loaded().as_deref() != Some(end.script.as_path()) {
        return None;
    }
    Some(LastRun {
        how: if end.finished { "Finished" } else { "Stopped" },
        ran: duration_text(end.ran),
        at: end.ended_clock(),
    })
}

/// VibeSkua's default ToggleScript hotkey: starts the loaded script, or stops the running one.
pub(super) const TOGGLE_KEY: egui::Key = egui::Key::F10;
const TOGGLE_KEY_NAME: &str = "F10";
/// VibeSkua's default LoadScript hotkey: Load Script's file dialog.
pub(super) const LOAD_KEY: egui::Key = egui::Key::F9;
const LOAD_KEY_NAME: &str = "F9";

/// VibeSkua's ToggleScript hotkey: what Start Script / Stop Script do, under the same rules (no
/// start while Auto or a Console snippet runs). A stop while stopping is Force Stop, once it is
/// offered ([`turu_bot::script::FORCE_STOP_AFTER`]).
pub(super) fn toggle_script() {
    let bot = crate::turu::bot();
    let status = bot.scripts.status();
    if status == ScriptStatus::Stopping {
        if bot.scripts.can_force_stop() {
            script::stop(&bot);
        }
    } else if status.is_active() {
        script::stop(&bot);
    } else if bot.scripts.loaded().is_some()
        && !super::auto::running(&bot)
        && !bot.scripts.is_snippet_running()
        && let Err(e) = script::start(&bot)
    {
        bot.log(format!("Couldn't start the script: {e}"));
    }
}

/// Why a button is disabled, as a line under its group: a touch screen shows no tooltips.
fn hint_line(ui: &mut Ui, text: &str) {
    ui.add(Label::new(RichText::new(text).size(12.0).color(MUTED)).wrap());
}

/// The status in words, for the status card (`ScriptStatus::label` keeps VibeSkua's "[Running]"
/// for the control API and the monitor).
pub(super) fn state_text(status: ScriptStatus) -> &'static str {
    match status {
        ScriptStatus::Empty => "No script loaded",
        ScriptStatus::Loaded => "Ready",
        ScriptStatus::Compiling => "Starting...",
        ScriptStatus::Running => "Running",
        ScriptStatus::Paused => "Paused",
        ScriptStatus::Stopping => "Stopping...",
        ScriptStatus::Error => "Stopped with an error",
    }
}

/// How long the script has run, as the card shows it: "45s", "12m", "3h 05m".
pub(super) fn duration_text(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m", secs / 60),
        _ => format!("{}h {:02}m", secs / 3600, secs / 60 % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::duration_text;
    use std::time::Duration;

    #[test]
    fn durations_read_at_a_glance() {
        assert_eq!(duration_text(Duration::from_secs(45)), "45s");
        assert_eq!(duration_text(Duration::from_secs(12 * 60 + 30)), "12m");
        assert_eq!(duration_text(Duration::from_secs(3 * 3600 + 5 * 60)), "3h 05m");
    }
}
