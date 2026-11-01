//! Plugins > Butler (VibeSkua ButlerV5's roster and settings windows): the other online
//! accounts running Butler, with Follow and Summon, Butler's options and its debug log.
//!
//! The roster is read from the sync files once a second. Summon and release only change this
//! client's state; Follow waits for a previous follow to end, so it runs off the UI thread.
//! [`show_suspend_notice`] asks what to do when a script starts while following.

use super::theme::{DANGER, EDGE, INSET, MUTED, SMALL_RADIUS, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use egui::{
    Align, Color32, ComboBox, Context, CornerRadius, DragValue, Frame, Label, Layout, Margin, Modal, RichText,
    ScrollArea, Stroke, TextStyle, TextWrapMode, Ui, vec2,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use turu_bot::Bot;
use turu_bot::butler::debug_log::{Level, LogLine};
use turu_bot::butler::sync::SyncData;
use turu_bot::butler::{self, ClassType, FollowSource, ParkSpot, SuspendNotice};

/// How often the roster is read again.
const REFRESH: Duration = Duration::from_secs(1);

#[derive(Default)]
pub struct ButlerWindow {
    roster: Vec<SyncData>,
    read_at: Option<Instant>,
    show_settings: bool,
    show_log: bool,
    /// The log lines shown, and the log's version they were read at.
    log: Vec<LogLine>,
    log_version: Option<u64>,
    /// The last action's outcome (set from the Follow thread too) and whether it failed.
    message: Arc<Mutex<Option<(String, bool)>>>,
}

impl ButlerWindow {
    /// With Settings and the debug log open.
    #[cfg(test)]
    pub(super) fn expanded() -> Self {
        Self {
            show_settings: true,
            show_log: true,
            ..Self::default()
        }
    }

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
        if self.show_log {
            self.log_card(ui, bot);
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
                .max_height((ui.available_height() - 60.0).max(60.0))
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
                let label = if self.show_log { "Hide Log" } else { "Debug Log" };
                if widgets::button(ui, label, Kind::Normal, true, vec2(0.0, 26.0), "")
                    .on_hover_text("Butler's diagnostics: what the broadcast, orders and follower did")
                    .clicked()
                {
                    self.show_log = !self.show_log;
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

impl ButlerWindow {
    /// Butler's debug log (VibeSkua ButlerV5's log viewer), newest at the bottom, with Copy,
    /// Save, Clear and Open folder.
    fn log_card(&mut self, ui: &mut Ui, bot: &Arc<Bot>) {
        let butler = &bot.butler;
        let version = butler.log_version();
        if self.log_version != Some(version) {
            self.log = butler.log_lines();
            self.log_version = Some(version);
        }
        let enabled = butler.settings().debug_logs;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "DEBUG LOG");
                widgets::count_badge(ui, self.log.len());
                if !enabled {
                    ui.label(
                        RichText::new("off: turn on Debug logs in Settings")
                            .small()
                            .color(WARNING),
                    );
                }
            });
            Frame::new()
                .fill(INSET)
                .stroke(Stroke::new(1.0, EDGE))
                .corner_radius(CornerRadius::same(SMALL_RADIUS))
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    let row_height = ui.text_style_height(&TextStyle::Monospace) + 2.0;
                    ui.set_width(ui.available_width());
                    ScrollArea::both()
                        .id_salt("turu_butler_log")
                        .auto_shrink([false, false])
                        .max_height(220.0)
                        .stick_to_bottom(true)
                        .show_rows(ui, row_height, self.log.len(), |ui, range| {
                            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                for line in &self.log[range] {
                                    ui.add(
                                        Label::new(
                                            RichText::new(&line.text).monospace().color(level_color(line.level)),
                                        )
                                        .wrap_mode(TextWrapMode::Extend)
                                        .selectable(true),
                                    );
                                }
                            });
                        });
                });
            ui.horizontal(|ui| {
                let size = vec2(0.0, 26.0);
                let any = !self.log.is_empty();
                if widgets::button(ui, "Copy", Kind::Normal, any, size, "")
                    .on_hover_text("Copy every log line to the clipboard")
                    .clicked()
                {
                    ui.ctx().copy_text(butler.log_text());
                    self.say(Ok(format!("Copied {} log lines.", self.log.len())));
                }
                if widgets::button(ui, "Save", Kind::Normal, any, size, "")
                    .on_hover_text("Write the log to a new file in the butler_logs folder")
                    .clicked()
                {
                    self.say(
                        butler
                            .save_log()
                            .map(|path| format!("Saved the log to {}", path.display()))
                            .map_err(|e| format!("Couldn't save the log: {e}")),
                    );
                }
                if widgets::button(ui, "Clear", Kind::Normal, any, size, "")
                    .on_hover_text("Empty the log")
                    .clicked()
                {
                    butler.clear_log();
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, "Open folder", Kind::Normal, true, size, "")
                        .on_hover_text("Open the butler_logs folder (saved and auto-saved logs)")
                        .clicked()
                    {
                        match std::fs::create_dir_all(&butler.logs_dir) {
                            Ok(()) => crate::mugetsu::open_folder(&butler.logs_dir),
                            Err(e) => self.say(Err(format!("Couldn't make {}: {e}", butler.logs_dir.display()))),
                        }
                    }
                });
            });
        });
    }
}

