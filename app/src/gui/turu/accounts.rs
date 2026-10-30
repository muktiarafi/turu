//! Army > Accounts (VibeSkua Manager's Accounts tab): the account list, groups, and launching
//! accounts, each in its own `turu` process (`turu_bot::army`). Passwords are kept in the
//! system keychain; reading and writing them happens off the UI thread.

use super::files::{self, Pending};
use super::theme::{MUTED, SUCCESS};
use super::widgets::{self, Kind};
use crate::gui::FilePicker;
use egui::{Align, ComboBox, Context, Layout, RichText, ScrollArea, Ui, vec2};
use rfd::AsyncFileDialog;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use turu_bot::accounts::{self, Account, Accounts};
use turu_bot::army::Launch;

/// The add / edit form.
#[derive(Default)]
struct Form {
    /// The username being edited (`None`: adding).
    editing: Option<String>,
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
    /// The username whose Remove asks for a second click.
    confirm_remove: Option<String>,
    pending_script: Pending,
    /// What the last action did, shown at the bottom (set by other threads too).
    status: Arc<Mutex<String>>,
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
        }
    }
}

fn set_status(status: &Mutex<String>, text: impl Into<String>) {
    *status.lock().unwrap_or_else(|e| e.into_inner()) = text.into();
}

impl AccountsWindow {
    pub fn show(&mut self, ctx: &Context, picker: &FilePicker, open: &mut bool) {
        if let Some(path) = self.pending_script.poll() {
            self.list.script = Some(path);
            self.save();
        }
        if !*open {
            return;
        }
        super::window(ctx, "Accounts", [520.0, 560.0], 4)
            .open(open)
            .show(ctx, |ui| self.ui(ui, picker));
        // Launches finish on their own thread.
        ctx.request_repaint_after(Duration::from_millis(500));
    }

    fn save(&self) {
        if let Err(e) = self.list.save(&crate::turu::bot().paths.accounts_file()) {
            set_status(&self.status, format!("Couldn't save the accounts: {e}"));
        }
    }

    fn ui(&mut self, ui: &mut Ui, picker: &FilePicker) {
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        self.form_card(ui);
        self.list_card(ui);
        self.launch_card(ui, picker);
        let status = self.status.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if !status.is_empty() {
            ui.label(RichText::new(status).small().color(MUTED));
        }
    }

