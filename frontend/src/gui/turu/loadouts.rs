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
use crate::models::loadouts::{self as model, ItemPlace as Have, State, WornFor, ago, have, worn_now};
use crate::models::scheduler::{Note, Tint};
use crate::models::tab_state::lock;
use egui::text::{CCursor, CCursorRange, LayoutJob, TextWrapping};
use egui::{
    Align, Button, Color32, Context, CornerRadius, FontId, Frame, Id, Key, Layout, Margin, Modal, Response, RichText,
    ScrollArea, Sense, Spinner, Stroke, Ui, Vec2, vec2,
};
use std::sync::Arc;
use std::time::Duration;
use turu_bot::gear::{GearFile, Slot};
use turu_bot::loadouts::{self, Loadout, SLOTS};

/// Below this width the list and the loadout are two pages.
const NARROW: f32 = 520.0;
/// The list pane's width on a wide window.
const LIST_WIDTH: f32 = 220.0;
const GAP: f32 = 8.0;
/// How often the window draws again to take the gear cache.
const GEAR_EVERY: Duration = Duration::from_secs(1);

use model::LoadoutWork as Busy;

fn tint_color(tint: Tint) -> Color32 {
    match tint {
        Tint::Plain => TEXT,
        Tint::Muted => MUTED,
        Tint::Success => SUCCESS,
        Tint::Warning => WARNING,
        Tint::Danger => DANGER,
    }
}

/// The list's pick.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pick {
    Current,
    Saved(usize),
}

/// The window's own state; the list, the gear and what a bot thread does are the tab's
/// [`State`] (`models::loadouts`).
pub struct LoadoutsWindow {
    pick: Option<Pick>,
    /// Narrow window: the loadout's page is shown rather than the list.
    detail: bool,
    /// The name field's text for the picked loadout.
    name: String,
    /// Put the cursor in the name field (all of it selected) next frame.
    focus_name: bool,
    confirm_delete: Option<usize>,
    /// The height under the slots last frame, so the slots take the rest of the pane.
    footer_height: Option<f32>,
    was_open: bool,
    /// `State::created_count` last seen: a loadout made since is picked and its name selected.
    created_seen: u32,
    /// Whose list the pick is in.
    username: Option<String>,
}

impl Default for LoadoutsWindow {
    fn default() -> Self {
        Self {
            pick: Some(Pick::Current),
            detail: false,
            name: String::new(),
            focus_name: false,
            confirm_delete: None,
            footer_height: None,
            was_open: false,
            created_seen: 0,
            username: None,
        }
    }
}

impl LoadoutsWindow {
    pub fn show(&mut self, ctx: &Context, open: &mut bool) {
        let bot = crate::turu::bot();
        let opened = *open && !self.was_open;
        if opened {
            // Another tab of the same character (or the native page) may have changed the file;
            // and read the gear now rather than within the gear thread's 30 s.
            model::read_gear(&bot);
        }
        self.was_open = *open;
        if !*open {
            return;
        }
        let state = model::STATE.of(&bot);
        let mut st = lock(&state);
        st.sync(&bot);
        if opened {
            // As reading the file did before the model: the pick and the question start over,
            // and a loadout made while the window was closed isn't jumped to.
            self.reset();
            self.created_seen = st.created_count;
        }
        if st.username != self.username {
            self.username = st.username.clone();
            self.pick = Some(Pick::Current);
            self.detail = false;
            self.confirm_delete = None;
        }
        if st.created_count != self.created_seen {
            self.created_seen = st.created_count;
            if let Some(i) = st.created.as_deref().and_then(|n| st.index_of(n)) {
                self.select(&st, Pick::Saved(i));
                self.focus_name = true;
            }
        }
        if let Some(Pick::Saved(i)) = self.pick
            && i >= st.list.len()
        {
            self.pick = Some(Pick::Current);
        }
        let mut window = super::window(ctx, "Loadouts Manager", [600.0, 500.0], 5).open(open);
        // On a phone the loadout's name field would sit under the keyboard: it fills the screen.
        let area = super::window_area(ctx);
        if widgets::touch() && (area.width() < 620.0 || area.height() < 520.0) {
            window = window.sheet(ctx);
        }
        window.show(ctx, |ui| self.ui(ui, &mut st, &bot));
        self.delete_dialog(ctx, &mut st);
        if st.busy.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else {
            ctx.request_repaint_after(GEAR_EVERY);
        }
    }

