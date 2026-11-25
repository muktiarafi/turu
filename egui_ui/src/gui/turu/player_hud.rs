//! Combat > Player HUD (Turu's): the HP and MP of the players in your cell, in a small panel over
//! the game (`turu_bot::vitals` follows the numbers). It starts on the left under the game's portrait of you; dragged
//! elsewhere it stays there (`AppSettings::player_hud_pos`), and a double-click puts it back.

use super::theme::{DANGER, EDGE, MANA, MUTED, PANEL, SMALL_RADIUS, SUCCESS, TEXT, TRACK, WARNING};
use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align, Color32, Context, CornerRadius, CursorIcon, FontId, Id, LayerId, Order, Painter, Pos2, Rect, Sense, Stroke,
    StrokeKind, TextFormat, Vec2, pos2, vec2,
};
use std::time::Duration;
use turu_bot::vitals::{RoomVitals, Vital};

const WIDTH: f32 = 216.0;
const PAD: f32 = 8.0;
/// The name column; the bars take what's left of the row but the figures.
const NAME_WIDTH: f32 = 82.0;
/// Room for "12.3k" or "Dead".
const VALUE_WIDTH: f32 = 34.0;
const GAP: f32 = 6.0;
const HP_BAR: f32 = 5.0;
const MP_BAR: f32 = 3.0;
/// A player's two lines: the name with HP, then MP.
const LINE: f32 = 14.0;
const ROW_GAP: f32 = 5.0;
const HEADER: f32 = 18.0;
/// Rows shown before "+N more" (fewer on a short screen, a phone held sideways).
const MAX_ROWS: usize = 8;
const MAX_ROWS_SHORT: usize = 5;
/// Where the game's portrait of you ends, down its height (AQW draws it in the top-left
/// corner, about 100 of the stage's 550 pixels high).
const UNDER_PORTRAIT: f32 = 0.2;

pub fn show(ctx: &Context) {
    let bot = crate::turu::bot();
    // Not in Turu Mas (AIR's overlay places the game its own way), Grid View (a shell's client
    // draws it with `shell_tiles`) or Headless Mode.
    if cfg!(feature = "air")
        || !bot.settings.read().show_player_hud
        || crate::grid::on()
        || !crate::grid::shell_tiles().is_empty()
        || crate::turu::headless_mode()
    {
        return;
    }
    // The numbers change between frames the UI would otherwise skip.
    ctx.request_repaint_after(Duration::from_millis(250));
    let Some(room) = bot.vitals.snapshot() else {
        return;
    };
    let area = super::window_area(ctx);
    if area.width() < WIDTH + 2.0 * PAD || area.height() < 80.0 {
        return;
    }
    let max_rows = if area.height() < 420.0 {
        MAX_ROWS_SHORT
    } else {
        MAX_ROWS
    };
    let size = vec2(WIDTH, height(room.players.len(), max_rows));
    // Dragged anywhere over the game (a window raises over it as over another window); a
    // double-click puts it back in the corner. Where it is follows the window's size.
    let id = Id::new("turu_player_hud");
    let saved = bot.settings.read().player_hud_pos;
    let mut panel = egui::Area::new(id)
        .order(Order::Middle)
        .movable(true)
        .sense(Sense::click_and_drag())
        .constrain_to(area);
    // Where a drag has the panel, until it is saved: egui moves it while the button is down, but
    // on the frame it comes up the drag is over and the saved spot would put it back.
    let held_id = id.with("held");
    let held: Option<Pos2> = ctx.data(|d| d.get_temp(held_id));
    panel = panel.current_pos(held.unwrap_or_else(|| place(saved, area, size)));
    let mut cut = Vec::new();
    let shown = panel.show(ctx, |ui| {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        cut = draw(ui.painter(), rect, &room, max_rows);
    });
    let response = shown.response;
    if response.dragged() {
        ctx.data_mut(|d| d.insert_temp(held_id, response.rect.min));
    }
    if response.double_clicked() {
        ctx.data_mut(|d| d.remove::<Pos2>(held_id));
        bot.settings.write().player_hud_pos = None;
        bot.save_settings();
    } else if response.drag_stopped() {
        let at = held.unwrap_or(response.rect.min);
        ctx.data_mut(|d| d.remove::<Pos2>(held_id));
        bot.settings.write().player_hud_pos = Some(spot(Rect::from_min_size(at, size), area));
        bot.save_settings();
    }
    if response.dragged() {
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    } else if response.hovered() {
        ctx.set_cursor_icon(CursorIcon::Grab);
    }
    // A name cut short shows whole under the pointer.
    if let Some(pointer) = ctx.pointer_hover_pos()
        && let Some((_, name)) = cut.iter().find(|(rect, _)| rect.contains(pointer))
    {
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("turu_player_hud_name")));
        let galley = painter.layout_no_wrap(name.clone(), FontId::proportional(12.0), TEXT);
        let size = galley.size() + vec2(12.0, 8.0);
        let at = pos2((pointer.x - size.x - 8.0).max(area.left()), pointer.y + 12.0);
        let rect = Rect::from_min_size(at, size);
        let radius = CornerRadius::same(6);
        painter.rect_filled(rect, radius, PANEL);
        painter.rect_stroke(rect, radius, Stroke::new(1.0, EDGE), StrokeKind::Inside);
        painter.galley(rect.min + vec2(6.0, 4.0), galley, TEXT);
    }
}

