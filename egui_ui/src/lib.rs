//! Turu's egui UI (docs/NATIVE_UI_PROPOSAL.md, L1's client part): the bot windows (`gui::turu`,
//! VibeSkua's layout) and a client's screen (`client_screen`). What a window shows and does is
//! `turu_frontend`'s (the tabs, the view models, the link); this crate lays it out and draws it.
//!
//! `turu_frontend`'s modules are re-exported at the root, so the windows reach them as they did
//! when they lived in that crate (`crate::turu`, `crate::models`, ...).

#[cfg(feature = "ruffle")]
pub mod client_screen;
pub mod gui;

pub use turu_frontend::{crash_reports, graphics_tips, grid, host, models, shell, turu};
#[cfg(feature = "ruffle")]
pub use turu_frontend::{engine, movie_texture, pacing};
