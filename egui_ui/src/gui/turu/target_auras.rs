//! Combat > Target Auras (Turu's): the auras on the monster you attack and who put each one there,
//! in a small panel over the game (`turu_bot::target_auras` follows them from the packets). The
//! game's own target portrait shows the auras; this says whose they are and what just came off.
//! Scroll of Enrage's two (Focus and Reckless) have a line of their own on top, with who cast them
//! and the seconds each has left, or how long since they came off. Under it each caster's auras
//! (you first, the boss's own last) with their stacks and a bar for the time left. It starts on
//! the right; dragged elsewhere it stays there (`AppSettings::target_auras_pos`), and a
//! double-click puts it back.

use super::player_hud::{LINE, PAD, UNDER_PORTRAIT, bar, figure, movable, short, text};
use super::theme::{CARD, DANGER, EDGE, INSET, MUTED, PANEL, SMALL_RADIUS, TEXT, WARNING};
use egui::{Align, Color32, Context, CornerRadius, FontId, Id, Painter, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use std::time::{Duration, Instant};
use turu_bot::target_auras::{AuraView, CasterAuras, CasterKind, PinState, PinView, TargetView};

const WIDTH: f32 = 236.0;
const HEADER: f32 = 20.0;
/// A pinned line: its label and caster, then its auras' bars.
const PIN_PAD: f32 = 6.0;
const PIN_HEIGHT: f32 = 2.0 * LINE + 2.0 * PIN_PAD;
const SECTION_GAP: f32 = 8.0;
/// An aura: its name and stacks, then the bar of its time left.
const CHIP: f32 = LINE + 4.0;
const CHIP_GAP: f32 = 10.0;
const DRAIN: f32 = 2.0;
/// The lines of auras shown before "+N more" (fewer on a short screen).
const MAX_LINES: usize = 9;
const MAX_LINES_SHORT: usize = 4;
/// Seconds left under which a bar turns yellow: time to put it back on.
const FADING: f32 = 2.0;

pub fn show(ctx: &Context) {
    let bot = crate::turu::bot();
    // Not in Turu Mas, Grid View or Headless Mode, as the Player HUD.
    if cfg!(feature = "air")
        || !bot.settings.read().show_target_auras
        || crate::grid::on()
        || !crate::grid::shell_tiles().is_empty()
        || crate::turu::headless_mode()
    {
        return;
    }
    let me = bot.state.lock().username.clone();
    let Some(view) = bot.target_auras.snapshot(me.as_deref(), Instant::now()) else {
        // A target can come between frames the UI would otherwise skip.
        ctx.request_repaint_after(Duration::from_millis(250));
        return;
    };
    // The bars drain between packets.
    ctx.request_repaint_after(Duration::from_millis(100));
    let area = super::window_area(ctx);
    if area.width() < WIDTH + 2.0 * PAD || area.height() < 120.0 {
        return;
    }
    let max_lines = if area.height() < 420.0 {
        MAX_LINES_SHORT
    } else {
        MAX_LINES
    };
    // Laid out once to know its height, then drawn where the panel lands.
    let measure = ctx.layer_painter(egui::LayerId::background());
    let height = lay(&measure, None, &view, max_lines);
    let size = vec2(WIDTH, height);
    let saved = bot.settings.read().target_auras_pos;
    let moved = movable(
        ctx,
        Id::new("turu_target_auras"),
        area,
        size,
        saved,
        [1.0, UNDER_PORTRAIT],
        |painter, rect| {
            let radius = CornerRadius::same(SMALL_RADIUS);
            painter.rect_filled(rect, radius, PANEL.gamma_multiply(0.88));
            painter.rect_stroke(rect, radius, Stroke::new(1.0, EDGE), StrokeKind::Inside);
            lay(painter, Some(rect.min), &view, max_lines);
        },
    );
    if let Some(spot) = moved {
        bot.settings.write().target_auras_pos = spot;
        bot.save_settings();
    }
}

/// Lays the panel out from `origin` (drawing it there; with `None` only measuring); its height.
fn lay(painter: &Painter, origin: Option<Pos2>, view: &TargetView, max_lines: usize) -> f32 {
    let at = origin.unwrap_or(Pos2::ZERO);
    let left = at.x + PAD;
    let right = at.x + WIDTH - PAD;
    let draw = origin.is_some();
    let mut y = at.y + PAD;

    if draw {
        header(painter, left, right, y, view);
    }
    y += HEADER;

    for pin in &view.pins {
        if draw {
            pinned(painter, left, right, y, pin);
        }
        y += PIN_HEIGHT + SECTION_GAP;
    }

    // Each caster: their name, then their auras in lines, as many as fit in `max_lines`.
    let mut lines = 0;
    let mut hidden = 0;
    for caster in &view.casters {
        if lines >= max_lines {
            hidden += caster.auras.len();
            continue;
        }
        if draw {
            caster_name(painter, left, right, y, caster);
        }
        y += LINE + 2.0;
        let rows = flow(painter, left, right, &caster.auras);
        let fit = rows.len().min(max_lines - lines);
        for (i, row) in rows.iter().enumerate() {
            if i >= fit {
                hidden += row.len();
                continue;
            }
            if draw {
                for (x, width, aura) in row {
                    chip(painter, *x, *width, y, aura, caster.kind);
                }
            }
            y += CHIP + 4.0;
        }
        lines += fit;
        y += 4.0;
    }
    if view.casters.is_empty() {
        if draw {
            text(
                painter,
                pos2(left, y),
                "No other auras",
                11.0,
                MUTED,
                right - left,
                Align::Min,
            );
        }
        y += LINE + 4.0;
    }
    if hidden > 0 {
        if draw {
            let label = format!("+{hidden} more");
            text(painter, pos2(left, y), &label, 11.0, MUTED, right - left, Align::Min);
        }
        y += LINE + 4.0;
    }
    y - 4.0 + PAD - at.y
}

/// The monster's name, its HP as a share of its max (or the figure, without a max) and a thin bar.
fn header(painter: &Painter, left: f32, right: f32, top: f32, view: &TargetView) {
    let hp = view.hp.unwrap_or(0).max(0);
    let share = (view.max_hp > 0).then(|| (hp as f32 / view.max_hp as f32).clamp(0.0, 1.0));
    let value = match (view.hp, share) {
        (Some(_), Some(f)) => format!("{}%", (f * 100.0).ceil() as i64),
        (Some(_), None) => short(hp),
        (None, _) => String::new(),
    };
    let galley = text(
        painter,
        pos2(left, top),
        &view.monster,
        12.0,
        TEXT,
        right - left - 44.0,
        Align::Min,
    );
    // A strong name (drawn twice, a fraction of a point apart), as the Player HUD marks you.
    painter.galley(pos2(left + 0.6, top + (LINE - galley.size().y) / 2.0), galley, TEXT);
    figure(painter, right, top, &value, MUTED);
    bar(painter, left, right, top + LINE + 2.5, 3.0, share, DANGER);
}

/// A pinned line ("Enrage"): who last cast it, then each of its auras with a bar and the seconds
/// left; while none is on, how long since they came off.
fn pinned(painter: &Painter, left: f32, right: f32, top: f32, pin: &PinView) {
    let rect = Rect::from_min_max(pos2(left, top), pos2(right, top + PIN_HEIGHT));
    let radius = CornerRadius::same(6);
    painter.rect_filled(rect, radius, INSET);
    painter.rect_stroke(rect, radius, Stroke::new(1.0, CARD), StrokeKind::Inside);
    let inner = rect.shrink(PIN_PAD);
    let label = text(painter, inner.left_top(), pin.label, 12.0, TEXT, 60.0, Align::Min);
    painter.galley(
        pos2(inner.left() + 0.6, inner.top() + (LINE - label.size().y) / 2.0),
        label,
        TEXT,
    );
    // In words too, so the colour isn't the only sign.
    let (status, color) = match pin.state {
        PinState::Up => (pin.caster.clone().unwrap_or_default(), TEXT),
        PinState::Down { ago, cleansed } => {
            let word = if cleansed { "Cleansed" } else { "Down" };
            (format!("{word} {}s ago", ago.as_secs()), DANGER)
        }
        PinState::Never => ("Not on yet".into(), MUTED),
    };
    let status_left = inner.left() + 64.0;
    text(
        painter,
        pos2(inner.right(), inner.top()),
        &status,
        11.0,
        color,
        inner.right() - status_left,
        Align::Max,
    );

    // Its auras side by side: name, bar, seconds.
    let count = pin.auras.len().max(1) as f32;
    let gap = 10.0;
    let slot = (inner.width() - gap * (count - 1.0)) / count;
    let y = inner.top() + LINE;
    for (i, (name, aura)) in pin.auras.iter().enumerate() {
        let x = inner.left() + i as f32 * (slot + gap);
        let on = aura.is_some();
        let name_color = if on { TEXT } else { MUTED };
        let galley = text(painter, pos2(x, y), name, 11.0, name_color, slot * 0.5, Align::Min);
        let seconds = aura
            .as_ref()
            .and_then(|a| a.left)
            .map_or_else(|| "–".to_owned(), |left| format!("{}s", left.ceil() as i64));
        let bar_left = x + galley.size().x + 6.0;
        let bar_right = x + slot - 24.0;
        let (share, color) = drain(aura.as_ref());
        bar(painter, bar_left, bar_right, y + LINE / 2.0, 3.0, share, color);
        figure(painter, x + slot, y, &seconds, if on { TEXT } else { MUTED });
    }
}

/// A caster's name: you strong, a caster not known and the boss's own in grey.
fn caster_name(painter: &Painter, left: f32, right: f32, top: f32, caster: &CasterAuras) {
    let color = match caster.kind {
        CasterKind::Me | CasterKind::Player => TEXT,
        CasterKind::Unknown | CasterKind::Monster => MUTED,
    };
    let galley = text(
        painter,
        pos2(left, top),
        &caster.name,
        12.0,
        color,
        right - left,
        Align::Min,
    );
    if caster.kind == CasterKind::Me {
        painter.galley(pos2(left + 0.6, top + (LINE - galley.size().y) / 2.0), galley, color);
    }
}

/// `auras` in lines across `left..right`: each one's left edge and width.
fn flow<'a>(painter: &Painter, left: f32, right: f32, auras: &'a [AuraView]) -> Vec<Vec<(f32, f32, &'a AuraView)>> {
    let mut rows: Vec<Vec<(f32, f32, &AuraView)>> = Vec::new();
    let mut x = left;
    for aura in auras {
        let width = chip_width(painter, aura).min(right - left);
        if rows.is_empty() || (x > left && x + width > right) {
            rows.push(Vec::new());
            x = left;
        }
        if let Some(row) = rows.last_mut() {
            row.push((x, width, aura));
        }
        x += width + CHIP_GAP;
    }
    rows
}