/// The panel's height for `players` rows, `max_rows` of them shown.
fn height(players: usize, max_rows: usize) -> f32 {
    let shown = players.min(max_rows);
    let rows_height = shown as f32 * (2.0 * LINE + ROW_GAP) - ROW_GAP;
    let more_height = if players > shown { LINE + ROW_GAP } else { 0.0 };
    PAD + HEADER + rows_height + more_height + PAD
}

/// Where the panel goes: the saved spot (see `AppSettings::player_hud_pos`), else on the left
/// under the game's own portrait of you (its name, HP and MP: the top [`UNDER_PORTRAIT`] of the
/// game).
fn place(saved: Option<[f32; 2]>, area: Rect, size: Vec2) -> Pos2 {
    let [across, down] = saved.unwrap_or([0.0, UNDER_PORTRAIT]);
    let room = (area.width() - size.x - 2.0 * PAD).max(0.0);
    let x = area.left() + PAD + room * across.clamp(0.0, 1.0);
    let y = area.top() + area.height() * down.clamp(0.0, 1.0);
    pos2(x, y.min(area.bottom() - size.y).max(area.top()))
}

/// The spot to save for a panel at `rect`.
fn spot(rect: Rect, area: Rect) -> [f32; 2] {
    let room = (area.width() - rect.width() - 2.0 * PAD).max(1.0);
    let across = ((rect.left() - area.left() - PAD) / room).clamp(0.0, 1.0);
    let down = ((rect.top() - area.top()) / area.height().max(1.0)).clamp(0.0, 1.0);
    [across, down]
}

/// Draws the panel in `rect`; the names cut short, with where they are.
fn draw(painter: &Painter, rect: Rect, room: &RoomVitals, max_rows: usize) -> Vec<(Rect, String)> {
    let shown = room.players.len().min(max_rows);
    let more = room.players.len() - shown;
    let radius = CornerRadius::same(SMALL_RADIUS);
    painter.rect_filled(rect, radius, PANEL.gamma_multiply(0.88));
    painter.rect_stroke(rect, radius, Stroke::new(1.0, EDGE), StrokeKind::Inside);

    let inner = rect.shrink(PAD);
    // The header: the cell, and how many are in it.
    let cell = if room.cell.is_empty() {
        "Cell"
    } else {
        room.cell.as_str()
    };
    text(
        painter,
        inner.left_top(),
        cell,
        12.0,
        TEXT,
        inner.width() - 40.0,
        Align::Min,
    );
    let count = room.players.len().to_string();
    text(
        painter,
        pos2(inner.right(), inner.top()),
        &count,
        12.0,
        MUTED,
        40.0,
        Align::Max,
    );

    let mut y = inner.top() + HEADER;
    let mut cut = Vec::new();
    for player in &room.players[..shown] {
        if row(painter, inner.left(), inner.right(), y, player) {
            let rect = Rect::from_min_size(pos2(inner.left(), y), vec2(NAME_WIDTH, LINE));
            cut.push((rect, player.name.clone()));
        }
        y += 2.0 * LINE + ROW_GAP;
    }
    if more > 0 {
        let label = format!("+{more} more");
        text(
            painter,
            pos2(inner.left(), y),
            &label,
            11.0,
            MUTED,
            inner.width(),
            Align::Min,
        );
    }
    cut
}

