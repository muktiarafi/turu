//! Turu's frontend pieces that don't depend on the window: what the desktop app (`app/`) and
//! other frontends share. The module names are the app's (`crate::turu`, `crate::grid`, ...), so
//! the app re-exports them at its root.
//!
//! See docs/ARCHITECTURE.md.

#[cfg(feature = "ruffle")]
pub mod capture;
pub mod crash_reports;
#[cfg(feature = "ruffle")]
pub mod engine;
pub mod grid;
pub mod gui;
pub mod map_items;
pub mod map_pads;
pub mod pacing;
pub mod platform;
#[cfg(feature = "ruffle")]
pub mod players;
pub mod process_memory;
pub mod touch;
pub mod touch_controls;
pub mod turu;
