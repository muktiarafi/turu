//! The game's focused text field as a native Android text field just above the soft keyboard
//! (Kotlin's `NativeField`, as AQW's own Android launcher does): the keyboard covers the game's
//! field, and a native one has Android's own caret, selection handles, text toolbar, suggestions
//! and clipboard.
//!
//! The screen keeps the two in step each time it draws ([`GameField::sync`]): Kotlin's field shows
//! when one of the game's editable fields takes the focus, follows the game's changes to it (a
//! message sent clears it), and goes when the focus does. Each edit there comes back as the whole
//! text and selection ([`GameField::edited`]) and is typed into the game's field as the change it
//! is (the replaced part as a `textInput`, as a keyboard types it), so the game sees ordinary
//! typing. Positions are UTF-16 units on both sides (Java's strings, Flash's text).

use ruffle_core::events::{
    KeyDescriptor, KeyLocation, LogicalKey, NamedKey, PhysicalKey, TextControlCode,
};
use ruffle_core::{Player, PlayerEvent, TextSelection};
use turu_frontend::shell::{FieldId, ToShell};

/// The most the game takes in a message: the native field stops there.
pub const MAX_CHARS: usize = 150;

/// What a text field holds: its text and selection (UTF-16 units).
#[derive(Clone, Debug, Default, PartialEq)]
struct Contents {
    text: String,
    start: usize,
    end: usize,
}

/// The game's focused field, when one you type in has the focus.
struct Focused {
    contents: Contents,
    password: bool,
}

/// Kotlin's field, as last shown or edited.
#[derive(Default)]
pub struct GameField {
    shown: Option<Contents>,
}

impl GameField {
    /// Shows, updates or hides Kotlin's field to match the game's focused field; hidden while
    /// `aside` (one of Turu's text boxes has the native field), the game's field keeping the focus.
    pub fn sync(&mut self, player: &mut Player, aside: bool) {
        let focused = if aside { None } else { focused(player) };
        match (&self.shown, focused) {
            (None, Some(focused)) => {
                show(&focused);
                self.shown = Some(focused.contents);
            }
            (Some(shown), Some(focused)) if *shown != focused.contents => {
                update(&focused.contents);
                self.shown = Some(focused.contents);
            }
            (Some(_), None) => {
                hide();
                self.shown = None;
            }
            _ => {}
        }
    }

    /// Kotlin's field changed: the game's field takes its `text` and selection.
    fn edited(&mut self, player: &mut Player, text: String, start: usize, end: usize) {
        player.mutate_with_update_context(|context| {
            let Some(field) = context.focus_tracker.get_as_edit_text() else {
                return;
            };
            if !field.is_editable() {
                return;
            }
            let old: Vec<u16> = field.text().iter().collect();
            let new: Vec<u16> = text.encode_utf16().collect();
            if old != new {
                // Only what changed is replaced: the common start and end stay.
                let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
                let most = old.len().min(new.len()) - prefix;
                let suffix = old
                    .iter()
                    .rev()
                    .zip(new.iter().rev())
                    .take(most)
                    .take_while(|(a, b)| a == b)
                    .count();
                field.set_selection(Some(TextSelection::for_range(prefix, old.len() - suffix)));
                let typed = String::from_utf16_lossy(&new[prefix..new.len() - suffix]);
                if typed.is_empty() {
                    field.text_control_input(TextControlCode::Backspace, context);
                } else {
                    field.text_input(typed, context);
                }
            }
            field.set_selection(Some(TextSelection::for_range(start, end)));
        });
        // What Kotlin holds now; if the game took something else (a character it doesn't allow),
        // the next sync shows the game's.
        self.shown = Some(Contents { text, start, end });
    }

    /// Kotlin's field was closed (Back): done typing, the game's field loses the focus.
    fn closed(&mut self, player: &mut Player) {
        self.shown = None;
        player.mutate_with_update_context(|context| {
            let tracker = context.focus_tracker;
            tracker.set(None, context);
        });
    }
}

/// The game's focused field, if it's one you type in.
fn focused(player: &mut Player) -> Option<Focused> {
    player.mutate_with_update_context(|context| {
        let field = context.focus_tracker.get_as_edit_text()?;
        if !field.is_editable() {
            return None;
        }
        let text = field.text();
        let (start, end) = field
            .selection()
            .map_or((text.len(), text.len()), |s| (s.start(), s.end()));
        Some(Focused {
            contents: Contents {
                text: text.to_utf8_lossy().into_owned(),
                start,
                end,
            },
            password: field.is_password(),
        })
    })
}

/// Whether the game's focused field is full: a typed character goes nowhere (a hardware
/// keyboard's, while the native field isn't up).
pub fn is_full(player: &mut Player) -> bool {
    focused(player).is_some_and(|f| {
        f.contents.start == f.contents.end && f.contents.text.encode_utf16().count() >= MAX_CHARS
    })
}

/// Enter in the game, as a keyboard presses it (AQW sends a chat message on it).
fn enter(player: &mut Player) {
    let key = KeyDescriptor {
        physical_key: PhysicalKey::Unknown,
        logical_key: LogicalKey::Named(NamedKey::Enter),
        key_location: KeyLocation::Standard,
    };
    player.handle_event(PlayerEvent::KeyDown { key });
    player.handle_event(PlayerEvent::TextControl {
        code: TextControlCode::Enter,
    });
    player.handle_event(PlayerEvent::KeyUp { key });
}

/// Shows the shell's field for the game's focused text field (`ToShell::FieldShow`: only slots run
/// the engine).
fn show(focused: &Focused) {
    let Contents { text, start, end } = &focused.contents;
    crate::slots::to_shell(ToShell::FieldShow {
        field: FieldId::Game,
        text: text.clone(),
        start: *start as u32,
        end: *end as u32,
        password: focused.password,
        max: MAX_CHARS as u32,
        hint: String::new(),
        lines: false,
        number: false,
    });
}

/// The shell's field takes the game field's text and selection.
fn update(contents: &Contents) {
    crate::slots::to_shell(ToShell::FieldUpdate {
        field: FieldId::Game,
        text: contents.text.clone(),
        start: contents.start as u32,
        end: contents.end as u32,
    });
}

/// Hides the shell's field.
fn hide() {
    crate::slots::to_shell(ToShell::FieldHide {
        field: FieldId::Game,
    });
}

/// The shell's field above the keyboard was edited (`ToClient::FieldEdited`).
pub(crate) fn edited(text: String, start: usize, end: usize) {
    crate::run_on_engine(move |player| {
        crate::with_screen(|screen| screen.game_field.edited(player, text, start, end));
    });
}

/// The field's Send.
pub(crate) fn entered() {
    crate::run_on_engine(enter);
}

/// The field went.
pub(crate) fn closed() {
    crate::run_on_engine(|player| {
        crate::with_screen(|screen| screen.game_field.closed(player));
    });
}

