//! Options > Game (VibeSkua's 612x720 window): a searchable list of the bot's options. Changes
//! are written to `bot.options` at once (the engine re-applies them every 250 ms); Save keeps
//! them in `options.json`.

use super::theme::{MUTED, TEXT};
use super::widgets::{self, Kind};
use egui::{ComboBox, Context, DragValue, Layout, RichText, ScrollArea, Ui, vec2};
use turu_bot::options::{BotOptions, HuntPriority};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Visuals,
    Combat,
    Drops,
    Relogin,
    Gameplay,
}

impl Section {
    const ALL: [Section; 5] = [Self::Visuals, Self::Combat, Self::Drops, Self::Relogin, Self::Gameplay];

    fn chip(self) -> &'static str {
        match self {
            Self::Visuals => "Visuals",
            Self::Combat => "Combat",
            Self::Drops => "Drops",
            Self::Relogin => "Relogin",
            Self::Gameplay => "Gameplay",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Visuals => "Performance & Visuals",
            Self::Combat => "Combat & Targeting",
            Self::Drops => "Drops & Looting",
            Self::Relogin => "Auto-Relogin & Network",
            Self::Gameplay => "Gameplay & Character",
        }
    }
}

enum Field {
    Bool(fn(&mut BotOptions) -> &mut bool),
    Int(fn(&mut BotOptions) -> &mut i32, Option<&'static str>),
    Text(fn(&mut BotOptions) -> &mut String),
    Server,
    Priority,
    ReloadMap,
}

struct Entry {
    section: Section,
    label: &'static str,
    field: Field,
}

macro_rules! b {
    ($section:ident, $label:expr, $f:ident) => {
        Entry {
            section: Section::$section,
            label: $label,
            field: Field::Bool(|o| &mut o.$f),
        }
    };
}
macro_rules! n {
    ($section:ident, $label:expr, $f:ident) => {
        Entry {
            section: Section::$section,
            label: $label,
            field: Field::Int(|o| &mut o.$f, None),
        }
    };
    ($section:ident, $label:expr, $f:ident, $suffix:expr) => {
        Entry {
            section: Section::$section,
            label: $label,
            field: Field::Int(|o| &mut o.$f, Some($suffix)),
        }
    };
}

/// Every field, in VibeSkua's order; the Gameplay section is sorted by label.
fn entries() -> Vec<Entry> {
    let mut list = vec![
        b!(Visuals, "Lag Killer", lag_killer),
        b!(Visuals, "Hide Players", hide_players),
        b!(Visuals, "Disable FX", disable_fx),
        b!(Visuals, "Disable Collisions", disable_collisions),
        b!(Visuals, "Show FPS", show_fps),
        n!(Visuals, "Set FPS", set_fps, "FPS"),
        Entry {
            section: Section::Visuals,
            label: "Reload Map",
            field: Field::ReloadMap,
        },
        b!(Combat, "Aggro Monsters", aggro_monsters),
        b!(Combat, "Aggro All Monsters", aggro_all_monsters),
        b!(Combat, "Attack Without Target", attack_without_target),
        b!(Combat, "Infinite Range", infinite_range),
        b!(Combat, "Magnetise", magnetise),
        n!(Combat, "Hunt Delay", hunt_delay, "ms"),
        n!(Combat, "Hunt Buffer", hunt_buffer),
        b!(Combat, "Rest Packets", rest_packets),
        b!(Combat, "Safe Timings", safe_timings),
        b!(Drops, "Accept AC Drops", accept_ac_drops),
        b!(Drops, "Accept All Drops", accept_all_drops),
        b!(Drops, "Reject All Drops", reject_all_drops),
        Entry {
            section: Section::Relogin,
            label: "Relogin Server",
            field: Field::Server,
        },
        b!(Relogin, "Auto Relogin", auto_relogin),
        b!(Relogin, "Auto Relogin Any", auto_relogin_any),
        b!(Relogin, "Retry Relogin", retry_relogin),
        b!(Relogin, "Safe Relogin", safe_relogin),
        n!(Relogin, "Relogin Tries", relogin_tries),
        n!(Relogin, "Login Timeout", login_timeout, "ms"),
    ];
    let mut gameplay = vec![
        b!(Gameplay, "Private Rooms", private_rooms),
        n!(Gameplay, "Private Number", private_number),
        b!(Gameplay, "Skip Cutscenes", skip_cutscenes),
        b!(Gameplay, "Headless Mode", headless_mode),
        b!(Gameplay, "Streamer Mode", streamer_mode),
        Entry {
            section: Section::Gameplay,
            label: "Custom Name",
            field: Field::Text(|o| &mut o.custom_name),
        },
        Entry {
            section: Section::Gameplay,
            label: "Custom Guild",
            field: Field::Text(|o| &mut o.custom_guild),
        },
        n!(Gameplay, "Walk Speed", walk_speed),
        n!(Gameplay, "Action Delay", action_delay, "ms"),
        n!(Gameplay, "Maximum Tries", maximum_tries),
        n!(Gameplay, "Join Map Tries", join_map_tries),
        n!(
            Gameplay,
            "Quest Accept And Complete Tries",
            quest_accept_and_complete_tries
        ),
        n!(Gameplay, "Load Timeout", load_timeout, "ms"),
        b!(Gameplay, "Disable Death Ads", disable_death_ads),
        Entry {
            section: Section::Gameplay,
            label: "Hunt Priority",
            field: Field::Priority,
        },
        b!(Gameplay, "Use Function Based Skills", use_function_based_skills),
    ];
    gameplay.sort_by_key(|e| e.label);
    list.extend(gameplay);
    list
}

/// VibeSkua's fallback server list for the Relogin Server box (until the game's list arrives).
const SERVERS: [&str; 12] = [
    "Artix",
    "Twilly",
    "Galanoth",
    "Gravelyn",
    "Safiria",
    "Sir Ver",
    "Sepulchure",
    "Yorumi",
    "Alteon",
    "Espada",
    "Yokai",
    "Zorbot",
];

pub struct GameOptionsWindow {
    entries: Vec<Entry>,
    search: String,
    filter: Option<Section>,
}

impl Default for GameOptionsWindow {
    fn default() -> Self {
        Self {
            entries: entries(),
            search: String::new(),
            filter: None,
        }
    }
}

impl GameOptionsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        super::window(ctx, "Game Options", [612.0, 720.0], 1)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();

        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let width = ui.available_width();
        widgets::text_box(ui, &mut self.search, "Search game options", width);
        ui.horizontal_wrapped(|ui| {
            if widgets::chip(ui, "All", self.filter.is_none()) {
                self.filter = None;
            }
            for section in Section::ALL {
                if widgets::chip(ui, section.chip(), self.filter == Some(section)) {
                    self.filter = Some(section);
                }
            }
        });

