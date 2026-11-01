//! Diagnostics > Console: runs a TypeScript snippet with `bot` in scope (VibeSkua's C# snippet
//! console). It runs on a bot thread that counts as a script thread, so Stop interrupts it, and
//! its `bot.log` lines, its result (the last expression's value, as JSON) or its error show
//! below the editor.

use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{
    Align, Color32, Context, Frame, Key, Label, Layout, Margin, Modifiers, RichText, ScrollArea, Stroke, TextEdit,
    TextWrapMode, Ui, vec2,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use turu_bot::script::{self, RunError};

/// VibeSkua's default snippet, in TypeScript.
const DEFAULT_CODE: &str = "bot.log(\"Test\");";

/// Most output lines kept; older ones are dropped (a snippet may log in a loop).
const MAX_OUTPUT: usize = 1000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// "› code": a run started.
    Run,
    Log,
    Value,
    Error,
    Stopped,
}

impl Entry {
    fn color(self) -> Color32 {
        match self {
            Self::Run => MUTED,
            Self::Log => TEXT,
            Self::Value => SUCCESS,
            Self::Error => DANGER,
            Self::Stopped => WARNING,
        }
    }
}

type Output = Arc<Mutex<Vec<(Entry, String)>>>;

fn push(output: &Output, entry: Entry, text: impl Into<String>) {
    let mut output = output.lock().unwrap_or_else(|e| e.into_inner());
    if output.len() >= MAX_OUTPUT {
        output.remove(0);
    }
    output.push((entry, text.into()));
}

pub struct ConsoleWindow {
    code: String,
    output: Output,
    /// From Run until the bot thread is done (the engine's flag is set a moment later).
    running: Arc<AtomicBool>,
}

impl Default for ConsoleWindow {
    fn default() -> Self {
        Self {
            code: DEFAULT_CODE.to_owned(),
            output: Output::default(),
            running: Arc::default(),
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
        if self.running.load(Ordering::Acquire) {
            // Log lines and the result arrive from the bot thread.
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        let running = self.running.load(Ordering::Acquire) || bot.scripts.is_snippet_running();
        let script_active = bot.scripts.status().is_active();
        let auto = super::auto::running(&bot);

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
                ui.add_sized(
                    [ui.available_width(), editor_height],
                    TextEdit::multiline(&mut self.code)
                        .id(editor_id)
                        .code_editor()
                        .lock_focus(true)
                        .hint_text("bot.player.gold"),
                );
            });

        let can_run = !running && !script_active && !auto && !self.code.trim().is_empty();
        ui.horizontal(|ui| {
            if widgets::button(ui, "Clear", Kind::Normal, !self.code.is_empty(), vec2(80.0, 28.0), "").clicked() {
                self.code.clear();
            }
            ui.label(RichText::new("Ctrl/Cmd+Enter runs").small().color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if running {
                    if widgets::button(ui, "Stop", Kind::Danger, true, vec2(80.0, 28.0), "").clicked() {
                        script::stop_snippet(&bot);
                    }
                } else {
                    let tip = if script_active {
                        "Stop the script first"
                    } else if auto {
                        "Stop Auto Attack/Hunt first"
                    } else {
                        "Write a snippet first"
                    };
                    run |= widgets::button(ui, "Run", Kind::Primary, can_run, vec2(80.0, 28.0), tip).clicked();
                }
            });
        });
        if run && can_run {
            self.run();
        }

        let output = self.output.lock().unwrap_or_else(|e| e.into_inner()).clone();
        ui.horizontal(|ui| {
            widgets::section_label(ui, "OUTPUT");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::small_button(ui, "Clear", !output.is_empty()).clicked() {
                    self.output.lock().unwrap_or_else(|e| e.into_inner()).clear();
                }
                if widgets::small_button(ui, "Copy", !output.is_empty()).clicked() {
                    let text: Vec<&str> = output.iter().map(|(_, text)| text.as_str()).collect();
                    ui.ctx().copy_text(text.join("\n"));
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
                                Label::new(RichText::new(text).monospace().color(entry.color()))
                                    .wrap_mode(TextWrapMode::Extend)
                                    .selectable(true),
                            );
                        }
                    });
            });
    }

    /// Runs the snippet on a bot thread of the active tab.
    fn run(&mut self) {
        let code = self.code.clone();
        let first = code.trim().lines().next().unwrap_or_default();
        let more = if code.trim().lines().nth(1).is_some() {
            " …"
        } else {
            ""
        };
        push(&self.output, Entry::Run, format!("› {first}{more}"));
        self.running.store(true, Ordering::Release);
        let (output, running) = (self.output.clone(), self.running.clone());
        crate::turu::spawn_action("Console", move |bot| {
            let sink = output.clone();
            let result = script::run_snippet(bot, &code, move |line| push(&sink, Entry::Log, line));
            match result {
                Ok(value) => push(&output, Entry::Value, value),
                Err(RunError::Stopped) => push(&output, Entry::Stopped, "Stopped"),
                Err(RunError::Failed(error)) => push(&output, Entry::Error, error),
            }
            running.store(false, Ordering::Release);
            Ok(())
        });
    }
}
