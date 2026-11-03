//! Turu: Grid View (VibeSkua's tabbed host's), every tab's game at once, each in a tile of the
//! window. The tabs stay what they are: the active one has the keyboard, sound and full frame
//! rate; the others run at [`TILE_FPS`] and, unlike hidden tabs, draw each frame. Clicking a tile
//! makes its tab the active one.

use crate::turu::TabId;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The frame rate of the tiles other than the active tab's (VibeSkua's tiles run unthrottled,
/// each in a process of its own; here every tab shares one thread).
pub const TILE_FPS: i32 = 8;

static ON: AtomicBool = AtomicBool::new(false);
/// Each tile's tab and area, in physical pixels of the window, for the UI's labels.
static TILES: Mutex<Vec<(TabId, Tile)>> = Mutex::new(Vec::new());

/// Grid View was turned on (it shows with two tabs or more).
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// The tiles shown, with their tabs; empty without Grid View.
pub fn tiles() -> Vec<(TabId, Tile)> {
    TILES.lock().map(|t| t.clone()).unwrap_or_default()
}

pub fn set_tiles(tiles: Vec<(TabId, Tile)>) {
    if let Ok(mut slot) = TILES.lock() {
        *slot = tiles;
    }
}

/// A tile's area, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Tile {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x as f64
            && y >= self.y as f64
            && x < (self.x + self.width) as f64
            && y < (self.y + self.height) as f64
    }
}

/// `count` tiles filling `width` x `height` in tab order, in rows of ⌈√count⌉ (VibeSkua's
/// layout: 2 side by side, 3-4 in 2x2, 5-6 in 3x2, 7-9 in 3x3). The last column and row take
/// the pixels left over.
pub fn layout(count: usize, width: u32, height: u32) -> Vec<Tile> {
    if count == 0 {
        return Vec::new();
    }
    let cols = (count as f64).sqrt().ceil() as u32;
    let rows = (count as u32).div_ceil(cols);
    let (cell_w, cell_h) = (width / cols, height / rows);
    (0..count as u32)
        .map(|i| {
            let (row, col) = (i / cols, i % cols);
            let (x, y) = (col * cell_w, row * cell_h);
            Tile {
                x,
                y,
                width: if col == cols - 1 { width - x } else { cell_w }.max(1),
                height: if row == rows - 1 { height - y } else { cell_h }.max(1),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_follow_vibeskuas_grid() {
        let two = layout(2, 1000, 600);
        assert_eq!(
            two,
            [
                Tile {
                    x: 0,
                    y: 0,
                    width: 500,
                    height: 600
                },
                Tile {
                    x: 500,
                    y: 0,
                    width: 500,
                    height: 600
                }
            ]
        );
        // 7 tabs: 3x3, the last row with one tile; the last column takes the odd pixel.
        let seven = layout(7, 1000, 600);
        assert_eq!(seven.len(), 7);
        assert_eq!(
            seven[2],
            Tile {
                x: 666,
                y: 0,
                width: 334,
                height: 200
            }
        );
        assert_eq!(
            seven[6],
            Tile {
                x: 0,
                y: 400,
                width: 333,
                height: 200
            }
        );
        assert!(seven[4].contains(500.0, 300.0) && !seven[4].contains(700.0, 300.0));
    }
}
