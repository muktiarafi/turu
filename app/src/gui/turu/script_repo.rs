//! Search Scripts (VibeSkua's 969x500 Script Repo, `ScriptRepoControl`): the repository's
//! scripts and your own, searchable, with Start, Load, Pin, Download/Update and Delete, and
//! Update all / Download all / Reset Scripts. The work runs in `turu_bot::repo` on a thread of
//! its own; this window shows its snapshot.

use super::files;
use super::theme::{MUTED, SUCCESS, TRACK, WARNING};
use super::widgets::{self, Kind};
use egui::{Align, ComboBox, Context, Layout, Modal, RichText, ScrollArea, Ui, vec2};
use std::path::PathBuf;
use std::time::Duration;
use turu_bot::repo::{self, Entry, Scope, Snapshot, Status};

/// One row of the list.
const ROW_HEIGHT: f32 = 66.0;

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
    sort: Sort,
    descending: bool,
}

#[derive(Default)]
pub struct ScriptRepoWindow {
    search: String,
    scope: Scope,
    /// "All", "Pinned", "Local" or a folder.
    category: String,
    sort: Sort,
    descending: bool,
    filtered: Vec<usize>,
    filter: Option<Filter>,
    categories: Vec<String>,
    confirm_reset: bool,
    vscode: Option<Option<PathBuf>>,
}

