//! Options > Application (VibeSkua's 580x650 `ApplicationOptionsView`): a searchable list of
//! Turu's own settings in five sections. Changes are saved at once: `settings.json`
//! (`AppSettings`), and for Relogin Try Delay and Relogin Watchdog, which are bot options,
//! just those two fields of `options.json`. Web Monitor replaces VibeSkua's Skua Nexus webhook.

use super::theme::{DANGER, MUTED, SUCCESS, TEXT};
use super::widgets::{self, Kind};
use egui::{DragValue, Layout, RichText, ScrollArea, Ui, vec2};
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::options::BotOptions;
use turu_bot::settings::AppSettings;
use turu_bot::webhooks::Notice;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Updates,
    Webhooks,
    Monitor,
    Report,
}

impl Section {
    const ALL: [Section; 5] = [
        Self::General,
        Self::Updates,
        Self::Webhooks,
        Self::Monitor,
        Self::Report,
    ];

    fn chip(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Updates => "Updates",
            Self::Webhooks => "Webhooks",
            Self::Monitor => "Monitor",
            Self::Report => "Report",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::General => "General & Performance",
            Self::Updates => "Auto Updates & Sync",
            Self::Webhooks => "Webhooks & Alerts",
            Self::Monitor => "Web Monitor",
            Self::Report => "Report to Another Monitor",
        }
    }
}

