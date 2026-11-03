//! Touch and keys, for the game (Ruffle's player events) or Turu's UI (egui's). Touch is the
//! mouse: a finger down is a left click, dragging moves the mouse. Keys (a hardware keyboard, or
//! the soft keyboard, which NativeActivity gets as key events) are named from Android's key code,
//! and their character comes from the device's key character map, so Shift and other layouts
//! type what they show. `screen.rs` decides who gets which.

use android_activity::AndroidApp;
use android_activity::input::{
    KeyAction, KeyEvent, KeyMapChar, Keycode, MetaState, MotionAction, MotionEvent,
};
use ruffle_core::PlayerEvent;
use ruffle_core::events::{
    KeyDescriptor, KeyLocation, LogicalKey, MouseButton, NamedKey, PhysicalKey, TextControlCode,
};

/// A touch, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touch {
    Down(f64, f64),
    Move(f64, f64),
    Up(f64, f64),
}

pub fn motion(event: &MotionEvent) -> Option<Touch> {
    let pointer = event.pointer_at_index(event.pointer_index());
    let (x, y) = (f64::from(pointer.x()), f64::from(pointer.y()));
    Some(match event.action() {
        MotionAction::Down | MotionAction::PointerDown | MotionAction::ButtonPress => Touch::Down(x, y),
        MotionAction::Up | MotionAction::PointerUp | MotionAction::ButtonRelease | MotionAction::Cancel => {
            Touch::Up(x, y)
        }
        MotionAction::Move | MotionAction::HoverMove => Touch::Move(x, y),
        _ => return None,
    })
}

impl Touch {
    /// For the game, with `top` pixels of Turu's bar above it.
    pub fn player_event(self, top: f64) -> PlayerEvent {
        match self {
            Touch::Down(x, y) => PlayerEvent::MouseDown {
                x,
                y: y - top,
                button: MouseButton::Left,
                index: None,
            },
            Touch::Move(x, y) => PlayerEvent::MouseMove { x, y: y - top },
            Touch::Up(x, y) => PlayerEvent::MouseUp {
                x,
                y: y - top,
                button: MouseButton::Left,
            },
        }
    }

    /// For egui, at `pixels_per_point`.
    pub fn egui_events(self, pixels_per_point: f32) -> Vec<egui::Event> {
        let at = |x: f64, y: f64| egui::pos2(x as f32 / pixels_per_point, y as f32 / pixels_per_point);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        match self {
            Touch::Down(x, y) => vec![egui::Event::PointerMoved(at(x, y)), button(at(x, y), true)],
            Touch::Move(x, y) => vec![egui::Event::PointerMoved(at(x, y))],
            Touch::Up(x, y) => vec![button(at(x, y), false), egui::Event::PointerGone],
        }
    }
}

/// A key press or release, before it's given to the game or to Turu's UI.
pub struct Key {
    pub down: bool,
    pub key: KeyDescriptor,
    /// The character it types, if any (with a key down).
    pub typed: Option<char>,
    /// What it does in a text field instead of typing (Backspace, arrows, Ctrl+V), with a key down.
    pub control: Option<TextControlCode>,
}

impl Key {
    /// For the game: KeyDown or KeyUp, and the text field's TextControl or TextInput (as the
    /// desktop's `app.rs` sends them).
    pub fn player_events(&self) -> Vec<PlayerEvent> {
        if !self.down {
            return vec![PlayerEvent::KeyUp { key: self.key }];
        }
        let mut events = vec![PlayerEvent::KeyDown { key: self.key }];
        if let Some(code) = self.control {
            events.push(PlayerEvent::TextControl { code });
        } else if let Some(c) = self.typed {
            events.push(PlayerEvent::TextInput { codepoint: c });
        }
        events
    }

