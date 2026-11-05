//! Several fingers on the game, which has one mouse. AQW's touch controls (its on-screen
//! joystick, skill buttons and keys, `liteAssets.mobile` in the client) all listen to the mouse:
//! a press on the control, then mouse moves and the mouse release on the stage. A second finger
//! pressing a skill button while the first one drags the joystick would move that one mouse
//! across the screen and release it, which ends the joystick's drag (and steers it towards the
//! button on the way).
//!
//! So the first finger on the game holds the mouse, and each finger that comes while it does is
//! a tap: the mouse is released where the holding finger is, pressed and released where the new
//! finger is (a skill button fires on that release), then given back to the holding finger by
//! pressing where it first came down and moving to where it is now. The joystick takes the press
//! as its centre, as it did the first time, so it steers on in the same direction within the same
//! frame. A holding finger that hadn't moved yet isn't pressed again until it does: a finger
//! resting on a skill button would fire it a second time when it lifts.
//!
//! The tapping finger's moves and release do nothing while a finger holds the mouse. When the
//! holding finger lifts, the mouse is released and the next finger to come down holds it.

/// What the game's mouse does, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mouse {
    Move(f64, f64),
    Down(f64, f64),
    Up(f64, f64),
}

/// The finger holding the mouse.
#[derive(Clone, Copy, Debug)]
struct Holder {
    id: i32,
    /// Where it came down.
    origin: (f64, f64),
    /// Where it is.
    at: (f64, f64),
    /// Whether it moved further than the slop from `origin` (a drag).
    dragged: bool,
    /// Whether the mouse is down for it. It isn't after a tap until it moves.
    pressed: bool,
}

/// The fingers on the game, as the one mouse it has.
#[derive(Debug)]
pub struct GameTouches {
    holder: Option<Holder>,
    /// How far a finger moves, in pixels, before it's a drag.
    slop: f64,
}

impl GameTouches {
    /// A finger counts as dragging once it moved `slop` pixels.
    pub fn new(slop: f64) -> Self {
        Self { holder: None, slop }
    }

    /// Finger `id` came down on the game.
    pub fn down(&mut self, id: i32, x: f64, y: f64) -> Vec<Mouse> {
        let Some(holder) = &mut self.holder else {
            self.holder = Some(Holder {
                id,
                origin: (x, y),
                at: (x, y),
                dragged: false,
                pressed: true,
            });
            return vec![Mouse::Move(x, y), Mouse::Down(x, y)];
        };
        if holder.id == id {
            // Its release was lost: start over.
            *holder = Holder {
                id,
                origin: (x, y),
                at: (x, y),
                dragged: false,
                pressed: true,
            };
            return vec![Mouse::Up(x, y), Mouse::Move(x, y), Mouse::Down(x, y)];
        }
        let mut out = Vec::new();
        if holder.pressed {
            out.push(Mouse::Up(holder.at.0, holder.at.1));
        }
        out.extend([Mouse::Move(x, y), Mouse::Down(x, y), Mouse::Up(x, y)]);
        holder.pressed = holder.dragged;
        if holder.pressed {
            out.extend(Self::press(holder));
        } else {
            out.push(Mouse::Move(holder.at.0, holder.at.1));
        }
        out
    }

    /// Finger `id` moved to `x`, `y`.
    pub fn moved(&mut self, id: i32, x: f64, y: f64) -> Vec<Mouse> {
        let Some(holder) = &mut self.holder else {
            // No button down: the mouse hovers.
            return vec![Mouse::Move(x, y)];
        };
        if holder.id != id {
            return Vec::new();
        }
        holder.at = (x, y);
        let (dx, dy) = (x - holder.origin.0, y - holder.origin.1);
        holder.dragged |= dx * dx + dy * dy >= self.slop * self.slop;
        if holder.pressed {
            vec![Mouse::Move(x, y)]
        } else if holder.dragged {
            holder.pressed = true;
            Self::press(holder)
        } else {
            Vec::new()
        }
    }

    /// Finger `id` lifted at `x`, `y`.
    pub fn up(&mut self, id: i32, x: f64, y: f64) -> Vec<Mouse> {
        match self.holder {
            Some(holder) if holder.id == id => {
                self.holder = None;
                if holder.pressed {
                    vec![Mouse::Up(x, y)]
                } else {
                    vec![Mouse::Move(x, y)]
                }
            }
            _ => Vec::new(),
        }
    }

