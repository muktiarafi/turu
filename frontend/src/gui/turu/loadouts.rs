//! Tools & Helpers > Loadouts Manager (VibeSkua `LoadoutsView` / `LoadoutsViewModel`): the
//! character's saved gear sets (`turu_bot::loadouts`), what each holds, Equip, Save current gear,
//! rename and Delete.
//!
//! VibeSkua's two panes stay on a wide window; on a narrow one (a phone held upright) the list
//! and a loadout are two pages, the loadout's with a back arrow. Unlike VibeSkua:
//!
//! * "Show current gear" is the list's first row, "Currently equipped", always there and live
//!   (from the gear cache, `turu_bot::gear`), with "Save as loadout" as its action.
//! * Save current gear names the new loadout after its class and puts the cursor in its name,
//!   rather than asking first in a dialog; renaming is the name field, saved on Enter.
//! * Each slot says whether the item is worn, in the bag or in the bank, so a loadout that would
//!   come up short shows it before Equip; the list marks the loadout that is worn now.
//! * Equip doesn't enhance and is held while a script runs ("Stop script & equip" stops it, and
//!   it stays stopped). Enhancements that differ from what's worn are offered as their own
//!   action, "Apply" (VibeSkua's `LoadoutEnhancerTemp`, as a script).
//! * Outcomes show in the window rather than in message boxes; only Delete asks first.

use super::theme::{ACCENT, DANGER, EDGE, INSET, MUTED, SUCCESS, TEXT, TRACK, WARNING};
use super::widgets::{self, Kind};
use egui::text::{CCursor, CCursorRange, LayoutJob, TextWrapping};
use egui::{
    Align, Button, Color32, Context, CornerRadius, FontId, Frame, Id, Key, Layout, Margin, Modal, Response, RichText,
    ScrollArea, Sense, Spinner, Stroke, Ui, Vec2, vec2,
};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use turu_bot::gear::{self, GearError, GearFile, Place, Slot};
use turu_bot::loadouts::{self, Enhanced, Loadout, SLOTS};

/// Below this width the list and the loadout are two pages.
const NARROW: f32 = 520.0;
/// The list pane's width on a wide window.
const LIST_WIDTH: f32 = 220.0;
const GAP: f32 = 8.0;
/// How often the window takes the gear cache again.
const GEAR_EVERY: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Muted,
    Success,
    Warning,
    Danger,
}

impl Tone {
    fn color(self) -> Color32 {
        match self {
            Tone::Muted => MUTED,
            Tone::Success => SUCCESS,
            Tone::Warning => WARNING,
            Tone::Danger => DANGER,
        }
    }
}

/// What a bot thread is doing for the window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Busy {
    Reading,
    Equipping,
    Enhancing,
}

/// What the worn gear read on a bot thread is for.
enum WornFor {
    /// A new loadout.
    New,
    /// Replacing the items of the loadout of this name.
    Replace(String),
}

/// What the bot threads hand back.
#[derive(Default)]
struct Shared {
    busy: Option<Busy>,
    status: Option<(String, Tone)>,
    /// The gear read for Save current gear or Replace with worn.
    worn: Option<(WornFor, Loadout)>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// The list's pick.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick {
    Current,
    Saved(usize),
}

/// Where a loadout's item is, from the gear cache.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Have {
    Worn,
    Bag,
    Bank,
    /// Neither in the bag nor in the bank as last read.
    Missing,
    /// Not in the bag, and the bank wasn't read yet.
    Unknown,
}

fn have(gear: &GearFile, item: &str) -> Have {
    let name = item.replace("&amp;", "&");
    let name = name.trim();
    let matching = |i: &&gear::GearItem| i.name.replace("&amp;", "&").trim().eq_ignore_ascii_case(name);
    let mut found = gear.items.iter().filter(matching).peekable();
    if found.peek().is_none() {
        return if gear.bank_as_of.is_some() {
            Have::Missing
        } else {
            Have::Unknown
        };
    }
    let mut best = Have::Bank;
    for item in found {
        if item.equipped {
            return Have::Worn;
        }
        if item.place == Place::Inventory {
            best = Have::Bag;
        }
    }
    best
}

/// Every item of the loadout is worn.
fn worn_now(gear: &GearFile, loadout: &Loadout) -> bool {
    let items = loadout.items();
    !items.is_empty() && items.iter().all(|i| have(gear, i) == Have::Worn)
}

pub struct LoadoutsWindow {
    /// Whose list is loaded (the logged-in username).
    username: Option<String>,
    list: Vec<Loadout>,
    pick: Option<Pick>,
    /// Narrow window: the loadout's page is shown rather than the list.
    detail: bool,
    /// The name field's text for the picked loadout.
    name: String,
    /// Put the cursor in the name field (all of it selected) next frame.
    focus_name: bool,
    confirm_delete: Option<usize>,
    gear: Option<GearFile>,
    gear_at: Option<Instant>,
    /// The height under the slots last frame, so the slots take the rest of the pane.
    footer_height: Option<f32>,
    was_open: bool,
    shared: Arc<Mutex<Shared>>,
}

