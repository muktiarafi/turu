//! Turu's engine with the game in Adobe AIR (docs/AIR_PROPOSAL.md), loaded by the AIR native
//! extension (`air/ane`, Kotlin's `Bridge`):
//!
//! * `nativeStart` starts the engine once per process: logging, TLS, the data folder, the flags
//!   in `turu-args.txt` (as the Ruffle build's), the control API and the web monitor.
//! * [`host::AirFlashHost`] lets bot threads call `skua.swf`'s callbacks: each call goes to AIR's
//!   thread as a status event, and the answer comes back through `nativeReply`.
//! * `nativeFlashCall` receives `skua.swf`'s `ExternalInterface.call(...)`s (`TuruExternal` in
//!   the host SWF), on AIR's thread.
//! * `nativePatch` rewrites a SWF's bytecode for AIR (`turu_air_patch`): skua.swf, and the
//!   game's client as the live one is fetched.

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod host;
