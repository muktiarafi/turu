//! Scripts > Edit Script Options (VibeSkua's "Script Options" dialog): the loaded script's options,
//! each with its description and an editor, then each option group under its own title. The window edits a copy taken when it opens. Save
//! writes the values to `options/<Storage>.cfg`; Cancel (or closing the window) drops them.
//! It also opens by itself when the script asks (before a run, or `bot.config.configure`); the
//! script waits until it closes.

use super::theme::{DANGER, MUTED, TEXT};
use super::widgets::{self, Kind as ButtonKind};
use egui::{Align, ComboBox, Context, DragValue, Layout, RichText, ScrollArea, TextEdit, Ui, vec2};
use serde_json::{Value, json};
use turu_bot::script::ScriptOptionsState;
use turu_bot::script::options::{Kind, MAIN, OptionDecl, ScriptOptions};

#[derive(Default)]
pub struct ScriptOptionsWindow {
    /// The values being edited. Taken from the bot when the window opens.
    draft: Option<ScriptOptions>,
    /// Why the last Save failed.
    error: Option<String>,
    /// The script's request (`bot.config.configure`, or before a run) the window is open for.
    request: Option<u64>,
    /// The draft is a Scheduler entry's options (VibeSkua `EditScriptConfig`): Save writes its
    /// own file and leaves the loaded script alone.
    queued: bool,
}

impl ScriptOptionsWindow {
    /// Edits a Scheduler entry's options, read from its `Queue_<name>_<id>` file.
    pub fn edit_queued(&mut self, options: ScriptOptions) {
        self.draft = Some(options);
        self.queued = true;
        self.error = None;
    }

    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        let scripts = &crate::turu::bot().scripts;
        let pending = scripts.configure_request();
        if pending.is_some() && pending != self.request {
            // The script waits for the window (VibeSkua's options dialog on Start).
            self.request = pending;
            if self.queued {
                self.queued = false;
                self.draft = None;
            }
            *open = true;
        } else if self.request.is_some() && pending != self.request {
            // The script was stopped while it waited.
            self.request = None;
            *open = false;
        }
        if !*open {
            if let Some(request) = self.request.take() {
                scripts.finish_configure(request);
            }
            self.draft = None;
            self.error = None;
            self.queued = false;
            return;
        }
        if self.draft.is_none()
            && let ScriptOptionsState::Loaded(options) = crate::turu::bot().scripts.options()
        {
            self.draft = Some(options);
        }
        let mut done = false;
        super::window(ctx, "Script Options", [560.0, 460.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &mut done));
        if done {
            *open = false;
        }
    }

    fn ui(&mut self, ui: &mut Ui, done: &mut bool) {
        let bot = crate::turu::bot();
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);

        let Some(draft) = self.draft.as_mut() else {
            // Nothing to edit: no script is loaded, or the loaded one declares no options.
            let message = match bot.scripts.options() {
                ScriptOptionsState::Failed(e) => format!("The script's options couldn't be read: {e}"),
                _ => "The loaded script declares no options.".into(),
            };
            ui.label(RichText::new(message).color(MUTED));
            if widgets::button(ui, "Close", ButtonKind::Normal, true, vec2(80.0, 28.0), "").clicked() {
                *done = true;
            }
            return;
        };

        let saved_in = draft
            .path()
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        ui.label(
            RichText::new(format!("Saved in options/{saved_in}"))
                .small()
                .color(MUTED),
        )
        .on_hover_text(draft.path().display().to_string());

        let categories: Vec<(String, Vec<OptionDecl>)> = draft
            .declaration()
            .categories()
            .filter(|(_, options)| !options.is_empty())
            .map(|(category, options)| (category.to_owned(), options.to_vec()))
            .collect();
        let footer = 44.0;
        let list_height = (ui.available_height() - footer).max(80.0);
        let mut edits: Vec<(String, String, Value)> = Vec::new();
        ScrollArea::vertical()
            .id_salt("script_options_list")
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (category, options) in &categories {
                    if category != MAIN {
                        // VibeSkua titles a group with its field name, `_` as spaces.
                        ui.label(RichText::new(category.replace('_', " ")).color(TEXT).strong());
                    }
                    widgets::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for option in options {
                            let value = draft.get_in(category, &option.name).cloned().unwrap_or(Value::Null);
                            ui.horizontal(|ui| {
                                ui.set_min_height(26.0);
                                let text_width = (ui.available_width() - 170.0).max(120.0);
                                ui.allocate_ui_with_layout(vec2(text_width, 0.0), Layout::top_down(Align::Min), |ui| {
                                    let title = ui.label(RichText::new(&option.display_name).color(TEXT).strong());
                                    title.on_hover_text(format!("Default: {}", default_text(&option.default_value)));
                                    if !option.description.is_empty() {
                                        ui.label(RichText::new(&option.description).small().color(MUTED));
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if let Some(value) = editor(ui, category, option, &value) {
                                        edits.push((category.clone(), option.name.clone(), value));
                                    }
                                });
                            });
                        }
                    });
                }
            });

        for (category, name, value) in edits {
            if let Err(e) = draft.set_in(&category, &name, value) {
                self.error = Some(e);
            }
        }

        if let Some(error) = &self.error {
            ui.label(RichText::new(error).small().color(DANGER));
        }
        ui.horizontal(|ui| {
            if widgets::button(ui, "Reset Defaults", ButtonKind::Normal, true, vec2(120.0, 28.0), "").clicked() {
                draft.reset();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Save", ButtonKind::Primary, true, vec2(80.0, 28.0), "").clicked() {
                    let saved = if self.queued {
                        draft
                            .save()
                            .map_err(|e| format!("couldn't save {}: {e}", draft.path().display()))
                    } else {
                        bot.scripts.save_options(draft.clone())
                    };
                    match saved {
                        Ok(()) => *done = true,
                        Err(e) => self.error = Some(e),
                    }
                }
                if widgets::button(ui, "Cancel", ButtonKind::Normal, true, vec2(80.0, 28.0), "").clicked() {
                    *done = true;
                }
            });
        });
    }
}

/// One option's editor: a switch, a number box, a text box or a drop-down. Returns the new value
/// when it changed.
fn editor(ui: &mut Ui, category: &str, option: &OptionDecl, value: &Value) -> Option<Value> {
    match option.kind {
        Kind::Bool => {
            let mut on = value.as_bool().unwrap_or(false);
            widgets::switch(ui, &mut on).changed().then_some(Value::Bool(on))
        }
        Kind::Int => {
            let mut number = value.as_i64().unwrap_or(0);
            ui.add(DragValue::new(&mut number)).changed().then(|| json!(number))
        }
        Kind::Text => {
            let mut text = value.as_str().unwrap_or_default().to_owned();
            let input = TextEdit::singleline(&mut text)
                .desired_width(150.0)
                .margin(egui::Margin::symmetric(8, 4));
            ui.add(input).changed().then_some(Value::String(text))
        }
        Kind::Choice => {
            // VibeSkua shows an enum's members with `_` as spaces.
            let current = value.as_str().unwrap_or_default();
            let mut picked = None;
            ComboBox::from_id_salt(("script_option", category, &option.name))
                .width(150.0)
                .selected_text(current.replace('_', " "))
                .show_ui(ui, |ui| {
                    for choice in &option.choices {
                        if ui
                            .selectable_label(choice == current, choice.replace('_', " "))
                            .clicked()
                        {
                            picked = Some(Value::String(choice.clone()));
                        }
                    }
                });
            picked
        }
    }
}

/// A default as the window's tooltip shows it (VibeSkua: "Default: True").
fn default_text(value: &Value) -> String {
    match value {
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