impl Default for LoadoutsWindow {
    fn default() -> Self {
        Self {
            username: None,
            list: Vec::new(),
            pick: Some(Pick::Current),
            detail: false,
            name: String::new(),
            focus_name: false,
            confirm_delete: None,
            gear: None,
            gear_at: None,
            footer_height: None,
            was_open: false,
            shared: Arc::default(),
        }
    }
}

impl LoadoutsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        if *open && !self.was_open {
            // Another tab of the same character may have changed the file; and read the gear
            // now rather than within the gear thread's 30 s.
            self.username = None;
            self.read_gear();
        }
        self.was_open = *open;
        if !*open {
            return;
        }
        let mut window = super::window(ctx, "Loadouts Manager", [600.0, 500.0], 5).open(open);
        // On a phone the loadout's name field would sit under the keyboard: it fills the screen.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 620.0 || area.height() < 520.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui));
        self.delete_dialog(ctx);
        if lock(&self.shared).busy.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(GEAR_EVERY);
        }
    }

    /// Follows the logged-in character: its file when it changed, the gear cache every second.
    fn sync(&mut self, bot: &turu_bot::Bot) {
        let username = bot
            .gear
            .logged_in()
            .then(|| bot.state.lock().username.clone())
            .flatten()
            .filter(|u| !u.trim().is_empty());
        if username != self.username {
            self.username = username;
            self.gear = None;
            self.gear_at = None;
            self.load(bot);
        }
        if self.gear_at.is_none_or(|t| t.elapsed() >= GEAR_EVERY) {
            self.gear = bot.gear.last();
            self.gear_at = Some(Instant::now());
        }
    }

    fn path(&self, bot: &turu_bot::Bot) -> Option<std::path::PathBuf> {
        let username = self.username.as_deref()?;
        Some(loadouts::path_for(&bot.paths.loadouts_dir(), username))
    }

    /// VibeSkua `LoadFromFile`. A file that can't be read is kept beside it as `.bad`.
    fn load(&mut self, bot: &turu_bot::Bot) {
        self.list.clear();
        self.pick = Some(Pick::Current);
        self.detail = false;
        self.confirm_delete = None;
        let Some(path) = self.path(bot) else { return };
        match loadouts::load(&path) {
            Ok(list) => self.list = list,
            Err(e) => {
                let bad = path.with_extension("json.bad");
                let _ = std::fs::rename(&path, &bad);
                self.set_status(
                    format!("Couldn't read {} ({e}): kept as {}", path.display(), bad.display()),
                    Tone::Danger,
                );
            }
        }
    }

    /// VibeSkua `SaveToFile`, after every change.
    fn save(&self, bot: &turu_bot::Bot) {
        let Some(path) = self.path(bot) else { return };
        if let Err(e) = loadouts::save(&path, &self.list) {
            self.set_status(format!("Couldn't save {}: {e}", path.display()), Tone::Danger);
        }
    }

    fn set_status(&self, text: String, tone: Tone) {
        lock(&self.shared).status = Some((text, tone));
    }

    fn select(&mut self, pick: Pick) {
        self.pick = Some(pick);
        self.name = match pick {
            Pick::Saved(i) => self.list.get(i).map(|l| l.name.clone()).unwrap_or_default(),
            Pick::Current => String::new(),
        };
        self.detail = true;
    }

    /// Takes what a bot thread read: a new loadout, or new items for one.
    fn take_worn(&mut self, bot: &turu_bot::Bot) {
        let Some((target, worn)) = lock(&self.shared).worn.take() else {
            return;
        };
        match target {
            WornFor::New => {
                let base = if worn.class.trim().is_empty() {
                    "Loadout".to_owned()
                } else {
                    worn.class.clone()
                };
                let name = self.unique_name(&base);
                self.list.push(worn.renamed(&name));
                self.save(bot);
                self.select(Pick::Saved(self.list.len() - 1));
                self.focus_name = true;
                self.set_status(
                    format!("Saved as \u{201C}{name}\u{201D}: type to rename it"),
                    Tone::Success,
                );
            }
            WornFor::Replace(name) => {
                if let Some(i) = self.list.iter().position(|l| l.name == name) {
                    self.list[i] = worn.renamed(&name);
                    self.save(bot);
                    self.set_status(format!("{name} now holds what you wear"), Tone::Success);
                }
            }
        }
    }

    /// `base`, or `base 2`, `base 3`, ... when a loadout has that name.
    fn unique_name(&self, base: &str) -> String {
        let taken = |n: &str| self.list.iter().any(|l| l.name.eq_ignore_ascii_case(n)) || n == loadouts::CURRENT;
        if !taken(base) {
            return base.to_owned();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|n| !taken(n))
            .expect("a free name")
    }

    fn ui(&mut self, ui: &mut Ui) {
        let bot = crate::turu::bot();
        self.sync(&bot);
        self.take_worn(&bot);
        if let Some(Pick::Saved(i)) = self.pick
            && i >= self.list.len()
        {
            self.pick = Some(Pick::Current);
        }
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        let height = ui.available_height();
        if ui.available_width() < NARROW {
            if self.detail && self.pick.is_some() {
                self.detail_pane(ui, &bot, true);
            } else {
                self.detail = false;
                self.list_pane(ui, height);
            }
            return;
        }
        ui.horizontal_top(|ui| {
            let list_width = LIST_WIDTH.min(ui.available_width() * 0.42);
            ui.allocate_ui_with_layout(vec2(list_width, height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(list_width);
                self.list_pane(ui, height);
            });
            ui.allocate_ui_with_layout(vec2(ui.available_width(), height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(ui.available_width());
                self.detail_pane(ui, &bot, false);
            });
        });
    }

    /// VibeSkua's left pane: the saved loadouts and Save current gear.
    fn list_pane(&mut self, ui: &mut Ui, height: f32) {
        let busy = lock(&self.shared).busy;
        let logged_in = self.username.is_some();
        let control = widgets::target(30.0, 48.0);
        ui.horizontal(|ui| {
            ui.set_min_height(control);
            widgets::section_label(ui, "SAVED LOADOUTS");
            widgets::count_badge(ui, self.list.len());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let size = Vec2::splat(widgets::target(24.0, 40.0));
                let refresh = ui
                    .add_enabled(
                        logged_in && busy.is_none(),
                        Button::new(RichText::new("\u{27F3}").size(15.0))
                            .min_size(size)
                            .fill(TRACK),
                    )
                    .on_hover_text("Read the file and the worn gear again");
                if refresh.clicked() {
                    self.username = None;
                    self.read_gear();
                }
            });
        });

        let row = widgets::target(42.0, 56.0);
        let well = (height - ui.min_rect().height() - control - GAP * 2.0).max(row * 2.0 + 10.0);
        let mut picked = None;
        widgets::list_well(ui, well, |ui| {
            if !logged_in {
                empty_text(ui, "Log in to see this character's loadouts.");
                return;
            }
            ScrollArea::vertical()
                .id_salt("turu_loadouts_list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let class = self
                        .gear
                        .as_ref()
                        .and_then(|g| g.worn(Slot::Class))
                        .map(|i| i.name.clone());
                    let current = self.pick == Some(Pick::Current);
                    let reading = self.gear.is_none();
                    let subtitle = class.unwrap_or_else(|| {
                        if reading {
                            "Reading...".into()
                        } else {
                            "\u{2014}".into()
                        }
                    });
                    if self
                        .row(ui, "Currently equipped", &subtitle, current, None, row)
                        .clicked()
                    {
                        picked = Some(Pick::Current);
                    }
                    let divider = ui
                        .allocate_exact_size(vec2(ui.available_width(), 5.0), Sense::hover())
                        .0;
                    ui.painter().hline(
                        divider.x_range().shrink(6.0),
                        divider.center().y,
                        Stroke::new(1.0, EDGE),
                    );
                    if self.list.is_empty() {
                        ui.add_space(6.0);
                        empty_text(
                            ui,
                            "No loadouts yet. Save current gear keeps what you wear now, under a name.",
                        );
                    }
                    for i in 0..self.list.len() {
                        let loadout = &self.list[i];
                        let worn = self.gear.as_ref().is_some_and(|g| worn_now(g, loadout));
                        let class = if loadout.class.trim().is_empty() {
                            "No class"
                        } else {
                            loadout.class.as_str()
                        };
                        let mark = worn.then_some(("Worn", SUCCESS));
                        let selected = self.pick == Some(Pick::Saved(i));
                        if self.row(ui, &loadout.name, class, selected, mark, row).clicked() {
                            picked = Some(Pick::Saved(i));
                        }
                    }
                });
        });
        if let Some(pick) = picked {
            self.select(pick);
        }

        let reading = busy == Some(Busy::Reading);
        let label = if reading {
            "Reading gear..."
        } else {
            "+  Save current gear"
        };
        let tip = if logged_in {
            "Wait for the last action"
        } else {
            "Log in first"
        };
        let save = widgets::button(
            ui,
            label,
            Kind::Normal,
            logged_in && busy.is_none(),
            vec2(ui.available_width(), control),
            tip,
        )
        .on_hover_text("Save what this character wears now as a new loadout");
        if save.clicked() {
            self.read_worn(WornFor::New);
        }
    }

    /// A list row: the name and, under it, `subtitle`; `mark` on the right.
    fn row(
        &self,
        ui: &mut Ui,
        name: &str,
        subtitle: &str,
        selected: bool,
        mark: Option<(&str, Color32)>,
        height: f32,
    ) -> Response {
        let response = widgets::list_row(ui, selected, height, |ui| {
            let small = FontId::proportional(widgets::small_label_size());
            let rect = ui.max_rect();
            let mark = mark.map(|(text, color)| ui.painter().layout_no_wrap(text.to_owned(), small.clone(), color));
            let mark_width = mark.as_ref().map_or(0.0, |m| m.size().x + 8.0);
            let width = (rect.width() - mark_width).max(20.0);
            let line = |text: &str, font: FontId, color: Color32| {
                let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
                job.wrap = TextWrapping::truncate_at_width(width);
                ui.painter().layout_job(job)
            };
            let name = line(name, FontId::proportional(13.0), TEXT);
            let subtitle = line(subtitle, small.clone(), MUTED);
            let top = rect.center().y - (name.size().y + 2.0 + subtitle.size().y) / 2.0;
            let sub_top = top + name.size().y + 2.0;
            ui.painter().galley(egui::pos2(rect.left(), top), name, TEXT);
            ui.painter().galley(egui::pos2(rect.left(), sub_top), subtitle, MUTED);
            if let Some(mark) = mark {
                let at = egui::pos2(rect.right() - mark.size().x, rect.center().y - mark.size().y / 2.0);
                ui.painter().galley(at, mark, TEXT);
            }
        });
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        response
    }

    /// VibeSkua's right pane: the picked loadout's name, slots and actions. `page`: it's the
    /// narrow window's second page, with a back arrow.
    fn detail_pane(&mut self, ui: &mut Ui, bot: &Arc<turu_bot::Bot>, page: bool) {
        let (busy, status) = {
            let s = lock(&self.shared);
            (s.busy, s.status.clone())
        };
        let logged_in = self.username.is_some();
        let control = widgets::target(30.0, 48.0);
        let pick = self.pick.unwrap_or(Pick::Current);
        let saved = match pick {
            Pick::Saved(i) => self.list.get(i).cloned(),
            Pick::Current => None,
        };
        let current = self
            .gear
            .as_ref()
            .map(|g| Loadout::from_worn(loadouts::CURRENT, &g.items))
            .unwrap_or_default();
        let shown = saved.as_ref().unwrap_or(&current);

        // Heading. On the narrow window's page: back, then the name under it. Wide: the name
        // beside Replace with worn, as VibeSkua's LOADOUT row and name box in one.
        let header = widgets::target(24.0, 40.0);
        let replace_button = |this: &mut Self, ui: &mut Ui, height: f32| {
            let replace = widgets::button(
                ui,
                "Replace with worn",
                Kind::Normal,
                logged_in && busy.is_none(),
                vec2(0.0, height),
                "Wait for the last action",
            )
            .on_hover_text("Put what you wear now in this loadout instead");
            if replace.clicked() {
                this.read_worn(WornFor::Replace(shown.name.clone()));
            }
        };
        if page {
            ui.horizontal(|ui| {
                ui.set_min_height(header);
                let size = vec2(widgets::target(28.0, 44.0), header);
                let back = ui
                    .add(
                        Button::new(RichText::new("\u{2039}").size(20.0).color(TEXT))
                            .fill(TRACK)
                            .min_size(size),
                    )
                    .on_hover_text("Back to the loadouts");
                if back.clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.detail = false;
                }
                widgets::section_label(ui, "LOADOUTS");
                if saved.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        replace_button(self, ui, header)
                    });
                }
            });
            self.title(ui, bot, pick, control);
        } else {
            ui.horizontal(|ui| {
                ui.set_min_height(control);
                if saved.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        replace_button(self, ui, control);
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            self.title(ui, bot, pick, control);
                        });
                    });
                } else {
                    self.title(ui, bot, pick, control);
                }
            });
        }

        // The slots take what the actions under them leave.
        let footer = self.footer_height.unwrap_or(control * 2.0 + GAP * 2.0);
        let slots_height = (ui.available_height() - footer - GAP).max(widgets::target(34.0, 44.0) * 2.0 + 8.0);
        let changes = match (&saved, &self.gear) {
            (Some(loadout), Some(gear)) => loadouts::enhancement_changes(loadout, &gear.items),
            _ => Vec::new(),
        };
        self.slots(ui, shown, saved.is_some(), &changes, slots_height);
        let spare = ui.available_height() - footer;
        if spare > 0.0 {
            ui.add_space(spare);
        }

        let top = ui.cursor().top();
        let script = bot.scripts.is_running().then(|| script_name(bot));
        if saved.is_some() && !changes.is_empty() {
            let text = if changes.len() == 1 {
                "1 enhancement differs from what's worn: Enhance applies it".to_owned()
            } else {
                format!(
                    "{} enhancements differ from what's worn: Enhance applies them",
                    changes.len()
                )
            };
            notice(ui, WARNING, &text);
        }
        if saved.is_some()
            && let Some(name) = &script
            && busy != Some(Busy::Enhancing)
        {
            notice(
                ui,
                WARNING,
                &format!("{name} is running. Equipping stops it first, and it stays stopped."),
            );
        }
        if let Some((text, tone)) = &status {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .size(widgets::small_label_size())
                        .color(tone.color()),
                )
                .wrap(),
            );
        }
        self.actions(
            ui,
            bot,
            saved.as_ref(),
            !changes.is_empty(),
            script.is_some(),
            busy,
            control,
        );
        let height = ui.cursor().top() - top;
        if self.footer_height.is_none_or(|h| (h - height).abs() > 0.5) {
            self.footer_height = Some(height);
            ui.ctx().request_repaint();
        }
    }

    /// The name field of a saved loadout (renames on Enter or when it loses focus, VibeSkua's
    /// binding), or the live entry's title; as wide as what's left of the row.
    fn title(&mut self, ui: &mut Ui, bot: &Arc<turu_bot::Bot>, pick: Pick, control: f32) {
        if let Pick::Saved(index) = pick {
            self.name_field(ui, bot, index, control);
            return;
        }
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), control),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_min_height(control);
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.add_space(((control - 34.0) / 2.0).max(0.0));
                ui.label(RichText::new("Currently equipped").size(15.0).strong().color(TEXT));
                ui.label(
                    RichText::new("What this character wears now")
                        .size(widgets::small_label_size())
                        .color(MUTED),
                );
            },
        );
    }

    fn name_field(&mut self, ui: &mut Ui, bot: &Arc<turu_bot::Bot>, index: usize, control: f32) {
        let id = Id::new("turu_loadouts_name");
        let response = ui.add_sized(
            vec2(ui.available_width(), control),
            egui::TextEdit::singleline(&mut self.name)
                .id(id)
                .hint_text("Loadout name")
                .font(FontId::proportional(15.0))
                .vertical_align(Align::Center)
                .margin(Margin::symmetric(10, 4))
                .background_color(INSET),
        );
        if std::mem::take(&mut self.focus_name) {
            response.request_focus();
            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                let end = CCursor::new(self.name.chars().count());
                state
                    .cursor
                    .set_char_range(Some(CCursorRange::two(CCursor::new(0), end)));
                state.store(ui.ctx(), id);
            }
        }
        if response.lost_focus() {
            self.rename(bot, index);
        }
    }

    /// VibeSkua `SelectedLoadoutName`'s setter, without its question: a rename is undone by
    /// renaming back.
    fn rename(&mut self, bot: &turu_bot::Bot, index: usize) {
        let Some(old) = self.list.get(index).map(|l| l.name.clone()) else {
            return;
        };
        let new = self.name.trim().to_owned();
        if new == old {
            self.name = old;
            return;
        }
        let clash = self
            .list
            .iter()
            .enumerate()
            .any(|(i, l)| i != index && l.name.eq_ignore_ascii_case(&new));
        let problem = if new.is_empty() {
            Some("A loadout needs a name".to_owned())
        } else if new.eq_ignore_ascii_case(loadouts::CURRENT) {
            Some(format!(
                "\u{201C}{}\u{201D} is the live entry's name",
                loadouts::CURRENT
            ))
        } else if clash {
            Some(format!("Another loadout is called \u{201C}{new}\u{201D}"))
        } else {
            None
        };
        if let Some(problem) = problem {
            self.set_status(format!("{problem}: kept \u{201C}{old}\u{201D}"), Tone::Warning);
            self.name = old;
            return;
        }
        self.list[index].name = new.clone();
        self.name = new.clone();
        self.save(bot);
        self.set_status(format!("Renamed to \u{201C}{new}\u{201D}"), Tone::Muted);
    }

    /// The seven slots: name, item, enhancement and, for a saved loadout, where the item is.
    fn slots(&self, ui: &mut Ui, loadout: &Loadout, saved: bool, changes: &[loadouts::EnhancementChange], height: f32) {
        let row = widgets::target(34.0, 44.0);
        let gear = self.gear.as_ref();
        Frame::new()
            .fill(INSET)
            .stroke(Stroke::new(1.0, EDGE))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(10, 2))
            .show(ui, |ui| {
                let inner = (height - 6.0).max(row);
                ScrollArea::vertical()
                    .id_salt("turu_loadouts_slots")
                    .max_height(inner)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (n, &slot) in SLOTS.iter().enumerate() {
                            if n > 0 {
                                let line = ui
                                    .allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover())
                                    .0;
                                ui.painter()
                                    .hline(line.x_range(), line.center().y, Stroke::new(1.0, EDGE));
                            }
                            let item = loadout.item(slot).trim();
                            let change = changes.iter().find(|c| c.slot == slot);
                            let place = (saved && !item.is_empty())
                                .then(|| gear.map(|g| have(g, item)))
                                .flatten();
                            let bank_as_of = gear.and_then(|g| g.bank_as_of);
                            slot_row(ui, slot, item, loadout, change, place, bank_as_of, row);
                        }
                    });
            });
    }

    /// Delete and Equip for a saved loadout; Save as loadout for the live entry.
    #[allow(clippy::too_many_arguments)]
    fn actions(
        &mut self,
        ui: &mut Ui,
        bot: &Arc<turu_bot::Bot>,
        saved: Option<&Loadout>,
        differs: bool,
        script_runs: bool,
        busy: Option<Busy>,
        control: f32,
    ) {
        let logged_in = self.username.is_some();
        let idle = logged_in && busy.is_none();
        let tip = if logged_in {
            "Wait for the last action"
        } else {
            "Log in first"
        };
        ui.horizontal(|ui| {
            ui.set_min_height(control);
            let Some(loadout) = saved else {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let save = widgets::button(
                        ui,
                        "Save as loadout",
                        Kind::Primary,
                        idle && self.gear.is_some(),
                        vec2(140.0, control),
                        tip,
                    );
                    if save.clicked() {
                        self.read_worn(WornFor::New);
                    }
                });
                return;
            };
            let delete = ui
                .add_enabled(
                    busy.is_none(),
                    Button::new(RichText::new("Delete").color(DANGER))
                        .fill(TRACK)
                        .min_size(vec2(84.0, control)),
                )
                .on_hover_text("Delete this loadout (its items stay where they are)");
            if delete.clicked()
                && let Some(Pick::Saved(i)) = self.pick
            {
                self.confirm_delete = Some(i);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let equipping = busy == Some(Busy::Equipping);
                let label = match (equipping, script_runs) {
                    (true, _) => "Equipping...",
                    (false, true) => "Stop script & equip",
                    (false, false) => "Equip",
                };
                let width = if script_runs || equipping { 160.0 } else { 112.0 };
                let has_items = !loadout.items().is_empty();
                let tip = if has_items { tip } else { "This loadout holds no items" };
                let equip = widgets::button(ui, label, Kind::Primary, idle && has_items, vec2(width, control), tip);
                if equipping {
                    ui.add(Spinner::new().size(14.0).color(ACCENT));
                }
                if equip.clicked() {
                    self.equip(bot, loadout.clone(), script_runs);
                }
                if differs {
                    let enhancing = busy == Some(Busy::Enhancing);
                    let tip = if script_runs { "Stop the script first" } else { tip };
                    let label = if enhancing { "Enhancing..." } else { "Enhance" };
                    let width = if enhancing { 112.0 } else { 84.0 };
                    let enhance =
                        widgets::button(ui, label, Kind::Normal, idle && !script_runs, vec2(width, control), tip)
                            .on_hover_text(
                                "Enhance the worn items as this loadout saved them, with CoreAdvanced: it may \
                                 travel to the enhancement shops and spend gold, then comes back here",
                            );
                    if enhancing {
                        ui.add(Spinner::new().size(14.0).color(ACCENT));
                    }
                    if enhance.clicked() {
                        self.enhance(bot, loadout.clone());
                    }
                }
            });
        });
    }

    /// Reads the worn gear on a bot thread for Save current gear or Replace with worn.
    fn read_worn(&self, target: WornFor) {
        {
            let mut s = lock(&self.shared);
            if s.busy.is_some() {
                return;
            }
            s.busy = Some(Busy::Reading);
        }
        let shared = self.shared.clone();
        crate::turu::spawn_action("Loadouts save", move |bot| {
            let file = gear::refresh(bot);
            let mut s = lock(&shared);
            s.busy = None;
            match file {
                Ok(Some(file)) => s.worn = Some((target, Loadout::from_worn("", &file.items))),
                Ok(None) => s.status = Some(("Not in a map: log in first".to_owned(), Tone::Danger)),
                Err(e) => s.status = Some((format!("Couldn't read your gear: {e}"), Tone::Danger)),
            }
            Ok(())
        });
    }

    /// Reads the gear now, so the window doesn't wait for the gear thread.
    fn read_gear(&mut self) {
        self.gear_at = None;
        crate::turu::spawn_action("Loadouts gear", |bot| {
            gear::refresh(bot)?;
            Ok(())
        });
    }

    /// VibeSkua `EquipLoadoutAsync` (without enhancing), on a bot thread, saying how it went.
    fn equip(&self, bot: &Arc<turu_bot::Bot>, loadout: Loadout, stop: bool) {
        {
            let mut s = lock(&self.shared);
            if s.busy.is_some() {
                return;
            }
            s.busy = Some(Busy::Equipping);
            s.status = Some((format!("Equipping {}...", loadout.name), Tone::Muted));
        }
        let shared = self.shared.clone();
        crate::turu::spawn_action_for(bot.clone(), "Loadouts equip", move |bot| {
            let outcome = loadouts::equip(bot, &loadout, stop);
            let _ = gear::refresh(bot);
            let mut s = lock(&shared);
            s.busy = None;
            s.status = Some(match outcome {
                Ok(report) => equip_outcome(&loadout.name, &report),
                Err(GearError::ScriptRunning(name)) => (
                    format!("{name} started meanwhile: press Equip again to stop it"),
                    Tone::Warning,
                ),
                Err(e) => (format!("Couldn't equip {}: {e}", loadout.name), Tone::Danger),
            });
            Ok(())
        });
    }

    /// VibeSkua's `LoadoutEnhancerTemp`, on a bot thread; it runs as a script until it's done.
    fn enhance(&self, bot: &Arc<turu_bot::Bot>, loadout: Loadout) {
        {
            let mut s = lock(&self.shared);
            if s.busy.is_some() {
                return;
            }
            s.busy = Some(Busy::Enhancing);
            s.status = Some((
                format!("Enhancing {}: Stop in the Script Loader stops it", loadout.name),
                Tone::Muted,
            ));
        }
        let shared = self.shared.clone();
        crate::turu::spawn_action_for(bot.clone(), "Loadouts enhance", move |bot| {
            let outcome = loadouts::apply_enhancements(bot, &loadout);
            let _ = gear::refresh(bot);
            let mut s = lock(&shared);
            s.busy = None;
            s.status = Some(match outcome {
                Ok(Enhanced::AlreadyDone) => ("Already enhanced as saved".to_owned(), Tone::Success),
                Ok(Enhanced::Ran { items, finished: true }) => (
                    format!(
                        "Enhanced {items} item{} of {}",
                        if items == 1 { "" } else { "s" },
                        loadout.name
                    ),
                    Tone::Success,
                ),
                Ok(Enhanced::Ran { finished: false, .. }) => ("Enhancing was stopped".to_owned(), Tone::Warning),
                Err(e) => (format!("Couldn't enhance: {e}"), Tone::Danger),
            });
            Ok(())
        });
    }

    /// VibeSkua `DeleteLoadout`'s question.
    fn delete_dialog(&mut self, ctx: &Context) {
        let Some(index) = self.confirm_delete else {
            return;
        };
        let Some(name) = self.list.get(index).map(|l| l.name.clone()) else {
            self.confirm_delete = None;
            return;
        };
        let mut answer = None;
        let response = Modal::new(Id::new("turu_loadouts_delete")).show(ctx, |ui| {
            ui.set_min_width(260.0);
            ui.set_max_width(380.0);
            ui.heading("Delete loadout");
            ui.label(format!(
                "Delete \u{201C}{name}\u{201D}? Its items stay in your inventory and bank."
            ));
            ui.add_space(4.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let size = vec2(84.0, widgets::target(28.0, 48.0));
                if widgets::button(ui, "Delete", Kind::Danger, true, size, "").clicked() {
                    answer = Some(true);
                }
                if widgets::button(ui, "Cancel", Kind::Normal, true, size, "").clicked() {
                    answer = Some(false);
                }
            });
        });
        if answer.is_none() && !response.should_close() {
            return;
        }
        self.confirm_delete = None;
        if answer == Some(true) && index < self.list.len() {
            self.list.remove(index);
            self.save(&crate::turu::bot());
            self.pick = Some(Pick::Current);
            self.detail = false;
            self.set_status(format!("Deleted \u{201C}{name}\u{201D}"), Tone::Muted);
        }
    }
}

