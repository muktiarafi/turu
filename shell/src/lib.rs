//! Turu: the shells' logic without a window (docs/NATIVE_UI_PROPOSAL.md, L1's shell part): the
//! desktop's ([`Shell`], the `desktop` feature: its clients as processes and their pictures on the
//! shell's GPU) and Android's ([`slots::SlotShell`]: the tab list over the slots, which the
//! activity carries out). They share the link's messages (`turu_frontend::shell`), and nothing
//! else. No winit, rfd or player here; the windows are fronts over this.

#[cfg(feature = "desktop")]
mod desktop;
pub mod slots;

#[cfg(feature = "desktop")]
pub use desktop::*;
