//! Grid View's tile labels and borders (`crate::grid`): each tab's name (with a dot while its
//! bot is busy and a "!" while it waits for you), the active tab's tile outlined.

use super::theme::{ACCENT, EDGE, SUCCESS, TEXT, WARNING};
use egui::{Color32, Context, CornerRadius, FontId, Id, LayerId, Order, Pos2, Rect, Stroke, StrokeKind, vec2};

pub fn show(ctx: &Context) {
    let tiles = crate::grid::tiles();
    if tiles.is_empty() {
        return;
    }
    let ppp = ctx.pixels_per_point();
    let active = crate::turu::active_tab();
    let painter = ctx.layer_painter(LayerId::new(Order::Background, Id::new("turu_grid_overlay")));
    for (tab, tile) in tiles {
        let rect = Rect::from_min_size(
            Pos2::new(tile.x as f32 / ppp, tile.y as f32 / ppp),
            vec2(tile.width as f32 / ppp, tile.height as f32 / ppp),
        );
        let is_active = tab == active;
        let stroke = if is_active {
            Stroke::new(2.0, ACCENT)
        } else {
            Stroke::new(1.0, EDGE)
        };
        painter.rect_stroke(rect.shrink(0.5), CornerRadius::ZERO, stroke, StrokeKind::Inside);
        let Some(bot) = crate::turu::tab_bot(tab) else {
            continue;
        };
        let waits = bot.scripts.waits_for().is_some() || bot.butler.suspend_notice().is_some();
        let label = format!("{}{}", crate::turu::tab_label(&bot), if waits { "  !" } else { "" });
        let font = FontId::proportional(12.0);
        let galley = painter.layout_no_wrap(label, font, if waits { WARNING } else { TEXT });
        let busy = super::busy(&bot);
        let pad = vec2(6.0, 3.0);
        let dot = if busy { 10.0 } else { 0.0 };
        let bg = Rect::from_min_size(rect.min + vec2(4.0, 4.0), galley.size() + pad * 2.0 + vec2(dot, 0.0));
        painter.rect_filled(bg, CornerRadius::same(4), Color32::from_black_alpha(170));
        if busy {
            painter.circle_filled(bg.left_center() + vec2(pad.x + 3.0, 0.0), 3.0, SUCCESS);
        }
        painter.galley(bg.min + pad + vec2(dot, 0.0), galley, TEXT);
    }
}
