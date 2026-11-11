//! Search Scripts (VibeSkua's 969x500 Script Repo, `ScriptRepoControl`): the repository's
//! scripts and your own, searchable, with Start, Load, Queue, Pin, Download/Update and Delete, and
//! Update all / Download all / Reset Scripts. The work runs in `turu_bot::repo` on a thread of
//! its own; this window shows its snapshot. On Android it is the only way to get a script: on a
//! phone it fills the screen, the filters fold behind a chip ([`Header`]), the rows are two
//! lines with Start and a menu, and every target is finger-sized.

use super::files;
use super::theme::{DANGER, MUTED, SUCCESS, TRACK, WARNING};
use super::widgets::{self, Kind};
use egui::{
    Align, Button, ComboBox, Context, CornerRadius, FontId, Key, Layout, Modal, Rect, RichText, ScrollArea, Sense, Ui,
    pos2, vec2,
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

/// The header's layouts, by the room the window has: at least [`WIDE`] across, the search
/// box and the filters share one line; at least [`NARROW`] across and [`SHORT`] high, the
/// filters take a line of their own; smaller (a phone, either way up), the filters fold behind
/// a Filters chip and the rows drop their path and tags ([`Header::Compact`]).
const WIDE: f32 = 900.0;
const SHORT: f32 = 420.0;
/// What the filters take on the search box's line in [`Header::Wide`].
const FILTERS_WIDTH: f32 = 660.0;
/// Narrower than this, the bottom bar's status takes a line above its buttons.
const STACKED_BAR: f32 = 520.0;

/// How the header and rows are laid out ([`Header::pick`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Header {
    /// Search and filters on one line.
    Wide,
    /// Search on a line, the filters (wrapping) on the next.
    Stacked,
    /// Search and a Filters chip; the filters open below it. Rows are two lines.
    Compact,
}

impl Header {
    fn pick(width: f32, height: f32) -> Self {
        if width < NARROW || height < SHORT {
            Header::Compact
        } else if width >= WIDE {
            Header::Wide
        } else {
            Header::Stacked
        }
    }
}

/// A [`Header::Compact`] row at least: name, then badge and description (51 px with Turu's
/// theme), plus the margins; it fits a 48-point touch button too.
const COMPACT_ROW_HEIGHT: f32 = 60.0;

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
    /// The rows' height: [`ROW_HEIGHT`] (or [`COMPACT_ROW_HEIGHT`]), or more once a row's text
    /// needed it; worked out again when [`Self::row_compact`] changes.
    row_height: f32,
    row_compact: bool,
    /// The Filters chip of [`Header::Compact`] is open.
    filters_open: bool,
    /// The scripts there are to search (libraries left out), for the search box's hint.
    total: usize,
    vscode: Option<Option<PathBuf>>,
    /// Opened to choose a script for another window ([`Self::pick`]).
    pick: Option<Pick>,
    /// A script was picked: the window closes once its download is done.
    picked: bool,
    /// Closes the window after this frame (a ready script was picked, or Cancel).
    close: bool,
}

/// Search Scripts opened to choose a script for another window (Accounts' launch script): the
/// rows' Pick hands the script's local path back, downloading it first when needed.
struct Pick {
    /// Who the script is for, on the banner ("launched accounts").
    purpose: &'static str,
    answer: crate::platform::Answer,
}

/// What the rows may do now, before asking whether a script is ready ([`Rules::for_row`]).
struct Rules {
    /// A repo task runs: only ready scripts can be loaded or started.
    busy: bool,
    start: bool,
    load: bool,
    queue: bool,
    /// Why Start or Load can't (another script runs, ...).
    reason: String,
    /// Why a script that needs a download waits ([`wait_tip`]).
    wait: String,
    queue_tip: String,
}

/// What one row's buttons may do, and why not.
struct Allowed {
    /// Pick (in [`ScriptRepoWindow::pick`]'s mode): a script that needs a download waits
    /// while a task runs.
    pick: bool,
    start: bool,
    load: bool,
    queue: bool,
    run_tip: String,
    queue_tip: String,
}