    /// Every finger lifted at once (Android cancelled the gesture).
    pub fn cancel(&mut self) -> Vec<Mouse> {
        match self.holder.take() {
            Some(holder) if holder.pressed => vec![Mouse::Up(holder.at.0, holder.at.1)],
            _ => Vec::new(),
        }
    }

    /// Whether a finger holds the mouse.
    pub fn holding(&self) -> bool {
        self.holder.is_some()
    }

    /// Press for `holder` again: where it came down, then where it is.
    fn press(holder: &Holder) -> Vec<Mouse> {
        vec![
            Mouse::Move(holder.origin.0, holder.origin.1),
            Mouse::Down(holder.origin.0, holder.origin.1),
            Mouse::Move(holder.at.0, holder.at.1),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Mouse::*;

    #[test]
    fn one_finger_is_the_mouse() {
        let mut t = GameTouches::new(10.0);
        assert_eq!(t.down(0, 5.0, 5.0), [Move(5.0, 5.0), Down(5.0, 5.0)]);
        assert_eq!(t.moved(0, 6.0, 5.0), [Move(6.0, 5.0)]);
        assert_eq!(t.up(0, 6.0, 5.0), [Up(6.0, 5.0)]);
        assert!(!t.holding());
    }

    #[test]
    fn a_tap_while_dragging_gives_the_drag_back() {
        let mut t = GameTouches::new(10.0);
        t.down(0, 100.0, 400.0);
        t.moved(0, 140.0, 400.0);
        assert_eq!(
            t.down(1, 900.0, 300.0),
            [
                Up(140.0, 400.0),
                Move(900.0, 300.0),
                Down(900.0, 300.0),
                Up(900.0, 300.0),
                Move(100.0, 400.0),
                Down(100.0, 400.0),
                Move(140.0, 400.0),
            ]
        );
        // The tapping finger does nothing more; the drag goes on.
        assert_eq!(t.moved(1, 905.0, 300.0), []);
        assert_eq!(t.up(1, 905.0, 300.0), []);
        assert_eq!(t.moved(0, 150.0, 390.0), [Move(150.0, 390.0)]);
        assert_eq!(t.up(0, 150.0, 390.0), [Up(150.0, 390.0)]);
    }

    #[test]
    fn a_resting_finger_is_pressed_again_only_when_it_moves() {
        let mut t = GameTouches::new(10.0);
        t.down(0, 800.0, 300.0);
        assert_eq!(
            t.down(1, 900.0, 300.0),
            [
                Up(800.0, 300.0),
                Move(900.0, 300.0),
                Down(900.0, 300.0),
                Up(900.0, 300.0),
                Move(800.0, 300.0)
            ]
        );
        assert_eq!(t.moved(0, 803.0, 300.0), []);
        // Lifting it now clicks nothing.
        assert_eq!(t.up(0, 803.0, 300.0), [Move(803.0, 300.0)]);

        t.down(0, 100.0, 400.0);
        t.down(1, 900.0, 300.0);
        assert_eq!(
            t.moved(0, 120.0, 400.0),
            [Move(100.0, 400.0), Down(100.0, 400.0), Move(120.0, 400.0)]
        );
        assert_eq!(t.up(0, 120.0, 400.0), [Up(120.0, 400.0)]);
    }

    #[test]
    fn the_next_finger_holds_the_mouse_after_the_holder_lifts() {
        let mut t = GameTouches::new(10.0);
        t.down(0, 100.0, 400.0);
        t.down(1, 900.0, 300.0);
        t.up(0, 100.0, 400.0);
        // Finger 1 is still down, but it was a tap: it only moves the mouse.
        assert_eq!(t.moved(1, 950.0, 300.0), [Move(950.0, 300.0)]);
        assert_eq!(t.down(2, 500.0, 500.0), [Move(500.0, 500.0), Down(500.0, 500.0)]);
    }

    #[test]
    fn cancel_releases_the_mouse() {
        let mut t = GameTouches::new(10.0);
        t.down(0, 1.0, 1.0);
        assert_eq!(t.cancel(), [Up(1.0, 1.0)]);
        assert!(!t.holding());
        assert_eq!(t.cancel(), []);
    }
}
