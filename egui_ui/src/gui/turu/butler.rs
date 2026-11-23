//! Plugins > Butler (VibeSkua ButlerV5's roster and settings windows): the other online
//! accounts running Butler, with Follow and Summon, Butler's options and its debug log.
//!
//! The roster is read from the sync files once a second. Summon and release only change this
//! client's state; Follow waits for a previous follow to end, and Send to player calls the
//! game, so both run off the UI thread.
//! [`show_suspend_notice`] asks what to do when a script starts while following.
//!
//! On a touch screen (Android) it's a full-screen sheet when the window would not fit, with
//! finger-sized Follow/Summon buttons and roomier roster rows.

use super::theme::{DANGER, EDGE, INSET, MUTED, SMALL_RADIUS, SUCCESS, TEXT, WARNING};
use super::widgets::{self, Kind};
use crate::models::butler as model;
use crate::models::form::{KindView, OptionValue};
use crate::models::scheduler::Tint;
use egui::{
    Align, Color32, ComboBox, Context, CornerRadius, Frame, Key, Label, Layout, Margin, Modal, Popup,
    PopupCloseBehavior, RectAlign, RichText, ScrollArea, Stroke, TextEdit, TextStyle, TextWrapMode, Ui, vec2,
};
use std::sync::Arc;
use turu_bot::Bot;
use turu_bot::butler::debug_log::{Level, LogLine};
use turu_bot::butler::sync::SyncData;
use turu_bot::butler::{self, SuspendNotice};

/// The height of Butler's buttons: finger-sized on a touch screen.
fn control_height() -> f32 {
    widgets::target(26.0, 40.0)
}

fn tint_color(tint: Tint) -> Color32 {
    match tint {
        Tint::Plain => TEXT,
        Tint::Muted => MUTED,
        Tint::Success => SUCCESS,
        Tint::Warning => WARNING,
        Tint::Danger => DANGER,
    }
}

