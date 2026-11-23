//! Turu's frontend pieces that don't depend on the window: what the desktop app (`app/`) and
//! other frontends share. The module names are the app's (`crate::turu`, `crate::grid`, ...), so
//! the app re-exports them at its root.
//!
//! See docs/ARCHITECTURE.md.

#[cfg(feature = "gpu")]
pub mod blit;
#[cfg(feature = "ruffle")]
pub mod capture;
pub mod client;
pub mod client_link;
pub mod crash_reports;
pub mod documents;
#[cfg(feature = "ruffle")]
pub mod engine;
pub mod game_files;
#[cfg(feature = "gpu")]
pub mod gpu_waiter;
pub mod graphics_tips;
pub mod grid;
pub mod layout;
pub mod map_items;
pub mod map_pads;
pub mod memory_kills;
pub mod models;
#[cfg(feature = "ruffle")]
pub mod movie_texture;
pub mod native_shell;
// Mugetsu's code (GPL-3.0, its author's): only for Ruffle, and kept out of Turu Mas
// (docs/TURU_MAS.md, "Licenses").
pub mod host;
#[cfg(feature = "ruffle")]
pub mod pacing;
#[cfg(feature = "ruffle")]
pub mod players;
pub mod process_memory;
pub mod saved_tabs;
pub mod shell;
pub mod slots;
pub mod tabs_log;
pub mod touch;
pub mod turu;
