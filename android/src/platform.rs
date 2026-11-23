//! What the bot windows ask of Android (`turu_frontend::platform`). File dialogs are the system's
//! picker (`saf.rs`), answering with a path as on a desktop. Web pages open in the browser;
//! folders and files only get logged, so Edit Script and Open VSCode are hidden.
//! Logs and crash reports go out through the share sheet (Share), and Copy (egui's
//! `OutputCommand::CopyText`, passed on by `screen.rs`) goes to Android's clipboard.
//! Notify Drop's drops and stopped scripts are notifications too (`notify::alert`).
//! An update's APK goes to Android's installer (`TuruActivity.installPackage`).
//! Notify Drop's beep plays on an AAudio stream of its own (`audio::beep`). Frame interpolation
//! (Options > Application) is saved in the data folder.

use crate::java::{TuruActivity, TuruService};
use android_activity::AndroidApp;
use jni::EnvUnowned;
use jni::JavaVM;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jint, jobject};
use std::path::Path;
use turu_frontend::crash_reports::{SHARE_BYTES, shareable};
use turu_frontend::platform::{Alert, Answer, FileDialog, Platform, Question};
use turu_frontend::shell::ToShell;

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
            // A process tab: the shell's activity opens it.
            None if crate::slots::slot().is_some() => {
                crate::slots::to_shell(ToShell::OpenUrl {
                    url: url.to_owned(),
                });
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

    // The graphics API, from each slot's next start (`--graphics` in turu-args.txt wins).
    fn graphics_choices(&self) -> Option<(Vec<(&'static str, &'static str)>, &'static str)> {
        let choices = vec![("", "Automatic"), ("gl", "OpenGL ES"), ("vulkan", "Vulkan")];
        let saved = match crate::saved_graphics() {
            crate::screen::Graphics::Auto => "",
            graphics => graphics.arg(),
        };
        Some((choices, saved))
    }

    fn set_graphics(&self, value: &str) {
        let graphics =
            crate::screen::Graphics::from_arg(value).unwrap_or(crate::screen::Graphics::Auto);
        crate::save_graphics(graphics);
        tracing::info!("Graphics: {} from each tab's next start", graphics.arg());
    }

    // Safe Drawing, from each slot's next start (`--safe-drawing` in turu-args.txt too).
    fn safe_drawing(&self) -> Option<bool> {
        let root = turu_bot::paths::Paths::new().root;
        Some(crate::saved_safe_drawing(&root))
    }

    fn set_safe_drawing(&self, on: bool) {
        crate::save_safe_drawing(on);
        tracing::info!(
            "Safe Drawing turned {} from each tab's next start",
            if on { "on" } else { "off" }
        );
    }

    fn set_frame_interpolation(&self, on: bool) {
        ruffle_core::frame_interpolation::set_enabled(on);
        crate::save_interpolation(on);
        tracing::info!(
            "Frame interpolation turned {}",
            if on { "on" } else { "off" }
        );
    }

    // Mugetsu 2.0.0's Performance mode, in Options > Application as on a desktop's tab, and what
    // the graphics tips offer (`turu_frontend::graphics_tips`).
    fn performance(&self) -> Option<u8> {
        Some(crate::performance())
    }

    fn set_performance(&self, level: u8) {
        crate::set_performance(level);
        tracing::info!(
            "Performance mode: {level} (the game is drawn at {:.0} % of its width and height)",
            crate::render_scale() * 100.0
        );
    }

    fn render_scale(&self) -> f64 {
        crate::render_scale()
    }

    // Mugetsu 2.0.0's other options that fit a phone (as a desktop tab's Options has them).
    // Frame Pacing and the FPS limit don't: Android's compositor shows every picture at the
    // display's rate, so uncapped (for G-Sync and FreeSync monitors) has nothing to do here.
    fn show_fps(&self) -> Option<bool> {
        Some(crate::show_fps())
    }

    fn set_show_fps(&self, on: bool) {
        crate::set_show_fps(on);
    }

    fn memory_limit_mb(&self) -> Option<u32> {
        Some(crate::memory_limit_mb())
    }

    fn set_memory_limit_mb(&self, mb: u32) {
        crate::set_memory_limit_mb(mb);
        tracing::info!(
            "Memory limit: {}",
            if mb == 0 {
                "off".to_string()
            } else {
                format!("{mb} MB")
            }
        );
    }

    fn memory_now_mb(&self) -> Option<u64> {
        turu_frontend::process_memory::Reading::now().map(|reading| reading.rss >> 20)
    }

    // On the engine thread, which has the player (Options runs there too, inside a draw).
    fn free_memory(&self) {
        crate::run_on_engine(crate::memory::free_now);
    }

    // The app's executable is Android's app_process, not a `turu` Army could start.
    fn launches_processes(&self) -> bool {
        false
    }

    // A phone's memory holds two or three clients (Options > Application > Tab Limit).
    fn limits_tabs(&self) -> bool {
        true
    }

    // A tab in each of the manifest's slots: Tab Limit (4 by default) keeps it to fewer.
    fn tab_max(&self) -> i32 {
        turu_frontend::slots::POOL as i32
    }

    // A phone's screen is too small for the tiles (the engine ignores `TabRequest::Grid`).
    fn has_grid_view(&self) -> bool {
        false
    }

    // No Options > HotKeys: a phone has no keyboard to bind, and the window only took room.
    fn has_hotkeys(&self) -> bool {
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
        let Some(available) =
            crate::notify::with_service(|env| TuruService::crash_uploads_available(env))
        else {
            return false;
        };
        *AVAILABLE.get_or_init(|| available)
    }

    fn set_crash_uploads(&self, on: bool) {
        crate::notify::with_service(|env| {
            TuruService::set_crash_uploads(env, on)?;
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
            activity
                .ask_crash_uploads(env, &title, &message, &note, &link, &link_url, &no, &yes)?;
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
            None if crate::slots::slot().is_some() => crate::slots::to_shell(ToShell::Share {
                subject: subject.to_owned(),
                text: shareable(text, SHARE_BYTES),
            }),
            None => tracing::info!("Not shared without the app on screen: {subject}"),
        }
    }

    fn posts_alerts(&self) -> bool {
        true
    }

    fn alert(&self, kind: Alert, title: &str, text: &str) {
        crate::notify::alert(kind, title, text);
    }

    fn installs_packages(&self) -> bool {
        true
    }

    /// `TuruActivity.installPackage(path)`: Android's installer (`PackageUpdate.kt`).
    fn install_package(&self, path: &Path) -> Result<(), String> {
        // A process tab: the shell's activity installs it, and says how it went (`Installed`).
        if crate::app().is_none() && crate::slots::slot().is_some() {
            crate::slots::to_shell(ToShell::Install {
                path: path.to_string_lossy().into_owned(),
            });
            return Ok(());
        }
        let app = crate::app().ok_or("open Turu to install the update")?;
        let error = with_activity(&app, |env, activity| {
            let path = env.new_string(path.to_string_lossy())?;
            let error = activity.install_package(env, &path)?;
            if error.is_null() {
                return Ok(None);
            }
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
    std::thread::spawn(move || turu_egui::gui::turu::answer_crash_upload_question(answer));
}

/// Puts `text` on Android's clipboard (egui's Copy).
pub fn copy_text(text: &str) {
    let text = shareable(text, SHARE_BYTES);
    let Some(app) = crate::app() else {
        // A process tab's slot uses the clipboard itself (Android lets any of the app's processes
        // while the app has the focus).
        crate::slots::copy_text(&text);
        return;
    };
    if let Err(e) = copy(&app, &text) {
        tracing::warn!("Couldn't copy to the clipboard: {e}");
    }
}

/// The text on Android's clipboard, if it holds some (the game's Ctrl+V).
pub fn clip_text() -> Option<String> {
    let Some(app) = crate::app() else {
        return crate::slots::clip_text().filter(|text| !text.is_empty());
    };
    paste(&app)
        .inspect_err(|e| tracing::warn!("Couldn't read the clipboard: {e}"))
        .ok()
        .flatten()
        .filter(|text| !text.is_empty())
}

/// Runs `work` with the activity (its methods bound in `java.rs`), on a thread attached to the
/// JavaVM. Only after [`crate::java::check`]: the cast looks the class up.
pub(crate) fn with_activity<T>(
    app: &AndroidApp,
    work: impl FnOnce(&mut jni::Env, &TuruActivity) -> jni::errors::Result<T>,
) -> jni::errors::Result<T> {
    // SAFETY: android-activity hands out the process's JavaVM.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    // A global reference: it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;
    crate::attach(&vm, |env| {
        // SAFETY: the activity outlives this call.
        let activity = unsafe { env.as_cast_raw::<TuruActivity>(&activity_raw) }?;
        work(env, &activity)
    })
}

/// Runs `work` with the activity as a plain object, on a thread attached to the JavaVM.
pub(crate) fn with_activity_object<T>(
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
    let text = shareable(text, SHARE_BYTES);
    with_activity(app, |env, activity| {
        let subject = env.new_string(subject)?;
        let text = env.new_string(&text)?;
        activity.share_text(env, &subject, &text)?;
        Ok(())
    })
}

/// `TuruActivity.clipText()`: the clipboard's text, or `None` for an empty clipboard.
fn paste(app: &AndroidApp) -> jni::errors::Result<Option<String>> {
    with_activity(app, |env, activity| {
        let text = activity.clip_text(env)?;
        if text.is_null() {
            return Ok(None);
        }
        Ok(Some(text.try_to_string(env)?))
    })
}

/// `TuruActivity.copyText(text)`: puts `text` on the clipboard.
fn copy(app: &AndroidApp, text: &str) -> jni::errors::Result<()> {
    with_activity(app, |env, activity| {
        let text = env.new_string(text)?;
        activity.copy_text(env, &text)?;
        Ok(())
    })
}

/// `TuruActivity.openUrl(url)`: the browser, or the app for the link.
fn open_in_browser(app: &AndroidApp, url: &str) -> jni::errors::Result<()> {
    with_activity(app, |env, activity| {
        let url = env.new_string(url)?;
        activity.open_url(env, &url)?;
        Ok(())
    })
}
