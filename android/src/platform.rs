//! What the bot windows ask of Android (`turu_frontend::platform`). There are no file dialogs
//! yet (the system picker answers through the activity, which needs Java of ours): the windows
//! hide Load Script and Save, and scripts come from Search Scripts. Web pages open in the
//! browser; folders and files only get logged, so Edit Script and Open VSCode are hidden too.
//! Logs and crash reports go out through the share sheet (Share), and Copy (egui's
//! `OutputCommand::CopyText`, passed on by `screen.rs`) goes to Android's clipboard.
//! Notify Drop's beep plays on an AAudio stream of its own (`audio::beep`). Frame interpolation
//! (Options > Application) is saved in the data folder.

use android_activity::AndroidApp;
use jni::JavaVM;
use jni::objects::{JObject, JValue};
use jni::sys::jobject;
use jni::{jni_sig, jni_str};
use std::path::Path;
use turu_frontend::platform::{Answer, FileDialog, Platform};

pub struct AndroidPlatform;

impl Platform for AndroidPlatform {
    fn pick_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        tracing::info!("No file dialogs on Android yet: Search Scripts downloads and loads scripts");
        false
    }

    fn save_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        tracing::info!("No file dialogs on Android yet");
        false
    }

    fn open_url(&self, url: &str) {
        match crate::app() {
            Some(app) => {
                if let Err(e) = open_in_browser(&app, url) {
                    tracing::warn!("Couldn't open {url}: {e}");
                }
            }
            None => tracing::info!("Not opened without the app on screen: {url}"),
        }
    }

    fn open_folder(&self, folder: &Path) {
        tracing::info!("The folder is {} (on the phone's storage)", folder.display());
    }

    fn open_file(&self, path: &Path) {
        tracing::info!("The file is {} (on the phone's storage)", path.display());
    }

    fn has_file_dialogs(&self) -> bool {
        false
    }

    fn opens_files(&self) -> bool {
        false
    }

    fn is_touch(&self) -> bool {
        true
    }

    fn set_frame_interpolation(&self, on: bool) {
        ruffle_core::frame_interpolation::set_enabled(on);
        crate::save_interpolation(on);
        tracing::info!("Frame interpolation turned {}", if on { "on" } else { "off" });
    }

    // `input::key` maps no function keys and sends egui no modifiers: no hotkeys to bind.
    fn has_hotkeys(&self) -> bool {
        false
    }

    // The app's executable is Android's app_process, not a `turu` Army could start.
    fn launches_processes(&self) -> bool {
        false
    }

    // No tabs yet: Army and Butler need several clients.
    fn has_army(&self) -> bool {
        false
    }

    fn shares_text(&self) -> bool {
        true
    }

    fn uploads_crash_reports(&self) -> bool {
        // The build's DSN doesn't change; asked once the service has started the engine.
        static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if let Some(available) = AVAILABLE.get() {
            return *available;
        }
        let Some(available) = crate::notify::with_service(|env, class| {
            env.call_static_method(class, jni_str!("crashUploadsAvailable"), jni_sig!("()Z"), &[])?
                .z()
        }) else {
            return false;
        };
        *AVAILABLE.get_or_init(|| available)
    }

    fn set_crash_uploads(&self, on: bool) {
        crate::notify::with_service(|env, class| {
            env.call_static_method(
                class,
                jni_str!("setCrashUploads"),
                jni_sig!("(Z)V"),
                &[JValue::Bool(on)],
            )?;
            Ok(())
        });
    }

    fn beep(&self) {
        crate::audio::beep();
    }

    fn share_text(&self, subject: &str, text: &str) {
        match crate::app() {
            Some(app) => {
                if let Err(e) = share(&app, subject, text) {
                    tracing::warn!("Couldn't open the share sheet: {e}");
                }
            }
            None => tracing::info!("Not shared without the app on screen: {subject}"),
        }
    }
}

