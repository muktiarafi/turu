//! The Tab Limit's notice (Turu on Android, Turu Mas: `Platform::limits_tabs`): a new tab asked
//! for with the tab strip's "+", Army > New Tab or Accounts' "In a tab" while as many tabs are
//! open as Options > Application > Tab Limit allows isn't opened; this box says why and offers
//! the setting (or, at the most Turu runs, to close a tab first).

use super::widgets::{self, Kind};
use crate::turu::{self, TabRequest};
use egui::{Align, Context, Layout, Modal, vec2};
use std::sync::atomic::{AtomicBool, Ordering};
use turu_bot::startup::Startup;

/// The most tabs Options > Application > Tab Limit allows.
const MAX: i32 = 4;

/// A tab was refused and the box isn't answered yet.
static REFUSED: AtomicBool = AtomicBool::new(false);

/// The Tab Limit, where the platform limits tabs.
fn limit() -> Option<usize> {
    crate::platform::platform()
        .limits_tabs()
        .then(|| turu::bot().settings.read().tab_limit.clamp(1, MAX) as usize)
}

/// As many tabs are open as the Tab Limit allows.
pub fn reached() -> bool {
    limit().is_some_and(|limit| turu::tabs().len() >= limit)
}

/// Asks for a new tab, unless the Tab Limit is reached: then the notice shows and it's false.
/// Any thread.
pub fn open_tab(startup: Startup) -> bool {
    if reached() {
        REFUSED.store(true, Ordering::Relaxed);
        return false;
    }
    turu::request_tab(TabRequest::Open(Box::new(startup)))
}

/// Shows the notice while a refused tab waits for an answer. True when the user picked "Raise
/// Tab Limit": the caller opens Options > Application at the setting.
pub fn show(ctx: &Context) -> bool {
    if !REFUSED.load(Ordering::Relaxed) {
        return false;
    }
    let Some(limit) = limit() else {
        REFUSED.store(false, Ordering::Relaxed);
        return false;
    };
    let at_most = limit as i32 >= MAX;
    let mut raise = false;
    let mut done = false;
    let size = vec2(0.0, widgets::target(28.0, 48.0));
    let response = Modal::new(egui::Id::new("turu_tab_limit")).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.set_max_width(420.0);
        ui.heading("Tab limit reached");
        ui.label(if at_most {
            format!("{limit} tabs are open, the most Turu runs at once. Close a tab to open another.")
        } else {
            format!(
                "{limit} {} open, as many as your Tab Limit allows. Raise it in Options > Application, \
                 or close a tab first.",
                if limit == 1 { "tab is" } else { "tabs are" }
            )
        });
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
        REFUSED.store(false, Ordering::Relaxed);
    }
    raise
}
