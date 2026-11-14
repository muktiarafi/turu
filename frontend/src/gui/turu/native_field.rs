//! Turu's own text boxes typed in through a native text field (Android): the soft keyboard types
//! into a real Android field just above it, with Android's caret, selection handles, text toolbar,
//! suggestions, autocorrect and autofill, as the game's text fields are (`android/src/game_field.rs`,
//! Kotlin's `NativeField`). egui alone gets only keys from the keyboard.
//!
//! Each of Turu's text boxes calls [`follow`] after it's added: while it has the focus, it says
//! what it holds (its text, selection and kind) for the frame, and takes the native field's edits.
//! The frontend reads the frame's focused box after the UI ran ([`take_focused`]), and keeps the
//! native field in step with it ([`NativeField::sync`]). The native field's edits come back as the
//! whole text and selection ([`edited`]); its Enter (Send) is Enter in egui, after the last edit
//! ([`enter`], [`input_events`]); Back takes the focus off the box ([`closed`]).
//!
//! Positions are UTF-16 units here, as Java's strings; egui's are characters.

use egui::text::{CCursor, CCursorRange};
use egui::{Context, Event, Id, Key, Modifiers, Response, TextEdit, Ui};

/// What a text box is for, which picks the native field's keyboard and look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// One line of text.
    Line,
    /// One line, shown as dots.
    Password,
    /// Several lines: Enter starts a new one.
    Lines,
    /// A number.
    Number,
}

/// What a text box holds: its text, and its selection in UTF-16 units.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contents {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// The text box with the focus in a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focused {
    pub id: Id,
    pub contents: Contents,
    pub kind: Kind,
    pub hint: String,
}

/// The native field's edit, for the box `id` to take in the next frame.
#[derive(Clone)]
struct Edit {
    id: Id,
    contents: Contents,
}

fn focused_id() -> Id {
    Id::new("turu_native_field_focused")
}

fn edit_id() -> Id {
    Id::new("turu_native_field_edit")
}

fn enter_id() -> Id {
    Id::new("turu_native_field_enter")
}

/// Call right after adding a text box (`response` its response, `text` what it edits): while it
/// has the focus, it takes the native field's edit, if one came, and says what it holds now.
pub fn follow(ui: &Ui, response: &mut Response, text: &mut String, hint: &str, kind: Kind) {
    if !response.has_focus() {
        return;
    }
    let ctx = ui.ctx();
    let id = response.id;
    let edit = ctx.data_mut(|d| {
        let ours = d.get_temp::<Edit>(edit_id()).is_some_and(|e| e.id == id);
        if ours { take::<Edit>(d, edit_id()) } else { None }
    });
    if let Some(edit) = edit {
        if *text != edit.contents.text {
            *text = edit.contents.text.clone();
            response.mark_changed();
        }
        let start = char_index(text, edit.contents.start);
        let end = char_index(text, edit.contents.end);
        let mut state = TextEdit::load_state(ctx, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(start), CCursor::new(end))));
        TextEdit::store_state(ctx, id, state);
        ctx.request_repaint();
    }
    let (start, end) = TextEdit::load_state(ctx, id)
        .and_then(|state| state.cursor.char_range())
        .map_or_else(
            || {
                let end = text.chars().count();
                (end, end)
            },
            |range| {
                let range = range.as_sorted_char_range();
                (range.start.0, range.end.0)
            },
        );
    let focused = Focused {
        id,
        contents: Contents {
            text: text.clone(),
            start: utf16_index(text, start),
            end: utf16_index(text, end),
        },
        kind,
        hint: hint.to_owned(),
    };
    ctx.data_mut(|d| d.insert_temp(focused_id(), focused));
}

/// The text box that had the focus in the frame that just ran, if it was one of Turu's
/// ([`follow`]). Call once after each frame.
pub fn take_focused(ctx: &Context) -> Option<Focused> {
    ctx.data_mut(|d| take::<Focused>(d, focused_id()))
        .filter(|f| ctx.memory(|m| m.focused()) == Some(f.id))
}

/// The native field's text or selection changed: the box `id` takes them in the next frame.
pub fn edited(ctx: &Context, id: Id, contents: Contents) {
    ctx.data_mut(|d| d.insert_temp(edit_id(), Edit { id, contents }));
    ctx.request_repaint();
}

/// The native field's Enter (the keyboard's Send): Enter in egui, once its last edit is in
/// ([`input_events`]).
pub fn enter(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(enter_id(), true));
    ctx.request_repaint();
}

/// The native field was closed (Back): the box `id` loses the focus.
pub fn closed(ctx: &Context, id: Id) {
    ctx.memory_mut(|m| m.surrender_focus(id));
    ctx.request_repaint();
}

/// Events for the next frame's input: Enter, when the native field pressed it and its last edit
/// is in the box (or Enter would act on the text before it).
pub fn input_events(ctx: &Context) -> Vec<Event> {
    let ready =
        ctx.data(|d| d.get_temp::<bool>(enter_id()).unwrap_or(false) && d.get_temp::<Edit>(edit_id()).is_none());
    if !ready {
        return Vec::new();
    }
    ctx.data_mut(|d| d.remove::<bool>(enter_id()));
    [true, false]
        .into_iter()
        .map(|pressed| Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .collect()
}

/// What to do with the native field after a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Show it for this box (a new one, or one of another kind).
    Show(Focused),
    /// The box's text or selection changed: the field shows them.
    Update(Contents),
    /// No box of Turu's has the focus.
    Hide,
}