/// One slot: its name, the item (or a dash), the enhancement chip, and where the item is.
#[allow(clippy::too_many_arguments)]
fn slot_row(
    ui: &mut Ui,
    slot: Slot,
    item: &str,
    loadout: &Loadout,
    change: Option<&loadouts::EnhancementChange>,
    place: Option<Have>,
    bank_as_of: Option<u64>,
    height: f32,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    let ui = &mut child;
    ui.spacing_mut().item_spacing.x = 6.0;
    let small = FontId::proportional(widgets::small_label_size());
    let label_width = if widgets::touch() { 60.0 } else { 56.0 };
    ui.allocate_ui_with_layout(vec2(label_width, height), Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_size(vec2(label_width, height));
        ui.label(
            RichText::new(loadouts::slot_label(slot))
                .font(small.clone())
                .color(MUTED),
        );
    });

    // Right to left: where it is, then the enhancement; the item takes the rest.
    let mut right: Vec<(String, Color32, Color32, String)> = Vec::new();
    if let Some(place) = place {
        let when = bank_as_of.map_or_else(|| "the bank wasn't read yet".to_owned(), ago);
        let (text, color, tip) = match place {
            Have::Worn => ("Worn", SUCCESS, "Worn now".to_owned()),
            Have::Bag => ("In bag", MUTED, "In the inventory: Equip puts it on".to_owned()),
            Have::Bank => (
                "In bank",
                MUTED,
                format!("In the bank as of {when}: Equip takes it out"),
            ),
            Have::Missing => (
                "Not found",
                WARNING,
                format!("Neither in the inventory nor in the bank ({when})"),
            ),
            Have::Unknown => (
                "Not in bag",
                WARNING,
                "Not in the inventory; the bank wasn't read yet (Equip looks there)".to_owned(),
            ),
        };
        right.push((text.to_owned(), color, Color32::TRANSPARENT, tip));
    }
    let enhancement = loadout.enhancement_name(slot).trim();
    let shown_enhancement = if loadouts::enhanced(slot) && !item.is_empty() {
        if enhancement.is_empty() || enhancement == "Unknown" {
            None
        } else {
            Some(enhancement.to_owned())
        }
    } else {
        None
    };
    if let Some(name) = shown_enhancement {
        match change {
            Some(change) => right.push((
                name,
                WARNING,
                WARNING.gamma_multiply(0.14),
                format!("Worn with {}: Apply enhances it as saved", change.worn.name()),
            )),
            None => right.push((name, TEXT, TRACK, "The enhancement this loadout keeps".to_owned())),
        }
    }

    let widths: Vec<f32> = right
        .iter()
        .map(|(text, ..)| ui.painter().layout_no_wrap(text.clone(), small.clone(), TEXT).size().x + 14.0)
        .collect();
    let right_width: f32 = widths.iter().sum::<f32>() + 6.0 * right.len() as f32;
    let item_width = (ui.available_width() - right_width).max(30.0);
    if item.is_empty() {
        ui.allocate_ui_with_layout(vec2(item_width, height), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_size(vec2(item_width, height));
            ui.label(RichText::new("\u{2014}").color(MUTED));
        });
    } else {
        ui.allocate_ui_with_layout(vec2(item_width, height), Layout::left_to_right(Align::Center), |ui| {
            ui.add(single_line(item, FontId::proportional(13.0), TEXT, item_width, true))
                .on_hover_text(item);
        });
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for ((text, color, fill, tip), width) in right.iter().zip(widths) {
            let chip = vec2(width, widgets::small_label_size() + 9.0);
            let (rect, response) = ui.allocate_exact_size(chip, Sense::hover());
            if *fill != Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, CornerRadius::same(9), *fill);
            }
            ui.painter()
                .text(rect.center(), egui::Align2::CENTER_CENTER, text, small.clone(), *color);
            response.on_hover_text(tip);
        }
    });
}

