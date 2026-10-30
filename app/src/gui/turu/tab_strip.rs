//! The tab strip above the nav bar (VibeSkua's `TabbedHostWindow`), once there are two tabs:
//! one tab per game client, named after its character, with a dot while its script or Auto
//! Attack runs, "×" to close it, and "+" for a new one at the login screen.

use super::theme::{ACCENT, EDGE, HOVER, MUTED, PANEL, SUCCESS, TEXT, TRACK};
use crate::gui::TAB_STRIP_HEIGHT;
use crate::turu::{self, TabRequest};
use egui::{Align2, Color32, CornerRadius, FontId, Frame, Margin, Panel, RichText, ScrollArea, Sense, Ui, pos2, vec2};

/// The widest a tab gets; longer names are cut.
const TAB_WIDTH: f32 = 168.0;

#[derive(Default)]
pub struct TabStrip;

impl TabStrip {
    pub fn show(&self, ui: &mut Ui) {
        let frame = Frame::new().fill(TRACK).inner_margin(Margin {
            left: 6,
            right: 6,
            top: 3,
            bottom: 0,
        });
        Panel::top("turu_tabs")
            .exact_size(TAB_STRIP_HEIGHT as f32)
            .frame(frame)
            .show_separator_line(false)
            .show(ui, |ui| {
                ScrollArea::horizontal()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let active = turu::active_tab();
                            for (tab, bot) in turu::tabs() {
                                let busy = bot.scripts.status().is_active() || super::auto::running(&bot);
                                tab_button(ui, tab, &turu::tab_label(&bot), tab == active, busy);
                            }
                            let plus = ui
                                .add(egui::Button::new(RichText::new("+").size(16.0)).frame(false))
                                .on_hover_text(
                                    "New tab (log in there, or start an account in it from Army > Accounts)",
                                );
                            if plus.clicked() {
                                turu::request_tab(TabRequest::Open(Box::default()));
                            }
                        });
                    });
            });
    }
}

fn tab_button(ui: &mut Ui, tab: turu::TabId, label: &str, active: bool, busy: bool) {
    let height = TAB_STRIP_HEIGHT as f32 - 3.0;
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), TEXT);
    let width = (galley.size().x + 52.0).min(TAB_WIDTH);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let close_rect = egui::Rect::from_center_size(pos2(rect.right() - 13.0, rect.center().y), vec2(16.0, 16.0));
    let close = ui.interact(close_rect, ui.id().with(("turu_tab_close", tab)), Sense::click());

    let fill = if active {
        PANEL
    } else if response.hovered() {
        HOVER
    } else {
        Color32::TRANSPARENT
    };
    let radius = CornerRadius {
        nw: 8,
        ne: 8,
        sw: 0,
        se: 0,
    };
    ui.painter().rect_filled(rect, radius, fill);
    if active {
        let top = egui::Rect::from_min_size(rect.min, vec2(rect.width(), 2.0));
        ui.painter().rect_filled(top, radius, ACCENT);
    } else {
        let edge = egui::Rect::from_min_size(pos2(rect.right() - 1.0, rect.top() + 6.0), vec2(1.0, height - 12.0));
        ui.painter().rect_filled(edge, 0.0, EDGE);
    }
    let dot = pos2(rect.left() + 12.0, rect.center().y);
    ui.painter().circle_filled(dot, 3.5, if busy { SUCCESS } else { MUTED });
    let text_rect = egui::Rect::from_min_max(
        pos2(rect.left() + 22.0, rect.top()),
        pos2(close_rect.left() - 4.0, rect.bottom()),
    );
    ui.painter().with_clip_rect(text_rect).text(
        pos2(text_rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        if active { TEXT } else { MUTED },
    );
    if close.hovered() {
        ui.painter().rect_filled(close_rect, CornerRadius::same(4), HOVER);
    }
    ui.painter().text(
        close_rect.center(),
        Align2::CENTER_CENTER,
        "×",
        FontId::proportional(13.0),
        MUTED,
    );

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if close.on_hover_text("Close this tab (stops its script)").clicked() {
        turu::request_tab(TabRequest::Close(tab));
    } else if response.on_hover_text(label).clicked() && !active {
        turu::request_tab(TabRequest::Select(tab));
    }
}