/// The native field as the frontend last showed it.
#[derive(Default)]
pub struct NativeField {
    shown: Option<Focused>,
}

impl NativeField {
    /// After a frame: what the native field should do to match `focused` ([`take_focused`]).
    pub fn sync(&mut self, focused: Option<Focused>) -> Option<Change> {
        let change = match (&self.shown, &focused) {
            (None, None) => None,
            (Some(_), None) => Some(Change::Hide),
            (Some(shown), Some(now)) if shown.id == now.id && shown.kind == now.kind && shown.hint == now.hint => {
                (shown.contents != now.contents).then(|| Change::Update(now.contents.clone()))
            }
            (_, Some(now)) => Some(Change::Show(now.clone())),
        };
        self.shown = focused;
        change
    }

    /// The box the native field is for, while it's up.
    pub fn id(&self) -> Option<Id> {
        self.shown.as_ref().map(|f| f.id)
    }

    /// Whether the native field is up.
    pub fn is_shown(&self) -> bool {
        self.shown.is_some()
    }

    /// The native field changed: it holds `contents` now (not to be sent back to it), and the box
    /// takes them ([`edited`]).
    pub fn edited(&mut self, ctx: &Context, contents: Contents) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        shown.contents = contents.clone();
        edited(ctx, shown.id, contents);
    }

    /// The native field was closed (Back): its box loses the focus ([`closed`]).
    pub fn closed(&mut self, ctx: &Context) {
        if let Some(shown) = self.shown.take() {
            closed(ctx, shown.id);
        }
    }
}

/// The temporary value `id` holds, removed.
fn take<T: 'static + Clone + Send + Sync>(data: &mut egui::util::IdTypeMap, id: Id) -> Option<T> {
    let value = data.get_temp::<T>(id);
    data.remove::<T>(id);
    value
}

/// The character index of UTF-16 index `utf16` in `text` (the end for one past it).
fn char_index(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (i, c) in text.chars().enumerate() {
        if units >= utf16 {
            return i;
        }
        units += c.len_utf16();
    }
    text.chars().count()
}

/// The UTF-16 index of character index `chars` in `text`.
fn utf16_index(text: &str, chars: usize) -> usize {
    text.chars().take(chars).map(char::len_utf16).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_count_utf16_units() {
        let text = "a😀b";
        assert_eq!(utf16_index(text, 2), 3);
        assert_eq!(char_index(text, 3), 2);
        assert_eq!(char_index(text, 99), 3);
        assert_eq!(utf16_index(text, 0), 0);
    }

    fn focused(id: &str, text: &str) -> Focused {
        Focused {
            id: Id::new(id),
            contents: Contents {
                text: text.into(),
                start: 0,
                end: 0,
            },
            kind: Kind::Line,
            hint: String::new(),
        }
    }

    #[test]
    fn sync_shows_updates_and_hides() {
        let mut field = NativeField::default();
        assert_eq!(field.sync(None), None);
        let a = focused("a", "x");
        assert_eq!(field.sync(Some(a.clone())), Some(Change::Show(a.clone())));
        assert_eq!(field.sync(Some(a.clone())), None);
        let changed = focused("a", "xy");
        assert_eq!(
            field.sync(Some(changed.clone())),
            Some(Change::Update(changed.contents))
        );
        let b = focused("b", "");
        assert_eq!(field.sync(Some(b.clone())), Some(Change::Show(b)));
        assert_eq!(field.sync(None), Some(Change::Hide));
    }

    /// An edit from the native field isn't sent back to it, and the box takes it in the next frame.
    #[test]
    fn edit_reaches_the_box() {
        let ctx = Context::default();
        let mut field = NativeField::default();
        let mut text = String::from("ab");
        let run = |text: &mut String| {
            let mut output = ctx.run_ui(Default::default(), |ui| {
                let mut response = ui.add(TextEdit::singleline(text));
                response.request_focus();
                follow(ui, &mut response, text, "", Kind::Line);
            });
            output.textures_delta.clear();
            take_focused(&ctx)
        };
        run(&mut text);
        let now = run(&mut text).expect("focused");
        assert!(matches!(field.sync(Some(now)), Some(Change::Show(_))));
        let typed = Contents {
            text: "abc".into(),
            start: 3,
            end: 3,
        };
        field.edited(&ctx, typed.clone());
        let now = run(&mut text).expect("focused");
        assert_eq!(text, "abc");
        assert_eq!(now.contents, typed);
        assert_eq!(field.sync(Some(now)), None);
    }

    /// Enter waits for an edit that hasn't reached the box.
    #[test]
    fn enter_follows_the_last_edit() {
        let ctx = Context::default();
        enter(&ctx);
        edited(&ctx, Id::new("a"), Contents::default());
        assert!(input_events(&ctx).is_empty());
        ctx.data_mut(|d| d.remove::<Edit>(edit_id()));
        assert_eq!(input_events(&ctx).len(), 2);
        assert!(input_events(&ctx).is_empty());
    }
}
