//! Army > Accounts (VibeSkua Manager's Accounts tab): the account list, groups, and launching
//! accounts, each in its own `turu` process (`turu_bot::army`). Passwords are kept in the
//! system keychain; reading and writing them happens off the UI thread.

use super::files::{self, Pending};
use super::theme::{ACCENT, DANGER, MUTED, SUCCESS, WARNING};
use super::widgets::{self, Kind};
use crate::platform::FileDialog;
use crate::turu::TabRequest;
use egui::{Align, ComboBox, Context, Label, Layout, Modal, RichText, ScrollArea, Ui, vec2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use turu_bot::accounts::{self, Account, Accounts, LaunchMode};
use turu_bot::army::Launch;
use turu_bot::startup::Startup;

/// How an account's last launch went, shown on its row until it's launched again.
#[derive(Clone, PartialEq)]
enum Launched {
    /// Waiting its turn in the stagger, or its client is starting.
    Starting,
    Started,
    /// Started without a saved password: it waits at the login screen.
    NeedsLogin,
    Failed(String),
}

/// The accounts of the last Start, in order, and how each went (set by the launch threads).
#[derive(Default)]
struct Launches {
    batch: Vec<String>,
    rows: HashMap<String, Launched>,
}

impl Launches {
    fn set(&mut self, username: &str, state: Launched) {
        self.rows.insert(username.to_owned(), state);
    }

    /// The last Start in a line: "Starting 3 of 10…", then "Started 8 of 10 · 2 need a login ·
    /// 1 failed".
    fn summary(&self) -> Option<String> {
        if self.batch.is_empty() {
            return None;
        }
        let count = |pick: fn(&Launched) -> bool| {
            self.batch
                .iter()
                .filter(|u| self.rows.get(*u).is_some_and(pick))
                .count()
        };
        let total = self.batch.len();
        let starting = count(|l| *l == Launched::Starting);
        let needs_login = count(|l| *l == Launched::NeedsLogin);
        let failed = count(|l| matches!(l, Launched::Failed(_)));
        let started = count(|l| *l == Launched::Started) + needs_login;
        if starting > 0 {
            return Some(format!("Starting {} of {total}…", total - starting + 1));
        }
        let mut line = format!("Started {started} of {total}");
        if needs_login > 0 {
            line.push_str(&format!(" · {needs_login} need a login"));
        }
        if failed > 0 {
            line.push_str(&format!(" · {failed} failed"));
        }
        Some(line)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Start Selected and Start All ask first when they would start more accounts than this.
const START_ASKS_ABOVE: usize = 3;

/// In the password field's place where nothing keeps passwords.
const NO_PASSWORDS: &str = "Passwords can't be saved here yet: log in by hand in the account's tab";

/// The password field's Show / Hide button.
const SHOW_WIDTH: f32 = 50.0;

/// The list's height when the window is short: about four rows.
const LIST_MIN_HEIGHT: f32 = 110.0;
/// Room kept under the list's scroll area for the list card's own margin and gaps.
const LIST_SPARE: f32 = 8.0;

/// The add / edit form.
#[derive(Default)]
struct Form {
    /// The username being edited (`None`: adding).
    editing: Option<String>,
    /// Open to add an account ("+ Add account"); editing opens it too.
    open: bool,
    username: String,
    password: String,
    show_password: bool,
    display_name: String,
    tags: String,
}

pub struct AccountsWindow {
    list: Accounts,
    form: Form,
    group_name: String,
    tag_filter: Option<String>,
    /// The username whose Remove is waiting for its question's answer.
    confirm_remove: Option<String>,
    pending_script: Pending,
    /// What the last action did, shown at the bottom (set by other threads too).
    status: Arc<Mutex<Status>>,
    /// Filters the list by name, username or tag (with the tag filter).
    search: String,
    launches: Arc<Mutex<Launches>>,
    /// Where the list's scroll area ended, and the height of what came after it in the window,
    /// on the last frame: the list takes the rest of the window's height.
    list_bottom: f32,
    below_list: f32,
    /// Start Selected or Start All is waiting for its question's answer: these would start.
    confirm_start: Option<Vec<Account>>,
}

impl Default for AccountsWindow {
    fn default() -> Self {
        Self {
            list: Accounts::load(&crate::turu::bot().paths.accounts_file()),
            form: Form::default(),
            group_name: String::new(),
            tag_filter: None,
            confirm_remove: None,
            pending_script: Pending::default(),
            status: Arc::default(),
            search: String::new(),
            launches: Arc::default(),
            confirm_start: None,
            list_bottom: 0.0,
            below_list: 0.0,
        }
    }
}

/// The bottom line: what the last action did, in red when it failed.
#[derive(Default, Clone)]
struct Status {
    text: String,
    error: bool,
}

fn set_status(status: &Mutex<Status>, text: impl Into<String>) {
    *lock(status) = Status {
        text: text.into(),
        error: false,
    };
}

fn set_error(status: &Mutex<Status>, text: impl Into<String>) {
    *lock(status) = Status {
        text: text.into(),
        error: true,
    };
}

impl AccountsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if let Some(path) = self.pending_script.poll() {
            self.list.script = Some(path);
            self.save();
        }
        if !*open {
            return;
        }
        super::window(ctx, "Accounts", [520.0, 560.0], 4)
            .open(open)
            .show(ctx, |ui| self.ui(ui));
        self.start_question(ctx);
        self.confirm_remove(ctx);
        // Launches finish on their own thread.
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn save(&self) {
        if let Err(e) = self.list.save(&crate::turu::bot().paths.accounts_file()) {
            set_error(&self.status, format!("Couldn't save the accounts: {e}"));
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        self.form_card(ui);
        self.list_card(ui);
        self.launch_card(ui);
        let status = lock(&self.status).clone();
        if !status.text.is_empty() {
            let color = if status.error { DANGER } else { MUTED };
            ui.label(RichText::new(status.text).small().color(color));
        }
        // What follows the list, for the next frame's list height.
        self.below_list = (ui.min_rect().bottom() - self.list_bottom).max(0.0);
    }

    /// The add / edit form, behind "+ Add account": you add an account once and launch it every
    /// day. Open while editing, or while there are no accounts yet.
    fn form_card(&mut self, ui: &mut Ui) {
        let shown = self.form.open || self.form.editing.is_some() || self.list.accounts.is_empty();
        if !shown {
            if widgets::button(ui, "+ Add account", Kind::Outline, true, vec2(0.0, 26.0), "").clicked() {
                self.form.open = true;
            }
            return;
        }
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            let editing = self.form.editing.is_some();
            // Adding needs no title (the button names it); editing says whose account it is.
            if let Some(username) = &self.form.editing {
                ui.label(RichText::new(format!("Editing {username}")).strong().size(13.0));
            }
            let half = (ui.available_width() - 6.0) / 2.0;
            // Enter in any field saves, as Add Account / Save Account would.
            let mut enter = false;
            let mut submitted = |response: &egui::Response| {
                enter |= response.lost_focus() && response.ctx.input(|i| i.key_pressed(egui::Key::Enter));
            };
            ui.horizontal(|ui| {
                let username = ui
                    .add_enabled_ui(!editing, |ui| {
                        widgets::text_box(ui, &mut self.form.username, "Username", half)
                    })
                    .inner;
                submitted(&username);
                username.on_disabled_hover_text(
                    "The username is the account's key: to rename it, remove the account and add it again",
                );
                submitted(&widgets::text_box(
                    ui,
                    &mut self.form.display_name,
                    "Display Name (optional)",
                    half,
                ));
            });
            ui.horizontal(|ui| {
                let hint = if editing {
                    "Password (blank: keep it)"
                } else {
                    "Password"
                };
                if accounts::keeps_passwords() {
                    // Show / Hide has a width of its own, so this row's fields line up with the
                    // row above (the password and the button together are as wide as the
                    // username).
                    let gap = ui.spacing().item_spacing.x;
                    let password = ui.add(
                        egui::TextEdit::singleline(&mut self.form.password)
                            .password(!self.form.show_password)
                            .hint_text(hint)
                            .desired_width(half - SHOW_WIDTH - gap)
                            // As tall as the other fields (widgets::text_box) on a touch screen.
                            .min_size(vec2(0.0, widgets::target(0.0, 40.0)))
                            .vertical_align(egui::Align::Center)
                            .margin(egui::Margin::symmetric(8, 5))
                            .background_color(super::theme::INSET),
                    );
                    submitted(&password);
                    let eye = if self.form.show_password { "Hide" } else { "Show" };
                    let size = vec2(SHOW_WIDTH, widgets::target(22.0, 40.0));
                    if widgets::button(ui, eye, Kind::Normal, true, size, "").clicked() {
                        self.form.show_password = !self.form.show_password;
                    }
                } else {
                    // Where nothing keeps passwords (Android, until a Keystore store), no field
                    // that would fail: what to do instead, in its place.
                    ui.add_sized(
                        vec2(half, ui.spacing().interact_size.y),
                        Label::new(RichText::new(NO_PASSWORDS).size(11.0).color(MUTED)).truncate(),
                    )
                    .on_hover_text(NO_PASSWORDS);
                }
                submitted(&widgets::text_box(
                    ui,
                    &mut self.form.tags,
                    "Tags, comma-separated",
                    half,
                ));
            });
            // Adding a username that's saved already would overwrite that account: say so and
            // offer to edit it instead.
            let duplicate = self.duplicate().cloned();
            if let Some(existing) = &duplicate {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} is already in the list", existing.label()))
                            .size(12.0)
                            .color(WARNING),
                    );
                    if ui.link("Edit it").clicked() {
                        self.edit(existing);
                    }
                });
            }
            ui.horizontal(|ui| {
                let can_save = !self.form.username.trim().is_empty() && duplicate.is_none();
                let label = if editing { "Save Account" } else { "Add Account" };
                if widgets::button(
                    ui,
                    label,
                    Kind::Primary,
                    can_save,
                    vec2(120.0, 28.0),
                    if duplicate.is_some() {
                        "That account is saved already: edit it instead"
                    } else {
                        "Enter a username"
                    },
                )
                .clicked()
                    || (enter && can_save)
                {
                    self.submit_form();
                }
                if !self.list.accounts.is_empty()
                    && widgets::button(ui, "Cancel", Kind::Normal, true, vec2(80.0, 28.0), "").clicked()
                {
                    self.form = Form::default();
                }
            });
        });
    }

    /// The saved account the form's username names while adding (ignoring case): adding it
    /// would overwrite that account.
    fn duplicate(&self) -> Option<&Account> {
        if self.form.editing.is_some() {
            return None;
        }
        self.list.find(self.form.username.trim())
    }

    fn submit_form(&mut self) {
        let form = std::mem::take(&mut self.form);
        let username = form.username.trim().to_owned();
        let selected = self.list.find(&username).is_some_and(|a| a.selected);
        let tags = form
            .tags
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect();
        let added = self.list.upsert(Account {
            username: username.clone(),
            display_name: form.display_name.trim().to_owned(),
            tags,
            selected,
        });
        self.save();
        let what = if added { "Added" } else { "Saved" };
        if form.password.is_empty() {
            set_status(&self.status, format!("{what} {username}."));
            return;
        }
        let status = self.status.clone();
        let password = form.password;
        let _ = std::thread::Builder::new().name("Turu Keychain".into()).spawn(move || {
            match accounts::save_password(&username, &password) {
                Ok(()) => set_status(
                    &status,
                    format!("{what} {username}; the password is in the system keychain."),
                ),
                Err(e) => set_error(&status, format!("{what} {username}, but {e}")),
            }
        });
    }

    fn edit(&mut self, account: &Account) {
        self.form = Form {
            editing: Some(account.username.clone()),
            open: true,
            username: account.username.clone(),
            password: String::new(),
            show_password: false,
            display_name: account.display_name.clone(),
            tags: account.tags.join(", "),
        };
    }

    fn list_card(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::section_label(ui, "ACCOUNTS");
                widgets::count_badge(ui, self.list.accounts.len());
                widgets::text_box(ui, &mut self.search, "Search", 120.0)
                    .on_hover_text("Shows the accounts whose name, username or a tag contains this");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let tags = self.list.tags();
                    if !tags.is_empty() {
                        ComboBox::from_id_salt("turu_accounts_tag")
                            .selected_text(self.tag_filter.as_deref().unwrap_or("All tags"))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.tag_filter, None, "All tags");
                                for tag in tags {
                                    let label = tag.clone();
                                    ui.selectable_value(&mut self.tag_filter, Some(tag), label);
                                }
                            });
                    }
                    // Only the rows the tag filter shows: a hidden account isn't started by surprise.
                    let (filter, search) = (self.tag_filter.clone(), self.search.clone());
                    if widgets::small_button(ui, "Unselect All", true).clicked() {
                        let shown = self
                            .list
                            .accounts
                            .iter_mut()
                            .filter(|a| shows(a, filter.as_deref(), &search));
                        shown.for_each(|a| a.selected = false);
                        self.save();
                    }
                    if widgets::small_button(ui, "Select All", true).clicked() {
                        let shown = self
                            .list
                            .accounts
                            .iter_mut()
                            .filter(|a| shows(a, filter.as_deref(), &search));
                        shown.for_each(|a| a.selected = true);
                        self.save();
                    }
                });
            });

            let mut changed = false;
            let mut action: Option<(Row, String)> = None;
            let launches = lock(&self.launches);
            // The height the window leaves once what follows the list (as tall as on the last
            // frame) has its room; at least about four rows.
            let height = (ui.available_height() - self.below_list - LIST_SPARE).max(LIST_MIN_HEIGHT);
            ScrollArea::vertical()
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if self.list.accounts.is_empty() {
                        ui.label(RichText::new("No accounts yet: add one above.").color(MUTED));
                    } else if !self
                        .list
                        .accounts
                        .iter()
                        .any(|a| shows(a, self.tag_filter.as_deref(), &self.search))
                    {
                        ui.label(RichText::new("No account matches the search or the tag filter.").color(MUTED));
                    }
                    let count = self.list.accounts.len();
                    let groups: Vec<Vec<String>> = self
                        .list
                        .accounts
                        .iter()
                        .map(|a| {
                            self.list
                                .groups_of(&a.username)
                                .into_iter()
                                .map(str::to_owned)
                                .collect()
                        })
                        .collect();
                    for (index, account) in self.list.accounts.iter_mut().enumerate() {
                        if !shows(account, self.tag_filter.as_deref(), &self.search) {
                            continue;
                        }
                        let running = is_running(&account.username);
                        let launched = launches.rows.get(&account.username);
                        // The buttons first, on the right; the rest gets the width left and is
                        // cut there, so a long failure can't push Start out of the window.
                        ui.horizontal(|ui| {
                            // Finger-sized on a touch screen: Start, ⋯ and the row itself.
                            if widgets::touch() {
                                ui.spacing_mut().interact_size.y = 40.0;
                                ui.spacing_mut().button_padding = vec2(12.0, 12.0);
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.menu_button("⋯", |ui| {
                                    if let Some(row) = row_menu(ui, index, count) {
                                        action = Some((row, account.username.clone()));
                                    }
                                })
                                .response
                                .on_hover_text("Edit, move, remove");
                                let size = vec2(0.0, widgets::target(22.0, 40.0));
                                if widgets::button(ui, "Start", Kind::Normal, !running, size, "").clicked() {
                                    action = Some((Row::Start, account.username.clone()));
                                }
                                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                    ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
                                    changed |= ui.checkbox(&mut account.selected, "").changed();
                                    if running {
                                        widgets::dot(ui, SUCCESS, 4.0);
                                    } else if launched == Some(&Launched::Starting) {
                                        ui.spinner();
                                    }
                                    let name = ui.add(Label::new(RichText::new(account.label()).strong()).truncate());
                                    if !account.display_name.trim().is_empty() {
                                        name.on_hover_text(&account.username);
                                    }
                                    // How the launch went comes before the tags: it's what a
                                    // narrow row keeps.
                                    launch_note(ui, launched, running, logged_in_tab(&account.username));
                                    let pills = groups[index]
                                        .iter()
                                        .map(|g| (g.as_str(), ACCENT))
                                        .chain(account.tags.iter().map(|t| (t.as_str(), MUTED)));
                                    tag_pills(ui, &pills.collect::<Vec<_>>());
                                });
                            });
                        });
                    }
                });
            drop(launches);
            self.list_bottom = ui.min_rect().bottom();
            if changed {
                self.save();
            }
            if let Some((row, username)) = action {
                self.row_action(row, &username);
            }

            ui.horizontal(|ui| {
                widgets::text_box(ui, &mut self.group_name, "Group name", 150.0);
                let any_selected = self.list.accounts.iter().any(|a| a.selected);
                let add = widgets::button(
                    ui,
                    "Add Selected to Group",
                    Kind::Normal,
                    any_selected && !self.group_name.trim().is_empty(),
                    vec2(0.0, 26.0),
                    "Name a group and check some accounts",
                );
                if add.clicked() {
                    let group = self.group_name.trim().to_owned();
                    let selected: Vec<String> = self
                        .list
                        .accounts
                        .iter()
                        .filter(|a| a.selected)
                        .map(|a| a.username.clone())
                        .collect();
                    for username in selected {
                        self.list.add_to_group(&group, &username);
                    }
                    self.save();
                }
                if !self.list.groups.is_empty() {
                    let mut pick: Option<String> = None;
                    let mut remove: Option<String> = None;
                    ComboBox::from_id_salt("turu_accounts_group")
                        .selected_text("Select a group's accounts")
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            ui.set_min_width(220.0);
                            for group in &self.list.groups {
                                ui.horizontal(|ui| {
                                    let label = format!("{} ({})", group.name, group.accounts.len());
                                    if ui
                                        .selectable_label(false, label)
                                        .on_hover_text("Checks this group's accounts, and only those")
                                        .clicked()
                                    {
                                        pick = Some(group.name.clone());
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::small_button(ui, "Remove", true)
                                            .on_hover_text("Removes the group; its accounts stay")
                                            .clicked()
                                        {
                                            remove = Some(group.name.clone());
                                        }
                                    });
                                });
                            }
                        });
                    if let Some(group) = pick {
                        self.list.select_group(&group);
                        self.save();
                    }
                    if let Some(group) = remove {
                        self.list.remove_group(&group);
                        self.save();
                        set_status(&self.status, format!("Removed the group {group}; its accounts stay."));
                    }
                }
            });
        });
    }

    fn row_action(&mut self, row: Row, username: &str) {
        if !matches!(row, Row::Remove(_)) {
            self.confirm_remove = None;
        }
        match row {
            Row::Start => {
                if let Some(account) = self.list.find(username).cloned() {
                    self.launch(vec![account]);
                }
            }
            Row::Edit => {
                if let Some(account) = self.list.find(username).cloned() {
                    self.edit(&account);
                }
            }
            Row::Shift(by) => {
                self.list.shift(username, by);
                self.save();
            }
            Row::Remove(false) => self.confirm_remove = Some(username.to_owned()),
            Row::Remove(true) => {
                self.confirm_remove = None;
                self.list.remove(username);
                self.save();
                let (status, username) = (self.status.clone(), username.to_owned());
                let _ = std::thread::Builder::new().name("Turu Keychain".into()).spawn(move || {
                    match accounts::delete_password(&username) {
                        Ok(()) => set_status(&status, format!("Removed {username}.")),
                        Err(e) => set_error(&status, format!("Removed {username}, but {e}")),
                    }
                });
            }
        }
    }

    fn launch_card(&mut self, ui: &mut Ui) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "LAUNCH");
            ui.horizontal(|ui| {
                ui.label("Server");
                let bot = crate::turu::bot();
                if let Some(server) = server_status_combo(ui, &bot, &self.list.server) {
                    self.list.server = server;
                    self.save();
                }
                ui.add_space(8.0);
                // Only tabs where Army can't start a `turu` of its own (Android).
                if !crate::platform::platform().launches_processes() {
                    ui.label("Where");
                    ui.label(RichText::new(LaunchMode::Tab.label()).color(MUTED))
                        .on_hover_text("Here accounts start in tabs of this app");
                    return;
                }
                ui.label("Where");
                let before = self.list.mode;
                ComboBox::from_id_salt("turu_accounts_mode")
                    .selected_text(self.list.mode.label())
                    .show_ui(ui, |ui| {
                        for mode in LaunchMode::ALL {
                            ui.selectable_value(&mut self.list.mode, mode, mode.label());
                        }
                    })
                    .response
                    .on_hover_text(
                        "A tab of this window, or a turu process of its own (with a window, or \
                         --headless); Army Control steers them all",
                    );
                if self.list.mode != before {
                    self.save();
                }
            });
            ui.horizontal(|ui| {
                let script = self.list.script.as_ref().map_or_else(
                    || "No script".to_owned(),
                    |p| {
                        p.file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                    },
                );
                let label = ui.label(RichText::new(script).monospace().color(MUTED));
                if let Some(path) = &self.list.script {
                    label.on_hover_text(path.display().to_string());
                }
                // Without file dialogs (Android), the script loaded in this tab instead.
                let platform = crate::platform::platform();
                let loaded = crate::turu::bot().scripts.loaded();
                if !platform.has_file_dialogs() {
                    let use_loaded = widgets::small_button(ui, "Use the loaded script", loaded.is_some())
                        .on_disabled_hover_text("Load one in the Script Loader (Search Scripts) first")
                        .on_hover_text("The script the Script Loader has loaded");
                    if use_loaded.clicked()
                        && let Some(path) = loaded
                    {
                        self.list.script = Some(path);
                        self.save();
                    }
                } else if widgets::small_button(ui, "Browse…", !self.pending_script.is_open()).clicked() {
                    let start = files::scripts_start_dir(crate::turu::bot().paths.scripts_dir());
                    let dialog = FileDialog::new()
                        .set_title("Script for launched accounts")
                        .add_filter("Scripts", &["ts", "js"])
                        .set_directory(start);
                    self.pending_script.pick_file(ui.ctx(), dialog);
                }
                if self.list.script.is_some() && widgets::small_button(ui, "Clear", true).clicked() {
                    self.list.script = None;
                    self.save();
                }
                ui.add_space(8.0);
                let has_script = self.list.script.is_some();
                let auto_start = ui.add_enabled_ui(has_script, |ui| {
                    if widgets::switch(ui, &mut self.list.auto_start).changed() {
                        self.save();
                    }
                    ui.label("Auto-start");
                });
                if !has_script {
                    auto_start
                        .response
                        .on_hover_text("Pick a script first: Auto-start runs it after the login");
                }
            });
            ui.horizontal(|ui| {
                let selected: Vec<Account> = self.list.accounts.iter().filter(|a| a.selected).cloned().collect();
                // Selected accounts the tag filter hides still start: said beside the buttons.
                let hidden = selected
                    .iter()
                    .filter(|a| !shows(a, self.tag_filter.as_deref(), &self.search))
                    .count();
                let selected_count = selected.len();
                let start = widgets::button(
                    ui,
                    "Start Selected",
                    Kind::Primary,
                    !selected.is_empty(),
                    vec2(130.0, 30.0),
                    "Check some accounts",
                );
                if start.clicked() {
                    self.start_or_ask(selected);
                }
                let all = self.list.accounts.clone();
                if widgets::button(ui, "Start All", Kind::Normal, !all.is_empty(), vec2(100.0, 30.0), "").clicked() {
                    self.start_or_ask(all);
                }
                // Beside the buttons: it's about what Start Selected would start.
                if hidden > 0 {
                    let text = format!("{selected_count} selected · {hidden} hidden by the filter");
                    ui.add(Label::new(RichText::new(text).size(12.0).color(WARNING)).truncate())
                        .on_hover_text("Start Selected starts them too; clear the search and set the filter to All tags to see them");
                }
            });
            // Under the buttons, on a line of its own, so the warning above never hides it.
            if let Some(summary) = lock(&self.launches).summary() {
                ui.add(Label::new(RichText::new(summary).size(12.0).color(MUTED)).truncate());
            }
        });
    }

    /// Start Selected and Start All: asks first when more than a few accounts would start.
    fn start_or_ask(&mut self, accounts: Vec<Account>) {
        let starting = accounts.iter().filter(|a| !is_running(&a.username)).count();
        if starting > START_ASKS_ABOVE {
            self.confirm_start = Some(accounts);
        } else {
            self.launch(accounts);
        }
    }

    /// Launches the accounts that aren't running, staggered, with their saved passwords.
    fn launch(&self, accounts: Vec<Account>) {
        let army = crate::turu::army().clone();
        let accounts: Vec<Account> = accounts.into_iter().filter(|a| !is_running(&a.username)).collect();
        if accounts.is_empty() {
            set_status(&self.status, "Those accounts are running already.");
            return;
        }
        let list = &self.list;
        let template = Launch {
            server: Some(list.server.trim().to_owned()).filter(|s| !s.is_empty()),
            script: list.script.clone(),
            run_script: list.auto_start,
            headless: list.mode == LaunchMode::Headless,
            ..Default::default()
        };
        let in_tabs = list.mode == LaunchMode::Tab || !crate::platform::platform().launches_processes();
        {
            let mut launches = lock(&self.launches);
            launches.batch = accounts.iter().map(|a| a.username.clone()).collect();
            for account in &accounts {
                launches.set(&account.username, Launched::Starting);
            }
        }
        let status = self.status.clone();
        let states = self.launches.clone();
        // The keychain may ask the user to unlock it: not on the UI thread.
        let _ = std::thread::Builder::new().name("Turu Keychain".into()).spawn(move || {
            let mut needs_login = Vec::new();
            let launches = accounts
                .into_iter()
                .map(|account| {
                    let password = accounts::password(&account.username).unwrap_or_else(|e| {
                        crate::turu::bot().logs.debug.push(format!("{}: {e}", account.username));
                        None
                    });
                    if password.is_none() {
                        needs_login.push(account.username.clone());
                    }
                    Launch {
                        username: account.username.clone(),
                        label: account.label().to_owned(),
                        password,
                        ..template.clone()
                    }
                })
                .collect();
            if in_tabs {
                open_tabs(launches, &status, &states);
                return;
            }
            army.launch_all(launches, move |username, result| {
                let (text, state) = match result {
                    Ok(()) if needs_login.iter().any(|u| u == username) => (
                        format!("Started {username}; log in in its window."),
                        Launched::NeedsLogin,
                    ),
                    Ok(()) => (format!("Started {username}."), Launched::Started),
                    Err(e) => (format!("{username}: {e}"), Launched::Failed(e.to_string())),
                };
                let failed = matches!(state, Launched::Failed(_));
                lock(&states).set(username, state);
                crate::turu::bot().logs.debug.push(text.clone());
                if failed {
                    set_error(&status, text);
                } else {
                    set_status(&status, text);
                }
            });
        });
    }
}