/// The report as one line: "Equipped Farm", or what came up short.
fn equip_outcome(name: &str, report: &gear::EquipReport) -> (String, Tone) {
    let stopped = report
        .stopped_script
        .as_ref()
        .map(|s| format!(" (stopped {s})"))
        .unwrap_or_default();
    if report.missing.is_empty() && report.failed.is_empty() {
        let text = if report.equipped.is_empty() {
            format!("{name} was already worn{stopped}")
        } else {
            format!("Equipped {name}{stopped}")
        };
        return (text, Tone::Success);
    }
    let mut short: Vec<String> = report.missing.iter().map(|m| format!("{m} (not owned)")).collect();
    short.extend(report.failed.iter().map(|f| format!("{} ({})", f.item, f.reason)));
    (
        format!("{name} partly equipped{stopped}. Not put on: {}", short.join(", ")),
        Tone::Warning,
    )
}

/// "5 min ago", for the bank's date.
fn ago(ms: u64) -> String {
    let seconds = gear::now_ms().saturating_sub(ms) / 1000;
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86_400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} days ago", seconds / 86_400),
    }
}

/// The running script's file name.
fn script_name(bot: &turu_bot::Bot) -> String {
    bot.scripts
        .loaded()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "A script".into())
}

/// A coloured dot and a wrapped line.
fn notice(ui: &mut Ui, color: Color32, text: &str) {
    ui.horizontal_top(|ui| {
        ui.add_space(1.0);
        ui.vertical(|ui| {
            ui.add_space(5.0);
            widgets::dot(ui, color, 3.0);
        });
        ui.add(egui::Label::new(RichText::new(text).size(widgets::small_label_size()).color(TEXT)).wrap());
    });
}

