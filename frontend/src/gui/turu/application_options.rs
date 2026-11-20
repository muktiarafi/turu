//! Options > Application (VibeSkua's 580x650 `ApplicationOptionsView`): a searchable list of
//! Turu's own settings in five sections. Changes are saved at once: `settings.json`
//! (`AppSettings`), and for Relogin Try Delay and Relogin Watchdog, which are bot options,
//! just those two fields of `options.json`. Frame Interpolation is Mugetsu's, saved by the
//! frontend (`Platform::set_frame_interpolation`), as is Safe Drawing where it's offered. Web Monitor replaces VibeSkua's Skua Nexus webhook.

use super::native_field;
use super::theme::{DANGER, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use egui::{RichText, ScrollArea, Ui, vec2};
use turu_bot::settings::AppSettings;
use turu_bot::webhooks::Notice;

use crate::models::application_options::{Entry, Field, Section, entries, save_relogin_options};

pub struct ApplicationOptionsWindow {
    entries: Vec<Entry>,
    search: String,
    filter: Option<Section>,
    /// "Clear" was clicked once; the next click deletes.
    confirm_clear: bool,
    /// The outcome of Clear Flash Cache, and whether it worked.
    cleared: Option<(String, bool)>,
    /// "Regenerate" was clicked once; the next click replaces the key.
    confirm_new_key: bool,
    /// The outcome of the last monitor button (Copy, Regenerate), and whether it worked.
    monitor_message: Option<(String, bool)>,
    /// "Regenerate" was clicked once for the push key.
    confirm_new_push_key: bool,
    show_qr: bool,
    /// The QR code drawn, and the link it holds.
    qr: Option<(String, egui::TextureHandle)>,
}

impl Default for ApplicationOptionsWindow {
    fn default() -> Self {
        Self {
            entries: entries(),
            search: String::new(),
            filter: None,
            confirm_clear: false,
            cleared: None,
            confirm_new_key: false,
            monitor_message: None,
            confirm_new_push_key: false,
            show_qr: false,
            qr: None,
        }
    }
}

impl ApplicationOptionsWindow {
    /// Shows the options matching `search` (the Tab Limit notice's "Raise Tab Limit").
    pub fn find(&mut self, search: &str) {
        self.search = search.to_owned();
        self.filter = None;
    }

    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        if !*open {
            self.confirm_clear = false;
            self.confirm_new_key = false;
            self.confirm_new_push_key = false;
            return;
        }
        super::window(ctx, "Application Options", [580.0, 650.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        // Out of `self` while the rows borrow it (Clear Flash Cache needs `&mut self`).
        let entries = std::mem::take(&mut self.entries);

        ui.spacing_mut().item_spacing = vec2(6.0, 10.0);
        let width = ui.available_width();
        widgets::text_box(ui, &mut self.search, "Search application options", width);
        let needle = self.search.trim().to_lowercase();
        let matches = |e: &Entry| {
            needle.is_empty()
                || e.label.to_lowercase().contains(&needle)
                || e.description.to_lowercase().contains(&needle)
        };
        ui.horizontal_wrapped(|ui| {
            if widgets::chip(ui, &format!("All  {}", entries.len()), self.filter.is_none()) {
                self.filter = None;
            }
            for section in Section::ALL {
                let count = entries.iter().filter(|e| e.section == section).count();
                let label = format!("{}  {count}", section.chip());
                if widgets::chip(ui, &label, self.filter == Some(section)) {
                    self.filter = Some(section);
                }
            }
        });

        let mut settings = bot.settings.read().clone();
        let mut options = bot.options.read().clone();
        let mut settings_changed = false;
        let mut options_changed = false;
        ScrollArea::vertical()
            .id_salt("application_options_list")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for section in Section::ALL {
                    if self.filter.is_some_and(|f| f != section) {
                        continue;
                    }
                    let rows: Vec<usize> = (0..entries.len())
                        .filter(|&i| entries[i].section == section && matches(&entries[i]))
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    widgets::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if self.filter.is_none() {
                            ui.horizontal(|ui| {
                                widgets::section_label(ui, &section.title().to_uppercase());
                                widgets::count_badge(ui, rows.len());
                            });
                            ui.add_space(2.0);
                        }
                        for i in rows {
                            let entry = &entries[i];
                            let size = vec2(96.0, widgets::target(24.0, 40.0));
                            let title = |ui: &mut Ui| {
                                ui.label(RichText::new(entry.label).color(TEXT));
                                if entry.restart {
                                    ui.label(RichText::new("Restart required").small().color(MUTED));
                                }
                            };
                            widgets::setting_row(ui, title, entry.description, control_width(&entry.field), |ui| {
                                match &entry.field {
                                    Field::Bool(access) => {
                                        settings_changed |= widgets::switch(ui, access(&mut settings)).changed();
                                    }
                                    Field::Unused(access) => {
                                        ui.add_enabled_ui(false, |ui| widgets::switch(ui, access(&mut settings)))
                                            .inner
                                            .on_disabled_hover_text("Coming soon: Turu doesn't update these yet");
                                    }
                                    Field::ClientUpdates(access) => {
                                        let nightly = turu_bot::updates::this_is_nightly();
                                        settings_changed |= ui
                                            .add_enabled_ui(!nightly, |ui| widgets::switch(ui, access(&mut settings)))
                                            .inner
                                            .on_disabled_hover_text(turu_bot::updates::NIGHTLY_NO_UPDATES)
                                            .changed();
                                    }
                                    Field::Int(access, suffix, min, max) => {
                                        let suffix = format!(" {suffix}");
                                        let value = access(&mut settings);
                                        settings_changed |=
                                            widgets::number(ui, entry.label, value, *min..=*max, 1.0, &suffix, size)
                                                .changed();
                                    }
                                    Field::Text(access) => {
                                        let text = access(&mut settings);
                                        let edit = egui::TextEdit::singleline(&mut *text)
                                            .desired_width(220.0)
                                            .margin(egui::Margin::symmetric(8, 4));
                                        let mut response = ui.add(edit);
                                        native_field::follow(ui, &mut response, text, "", native_field::Kind::Line);
                                        settings_changed |= response.changed();
                                    }
                                    Field::OptionBool(access) => {
                                        options_changed |= widgets::switch(ui, access(&mut options)).changed();
                                    }
                                    Field::OptionInt(access, suffix) => {
                                        let value = access(&mut options);
                                        let speed = (value.abs() as f64 / 200.0).max(1.0);
                                        let suffix = format!(" {suffix}");
                                        options_changed |=
                                            widgets::number(ui, entry.label, value, 0..=i32::MAX, speed, &suffix, size)
                                                .changed();
                                    }
                                    Field::Test => {
                                        let enabled = !settings.discord_webhook_url.trim().is_empty();
                                        if widgets::button(
                                            ui,
                                            "Test",
                                            Kind::Normal,
                                            enabled,
                                            vec2(80.0, 24.0),
                                            "Set the URL first",
                                        )
                                        .clicked()
                                        {
                                            bot.webhooks.send(Notice::Test);
                                        }
                                    }
                                    Field::ClearFlashCache => {
                                        self.clear_flash_cache(ui);
                                    }
                                    Field::Secret(access) => {
                                        let text = access(&mut settings);
                                        let edit = egui::TextEdit::singleline(&mut *text)
                                            .password(true)
                                            .desired_width(220.0)
                                            .margin(egui::Margin::symmetric(8, 4));
                                        let mut response = ui.add(edit);
                                        let kind = native_field::Kind::Password;
                                        native_field::follow(ui, &mut response, text, "", kind);
                                        settings_changed |= response.changed();
                                    }
                                    Field::MonitorLinks => self.monitor_links(ui, &settings),
                                    Field::PhoneQr => {
                                        let label = if self.show_qr { "Hide" } else { "Show" };
                                        if widgets::button(ui, label, Kind::Normal, true, vec2(80.0, 24.0), "")
                                            .clicked()
                                        {
                                            self.show_qr = !self.show_qr;
                                        }
                                    }
                                    Field::RegenerateMonitorKey => self.regenerate_key(ui),
                                    Field::PushKey => self.push_key(ui, &settings),
                                    Field::CheckClientUpdates => {
                                        let checking = turu_bot::updates::state().checking;
                                        let from_shell = crate::turu::updates_from_shell();
                                        let nightly = turu_bot::updates::this_is_nightly();
                                        let why = if nightly {
                                            turu_bot::updates::NIGHTLY_NO_UPDATES
                                        } else if crate::turu::launched_by_army() && !from_shell {
                                            "The client that launched this one checks"
                                        } else {
                                            "Checking…"
                                        };
                                        let enabled =
                                            !nightly && !checking && (!crate::turu::launched_by_army() || from_shell);
                                        if widgets::button(
                                            ui,
                                            "Check Now",
                                            Kind::Normal,
                                            enabled,
                                            vec2(96.0, 24.0),
                                            why,
                                        )
                                        .clicked()
                                        {
                                            crate::turu::check_client_updates_now();
                                        }
                                    }
                                    Field::FrameInterpolation => {
                                        #[cfg(feature = "ruffle")]
                                        let mut on = ruffle_core::frame_interpolation::enabled();
                                        #[cfg(not(feature = "ruffle"))]
                                        let mut on = false;
                                        if widgets::switch(ui, &mut on).changed() {
                                            crate::platform::platform().set_frame_interpolation(on);
                                        }
                                    }
                                    Field::SafeDrawing => {
                                        let platform = crate::platform::platform();
                                        let mut on = platform.safe_drawing() == Some(true);
                                        if widgets::switch(ui, &mut on).changed() {
                                            platform.set_safe_drawing(on);
                                        }
                                    }
                                    Field::Graphics => {
                                        let platform = crate::platform::platform();
                                        if let Some((choices, saved)) = platform.graphics_choices() {
                                            ui.horizontal_wrapped(|ui| {
                                                for (value, name) in choices {
                                                    if widgets::chip(ui, name, value == saved) && value != saved {
                                                        platform.set_graphics(value);
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    Field::ShowFps => {
                                        let platform = crate::platform::platform();
                                        let mut on = platform.show_fps() == Some(true);
                                        if widgets::switch(ui, &mut on).changed() {
                                            platform.set_show_fps(on);
                                        }
                                    }
                                    Field::FramePacing => {
                                        let platform = crate::platform::platform();
                                        let uncapped = platform.frame_pacing().is_some_and(|(on, _)| on);
                                        ui.horizontal(|ui| {
                                            for (on, label) in [(false, "V-sync"), (true, "Uncapped")] {
                                                if widgets::chip(ui, label, uncapped == on) && uncapped != on {
                                                    platform.set_uncapped(on);
                                                }
                                            }
                                        });
                                    }
                                    Field::FpsLimit => {
                                        let platform = crate::platform::platform();
                                        let limit = platform.frame_pacing().map_or(0, |(_, limit)| limit);
                                        // A number set elsewhere (the window's Custom) shows as one more choice.
                                        let mut choices = vec![(0, "Off".to_owned()), (1, "Screen".to_owned())];
                                        choices.extend([60, 120, 144].map(|fps| (fps, fps.to_string())));
                                        if !choices.iter().any(|(value, _)| *value == limit) {
                                            choices.push((limit, limit.to_string()));
                                        }
                                        ui.horizontal(|ui| {
                                            for (value, label) in choices {
                                                if widgets::chip(ui, &label, limit == value) && limit != value {
                                                    platform.set_fps_limit(value);
                                                }
                                            }
                                        });
                                    }
                                    Field::Performance => {
                                        let platform = crate::platform::platform();
                                        let level = platform.performance().unwrap_or(0);
                                        ui.horizontal(|ui| {
                                            for (i, label) in ["Off", "Balanced", "Fast"].into_iter().enumerate() {
                                                if widgets::chip(ui, label, level == i as u8) && level != i as u8 {
                                                    platform.set_performance(i as u8);
                                                }
                                            }
                                        });
                                    }
                                    Field::MemoryLimit => {
                                        let platform = crate::platform::platform();
                                        let limit = platform.memory_limit_mb().unwrap_or(0);
                                        ui.horizontal(|ui| {
                                            for (mb, label) in
                                                [(0, "Off"), (1024, "1 GB"), (2048, "2 GB"), (3072, "3 GB")]
                                            {
                                                if widgets::chip(ui, label, limit == mb) && limit != mb {
                                                    platform.set_memory_limit_mb(mb);
                                                }
                                            }
                                            if let Some(now) = platform.memory_now_mb() {
                                                ui.label(format!("Now: {now} MB"));
                                            }
                                        });
                                    }
                                    Field::FreeMemory => {
                                        if widgets::button(ui, "Free", Kind::Normal, true, vec2(80.0, 24.0), "")
                                            .clicked()
                                        {
                                            crate::platform::platform().free_memory();
                                        }
                                    }
                                    Field::FullScreen => {
                                        let platform = crate::platform::platform();
                                        let mut on = platform.full_screen() == Some(true);
                                        if widgets::switch(ui, &mut on).changed() {
                                            platform.set_full_screen(on);
                                        }
                                    }
                                    Field::OpenLogFolder => {
                                        if widgets::button(ui, "Open", Kind::Normal, true, vec2(80.0, 24.0), "")
                                            .clicked()
                                        {
                                            crate::platform::platform().open_log_folder();
                                        }
                                    }
                                    Field::CrashReports => {
                                        let mut on = settings.send_crash_reports == Some(true);
                                        if widgets::switch(ui, &mut on).changed() {
                                            settings.send_crash_reports = Some(on);
                                            crate::platform::platform().set_crash_uploads(on);
                                            settings_changed = true;
                                        }
                                    }
                                }
                            });
                        }
                        if section == Section::Monitor && self.show_qr {
                            self.phone_qr(ui, &settings);
                        }
                        if section == Section::Report {
                            report_status(ui, &settings);
                        }
                        if section == Section::Updates {
                            client_update_status(ui);
                        }
                        if section == Section::Monitor {
                            monitor_status(ui);
                            if let Some((text, ok)) = &self.monitor_message {
                                ui.label(RichText::new(text).small().color(if *ok { SUCCESS } else { DANGER }));
                            }
                        }
                        if section == Section::General
                            && let Some((text, ok)) = &self.cleared
                        {
                            ui.label(RichText::new(text).small().color(if *ok { SUCCESS } else { DANGER }));
                        }
                    });
                }
            });

        self.entries = entries;
        if settings_changed {
            crate::turu::apply_monitor(&settings);
            *bot.settings.write() = settings;
            bot.save_settings();
        }
        if options_changed {
            save_relogin_options(&bot, &options);
        }
    }

    /// VibeSkua deletes Flash Player's folders; Ruffle keeps the same data in its SharedObjects
    /// folder. Asks once before deleting.
    fn clear_flash_cache(&mut self, ui: &mut Ui) {
        let (label, kind) = if self.confirm_clear {
            ("Confirm", Kind::Danger)
        } else {
            ("Clear", Kind::Normal)
        };
        let Some(dir) = crate::turu::save_directory() else {
            widgets::button(ui, label, kind, false, vec2(80.0, 24.0), "No SharedObjects folder");
            return;
        };
        if !widgets::button(ui, label, kind, true, vec2(80.0, 24.0), "").clicked() {
            return;
        }
        if !self.confirm_clear {
            self.confirm_clear = true;
            return;
        }
        self.confirm_clear = false;
        self.cleared = Some(match std::fs::remove_dir_all(dir) {
            Ok(()) => (format!("Cleared {}. Restart Turu.", dir.display()), true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ("The Flash cache is already empty.".into(), true),
            Err(e) => (format!("Couldn't clear {}: {e}", dir.display()), false),
        });
    }
}

impl ApplicationOptionsWindow {
    /// Open, Copy Link (this computer's, or the network's with Allow Other Devices) and Copy
    /// Key. Links carry the key after `#`, which browsers don't send.
    fn monitor_links(&mut self, ui: &mut Ui, settings: &AppSettings) {
        let serving = match crate::turu::monitor_status() {
            crate::turu::MonitorStatus::Serving(addr) => Some(addr),
            _ => None,
        };
        let size = vec2(72.0, 24.0);
        let why = "Enable Web Monitor first";
        // Right to left: Copy Key, Copy Link, Open.
        if widgets::button(ui, "Copy Key", Kind::Normal, true, size, "").clicked() {
            self.monitor_message = Some(match crate::turu::monitor_key() {
                Ok(key) => {
                    ui.ctx().copy_text(key);
                    (
                        "Key copied. Paste it in the page's Key button on the other device.".into(),
                        true,
                    )
                }
                Err(e) => (format!("Couldn't read the key: {e}"), false),
            });
        }
        if widgets::button(ui, "Copy Link", Kind::Normal, serving.is_some(), size, why).clicked()
            && let Some(addr) = serving
        {
            let host = match crate::turu::lan_address() {
                Some(ip) if settings.monitor_allow_other_devices => ip.to_string(),
                _ => "127.0.0.1".into(),
            };
            self.monitor_message = Some(match crate::turu::monitor_key() {
                Ok(key) => {
                    let host = if host.contains(':') { format!("[{host}]") } else { host };
                    ui.ctx().copy_text(format!("http://{host}:{}/#key={key}", addr.port()));
                    (
                        "Link copied, with the key: share it only with your own devices.".into(),
                        true,
                    )
                }
                Err(e) => (format!("Couldn't read the key: {e}"), false),
            });
        }
        if widgets::button(ui, "Open", Kind::Normal, serving.is_some(), size, why).clicked()
            && let Some(addr) = serving
            && let Ok(url) = url::Url::parse(&format!("http://127.0.0.1:{}/", addr.port()))
        {
            crate::platform::platform().open_url(url.as_str());
        }
    }

    /// The QR code of the link a phone opens: this computer's network address and the key.
    fn phone_qr(&mut self, ui: &mut Ui, settings: &AppSettings) {
        let link = match (crate::turu::monitor_status(), crate::turu::lan_address()) {
            (crate::turu::MonitorStatus::Serving(addr), Some(ip)) if settings.monitor_allow_other_devices => {
                let host = if ip.is_ipv6() {
                    format!("[{ip}]")
                } else {
                    ip.to_string()
                };
                crate::turu::monitor_key().map(|key| format!("http://{host}:{}/#key={key}", addr.port()))
            }
            (crate::turu::MonitorStatus::Serving(_), _) if settings.monitor_allow_other_devices => {
                Err("this computer has no network address".into())
            }
            _ => Err("turn on Enable Web Monitor and Allow Other Devices first".into()),
        };
        let link = match link {
            Ok(link) => link,
            Err(e) => {
                ui.label(RichText::new(format!("No QR code: {e}.")).small().color(MUTED));
                return;
            }
        };
        if self.qr.as_ref().is_none_or(|(shown, _)| *shown != link) {
            match qr_image(&link) {
                Some(image) => {
                    let texture = ui
                        .ctx()
                        .load_texture("turu-monitor-qr", image, egui::TextureOptions::NEAREST);
                    self.qr = Some((link, texture));
                }
                None => {
                    ui.label(
                        RichText::new("The link is too long for a QR code.")
                            .small()
                            .color(DANGER),
                    );
                    return;
                }
            }
        }
        if let Some((_, texture)) = &self.qr {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(texture).fit_to_exact_size(vec2(180.0, 180.0)));
                ui.label(
                    RichText::new(
                        "Holds the key: anyone who scans it can see your bots (and control them with Allow Control). \
                         Over Tailscale, open the Tailscale address instead and enter the key with Copy Key.",
                    )
                    .small()
                    .color(MUTED),
                );
            });
        }
    }

    /// Copy Report Settings (this monitor's address and the push key, for another machine's
    /// Report To) and Regenerate, asking once.
    fn push_key(&mut self, ui: &mut Ui, settings: &AppSettings) {
        let (label, kind) = if self.confirm_new_push_key {
            ("Confirm", Kind::Danger)
        } else {
            ("Regenerate", Kind::Normal)
        };
        if widgets::button(ui, label, kind, true, vec2(90.0, 24.0), "").clicked() {
            if self.confirm_new_push_key {
                self.confirm_new_push_key = false;
                self.monitor_message = Some(match crate::turu::regenerate_push_key() {
                    Ok(()) => (
                        "New push key made: reporting machines need it in their Push Key.".into(),
                        true,
                    ),
                    Err(e) => (format!("Couldn't make a new push key: {e}"), false),
                });
            } else {
                self.confirm_new_push_key = true;
            }
        }
        if widgets::button(ui, "Copy Report Settings", Kind::Normal, true, vec2(130.0, 24.0), "").clicked() {
            let address = match crate::turu::lan_address() {
                Some(ip) if settings.monitor_allow_other_devices => {
                    let host = if ip.is_ipv6() {
                        format!("[{ip}]")
                    } else {
                        ip.to_string()
                    };
                    format!("http://{host}:{}", settings.monitor_port)
                }
                _ => format!("http://<this computer>:{}", settings.monitor_port),
            };
            self.monitor_message = Some(match crate::turu::monitor_push_key() {
                Ok(key) => {
                    ui.ctx().copy_text(format!("Report To: {address}\nPush Key: {key}"));
                    (
                        "Copied for the other machine's Report To and Push Key. Over Tailscale, use this computer's Tailscale name or 100.x address instead.".into(),
                        true,
                    )
                }
                Err(e) => (format!("Couldn't read the push key: {e}"), false),
            });
        }
    }

    /// Asks once before replacing the key.
    fn regenerate_key(&mut self, ui: &mut Ui) {
        let (label, kind) = if self.confirm_new_key {
            ("Confirm", Kind::Danger)
        } else {
            ("Regenerate", Kind::Normal)
        };
        if !widgets::button(ui, label, kind, true, vec2(96.0, 24.0), "").clicked() {
            return;
        }
        if !self.confirm_new_key {
            self.confirm_new_key = true;
            return;
        }
        self.confirm_new_key = false;
        self.monitor_message = Some(match crate::turu::regenerate_monitor_key() {
            Ok(()) => ("New key made: copy the link again for your devices.".into(), true),
            Err(e) => (format!("Couldn't make a new key: {e}"), false),
        });
    }
}

/// The room `field`'s control takes at its row's right ([`widgets::setting_row`]).
fn control_width(field: &Field) -> f32 {
    match field {
        Field::Text(_) | Field::Secret(_) => 220.0,
        // Open, Copy Link, Copy Key / Copy Report Settings, Regenerate.
        Field::MonitorLinks | Field::PushKey => 228.0,
        Field::Bool(_)
        | Field::Unused(_)
        | Field::ClientUpdates(_)
        | Field::OptionBool(_)
        | Field::FrameInterpolation
        | Field::SafeDrawing
        | Field::ShowFps
        | Field::FullScreen
        | Field::CrashReports => 48.0,
        // Automatic, Vulkan, DirectX 12 (and OpenGL).
        Field::Graphics => 330.0,
        Field::Performance => 230.0,
        Field::FramePacing => 190.0,
        Field::FpsLimit => 330.0,
        Field::MemoryLimit => 330.0,
        _ => 96.0,
    }
}

/// The QR code of `text`, black on white with a quiet zone, 4 pixels a module.
fn qr_image(text: &str) -> Option<egui::ColorImage> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    let (modules, quiet, scale) = (code.width(), 4, 4);
    let side = (modules + 2 * quiet) * scale;
    let colors = code.to_colors();
    let mut pixels = vec![egui::Color32::WHITE; side * side];
    for (i, color) in colors.iter().enumerate() {
        if *color != qrcode::Color::Dark {
            continue;
        }
        let (x, y) = (i % modules + quiet, i / modules + quiet);
        for dy in 0..scale {
            let row = (y * scale + dy) * side;
            pixels[row + x * scale..row + (x + 1) * scale].fill(egui::Color32::BLACK);
        }
    }
    Some(egui::ColorImage::new([side, side], pixels))
}

fn report_status(ui: &mut Ui, settings: &AppSettings) {
    if turu_bot::monitor::push::insecure(&settings.monitor_report_to) {
        ui.label(
            RichText::new("Plain http:// across the internet: reports and the push key travel unencrypted. Use https:// or Tailscale.")
                .small()
                .color(DANGER),
        );
    }
    let (text, color) = match crate::turu::report_state() {
        crate::turu::ReportState::Off => (
            format!(
                "Not reporting. This machine would be \"{}\".",
                crate::turu::default_machine_name()
            ),
            MUTED,
        ),
        crate::turu::ReportState::Failed(e) => (format!("Not reporting: {e}"), DANGER),
        crate::turu::ReportState::Reporting(machine, status) => match (status.error, status.last_ok) {
            (Some(e), _) => (format!("Reporting as \"{machine}\" fails: {e}"), DANGER),
            (None, Some(_)) => (format!("Reporting as \"{machine}\"."), SUCCESS),
            (None, None) => (format!("Reporting as \"{machine}\": connecting…"), MUTED),
        },
    };
    ui.label(RichText::new(text).small().color(color));
}

/// The last client update check: when, and what it found.
fn client_update_status(ui: &mut Ui) {
    let state = turu_bot::updates::state();
    let (text, color) = if turu_bot::updates::this_is_nightly() {
        (format!("{}.", turu_bot::updates::NIGHTLY_NO_UPDATES), MUTED)
    } else if state.checking {
        (format!("Checking for a new {}…", turu_bot::updates::app_name()), MUTED)
    } else if let Some(error) = &state.error {
        (
            format!("Couldn't check for a new {}: {error}", turu_bot::updates::app_name()),
            DANGER,
        )
    } else if let Some(release) = &state.available {
        (
            format!(
                "{} {} is available: open Update in the bar",
                turu_bot::updates::app_name(),
                release.version
            ),
            SUCCESS,
        )
    } else if let Some(at) = state.checked_at {
        (
            format!(
                "{} {} is the newest (checked at {})",
                turu_bot::updates::app_name(),
                crate::turu::VERSION,
                at.format("%H:%M")
            ),
            MUTED,
        )
    } else {
        return;
    };
    ui.label(RichText::new(text).small().color(color));
}

fn monitor_status(ui: &mut Ui) {
    let (text, color) = match crate::turu::monitor_status() {
        crate::turu::MonitorStatus::Off => ("The web monitor is off.".to_owned(), MUTED),
        crate::turu::MonitorStatus::Serving(addr) if addr.ip().is_unspecified() => (
            format!("Serving on port {}, for this computer and your network.", addr.port()),
            SUCCESS,
        ),
        crate::turu::MonitorStatus::Serving(addr) => (format!("Serving on http://{addr}, for this computer."), SUCCESS),
        crate::turu::MonitorStatus::Failed(e) => (format!("Not running: {e}"), DANGER),
        crate::turu::MonitorStatus::LaunchedByArmy => (
            "Army > Accounts launched this client: the window that launched it serves the monitor.".to_owned(),
            MUTED,
        ),
    };
    ui.label(RichText::new(text).small().color(color));
}
