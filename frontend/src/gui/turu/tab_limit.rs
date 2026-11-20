//! The Tab Limit's notice (Turu on Android, Turu Mas: `Platform::limits_tabs`): a new tab asked
//! for with the tab strip's "+", Army > New Tab or Accounts' "In a tab" while as many tabs are
//! open as Options > Application > Tab Limit allows isn't opened; this box says why and offers
//! the setting (or, at the most Turu runs, to close a tab first). The state is `models::prompts`',
//! which a native shell shows instead.

use super::widgets::{self, Kind};
use crate::models::prompts;
use egui::{Align, Context, Layout, Modal, vec2};
use turu_bot::startup::Startup;

/// As many tabs are open as the Tab Limit allows.
pub fn reached() -> bool {
    prompts::tab_limit_reached()
}

/// Asks for a new tab, unless the Tab Limit is reached: then the notice shows and it's false.
/// Any thread.
pub fn open_tab(startup: Startup) -> bool {
    prompts::open_tab(startup)
}

/// Shows the notice while a refused tab waits for an answer. True when the user picked "Raise
/// Tab Limit": the caller opens Options > Application at the setting.
pub fn show(ctx: &Context) -> bool {
    if prompts::native_shown() {
        return false;
    }
    let Some(limit) = prompts::tab_refused() else {
        return false;
    };
    let (text, at_most) = prompts::tab_limit_text(limit);
    let mut raise = false;
    let mut done = false;
    let size = vec2(0.0, widgets::target(28.0, 48.0));
    let response = Modal::new(egui::Id::new("turu_tab_limit")).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.set_max_width(420.0);
        ui.heading("Tab limit reached");
        ui.label(text);
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if at_most {
                done |= widgets::button(ui, "OK", Kind::Primary, true, size.max(vec2(72.0, 0.0)), "").clicked();
            } else {
                raise = widgets::button(ui, "Raise Tab Limit", Kind::Primary, true, size, "").clicked();
                done |= widgets::button(ui, "Not now", Kind::Normal, true, size.max(vec2(72.0, 0.0)), "").clicked();
            }
        });
    });
    if raise || done || response.should_close() {
        prompts::answer_tab_refused();
    }
    raise
}