    /// Back to Currently equipped, with no question pending (the list was read again).
    fn reset(&mut self) {
        self.pick = Some(Pick::Current);
        self.detail = false;
        self.name.clear();
        self.focus_name = false;
        self.confirm_delete = None;
    }

    fn select(&mut self, st: &State, pick: Pick) {
        self.pick = Some(pick);
        self.name = match pick {
            Pick::Saved(i) => st.list.get(i).map(|l| l.name.clone()).unwrap_or_default(),
            Pick::Current => String::new(),
        };
        self.detail = true;
    }

    fn ui(&mut self, ui: &mut Ui, st: &mut State, bot: &Arc<turu_bot::Bot>) {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        let height = ui.available_height();
        if ui.available_width() < NARROW {
            if self.detail && self.pick.is_some() {
                self.detail_pane(ui, st, bot, true);
            } else {
                self.detail = false;
                self.list_pane(ui, st, height);
            }
            return;
        }
        ui.horizontal_top(|ui| {
            let list_width = LIST_WIDTH.min(ui.available_width() * 0.42);
            ui.allocate_ui_with_layout(vec2(list_width, height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(list_width);
                self.list_pane(ui, st, height);
            });
            ui.allocate_ui_with_layout(vec2(ui.available_width(), height), Layout::top_down(Align::Min), |ui| {
                ui.set_width(ui.available_width());
                self.detail_pane(ui, st, bot, false);
            });
        });
    }