fn level_color(level: Level) -> Color32 {
    match level {
        Level::Info => TEXT,
        Level::Success => SUCCESS,
        Level::Warning => WARNING,
        Level::Error => DANGER,
    }
}

/// "A script started while following" (VibeSkua ButlerV5 `ShowSuspendNotice`), for the active
/// tab, over the other windows: Yes keeps the follow suspended until the script stops (as does
/// closing the box), No stops following.
pub fn show_suspend_notice(ctx: &Context) {
    let bot = crate::turu::bot();
    // One question at a time: the script's own box first.
    if bot.scripts.message_box().is_some() {
        return;
    }
    let Some(notice) = bot.butler.suspend_notice() else {
        return;
    };
    let mut answer = None;
    let response = Modal::new(egui::Id::new(("turu_butler_notice", notice.id))).show(ctx, |ui| {
        ui.set_min_width(300.0);
        ui.set_max_width(420.0);
        ui.heading(SuspendNotice::CAPTION);
        ui.label(notice.message());
        ui.add_space(4.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::button(ui, "Yes", Kind::Primary, true, vec2(72.0, 28.0), "").clicked() {
                answer = Some(true);
            }
            if widgets::button(ui, "No", Kind::Normal, true, vec2(72.0, 28.0), "").clicked() {
                answer = Some(false);
            }
        });
    });
    if answer.is_none() && response.should_close() {
        answer = Some(true);
    }
    if let Some(keep_suspended) = answer {
        // Only flags: no game call, no wait.
        butler::answer_suspend(&bot, notice.id, keep_suspended);
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
        widgets::section_label(ui, "WHILE FOLLOWING");
        row(
            ui,
            "Quest bypasses",
            "When the butler lands on a map with quest-locked cells or monsters, fake the gate quest on this \
             client so the cell opens and the monsters can be targeted. Only on maps in the built-in table; the \
             server is never told anything.",
            |ui| widgets::switch(ui, &mut settings.quest_bypass),
        );
        row(
            ui,
            "Custom bypass quest IDs",
            "Extra quest IDs to fake when the follow starts, separated by commas (1234, 5678). For a map whose \
             cells stay locked for butlers and isn't in the built-in table yet. Empty: none.",
            |ui| widgets::text_box(ui, &mut settings.quest_bypass_custom, "1234, 5678", 140.0),
        );
        row(
            ui,
            "Unlock all quests (experimental)",
            "Sets every quest slot to the maximum on this client, so every quest-locked cell, monster and map \
             opens without a bypass list. Overrides Quest bypasses while on. Client side only and undone by a \
             relogin, but it scrambles this account's quest log until then: avoid it on an account you play by \
             hand.",
            |ui| widgets::switch(ui, &mut settings.quest_unlock_all),
        );
        row(
            ui,
            "Fake level 100",
            "Shows this client as level 100 so level-gated maps (icestormunder) let the butler in. Client side \
             only, but your character shows as level 100 to you until you relog.",
            |ui| widgets::switch(ui, &mut settings.fake_level),
        );
        row(
            ui,
            "AntiLag while summoned",
            "Turn on AntiLag (Lag Killer, the Lite animation cuts, hidden monsters) while this account is \
             summoned: good for background alts. Your own settings come back when the follow ends.",
            |ui| widgets::switch(ui, &mut settings.anti_lag_summoned),
        );
        row(
            ui,
            "AntiLag while following",
            "The same, but only while following by hand (Follow in this window), not when summoned.",
            |ui| widgets::switch(ui, &mut settings.anti_lag_following),
        );
        row(
            ui,
            "Debug logs",
            "Record Butler's diagnostics in the debug log (Debug Log below). Off: nothing is recorded.",
            |ui| widgets::switch(ui, &mut settings.debug_logs),
        );
        row(
            ui,
            "Auto-save logs",
            "Also append every log line to a file in the butler_logs folder as it happens, one file per account.",
            |ui| widgets::switch(ui, &mut settings.auto_save_log),
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