/// After the name on an account's row: how its last launch went, while it matters. A failure
/// stays until the account is launched again; "Log in by hand" until it's logged in in a tab
/// (Army can't tell when a client of its own logs in: there it stays until the next launch).
fn launch_note(ui: &mut Ui, launched: Option<&Launched>, running: bool, logged_in: bool) {
    let (text, color, tip) = match launched {
        Some(Launched::Starting) if !running => ("Starting…".to_owned(), MUTED, None),
        Some(Launched::NeedsLogin) if !logged_in => (
            "Log in by hand".to_owned(),
            WARNING,
            Some(if accounts::keeps_passwords() {
                "No saved password: it opened at the login screen. Edit the account to save one."
            } else {
                NO_PASSWORDS
            }),
        ),
        Some(Launched::Failed(why)) if !running => (format!("Failed: {why}"), DANGER, None),
        _ => return,
    };
    let note = ui.add(Label::new(RichText::new(&text).size(11.0).color(color)).truncate());
    note.on_hover_text(tip.map_or(text.clone(), str::to_owned));
}

impl AccountsWindow {
    /// Remove's question: the account goes, and its password with it.
    fn confirm_remove(&mut self, ctx: &Context) {
        let Some(username) = self.confirm_remove.clone() else {
            return;
        };
        let label = self
            .list
            .find(&username)
            .map_or_else(|| username.clone(), |a| a.label().to_owned());
        let mut answer = None;
        Modal::new(egui::Id::new("turu_accounts_remove")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(format!("Remove {label}?"));
            ui.label("Its saved password is deleted from the system keychain too.");
            ui.horizontal(|ui| {
                if widgets::button(ui, "Remove", Kind::Danger, true, vec2(84.0, 28.0), "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, vec2(84.0, 28.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => self.row_action(Row::Remove(true), &username),
            Some(false) => self.confirm_remove = None,
            None => {}
        }
    }

    /// Start Selected's and Start All's question, when they would start more than a few accounts.
    fn start_question(&mut self, ctx: &Context) {
        let Some(accounts) = &self.confirm_start else {
            return;
        };
        let count = accounts.iter().filter(|a| !is_running(&a.username)).count();
        let mut answer = None;
        Modal::new(egui::Id::new("turu_accounts_start_all")).show(ctx, |ui| {
            ui.set_max_width(360.0);
            ui.heading(format!("Start {count} accounts?"));
            ui.label("Each one opens a game client and logs in, a few seconds apart.");
            ui.horizontal(|ui| {
                if widgets::button(ui, "Start", Kind::Primary, true, vec2(84.0, 28.0), "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, vec2(84.0, 28.0), "").clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(start) = answer
            && let Some(accounts) = self.confirm_start.take()
            && start
        {
            self.launch(accounts);
        }
    }
}

/// The launch server's drop-down with each server's status (VibeSkua Manager's
/// `ServerStatusInline`): "Twilly [M] - 42ms - 120/1000", the ping measured with the game's
/// handshake (`turu_bot::accounts::ping`). Falls back to the names alone without the list.
fn server_status_combo(ui: &mut Ui, bot: &std::sync::Arc<turu_bot::Bot>, current: &str) -> Option<String> {
    turu_bot::api::servers::refresh_in_background(bot);
    let servers = bot.servers.cached();
    let pings = turu_bot::accounts::ping::Pings::global();
    pings.measure_in_background(servers.clone());
    if servers.is_empty() {
        return widgets::server_combo(ui, "turu_accounts_server", bot, current);
    }
    if pings.is_measuring() || bot.servers.is_refreshing() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }
    let mut picked = None;
    ComboBox::from_id_salt("turu_accounts_server")
        .selected_text(current)
        .width(150.0)
        .show_ui(ui, |ui| {
            ui.spacing_mut().interact_size.y = widgets::target(24.0, 40.0);
            ui.set_min_width(widgets::target(260.0, 240.0));
            for server in &servers {
                let name = server.name.trim();
                let line = turu_bot::accounts::ping::status_line(server, pings.get(name));
                if ui.selectable_label(current.eq_ignore_ascii_case(name), line).clicked() {
                    picked = Some(name.to_owned());
                }
            }
        })
        .response
        .on_hover_text("Which server to login when launching accounts");
    picked
}

/// Whether the list shows the account with this tag filter and search (its name, username or a
/// tag containing the search, ignoring case).
fn shows(account: &Account, tag_filter: Option<&str>, search: &str) -> bool {
    let tagged = tag_filter.is_none_or(|tag| account.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)));
    let needle = search.trim().to_lowercase();
    let found = needle.is_empty()
        || account.username.to_lowercase().contains(&needle)
        || account.display_name.to_lowercase().contains(&needle)
        || account.tags.iter().any(|t| t.to_lowercase().contains(&needle));
    tagged && found
}

/// The account plays in a tab, or in a client Army launched.
fn is_running(username: &str) -> bool {
    crate::turu::army().client(username).is_some() || logged_in_tab(username)
}

/// The account is logged in in one of this window's tabs.
fn logged_in_tab(username: &str) -> bool {
    crate::turu::tabs()
        .iter()
        .any(|(_, bot)| turu_bot::api::servers::login_name(bot).is_some_and(|name| name.eq_ignore_ascii_case(username)))
}

/// Opens a tab for each launch, 500-900 ms apart; each logs in once its game has loaded.
fn open_tabs(launches: Vec<Launch>, status: &Mutex<Status>, states: &Mutex<Launches>) {
    for (i, launch) in launches.into_iter().enumerate() {
        if i > 0 {
            std::thread::sleep(turu_bot::army::launch_delay());
        }
        let startup = Startup {
            username: Some(launch.username.clone()),
            password: launch.password,
            server: launch.server,
            script: launch.script,
            run_script: launch.run_script,
            ..Default::default()
        };
        let logs_in = startup.logs_in();
        if !logs_in {
            crate::turu::bot().logs.debug.push(format!(
                "{}: no saved password, log in yourself in its tab",
                launch.username
            ));
        }
        let (text, state) = if crate::turu::request_tab(TabRequest::Open(Box::new(startup))) {
            let state = if logs_in {
                Launched::Started
            } else {
                Launched::NeedsLogin
            };
            (format!("Opened a tab for {}.", launch.username), state)
        } else {
            let failed = "couldn't open a tab".to_owned();
            (format!("{}: {failed}", launch.username), Launched::Failed(failed))
        };
        let failed = matches!(state, Launched::Failed(_));
        lock(states).set(&launch.username, state);
        if failed {
            set_error(status, text);
        } else {
            set_status(status, text);
        }
    }
}

/// An account's groups (in the accent colour) and tags (grey) as pills, as many as fit; "+2"
/// for the rest (all of them on hover).
fn tag_pills(ui: &mut Ui, pills: &[(&str, egui::Color32)]) {
    let font = egui::FontId::proportional(10.0);
    let gap = ui.spacing().item_spacing.x;
    for (i, &(text, color)) in pills.iter().enumerate() {
        let text_width = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font.clone(), color)
            .size()
            .x;
        // The pill's margins (`widgets::pill`), and room for a "+N" after it.
        let more = if i + 1 < pills.len() { 28.0 } else { 0.0 };
        if text_width + 18.0 + gap + more > ui.available_width() {
            let rest = pills.len() - i;
            let all: Vec<&str> = pills.iter().map(|(text, _)| *text).collect();
            ui.label(RichText::new(format!("+{rest}")).size(10.0).color(MUTED))
                .on_hover_text(all.join(", "));
            return;
        }
        widgets::pill(ui, |ui| ui.label(RichText::new(text).size(10.0).color(color)));
    }
}

/// An account row's ⋯ menu: Edit, Move up / down (in the whole list, `index` of `count`) and
/// Remove (which asks first).
fn row_menu(ui: &mut Ui, index: usize, count: usize) -> Option<Row> {
    // Rows about 44 points high on a touch screen, as the nav bar's menus.
    if widgets::touch() {
        ui.spacing_mut().button_padding.y = 14.0;
    }
    let mut row = None;
    if ui.button("Edit").clicked() {
        row = Some(Row::Edit);
    }
    if ui.add_enabled(index > 0, egui::Button::new("↑  Move up")).clicked() {
        row = Some(Row::Shift(-1));
    }
    if ui
        .add_enabled(index + 1 < count, egui::Button::new("↓  Move down"))
        .clicked()
    {
        row = Some(Row::Shift(1));
    }
    ui.separator();
    if ui.button(RichText::new("Remove…").color(DANGER)).clicked() {
        row = Some(Row::Remove(false));
    }
    if row.is_some() {
        ui.close();
    }
    row
}

#[derive(Clone, Copy)]
enum Row {
    Start,
    Edit,
    Shift(isize),
    /// Confirmed (the second click).
    Remove(bool),
}

#[cfg(test)]
mod tests {
    use super::{Launched, Launches, lock, shows};
    use turu_bot::accounts::Account;

    /// With many accounts the list takes the window's height (more than the old 200 px), and
    /// everything still fits: the launch card isn't pushed out of the window.
    #[test]
    fn list_takes_the_height_left() {
        let mut window = super::AccountsWindow {
            list: Default::default(),
            ..Default::default()
        };
        for n in 0..30 {
            window.list.accounts.push(Account {
                username: format!("Account{n}"),
                ..Default::default()
            });
        }
        let ctx = egui::Context::default();
        let (width, height) = (496.0, 560.0);
        let mut used = egui::Vec2::ZERO;
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0))),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                ui.allocate_ui(egui::vec2(width, height), |ui| {
                    ui.set_width(width);
                    let top = ui.min_rect().top();
                    window.ui(ui);
                    used = egui::vec2(window.list_bottom - top, ui.min_rect().height());
                });
            });
            output.textures_delta.clear();
        }
        let (list_end, total) = (used.x, used.y);
        assert!(
            total <= height + 1.0,
            "the window's content is {total} px tall, in {height}"
        );
        assert!(
            list_end > 320.0,
            "the list ends at {list_end} px: it doesn't use the room"
        );
    }

    /// Adding a username that's saved already (in any case) is refused; editing it isn't.
    #[test]
    fn adding_a_saved_username_is_refused() {
        let mut window = super::AccountsWindow {
            list: Default::default(),
            ..Default::default()
        };
        window.list.upsert(Account {
            username: "Hero".into(),
            ..Default::default()
        });
        window.form.username = " hero ".into();
        assert_eq!(window.duplicate().map(|a| a.username.as_str()), Some("Hero"));
        window.form.editing = Some("Hero".into());
        assert!(window.duplicate().is_none());
    }

    /// A row with a long name, tags and a long failure stays inside the window's width: the
    /// buttons keep their place and the rest is cut.
    #[test]
    fn rows_fit_with_a_long_failure() {
        let mut window = super::AccountsWindow {
            list: Default::default(),
            ..Default::default()
        };
        let name = "AnAccountWithAVeryLongNameIndeedThatGoesOn";
        window.list.accounts.push(Account {
            username: name.into(),
            tags: vec!["farm".into(), "nulgath".into(), "overnight".into()],
            ..Default::default()
        });
        lock(&window.launches).set(
            name,
            Launched::Failed("couldn't start turu: No such file or directory (os error 2) at /a/long/path".into()),
        );
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 1000.0))),
            ..Default::default()
        };
        let width = 496.0;
        let mut used = 0.0;
        let mut output = ctx.run_ui(input, |ui| {
            ui.allocate_ui(egui::vec2(width, 600.0), |ui| {
                ui.set_width(width);
                window.list_card(ui);
                used = ui.min_rect().width();
            });
        });
        output.textures_delta.clear();
        assert!(used <= width + 1.0, "the rows took {used} px of {width}");
    }

    /// The tag filter shows an account with the tag (ignoring case), and every account without
    /// a filter: what Select All and the hidden count go by.
    #[test]
    fn filter_shows_tagged_accounts() {
        let account = Account {
            username: "Hero".into(),
            tags: vec!["Farm".into()],
            ..Default::default()
        };
        assert!(shows(&account, None, ""));
        assert!(shows(&account, Some("farm"), ""));
        assert!(!shows(&account, Some("boss"), ""));
        // The search matches the username or a tag, in part and in any case.
        assert!(shows(&account, None, " he "));
        assert!(shows(&account, Some("farm"), "FA"));
        assert!(!shows(&account, None, "nobody"));
    }

    /// The launch line counts the last Start's accounts: those still starting, then how many
    /// started, need a login, or failed.
    #[test]
    fn summary_counts_the_last_start() {
        let mut launches = Launches::default();
        assert_eq!(launches.summary(), None);
        launches.batch = ["a", "b", "c", "d"].map(str::to_owned).to_vec();
        for name in ["a", "b", "c", "d"] {
            launches.set(name, Launched::Starting);
        }
        launches.set("a", Launched::Started);
        assert_eq!(launches.summary().as_deref(), Some("Starting 2 of 4…"));
        launches.set("b", Launched::NeedsLogin);
        launches.set("c", Launched::Failed("couldn't start".into()));
        launches.set("d", Launched::Started);
        assert_eq!(
            launches.summary().as_deref(),
            Some("Started 3 of 4 · 1 need a login · 1 failed")
        );
    }
}