    /// For egui: the typed text, or the named key.
    pub fn egui_events(&self) -> Vec<egui::Event> {
        if let (true, Some(c)) = (self.down, self.typed) {
            return vec![egui::Event::Text(c.to_string())];
        }
        let LogicalKey::Named(named) = self.key.logical_key else {
            return Vec::new();
        };
        let key = match named {
            NamedKey::Enter => egui::Key::Enter,
            NamedKey::Tab => egui::Key::Tab,
            NamedKey::Backspace => egui::Key::Backspace,
            NamedKey::Delete => egui::Key::Delete,
            NamedKey::Escape => egui::Key::Escape,
            NamedKey::ArrowUp => egui::Key::ArrowUp,
            NamedKey::ArrowDown => egui::Key::ArrowDown,
            NamedKey::ArrowLeft => egui::Key::ArrowLeft,
            NamedKey::ArrowRight => egui::Key::ArrowRight,
            NamedKey::Home => egui::Key::Home,
            NamedKey::End => egui::Key::End,
            NamedKey::PageUp => egui::Key::PageUp,
            NamedKey::PageDown => egui::Key::PageDown,
            _ => return Vec::new(),
        };
        vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: self.down,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }
}

/// A key event, read: None for keys neither the game nor the UI uses (volume, back).
pub fn key(app: &AndroidApp, event: &KeyEvent) -> Option<Key> {
    let map = app.device_key_character_map(event.device_id()).ok();
    let character = |meta| {
        map.as_ref()
            .and_then(|map| map.get(event.key_code(), meta).ok())
            .and_then(|c| match c {
                KeyMapChar::Unicode(c) => Some(c),
                _ => None,
            })
    };
    let meta = event.meta_state();
    let typed = character(meta);
    // With Ctrl a key types nothing, but it's still its letter (Ctrl+A, Ctrl+V in a text field).
    let letter = typed.or_else(|| meta.ctrl_on().then(|| character(MetaState(0))).flatten());
    let (logical_key, key_location) = match named(event.key_code()) {
        Some(named) => named,
        None => (LogicalKey::Character(letter?), KeyLocation::Standard),
    };
    let down = match event.action() {
        KeyAction::Down => true,
        KeyAction::Up => false,
        _ => return None,
    };
    let control = text_control(logical_key, meta.shift_on(), meta.ctrl_on()).filter(|_| down);
    Some(Key {
        down,
        key: KeyDescriptor {
            physical_key: PhysicalKey::Unknown,
            logical_key,
            key_location,
        },
        typed: typed.filter(|c| down && !c.is_control()),
        control,
    })
}

/// A key's text field action: the desktop's `winit_to_ruffle_text_control`.
fn text_control(key: LogicalKey, shift: bool, ctrl: bool) -> Option<TextControlCode> {
    use TextControlCode as C;
    let named = match key {
        LogicalKey::Character(c) if ctrl => {
            return match c.to_ascii_lowercase() {
                'a' => Some(C::SelectAll),
                'c' => Some(C::Copy),
                'v' => Some(C::Paste),
                'x' => Some(C::Cut),
                _ => None,
            };
        }
        LogicalKey::Named(named) => named,
        _ => return None,
    };
    // Each key: plain, with Shift, with Ctrl, with both.
    let [plain, shifted, word, shifted_word] = match named {
        NamedKey::Enter => return Some(C::Enter),
        NamedKey::Backspace => return Some(if ctrl { C::BackspaceWord } else { C::Backspace }),
        NamedKey::Delete => return Some(if ctrl { C::DeleteWord } else { C::Delete }),
        NamedKey::ArrowLeft => [C::MoveLeft, C::SelectLeft, C::MoveLeftWord, C::SelectLeftWord],
        NamedKey::ArrowRight => [C::MoveRight, C::SelectRight, C::MoveRightWord, C::SelectRightWord],
        NamedKey::Home => [C::MoveLeftLine, C::SelectLeftLine, C::MoveLeftDocument, C::SelectLeftDocument],
        NamedKey::End => [C::MoveRightLine, C::SelectRightLine, C::MoveRightDocument, C::SelectRightDocument],
        _ => return None,
    };
    Some(match (ctrl, shift) {
        (false, false) => plain,
        (false, true) => shifted,
        (true, false) => word,
        (true, true) => shifted_word,
    })
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
