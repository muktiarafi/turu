//! Options > Application (VibeSkua's 580x650 `ApplicationOptionsView`): a searchable list of
//! Turu's own settings in three sections. Changes are saved at once: `settings.json`
//! (`AppSettings`), and for Relogin Try Delay and Relogin Watchdog, which are bot options,
//! just those two fields of `options.json`.

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
}

impl Section {
    const ALL: [Section; 3] = [Self::General, Self::Updates, Self::Webhooks];

    fn chip(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Updates => "Updates",
            Self::Webhooks => "Webhooks",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::General => "General & Performance",
            Self::Updates => "Auto Updates & Sync",
            Self::Webhooks => "Webhooks & Alerts",
        }
    }
}

enum Field {
    Bool(fn(&mut AppSettings) -> &mut bool),
    /// A number with its suffix and smallest value.
    Int(fn(&mut AppSettings) -> &mut i32, &'static str, i32),
    Text(fn(&mut AppSettings) -> &mut String),
    /// Stored, but nothing reads it yet: shown greyed.
    Unused(fn(&mut AppSettings) -> &mut bool),
    OptionBool(fn(&mut BotOptions) -> &mut bool),
    OptionInt(fn(&mut BotOptions) -> &mut i32, &'static str),
    Test {
        nexus: bool,
    },
    ClearFlashCache,
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
                Field::Int(|s| &mut s.animation_frame_rate, "FPS", 1)
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
        entry!(Webhooks, "Test Discord Webhook", "", Field::Test { nexus: false }),
        entry!(
            Webhooks,
            "Skua Nexus Webhook URL",
            "URL to send Skua Nexus notifications and live bot statistics",
            Field::Text(|s| &mut s.nexus_webhook_url)
        ),
        entry!(Webhooks, "Test Skua Nexus Webhook", "", Field::Test { nexus: true }),
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
            Field::Int(|s| &mut s.webhook_ping_interval, "Min", 1)
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
}

impl Default for ApplicationOptionsWindow {
    fn default() -> Self {
        Self {
            entries: entries(),
            search: String::new(),
            filter: None,
            confirm_clear: false,
            cleared: None,
        }
    }
}

impl ApplicationOptionsWindow {
    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        if !*open {
            self.confirm_clear = false;
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
                                Field::Text(_) => 230.0,
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
                                    Field::Int(access, suffix, min) => {
                                        let drag = DragValue::new(access(&mut settings))
                                            .range(*min..=i32::MAX)
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
                                    Field::Test { nexus } => {
                                        let url = if *nexus {
                                            &settings.nexus_webhook_url
                                        } else {
                                            &settings.discord_webhook_url
                                        };
                                        let enabled = !url.trim().is_empty();
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
                                            bot.webhooks.send(Notice::Test { nexus: *nexus });
                                        }
                                    }
                                    Field::ClearFlashCache => {
                                        self.clear_flash_cache(ui);
                                    }
                                });
                            });
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