fn chip_width(painter: &Painter, aura: &AuraView) -> f32 {
    let name = painter
        .layout_no_wrap(aura.name.clone(), FontId::proportional(11.0), TEXT)
        .size()
        .x;
    let stacks = stacks(aura).map_or(0.0, |s| {
        3.0 + painter.layout_no_wrap(s, FontId::monospace(11.0), MUTED).size().x
    });
    (name + stacks).max(28.0)
}

/// "×10" for an aura of more than one stack.
fn stacks(aura: &AuraView) -> Option<String> {
    (aura.stacks > 1).then(|| format!("×{}", aura.stacks))
}

/// One aura: its name and stacks, a bar under it for the time it has left (none for one that
/// doesn't run out).
fn chip(painter: &Painter, x: f32, width: f32, top: f32, aura: &AuraView, kind: CasterKind) {
    let color = if kind == CasterKind::Monster { MUTED } else { TEXT };
    let stacks = stacks(aura);
    let stacks_width = stacks.as_ref().map_or(0.0, |s| {
        3.0 + painter
            .layout_no_wrap(s.clone(), FontId::monospace(11.0), MUTED)
            .size()
            .x
    });
    let name = text(
        painter,
        pos2(x, top),
        &aura.name,
        11.0,
        color,
        width - stacks_width,
        Align::Min,
    );
    if let Some(stacks) = stacks {
        let galley = painter.layout_no_wrap(stacks, FontId::monospace(11.0), MUTED);
        let y = top + (LINE - galley.size().y) / 2.0;
        painter.galley(pos2(x + name.size().x + 3.0, y), galley, MUTED);
    }
    if aura.left.is_some() {
        let (share, color) = drain(Some(aura));
        bar(painter, x, x + width, top + LINE + DRAIN, DRAIN, share, color);
    }
}

/// How much of an aura's time is left and the bar's colour: yellow in its last seconds.
fn drain(aura: Option<&AuraView>) -> (Option<f32>, Color32) {
    let Some((left, duration)) = aura.and_then(|a| Some((a.left?, a.duration?))) else {
        return (None, TEXT);
    };
    let share = (duration > 0.0).then(|| (left / duration).clamp(0.0, 1.0));
    let color = if left <= FADING {
        WARNING
    } else {
        TEXT.gamma_multiply(0.7)
    };
    (share, color)
}
