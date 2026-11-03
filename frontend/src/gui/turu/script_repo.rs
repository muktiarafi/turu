//! Search Scripts (VibeSkua's 969x500 Script Repo, `ScriptRepoControl`): the repository's
//! scripts and your own, searchable, with Start, Load, Queue, Pin, Download/Update and Delete, and
//! Update all / Download all / Reset Scripts. The work runs in `turu_bot::repo` on a thread of
//! its own; this window shows its snapshot. On Android it is the only way to get a script, so
//! on a narrow or touch screen the rows keep Start and a menu, and every target is finger-sized.

use super::files;
use super::theme::{DANGER, MUTED, SUCCESS, TRACK, WARNING};
use super::widgets::{self, Kind};
use egui::{
    Align, Button, ComboBox, Context, CornerRadius, FontId, Layout, Modal, RichText, ScrollArea, Sense, Ui, vec2,
};
use std::path::PathBuf;
use std::time::Duration;
use turu_bot::repo::{self, Entry, Scope, Snapshot, Status};

/// One row of the list, at least: the text takes 69 px with Turu's theme, plus the margins
/// ([`ROW_MARGIN`] above and below). A row whose text needs more makes every row taller.
const ROW_HEIGHT: f32 = 78.0;
const ROW_MARGIN: f32 = 4.0;

/// Tag chips: height, padding left and right of the text, gap between chips.
const CHIP_HEIGHT: f32 = 16.0;
const CHIP_PADDING: f32 = 6.0;
const CHIP_GAP: f32 = 4.0;

/// Narrower than this, the rows keep Start and the menu (Load and Queue move into it).
const NARROW: f32 = 640.0;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Sort {
    #[default]
    Name,
    Added,
}

/// What the list was filtered for; it is worked out again when this changes.
#[derive(Clone, PartialEq, Default)]
struct Filter {
    generation: u64,
    search: String,
    scope: Scope,
    category: String,
    pinned_only: bool,
    local_only: bool,
    sort: Sort,
    descending: bool,
}

/// A Delete waiting for its confirmation.
struct DeleteAsk {
    path: String,
    name: String,
    status: Status,
}

#[derive(Default)]
pub struct ScriptRepoWindow {
    search: String,
    scope: Scope,
    /// "All" or a folder.
    category: String,
    /// The Pinned and Local chips.
    pinned_only: bool,
    local_only: bool,
    sort: Sort,
    descending: bool,
    filtered: Vec<usize>,
    filter: Option<Filter>,
    categories: Vec<String>,
    confirm_reset: bool,
    confirm_delete: Option<DeleteAsk>,
    /// The script the running task is for (its row shows a spinner), and whether it is a Start.
    working: Option<(String, bool)>,
    /// A Start finished well: the Script Loader should come up ([`Self::take_open_loader`]).
    open_loader: bool,
    /// The rows' height: [`ROW_HEIGHT`], or more once a row's text needed it.
    row_height: f32,
    vscode: Option<Option<PathBuf>>,
}

/// What a row's buttons may do, and why not.
struct Allowed {
    start: bool,
    load: bool,
    queue: bool,
    run_tip: String,
    queue_tip: String,
}