impl Rules {
    /// While a task runs, a script that's downloaded and current still loads and starts
    /// (`ScriptRepo::ready`); one that needs a download waits.
    fn for_row(&self, path: &str) -> Allowed {
        let can_run = !self.busy || crate::turu::script_repo().ready(path).is_some();
        let run_tip = if !can_run {
            format!("{} (this script needs a download first)", self.wait)
        } else {
            self.reason.clone()
        };
        Allowed {
            pick: can_run,
            start: self.start && can_run,
            load: self.load && can_run,
            queue: self.queue,
            run_tip,
            queue_tip: self.queue_tip.clone(),
        }
    }
}

/// How [`ScriptRepoWindow::row`] lays a row out.
#[derive(Clone, Copy)]
struct RowStyle {
    /// Start and the menu only (Load and Queue are in it).
    narrow: bool,
    /// Two lines of text, no path or tags ([`Header::Compact`]).
    compact: bool,
    /// The top search hit: Start has the accent.
    top_hit: bool,
    min_height: f32,
}

/// How a row's work went at once.
enum Ran {
    /// It runs as the repo's task.
    Spawned,
    /// Done at once (a ready script); whether it worked.
    Done(bool),
    /// Another task runs.
    Refused,
}

/// The key that, with Enter in the search box, starts the top hit.
const COMMAND_KEY: &str = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };

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
        // The picked script is downloaded (or its download failed, which the bottom bar says).
        if self.picked && snapshot.busy.is_none() {
            self.picked = false;
            if !snapshot.failed {
                *open = false;
            }
        }
        if !*open {
            // Closed without a pick: the asking window gets no script.
            self.pick = None;
            return;
        }
        // On a phone it's the only way to get a script, and a floating window there leaves
        // half the list off screen: it fills the screen instead.
        let mut window = super::window(ctx, "Search Scripts", [969.0, 500.0], 2).open(open);
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 1000.0 || area.height() < 600.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui, &snapshot));
        if std::mem::take(&mut self.close) {
            *open = false;
        }
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

    /// Opens the window's pick mode: the rows offer Pick, which calls `answer` with the
    /// script's local path (downloaded first if needed) and closes the window. Closing it
    /// without a pick drops `answer`. The caller opens the window.
    pub fn pick(&mut self, purpose: &'static str, answer: crate::platform::Answer) {
        self.pick = Some(Pick { purpose, answer });
        self.picked = false;
    }

    /// Hands the script at `path` to the window that asked ([`Self::pick`]): at once when it's
    /// ready, else once the repo's task has downloaded it.
    fn pick_script(&mut self, path: &str) {
        let repo = crate::turu::script_repo();
        let Some(pick) = self.pick.take() else {
            return;
        };
        if let Some(local) = repo.ready(path) {
            (pick.answer)(Some(local));
            self.close = true;
            return;
        }
        if repo.is_busy() {
            self.pick = Some(pick);
            return;
        }
        let owned = path.to_owned();
        let answer = pick.answer;
        let spawned = repo.spawn("Download", move |repo| {
            let local = repo.ensure(&owned)?;
            answer(Some(local));
            Ok(format!("Picked {owned}."))
        });
        if spawned {
            self.working = Some((path.to_owned(), false));
            self.picked = true;
        }
    }

    /// Pick mode's banner: what the script is for, and Cancel.
    fn pick_banner(&mut self, ui: &mut Ui) {
        let Some(pick) = &self.pick else {
            return;
        };
        let text = format!("Pick the script for {}", pick.purpose);
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let size = vec2(80.0, widgets::target(24.0, 40.0));
                    if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                        self.pick = None;
                        self.close = true;
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add(egui::Label::new(RichText::new(text).strong()).truncate());
                    });
                });
            });
        });
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
        self.pick_banner(ui);
        let touch = widgets::touch();
        let header = Header::pick(ui.available_width(), ui.available_height());
        // The rows keep Start and the menu on a touch screen too (no hover for the rest).
        let narrow = touch || ui.available_width() < NARROW;
        let entries = &snapshot.entries;

        // Enter in the search box loads the top hit, Cmd/Ctrl+Enter starts it.
        let mut enter = None;
        let mut search_box = |this: &mut Self, ui: &mut Ui, width: f32| {
            let search = this.search_box(ui, width);
            if search.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                enter = Some(ui.input(|i| i.modifiers.command));
            }
        };
        match header {
            Header::Wide => {
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - FILTERS_WIDTH).max(200.0);
                    search_box(self, ui, width);
                    self.filters(ui, header);
                });
            }
            Header::Stacked => {
                let width = ui.available_width();
                search_box(self, ui, width);
                ui.horizontal_wrapped(|ui| self.filters(ui, header));
            }
            Header::Compact => {
                let active = self.active_filters();
                let label = if active > 0 {
                    format!("Filters · {active}")
                } else {
                    "Filters".to_owned()
                };
                let found =
                    (active > 0 || !self.search.trim().is_empty()).then(|| format!("{} found", self.filtered.len()));
                ui.horizontal(|ui| {
                    let text_width = |ui: &Ui, text: &str, size: f32| {
                        let font = FontId::proportional(size);
                        ui.painter().layout_no_wrap(text.to_owned(), font, MUTED).size().x
                    };
                    // The chip is its text plus 22 (`widgets::chip`).
                    let gap = ui.spacing().item_spacing.x;
                    let mut right = text_width(ui, &label, 12.0) + 22.0 + gap;
                    if let Some(found) = &found {
                        right += text_width(ui, found, 12.0) + gap;
                    }
                    let width = (ui.available_width() - right).max(80.0);
                    search_box(self, ui, width);
                    if let Some(found) = &found {
                        ui.label(RichText::new(found).size(12.0).color(MUTED));
                    }
                    if widgets::chip(ui, &label, self.filters_open) {
                        self.filters_open = !self.filters_open;
                    }
                });
                if self.filters_open {
                    ui.horizontal_wrapped(|ui| self.filters(ui, header));
                }
            }
        }

        let rules = rules(snapshot);
        if let Some(start) = enter
            && !self.search.trim().is_empty()
            && let Some(&top) = self.filtered.first()
        {
            let path = entries[top].info.path.clone();
            let allowed = rules.for_row(&path);
            if self.pick.is_some() {
                if allowed.pick {
                    self.pick_script(&path);
                }
            } else if start && allowed.start {
                self.run(&path, Work::Start);
            } else if !start && allowed.load {
                self.run(&path, Work::Load);
            }
        }

        // The list, above the bottom bar (narrower than STACKED_BAR, two lines: the status,
        // then the buttons).
        let stacked = ui.available_width() < STACKED_BAR;
        let list_height = (ui.available_height() - bar_height(snapshot, stacked)).max(80.0);
        let compact = header == Header::Compact;
        if self.row_compact != compact {
            self.row_compact = compact;
            self.row_height = 0.0;
        }
        let min_row = if compact { COMPACT_ROW_HEIGHT } else { ROW_HEIGHT };
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
                        self.clear_filters();
                    }
                });
            } else {
                // The top search hit gets the accent on a touch screen (no hover there).
                let top_hit = (!self.search.trim().is_empty()).then(|| self.filtered[0]);
                let filtered = std::mem::take(&mut self.filtered);
                ScrollArea::vertical().auto_shrink([false, false]).show_rows(
                    ui,
                    self.row_height.max(min_row),
                    filtered.len(),
                    |ui, rows| {
                        for i in rows {
                            let index = filtered[i];
                            let style = RowStyle {
                                narrow,
                                compact,
                                top_hit: top_hit == Some(index),
                                min_height: min_row,
                            };
                            self.row(ui, &entries[index], &rules, style);
                        }
                    },
                );
                self.filtered = filtered;
            }
        });

        self.bottom_bar(ui, snapshot, narrow, stacked);
    }

    /// The search box, `width` wide, with the scripts' count as its hint and a ✕ that clears
    /// it inside its right end.
    fn search_box(&mut self, ui: &mut Ui, width: f32) -> egui::Response {
        let height = widgets::target(26.0, 40.0);
        let hint = if self.total > 0 {
            format!("Search {} scripts", self.total)
        } else {
            "Search scripts".to_owned()
        };
        // The text stops short of the ✕ (a square as high as the box). The width is the box's,
        // margins included.
        let margin = egui::Margin {
            left: 8,
            right: height as i8,
            top: 5,
            bottom: 5,
        };
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text(hint)
                .desired_width(width.max(80.0))
                .min_size(vec2(0.0, height))
                .vertical_align(Align::Center)
                .margin(margin)
                .background_color(super::theme::INSET),
        );
        if !self.search.is_empty() {
            let rect = Rect::from_min_size(
                pos2(response.rect.right() - height, response.rect.top()),
                vec2(height, response.rect.height()),
            );
            let clear = ui
                .put(rect, Button::new(RichText::new("✕").color(MUTED)).frame(false))
                .on_hover_text("Clear the search");
            if clear.clicked() {
                self.search.clear();
            }
        }
        response
    }

    /// What the search looks in, the folder, Pinned and Local, and the order, then (but in
    /// [`Header::Compact`]) the count of what they leave and (but in [`Header::Wide`]) Clear.
    fn filters(&mut self, ui: &mut Ui, header: Header) {
        if widgets::touch() {
            ui.spacing_mut().interact_size.y = 40.0;
        }
        let control = widgets::target(26.0, 40.0);
        // A drop-down doesn't wrap in a wrapping line by itself: it moves down when it wouldn't fit.
        let fit = |ui: &mut Ui, width: f32| {
            if ui.available_size_before_wrap().x < width {
                ui.end_row();
            }
        };
        fit(ui, 100.0);
        ComboBox::from_id_salt("turu_repo_scope")
            .width(100.0)
            .selected_text(scope_label(self.scope))
            .show_ui(ui, |ui| {
                touch_rows(ui);
                for scope in Scope::ALL {
                    ui.selectable_value(&mut self.scope, scope, scope_label(scope));
                }
            })
            .response
            .on_hover_text("What the search looks in");
        fit(ui, 120.0);
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
        // VibeSkua `CycleSort`: one button steps through A to Z, Z to A, Newest, Oldest.
        let order = match (self.sort, self.descending) {
            (Sort::Name, false) => "Name A to Z",
            (Sort::Name, true) => "Name Z to A",
            (Sort::Added, true) => "Newest first",
            (Sort::Added, false) => "Oldest first",
        };
        fit(ui, 110.0);
        if widgets::button(ui, order, Kind::Normal, true, vec2(110.0, control), "")
            .on_hover_text("Change the order: name A to Z, Z to A, newest, oldest")
            .clicked()
        {
            (self.sort, self.descending) = match (self.sort, self.descending) {
                (Sort::Name, false) => (Sort::Name, true),
                (Sort::Name, true) => (Sort::Added, true),
                (Sort::Added, true) => (Sort::Added, false),
                (Sort::Added, false) => (Sort::Name, false),
            };
        }
        // Compact, the search box's line has the count.
        if header != Header::Compact {
            ui.label(
                RichText::new(format!("{} shown", self.filtered.len()))
                    .size(12.0)
                    .color(MUTED),
            );
        }
        // Wide, the line has no room for it (the empty list's Clear the filters is there).
        if header != Header::Wide
            && self.active_filters() > 0
            && widgets::button(ui, "Clear", Kind::Outline, true, vec2(56.0, control), "")
                .on_hover_text("Search in all fields, all folders, Pinned and Local off")
                .clicked()
        {
            self.clear_filters();
        }
    }

    /// The filters that leave scripts out (the order doesn't).
    fn active_filters(&self) -> usize {
        [
            self.scope != Scope::All,
            self.category != "All",
            self.pinned_only,
            self.local_only,
        ]
        .into_iter()
        .filter(|&on| on)
        .count()
    }

    fn clear_filters(&mut self) {
        self.scope = Scope::All;
        self.category = "All".into();
        self.pinned_only = false;
        self.local_only = false;
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
        self.total = entries.iter().filter(|e| !e.info.library).count();
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
                let by_name = || a.info.name.to_lowercase().cmp(&b.info.name.to_lowercase());
                match self.sort {
                    Sort::Name if self.descending => by_name().reverse(),
                    Sort::Name => by_name(),
                    // VibeSkua `CompareScripts`: scripts without a date go last either way, and
                    // scripts of the same date are in name order.
                    Sort::Added => {
                        let (a_dated, b_dated) = (!a.info.added.is_empty(), !b.info.added.is_empty());
                        b_dated.cmp(&a_dated).then_with(|| {
                            let dates = a.info.added.cmp(&b.info.added);
                            if self.descending { dates.reverse() } else { dates }
                        })
                    }
                    .then_with(by_name),
                }
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
    /// pointer and the top search hit, so a long list doesn't shout. Compact, the text is two
    /// lines ([`row_text_compact`]).
    fn row(&mut self, ui: &mut Ui, entry: &Entry, rules: &Rules, style: RowStyle) {
        let RowStyle {
            narrow,
            compact,
            top_hit,
            min_height,
        } = style;
        let info = &entry.info;
        let allowed = &rules.for_row(&info.path);
        let height = self.row_height.max(min_height);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
        let lead = top_hit || ui.rect_contains_pointer(rect);
        let working = self.working.as_ref().is_some_and(|(path, _)| *path == info.path);
        // Picking, a row has Pick and the menu only (without Load and Queue).
        let picking = self.pick.is_some();
        let menu_actions = narrow && !picking;
        let narrow = narrow || picking;
        let button_height = widgets::target(28.0, 48.0);
        let menu_width = widgets::target(30.0, 48.0);
        let spinner = if working { 24.0 } else { 0.0 };
        let buttons = if narrow { 64.0 } else { 64.0 + 56.0 + 64.0 } + menu_width + spinner + 24.0;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(4.0, ROW_MARGIN))));
        let ui = &mut child;
        ui.horizontal(|ui| {
            ui.allocate_ui(
                vec2((ui.available_width() - buttons).max(0.0), height - 2.0 * ROW_MARGIN),
                |ui| {
                    // Long names and paths stop at the buttons.
                    ui.set_clip_rect(ui.max_rect().intersect(ui.clip_rect()));
                    // The list needs one height for every row: grow them all if this one needs more.
                    let text = if compact {
                        row_text_compact(ui, entry)
                    } else {
                        row_text(ui, entry)
                    };
                    let needed = text + 2.0 * ROW_MARGIN;
                    if needed > height + 0.5 {
                        self.row_height = needed.ceil();
                        ui.ctx().request_repaint();
                    }
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.row_menu(
                    ui,
                    entry,
                    allowed,
                    menu_actions,
                    compact,
                    vec2(menu_width, button_height),
                );
                if picking {
                    let kind = if lead { Kind::Primary } else { Kind::Normal };
                    let pick = widgets::button(
                        ui,
                        "Pick",
                        kind,
                        allowed.pick,
                        vec2(64.0, button_height),
                        &allowed.run_tip,
                    )
                    .on_hover_text(if top_hit {
                        "Use this script (Enter); it's downloaded first if needed"
                    } else {
                        "Use this script; it's downloaded first if needed"
                    });
                    if pick.clicked() {
                        self.pick_script(&info.path);
                    }
                    if working {
                        ui.spinner();
                    }
                    return;
                }
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
                    .on_hover_text(if top_hit {
                        "Load it in the Script Loader (Enter)".to_owned()
                    } else {
                        "Load it in the Script Loader".to_owned()
                    })
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
                .on_hover_text(if top_hit {
                    format!("Load it and start it ({COMMAND_KEY}+Enter)")
                } else {
                    "Load it and start it".to_owned()
                })
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

    /// Starts `work` for the script at `path`, its row showing a spinner until it's done; a
    /// Start that worked brings up the Script Loader.
    fn run(&mut self, path: &str, work: Work) {
        match run(path, work) {
            Ran::Spawned => self.working = Some((path.to_owned(), work == Work::Start)),
            Ran::Done(worked) => self.open_loader |= worked && work == Work::Start,
            Ran::Refused => {}
        }
    }

    fn row_menu(
        &mut self,
        ui: &mut Ui,
        entry: &Entry,
        allowed: &Allowed,
        narrow: bool,
        compact: bool,
        size: egui::Vec2,
    ) {
        let info = &entry.info;
        let repo = crate::turu::script_repo();
        let busy = repo.is_busy();
        let button = Button::new("…").min_size(size);
        egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
            ui.set_min_width(190.0);
            touch_rows(ui);
            let touch = widgets::touch();
            // A compact row has no room for its path: the menu starts with it.
            if compact {
                ui.add(egui::Label::new(RichText::new(&info.path).monospace().size(11.0).color(MUTED)).wrap());
            }
            // A touch screen shows no tooltips: what the badge means, and why an item rests,
            // are lines in the menu.
            if touch && let Some((label, _, meaning)) = badge(entry.status) {
                reason_line(ui, &format!("{label}: {meaning}"));
            }
            if compact || (touch && badge(entry.status).is_some()) {
                ui.separator();
            }
            if narrow {
                let load = ui
                    .add_enabled(allowed.load, Button::new("Load"))
                    .on_disabled_hover_text(&allowed.run_tip);
                if load.clicked() {
                    self.run(&info.path, Work::Load);
                    ui.close();
                }
                if touch && !allowed.load {
                    reason_line(ui, &allowed.run_tip);
                }
                let queue = ui
                    .add_enabled(allowed.queue, Button::new("Add to the queue"))
                    .on_disabled_hover_text(&allowed.queue_tip);
                if queue.clicked() {
                    self.run(&info.path, Work::Queue);
                    ui.close();
                }
                if touch && !allowed.queue {
                    reason_line(ui, &allowed.queue_tip);
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

    /// Update all, Refresh, Download all and the menu, with the running task or the last
    /// result on the right. With `narrow` rows Download all moves into the menu; `stacked`
    /// (a phone held upright), the status takes a line above the buttons ([`bar_height`]).
    fn bottom_bar(&mut self, ui: &mut Ui, snapshot: &Snapshot, narrow: bool, stacked: bool) {
        let repo = crate::turu::script_repo();
        let busy = snapshot.busy.is_some();
        let height = widgets::target(30.0, 48.0);
        let outdated = outdated_count(&snapshot.entries);
        let wait = wait_tip(snapshot);
        ui.add_space(2.0);
        if stacked {
            let line = status_height(snapshot, height);
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), line),
                Layout::right_to_left(Align::Center),
                |ui| status_line(ui, snapshot, height),
            );
        }
        ui.horizontal(|ui| {
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
            if !narrow
                && widgets::button(ui, "Download all", Kind::Normal, !busy, vec2(104.0, height), &wait)
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
                if narrow && ui.add_enabled(!busy, Button::new("Download all")).clicked() {
                    repo.spawn("Download all", |repo| repo.download_all());
                    ui.close();
                }
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
            if !stacked {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    status_line(ui, snapshot, height)
                });
            }
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

/// What [`ScriptRepoWindow::bottom_bar`] takes under the list, its spacing included.
fn bar_height(snapshot: &Snapshot, stacked: bool) -> f32 {
    let line = widgets::target(30.0, 48.0);
    if stacked {
        line + status_height(snapshot, line) + 22.0
    } else {
        line + 14.0
    }
}

/// The status's line above a stacked bar: as high as its Cancel while a task runs, else one
/// line of small text.
fn status_height(snapshot: &Snapshot, line: f32) -> f32 {
    if snapshot.busy.is_some() { line } else { 18.0 }
}

/// The running task with its progress and Cancel, or the last result (red when it failed, a
/// green dot when it worked) and how fresh the list is. Laid out right to left.
fn status_line(ui: &mut Ui, snapshot: &Snapshot, height: f32) {
    if let Some(progress) = &snapshot.busy {
        if widgets::button(ui, "Cancel", Kind::Outline, true, vec2(70.0, height - 4.0), "").clicked() {
            crate::turu::script_repo().cancel();
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
        return;
    }
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

/// A muted line under a menu item: why it rests, or what a badge means (a touch screen shows
/// no tooltips).
fn reason_line(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(12.0).color(MUTED)).wrap());
}

/// What the rows can do now, and the reasons they can't (as tooltips, or in the menu).
fn rules(snapshot: &Snapshot) -> Rules {
    let bot = crate::turu::bot();
    let busy = snapshot.busy.is_some();
    let active = bot.scripts.status().is_active();
    let auto = super::auto::running(&bot);
    let snippet = bot.scripts.is_snippet_running();
    let queue_running = bot.scheduler.is_running();
    let wait = wait_tip(snapshot);
    let reason = if active {
        "Stop the script first"
    } else if snippet {
        "Stop the Console snippet first"
    } else {
        "Stop Auto Attack/Hunt first"
    };
    Rules {
        busy,
        start: !active && !auto && !snippet,
        load: !active,
        queue: !busy && !queue_running,
        reason: reason.into(),
        queue_tip: if busy {
            wait.clone()
        } else {
            "Cannot add scripts while the playlist is running".into()
        },
        wait,
    }
}

/// Why the buttons wait: one task runs at a time, and it can be cancelled.
/// What Update all would update. Libraries (CoreBots, ...) count although the list hides them:
/// an update that only changes CoreBots must still enable the button.
fn outdated_count(entries: &[Entry]) -> usize {
    entries.iter().filter(|e| e.status == Status::Outdated).count()
}

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
        Status::Removed => {
            "The repository no longer has this script, so this is the only copy. The file moves to \
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

/// Runs `work` for the script at `path`: a Load or Start of a ready script at once (it needs
/// no download, so it doesn't wait for a task), the rest on the repo's thread (one task at a
/// time).
fn run(path: &str, work: Work) -> Ran {
    let bot = crate::turu::bot();
    let repo = crate::turu::script_repo();
    if matches!(work, Work::Start | Work::Load)
        && let Some(local) = repo.ready(path)
    {
        bot.scripts.load(&local);
        bot.log(format!("Loaded script {}", local.display()));
        let result = if work == Work::Start {
            turu_bot::script::start(&bot)
                .map(|()| format!("Started {path}."))
                .map_err(|e| format!("Start failed: couldn't start the script: {e}"))
        } else {
            Ok(format!("Loaded {path}."))
        };
        let worked = result.is_ok();
        let (message, failed) = match result {
            Ok(message) => (message, false),
            Err(message) => (message, true),
        };
        repo.set_message(message, failed);
        return Ran::Done(worked);
    }
    let path = path.to_owned();
    let spawned = match work {
        // Downloads the script (and what it imports), then loads it in the active tab and, for
        // Start, starts it.
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
    };
    if spawned { Ran::Spawned } else { Ran::Refused }
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

/// A compact row's text: the name, then the badge and the description. Returns its height.
fn row_text_compact(ui: &mut Ui, entry: &Entry) -> f32 {
    let info = &entry.info;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.horizontal(|ui| {
            if entry.pinned {
                ui.label(RichText::new("★").color(WARNING)).on_hover_text("Pinned");
            }
            ui.add(egui::Label::new(RichText::new(&info.name).strong()).truncate());
        });
        ui.horizontal(|ui| {
            badges(ui, entry.status);
            let description = info.description.as_str();
            ui.add(egui::Label::new(RichText::new(description).color(MUTED)).truncate())
                .on_hover_text(description);
        });
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

/// VibeSkua's "Update" and "Local" badges, and Turu's Modified and Removed: label, colour and
/// what it means.
fn badge(status: Status) -> Option<(&'static str, egui::Color32, &'static str)> {
    Some(match status {
        Status::Outdated => ("Update", WARNING, "The repository has a newer version"),
        Status::Modified => ("Modified", WARNING, "Edited here: updates keep it"),
        Status::Local => ("Local", MUTED, "Your own script, not in the repository"),
        Status::Removed => ("Removed", MUTED, "No longer in the repository"),
        Status::UpToDate => ("Downloaded", SUCCESS, "Up to date"),
        Status::NotDownloaded => return None,
    })
}

fn badges(ui: &mut Ui, status: Status) {
    if let Some((label, color, meaning)) = badge(status) {
        widgets::pill(ui, |ui| ui.label(RichText::new(label).size(10.0).color(color))).on_hover_text(meaning);
    }
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
            info: std::sync::Arc::new(ScriptInfo {
                path: path.into(),
                name: name.into(),
                description: format!("farms {name}"),
                added: added.into(),
                ..Default::default()
            }),
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

    /// An outdated library alone (a CoreBots-only update) enables Update all.
    #[test]
    fn outdated_libraries_count_for_update_all() {
        let mut core = entry("core/CoreBots.ts", "CoreBots", "", Status::Outdated, false);
        Arc::make_mut(&mut core.info).library = true;
        let entries = vec![
            entry("farm/GoldFarm.ts", "Gold Farm", "", Status::UpToDate, false),
            entry("farm/Exp.ts", "Experience", "", Status::Modified, false),
            core,
        ];
        assert_eq!(outdated_count(&entries), 1);
        assert_eq!(outdated_count(&entries[..2]), 0);
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
        std::sync::Arc::make_mut(&mut e.info).tags = vec!["gold".into(), "farm".into()];
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

    /// A phone either way up gets the compact layout, a desktop window the wide one; a
    /// mid-sized one stacks the filters under the search box.
    #[test]
    fn the_header_follows_the_room() {
        assert_eq!(Header::pick(945.0, 450.0), Header::Wide);
        assert_eq!(Header::pick(720.0, 500.0), Header::Stacked);
        // A phone held upright, and sideways (wide but short).
        assert_eq!(Header::pick(380.0, 800.0), Header::Compact);
        assert_eq!(Header::pick(880.0, 300.0), Header::Compact);
    }

    /// A compact row's text fits the compact row height, so rows don't grow on the first frame.
    #[test]
    fn compact_row_text_fits_the_row() {
        let ctx = egui::Context::default();
        super::super::theme::install(&ctx);
        let e = entry("farm/GoldFarm.ts", "Gold Farm", "", Status::Outdated, true);
        for _ in 0..2 {
            let mut height = 0.0;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                height = ui.allocate_ui(vec2(300.0, 200.0), |ui| row_text_compact(ui, &e)).inner;
            });
            output.textures_delta.clear();
            assert!(
                height + 2.0 * ROW_MARGIN <= COMPACT_ROW_HEIGHT,
                "the compact row's text is {height} px: raise COMPACT_ROW_HEIGHT"
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
        assert!(delete_text(Status::Removed).contains("only copy"));
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
                    info: std::sync::Arc::new(ScriptInfo {
                        library: true,
                        ..(*entry("core/CoreBots.ts", "CoreBots", "", Status::UpToDate, true).info).clone()
                    }),
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
