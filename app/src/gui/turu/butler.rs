//! Plugins > Butler (VibeSkua ButlerV5's roster and settings windows): the other online
//! accounts running Butler, with Follow and Summon, and Butler's options.
//!
//! The roster is read from the sync files once a second. Summon and release only change this
//! client's state; Follow waits for a previous follow to end, so it runs off the UI thread.

use super::theme::{DANGER, MUTED, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{Align, Color32, ComboBox, Context, DragValue, Layout, RichText, ScrollArea, Ui, vec2};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::butler::sync::SyncData;
use turu_bot::butler::{self, ClassType, FollowSource, ParkSpot};

/// How often the roster is read again.
const REFRESH: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct ButlerWindow {
    roster: Vec<SyncData>,
    read_at: Option<Instant>,
    show_settings: bool,
    /// The last action's outcome (set from the Follow thread too) and whether it failed.
    message: Arc<Mutex<Option<(String, bool)>>>,
}

impl ButlerWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        let bot = crate::turu::bot();
        if self.read_at.is_none_or(|t| t.elapsed() >= REFRESH) {
            self.refresh(&bot);
        }
        super::window(ctx, "Butler", [520.0, 560.0], 6)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &bot));
        ctx.request_repaint_after(REFRESH);
    }

    fn refresh(&mut self, bot: &Bot) {
        self.roster = bot.butler.roster();
        self.read_at = Some(Instant::now());
    }

    fn say(&self, result: Result<String, String>) {
        let message = match result {
            Ok(text) => (text, false),
            Err(text) => (text, true),
        };
        *self.message.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
    }

    fn ui(&mut self, ui: &mut Ui, bot: &Arc<Bot>) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let mut changed = false;
        changed |= self.header_card(ui, bot);
        changed |= self.roster_card(ui, bot);
        changed |= self.footer(ui, bot);
        if self.show_settings {
            settings_card(ui, bot);
        }
        if changed {
            // Show the new state now rather than at the next read.
            self.refresh(bot);
        }
    }

    /// This account, whom it follows, and Stop following / Release all.
    fn header_card(&self, ui: &mut Ui, bot: &Arc<Bot>) -> bool {
        let butler = &bot.butler;
        let me = butler.username();
        let following = butler.following();
        let followers = butler.followers();
        let enabled = butler.enabled();
        let mut changed = false;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let name = if me.is_empty() { "(not logged in)" } else { me.as_str() };
                    ui.label(RichText::new(name).size(16.0).strong().color(TEXT));
                    let (state, color) = match &following {
                        Some((master, _)) if butler.is_parked() => (format!("Parked, waiting for {master}"), WARNING),
                        Some((master, source)) => {
                            let how = match source {
                                FollowSource::Ordered => "summoned",
                                FollowSource::Manual => "manual",
                            };
                            (format!("Following {master} ({how})"), SUCCESS)
                        }
                        None if butler.is_parking() => ("Going to the park spot".into(), MUTED),
                        None if !followers.is_empty() => (format!("Summoning {}", followers.join(", ")), SUCCESS),
                        None => ("Not following anyone".into(), MUTED),
                    };
                    ui.label(RichText::new(state).size(12.0).color(color));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !followers.is_empty()
                        && widgets::button(ui, "Release all", Kind::Normal, enabled, vec2(0.0, 26.0), "")
                            .on_hover_text("Clear the summon list; released butlers go to their park spot")
                            .clicked()
                    {
                        butler::release_all(bot);
                        self.say(Ok("Released all followers.".into()));
                        changed = true;
                    }
                    if following.is_some()
                        && widgets::button(ui, "Stop following", Kind::Danger, true, vec2(0.0, 26.0), "").clicked()
                    {
                        butler::stop_follow(bot);
                        self.say(Ok("Stopped following.".into()));
                        changed = true;
                    }
                });
            });
            let message = self.message.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if let Some((text, failed)) = message {
                ui.label(RichText::new(text).small().color(if failed { DANGER } else { MUTED }));
            }
        });
        changed
    }

    /// The other online accounts, each with Follow/Stop and Summon/Release.
    fn roster_card(&self, ui: &mut Ui, bot: &Arc<Bot>) -> bool {
        let mut changed = false;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "ACCOUNTS");
                widgets::count_badge(ui, self.roster.len());
            });
            ScrollArea::vertical()
                .id_salt("turu_butler_roster")
                .max_height((ui.available_height() - 60.0).max(120.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if self.roster.is_empty() {
                        ui.label(
                            RichText::new(
                                "No other accounts online. Accounts show up here once they log in to Turu \
                                 (in a tab, another window or without a window).",
                            )
                            .color(MUTED),
                        );
                    }
                    for (i, account) in self.roster.iter().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        changed |= self.row(ui, bot, account);
                    }
                });
        });
        changed
    }

    fn row(&self, ui: &mut Ui, bot: &Arc<Bot>, account: &SyncData) -> bool {
        let butler = &bot.butler;
        let me = butler.username();
        let enabled = butler.enabled();
        let following = butler.following().map(|(m, _)| m);
        let summoned = butler
            .followers()
            .iter()
            .any(|f| f.eq_ignore_ascii_case(&account.username));
        let am_following = following
            .as_deref()
            .is_some_and(|m| m.eq_ignore_ascii_case(&account.username));
        let follows_me = !me.is_empty() && account.following.eq_ignore_ascii_case(&me);
        let follows_other = !account.following.is_empty() && !follows_me;
        let my_server = butler.server();
        let other_server =
            !account.server.is_empty() && !my_server.is_empty() && !account.server.eq_ignore_ascii_case(&my_server);
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&account.username).strong().color(TEXT));
                    if account.attacking {
                        chip(ui, "fighting", WARNING);
                    }
                    if follows_me {
                        chip(ui, "following me", SUCCESS);
                    } else if follows_other {
                        chip(ui, &format!("following {}", account.following), MUTED);
                    } else if summoned {
                        chip(ui, "summoned", MUTED);
                    }
                    if account.script_on {
                        let script = if account.script_name.is_empty() {
                            "script"
                        } else {
                            &account.script_name
                        };
                        chip(ui, &format!("busy: {script}"), WARNING);
                    }
                    if account.parking {
                        chip(ui, "parking", MUTED);
                    }
                    if other_server {
                        chip(ui, "other server", DANGER);
                    }
                });
                let place: Vec<String> = [
                    account.map_with_room(),
                    account.server.clone(),
                    account.class_name.clone(),
                ]
                .into_iter()
                .filter(|v| !v.is_empty())
                .collect();
                ui.label(RichText::new(place.join("  ·  ")).size(11.0).color(MUTED));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let size = vec2(72.0, 26.0);
                // Summon / Release.
                if summoned {
                    if widgets::button(ui, "Release", Kind::Normal, true, size, "").clicked() {
                        self.say(butler::toggle_follower(bot, &account.username));
                        changed = true;
                    }
                } else {
                    let block = if !enabled {
                        Some("Butler is disabled".to_owned())
                    } else if let Some(master) = &following {
                        Some(format!("You're following {master}: stop before summoning anyone"))
                    } else {
                        butler::unavailable(bot, account).map(|why| format!("{} {why}", account.username))
                    };
                    let response = widgets::button(
                        ui,
                        "Summon",
                        Kind::Primary,
                        block.is_none(),
                        size,
                        block.as_deref().unwrap_or_default(),
                    );
                    if response
                        .on_hover_text(format!("Have {} follow me", account.username))
                        .clicked()
                    {
                        self.say(butler::toggle_follower(bot, &account.username));
                        changed = true;
                    }
                }
                // Follow / Stop.
                if am_following {
                    if widgets::button(ui, "Stop", Kind::Danger, true, size, "").clicked() {
                        butler::stop_follow(bot);
                        self.say(Ok(format!("Stopped following {}.", account.username)));
                        changed = true;
                    }
                } else {
                    let block = if !enabled {
                        Some("Butler is disabled".to_owned())
                    } else if bot.scripts.is_running() {
                        Some("A script is running here: stop it first".to_owned())
                    } else if let Some(master) = &following {
                        Some(format!("You're following {master}: stop first"))
                    } else if summoned {
                        Some(format!("Release {} before following them", account.username))
                    } else if follows_me {
                        Some(format!("{} follows you", account.username))
                    } else if follows_other {
                        Some(format!("{} follows {}", account.username, account.following))
                    } else if other_server {
                        Some(format!("{} is on {}", account.username, account.server))
                    } else {
                        None
                    };
                    let response = widgets::button(
                        ui,
                        "Follow",
                        Kind::Normal,
                        block.is_none(),
                        size,
                        block.as_deref().unwrap_or_default(),
                    );
                    if response.on_hover_text(format!("Follow {}", account.username)).clicked() {
                        let master = account.username.clone();
                        let message = self.message.clone();
                        crate::turu::spawn_action("Butler Follow", move |bot| {
                            let result = butler::start_follow(bot, &master);
                            let text = match &result {
                                Ok(()) => (format!("Following {master}."), false),
                                Err(e) => (e.clone(), true),
                            };
                            *message.lock().unwrap_or_else(|e| e.into_inner()) = Some(text);
                            Ok(())
                        });
                        changed = true;
                    }
                }
            });
        });
        changed
    }

    /// The online count, Summon all and Settings.
    fn footer(&mut self, ui: &mut Ui, bot: &Arc<Bot>) -> bool {
        let butler = &bot.butler;
        let enabled = butler.enabled();
        let following = butler.following();
        let mut changed = false;
        ui.horizontal(|ui| {
            let text = if enabled {
                let n = self.roster.len();
                format!("{n} other account{} online", if n == 1 { "" } else { "s" })
            } else {
                "Butler is disabled: turn it on in Settings".into()
            };
            ui.label(RichText::new(text).small().color(if enabled { MUTED } else { DANGER }));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if self.show_settings {
                    "Hide Settings"
                } else {
                    "Settings"
                };
                if widgets::button(ui, label, Kind::Normal, true, vec2(0.0, 26.0), "").clicked() {
                    self.show_settings = !self.show_settings;
                }
                let block = if !enabled {
                    "Butler is disabled"
                } else if following.is_some() {
                    "You're following someone: stop first"
                } else {
                    ""
                };
                let summon = widgets::button(
                    ui,
                    "Summon all",
                    Kind::Primary,
                    block.is_empty(),
                    vec2(0.0, 26.0),
                    block,
                );
                if summon
                    .on_hover_text("Have every available account on this server follow this one")
                    .clicked()
                {
                    self.say(butler::summon_all(bot));
                    changed = true;
                }
            });
        });
        changed
    }
}