impl ScriptRepoWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        let repo = crate::turu::script_repo();
        let snapshot = repo.snapshot();
        // The task a row started is done: after a Start that worked, the Script Loader shows it.
        if snapshot.busy.is_none()
            && let Some((_, start)) = self.working.take()
            && start
            && !snapshot.failed
        {
            self.open_loader = true;
        }
        if !*open {
            return;
        }
        super::window(ctx, "Search Scripts", [969.0, 500.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &snapshot));
        if self.confirm_reset {
            self.reset_modal(ctx);
        }
        if self.confirm_delete.is_some() {
            self.delete_modal(ctx);
        }
        if snapshot.busy.is_some() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }

    /// Whether a Start from this window just worked, so the Script Loader should open.
    pub fn take_open_loader(&mut self) -> bool {
        std::mem::take(&mut self.open_loader)
    }

    fn ui(&mut self, ui: &mut Ui, snapshot: &Snapshot) {
        if self.category.is_empty() {
            self.category = "All".into();
        }
        self.refilter(snapshot);
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);
        let touch = widgets::touch();
        let narrow = touch || ui.available_width() < NARROW;
        let entries = &snapshot.entries;

        // The search box across the window, then what it looks in and the filters, which wrap
        // on a narrow screen. On a touch screen they're 48 points high, their rows about 44.
        // Words, not symbols: Turu's fonts have no ✕ or arrows.
        let control = widgets::target(26.0, 40.0);
        ui.horizontal(|ui| {
            // The box's width leaves out its margins (8 each side).
            let width = (ui.available_width() - 56.0 - 6.0 - 16.0).max(120.0);
            widgets::text_box(ui, &mut self.search, "Search scripts", width);
            if widgets::button(
                ui,
                "Clear",
                Kind::Normal,
                !self.search.is_empty(),
                vec2(56.0, control),
                "",
            )
            .on_hover_text("Clear the search")
            .clicked()
            {
                self.search.clear();
            }
        });
        ui.horizontal_wrapped(|ui| {
            if touch {
                ui.spacing_mut().interact_size.y = 40.0;
            }
            ui.label(RichText::new("Search in").color(MUTED));
            ComboBox::from_id_salt("turu_repo_scope")
                .width(100.0)
                .selected_text(scope_label(self.scope))
                .show_ui(ui, |ui| {
                    touch_rows(ui);
                    for scope in Scope::ALL {
                        ui.selectable_value(&mut self.scope, scope, scope_label(scope));
                    }
                });
            ComboBox::from_id_salt("turu_repo_category")
                .width(120.0)
                .selected_text(if self.category == "All" {
                    "All folders"
                } else {
                    self.category.as_str()
                })
                .show_ui(ui, |ui| {
                    touch_rows(ui);
                    for category in &self.categories {
                        let label = if category == "All" { "All folders" } else { category };
                        ui.selectable_value(&mut self.category, category.clone(), label);
                    }
                })
                .response
                .on_hover_text("The folder the scripts are in");
            if widgets::chip(ui, "Pinned", self.pinned_only) {
                self.pinned_only = !self.pinned_only;
            }
            if widgets::chip(ui, "Local", self.local_only) {
                self.local_only = !self.local_only;
            }
            ComboBox::from_id_salt("turu_repo_sort")
                .width(100.0)
                .selected_text(match self.sort {
                    Sort::Name => "Name",
                    Sort::Added => "Date added",
                })
                .show_ui(ui, |ui| {
                    touch_rows(ui);
                    ui.selectable_value(&mut self.sort, Sort::Name, "Name");
                    ui.selectable_value(&mut self.sort, Sort::Added, "Date added");
                })
                .response
                .on_hover_text("Sort by");
            let order = match (self.sort, self.descending) {
                (Sort::Name, false) => "A to Z",
                (Sort::Name, true) => "Z to A",
                (Sort::Added, false) => "Oldest first",
                (Sort::Added, true) => "Newest first",
            };
            if widgets::button(ui, order, Kind::Normal, true, vec2(96.0, control), "")
                .on_hover_text("Reverse the order")
                .clicked()
            {
                self.descending = !self.descending;
            }
            ui.label(
                RichText::new(format!("{} shown", self.filtered.len()))
                    .size(12.0)
                    .color(MUTED),
            );
        });

        // The list, above the bottom bar.
        let bar = widgets::target(30.0, 48.0) + 14.0;
        let list_height = (ui.available_height() - bar).max(80.0);
        ui.allocate_ui(vec2(ui.available_width(), list_height), |ui| {
            let busy = snapshot.busy.is_some();
            if entries.iter().all(|e| e.info.library) {
                let (title, text) = if busy {
                    (
                        "Loading scripts...",
                        "Getting the script list, this can take a few seconds.",
                    )
                } else {
                    ("No scripts yet", "Get the script list from the repository.")
                };
                empty_state(ui, title, text);
                if !busy {
                    ui.vertical_centered(|ui| {
                        let size = vec2(110.0, widgets::target(28.0, 48.0));
                        if widgets::button(ui, "Refresh", Kind::Primary, true, size, "").clicked() {
                            crate::turu::script_repo().spawn("Refresh", |repo| repo.refresh());
                        }
                    });
                }
            } else if self.filtered.is_empty() {
                empty_state(ui, "No scripts found", "Nothing matches the search and filters.");
                ui.vertical_centered(|ui| {
                    let size = vec2(130.0, widgets::target(28.0, 48.0));
                    if widgets::button(ui, "Clear the filters", Kind::Normal, true, size, "").clicked() {
                        self.search.clear();
                        self.scope = Scope::All;
                        self.category = "All".into();
                        self.pinned_only = false;
                        self.local_only = false;
                    }
                });
            } else {
                let allowed = allowed(snapshot);
                // The top search hit gets the accent on a touch screen (no hover there).
                let top_hit = (!self.search.trim().is_empty()).then(|| self.filtered[0]);
                let filtered = std::mem::take(&mut self.filtered);
                ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                    ui,
                    self.row_height.max(ROW_HEIGHT),
                    filtered.len(),
                    |ui, rows| {
                        for i in rows {
                            let index = filtered[i];
                            self.row(ui, &entries[index], &allowed, narrow, top_hit == Some(index));
                        }
                    },
                );
                self.filtered = filtered;
            }
        });

        self.bottom_bar(ui, snapshot);
    }

    fn refilter(&mut self, snapshot: &Snapshot) {
        let filter = Filter {
            generation: snapshot.generation,
            search: self.search.clone(),
            scope: self.scope,
            category: self.category.clone(),
            pinned_only: self.pinned_only,
            local_only: self.local_only,
            sort: self.sort,
            descending: self.descending,
        };
        if self.filter.as_ref() == Some(&filter) {
            return;
        }
        let entries = &snapshot.entries;
        let mut folders: Vec<String> = entries
            .iter()
            .filter(|e| !e.info.library)
            .filter_map(|e| e.info.category())
            .map(capitalize)
            .collect();
        folders.sort();
        folders.dedup();
        self.categories = vec!["All".into()];
        self.categories.extend(folders);

        // Libraries (CoreBots, ...) have nothing to run; they come with the scripts importing them.
        let mut rows: Vec<(usize, u8)> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.info.library)
            .filter(|(_, e)| self.category == "All" || repo::in_category(&e.info, &self.category))
            .filter(|(_, e)| !self.pinned_only || e.pinned)
            .filter(|(_, e)| !self.local_only || e.status == Status::Local)
            .filter_map(|(i, e)| repo::search_rank(&e.info, &self.search, self.scope).map(|rank| (i, rank)))
            .collect();
        // VibeSkua: pinned first, then the sort; the search rank orders what that gives.
        rows.sort_by(|&(a, _), &(b, _)| {
            let (a, b) = (&entries[a], &entries[b]);
            b.pinned.cmp(&a.pinned).then_with(|| {
                let order = match self.sort {
                    Sort::Name => a.info.name.to_lowercase().cmp(&b.info.name.to_lowercase()),
                    Sort::Added => a.info.added.cmp(&b.info.added),
                };
                if self.descending { order.reverse() } else { order }
            })
        });
        if !self.search.trim().is_empty() {
            rows.sort_by_key(|&(_, rank)| rank);
        }
        self.filtered = rows.into_iter().map(|(i, _)| i).collect();
        self.filter = Some(filter);
    }

    /// One script: its text, then Start, Load, Queue and the menu (on a narrow screen Start and
    /// the menu, which holds Load and Queue). Start is a plain button but on the row under the
    /// pointer and the top search hit, so a long list doesn't shout.
    fn row(&mut self, ui: &mut Ui, entry: &Entry, allowed: &Allowed, narrow: bool, top_hit: bool) {
        let info = &entry.info;
        let height = self.row_height.max(ROW_HEIGHT);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
        let lead = top_hit || ui.rect_contains_pointer(rect);
        let working = self.working.as_ref().is_some_and(|(path, _)| *path == info.path);
        let button_height = widgets::target(28.0, 48.0);
        let menu_width = widgets::target(30.0, 48.0);
        let buttons = if narrow { 64.0 } else { 64.0 + 56.0 + 64.0 } + menu_width + 30.0 + 24.0;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, ROW_MARGIN))));
        let ui = &mut child;
        ui.horizontal(|ui| {
            ui.allocate_ui(
                vec2((ui.available_width() - buttons).max(0.0), height - 2.0 * ROW_MARGIN),
                |ui| {
                    // Long names and paths stop at the buttons.
                    ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
                    // The list needs one height for every row: grow them all if this one needs more.
                    let needed = row_text(ui, entry) + 2.0 * ROW_MARGIN;
                    if needed > height + 0.5 {
                        self.row_height = needed.ceil();
                        ui.ctx().request_repaint();
                    }
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.row_menu(ui, entry, allowed, narrow, vec2(menu_width, button_height));
                if !narrow {
                    if widgets::button(
                        ui,
                        "Queue",
                        Kind::Normal,
                        allowed.queue,
                        vec2(64.0, button_height),
                        &allowed.queue_tip,
                    )
                    .on_hover_text("Add to the Scheduler queue")
                    .clicked()
                    {
                        self.run(&info.path, Work::Queue);
                    }
                    if widgets::button(
                        ui,
                        "Load",
                        Kind::Normal,
                        allowed.load,
                        vec2(56.0, button_height),
                        &allowed.run_tip,
                    )
                    .on_hover_text("Load it in the Script Loader")
                    .clicked()
                    {
                        self.run(&info.path, Work::Load);
                    }
                }
                let kind = if lead { Kind::Primary } else { Kind::Normal };
                if widgets::button(
                    ui,
                    "Start",
                    kind,
                    allowed.start,
                    vec2(64.0, button_height),
                    &allowed.run_tip,
                )
                .on_hover_text("Load it and start it")
                .clicked()
                {
                    self.run(&info.path, Work::Start);
                }
                if working {
                    ui.spinner();
                }
            });
        });
        ui.painter()
            .hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0, TRACK));
    }

    /// Starts `work` for the script at `path`, its row showing a spinner until it's done.
    fn run(&mut self, path: &str, work: Work) {
        if run(path, work) {
            self.working = Some((path.to_owned(), work == Work::Start));
        }
    }

    fn row_menu(&mut self, ui: &mut Ui, entry: &Entry, allowed: &Allowed, narrow: bool, size: egui::Vec2) {
        let info = &entry.info;
        let repo = crate::turu::script_repo();
        let busy = repo.is_busy();
        let button = Button::new("…").min_size(size);
        egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
            ui.set_min_width(190.0);
            touch_rows(ui);
            if narrow {
                let load = ui
                    .add_enabled(allowed.load, Button::new("Load"))
                    .on_disabled_hover_text(&allowed.run_tip);
                if load.clicked() {
                    self.run(&info.path, Work::Load);
                    ui.close();
                }
                let queue = ui
                    .add_enabled(allowed.queue, Button::new("Add to the queue"))
                    .on_disabled_hover_text(&allowed.queue_tip);
                if queue.clicked() {
                    self.run(&info.path, Work::Queue);
                    ui.close();
                }
                ui.separator();
            }
            if ui.button(if entry.pinned { "Unpin" } else { "Pin" }).clicked() {
                repo.set_pinned(&info.path, !entry.pinned);
                ui.close();
            }
            let download = match entry.status {
                Status::NotDownloaded => Some(("Download", Work::Download)),
                Status::Outdated => Some(("Update", Work::Download)),
                Status::Modified => Some(("Replace with the repository's", Work::Replace)),
                _ => None,
            };
            if let Some((label, work)) = download
                && ui.add_enabled(!busy, Button::new(label)).clicked()
            {
                self.run(&info.path, work);
                ui.close();
            }
            let local = repo.local_path(&info.path).filter(|p| p.is_file());
            let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
            if ui
                .add_enabled(local.is_some() && code.is_some(), Button::new("Open in VS Code"))
                .clicked()
                && let (Some(code), Some(local)) = (&code, &local)
            {
                if let Err(e) = files::open_in_vscode(code, local) {
                    crate::turu::bot().log(format!("Couldn't open VS Code: {e}"));
                }
                ui.close();
            }
            if ui
                .add_enabled(local.is_some() && !busy, Button::new("Delete..."))
                .clicked()
            {
                self.confirm_delete = Some(DeleteAsk {
                    path: info.path.clone(),
                    name: info.name.clone(),
                    status: entry.status,
                });
                ui.close();
            }
        });
    }

    fn bottom_bar(&mut self, ui: &mut Ui, snapshot: &Snapshot) {
        let repo = crate::turu::script_repo();
        let busy = snapshot.busy.is_some();
        let height = widgets::target(30.0, 48.0);
        let outdated = snapshot
            .entries
            .iter()
            .filter(|e| !e.info.library && e.status == Status::Outdated)
            .count();
        let wait = wait_tip(snapshot);
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            let update_tip = if busy {
                wait.as_str()
            } else {
                "Everything is up to date"
            };
            if widgets::button(
                ui,
                "Update all",
                Kind::Primary,
                !busy && outdated > 0,
                vec2(96.0, height),
                update_tip,
            )
            .on_hover_text("Update the outdated scripts (your edits are kept)")
            .clicked()
            {
                repo.spawn("Update all", |repo| repo.update_all());
            }
            if outdated > 0 {
                ui.label(RichText::new(format!("{outdated} to update")).size(12.0).color(WARNING));
            }
            if widgets::button(ui, "Download all", Kind::Normal, !busy, vec2(104.0, height), &wait)
                .on_hover_text("Download every script that's missing or outdated")
                .clicked()
            {
                repo.spawn("Download all", |repo| repo.download_all());
            }
            if widgets::button(ui, "Refresh", Kind::Normal, !busy, vec2(76.0, height), &wait)
                .on_hover_text("Get the script list again")
                .clicked()
            {
                repo.spawn("Refresh", |repo| repo.refresh());
            }
            let more = Button::new("…").min_size(vec2(widgets::target(30.0, 48.0), height));
            egui::containers::menu::MenuButton::from_button(more).ui(ui, |ui| {
                ui.set_min_width(200.0);
                touch_rows(ui);
                if ui.add_enabled(!busy, Button::new("Reset Scripts...")).clicked() {
                    self.confirm_reset = true;
                    ui.close();
                }
                let dir = crate::turu::bot().paths.scripts_dir();
                if ui.button("Open the scripts folder").clicked() {
                    crate::platform::platform().open_folder(&dir);
                    ui.close();
                }
                let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
                if ui
                    .add_enabled(code.is_some(), Button::new("Open the scripts folder in VS Code"))
                    .clicked()
                    && let Some(code) = &code
                {
                    if let Err(e) = files::open_in_vscode(code, &dir) {
                        crate::turu::bot().log(format!("Couldn't open VS Code: {e}"));
                    }
                    ui.close();
                }
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(progress) = &snapshot.busy {
                    if widgets::button(ui, "Cancel", Kind::Outline, true, vec2(70.0, height - 4.0), "").clicked() {
                        repo.cancel();
                    }
                    if progress.total > 0 {
                        ui.add(
                            egui::ProgressBar::new(progress.done as f32 / progress.total as f32)
                                .desired_width(120.0)
                                .desired_height(8.0),
                        );
                    } else {
                        ui.spinner();
                    }
                    ui.add(egui::Label::new(RichText::new(&progress.message).size(12.0)).truncate());
                } else {
                    // The last result (red when it failed), then how fresh the list is.
                    let checked = snapshot.fetched_ago().map(|ago| {
                        let text = if ago < Duration::from_secs(60) {
                            "List updated just now".to_owned()
                        } else {
                            format!("List updated {} ago", super::script_loader::duration_text(ago))
                        };
                        let commit: String = snapshot.commit.chars().take(7).collect();
                        (text, format!("Commit {commit}, fetched {} UTC", snapshot.fetched))
                    });
                    if let Some((text, tip)) = checked {
                        ui.label(RichText::new(text).size(12.0).color(MUTED)).on_hover_text(tip);
                    }
                    if !snapshot.message.is_empty() {
                        let color = if snapshot.failed { DANGER } else { MUTED };
                        ui.add(egui::Label::new(RichText::new(&snapshot.message).size(12.0).color(color)).truncate());
                        if !snapshot.failed {
                            widgets::dot(ui, SUCCESS, 4.0);
                        }
                    }
                }
            });
        });
    }

    fn reset_modal(&mut self, ctx: &Context) {
        let response = Modal::new(egui::Id::new("turu_reset_scripts")).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.heading("Reset Scripts");
            ui.label(
                "The downloaded scripts, edited ones included, move to scripts-backup in the data folder, \
                 and every script is downloaded again. Your own scripts stay.",
            );
            ui.add_space(4.0);
            let size = vec2(72.0, widgets::target(28.0, 48.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Reset", Kind::Danger, true, size, "").clicked() {
                    crate::turu::script_repo().spawn("Reset Scripts", |repo| repo.reset());
                    self.confirm_reset = false;
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    self.confirm_reset = false;
                }
            });
        });
        if response.should_close() {
            self.confirm_reset = false;
        }
    }

    /// Delete's confirmation: the file moves to scripts-backup; your own script (Local) or your
    /// edits (Modified) have no other copy, and it says so.
    fn delete_modal(&mut self, ctx: &Context) {
        let Some(ask) = &self.confirm_delete else {
            return;
        };
        let mut close = false;
        let response = Modal::new(egui::Id::new("turu_delete_script")).show(ctx, |ui| {
            ui.set_max_width(400.0);
            ui.heading(format!("Delete {}?", ask.name));
            ui.label(delete_text(ask.status));
            ui.add_space(4.0);
            let size = vec2(72.0, widgets::target(28.0, 48.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Delete", Kind::Danger, true, size, "").clicked() {
                    let (path, name) = (ask.path.clone(), ask.name.clone());
                    crate::turu::script_repo().spawn("Delete", move |repo| {
                        Ok(match repo.delete(&path)? {
                            Some(_) => format!("Moved {name} to scripts-backup."),
                            None => format!("{name} was already gone."),
                        })
                    });
                    close = true;
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    close = true;
                }
            });
        });
        if close || response.should_close() {
            self.confirm_delete = None;
        }
    }
}