/// The window's own state; what Butler shows and decides is `models::butler`, shared with a
/// native page.
#[derive(Default)]
pub struct ButlerWindow {
    roster: Vec<SyncData>,
    show_settings: bool,
    show_log: bool,
    /// The log lines shown, the log's version they were read at, and Streamer Mode's names
    /// they were masked with.
    log: Vec<LogLine>,
    log_version: Option<u64>,
    log_masked: Option<model::Masks>,
    /// Send to player: the pop-up is open, its name box, and the recent names shown.
    send_open: bool,
    send_target: String,
    recent_targets: Vec<String>,
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
        self.roster = model::roster(&bot, false);
        let mut window = super::window(ctx, "Butler", [520.0, 560.0], 6).open(open);
        // On a phone a floating window would cover the game and still be cramped: a sheet.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 560.0 || area.height() < 560.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui, &bot));
        ctx.request_repaint_after(model::REFRESH);
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
            self.roster = model::roster(bot, true);
        }
    }

    /// This account, whom it follows, and Stop following / Release all.
    fn header_card(&self, ui: &mut Ui, bot: &Arc<Bot>) -> bool {
        let header = model::header(bot, model::masks(bot).as_ref());
        let mut changed = false;
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    let name = if header.me.is_empty() {
                        "(not logged in)"
                    } else {
                        header.me.as_str()
                    };
                    ui.label(RichText::new(name).size(16.0).strong().color(TEXT));
                    ui.label(RichText::new(&header.state).size(12.0).color(tint_color(header.tint)));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let control = vec2(0.0, control_height());
                    if header.summoning
                        && widgets::button(ui, "Release all", Kind::Normal, header.enabled, control, "")
                            .on_hover_text("Clear the summon list; released butlers go to their park spot")
                            .clicked()
                    {
                        butler::release_all(bot);
                        model::say(bot, Ok("Released all followers.".into()));
                        changed = true;
                    }
                    if header.following
                        && widgets::button(ui, "Stop following", Kind::Danger, true, control, "").clicked()
                    {
                        butler::stop_follow(bot);
                        model::say(bot, Ok("Stopped following.".into()));
                        changed = true;
                    }
                });
            });
            if let Some(note) = model::message(bot) {
                ui.label(RichText::new(note.text).small().color(tint_color(note.tint)));
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
                    // Roomier rows for a finger.
                    let separator = if widgets::touch() { 14.0 } else { 6.0 };
                    let masks = model::masks(bot);
                    for (i, account) in self.roster.iter().enumerate() {
                        if i > 0 {
                            ui.add(egui::Separator::default().spacing(separator));
                        }
                        changed |= row(ui, bot, &model::account(bot, account, masks.as_ref()));
                    }
                });
        });
        changed
    }

    /// The online count on the left; Send to player, Summon all, Settings and the debug log
    /// on the right, the count on its own line above them when one row is too narrow
    /// (VibeSkua's `FooterBar`).
    fn footer(&mut self, ui: &mut Ui, bot: &Arc<Bot>) -> bool {
        let enabled = bot.butler.enabled();
        let text = if enabled {
            let n = self.roster.len();
            format!("{n} other account{} online", if n == 1 { "" } else { "s" })
        } else {
            "Butler is disabled: turn it on in Settings".into()
        };
        let tip = if enabled {
            format!("The list is read again every {}s.", model::REFRESH.as_secs())
        } else {
            "Open Settings and turn on Enable Butler.".to_owned()
        };
        let status = RichText::new(text).small().color(if enabled { MUTED } else { DANGER });
        let mut changed = false;
        if ui.available_width() < 480.0 {
            ui.label(status).on_hover_text(tip);
            ui.with_layout(Layout::right_to_left(Align::Center).with_main_wrap(true), |ui| {
                changed |= self.footer_buttons(ui, bot);
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(status).on_hover_text(tip);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    changed |= self.footer_buttons(ui, bot);
                });
            });
        }
        changed
    }

    /// The footer's buttons, right to left.
    fn footer_buttons(&mut self, ui: &mut Ui, bot: &Arc<Bot>) -> bool {
        let enabled = bot.butler.enabled();
        let mut changed = false;
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
        let rule = model::summon_all_control(bot);
        let summon = widgets::button(
            ui,
            "Summon all",
            Kind::Primary,
            rule.enabled,
            vec2(0.0, 26.0),
            &rule.reason,
        );
        if summon
            .on_hover_text("Have every available account on this server follow this one")
            .clicked()
        {
            model::say(bot, butler::summon_all(bot));
            changed = true;
        }
        self.send_to_player(ui, bot, enabled);
        changed
    }

    /// "Send to player" and its pop-up (VibeSkua's flyout): a name box (Enter sends), Send, and
    /// the recent names to pick from.
    fn send_to_player(&mut self, ui: &mut Ui, bot: &Arc<Bot>, enabled: bool) {
        let tip = if enabled {
            "Send this account and every available butler to another player."
        } else {
            "Butler is disabled. Turn it on in Settings first."
        };
        let button = widgets::button(ui, "Send to player", Kind::Normal, enabled, vec2(0.0, 26.0), tip);
        if button.clicked() {
            self.send_open = !self.send_open;
            if self.send_open {
                // Other players' names: none to pick from in Streamer Mode.
                self.recent_targets = if bot.opts().streamer_mode {
                    Vec::new()
                } else {
                    bot.butler.recent_targets()
                };
            }
        }
        if !enabled {
            self.send_open = false;
        }
        let mut send = false;
        let width = (ui.ctx().content_rect().width() - 32.0).min(340.0);
        Popup::from_response(&button)
            .open_bool(&mut self.send_open)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .align(RectAlign::TOP_END)
            .width(width)
            .show(|ui| {
                ui.set_width(width);
                ui.label(
                    RichText::new("/goto a player with this account and every available butler.")
                        .small()
                        .color(MUTED),
                );
                ui.horizontal(|ui| {
                    let height = widgets::target(26.0, 40.0);
                    let send_width = 64.0;
                    let hint = "Player name (not one of your butlers)";
                    let mut edit = ui.add(
                        TextEdit::singleline(&mut self.send_target)
                            .hint_text(hint)
                            .desired_width(ui.available_width() - send_width - 8.0)
                            .min_size(vec2(0.0, height)),
                    );
                    let kind = super::native_field::Kind::Line;
                    super::native_field::follow(ui, &mut edit, &mut self.send_target, hint, kind);
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        send = true;
                    }
                    if widgets::button(ui, "Send", Kind::Primary, true, vec2(send_width, height), "").clicked() {
                        send = true;
                    }
                });
                if !self.recent_targets.is_empty() {
                    ui.label(RichText::new("Recent").small().color(MUTED));
                    ui.horizontal_wrapped(|ui| {
                        for name in self.recent_targets.iter().take(8) {
                            if widgets::small_button(ui, name, true)
                                .on_hover_text(format!("Use {name}."))
                                .clicked()
                            {
                                self.send_target = name.clone();
                            }
                        }
                    });
                }
            });
        if send {
            let target = self.send_target.trim().to_owned();
            if !target.is_empty() {
                self.send_open = false;
            }
            model::send_to_player(bot, target);
        }
    }
}

