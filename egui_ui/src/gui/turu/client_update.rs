//! The "Update available" window, opened from the nav bar's notice (VibeSkua's Manager asks in
//! a message box; Turu never interrupts: a script may be running). It shows the new version,
//! its release notes link and, on desktops, Download & restart and Update when I quit; on
//! Android, Download & install hands the checked APK to Android's installer. What it says and
//! allows is `models::client_update`'s, which Android's native page reads too.

use super::theme::{DANGER, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use crate::models::client_update::{self, ClientUpdateView, InstallWay, UpdateStep};
use egui::{Align, Context, Layout, RichText, Spinner, Ui, vec2};
use std::time::Duration;
use turu_bot::updates;

/// The window's title (and egui id): "Update Turu", or "Update Turu Mas".
pub fn title() -> &'static str {
    if updates::app_name() == "Turu Mas" {
        "Update Turu Mas"
    } else {
        "Update Turu"
    }
}

pub fn show(ctx: &Context, open: &mut bool) {
    if !*open {
        return;
    }
    let view = client_update::view();
    if view.release.is_none() {
        *open = false;
        return;
    }
    super::window(ctx, title(), [400.0, 250.0], 2)
        .open(open)
        .resizable(false)
        .show(ctx, |ui| contents(ui, &view));
    if view.status.as_ref().is_some_and(|s| s.step == UpdateStep::Working) {
        ctx.request_repaint_after(Duration::from_millis(200));
    } else {
        // A script that stops enables Download & restart.
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn contents(ui: &mut Ui, view: &ClientUpdateView) {
    let Some(release) = &view.release else {
        return;
    };
    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(format!("{} {}", view.app, release.version))
                .size(17.0)
                .strong()
                .color(TEXT),
        );
        let channel = if release.prerelease { " · pre-release" } else { "" };
        ui.label(
            RichText::new(format!("You have {}{channel}", view.current))
                .size(widgets::small_label_size() + 1.0)
                .color(MUTED),
        );
        ui.add_space(2.0);
        if ui.link("Release notes").on_hover_text(&release.notes_url).clicked() {
            crate::host::host().open_url(&release.notes_url);
        }
    });

    match view.way {
        InstallWay::NoPackage => {
            note(ui, &view.footnote, MUTED);
            return;
        }
        InstallWay::Link => {
            link_only(ui, view);
            return;
        }
        InstallWay::Package | InstallWay::InPlace => {}
    }
    if let Some(status) = &view.status {
        match status.step {
            UpdateStep::Working => {
                ui.horizontal(|ui| {
                    ui.add(Spinner::new().size(14.0).color(TEXT));
                    ui.label(RichText::new(&status.text).color(TEXT));
                });
            }
            UpdateStep::Waiting => note(ui, &status.text, SUCCESS),
            UpdateStep::Failed => note(ui, &status.text, DANGER),
            UpdateStep::Held => note(ui, &status.text, MUTED),
        }
    }
    let height = widgets::target(28.0, 40.0);
    ui.add_space(2.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        Layout::right_to_left(Align::Center),
        |ui| {
            let width = if view.way == InstallWay::Package { 160.0 } else { 150.0 };
            if widgets::button(
                ui,
                &view.install_label,
                Kind::Primary,
                view.install.enabled,
                vec2(width, height),
                &view.install.reason,
            )
            .clicked()
            {
                start(false);
            }
            if view.way == InstallWay::InPlace
                && widgets::button(
                    ui,
                    "Update when I quit",
                    Kind::Normal,
                    view.on_quit.enabled,
                    vec2(140.0, height),
                    &view.on_quit.reason,
                )
                .clicked()
            {
                start(true);
            }
        },
    );
    ui.label(
        RichText::new(&view.footnote)
            .size(widgets::small_label_size())
            .color(MUTED),
    );
}

/// Platforms without a package, or an APK without its sums: the APK, or the release page.
fn link_only(ui: &mut Ui, view: &ClientUpdateView) {
    let height = widgets::target(28.0, 40.0);
    ui.add_space(2.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        Layout::right_to_left(Align::Center),
        |ui| {
            if widgets::button(ui, &view.link_label, Kind::Primary, true, vec2(150.0, height), "").clicked() {
                crate::host::host().open_url(&view.link_url);
            }
        },
    );
    if !view.footnote.is_empty() {
        ui.label(
            RichText::new(&view.footnote)
                .size(widgets::small_label_size())
                .color(MUTED),
        );
    }
}

fn note(ui: &mut Ui, text: &str, color: egui::Color32) {
    ui.label(RichText::new(text).color(color));
}

fn start(on_quit: bool) {
    // A failure to start shows as the install's state (the model sets it).
    if let Err(e) = client_update::install(on_quit) {
        tracing::warn!("The update didn't start: {e}");
    }
}
