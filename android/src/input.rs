//! Touch and keys, for the game (Ruffle's player events) or Turu's UI (egui's). Touch is the
//! mouse: a finger down is a left click, dragging moves the mouse (several fingers share the
//! game's one mouse: `turu_frontend::touch`); egui also gets it as a touch, so that it knows
//! it's on a touch screen (a drag on a list scrolls it). Keys (a hardware keyboard, or
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
use turu_frontend::touch::Mouse;

/// A finger (Android's pointer ID) coming down, moving or lifting, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touch {
    Down(i32, f64, f64),
    Move(i32, f64, f64),
    Up(i32, f64, f64),
    /// Every finger lifted: Android took the gesture away.
    Cancel,
}

/// What `event` says each finger did. A move carries every finger that's down; a mouse or stylus
/// (with its button down or hovering) is a finger too.
pub fn motion(event: &MotionEvent) -> Vec<Touch> {
    let changed = || {
        let pointer = event.pointer_at_index(event.pointer_index());
        (pointer.pointer_id(), f64::from(pointer.x()), f64::from(pointer.y()))
    };
    match event.action() {
        MotionAction::Down | MotionAction::PointerDown => {
            let (id, x, y) = changed();
            vec![Touch::Down(id, x, y)]
        }
        MotionAction::Up | MotionAction::PointerUp => {
            let (id, x, y) = changed();
            vec![Touch::Up(id, x, y)]
        }
        MotionAction::Cancel => vec![Touch::Cancel],
        MotionAction::Move | MotionAction::HoverMove => event
            .pointers()
            .map(|pointer| Touch::Move(pointer.pointer_id(), f64::from(pointer.x()), f64::from(pointer.y())))
            .collect(),
        // A mouse button comes with a Down or an Up too.
        _ => Vec::new(),
    }
}

/// The game's mouse doing `mouse`, with `left` pixels of Turu's rail beside the game and `top`
/// pixels of its tab strip above it.
pub fn player_event(mouse: Mouse, left: f64, top: f64) -> PlayerEvent {
    match mouse {
        Mouse::Down(x, y) => PlayerEvent::MouseDown {
            x: x - left,
            y: y - top,
            button: MouseButton::Left,
            index: None,
        },
        Mouse::Move(x, y) => PlayerEvent::MouseMove {
            x: x - left,
            y: y - top,
        },
        Mouse::Up(x, y) => PlayerEvent::MouseUp {
            x: x - left,
            y: y - top,
            button: MouseButton::Left,
        },
    }
}

/// For egui, at `pixels_per_point`: a touch at `x`, `y`, then the mouse events it stands for (as
/// egui-winit sends them). Without the touch, egui takes the screen for a mouse's and scrolls
/// nothing on a drag (`ScrollArea`'s drag-to-scroll is on touch screens only). egui follows one
/// finger at a time (`Screen::touch`), so it's always touch 0, as it's the one mouse.
pub fn egui_events(phase: egui::TouchPhase, x: f64, y: f64, pixels_per_point: f32) -> Vec<egui::Event> {
    let pos = egui::pos2(x as f32 / pixels_per_point, y as f32 / pixels_per_point);
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let touch = egui::Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(0),
        phase,
        pos,
        force: None,
    };
    match phase {
        egui::TouchPhase::Start => vec![touch, egui::Event::PointerMoved(pos), button(true)],
        egui::TouchPhase::Move => vec![touch, egui::Event::PointerMoved(pos)],
        egui::TouchPhase::End | egui::TouchPhase::Cancel => vec![touch, button(false), egui::Event::PointerGone],
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
