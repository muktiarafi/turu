//! Diagnostics > Console: runs a TypeScript snippet with `bot` in scope (VibeSkua's C# snippet
//! console). It runs on a bot thread that counts as a script thread, so Stop interrupts it, and
//! its `bot.log` lines, its result (the last expression's value, as JSON) or its error show
//! below the editor. The run and its output are the tab's (`models::console`).

use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::models::console::{self as model, LineKind};
use egui::{
    Align, Color32, Context, Frame, Key, Label, Layout, Margin, Modifiers, RichText, ScrollArea, Stroke, TextEdit,
    TextWrapMode, Ui, vec2,
};
use std::time::Duration;

fn color(kind: LineKind) -> Color32 {
    match kind {
        LineKind::Run => MUTED,
        LineKind::Log => TEXT,
        LineKind::Value => SUCCESS,
        LineKind::Error => DANGER,
        LineKind::Stopped => WARNING,
    }
}

/// The editor is the window's; the output and the run are the tab's (`models::console`, shared
/// with a native page).
pub struct ConsoleWindow {
    code: String,
}

impl Default for ConsoleWindow {
    fn default() -> Self {
        Self {
            code: model::DEFAULT_CODE.to_owned(),
        }
    }
}

impl ConsoleWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Console", [700.0, 520.0], 4)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        if model::status(&crate::turu::bot()).0 {
            // Log lines and the result arrive from the bot thread.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let (running, can) = model::status(&bot);

        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    widgets::section_label(ui, "TS SNIPPET");
                    ui.label(
                        RichText::new("runs with `bot` in scope; shows the last expression's value")
                            .small()
                            .color(MUTED),
                    );
                });
                if running {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        widgets::pill(ui, |ui| {
                            ui.label(RichText::new("RUNNING").size(9.0).strong().color(SUCCESS));
                        });
                    });
                }
            });
        });

        // The editor takes about half of what is left; the output below fills the rest.
        let editor_height = ((ui.available_height() - 90.0) * 0.5).max(80.0);
        let editor_id = ui.make_persistent_id("console_code");
        let mut run =
            ui.memory(|m| m.has_focus(editor_id)) && ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Enter));
        ScrollArea::vertical()
            .id_salt("console_code_scroll")
            .max_height(editor_height)
            .show(ui, |ui| {
                let mut response = ui.add_sized(
                    [ui.available_width(), editor_height],
                    TextEdit::multiline(&mut self.code)
                        .id(editor_id)
                        .code_editor()
                        .lock_focus(true)
                        .hint_text("bot.player.gold"),
                );
                let kind = super::native_field::Kind::Lines;
                super::native_field::follow(ui, &mut response, &mut self.code, "bot.player.gold", kind);
            });

        let can_run = can.enabled && !self.code.trim().is_empty();
        ui.horizontal(|ui| {
            if widgets::button(ui, "Clear", Kind::Normal, !self.code.is_empty(), vec2(80.0, 28.0), "").clicked() {
                self.code.clear();
            }
            ui.label(RichText::new("Ctrl/Cmd+Enter runs").small().color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if running {
                    if widgets::button(ui, "Stop", Kind::Danger, true, vec2(80.0, 28.0), "").clicked() {
                        model::stop(&bot);
                    }
                } else {
                    let tip = if can.enabled {
                        "Write a snippet first"
                    } else {
                        can.reason.as_str()
                    };
                    run |= widgets::button(ui, "Run", Kind::Primary, can_run, vec2(80.0, 28.0), tip).clicked();
                }
            });
        });
        if run && can_run {
            let _ = model::run(&bot, &self.code);
        }

        let output = model::output(&bot);
        ui.horizontal(|ui| {
            widgets::section_label(ui, "OUTPUT");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::small_button(ui, "Clear", !output.is_empty()).clicked() {
                    model::clear(&bot);
                }
                if widgets::small_button(ui, "Copy", !output.is_empty()).clicked() {
                    ui.ctx().copy_text(model::copy_text(&bot));
                }
            });
        });
        let height = ui.available_height();
        Frame::new()
            .fill(super::theme::INSET)
            .stroke(Stroke::new(1.0, super::theme::EDGE))
            .corner_radius(super::theme::SMALL_RADIUS)
            .inner_margin(Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ScrollArea::both()
                    .id_salt("console_output")
                    .auto_shrink([false, false])
                    .max_height((height - 14.0).max(60.0))
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        if output.is_empty() {
                            ui.label(RichText::new("Run a snippet to see its output here").color(MUTED));
                        }
                        for (entry, text) in &output {
                            ui.add(
                                Label::new(RichText::new(text).monospace().color(color(*entry)))
                                    .wrap_mode(TextWrapMode::Extend)
                                    .selectable(true),
                            );
                        }
                    });
            });
    }
}
