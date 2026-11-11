//! The "Update available" window, opened from the nav bar's notice (VibeSkua's Manager asks in
//! a message box; Turu never interrupts: a script may be running). It shows the new version,
//! its release notes link and, on desktops, Download & restart and Update when I quit; on
//! Android, Download APK opens the package's URL for the system to install.

use super::theme::{DANGER, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use egui::{Align, Context, Layout, RichText, Spinner, Ui, vec2};
use std::time::Duration;
use turu_bot::updates::{self, Install, Release};

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
    let state = updates::state();
    let Some(release) = state.available.clone() else {
        *open = false;
        return;
    };
    super::window(ctx, title(), [400.0, 250.0], 2)
        .open(open)
        .resizable(false)
        .show(ctx, |ui| contents(ui, &release, &state.install));
    if matches!(state.install, Install::Busy(_)) {
        ctx.request_repaint_after(Duration::from_millis(200));
    } else {
        // A script that stops enables Download & restart.
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}

fn contents(ui: &mut Ui, release: &Release, install: &Install) {
    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new(format!("{} {}", updates::app_name(), release.version))
                .size(17.0)
                .strong()
                .color(TEXT),
        );
        let channel = if release.prerelease { " · pre-release" } else { "" };
        ui.label(
            RichText::new(format!("You have {}{channel}", crate::turu::VERSION))
                .size(widgets::small_label_size() + 1.0)
                .color(MUTED),
        );
        ui.add_space(2.0);
        if ui.link("Release notes").on_hover_text(&release.html_url).clicked() {
            crate::platform::platform().open_url(&release.html_url);
        }
    });

    let in_place = updates::Target::current().is_some_and(updates::Target::updates_in_place);
    if !in_place {
        link_only(ui, release);
        return;
    }
    if !release.installable() {
        note(
            ui,
            "This release has no package for this computer: download it from the release page.",
            MUTED,
        );
        return;
    }
    let running = crate::turu::any_script_running();
    match install {
        Install::Busy(what) => {
            ui.horizontal(|ui| {
                ui.add(Spinner::new().size(14.0).color(TEXT));
                ui.label(RichText::new(what).color(TEXT));
            });
        }
        Install::OnQuit(version) => {
            note(
                ui,
                &format!("Turu {version} is downloaded and installs when you quit Turu."),
                SUCCESS,
            );
        }
        Install::Failed(error) => note(ui, &format!("The update failed: {error}. Try again."), DANGER),
        Install::Idle if running => note(
            ui,
            "A script is running. Stop it to restart now, or update when you quit Turu.",
            MUTED,
        ),
        Install::Idle => {}
    }
    let busy = matches!(install, Install::Busy(_));
    let staged = matches!(install, Install::OnQuit(_));
    let height = widgets::target(28.0, 40.0);
    ui.add_space(2.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        Layout::right_to_left(Align::Center),
        |ui| {
            let why = if busy {
                "The update is downloading"
            } else {
                "Stop the running script first, or choose Update when I quit"
            };
            if widgets::button(
                ui,
                "Download & restart",
                Kind::Primary,
                !busy && !running,
                vec2(150.0, height),
                why,
            )
            .clicked()
            {
                start(false);
            }
            let why = if staged {
                "Already downloaded: it installs when you quit"
            } else {
                "The update is downloading"
            };
            if widgets::button(
                ui,
                "Update when I quit",
                Kind::Normal,
                !busy && !staged,
                vec2(140.0, height),
                why,
            )
            .clicked()
            {
                start(true);
            }
        },
    );
    ui.label(
        RichText::new("Checked against the release's SHA256SUMS.txt before it replaces Turu.")
            .size(widgets::small_label_size())
            .color(MUTED),
    );
}

/// Android (and platforms without a package): the APK, or the release page.
fn link_only(ui: &mut Ui, release: &Release) {
    let height = widgets::target(28.0, 40.0);
    ui.add_space(2.0);
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        Layout::right_to_left(Align::Center),
        |ui| {
            let (label, url) = match &release.asset {
                Some(asset) if asset.name.ends_with(".apk") => ("Download APK", &asset.url),
                _ => ("Open release page", &release.html_url),
            };
            if widgets::button(ui, label, Kind::Primary, true, vec2(150.0, height), "").clicked() {
                crate::platform::platform().open_url(url);
            }
        },
    );
    if release.asset.as_ref().is_some_and(|a| a.name.ends_with(".apk")) {
        ui.label(
            RichText::new("Android installs it over this app; your settings and scripts stay.")
                .size(widgets::small_label_size())
                .color(MUTED),
        );
    }
}

fn note(ui: &mut Ui, text: &str, color: egui::Color32) {
    ui.label(RichText::new(text).color(color));
}

fn start(on_quit: bool) {
    if let Err(e) = crate::turu::install_update(on_quit) {
        updates::update_state(|s| s.install = Install::Failed(e));
    }
}