/// What a row's buttons can do now, and the reasons they can't (as tooltips, or in the menu).
fn allowed(snapshot: &Snapshot) -> Allowed {
    let bot = crate::turu::bot();
    let busy = snapshot.busy.is_some();
    let active = bot.scripts.status().is_active();
    let auto = super::auto::running(&bot);
    let snippet = bot.scripts.is_snippet_running();
    let queue_running = bot.scheduler.is_running();
    let wait = wait_tip(snapshot);
    let run_tip = if busy {
        wait.clone()
    } else if active {
        "Stop the script first".into()
    } else if snippet {
        "Stop the Console snippet first".into()
    } else {
        "Stop Auto Attack/Hunt first".into()
    };
    Allowed {
        start: !busy && !active && !auto && !snippet,
        load: !busy && !active,
        queue: !busy && !queue_running,
        run_tip,
        queue_tip: if busy {
            wait
        } else {
            "Cannot add scripts while the playlist is running".into()
        },
    }
}

/// Why the buttons wait: one task runs at a time, and it can be cancelled.
fn wait_tip(snapshot: &Snapshot) -> String {
    match &snapshot.busy {
        Some(progress) => format!("Wait for {} to finish, or Cancel it", progress.task),
        None => String::new(),
    }
}

/// The scope dropdown's words: "All fields" rather than a second "All".
fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::All => "All fields",
        other => other.label(),
    }
}

