//! Turu's engine with the game in Adobe AIR (docs/AIR_PROPOSAL.md), loaded by the AIR native
//! extension (`air/ane`, Kotlin's `Bridge`):
//!
//! * `nativeStart` starts the engine once per process: logging, TLS, the data folder, the flags
//!   in `turu-args.txt` (as the Ruffle build's), the control API and the web monitor.
//! * [`host::AirFlashHost`] lets bot threads call `turu.swf`'s callbacks: each call goes to AIR's
//!   thread as a status event, and the answer comes back through `nativeReply`.
//! * `nativeFlashCall` receives `turu.swf`'s `ExternalInterface.call(...)`s (`TuruExternal` in
//!   the host SWF), on AIR's thread, for the tab whose client made them.
//! * [`clients`] runs the tabs: one game client each, all on AIR's one stage.
//! * [`overlay`] draws Turu's nav rail and bot windows over the game (`nativeSurface`,
//!   `nativeTouch`, `nativeOnUi`, `nativeText`).
//! * `nativePatch` rewrites a SWF's bytecode for AIR (`turu_air_patch`): the game's client as the
//!   live one is fetched, and the SWFs it loads.

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod clients;
#[cfg(target_os = "android")]
mod crash_uploads;
#[cfg(target_os = "android")]
mod host;
#[cfg(target_os = "android")]
mod notify;
#[cfg(target_os = "android")]
mod overlay;
#[cfg(target_os = "android")]
mod passwords;
#[cfg(target_os = "android")]
mod platform;
#[cfg(target_os = "android")]
mod saf;