    /// VibeSkua's left pane: the saved loadouts and Save current gear.
    fn list_pane(&mut self, ui: &mut Ui, st: &State, height: f32) {
        let busy = st.busy;
        let logged_in = st.username.is_some();
        let control = widgets::target(30.0, 48.0);
        ui.horizontal(|ui| {
            ui.set_min_height(control);
            widgets::section_label(ui, "SAVED LOADOUTS");
            widgets::count_badge(ui, st.list.len());
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
                    model::read_gear(&crate::turu::bot());
                    self.reset();
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
                    let class = st
                        .gear
                        .as_ref()
                        .and_then(|g| g.worn(Slot::Class))
                        .map(|i| i.name.clone());
                    let current = self.pick == Some(Pick::Current);
                    let reading = st.gear.is_none();
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
                    if st.list.is_empty() {
                        ui.add_space(6.0);
                        empty_text(
                            ui,
                            "No loadouts yet. Save current gear keeps what you wear now, under a name.",
                        );
                    }
                    for i in 0..st.list.len() {
                        let loadout = &st.list[i];
                        let worn = st.gear.as_ref().is_some_and(|g| worn_now(g, loadout));
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
            self.select(st, pick);
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
            model::read_worn(&crate::turu::bot(), WornFor::New);
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
    fn detail_pane(&mut self, ui: &mut Ui, st: &mut State, bot: &Arc<turu_bot::Bot>, page: bool) {
        let (busy, status) = (st.busy, st.note.clone());
        let logged_in = st.username.is_some();
        let control = widgets::target(30.0, 48.0);
        let pick = self.pick.unwrap_or(Pick::Current);
        let saved = match pick {
            Pick::Saved(i) => st.list.get(i).cloned(),
            Pick::Current => None,
        };
        let current = st
            .gear
            .as_ref()
            .map(|g| Loadout::from_worn(loadouts::CURRENT, &g.items))
            .unwrap_or_default();
        let shown = saved.as_ref().unwrap_or(&current);

        // Heading. On the narrow window's page: back, then the name under it. Wide: the name
        // beside Replace with worn, as VibeSkua's LOADOUT row and name box in one.
        let header = widgets::target(24.0, 40.0);
        let replace_button = |ui: &mut Ui, height: f32| {
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
                model::read_worn(bot, WornFor::Replace(shown.name.clone()));
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
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| replace_button(ui, header));
                }
            });
            self.title(ui, st, bot, pick, control);
        } else {
            ui.horizontal(|ui| {
                ui.set_min_height(control);
                if saved.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        replace_button(ui, control);
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            self.title(ui, st, bot, pick, control);
                        });
                    });
                } else {
                    self.title(ui, st, bot, pick, control);
                }
            });
        }

        // The slots take what the actions under them leave.
        let footer = self.footer_height.unwrap_or(control * 2.0 + GAP * 2.0);
        let slots_height = (ui.available_height() - footer - GAP).max(widgets::target(34.0, 44.0) * 2.0 + 8.0);
        let changes = match (&saved, &st.gear) {
            (Some(loadout), Some(gear)) => loadouts::enhancement_changes(loadout, &gear.items),
            _ => Vec::new(),
        };
        self.slots(ui, st.gear.as_ref(), shown, saved.is_some(), &changes, slots_height);
        let spare = ui.available_height() - footer;
        if spare > 0.0 {
            ui.add_space(spare);
        }

        let top = ui.cursor().top();
        let script = bot.scripts.is_running().then(|| model::script_name(bot));
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
        if let Some(Note { text, tint }) = &status {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .size(widgets::small_label_size())
                        .color(tint_color(*tint)),
                )
                .wrap(),
            );
        }
        self.actions(
            ui,
            st,
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
    fn title(&mut self, ui: &mut Ui, st: &mut State, bot: &Arc<turu_bot::Bot>, pick: Pick, control: f32) {
        if let Pick::Saved(index) = pick {
            self.name_field(ui, st, bot, index, control);
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

    fn name_field(&mut self, ui: &mut Ui, st: &mut State, bot: &Arc<turu_bot::Bot>, index: usize, control: f32) {
        let id = Id::new("turu_loadouts_name");
        let mut response = ui.add_sized(
            vec2(ui.available_width(), control),
            egui::TextEdit::singleline(&mut self.name)
                .id(id)
                .hint_text("Loadout name")
                .font(FontId::proportional(15.0))
                .vertical_align(Align::Center)
                .margin(Margin::symmetric(10, 4))
                .background_color(INSET),
        );
        let kind = super::native_field::Kind::Line;
        super::native_field::follow(ui, &mut response, &mut self.name, "Loadout name", kind);
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
        if response.lost_focus()
            && let Some(old) = st.list.get(index).map(|l| l.name.clone())
        {
            let typed = self.name.clone();
            self.name = st.rename(bot, &old, &typed);
        }
    }

    /// The seven slots: name, item, enhancement and, for a saved loadout, where the item is.
    fn slots(
        &self,
        ui: &mut Ui,
        gear: Option<&GearFile>,
        loadout: &Loadout,
        saved: bool,
        changes: &[loadouts::EnhancementChange],
        height: f32,
    ) {
        let row = widgets::target(34.0, 44.0);
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
        st: &State,
        bot: &Arc<turu_bot::Bot>,
        saved: Option<&Loadout>,
        differs: bool,
        script_runs: bool,
        busy: Option<Busy>,
        control: f32,
    ) {
        let logged_in = st.username.is_some();
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
                        idle && st.gear.is_some(),
                        vec2(140.0, control),
                        tip,
                    );
                    if save.clicked() {
                        model::read_worn(bot, WornFor::New);
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
                    model::equip(bot, loadout.clone(), script_runs);
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
                        model::enhance(bot, loadout.clone());
                    }
                }
            });
        });
    }

    /// Delete asks first; its items stay where they are.
    fn delete_dialog(&mut self, ctx: &Context, st: &mut State) {
        let Some(index) = self.confirm_delete else {
            return;
        };
        let Some(name) = st.list.get(index).map(|l| l.name.clone()) else {
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
        if answer == Some(true) {
            st.delete(&crate::turu::bot(), &name);
            self.pick = Some(Pick::Current);
            self.detail = false;
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
    if let Some(place) = place.filter(|p| *p != Have::None) {
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
            Have::None | Have::Unknown => (
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