/// A small outlined label next to a name.
fn chip(ui: &mut Ui, text: &str, color: Color32) {
    widgets::pill(ui, |ui| ui.label(RichText::new(text).size(10.0).color(color)));
}

/// Butler's options (shared by every client, saved at once) and this account's own.
fn settings_card(ui: &mut Ui, bot: &Arc<Bot>) {
    let butler = &bot.butler;
    let mut settings = butler.settings();
    let before = settings.clone();
    let me = butler.username();
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::section_label(ui, "SETTINGS (EVERY CLIENT)");
        row(
            ui,
            "Enable Butler",
            "Off: no client broadcasts, follows or summons. An active follow stops where it stands.",
            |ui| widgets::switch(ui, &mut settings.enabled),
        );
        row(
            ui,
            "Only attack when the master attacks",
            "Off: always fight beside the master.",
            |ui| widgets::switch(ui, &mut settings.passive_attack),
        );
        row(
            ui,
            "Leech mode",
            "Stay in the master's room at the Enter cell and never fight: for dragging a weak alt along.",
            |ui| widgets::switch(ui, &mut settings.leech_mode),
        );
        row(
            ui,
            "Always goto the master",
            "Keep sending goto while already in the master's room: fixes a butler stuck in the wrong cell \
             after dying, at the cost of a goto every Goto delay.",
            |ui| widgets::switch(ui, &mut settings.always_goto),
        );
        row(
            ui,
            "Goto delay",
            "Time between gotos while chasing the master (250 ms is the loop's floor).",
            |ui| {
                ui.add(
                    DragValue::new(&mut settings.goto_delay)
                        .range(250..=10_000)
                        .speed(10)
                        .suffix(" ms"),
                )
            },
        );
        row(
            ui,
            "Failed gotos before rescue",
            "Gotos that don't reach the master before joining its room from its file (how locked maps \
             are entered). Lower reacts faster but can trigger on map-change lag.",
            |ui| ui.add(DragValue::new(&mut settings.rescue_threshold).range(1..=10)),
        );
        row(
            ui,
            "Park location",
            "Where a butler goes when it's released or its master goes offline. Stay can leave it in a boss \
             room taking hits.",
            |ui| {
                ComboBox::from_id_salt("turu_butler_park")
                    .selected_text(park_label(settings.park))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        for spot in [ParkSpot::House, ParkSpot::Whitemap, ParkSpot::Stay] {
                            ui.selectable_value(&mut settings.park, spot, park_label(spot));
                        }
                    })
                    .response
            },
        );

        ui.add_space(4.0);
        widgets::section_label(ui, "THIS ACCOUNT");
        if me.is_empty() {
            ui.label(RichText::new("Log in to set this account's options.").color(MUTED));
        } else {
            let mut account = settings.account(&me);
            row(
                ui,
                "Class type",
                "The CoreBots class (Options > CoreBots) this account equips when it starts following. None \
                 keeps the current gear.",
                |ui| {
                    ComboBox::from_id_salt("turu_butler_class")
                        .selected_text(format!("{:?}", account.class_type))
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for class in [
                                ClassType::None,
                                ClassType::Farm,
                                ClassType::Solo,
                                ClassType::Dodge,
                                ClassType::Boss,
                            ] {
                                ui.selectable_value(&mut account.class_type, class, format!("{class:?}"));
                            }
                        })
                        .response
                },
            );
            row(
                ui,
                "Obey summons",
                "Off: this account ignores every summon. Turn it off on the account you play by hand.",
                |ui| widgets::switch(ui, &mut account.obey_summons),
            );
            settings.set_account(&me, account);
        }
    });
    if settings != before
        && let Err(e) = butler.set_settings(settings)
    {
        bot.logs.debug.push(format!("Couldn't save the Butler settings: {e}"));
    }
}

/// A label (with its description on hover) and its control on the right.
fn row(ui: &mut Ui, label: &str, help: &str, control: impl FnOnce(&mut Ui) -> egui::Response) {
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        ui.label(RichText::new(label).color(TEXT)).on_hover_text(help);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            control(ui).on_hover_text(help);
        });
    });
}

fn park_label(spot: ParkSpot) -> &'static str {
    match spot {
        ParkSpot::House => "House",
        ParkSpot::Whitemap => "Whitemap",
        ParkSpot::Stay => "Stay",
    }
}
