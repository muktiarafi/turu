//! What the bot windows ask of Android (`turu_frontend::platform`). File dialogs are the system's
//! picker (`saf.rs`), answering with a path as on a desktop. Web pages open in the browser;
//! folders and files only get logged, so Edit Script and Open VSCode are hidden.
//! Logs and crash reports go out through the share sheet (Share), and Copy (egui's
//! `OutputCommand::CopyText`, passed on by `screen.rs`) goes to Android's clipboard.
//! An update's APK goes to Android's installer (`TuruActivity.installPackage`).
//! Notify Drop's beep plays on an AAudio stream of its own (`audio::beep`). Frame interpolation
//! (Options > Application) is saved in the data folder.

use android_activity::AndroidApp;
use jni::JavaVM;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::{jint, jobject};
use jni::{EnvUnowned, jni_sig, jni_str};
use std::path::Path;
use turu_frontend::platform::{Answer, FileDialog, Platform, Question};

pub struct AndroidPlatform;

impl Platform for AndroidPlatform {
    fn pick_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        crate::saf::pick_file(&dialog, answer)
    }

    fn save_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        crate::saf::save_file(&dialog, answer)
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
        tracing::info!(
            "The folder is {} (on the phone's storage)",
            folder.display()
        );
    }

    fn open_file(&self, path: &Path) {
        tracing::info!("The file is {} (on the phone's storage)", path.display());
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
        tracing::info!(
            "Frame interpolation turned {}",
            if on { "on" } else { "off" }
        );
    }

    fn safe_drawing(&self) -> Option<bool> {
        Some(
            turu_bot::paths::Paths::new()
                .root
                .join(crate::SAFE_DRAWING_FILE)
                .exists(),
        )
    }

    fn set_safe_drawing(&self, on: bool) {
        crate::save_switch(crate::SAFE_DRAWING_FILE, on);
        tracing::info!(
            "Safe Drawing turned {} for the next start",
            if on { "on" } else { "off" }
        );
    }

    // The app's executable is Android's app_process, not a `turu` Army could start.
    fn launches_processes(&self) -> bool {
        false
    }

    // A phone's memory holds two or three clients (Options > Application > Tab Limit).
    fn limits_tabs(&self) -> bool {
        true
    }

    // A phone's screen is too small for the tiles (the engine ignores `TabRequest::Grid`).
    fn has_grid_view(&self) -> bool {
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
            env.call_static_method(
                class,
                jni_str!("crashUploadsAvailable"),
                jni_sig!("()Z"),
                &[],
            )?
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

    fn ask_crash_uploads(&self, question: &Question) -> bool {
        let Some(app) = crate::app() else {
            return false;
        };
        let asked = with_activity(&app, |env, activity| {
            let strings = [
                &question.title,
                &question.message,
                &question.note,
                &question.link,
                &question.link_url,
                &question.no,
                &question.yes,
            ]
            .map(|text| env.new_string(text));
            let [title, message, note, link, link_url, no, yes] = strings;
            let (title, message, note, link, link_url, no, yes) =
                (title?, message?, note?, link?, link_url?, no?, yes?);
            env.call_method(
                activity,
                jni_str!("askCrashUploads"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V"),
                &[
                    JValue::Object(&title),
                    JValue::Object(&message),
                    JValue::Object(&note),
                    JValue::Object(&link),
                    JValue::Object(&link_url),
                    JValue::Object(&no),
                    JValue::Object(&yes),
                ],
            )?;
            Ok(())
        });
        asked
            .inspect_err(|e| tracing::warn!("Couldn't ask about crash reports: {e}"))
            .is_ok()
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

    fn installs_packages(&self) -> bool {
        true
    }

    /// `TuruActivity.installPackage(path)`: Android's installer (`PackageUpdate.kt`).
    fn install_package(&self, path: &Path) -> Result<(), String> {
        let app = crate::app().ok_or("open Turu to install the update")?;
        let error = with_activity(&app, |env, activity| {
            let path = env.new_string(path.to_string_lossy())?;
            let error = env
                .call_method(
                    activity,
                    jni_str!("installPackage"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[JValue::Object(&path)],
                )?
                .l()?;
            if error.is_null() {
                return Ok(None);
            }
            let error = env.cast_local::<JString>(error)?;
            Ok(Some(error.try_to_string(env)?))
        })
        .map_err(|e| e.to_string())?;
        error.map_or(Ok(()), Err)
    }
}

/// `TuruActivity.nativePackageInstalled(status, message)`, on the UI thread: how Android's
/// installer ended the update's install (`PackageUpdate`: 0 installed, 1 cancelled, 2 failed).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativePackageInstalled<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    status: jint,
    message: JString<'local>,
) {
    let message = unowned_env
        .with_env(|env| -> jni::errors::Result<Option<String>> {
            if message.is_null() {
                return Ok(None);
            }
            Ok(Some(message.try_to_string(env)?))
        })
        .resolve::<jni::errors::LogErrorAndDefault>();
    turu_frontend::turu::package_install_finished(status, message);
}

/// `TuruActivity.nativeCrashUploadsAnswer(answer)`, on the UI thread: "Send crash reports?" was
/// answered yes (1) or no (0), or closed without one (-1).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeCrashUploadsAnswer<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    answer: jint,
) {
    let answer = (answer >= 0).then_some(answer == 1);
    std::thread::spawn(move || turu_frontend::gui::turu::answer_crash_upload_question(answer));
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

/// The text on Android's clipboard, if it holds some (the game's Ctrl+V).
pub fn clip_text() -> Option<String> {
    let app = crate::app()?;
    paste(&app)
        .inspect_err(|e| tracing::warn!("Couldn't read the clipboard: {e}"))
        .ok()
        .flatten()
        .filter(|text| !text.is_empty())
}

/// Runs `work` with the activity, on a thread attached to the JavaVM.
pub(crate) fn with_activity<T>(
    app: &AndroidApp,
    work: impl FnOnce(&mut jni::Env, &JObject) -> jni::errors::Result<T>,
) -> jni::errors::Result<T> {
    // SAFETY: android-activity hands out the process's JavaVM.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    // A global reference: it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;
    crate::attach(&vm, |env| {
        // SAFETY: the activity outlives this call.
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
        work(env, &activity)
    })
}

/// `TuruActivity.shareText(subject, text)`: the share sheet.
fn share(app: &AndroidApp, subject: &str, text: &str) -> jni::errors::Result<()> {
    let text = turu_frontend::crash_reports::shareable(text, MAX_TEXT_BYTES);
    with_activity(app, |env, activity| {
        let subject = env.new_string(subject)?;
        let text = env.new_string(&text)?;
        env.call_method(
            activity,
            jni_str!("shareText"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
            &[JValue::Object(&subject), JValue::Object(&text)],
        )?;
        Ok(())
    })
}

/// `TuruActivity.clipText()`: the clipboard's text, or `None` for an empty clipboard.
fn paste(app: &AndroidApp) -> jni::errors::Result<Option<String>> {
    with_activity(app, |env, activity| {
        let text = env
            .call_method(
                activity,
                jni_str!("clipText"),
                jni_sig!("()Ljava/lang/String;"),
                &[],
            )?
            .l()?;
        if text.is_null() {
            return Ok(None);
        }
        let text = env.cast_local::<JString>(text)?;
        Ok(Some(text.try_to_string(env)?))
    })
}

/// `TuruActivity.copyText(text)`: puts `text` on the clipboard.
fn copy(app: &AndroidApp, text: &str) -> jni::errors::Result<()> {
    with_activity(app, |env, activity| {
        let text = env.new_string(text)?;
        env.call_method(
            activity,
            jni_str!("copyText"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&text)],
        )?;
        Ok(())
    })
}

/// `TuruActivity.openUrl(url)`: the browser, or the app for the link.
fn open_in_browser(app: &AndroidApp, url: &str) -> jni::errors::Result<()> {
    with_activity(app, |env, activity| {
        let url = env.new_string(url)?;
        env.call_method(
            activity,
            jni_str!("openUrl"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&url)],
        )?;
        Ok(())
    })
}