/// One player: the name, the HP bar and its figure; under them "Low HP" (under a quarter) or
/// nothing, the MP bar and its figure. True when the name was cut short.
fn row(painter: &Painter, left: f32, right: f32, top: f32, player: &Vital) -> bool {
    let name_color = if player.dead { MUTED } else { TEXT };
    let size = 12.0;
    let galley = text(
        painter,
        pos2(left, top),
        &player.name,
        size,
        name_color,
        NAME_WIDTH,
        Align::Min,
    );
    let whole = painter.layout_no_wrap(player.name.clone(), FontId::proportional(size), name_color);
    let cut = whole.size().x > NAME_WIDTH;
    if player.me {
        // You: a strong name (drawn twice, a fraction of a point apart), not a colour.
        let y = top + (LINE - galley.size().y) / 2.0;
        painter.galley(pos2(left + 0.6, y), galley, name_color);
    }
    let bar_left = left + NAME_WIDTH + GAP;
    let bar_right = right - VALUE_WIDTH - GAP;
    let hp_fraction = fraction(player.hp, player.max_hp);
    let hp_color = if player.dead {
        MUTED
    } else {
        match hp_fraction {
            Some(f) if f < 0.25 => DANGER,
            Some(f) if f < 0.5 => WARNING,
            _ => SUCCESS,
        }
    };
    let hp_mid = top + LINE / 2.0;
    bar(painter, bar_left, bar_right, hp_mid, HP_BAR, hp_fraction, hp_color);
    let hp_text = if player.dead {
        "Dead".to_owned()
    } else {
        short(player.hp)
    };
    let value_color = if player.dead || hp_color == SUCCESS {
        TEXT
    } else {
        hp_color
    };
    figure(painter, right, top, &hp_text, value_color);

    let mp_top = top + LINE;
    // In words too, so the colour isn't the only sign.
    if hp_color == DANGER {
        text(
            painter,
            pos2(left, mp_top),
            "Low HP",
            11.0,
            DANGER,
            NAME_WIDTH,
            Align::Min,
        );
    }
    let mp_fraction = fraction(player.mp, player.max_mp);
    bar(
        painter,
        bar_left,
        bar_right,
        mp_top + LINE / 2.0,
        MP_BAR,
        mp_fraction,
        MANA,
    );
    figure(painter, right, mp_top, &short(player.mp), MUTED);
    cut
}

/// A bar `thickness` high centred on `mid`; without a fraction (no max known) only its track.
fn bar(painter: &Painter, left: f32, right: f32, mid: f32, thickness: f32, fraction: Option<f32>, color: Color32) {
    if right <= left {
        return;
    }
    let track = Rect::from_min_max(pos2(left, mid - thickness / 2.0), pos2(right, mid + thickness / 2.0));
    let radius = CornerRadius::same((thickness / 2.0) as u8);
    painter.rect_filled(track, radius, TRACK);
    if let Some(f) = fraction.filter(|f| *f > 0.0) {
        let fill = Rect::from_min_max(track.min, pos2(left + (right - left) * f, track.max.y));
        painter.rect_filled(fill, radius, color);
    }
}

/// A figure right-aligned at `right`, in a fixed-width face so it doesn't shift as it changes.
fn figure(painter: &Painter, right: f32, top: f32, value: &str, color: Color32) {
    let galley = painter.layout_no_wrap(value.to_owned(), FontId::monospace(11.0), color);
    let y = top + (LINE - galley.size().y) / 2.0;
    painter.galley(pos2(right - galley.size().x, y), galley, color);
}

/// One line of text cut with "…" at `width`, its left edge (or right, with `Align::Max`) at
/// `at`; the galley, to draw again.
fn text(
    painter: &Painter,
    at: Pos2,
    value: &str,
    size: f32,
    color: Color32,
    width: f32,
    align: Align,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::single_section(
        value.to_owned(),
        TextFormat {
            font_id: FontId::proportional(size),
            color,
            ..Default::default()
        },
    );
    job.wrap = TextWrapping::truncate_at_width(width.max(1.0));
    let galley = painter.layout_job(job);
    let x = match align {
        Align::Max => at.x - galley.size().x,
        _ => at.x,
    };
    let y = at.y + (LINE - galley.size().y) / 2.0;
    painter.galley(pos2(x, y), galley.clone(), color);
    galley
}

/// How full a bar is, `None` when the max isn't known.
fn fraction(value: i64, max: i64) -> Option<f32> {
    (max > 0).then(|| (value as f32 / max as f32).clamp(0.0, 1.0))
}

/// A figure in at most five characters: 950, 2.4k, 12k, 1.2m (rounded down, so a bar that isn't
/// full never reads as its max).
fn short(value: i64) -> String {
    let value = value.max(0);
    match value {
        0..=999 => value.to_string(),
        1_000..=9_999 => format!("{}.{}k", value / 1000, value % 1000 / 100),
        10_000..=999_999 => format!("{}k", value / 1000),
        _ => format!("{}.{}m", value / 1_000_000, value % 1_000_000 / 100_000),
    }
}

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn figures_stay_short() {
        assert_eq!(short(950), "950");
        assert_eq!(short(2_400), "2.4k");
        assert_eq!(short(9_999), "9.9k");
        assert_eq!(short(24_000), "24k");
        assert_eq!(short(250_000), "250k");
        assert_eq!(short(1_250_000), "1.2m");
        assert_eq!(short(-5), "0");
    }
}
