//! Turu's own focused text box (egui's) as a native Android text field just above the soft
//! keyboard, as the game's are (`game_field.rs`): Kotlin's `NativeField`, with Android's caret,
//! selection handles, text toolbar, suggestions, autocorrect and autofill. egui alone gets only
//! keys from the keyboard.
//!
//! The text boxes say what they hold each frame (`turu_frontend::gui::turu::native_field`); the
//! screen keeps Kotlin's field in step after each frame ([`UiField::sync`]). Kotlin's edits come
//! back here as the whole text and selection and reach the box in the next frame; its Send is
//! Enter in the box, and Back takes the focus off it.

use turu_frontend::gui::turu::native_field::{self, Change, Contents, Kind, NativeField};
use turu_frontend::shell::{FieldId, ToShell};

/// Kotlin's field for Turu's text boxes, as last shown or edited.
#[derive(Default)]
pub struct UiField {
    field: NativeField,
}

impl UiField {
    /// After a UI frame: shows, updates or hides Kotlin's field to match the focused text box.
    /// True while it's up (egui's own keyboard path then stays down).
    pub fn sync(&mut self, ctx: &egui::Context) -> bool {
        match self.field.sync(native_field::take_focused(ctx)) {
            Some(Change::Show(focused)) => show(&focused.contents, focused.kind, &focused.hint),
            Some(Change::Update(contents)) => update(&contents),
            Some(Change::Hide) => hide(),
            None => {}
        }
        self.field.is_shown()
    }

    /// Whether Kotlin's field is up.
    pub fn is_shown(&self) -> bool {
        self.field.is_shown()
    }
}

/// Shows the shell's field for Turu's text box (`ToShell::FieldShow`: only slots run the engine).
fn show(contents: &Contents, kind: Kind, hint: &str) {
    crate::slots::to_shell(ToShell::FieldShow {
        field: FieldId::Ui,
        text: contents.text.clone(),
        start: contents.start as u32,
        end: contents.end as u32,
        password: kind == Kind::Password,
        max: 0,
        hint: hint.to_owned(),
        lines: kind == Kind::Lines,
        number: kind == Kind::Number,
    });
}

/// The shell's field takes the text box's text and selection.
fn update(contents: &Contents) {
    crate::slots::to_shell(ToShell::FieldUpdate {
        field: FieldId::Ui,
        text: contents.text.clone(),
        start: contents.start as u32,
        end: contents.end as u32,
    });
}

/// Hides the shell's field.
fn hide() {
    crate::slots::to_shell(ToShell::FieldHide { field: FieldId::Ui });
}

/// The shell's field above the keyboard was edited (`ToClient::FieldEdited`).
pub(crate) fn edited(contents: Contents) {
    crate::run_on_engine(move |_| {
        crate::with_screen(|screen| {
            let (field, ctx) = screen.ui_field();
            field.field.edited(ctx, contents);
        });
    });
}

/// The field's Send.
pub(crate) fn entered() {
    crate::run_on_engine(|_| {
        crate::with_screen(|screen| native_field::enter(screen.ui_field().1));
    });
}

/// The field went.
pub(crate) fn closed() {
    crate::run_on_engine(|_| {
        crate::with_screen(|screen| {
            let (field, ctx) = screen.ui_field();
            field.field.closed(ctx);
        });
    });
}