        let footer = 44.0;
        let list_height = (ui.available_height() - footer).max(80.0);
        let needle = self.search.trim().to_lowercase();
        let mut snapshot = bot.options.read().clone();
        ScrollArea::vertical()
            .id_salt("game_options_list")
            .max_height(list_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for section in Section::ALL {
                    if self.filter.is_some_and(|f| f != section) {
                        continue;
                    }
                    let rows: Vec<&Entry> = self
                        .entries
                        .iter()
                        .filter(|e| {
                            e.section == section
                                && (needle.is_empty()
                                    || e.label.to_lowercase().contains(&needle)
                                    || section.title().to_lowercase().contains(&needle))
                        })
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    widgets::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        widgets::section_label(ui, &section.title().to_uppercase());
                        ui.add_space(2.0);
                        for entry in rows {
                            ui.horizontal(|ui| {
                                ui.set_min_height(26.0);
                                ui.label(RichText::new(entry.label).color(TEXT));
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    edit(ui, &entry.field, &mut snapshot, bot);
                                });
                            });
                        }
                    });
                }
            });

        ui.horizontal(|ui| {
            if widgets::button(ui, "Defaults", Kind::Normal, true, vec2(80.0, 28.0), "").clicked() {
                *bot.options.write() = BotOptions::default();
            }
            if widgets::button(ui, "Reset", Kind::Normal, true, vec2(80.0, 28.0), "").clicked() {
                *bot.options.write() = bot.paths.load_options().unwrap_or_default();
            }
            ui.label(RichText::new("Changes apply now; Save keeps them").small().color(MUTED));
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button(ui, "Save", Kind::Primary, true, vec2(80.0, 28.0), "").clicked() {
                    bot.save_options();
                }
            });
        });
    }
}

/// One option's control. Writes to the live options as soon as it changes.
fn edit(ui: &mut Ui, field: &Field, snapshot: &mut BotOptions, bot: &std::sync::Arc<turu_bot::Bot>) {
    match field {
        Field::Bool(access) => {
            let mut value = *access(snapshot);
            if widgets::switch(ui, &mut value).changed() {
                *access(&mut bot.options.write()) = value;
            }
        }
        Field::Int(access, suffix) => {
            let mut value = *access(snapshot);
            let speed = (value.abs() as f64 / 200.0).max(1.0);
            let mut drag = DragValue::new(&mut value).range(0..=i32::MAX).speed(speed);
            if let Some(suffix) = suffix {
                drag = drag.suffix(format!(" {suffix}"));
            }
            if ui.add(drag).changed() {
                *access(&mut bot.options.write()) = value;
            }
        }
        Field::Text(access) => {
            let mut value = access(snapshot).clone();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut value)
                        .desired_width(200.0)
                        .margin(egui::Margin::symmetric(8, 4)),
                )
                .changed()
            {
                *access(&mut bot.options.write()) = value;
            }
        }
        Field::Server => {
            let current = snapshot.relogin_server.clone();
            ComboBox::from_id_salt("relogin_server")
                .selected_text(&current)
                .width(150.0)
                .show_ui(ui, |ui| {
                    turu_bot::api::servers::refresh_in_background(bot);
                    let cached = bot.servers.cached();
                    let mut names: Vec<&str> = if cached.is_empty() {
                        SERVERS.to_vec()
                    } else {
                        cached.iter().map(|s| s.name.as_str()).collect()
                    };
                    if !names.contains(&current.as_str()) {
                        names.insert(0, &current);
                    }
                    for name in names {
                        if ui.selectable_label(current == name, name).clicked() {
                            bot.options.write().relogin_server = name.to_owned();
                        }
                    }
                });
        }
        Field::Priority => {
            let current = snapshot.hunt_priority;
            ComboBox::from_id_salt("hunt_priority")
                .selected_text(format!("{current:?}"))
                .width(150.0)
                .show_ui(ui, |ui| {
                    for choice in [
                        HuntPriority::None,
                        HuntPriority::HighHP,
                        HuntPriority::LowHP,
                        HuntPriority::Close,
                    ] {
                        if ui.selectable_label(current == choice, format!("{choice:?}")).clicked() {
                            bot.options.write().hunt_priority = choice;
                        }
                    }
                });
        }
        Field::ReloadMap => {
            if widgets::button(ui, "Reload", Kind::Normal, true, vec2(80.0, 24.0), "").clicked() {
                crate::turu::spawn_action("Reload Map", |bot| {
                    turu_bot::api::dispatch(bot, "map.reload", &[]).map(|_| ())
                });
            }
        }
    }
}