/// What Delete's confirmation says for a script of `status`.
fn delete_text(status: Status) -> &'static str {
    match status {
        Status::Local => {
            "This is your own script: the repository has no copy of it. The file moves to \
             scripts-backup in the data folder, where you can get it back."
        }
        Status::Modified => {
            "Your edits go with it. The file moves to scripts-backup in the data folder, where you \
             can get it back; the repository's version can be downloaded again."
        }
        _ => {
            "The file moves to scripts-backup in the data folder; you can download it again from \
             the repository."
        }
    }
}

/// On a touch screen, a menu's or drop-down's rows are about 44 points high.
fn touch_rows(ui: &mut Ui) {
    if widgets::touch() {
        ui.spacing_mut().button_padding.y = 14.0;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Work {
    Start,
    Load,
    Queue,
    Download,
    /// Download over the user's edits.
    Replace,
}

/// Runs `work` for the script at `path` on the repo's thread; whether it started (one task
/// runs at a time).
fn run(path: &str, work: Work) -> bool {
    let bot = crate::turu::bot();
    let path = path.to_owned();
    let repo = crate::turu::script_repo();
    match work {
        // Downloads the script (and what it imports) if needed, then loads it in the active tab
        // and, for Start, starts it.
        Work::Start | Work::Load => {
            let start = work == Work::Start;
            repo.spawn(if start { "Start" } else { "Load" }, move |repo| {
                let local = repo.ensure(&path)?;
                bot.scripts.load(&local);
                bot.log(format!("Loaded script {}", local.display()));
                if start {
                    turu_bot::script::start(&bot).map_err(|e| format!("couldn't start the script: {e}"))?;
                    return Ok(format!("Started {path}."));
                }
                Ok(format!("Loaded {path}."))
            })
        }
        // Scheduler's Queue (VibeSkua `QueueScript`): downloads the script if needed, then adds
        // it to the active tab's playlist.
        Work::Queue => repo.spawn("Queue", move |repo| {
            let local = repo.ensure(&path)?;
            bot.scheduler.add(&local)?;
            Ok(format!("Queued {path}."))
        }),
        Work::Download | Work::Replace => repo.spawn("Download", move |repo| {
            let count = repo.download(&[path], work == Work::Replace)?;
            Ok(format!("Downloaded {count} files."))
        }),
    }
}

/// A row's left side: name, badges and path; the description; the tags. Returns its height.
fn row_text(ui: &mut Ui, entry: &Entry) -> f32 {
    let info = &entry.info;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.horizontal(|ui| {
            if entry.pinned {
                ui.label(RichText::new("★").color(WARNING)).on_hover_text("Pinned");
            }
            ui.label(RichText::new(&info.name).strong());
            badges(ui, entry.status);
            ui.add(egui::Label::new(RichText::new(&info.path).monospace().size(10.5).color(MUTED)).truncate());
        });
        // No description: an empty line, so the rows keep one height.
        let description = info.description.as_str();
        ui.add(egui::Label::new(RichText::new(description).color(MUTED)).truncate())
            .on_hover_text(description);
        tag_chips(ui, &info.tags);
    })
    .response
    .rect
    .height()
}

