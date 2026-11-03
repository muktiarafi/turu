//! Scripts > Script Loader (VibeSkua's 370x500 window): loaded script and status, the run
//! controls, the setup buttons, and the script log.

use super::files::{self, Pending, SaveText};
use super::theme::{DANGER, MUTED, TEXT};
use super::widgets::{self, Kind};
use crate::platform::FileDialog;
use egui::{Align, Context, CursorIcon, Label, Layout, RichText, Sense, Ui, vec2};
use std::path::PathBuf;
use std::time::Duration;
use turu_bot::script::{self, ScriptOptionsState, ScriptStatus};

#[derive(Default)]
pub struct ScriptLoader {
    pending_load: Pending,
    save: SaveText,
    log: widgets::LogView,
    vscode: Option<Option<PathBuf>>,
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

    fn ui(&mut self, ui: &mut Ui, options_open: &mut bool, repo_open: &mut bool) {
        let bot = crate::turu::bot();
        let status = bot.scripts.status();
        let loaded = bot.scripts.loaded();
        let options = bot.scripts.options();
        let active = status.is_active();
        let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
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
                ui.label(RichText::new(state).size(16.0).strong().color(TEXT));
            });
            match &loaded {
                Some(path) => {
                    let name = path
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                    ui.add(Label::new(RichText::new(name).monospace().color(TEXT)).truncate())
                        .on_hover_text(path.display().to_string());
                }
                None => {
                    ui.label(RichText::new("Load Script or Search Scripts to pick one").color(MUTED));
                }
            }
            if let Some(error) = &error {
                let first = error.lines().find(|l| !l.trim().is_empty()).unwrap_or_default().trim();
                let line = ui
                    .add(
                        Label::new(RichText::new(first).size(12.0).color(DANGER))
                            .truncate()
                            .sense(Sense::click()),
                    )
                    .on_hover_text("Show the error in the script log");
                if line.clicked() {
                    self.log.reveal(first);
                }
                if line.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
            }
        });

        // The run controls lead, right under the status: they are what a check-in reaches for.
        // The setup buttons follow, smaller and outlined. 6 px inside a group, 14 px between.
        // On a touch screen every button is 48 points high and the reasons a button is disabled
        // (tooltips elsewhere) show as a line under its group.
        let touch = widgets::touch();
        let full = ui.available_width();
        let half = (full - 6.0) / 2.0;
        let run = ((full - 6.0) * 0.62).floor();
        let run_height = widgets::target(38.0, 48.0);
        let setup_height = widgets::target(28.0, 48.0);
        let auto = super::auto::running(&bot);
        let snippet = bot.scripts.is_snippet_running();
        let mut run_hint = None;
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            if active {
                // While stopping, CoreBots may still be going to the stop location; a second
                // press interrupts that.
                let stopping = status == ScriptStatus::Stopping;
                let label = if stopping { "Force Stop" } else { "Stop Script" };
                let force_tip = "Interrupts the script's stopping steps (such as going to the stop location)";
                if stopping {
                    run_hint = Some(force_tip);
                }
                if widgets::button(ui, label, Kind::Danger, true, vec2(run, run_height), "")
                    .on_hover_text(if stopping { force_tip } else { "" })
                    .clicked()
                {
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
                    vec2(run, run_height),
                    tip,
                )
                .clicked()
                    && let Err(e) = script::start(&bot)
                {
                    bot.log(format!("Couldn't start the script: {e}"));
                }
            }
            let running = matches!(status, ScriptStatus::Running | ScriptStatus::Paused);
            let label = if status == ScriptStatus::Paused {
                "Resume Script"
            } else {
                "Pause Script"
            };
            if widgets::button(
                ui,
                label,
                Kind::Normal,
                running,
                vec2(full - 6.0 - run, run_height),
                "No script is running",
            )
            .clicked()
            {
                script::set_paused(&bot, status != ScriptStatus::Paused);
            }
        });
        if touch && let Some(hint) = run_hint {
            hint_line(ui, hint);
        }

        // What the platform can't do isn't offered: Android has no file dialogs and opens no
        // files in an app, so it keeps Search Scripts and Edit Script Options.
        let platform = crate::platform::platform();
        let vscode_tip = if code.is_none() {
            "VS Code (the `code` command) wasn't found"
        } else {
            "Load a script first"
        };
        let (has_options, options_tip) = match (&loaded, &options) {
            (None, _) => (false, "Load a script first"),
            (Some(_), ScriptOptionsState::Loaded(_)) => (true, ""),
            (Some(_), ScriptOptionsState::Failed(_)) => (false, "The script's options couldn't be read"),
            (Some(_), ScriptOptionsState::NoOptions) => (false, "This script declares no options"),
        };
        let mut setup = Vec::new();
        if platform.has_file_dialogs() {
            let enabled = !active && !self.pending_load.is_open();
            setup.push((Setup::Load, "Load Script", enabled, "Stop the script first"));
        }
        setup.push((Setup::Search, "Search Scripts", true, ""));
        if platform.opens_files() {
            setup.push((Setup::Edit, "Edit Script", loaded.is_some(), "Load a script first"));
            let enabled = code.is_some() && loaded.is_some();
            setup.push((Setup::VsCode, "Open VSCode", enabled, vscode_tip));
        }
        setup.push((Setup::Options, "Edit Script Options", has_options, options_tip));

        ui.add_space(8.0);
        for row in setup.chunks(2) {
            ui.horizontal(|ui| {
                let width = if row.len() == 1 { full } else { half };
                for &(action, label, enabled, tip) in row {
                    if !widgets::button(ui, label, Kind::Outline, enabled, vec2(width, setup_height), tip).clicked() {
                        continue;
                    }
                    match action {
                        Setup::Load => {
                            let start = files::scripts_start_dir(bot.paths.scripts_dir());
                            let dialog = FileDialog::new()
                                .set_title("Load Script")
                                .add_filter("Scripts", &["ts", "js"])
                                .set_directory(start);
                            self.pending_load.pick_file(ui.ctx(), dialog);
                        }
                        Setup::Search => *repo_open = true,
                        Setup::Edit => {
                            if let Some(path) = &loaded {
                                files::open_with_default(path);
                            }
                        }
                        Setup::VsCode => {
                            if let (Some(code), Some(path)) = (&code, &loaded) {
                                files::open_in_vscode(code, path);
                            }
                        }
                        Setup::Options => *options_open = true,
                    }
                }
            });
        }
        if touch && loaded.is_some() && !has_options {
            hint_line(ui, options_tip);
        }

        // Script log.
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            widgets::section_label(ui, "SCRIPT LOG");
            widgets::count_badge(ui, self.log.count());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                super::logs::log_buttons(ui, &bot.logs.script, &mut self.save, "script-log.txt");
            });
        });
        let height = ui.available_height();
        self.log.show(ui, "script_log", height);
    }
}

/// The setup buttons, in their order.
#[derive(Clone, Copy)]
enum Setup {
    Load,
    Search,
    Edit,
    VsCode,
    Options,
}

/// Why a button is disabled, as a line under its group: a touch screen shows no tooltips.
fn hint_line(ui: &mut Ui, text: &str) {
    ui.add(Label::new(RichText::new(text).size(12.0).color(MUTED)).wrap());
}

/// The status in words, for the status card (`ScriptStatus::label` keeps VibeSkua's "[Running]"
/// for the control API and the monitor).
fn state_text(status: ScriptStatus) -> &'static str {
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
fn duration_text(elapsed: Duration) -> String {
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
