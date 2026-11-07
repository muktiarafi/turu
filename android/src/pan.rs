//! Keeps Turu's own text box above the soft keyboard: while an egui text field has the focus and
//! the keyboard is up, the window (layer) that holds it is moved up just enough for the field to
//! show above the keyboard, like Android's `adjustPan` but for that window only; it moves back
//! when the keyboard goes down or the field loses the focus. egui moves the window's input with
//! it (a layer transform).

use egui::emath::TSTransform;
use egui::output::IMEOutput;
use egui::{Context, LayerId, vec2};
use std::cell::Cell;

/// The space kept between the field and the keyboard, in points.
const MARGIN: f32 = 8.0;

thread_local! {
    /// The layer moved up, and by how much (points). The UI runs on the engine thread.
    static PANNED: Cell<Option<(LayerId, f32)>> = const { Cell::new(None) };
}

/// After a UI frame: `ime` is the frame's IME output (the focused text field), `height` the
/// screen's height and `keyboard` the keyboard's, in points. The move shows from the next frame.
pub(crate) fn follow_text_field(ctx: &Context, ime: Option<&IMEOutput>, height: f32, keyboard: f32) {
    let before = PANNED.get();
    let wanted = ime.filter(|_| keyboard > 0.0).and_then(|ime| {
        let layer = ctx.layer_id_at(ime.rect.center())?;
        // Where the field is with the layer where it belongs.
        let moved = match before {
            Some((panned, by)) if panned == layer => by,
            _ => 0.0,
        };
        let bottom = ime.rect.bottom() + moved;
        let top = ime.rect.top() + moved;
        let visible_bottom = height - keyboard - MARGIN;
        // Not above the screen's top either.
        let by = (bottom - visible_bottom).min(top - MARGIN).max(0.0);
        (by > 0.0).then_some((layer, by))
    });
    if wanted == before {
        return;
    }
    if let Some((layer, _)) = before
        && wanted.is_none_or(|(now, _)| now != layer)
    {
        ctx.set_transform_layer(layer, TSTransform::IDENTITY);
    }
    if let Some((layer, by)) = wanted {
        ctx.set_transform_layer(layer, TSTransform::from_translation(vec2(0.0, -by)));
    }
    PANNED.set(wanted);
    ctx.request_repaint();
}