/// The tags as chips on one line: as many as fit in the width left, then "+N", whose hover
/// lists the others. One line keeps the rows the same height (the list only lays out the rows
/// in view).
fn tag_chips(ui: &mut Ui, tags: &[String]) {
    let font = FontId::proportional(10.0);
    let layout = |text: String| ui.painter().layout_no_wrap(text, font.clone(), MUTED);
    let galleys: Vec<_> = tags.iter().map(|t| layout(t.clone())).collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 2.0 * CHIP_PADDING).collect();
    let shown = fit_chips(&widths, ui.available_width(), |hidden| {
        layout(format!("+{hidden}")).size().x + 2.0 * CHIP_PADDING
    });
    let hidden = tags.len() - shown;
    let more = (hidden > 0).then(|| layout(format!("+{hidden}")));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = CHIP_GAP;
        for (galley, width) in galleys.into_iter().zip(widths).take(shown) {
            chip(ui, galley, width);
        }
        if let Some(galley) = more {
            let width = galley.size().x + 2.0 * CHIP_PADDING;
            chip(ui, galley, width).on_hover_text(tags[shown..].join(", "));
        }
    });
}

/// How many chips of `widths` fit in `available`, leaving room for the "+N" chip (`more(N)`
/// wide) when some don't.
fn fit_chips(widths: &[f32], available: f32, more: impl Fn(usize) -> f32) -> usize {
    let mut used = 0.0;
    for (i, width) in widths.iter().enumerate() {
        let next = used + if i > 0 { CHIP_GAP } else { 0.0 } + width;
        let rest = widths.len() - i - 1;
        let reserve = if rest > 0 { CHIP_GAP + more(rest) } else { 0.0 };
        if next + reserve > available {
            return i;
        }
        used = next;
    }
    widths.len()
}