/// What a clip or a share holds at most (Android's binder limit is ~1 MB a transaction).
const MAX_TEXT_BYTES: usize = 400_000;

/// Puts `text` on Android's clipboard (egui's Copy).
pub fn copy_text(text: &str) {
    let Some(app) = crate::app() else {
        return;
    };
    let text = turu_frontend::crash_reports::shareable(text, MAX_TEXT_BYTES);
    if let Err(e) = copy(&app, &text) {
        tracing::warn!("Couldn't copy to the clipboard: {e}");
    }
}

/// Runs `work` with the activity, on a thread attached to the JavaVM.
fn with_activity<T>(
    app: &AndroidApp,
    work: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<T>,
) -> jni::errors::Result<T> {
    // SAFETY: android-activity hands out the process's JavaVM.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    // A global reference: it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;
    vm.attach_current_thread(|env| {
        // SAFETY: the activity outlives this call.
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
        work(env, &activity)
    })
}

/// `startActivity(Intent.createChooser(new Intent(ACTION_SEND) with the subject and text, null))`.
fn share(app: &AndroidApp, subject: &str, text: &str) -> jni::errors::Result<()> {
    let text = turu_frontend::crash_reports::shareable(text, MAX_TEXT_BYTES);
    with_activity(app, |env, activity| {
        let action = env.new_string("android.intent.action.SEND")?;
        let send = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&action)],
        )?;
        let mime = env.new_string("text/plain")?;
        env.call_method(
            &send,
            jni_str!("setType"),
            jni_sig!("(Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&mime)],
        )?;
        for (key, value) in [
            ("android.intent.extra.SUBJECT", subject),
            ("android.intent.extra.TEXT", text.as_str()),
        ] {
            let key = env.new_string(key)?;
            let value = env.new_string(value)?;
            env.call_method(
                &send,
                jni_str!("putExtra"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
                &[JValue::Object(&key), JValue::Object(&value)],
            )?;
        }
        let chooser = env
            .call_static_method(
                jni_str!("android/content/Intent"),
                jni_str!("createChooser"),
                jni_sig!("(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;"),
                &[JValue::Object(&send), JValue::Object(&JObject::null())],
            )?
            .l()?;
        env.call_method(
            activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&chooser)],
        )?;
        Ok(())
    })
}

/// `getSystemService("clipboard").setPrimaryClip(ClipData.newPlainText("Turu", text))`.
fn copy(app: &AndroidApp, text: &str) -> jni::errors::Result<()> {
    with_activity(app, |env, activity| {
        let name = env.new_string("clipboard")?;
        let clipboard = env
            .call_method(
                activity,
                jni_str!("getSystemService"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[JValue::Object(&name)],
            )?
            .l()?;
        let label = env.new_string("Turu")?;
        let text = env.new_string(text)?;
        let clip = env
            .call_static_method(
                jni_str!("android/content/ClipData"),
                jni_str!("newPlainText"),
                jni_sig!("(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"),
                &[JValue::Object(&label), JValue::Object(&text)],
            )?
            .l()?;
        env.call_method(
            &clipboard,
            jni_str!("setPrimaryClip"),
            jni_sig!("(Landroid/content/ClipData;)V"),
            &[JValue::Object(&clip)],
        )?;
        Ok(())
    })
}

/// `activity.startActivity(new Intent(ACTION_VIEW, Uri.parse(url)))`.
fn open_in_browser(app: &AndroidApp, url: &str) -> jni::errors::Result<()> {
    with_activity(app, |env, activity| {
        let uri_text = env.new_string(url)?;
        let uri = env
            .call_static_method(
                jni_str!("android/net/Uri"),
                jni_str!("parse"),
                jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                &[JValue::Object(&uri_text)],
            )?
            .l()?;
        let action = env.new_string("android.intent.action.VIEW")?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
            &[JValue::Object(&action), JValue::Object(&uri)],
        )?;
        env.call_method(
            activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&intent)],
        )?;
        Ok(())
    })
}
