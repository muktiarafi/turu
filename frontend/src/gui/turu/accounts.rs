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
    status: Arc<Mutex<String>>,
    launches: Arc<Mutex<Launches>>,
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
            launches: Arc::default(),
            confirm_start: None,
        }
    }
}

fn set_status(status: &Mutex<String>, text: impl Into<String>) {
    *status.lock().unwrap_or_else(|e| e.into_inner()) = text.into();
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
            set_status(&self.status, format!("Couldn't save the accounts: {e}"));
        }
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        self.form_card(ui);
        self.list_card(ui);
        self.launch_card(ui);
        let status = self.status.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if !status.is_empty() {
            ui.label(RichText::new(status).small().color(MUTED));
        }
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
            widgets::section_label(ui, if editing { "EDIT ACCOUNT" } else { "ADD ACCOUNT" });
            let half = (ui.available_width() - 6.0) / 2.0;
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!editing, |ui| {
                    widgets::text_box(ui, &mut self.form.username, "Username", half)
                });
                widgets::text_box(ui, &mut self.form.display_name, "Display Name (optional)", half);
            });
            ui.horizontal(|ui| {
                let hint = if editing {
                    "Password (blank: keep it)"
                } else {
                    "Password"
                };
                ui.add(
                    egui::TextEdit::singleline(&mut self.form.password)
                        .password(!self.form.show_password)
                        .hint_text(hint)
                        .desired_width(half - 50.0)
                        .margin(egui::Margin::symmetric(8, 5))
                        .background_color(super::theme::INSET),
                );
                let eye = if self.form.show_password { "Hide" } else { "Show" };
                if widgets::small_button(ui, eye, true).clicked() {
                    self.form.show_password = !self.form.show_password;
                }
                widgets::text_box(ui, &mut self.form.tags, "Tags, comma-separated", half);
            });
            ui.horizontal(|ui| {
                let can_save = !self.form.username.trim().is_empty();
                let label = if editing { "Save Account" } else { "Add Account" };
                if widgets::button(
                    ui,
                    label,
                    Kind::Primary,
                    can_save,
                    vec2(120.0, 28.0),
                    "Enter a username",
                )
                .clicked()
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
                Err(e) => set_status(&status, format!("{what} {username}, but {e}")),
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
                    let filter = self.tag_filter.clone();
                    if widgets::small_button(ui, "Unselect All", true).clicked() {
                        let shown = self.list.accounts.iter_mut().filter(|a| shows(a, filter.as_deref()));
                        shown.for_each(|a| a.selected = false);
                        self.save();
                    }
                    if widgets::small_button(ui, "Select All", true).clicked() {
                        let shown = self.list.accounts.iter_mut().filter(|a| shows(a, filter.as_deref()));
                        shown.for_each(|a| a.selected = true);
                        self.save();
                    }
                });
            });

            let mut changed = false;
            let mut action: Option<(Row, String)> = None;
            let launches = lock(&self.launches);
            ScrollArea::vertical()
                .max_height(200.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if self.list.accounts.is_empty() {
                        ui.label(RichText::new("No accounts yet: add one above.").color(MUTED));
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
                        if !shows(account, self.tag_filter.as_deref()) {
                            continue;
                        }
                        let running = is_running(&account.username);
                        let launched = launches.rows.get(&account.username);
                        // The buttons first, on the right; the rest gets the width left and is
                        // cut there, so a long failure can't push Start out of the window.
                        ui.horizontal(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.menu_button("⋯", |ui| {
                                    if let Some(row) = row_menu(ui, index, count) {
                                        action = Some((row, account.username.clone()));
                                    }
                                })
                                .response
                                .on_hover_text("Edit, move, remove");
                                if widgets::small_button(ui, "Start", !running).clicked() {
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
                        Err(e) => set_status(&status, format!("Removed {username}, but {e}")),
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
                if let Some(server) = widgets::server_combo(ui, "turu_accounts_server", &bot, &self.list.server) {
                    self.list.server = server;
                    self.save();
                }
                ui.add_space(8.0);
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
                if widgets::small_button(ui, "Browse…", !self.pending_script.is_open()).clicked() {
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
                ui.add_enabled_ui(has_script, |ui| {
                    if widgets::switch(ui, &mut self.list.auto_start).changed() {
                        self.save();
                    }
                    ui.label("Auto-start");
                });
            });
            ui.horizontal(|ui| {
                let selected: Vec<Account> = self.list.accounts.iter().filter(|a| a.selected).cloned().collect();
                // Selected accounts the tag filter hides still start: said beside the buttons.
                let hidden = selected
                    .iter()
                    .filter(|a| !shows(a, self.tag_filter.as_deref()))
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
                        .on_hover_text("Start Selected starts them too; set the filter to All tags to see them");
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
        let in_tabs = list.mode == LaunchMode::Tab;
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
                lock(&states).set(username, state);
                crate::turu::bot().logs.debug.push(text.clone());
                set_status(&status, text);
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
            Some("No saved password: it opened at the login screen. Edit the account to save one."),
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

/// Whether the list shows the account with this tag filter.
fn shows(account: &Account, tag_filter: Option<&str>) -> bool {
    tag_filter.is_none_or(|tag| account.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
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
fn open_tabs(launches: Vec<Launch>, status: &Mutex<String>, states: &Mutex<Launches>) {
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
        lock(states).set(&launch.username, state);
        set_status(status, text);
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
        assert!(shows(&account, None));
        assert!(shows(&account, Some("farm")));
        assert!(!shows(&account, Some("boss")));
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
