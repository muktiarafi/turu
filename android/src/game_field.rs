//! The game's focused text field as a native Android text field just above the soft keyboard
//! (Kotlin's `GameField`, as AQW's own Android launcher does): the keyboard covers the game's
//! field, and a native one has Android's own caret, selection handles, text toolbar, suggestions
//! and clipboard.
//!
//! The screen keeps the two in step each time it draws ([`GameField::sync`]): Kotlin's field shows
//! when one of the game's editable fields takes the focus, follows the game's changes to it (a
//! message sent clears it), and goes when the focus does. Each edit there comes back as the whole
//! text and selection ([`GameField::edited`]) and is typed into the game's field as the change it
//! is (the replaced part as a `textInput`, as a keyboard types it), so the game sees ordinary
//! typing. Positions are UTF-16 units on both sides (Java's strings, Flash's text).

use crate::platform::with_activity;
use jni::objects::{JClass, JString, JValue};
use jni::sys::jint;
use jni::{EnvUnowned, jni_sig, jni_str};
use ruffle_core::events::{
    KeyDescriptor, KeyLocation, LogicalKey, NamedKey, PhysicalKey, TextControlCode,
};
use ruffle_core::{Player, PlayerEvent, TextSelection};

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
    /// Shows, updates or hides Kotlin's field to match the game's focused field.
    pub fn sync(&mut self, player: &mut Player) {
        let focused = focused(player);
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

/// `TuruActivity.showGameField(text, start, end, password, max)`.
fn show(focused: &Focused) {
    let Some(app) = crate::app() else {
        return;
    };
    let Contents { text, start, end } = &focused.contents;
    let result = with_activity(&app, |env, activity| {
        let text = env.new_string(text)?;
        env.call_method(
            activity,
            jni_str!("showGameField"),
            jni_sig!("(Ljava/lang/String;IIZI)V"),
            &[
                JValue::Object(&text),
                JValue::Int(*start as jint),
                JValue::Int(*end as jint),
                JValue::Bool(focused.password),
                JValue::Int(MAX_CHARS as jint),
            ],
        )?;
        Ok(())
    });
    if let Err(e) = result {
        tracing::warn!("Couldn't show the game's text field: {e}");
    }
}

/// `TuruActivity.updateGameField(text, start, end)`.
fn update(contents: &Contents) {
    let Some(app) = crate::app() else {
        return;
    };
    let result = with_activity(&app, |env, activity| {
        let text = env.new_string(&contents.text)?;
        env.call_method(
            activity,
            jni_str!("updateGameField"),
            jni_sig!("(Ljava/lang/String;II)V"),
            &[
                JValue::Object(&text),
                JValue::Int(contents.start as jint),
                JValue::Int(contents.end as jint),
            ],
        )?;
        Ok(())
    });
    if let Err(e) = result {
        tracing::warn!("Couldn't update the game's text field: {e}");
    }
}

/// `TuruActivity.hideGameField()`.
fn hide() {
    let Some(app) = crate::app() else {
        return;
    };
    let result = with_activity(&app, |env, activity| {
        env.call_method(activity, jni_str!("hideGameField"), jni_sig!("()V"), &[])?;
        Ok(())
    });
    if let Err(e) = result {
        tracing::warn!("Couldn't hide the game's text field: {e}");
    }
}

/// `TuruActivity.nativeGameFieldEdited(text, start, end)`, on the UI thread.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeGameFieldEdited<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    text: JString<'local>,
    start: jint,
    end: jint,
) {
    let text = unowned_env
        .with_env(|env| -> jni::errors::Result<String> { text.try_to_string(env) })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let (start, end) = (start.max(0) as usize, end.max(0) as usize);
    crate::run_on_engine(move |player| {
        crate::with_screen(|screen| screen.game_field.edited(player, text, start, end));
    });
}

/// `TuruActivity.nativeGameFieldEnter()`, on the UI thread.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeGameFieldEnter<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    crate::run_on_engine(enter);
}

/// `TuruActivity.nativeGameFieldClosed()`, on the UI thread.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeGameFieldClosed<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    crate::run_on_engine(|player| {
        crate::with_screen(|screen| screen.game_field.closed(player));
    });
}