enum Field {
    Bool(fn(&mut AppSettings) -> &mut bool),
    /// A number with its suffix, smallest and largest value.
    Int(fn(&mut AppSettings) -> &mut i32, &'static str, i32, i32),
    Text(fn(&mut AppSettings) -> &mut String),
    /// Text shown as dots (a key).
    Secret(fn(&mut AppSettings) -> &mut String),
    /// Stored, but nothing reads it yet: shown greyed.
    Unused(fn(&mut AppSettings) -> &mut bool),
    OptionBool(fn(&mut BotOptions) -> &mut bool),
    OptionInt(fn(&mut BotOptions) -> &mut i32, &'static str),
    Test,
    ClearFlashCache,
    /// Where the monitor is served, with Open, Copy Link and Copy Key.
    MonitorLinks,
    /// Show / Hide the phone link's QR code.
    PhoneQr,
    RegenerateMonitorKey,
    /// Copy Report Settings and Regenerate, for the push key.
    PushKey,
}

struct Entry {
    section: Section,
    label: &'static str,
    description: &'static str,
    field: Field,
    /// VibeSkua's `RequiresRestart`.
    restart: bool,
}

macro_rules! entry {
    ($section:ident, $label:expr, $description:expr, $field:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            description: $description,
            field: $field,
            restart: false,
        }
    };
}

/// Every setting, in VibeSkua's order.
fn entries() -> Vec<Entry> {
    vec![
        entry!(
            General,
            "Show Username in Title",
            "Whether to display logged-in character name in window title",
            Field::Bool(|s| &mut s.show_username_in_title)
        ),
        Entry {
            restart: true,
            ..entry!(
                General,
                "Client Animation Frame-rate",
                "The frame rate the game client starts with; the game's own FPS setting, or Set FPS (Game options), takes over in game. AQW is made for 24",
                Field::Int(|s| &mut s.animation_frame_rate, "FPS", 1, i32::MAX)
            )
        },
        entry!(
            General,
            "Relogin Try Delay",
            "Delay in milliseconds between login attempts when disconnected",
            Field::OptionInt(|o| &mut o.relogin_try_delay, "ms")
        ),
        entry!(
            General,
            "Relogin Watchdog",
            "Watch for unexpected disconnections, even with no script running, and relog when Auto Relogin (Game Options) is on. Off leaves relogs to kicks and server logouts",
            Field::OptionBool(|o| &mut o.relogin_watchdog)
        ),
        entry!(
            General,
            "Clear Flash Cache",
            "Deletes the game's saved data (SharedObjects), such as the remembered login. Restart Turu afterwards",
            Field::ClearFlashCache
        ),
        entry!(
            Updates,
            "Check for Script Updates",
            "Whether to check for scripts updates when launching Turu",
            Field::Bool(|s| &mut s.check_bot_scripts_updates)
        ),
        entry!(
            Updates,
            "Auto Update Scripts",
            "Whether to auto update scripts when launching Turu, needs \"Check for Scripts Updates\" to be true",
            Field::Bool(|s| &mut s.auto_update_bot_scripts)
        ),
        entry!(
            Updates,
            "Check for AdvanceSkill Sets Updates",
            "Whether to check for scripts updates when launching the Manager",
            Field::Unused(|s| &mut s.check_advance_skill_sets_updates)
        ),
        entry!(
            Updates,
            "Auto Update AdvanceSkill Sets",
            "Whether to auto update advance skill sets when launching the Manager, needs \"Check for AdvanceSkill Sets updates\" to be true",
            Field::Unused(|s| &mut s.auto_update_advance_skill_sets_updates)
        ),
        entry!(
            Updates,
            "Check for Junk Items Updates",
            "Whether to check for junk items updates when launching the Manager",
            Field::Unused(|s| &mut s.check_junk_items_updates)
        ),
        entry!(
            Updates,
            "Auto Update Junk Items",
            "Whether to auto update junk items when launching the Manager, needs \"Check for Junk Items Updates\" to be true",
            Field::Unused(|s| &mut s.auto_update_junk_items)
        ),
        entry!(
            Webhooks,
            "Discord Webhook URL",
            "URL to send Discord notifications and live statistics when scripts start, stop, or crash",
            Field::Text(|s| &mut s.discord_webhook_url)
        ),
        entry!(Webhooks, "Test Discord Webhook", "", Field::Test),
        entry!(
            Webhooks,
            "Notify on Script Started",
            "Send alerts and initial statistics when a script starts running",
            Field::Bool(|s| &mut s.webhook_notify_started)
        ),
        entry!(
            Webhooks,
            "Notify on Script Stopped",
            "Send session summary and statistics when a script finishes or stops",
            Field::Bool(|s| &mut s.webhook_notify_stopped)
        ),
        entry!(
            Webhooks,
            "Notify on Script Crashed",
            "Send alerts when a script crashes with an error",
            Field::Bool(|s| &mut s.webhook_notify_crashed)
        ),
        entry!(
            Webhooks,
            "Notify on Relog",
            "Send alerts when Turu disconnects and relogs",
            Field::Bool(|s| &mut s.webhook_notify_relogged)
        ),
        entry!(
            Webhooks,
            "Notify on Item Drop",
            "Send webhook notifications when a specific item drops",
            Field::Bool(|s| &mut s.webhook_notify_item_drops)
        ),
        entry!(
            Webhooks,
            "Drop List",
            "Comma-separated list of items to notify on drop",
            Field::Text(|s| &mut s.webhook_notify_item_drops_list)
        ),
        entry!(
            Webhooks,
            "Enable Health Pings",
            "Whether to send periodic health and bot statistics pings to Discord",
            Field::Bool(|s| &mut s.webhook_notify_health_ping)
        ),
        entry!(
            Webhooks,
            "Health Ping Interval",
            "Minutes between health pings (e.g. 5, 10, 30)",
            Field::Int(|s| &mut s.webhook_ping_interval, "Min", 1, i32::MAX)
        ),
        entry!(
            Monitor,
            "Enable Web Monitor",
            "Show every tab's bot, live, on a web page: in a browser here, or on your phone",
            Field::Bool(|s| &mut s.monitor_enabled)
        ),
        entry!(
            Monitor,
            "Allow Other Devices",
            "Listen on your network too, for phones and Tailscale Serve (tailscale serve --bg <port>). They need the key; don't forward the port on your router",
            Field::Bool(|s| &mut s.monitor_allow_other_devices)
        ),
        entry!(
            Monitor,
            "Allow Control",
            "The page can start, stop, pause and relog bots, send them to a map and forget offline ones",
            Field::Bool(|s| &mut s.monitor_allow_control)
        ),
        entry!(
            Monitor,
            "Monitor Port",
            "The port the page is served on",
            Field::Int(|s| &mut s.monitor_port, "", 1024, 65535)
        ),
        entry!(Monitor, "Monitor Link", "", Field::MonitorLinks),
        entry!(
            Monitor,
            "Phone QR Code",
            "Scan it with your phone's camera: the link on your network, with the key (needs Allow Other Devices)",
            Field::PhoneQr
        ),
        entry!(
            Monitor,
            "Regenerate Key",
            "Devices given the old key or link lose access",
            Field::RegenerateMonitorKey
        ),
        entry!(
            Monitor,
            "Push Key",
            "Other machines report their bots here with it: Copy Report Settings gives their Report To address and key",
            Field::PushKey
        ),
        entry!(
            Report,
            "Report To",
            "Another Turu's monitor, e.g. https://my-pc.tail1234.ts.net or http://100.101.102.103:8790: this machine's bots show on its page. Empty: off",
            Field::Text(|s| &mut s.monitor_report_to)
        ),
        entry!(
            Report,
            "Push Key",
            "The push key from that monitor's Copy Report Settings",
            Field::Secret(|s| &mut s.monitor_report_key)
        ),
        entry!(
            Report,
            "Machine Name",
            "This machine's name on that page; empty: the computer's name",
            Field::Text(|s| &mut s.monitor_machine_name)
        ),
        entry!(
            Report,
            "Accept Commands",
            "Run the commands that monitor sends (start, stop, relog, ...). Its log views work either way",
            Field::Bool(|s| &mut s.monitor_accept_commands)
        ),
    ]
}

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

        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
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
                            let control_width = match entry.field {
                                Field::Text(_) | Field::Secret(_) | Field::MonitorLinks | Field::PushKey => 230.0,
                                _ => 130.0,
                            };
                            ui.horizontal(|ui| {
                                ui.set_min_height(28.0);
                                ui.vertical(|ui| {
                                    ui.set_width((ui.available_width() - control_width - 8.0).max(140.0));
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(entry.label).color(TEXT));
                                        if entry.restart {
                                            ui.label(RichText::new("Restart required").small().color(MUTED));
                                        }
                                    });
                                    if !entry.description.is_empty() {
                                        ui.label(RichText::new(entry.description).small().color(MUTED));
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| match &entry.field {
                                    Field::Bool(access) => {
                                        settings_changed |= widgets::switch(ui, access(&mut settings)).changed();
                                    }
                                    Field::Unused(access) => {
                                        ui.add_enabled_ui(false, |ui| widgets::switch(ui, access(&mut settings)))
                                            .inner
                                            .on_disabled_hover_text("Coming soon: Turu doesn't update these yet");
                                    }
                                    Field::Int(access, suffix, min, max) => {
                                        let drag = DragValue::new(access(&mut settings))
                                            .range(*min..=*max)
                                            .suffix(format!(" {suffix}"));
                                        settings_changed |= ui.add(drag).changed();
                                    }
                                    Field::Text(access) => {
                                        let edit = egui::TextEdit::singleline(access(&mut settings))
                                            .desired_width(220.0)
                                            .margin(egui::Margin::symmetric(8, 4));
                                        settings_changed |= ui.add(edit).changed();
                                    }
                                    Field::OptionBool(access) => {
                                        options_changed |= widgets::switch(ui, access(&mut options)).changed();
                                    }
                                    Field::OptionInt(access, suffix) => {
                                        let value = access(&mut options);
                                        let speed = (value.abs() as f64 / 200.0).max(1.0);
                                        let drag = DragValue::new(value)
                                            .range(0..=i32::MAX)
                                            .speed(speed)
                                            .suffix(format!(" {suffix}"));
                                        options_changed |= ui.add(drag).changed();
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
                                        let edit = egui::TextEdit::singleline(access(&mut settings))
                                            .password(true)
                                            .desired_width(220.0)
                                            .margin(egui::Margin::symmetric(8, 4));
                                        settings_changed |= ui.add(edit).changed();
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
                                });
                            });
                        }
                        if section == Section::Monitor && self.show_qr {
                            self.phone_qr(ui, &settings);
                        }
                        if section == Section::Report {
                            report_status(ui, &settings);
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
            crate::util::open_url(&url);
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

/// Applies Relogin Try Delay and Relogin Watchdog now and saves them, leaving the other saved
/// options as they are (those wait for the Game window's Save).
fn save_relogin_options(bot: &Arc<Bot>, edited: &BotOptions) {
    {
        let mut live = bot.options.write();
        live.relogin_try_delay = edited.relogin_try_delay;
        live.relogin_watchdog = edited.relogin_watchdog;
    }
    let mut saved = bot.paths.load_options().unwrap_or_default();
    saved.relogin_try_delay = edited.relogin_try_delay;
    saved.relogin_watchdog = edited.relogin_watchdog;
    if let Err(e) = bot.paths.save_options(&saved) {
        tracing::warn!("Couldn't save the bot options: {e}");
    }
}