/// Centered muted text in a list.
fn empty_text(ui: &mut Ui, text: &str) {
    ui.add_space(14.0);
    ui.vertical_centered(|ui| {
        ui.add(egui::Label::new(RichText::new(text).color(MUTED)).wrap());
    });
}

/// One line cut to `width` with an ellipsis; `hover`: it can show a tooltip.
fn single_line(text: &str, font: FontId, color: Color32, width: f32, hover: bool) -> egui::Label {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(width.max(10.0));
    let label = egui::Label::new(job).selectable(false);
    if hover { label.sense(Sense::hover()) } else { label }
}

#[cfg(test)]
mod tests {
    use super::*;
    use turu_bot::gear::GearItem;

    fn item(name: &str, slot: Slot, equipped: bool, place: Place) -> GearItem {
        GearItem {
            id: 1,
            name: name.into(),
            category: String::new(),
            slot,
            equipped,
            place,
            enhancement_level: 0,
            enhancement_pattern_id: 9,
            proc_id: 0,
        }
    }

    #[test]
    fn says_where_each_item_is() {
        let mut gear = GearFile {
            items: vec![
                item("Legion Revenant", Slot::Class, true, Place::Inventory),
                item("Blade & Shield", Slot::Weapon, false, Place::Inventory),
                item("Cape", Slot::Cape, false, Place::Bank),
            ],
            ..Default::default()
        };
        assert!(have(&gear, "legion revenant") == Have::Worn);
        assert!(have(&gear, "Blade &amp; Shield") == Have::Bag);
        assert!(have(&gear, "Cape") == Have::Bank);
        assert!(have(&gear, "Pet") == Have::Unknown);
        gear.bank_as_of = Some(1);
        assert!(have(&gear, "Pet") == Have::Missing);

        let loadout = Loadout {
            class: "Legion Revenant".into(),
            ..Default::default()
        };
        assert!(worn_now(&gear, &loadout));
        assert!(!worn_now(&gear, &Loadout::default()));
    }

    #[test]
    fn names_new_loadouts_apart() {
        let mut window = LoadoutsWindow::default();
        window.list.push(Loadout {
            name: "Legion Revenant".into(),
            ..Default::default()
        });
        assert_eq!(window.unique_name("Legion Revenant"), "Legion Revenant 2");
        assert_eq!(window.unique_name("Farm"), "Farm");
    }
}
