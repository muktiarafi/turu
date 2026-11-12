//! Crash reports sent to Turu's developers through Sentry, once the user said yes (the question the
//! windows ask once, and Options > Application's switch: `sendCrashReports` in `settings.json`,
//! through `Platform`). Sentry itself is the extension's Kotlin (`CrashUploads.kt`, with
//! `io.sentry:sentry-android`), as in Turu's Android app: it catches Java and native crashes (the
//! engine's and AIR's runtime's), ANRs and tombstones, and sends the exits Android records. This is
//! the engine's side: whether the build can send them, the user's answer, and a panic's message.
//!
//! A release build aborts on a panic: the panic hook hands its message over first ([`panicked`]),
//! and Sentry's native handler then reports the abort with it, as one event.

use crate::host::BRIDGE;
use jni::objects::JValue;
use jni::{Env, jni_sig, jni_str};
use std::sync::OnceLock;

/// Whether this build can send crash reports (the extension was built with a DSN).
pub(crate) fn available() -> bool {
    // The build's DSN doesn't change; asked once the extension is up.
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    if let Some(available) = AVAILABLE.get() {
        return *available;
    }
    let Some((vm, class)) = BRIDGE.get() else {
        return false;
    };
    let asked = crate::android::attach(vm, |env: &mut Env| {
        env.call_static_method(
            class,
            jni_str!("crashUploadsAvailable"),
            jni_sig!("()Z"),
            &[],
        )?
        .z()
    });
    match asked {
        Ok(available) => *AVAILABLE.get_or_init(|| available),
        Err(e) => {
            tracing::warn!("Couldn't ask whether crash reports can be sent: {e}");
            false
        }
    }
}

/// The user's answer (Options > Application, or the question): applied at once.
pub(crate) fn set_enabled(on: bool) {
    let Some((vm, class)) = BRIDGE.get() else {
        return;
    };
    let set = crate::android::attach(vm, |env: &mut Env| -> jni::errors::Result<()> {
        env.call_static_method(
            class,
            jni_str!("setCrashUploads"),
            jni_sig!("(Z)V"),
            &[JValue::Bool(on)],
        )?;
        Ok(())
    });
    if let Err(e) = set {
        tracing::warn!(
            "Couldn't turn crash reports {}: {e}",
            if on { "on" } else { "off" }
        );
    }
}

/// From the panic hook, before a release build's abort: the panic's message goes with the native
/// crash Sentry reports.
pub(crate) fn panicked(message: &str) {
    let Some((vm, class)) = BRIDGE.get() else {
        return;
    };
    let _ = crate::android::attach(vm, |env: &mut Env| -> jni::errors::Result<()> {
        let message = env.new_string(message)?;
        env.call_static_method(
            class,
            jni_str!("panicked"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&message)],
        )?;
        Ok(())
    });
}
