//! What the engine asks of the user over the game, one at a time (docs/NATIVE_UI_PROPOSAL.md,
//! L3): the dialog `models::prompts` says is waiting for the tab on screen, answered by its ID
//! there. A script's message box (`bot.showMessageBox`, VibeSkua `ShowMessageBox`), Butler's "a
//! script started while following", the crash notice, "Send crash reports?", the Tab Limit's
//! notice and the graphics tips all come through here, in the model's order, as a native shell
//! shows them; while a shell reads the prompts (`prompts::native_shown`), egui leaves them to it.
//! Closing one without a button (Escape, Back, a click outside) answers "closed", which each
//! prompt reads its own way (a script's box: closed; Butler: keep the follow suspended).

use super::widgets::{self, Kind};
use crate::models::prompts::{self, PromptAnswer, PromptView};
use egui::{Align, Color32, Context, Frame, Layout, Margin, Modal, RichText, ScrollArea, TextStyle, Ui, vec2};

/// What an answer asks the windows to open.
pub enum Opens {
    /// Logs > Crashes, on this report (by its file name).
    CrashReport(String),
    /// Options > Application, at Tab Limit.
    TabLimit,
}

/// Shows the prompt waiting for the tab on screen, if any; what the answer opens.
pub fn show(ctx: &Context) -> Option<Opens> {
    if prompts::native_shown() {
        return None;
    }
    let bot = crate::turu::bot();
    let prompt = prompts::current(&bot)?;
    // Turu Mas asks "Send crash reports?" with Android's own dialog, once a run; it answers
    // through `answer_crash_upload_question`.
    if prompt.id == "uploads" && crate::host::host().ask_crash_uploads(&prompts::crash_upload_question()) {
        prompts::answer_crash_upload_question(None);
        return None;
    }
    let (clicked, closed) = dialog(ctx, &prompt);
    if clicked.is_none() && !closed {
        return None;
    }
    let answer: PromptAnswer = prompts::answer_prompt(&bot, &prompt.id, clicked).ok()?;
    match answer.open.as_str() {
        "crashes" if !answer.report.is_empty() => Some(Opens::CrashReport(answer.report)),
        "app_options" => Some(Opens::TabLimit),
        _ => None,
    }
}

/// The dialog, for a desktop window and a phone alike: as wide as the text reads well (420
/// points) but never wider than the screen less a margin, the text scrolling when it's long or
/// the screen short, and the buttons (the last is the main one) side by side on the right, or
/// stacked full width with the main one on top when they don't fit. Returns the button clicked,
/// and whether the dialog was closed without one.
fn dialog(ctx: &Context, prompt: &PromptView) -> (Option<usize>, bool) {
    let screen = ctx.content_rect();
    let touch = widgets::touch();
    let margin: f32 = if touch { 24.0 } else { 20.0 };
    // Off the screen's edges by at least this much.
    let edge = 16.0;
    let width = (screen.width() - 2.0 * (edge + margin)).clamp(200.0, 420.0);
    let button_height = widgets::target(28.0, 44.0);
    let frame = Frame::popup(&ctx.global_style()).inner_margin(Margin::same(margin as i8));
    let mut clicked = None;
    let response = Modal::new(egui::Id::new(("turu_prompt", &prompt.id)))
        .frame(frame)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.label(RichText::new(&prompt.title).heading().strong());
            ui.add_space(10.0);
            // What the screen leaves for the text once the heading and the buttons (stacked, at
            // worst) have their room.
            let stacked = button_height * prompt.buttons.len() as f32;
            let chrome = 2.0 * (edge + margin) + 40.0 + stacked + 32.0;
            let room = (screen.height() - chrome).max(64.0);
            ScrollArea::vertical()
                .max_height(if prompt.long { room.min(320.0) } else { room })
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.label(&prompt.text);
                    if !prompt.note.is_empty() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(&prompt.note)
                                .size(widgets::small_label_size() + 1.0)
                                .color(super::theme::MUTED),
                        );
                    }
                    // The AIR SDK's license (section 6.4) wants the crash policy linked in the
                    // app: Turu Mas.
                    if !prompt.link.is_empty() {
                        ui.add_space(6.0);
                        if ui.link(&prompt.link).clicked() {
                            crate::host::host().open_url(&prompt.link_url);
                        }
                    }
                });
            ui.add_space(if touch { 20.0 } else { 16.0 });
            clicked = buttons(ui, &prompt.buttons, width, button_height);
        });
    (clicked, clicked.is_none() && response.should_close())
}

/// The buttons, the last the main one: side by side on the right when they fit, else stacked
/// full width with the main one on top.
fn buttons(ui: &mut Ui, labels: &[String], width: f32, height: f32) -> Option<usize> {
    let touch = widgets::touch();
    let spacing = ui.spacing().item_spacing.x;
    let font = TextStyle::Button.resolve(ui.style());
    let padding = 2.0 * ui.spacing().button_padding.x.max(if touch { 16.0 } else { 12.0 });
    let min_width = if touch { 96.0 } else { 72.0 };
    let widths: Vec<f32> = labels
        .iter()
        .map(|label| {
            let text = ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), font.clone(), Color32::WHITE).size().x);
            (text + padding).max(min_width)
        })
        .collect();
    let main = labels.len().saturating_sub(1);
    let kind = |i: usize| if i == main { Kind::Primary } else { Kind::Normal };
    let mut clicked = None;
    let side_by_side = widths.iter().sum::<f32>() + spacing * widths.len().saturating_sub(1) as f32 <= width;
    if side_by_side {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for i in (0..labels.len()).rev() {
                if widgets::button(ui, &labels[i], kind(i), true, vec2(widths[i], height), "").clicked() {
                    clicked = Some(i);
                }
            }
        });
    } else {
        for i in (0..labels.len()).rev() {
            if widgets::button(ui, &labels[i], kind(i), true, vec2(width, height), "").clicked() {
                clicked = Some(i);
            }
        }
    }
    clicked
}