fn chip(ui: &mut Ui, galley: std::sync::Arc<egui::Galley>, width: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, CHIP_HEIGHT), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same((CHIP_HEIGHT / 2.0) as u8), TRACK);
    let text = rect.center() - galley.size() / 2.0;
    ui.painter().galley(text, galley, MUTED);
    response
}

/// VibeSkua's "Update" and "Local" badges, and Turu's Modified and Removed.
fn badges(ui: &mut Ui, status: Status) {
    let (label, color, tip) = match status {
        Status::Outdated => ("Update", WARNING, "The repository has a newer version"),
        Status::Modified => ("Modified", WARNING, "Edited here: updates keep it"),
        Status::Local => ("Local", MUTED, "Your own script, not in the repository"),
        Status::Removed => ("Removed", MUTED, "No longer in the repository"),
        Status::UpToDate => ("Downloaded", SUCCESS, "Up to date"),
        Status::NotDownloaded => return,
    };
    widgets::pill(ui, |ui| ui.label(RichText::new(label).size(10.0).color(color))).on_hover_text(tip);
}

fn empty_state(ui: &mut Ui, title: &str, text: &str) {
    ui.add_space(60.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).strong().size(14.0));
        ui.label(RichText::new(text).color(MUTED));
    });
}

fn capitalize(folder: &str) -> String {
    let mut chars = folder.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use turu_bot::repo::ScriptInfo;

    fn entry(path: &str, name: &str, added: &str, status: Status, pinned: bool) -> Entry {
        Entry {
            info: ScriptInfo {
                path: path.into(),
                name: name.into(),
                description: format!("farms {name}"),
                added: added.into(),
                ..Default::default()
            },
            status,
            pinned,
        }
    }

    fn names(window: &ScriptRepoWindow, snapshot: &Snapshot) -> Vec<String> {
        window
            .filtered
            .iter()
            .map(|&i| snapshot.entries[i].info.name.clone())
            .collect()
    }

    /// VibeSkua's order: pinned first, then the sort; a search orders by rank; the categories
    /// are the top folders.
    /// A row's text fits the default row height with Turu's theme (so rows don't grow on the
    /// first frame), tags included.
    #[test]
    fn row_text_fits_the_row() {
        let ctx = egui::Context::default();
        super::super::theme::install(&ctx);
        let mut e = entry("farm/GoldFarm.ts", "Gold Farm", "", Status::Outdated, true);
        e.info.tags = vec!["gold".into(), "farm".into()];
        for _ in 0..2 {
            let mut height = 0.0;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                height = ui.allocate_ui(vec2(500.0, 200.0), |ui| row_text(ui, &e)).inner;
            });
            output.textures_delta.clear();
            assert!(
                height + 2.0 * ROW_MARGIN <= ROW_HEIGHT,
                "the row's text is {height} px: raise ROW_HEIGHT"
            );
        }
    }

    /// Tags fill the line, then "+N" takes the place of the rest.
    #[test]
    fn tags_that_dont_fit_become_plus_n() {
        let more = |_: usize| 20.0;
        // 3 x 30 + 2 gaps = 98: all fit in 100, no "+N" needed.
        assert_eq!(fit_chips(&[30.0, 30.0, 30.0], 100.0, more), 3);
        // In 90, two chips (64) leave no room for the third; with "+1" (24) they take 88.
        assert_eq!(fit_chips(&[30.0, 30.0, 30.0], 90.0, more), 2);
        // In 60, one chip (30) and "+2" (24).
        assert_eq!(fit_chips(&[30.0, 30.0, 30.0], 60.0, more), 1);
        assert_eq!(fit_chips(&[30.0, 30.0, 30.0], 10.0, more), 0);
        assert_eq!(fit_chips(&[], 10.0, more), 0);
    }

    /// The chips stay inside the width they're given, whatever the tags.
    #[test]
    fn tag_chips_stay_in_their_width() {
        let ctx = egui::Context::default();
        let tags: Vec<String> = [
            "gold",
            "battle ground e",
            "honor hall",
            "berserker bunny",
            "farm",
            "a very long tag indeed",
        ]
        .map(String::from)
        .into();
        for width in [40.0, 120.0, 260.0, 600.0] {
            let mut right = 0.0;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                ui.allocate_ui(vec2(width, 30.0), |ui| {
                    let left = ui.min_rect().left();
                    tag_chips(ui, &tags);
                    right = ui.min_rect().right() - left;
                });
            });
            output.textures_delta.clear();
            assert!(right <= width + 0.5, "{right} > {width}");
        }
    }

    /// The window's symbols are in Turu's fonts (no empty boxes).
    #[test]
    fn symbols_render() {
        let ctx = egui::Context::default();
        super::super::theme::install(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
        output.textures_delta.clear();
        let missing: Vec<char> = "★…"
            .chars()
            .filter(|&c| !ctx.fonts_mut(|f| f.has_glyph(&FontId::proportional(13.0), c)))
            .collect();
        assert!(missing.is_empty(), "missing glyphs: {missing:?}");
    }

    /// Delete says what has no other copy.
    #[test]
    fn delete_warns_about_your_own_work() {
        assert!(delete_text(Status::Local).contains("your own script"));
        assert!(delete_text(Status::Modified).contains("Your edits"));
        assert!(delete_text(Status::UpToDate).contains("download it again"));
    }

    #[test]
    fn filters_and_sorts_like_vibeskua() {
        let snapshot = Snapshot {
            entries: Arc::new(vec![
                entry(
                    "farm/GoldFarm.ts",
                    "Gold Farm",
                    "2026-01-02T00:00:00Z",
                    Status::UpToDate,
                    false,
                ),
                entry(
                    "nation/Bags.ts",
                    "Bags",
                    "2026-01-03T00:00:00Z",
                    Status::NotDownloaded,
                    false,
                ),
                entry(
                    "farm/Exp.ts",
                    "Experience",
                    "2026-01-01T00:00:00Z",
                    Status::Outdated,
                    true,
                ),
                entry("mine/Gold.ts", "My Gold", "", Status::Local, false),
                Entry {
                    info: ScriptInfo {
                        library: true,
                        ..entry("core/CoreBots.ts", "CoreBots", "", Status::UpToDate, true).info
                    },
                    status: Status::UpToDate,
                    pinned: true,
                },
            ]),
            generation: 1,
            commit: String::new(),
            fetched: String::new(),
            busy: None,
            message: String::new(),
            failed: false,
        };
        let mut window = ScriptRepoWindow {
            category: "All".into(),
            ..Default::default()
        };
        window.refilter(&snapshot);
        assert_eq!(
            names(&window, &snapshot),
            ["Experience", "Bags", "Gold Farm", "My Gold"]
        );
        assert_eq!(window.categories, ["All", "Farm", "Mine", "Nation"]);

        window.sort = Sort::Added;
        window.descending = true;
        window.refilter(&snapshot);
        assert_eq!(
            names(&window, &snapshot),
            ["Experience", "Bags", "Gold Farm", "My Gold"]
        );

        // "gold" is in two names (rank 0) and in no description.
        window.search = "gold".into();
        window.refilter(&snapshot);
        assert_eq!(names(&window, &snapshot), ["Gold Farm", "My Gold"]);

        window.search.clear();
        window.category = "Farm".into();
        window.refilter(&snapshot);
        assert_eq!(names(&window, &snapshot), ["Experience", "Gold Farm"]);
        window.category = "All".into();
        window.local_only = true;
        window.refilter(&snapshot);
        assert_eq!(names(&window, &snapshot), ["My Gold"]);
        window.local_only = false;
        window.pinned_only = true;
        window.refilter(&snapshot);
        assert_eq!(names(&window, &snapshot), ["Experience"]);
    }
}