/// A roster row: the name and its chips, where it is, then Summon/Release and Follow/Stop.
fn row(ui: &mut Ui, bot: &Arc<Bot>, account: &model::ButlerAccount) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&account.label).strong().color(TEXT));
                for c in &account.chips {
                    chip(ui, &c.text, tint_color(c.tint));
                }
            });
            ui.label(
                RichText::new(&account.place)
                    .size(widgets::small_label_size())
                    .color(MUTED),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let size = if widgets::touch() {
                vec2(84.0, control_height())
            } else {
                vec2(72.0, 26.0)
            };
            if account.summoned {
                if widgets::button(ui, "Release", Kind::Normal, true, size, "").clicked() {
                    model::say(bot, butler::toggle_follower(bot, &account.username));
                    changed = true;
                }
            } else {
                let rule = &account.summon;
                let response = widgets::button(ui, "Summon", Kind::Primary, rule.enabled, size, &rule.reason);
                if response
                    .on_hover_text(format!("Have {} follow me", account.label))
                    .clicked()
                {
                    model::say(bot, butler::toggle_follower(bot, &account.username));
                    changed = true;
                }
            }
            if account.followed {
                if widgets::button(ui, "Stop", Kind::Danger, true, size, "").clicked() {
                    butler::stop_follow(bot);
                    model::say(bot, Ok(format!("Stopped following {}.", account.label)));
                    changed = true;
                }
            } else {
                let rule = &account.follow;
                let response = widgets::button(ui, "Follow", Kind::Normal, rule.enabled, size, &rule.reason);
                if response.on_hover_text(format!("Follow {}", account.label)).clicked() {
                    model::follow(bot, account.username.clone());
                    changed = true;
                }
            }
        });
    });
    changed
}

impl ButlerWindow {
    /// Butler's debug log (VibeSkua ButlerV5's log viewer), newest at the bottom, with Copy,
    /// Save, Clear and Open folder.
    fn log_card(&mut self, ui: &mut Ui, bot: &Arc<Bot>) {
        let butler = &bot.butler;
        let version = butler.log_version();
        let masks = model::masks(bot);
        if self.log_version != Some(version) || self.log_masked != masks {
            self.log = butler.log_lines();
            if let Some(masks) = &masks {
                for line in &mut self.log {
                    line.text = masks.text(&line.text);
                }
            }
            self.log_version = Some(version);
            self.log_masked = masks;
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
                    model::say(bot, Ok(format!("Copied {} log lines.", self.log.len())));
                }
                if widgets::button(ui, "Save", Kind::Normal, any, size, "")
                    .on_hover_text("Write the log to a new file in the butler_logs folder")
                    .clicked()
                {
                    model::say(
                        bot,
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
                            Ok(()) => crate::host::host().open_folder(&butler.logs_dir),
                            Err(e) => model::say(bot, Err(format!("Couldn't make {}: {e}", butler.logs_dir.display()))),
                        }
                    }
                });
            });
        });
    }
}

