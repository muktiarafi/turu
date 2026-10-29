//! Scripts > Script Loader (VibeSkua's 370x500 window): loaded script and status, the button
//! grid, and the script log.

use super::files::{self, Pending, SaveText};
use super::theme::MUTED;
use super::widgets::{self, Kind};
use crate::gui::FilePicker;
use egui::{Align, Context, Layout, RichText, Ui, vec2};
use rfd::AsyncFileDialog;
use std::path::PathBuf;
use std::time::Duration;
use turu_bot::script::{self, ScriptStatus};

#[derive(Default)]
pub struct ScriptLoader {
    pending_load: Pending,
    save: SaveText,
    log: widgets::LogView,
    vscode: Option<Option<PathBuf>>,
}

impl ScriptLoader {
    pub fn show(&mut self, ctx: &Context, picker: &FilePicker, open: &mut bool) {
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
            .show(ctx, |ui| self.ui(ui, picker));
        // The script writes its log and changes status from its own thread.
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn ui(&mut self, ui: &mut Ui, picker: &FilePicker) {
        let bot = crate::turu::bot();
        let status = bot.scripts.status();
        let loaded = bot.scripts.loaded();
        let active = status.is_active();
        let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
        self.log.refresh(&bot.logs.script);

        ui.spacing_mut().item_spacing = vec2(6.0, 10.0);

        // Header card.
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    widgets::section_label(ui, "LOADED SCRIPT");
                    match &loaded {
                        Some(path) => {
                            let name = path
                                .file_name()
                                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                            ui.label(RichText::new(name).monospace().size(13.0).strong())
                                .on_hover_text(path.display().to_string());
                        }
                        None => {
                            ui.label(
                                RichText::new("No script loaded")
                                    .monospace()
                                    .size(13.0)
                                    .strong()
                                    .color(MUTED),
                            );
                        }
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    widgets::pill(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(RichText::new(status.label()).size(11.0));
                        widgets::dot(ui, widgets::status_color(status), 4.0);
                    });
                });
            });
        });

        // Buttons: two columns with 6 px gaps.
        let half = (ui.available_width() - 6.0) / 2.0;
        let full = ui.available_width();
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            let load = widgets::button(
                ui,
                "Load Script",
                Kind::Normal,
                !active && !self.pending_load.is_open(),
                vec2(half, 34.0),
                "Stop the script first",
            );
            if load.clicked() {
                let start = files::scripts_start_dir(bot.paths.scripts_dir());
                let dialog = AsyncFileDialog::new()
                    .set_title("Load Script")
                    .add_filter("Scripts", &["ts", "js"])
                    .set_directory(start);
                self.pending_load.pick_file(picker, ui.ctx(), dialog);
            }
            let vscode_tip = if code.is_none() {
                "VS Code (the `code` command) wasn't found"
            } else {
                "Load a script first"
            };
            let vscode = widgets::button(
                ui,
                "Open VSCode",
                Kind::Normal,
                code.is_some() && loaded.is_some(),
                vec2(half, 34.0),
                vscode_tip,
            );
            if vscode.clicked()
                && let (Some(code), Some(path)) = (&code, &loaded)
            {
                files::open_in_vscode(code, path);
            }
        });
        ui.horizontal(|ui| {
            let edit = widgets::button(
                ui,
                "Edit Script",
                Kind::Normal,
                loaded.is_some(),
                vec2(half, 30.0),
                "Load a script first",
            );
            if edit.clicked()
                && let Some(path) = &loaded
            {
                files::open_with_default(path);
            }
            widgets::button(
                ui,
                "Search Scripts",
                Kind::Normal,
                false,
                vec2(half, 30.0),
                "Coming soon",
            );
        });
        widgets::button(
            ui,
            "Edit Script Options",
            Kind::Normal,
            false,
            vec2(full, 30.0),
            "Coming soon",
        );
        ui.horizontal(|ui| {
            let auto = super::auto::running();
            if active {
                if widgets::button(
                    ui,
                    "Stop Script",
                    Kind::Danger,
                    status != ScriptStatus::Stopping,
                    vec2(half, 30.0),
                    "",
                )
                .clicked()
                {
                    script::stop(bot);
                }
            } else {
                let tip = if auto {
                    "Stop Auto Attack first"
                } else {
                    "Load a script first"
                };
                if widgets::button(
                    ui,
                    "Start Script",
                    Kind::Primary,
                    loaded.is_some() && !auto,
                    vec2(half, 30.0),
                    tip,
                )
                .clicked()
                    && let Err(e) = script::start(bot)
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
                vec2(half, 30.0),
                "No script is running",
            )
            .clicked()
            {
                script::set_paused(bot, status != ScriptStatus::Paused);
            }
        });

        // Script log.
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            widgets::section_label(ui, "SCRIPT LOG");
            widgets::count_badge(ui, self.log.count());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                super::logs::log_buttons(ui, picker, &bot.logs.script, &mut self.save, "script-log.txt");
            });
        });
        let height = ui.available_height();
        self.log.show(ui, "script_log", height);
    }
}