    fn form_card(&mut self, ui: &mut Ui) {
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
                if (editing || !self.form.username.is_empty())
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
            username: account.username.clone(),
            password: String::new(),
            show_password: false,
            display_name: account.display_name.clone(),
            tags: account.tags.join(", "),
        };
    }

    fn list_card(&mut self, ui: &mut Ui) {
        let army = crate::turu::army();
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
                    if widgets::small_button(ui, "Unselect All", true).clicked() {
                        self.list.accounts.iter_mut().for_each(|a| a.selected = false);
                        self.save();
                    }
                    if widgets::small_button(ui, "Select All", true).clicked() {
                        self.list.accounts.iter_mut().for_each(|a| a.selected = true);
                        self.save();
                    }
                });
            });

            let mut changed = false;
            let mut action: Option<(Row, String)> = None;
            ScrollArea::vertical()
                .max_height(200.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if self.list.accounts.is_empty() {
                        ui.label(RichText::new("No accounts yet: add one above.").color(MUTED));
                    }
                    for account in &mut self.list.accounts {
                        if let Some(tag) = &self.tag_filter
                            && !account.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
                        {
                            continue;
                        }
                        ui.horizontal(|ui| {
                            changed |= ui.checkbox(&mut account.selected, "").changed();
                            if army.client(&account.username).is_some() {
                                widgets::dot(ui, SUCCESS, 4.0);
                            }
                            ui.label(RichText::new(account.label()).strong());
                            if !account.display_name.trim().is_empty() {
                                ui.label(RichText::new(&account.username).small().color(MUTED));
                            }
                            for tag in &account.tags {
                                widgets::pill(ui, |ui| ui.label(RichText::new(tag).size(10.0).color(MUTED)));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let confirming = self.confirm_remove.as_deref() == Some(account.username.as_str());
                                let remove = if confirming { "Sure?" } else { "Remove" };
                                if widgets::small_button(ui, remove, true).clicked() {
                                    action = Some((Row::Remove(confirming), account.username.clone()));
                                }
                                if widgets::small_button(ui, "↓", true).clicked() {
                                    action = Some((Row::Shift(1), account.username.clone()));
                                }
                                if widgets::small_button(ui, "↑", true).clicked() {
                                    action = Some((Row::Shift(-1), account.username.clone()));
                                }
                                if widgets::small_button(ui, "Edit", true).clicked() {
                                    action = Some((Row::Edit, account.username.clone()));
                                }
                                let running = army.client(&account.username).is_some();
                                if widgets::small_button(ui, "Start", !running).clicked() {
                                    action = Some((Row::Start, account.username.clone()));
                                }
                            });
                        });
                    }
                });
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
                    ComboBox::from_id_salt("turu_accounts_group")
                        .selected_text("Select group")
                        .show_ui(ui, |ui| {
                            for group in &self.list.groups {
                                let label = format!("{} ({})", group.name, group.accounts.len());
                                if ui.selectable_label(false, label).clicked() {
                                    pick = Some(group.name.clone());
                                }
                            }
                        });
                    if let Some(group) = pick {
                        self.list.select_group(&group);
                        self.save();
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

    fn launch_card(&mut self, ui: &mut Ui, picker: &FilePicker) {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::section_label(ui, "LAUNCH");
            ui.horizontal(|ui| {
                ui.label("Server");
                if widgets::text_box(ui, &mut self.list.server, "Twilly", 120.0).lost_focus() {
                    self.save();
                }
                ui.add_space(8.0);
                if widgets::switch(ui, &mut self.list.headless).changed() {
                    self.save();
                }
                ui.label("Without a window")
                    .on_hover_text("--headless: no window, steer it from Army Control");
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
                    let dialog = AsyncFileDialog::new()
                        .set_title("Script for launched accounts")
                        .add_filter("Scripts", &["ts", "js"])
                        .set_directory(start);
                    self.pending_script.pick_file(picker, ui.ctx(), dialog);
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
                let start = widgets::button(
                    ui,
                    "Start Selected",
                    Kind::Primary,
                    !selected.is_empty(),
                    vec2(130.0, 30.0),
                    "Check some accounts",
                );
                if start.clicked() {
                    self.launch(selected);
                }
                let all = self.list.accounts.clone();
                if widgets::button(ui, "Start All", Kind::Normal, !all.is_empty(), vec2(100.0, 30.0), "").clicked() {
                    self.launch(all);
                }
            });
        });
    }

    /// Launches the accounts that aren't running, staggered, with their saved passwords.
    fn launch(&self, accounts: Vec<Account>) {
        let army = crate::turu::army().clone();
        let accounts: Vec<Account> = accounts
            .into_iter()
            .filter(|a| army.client(&a.username).is_none())
            .collect();
        if accounts.is_empty() {
            set_status(&self.status, "Those accounts are running already.");
            return;
        }
        let list = &self.list;
        let template = Launch {
            server: Some(list.server.trim().to_owned()).filter(|s| !s.is_empty()),
            script: list.script.clone(),
            run_script: list.auto_start,
            headless: list.headless,
            ..Default::default()
        };
        set_status(&self.status, format!("Starting {} account(s)…", accounts.len()));
        let status = self.status.clone();
        // The keychain may ask the user to unlock it: not on the UI thread.
        let _ = std::thread::Builder::new().name("Turu Keychain".into()).spawn(move || {
            let launches = accounts
                .into_iter()
                .map(|account| {
                    let password = accounts::password(&account.username).unwrap_or_else(|e| {
                        crate::turu::bot().logs.debug.push(format!("{}: {e}", account.username));
                        None
                    });
                    Launch {
                        username: account.username.clone(),
                        label: account.label().to_owned(),
                        password,
                        ..template.clone()
                    }
                })
                .collect();
            army.launch_all(launches, move |username, result| {
                let text = match result {
                    Ok(()) => format!("Started {username}."),
                    Err(e) => format!("{username}: {e}"),
                };
                crate::turu::bot().logs.debug.push(text.clone());
                set_status(&status, text);
            });
        });
    }
}

#[derive(Clone, Copy)]
enum Row {
    Start,
    Edit,
    Shift(isize),
    /// Confirmed (the second click).
    Remove(bool),
}