fn level_color(level: Level) -> Color32 {
    tint_color(model::level_tint(level))
}

/// "A script started while following" (VibeSkua ButlerV5 `ShowSuspendNotice`), for the active
/// tab, over the other windows: Yes keeps the follow suspended until the script stops (as does
/// closing the box), No stops following.
pub fn show_suspend_notice(ctx: &Context) {
    let bot = crate::turu::bot();
    // One question at a time: the script's own box first; a native shell shows it itself.
    if bot.scripts.message_box().is_some() || crate::models::prompts::native_shown() {
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

/// Butler's options (shared by every client, saved at once) and this account's own, from the
/// model's option rows (a native page shows the same).
fn settings_card(ui: &mut Ui, bot: &Arc<Bot>) {
    let butler = &bot.butler;
    let me = butler.username();
    let options = model::options(&butler.settings(), &me);
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        for section in model::sections() {
            if section.chip != model::EVERY_CLIENT {
                ui.add_space(4.0);
            }
            widgets::section_label(ui, &section.title.to_uppercase());
            if section.chip == model::THIS_ACCOUNT && me.is_empty() {
                ui.label(RichText::new("Log in to set this account's options.").color(MUTED));
            }
            for o in options.iter().filter(|o| o.section == section.chip) {
                let set = |value: serde_json::Value| {
                    if let Err(e) = model::set(bot, o.label, &value) {
                        bot.logs.debug.push(e);
                    }
                };
                row_ui(ui, o.label, o.description, |ui| match (&o.kind, &o.value) {
                    (KindView::Switch, OptionValue::Switch(on)) => {
                        let mut now = *on;
                        let response = widgets::switch(ui, &mut now);
                        if now != *on {
                            set(serde_json::json!(now));
                        }
                        response
                    }
                    (KindView::Number { min, max, unit }, OptionValue::Number(n)) => {
                        let mut now = *n;
                        let size = vec2(80.0, widgets::target(24.0, 40.0));
                        let max = max.map_or(i64::MAX, i64::from);
                        let unit = unit.map(|u| format!(" {u}")).unwrap_or_default();
                        let response =
                            widgets::number(ui, o.label, &mut now, i64::from(*min)..=max, 10.0, unit.as_str(), size);
                        if now != *n {
                            set(serde_json::json!(now));
                        }
                        response
                    }
                    (KindView::Choice { choices }, OptionValue::Text(key)) => {
                        let mut picked = None;
                        let response = ComboBox::from_id_salt(("turu_butler_choice", o.label))
                            .selected_text(key.as_str())
                            .width(120.0)
                            .show_ui(ui, |ui| {
                                for (k, words) in choices {
                                    if ui.selectable_label(k == key, words).clicked() && k != key {
                                        picked = Some(k.clone());
                                    }
                                }
                            })
                            .response;
                        if let Some(k) = picked {
                            set(serde_json::json!(k));
                        }
                        response
                    }
                    (KindView::Text { hint }, OptionValue::Text(text)) => {
                        let mut now = text.clone();
                        let response = widgets::text_box(ui, &mut now, hint, 140.0);
                        if now != *text {
                            set(serde_json::json!(now));
                        }
                        response
                    }
                    _ => ui.label(""),
                });
            }
        }
    });
}

/// A label (with its description on hover) and its control on the right.
fn row_ui(ui: &mut Ui, label: &str, help: &str, control: impl FnOnce(&mut Ui) -> egui::Response) {
    ui.horizontal(|ui| {
        ui.set_min_height(26.0);
        ui.label(RichText::new(label).color(TEXT)).on_hover_text(help);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            control(ui).on_hover_text(help);
        });
    });
}