impl ScriptRepoWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if !*open {
            return;
        }
        let repo = crate::turu::script_repo();
        let snapshot = repo.snapshot();
        super::window(ctx, "Search Scripts", [969.0, 500.0], 2)
            .open(open)
            .show(ctx, |ui| self.ui(ui, &snapshot));
        if self.confirm_reset {
            self.reset_modal(ctx);
        }
        if snapshot.busy.is_some() {
            ctx.request_repaint_after(Duration::from_millis(150));
        }
    }

    fn ui(&mut self, ui: &mut Ui, snapshot: &Snapshot) {
        if self.category.is_empty() {
            self.category = "All".into();
        }
        self.refilter(snapshot);
        ui.spacing_mut().item_spacing = vec2(6.0, 8.0);

        // Search box, scope, category, sort.
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 120.0 - 140.0 - 120.0 - 34.0 - 60.0).max(160.0);
            widgets::text_box(ui, &mut self.search, "Search scripts by name, path or tag", width);
            if widgets::button(ui, "✕", Kind::Normal, !self.search.is_empty(), vec2(26.0, 26.0), "")
                .on_hover_text("Clear")
                .clicked()
            {
                self.search.clear();
            }
            ComboBox::from_id_salt("turu_repo_scope")
                .width(110.0)
                .selected_text(self.scope.label())
                .show_ui(ui, |ui| {
                    for scope in Scope::ALL {
                        ui.selectable_value(&mut self.scope, scope, scope.label());
                    }
                });
            ComboBox::from_id_salt("turu_repo_category")
                .width(130.0)
                .selected_text(self.category.as_str())
                .show_ui(ui, |ui| {
                    for category in &self.categories {
                        ui.selectable_value(&mut self.category, category.clone(), category);
                    }
                });
            ComboBox::from_id_salt("turu_repo_sort")
                .width(110.0)
                .selected_text(match self.sort {
                    Sort::Name => "Name",
                    Sort::Added => "Date added",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort, Sort::Name, "Name");
                    ui.selectable_value(&mut self.sort, Sort::Added, "Date added");
                });
            let (arrow, tip) = match (self.sort, self.descending) {
                (Sort::Name, false) => ("↑", "Name A to Z"),
                (Sort::Name, true) => ("↓", "Name Z to A"),
                (Sort::Added, false) => ("↑", "Oldest first"),
                (Sort::Added, true) => ("↓", "Newest first"),
            };
            if widgets::button(ui, arrow, Kind::Normal, true, vec2(26.0, 26.0), "")
                .on_hover_text(tip)
                .clicked()
            {
                self.descending = !self.descending;
            }
        });

        // Counts.
        let entries = &snapshot.entries;
        let count = |f: fn(&Entry) -> bool| entries.iter().filter(|e| f(e)).count();
        ui.horizontal(|ui| {
            let text = format!(
                "{} shown · {} in the repository · {} downloaded · {} to update · {} local · {} pinned",
                self.filtered.len(),
                count(|e| !matches!(e.status, Status::Local | Status::Removed)),
                count(|e| e.status.is_downloaded() && e.status != Status::Local),
                count(|e| e.status == Status::Outdated),
                count(|e| e.status == Status::Local),
                count(|e| e.pinned),
            );
            ui.label(RichText::new(text).size(11.0).color(MUTED));
        });

        // The list, above the bottom bar.
        let list_height = (ui.available_height() - 44.0).max(80.0);
        ui.allocate_ui(vec2(ui.available_width(), list_height), |ui| {
            if entries.is_empty() {
                let (title, text) = if snapshot.busy.is_some() {
                    (
                        "Loading scripts...",
                        "Getting the script list, this can take a few seconds.",
                    )
                } else {
                    ("No scripts yet", "Refresh to get the script list.")
                };
                empty_state(ui, title, text);
            } else if self.filtered.is_empty() {
                empty_state(ui, "No scripts found", "Nothing matches the search or category.");
                ui.vertical_centered(|ui| {
                    if widgets::button(ui, "Clear search", Kind::Normal, true, vec2(110.0, 28.0), "").clicked() {
                        self.search.clear();
                        self.scope = Scope::All;
                        self.category = "All".into();
                    }
                });
            } else {
                let busy = snapshot.busy.is_some();
                ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                    ui,
                    ROW_HEIGHT,
                    self.filtered.len(),
                    |ui, rows| {
                        for i in rows {
                            self.row(ui, &entries[self.filtered[i]], busy);
                        }
                    },
                );
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
            sort: self.sort,
            descending: self.descending,
        };
        if self.filter.as_ref() == Some(&filter) {
            return;
        }
        let entries = &snapshot.entries;
        let mut folders: Vec<String> = entries
            .iter()
            .filter_map(|e| e.info.category())
            .map(capitalize)
            .collect();
        folders.sort();
        folders.dedup();
        self.categories = ["All", "Pinned", "Local"].map(String::from).into();
        self.categories.extend(folders);

        let mut rows: Vec<(usize, u8)> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| match self.category.as_str() {
                "All" => true,
                "Pinned" => e.pinned,
                "Local" => e.status == Status::Local,
                category => repo::in_category(&e.info, category),
            })
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

    fn row(&mut self, ui: &mut Ui, entry: &Entry, busy: bool) {
        let info = &entry.info;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), egui::Sense::hover());
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, 4.0))));
        let ui = &mut child;
        ui.horizontal(|ui| {
            let buttons = 64.0 + 56.0 + 30.0 + 18.0;
            ui.allocate_ui(vec2(ui.available_width() - buttons, ROW_HEIGHT - 8.0), |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.horizontal(|ui| {
                        if entry.pinned {
                            ui.label(RichText::new("★").color(WARNING));
                        }
                        ui.label(RichText::new(&info.name).strong());
                        badges(ui, entry.status);
                        ui.label(RichText::new(&info.path).monospace().size(10.5).color(MUTED));
                    });
                    let description = if info.description.is_empty() {
                        "No description provided."
                    } else {
                        &info.description
                    };
                    ui.add(egui::Label::new(RichText::new(description).color(MUTED)).truncate())
                        .on_hover_text(description);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for tag in info.tags.iter().take(8) {
                            widgets::pill(ui, |ui| {
                                ui.label(RichText::new(tag).size(10.0).color(MUTED));
                            });
                        }
                    });
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.row_menu(ui, entry, busy);
                let bot = crate::turu::bot();
                let active = bot.scripts.status().is_active();
                let auto = super::auto::running(&bot);
                let tip = if busy {
                    "Wait for the current task"
                } else if active {
                    "Stop the script first"
                } else {
                    "Stop Auto Attack first"
                };
                if widgets::button(ui, "Load", Kind::Normal, !busy && !active, vec2(56.0, 28.0), tip).clicked() {
                    load(&info.path, false);
                }
                if widgets::button(
                    ui,
                    "Start",
                    Kind::Primary,
                    !busy && !active && !auto,
                    vec2(64.0, 28.0),
                    tip,
                )
                .clicked()
                {
                    load(&info.path, true);
                }
            });
        });
        ui.painter()
            .hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0, TRACK));
    }

    fn row_menu(&mut self, ui: &mut Ui, entry: &Entry, busy: bool) {
        let info = &entry.info;
        let repo = crate::turu::script_repo();
        ui.menu_button("…", |ui| {
            ui.set_min_width(190.0);
            if ui.button(if entry.pinned { "Unpin" } else { "Pin" }).clicked() {
                repo.set_pinned(&info.path, !entry.pinned);
                ui.close();
            }
            let download = match entry.status {
                Status::NotDownloaded => Some(("Download", false)),
                Status::Outdated => Some(("Update", false)),
                Status::Modified => Some(("Replace with the repository's", true)),
                _ => None,
            };
            if let Some((label, overwrite)) = download
                && ui.add_enabled(!busy, egui::Button::new(label)).clicked()
            {
                let path = info.path.clone();
                repo.spawn("Download", move |repo| {
                    let count = repo.download(&[path], overwrite)?;
                    Ok(format!("Downloaded {count} files."))
                });
                ui.close();
            }
            let local = repo.local_path(&info.path).filter(|p| p.is_file());
            let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
            if ui
                .add_enabled(local.is_some() && code.is_some(), egui::Button::new("Open in VS Code"))
                .clicked()
                && let (Some(code), Some(local)) = (&code, &local)
            {
                files::open_in_vscode(code, local);
                ui.close();
            }
            if ui
                .add_enabled(local.is_some() && !busy, egui::Button::new("Delete"))
                .clicked()
            {
                if let Err(e) = repo.delete(&info.path) {
                    crate::turu::bot().log(format!("Couldn't delete {}: {e}", info.path));
                }
                ui.close();
            }
        });
    }

    fn bottom_bar(&mut self, ui: &mut Ui, snapshot: &Snapshot) {
        let repo = crate::turu::script_repo();
        let busy = snapshot.busy.is_some();
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            if widgets::button(
                ui,
                "Update all",
                Kind::Primary,
                !busy,
                vec2(96.0, 30.0),
                "Wait for the current task",
            )
            .on_hover_text("Update the outdated scripts (your edits are kept)")
            .clicked()
            {
                repo.spawn("Update all", |repo| repo.update_all());
            }
            if widgets::button(
                ui,
                "Download all",
                Kind::Normal,
                !busy,
                vec2(104.0, 30.0),
                "Wait for the current task",
            )
            .on_hover_text("Download every script that's missing or outdated")
            .clicked()
            {
                repo.spawn("Download all", |repo| repo.download_all());
            }
            if widgets::button(
                ui,
                "⟳",
                Kind::Normal,
                !busy,
                vec2(30.0, 30.0),
                "Wait for the current task",
            )
            .on_hover_text("Refresh the script list")
            .clicked()
            {
                repo.spawn("Refresh", |repo| repo.refresh());
            }
            ui.menu_button("…", |ui| {
                ui.set_min_width(200.0);
                if ui.add_enabled(!busy, egui::Button::new("Reset Scripts...")).clicked() {
                    self.confirm_reset = true;
                    ui.close();
                }
                let dir = crate::turu::bot().paths.scripts_dir();
                if ui.button("Open the scripts folder").clicked() {
                    files::open_with_default(&dir);
                    ui.close();
                }
                let code = self.vscode.get_or_insert_with(files::find_vscode).clone();
                if ui
                    .add_enabled(code.is_some(), egui::Button::new("Open the scripts folder in VS Code"))
                    .clicked()
                    && let Some(code) = &code
                {
                    files::open_in_vscode(code, &dir);
                    ui.close();
                }
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(progress) = &snapshot.busy {
                    if widgets::button(ui, "Cancel", Kind::Outline, true, vec2(70.0, 26.0), "").clicked() {
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
                    ui.add(egui::Label::new(RichText::new(&progress.message).size(11.0)).truncate());
                } else {
                    let mut text = snapshot.message.clone();
                    if !snapshot.commit.is_empty() {
                        let commit: String = snapshot.commit.chars().take(7).collect();
                        let fetched = snapshot
                            .fetched
                            .get(..16)
                            .unwrap_or(&snapshot.fetched)
                            .replace('T', " ");
                        if !text.is_empty() {
                            text.push_str("  ·  ");
                        }
                        text.push_str(&format!("{commit}, checked {fetched} UTC"));
                    }
                    ui.add(egui::Label::new(RichText::new(text).size(11.0).color(MUTED)).truncate());
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
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::button(ui, "Reset", Kind::Danger, true, vec2(72.0, 28.0), "").clicked() {
                    crate::turu::script_repo().spawn("Reset Scripts", |repo| repo.reset());
                    self.confirm_reset = false;
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, vec2(72.0, 28.0), "").clicked() {
                    self.confirm_reset = false;
                }
            });
        });
        if response.should_close() {
            self.confirm_reset = false;
        }
    }
}

/// Downloads the script (and what it imports) if needed, then loads it in the active tab and,
/// with `start`, starts it.
fn load(path: &str, start: bool) {
    let bot = crate::turu::bot();
    let path = path.to_owned();
    let task = if start { "Start" } else { "Load" };
    crate::turu::script_repo().spawn(task, move |repo| {
        let local = repo.ensure(&path)?;
        bot.scripts.load(&local);
        bot.log(format!("Loaded script {}", local.display()));
        if start {
            turu_bot::script::start(&bot).map_err(|e| format!("couldn't start the script: {e}"))?;
            return Ok(format!("Started {path}."));
        }
        Ok(format!("Loaded {path}."))
    });
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
            ]),
            generation: 1,
            commit: String::new(),
            fetched: String::new(),
            busy: None,
            message: String::new(),
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
        assert_eq!(window.categories, ["All", "Pinned", "Local", "Farm", "Mine", "Nation"]);

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
        window.category = "Local".into();
        window.refilter(&snapshot);
        assert_eq!(names(&window, &snapshot), ["My Gold"]);
    }
}
