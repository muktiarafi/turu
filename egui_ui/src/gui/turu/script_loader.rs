//! Scripts > Script Loader (VibeSkua's 370x500 window): loaded script and status, the run
//! controls, the setup buttons, and the script log.

use super::files::{self, Pending, SaveText};
use super::hotkeys;
use super::theme::{DANGER, MUTED, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::host::FileDialog;
use crate::models::scripts::{self as model, FORCE_STOP_DOES, Status};
use egui::{Align, Context, CursorIcon, Key, Label, Layout, RichText, Sense, Ui, vec2};
use std::path::{Path, PathBuf};
use std::time::Duration;
use turu_bot::script::ScriptStatus;

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
        if let Some(path) = self.pending_load.poll()
            && let Err(e) = model::load(&bot, &path)
        {
            bot.log(format!("Couldn't load {}: {e}", path.display()));
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
        if !model::view(&bot).controls.load.enabled || self.pending_load.is_open() {
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
        // What the window shows and allows: the scripts model's (`crate::models::scripts`).
        let view = model::view(&bot);
        let status = bot.scripts.status();
        let active = view.status.is_active();
        let platform = crate::host::host();
        let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
        self.log.set_wrap(true);
        self.log.set_empty_text(if active {
            "Nothing logged yet"
        } else {
            "The script's log shows here when it runs"
        });
        self.log.refresh(&bot.logs.script);

        ui.spacing_mut().item_spacing = vec2(6.0, 10.0);

        // Status card: what a check-in looks for. The state leads, then the script and, after a
        // failure, the error's first line (clicking it shows the error in the log).
        let elapsed = view.elapsed_secs.map(Duration::from_secs);
        let error = view.error.clone();
        if error.is_none() {
            self.error_open = false;
        }
        let last_run = view.last_run.as_ref().map(LastRun::from);
        // While it runs: how long ago the script last logged, the answer to "is it stuck?". At
        // the state's right where it fits, else on a line of its own under it.
        let last_log = view.last_log.as_ref().map(|log| {
            let quiet = Duration::from_secs(log.quiet_secs);
            let text = if quiet < Duration::from_secs(5) {
                "last log just now".to_owned()
            } else {
                format!("last log {} ago", duration_text(quiet))
            };
            let color = if log.stale { WARNING } else { MUTED };
            let tip = format!("The script log's newest line, at {}", log.at);
            (RichText::new(text).size(12.0).color(color), tip)
        });
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            let mut age_below = None;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                widgets::dot(ui, widgets::status_color(status), 5.0);
                let mut state = view.state.to_owned();
                if let Some(elapsed) = elapsed {
                    state.push_str(" · ");
                    state.push_str(&duration_text(elapsed));
                }
                // After a run: how it ended and how long it ran, until the next start or load.
                if let Some(run) = &last_run {
                    state = match view.status {
                        Status::Error => format!("{state} · ran {}", run.ran),
                        _ => format!("{} · ran {}", run.how, run.ran),
                    };
                }
                // Cut to fit a narrow window (the whole state on hover).
                let label = ui.add(Label::new(RichText::new(state).size(16.0).strong().color(TEXT)).truncate());
                if let Some(run) = &last_run {
                    label.on_hover_text(format!("Ended at {}", run.at));
                }
                if let Some((text, tip)) = last_log {
                    let width = egui::WidgetText::from(text.clone())
                        .into_galley(
                            ui,
                            Some(egui::TextWrapMode::Extend),
                            f32::INFINITY,
                            egui::TextStyle::Body,
                        )
                        .size()
                        .x;
                    if ui.available_width() >= width + 8.0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(text).on_hover_text(tip);
                        });
                    } else {
                        age_below = Some((text, tip));
                    }
                }
            });
            if let Some((text, tip)) = age_below {
                ui.label(text).on_hover_text(tip);
            }
            match &view.script {
                Some(script) => {
                    let (path, name, favourite) = (Path::new(&script.path), &script.name, script.favourite);
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
                                        Some(code) => {
                                            if let Err(e) = files::open_in_vscode(code, path) {
                                                bot.log(format!("Couldn't open VS Code: {e}"));
                                            }
                                        }
                                        None => files::open_with_default(path),
                                    }
                                }
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    favorite_star(ui, &bot, favourite);
                                    script_name(ui, name, path);
                                });
                            });
                        } else {
                            favorite_star(ui, &bot, favourite);
                            script_name(ui, name, path);
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
                let first = error.first_line.as_str();
                if self.error_open {
                    // Its line is gone from the log (cleared, or pushed out): the whole error here.
                    ui.add(
                        Label::new(RichText::new(&error.text).size(12.0).color(DANGER))
                            .wrap()
                            .selectable(true),
                    );
                    if widgets::small_button(ui, "Hide", true).clicked() {
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
        let controls = &view.controls;
        let second = controls.pause.is_some() || controls.force_stop.is_some();
        let main_width = if second { run } else { full };
        let mut run_hint = None;
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            if let Some(stop) = &controls.stop {
                // While stopping the card says so; the button keeps its name and rests.
                let button = widgets::button(
                    ui,
                    "Stop Script",
                    Kind::Danger,
                    stop.enabled,
                    vec2(main_width, run_height),
                    &stop.reason,
                )
                .on_hover_text(format!("Stops the script{}", hotkeys::hint("ToggleScript")));
                if button.clicked() {
                    model::stop(&bot);
                }
            } else if let Some(start) = &controls.start {
                // Without a script the status card already says how to get one.
                if view.script.is_some() && !start.enabled {
                    run_hint = Some(start.reason.clone());
                }
                if widgets::button(
                    ui,
                    "Start Script",
                    Kind::Primary,
                    start.enabled,
                    vec2(main_width, run_height),
                    &start.reason,
                )
                .on_hover_text(format!("Starts the loaded script{}", hotkeys::hint("ToggleScript")))
                .clicked()
                    && let Err(e) = model::start(&bot)
                {
                    bot.log(format!("Couldn't start the script: {e}"));
                }
            }
            let size = vec2(full - 6.0 - run, run_height);
            if let Some(force) = &controls.force_stop {
                run_hint = Some(if force.enabled {
                    FORCE_STOP_DOES.to_owned()
                } else {
                    force.reason.clone()
                });
                if widgets::button(ui, "Force Stop", Kind::Danger, force.enabled, size, &force.reason)
                    .on_hover_text(format!("{FORCE_STOP_DOES}{}", hotkeys::hint("ToggleScript")))
                    .clicked()
                {
                    model::stop(&bot);
                }
            } else if controls.pause.is_some() {
                // Paused, Resume is what the user came back for: it leads, not Stop.
                let (label, kind) = if controls.paused {
                    ("Resume Script", Kind::Primary)
                } else {
                    ("Pause Script", Kind::Normal)
                };
                if widgets::button(ui, label, kind, true, size, "").clicked() {
                    model::set_paused(&bot, !controls.paused);
                }
            }
        });
        if touch && let Some(hint) = run_hint {
            hint_line(ui, &hint);
        }

        // Getting a script, then setting it up. What the platform can't do isn't offered:
        // Android has no file dialogs, so it keeps Search Scripts and Edit Script Options.
        let (has_options, options_tip) = (controls.edit_options.enabled, controls.edit_options.reason.as_str());
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let width = if platform.has_file_dialogs() { half } else { full };
            if platform.has_file_dialogs() {
                let enabled = controls.load.enabled && !self.pending_load.is_open();
                let size = vec2(width, setup_height);
                if widgets::button(ui, "Load Script", Kind::Outline, enabled, size, &controls.load.reason)
                    .on_hover_text(format!("Opens a script file{}", hotkeys::hint("LoadScript")))
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
        if touch && view.script.is_some() && !has_options {
            hint_line(ui, options_tip);
        }

        // Script log, with a find box: the lines that contain its text (Escape clears it).
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            widgets::section_label(ui, "SCRIPT LOG");
            if !self.log.filter_mut().trim().is_empty() {
                widgets::count_badge(ui, self.log.matches());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                super::logs::log_buttons(ui, &bot.logs.script, &mut self.save, "script-log.txt");
                let width = (ui.available_width() - 4.0).max(60.0);
                // Its count shows before it once it has text: a fixed id keeps the focus.
                let id = egui::Id::new("turu_script_log_find");
                let find = widgets::text_box_with_id(ui, id, self.log.filter_mut(), "Find", width);
                // egui lets go of the focus on Escape before the window is drawn.
                if find.lost_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
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

/// The star before the loaded script's name: a favourite is listed on the side bar's script
/// chip, which starts it with a tap (`turu_bot::quick_scripts`).
fn favorite_star(ui: &mut Ui, bot: &turu_bot::Bot, on: bool) {
    let (color, tip) = if on {
        (
            WARNING,
            "A favourite: the side bar's script list starts it. Click to take it off",
        )
    } else {
        (
            MUTED,
            "Add to favourites: the side bar's script list starts it with a tap",
        )
    };
    let side = widgets::target(22.0, 40.0);
    let star = egui::Button::new(RichText::new("★").size(16.0).color(color))
        .frame(false)
        .min_size(vec2(side, side));
    if ui.add(star).on_hover_text(tip).clicked() {
        model::toggle_favourite(bot);
    }
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

impl From<&model::LastRun> for LastRun {
    fn from(run: &model::LastRun) -> Self {
        Self {
            how: if run.finished { "Finished" } else { "Stopped" },
            ran: duration_text(Duration::from_secs(run.ran_secs)),
            at: run.ended_at.clone(),
        }
    }
}

/// The last run's end while it's what the window shows (`model::ScriptsView::last_run`).
pub(super) fn last_run(bot: &turu_bot::Bot) -> Option<LastRun> {
    model::view(bot).last_run.as_ref().map(LastRun::from)
}

/// Why Force Stop is disabled at first.
pub(super) const FORCE_STOP_WAIT: &str = model::FORCE_STOP_WAIT;

/// VibeSkua's ToggleScript hotkey (`model::toggle`).
pub(super) fn toggle_script() {
    model::toggle(&crate::turu::bot());
}

/// Why a button is disabled, as a line under its group: a touch screen shows no tooltips.
fn hint_line(ui: &mut Ui, text: &str) {
    ui.add(Label::new(RichText::new(text).size(12.0).color(MUTED)).wrap());
}

/// The status in words (`model::state_text`).
pub(super) fn state_text(status: ScriptStatus) -> &'static str {
    model::state_text(status)
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
