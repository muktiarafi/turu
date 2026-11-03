//! Touch and keys to Ruffle's player events. Touch is the mouse: a finger down is a left click,
//! dragging moves the mouse. Keys (a hardware keyboard, or the soft keyboard, which NativeActivity
//! gets as key events) are named from Android's key code, and their character comes from the
//! device's key character map, so Shift and other layouts type what they show.

use android_activity::AndroidApp;
use android_activity::input::{
    KeyAction, KeyEvent, KeyMapChar, Keycode, MotionAction, MotionEvent,
};
use ruffle_core::PlayerEvent;
use ruffle_core::events::{KeyDescriptor, KeyLocation, LogicalKey, MouseButton, NamedKey, PhysicalKey};

/// The player events for a touch, in window pixels (the player's viewport).
pub fn motion(event: &MotionEvent) -> Option<PlayerEvent> {
    let pointer = event.pointer_at_index(event.pointer_index());
    let (x, y) = (f64::from(pointer.x()), f64::from(pointer.y()));
    Some(match event.action() {
        MotionAction::Down | MotionAction::PointerDown | MotionAction::ButtonPress => PlayerEvent::MouseDown {
            x,
            y,
            button: MouseButton::Left,
            index: None,
        },
        MotionAction::Up | MotionAction::PointerUp | MotionAction::ButtonRelease | MotionAction::Cancel => {
            PlayerEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            }
        }
        MotionAction::Move | MotionAction::HoverMove => PlayerEvent::MouseMove { x, y },
        _ => return None,
    })
}

/// The player events for a key: KeyDown or KeyUp, and with a key down that types a character,
/// TextInput. None for keys the player has no use for (volume, back).
pub fn key(app: &AndroidApp, event: &KeyEvent) -> Option<Vec<PlayerEvent>> {
    let typed = app
        .device_key_character_map(event.device_id())
        .ok()
        .and_then(|map| map.get(event.key_code(), event.meta_state()).ok())
        .and_then(|c| match c {
            KeyMapChar::Unicode(c) => Some(c),
            _ => None,
        });
    let (logical_key, key_location) = match named(event.key_code()) {
        Some(named) => named,
        None => (LogicalKey::Character(typed?), KeyLocation::Standard),
    };
    let key = KeyDescriptor {
        physical_key: PhysicalKey::Unknown,
        logical_key,
        key_location,
    };
    match event.action() {
        KeyAction::Down => {
            let mut events = vec![PlayerEvent::KeyDown { key }];
            if let Some(c) = typed.filter(|c| !c.is_control()) {
                events.push(PlayerEvent::TextInput { codepoint: c });
            }
            Some(events)
        }
        KeyAction::Up => Some(vec![PlayerEvent::KeyUp { key }]),
        _ => None,
    }
}

/// Keys that don't type a character.
fn named(code: Keycode) -> Option<(LogicalKey, KeyLocation)> {
    let (key, location) = match code {
        Keycode::Enter | Keycode::NumpadEnter => (NamedKey::Enter, KeyLocation::Standard),
        Keycode::Tab => (NamedKey::Tab, KeyLocation::Standard),
        Keycode::Del => (NamedKey::Backspace, KeyLocation::Standard),
        Keycode::ForwardDel => (NamedKey::Delete, KeyLocation::Standard),
        Keycode::Escape => (NamedKey::Escape, KeyLocation::Standard),
        Keycode::DpadUp => (NamedKey::ArrowUp, KeyLocation::Standard),
        Keycode::DpadDown => (NamedKey::ArrowDown, KeyLocation::Standard),
        Keycode::DpadLeft => (NamedKey::ArrowLeft, KeyLocation::Standard),
        Keycode::DpadRight => (NamedKey::ArrowRight, KeyLocation::Standard),
        Keycode::MoveHome => (NamedKey::Home, KeyLocation::Standard),
        Keycode::MoveEnd => (NamedKey::End, KeyLocation::Standard),
        Keycode::PageUp => (NamedKey::PageUp, KeyLocation::Standard),
        Keycode::PageDown => (NamedKey::PageDown, KeyLocation::Standard),
        Keycode::ShiftLeft => (NamedKey::Shift, KeyLocation::Left),
        Keycode::ShiftRight => (NamedKey::Shift, KeyLocation::Right),
        Keycode::CtrlLeft => (NamedKey::Control, KeyLocation::Left),
        Keycode::CtrlRight => (NamedKey::Control, KeyLocation::Right),
        Keycode::AltLeft => (NamedKey::Alt, KeyLocation::Left),
        Keycode::AltRight => (NamedKey::Alt, KeyLocation::Right),
        Keycode::CapsLock => (NamedKey::CapsLock, KeyLocation::Standard),
        _ => return None,
    };
    Some((LogicalKey::Named(key), location))
}
