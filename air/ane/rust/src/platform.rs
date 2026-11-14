//! What the bot windows can do under AIR (`turu_frontend::platform::Platform`): links and the share
//! sheet through Kotlin's `Bridge`; file dialogs through Android's system picker (`saf.rs`); tabs
//! (game clients sharing AIR's one stage: `clients`), Army and Butler in tabs, under the Tab
//! Limit, without Grid View. An update's APK goes to Android's installer (`Bridge.installPackage`).

use crate::host::BRIDGE;
use jni::objects::{JString, JValue};
use jni::{Env, jni_sig, jni_str};
use std::path::Path;
use turu_frontend::platform::{Answer, FileDialog, Platform, Question};

pub(crate) struct AirPlatform;

impl Platform for AirPlatform {
    fn pick_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        crate::saf::pick_file(&dialog, answer)
    }

    fn save_file(&self, dialog: FileDialog, answer: Answer) -> bool {
        crate::saf::save_file(&dialog, answer)
    }

    fn open_url(&self, url: &str) {
        let Some((vm, class)) = BRIDGE.get() else {
            return;
        };
        let opened = crate::android::attach(vm, |env: &mut Env| -> jni::errors::Result<()> {
            let url = env.new_string(url)?;
            env.call_static_method(
                class,
                jni_str!("openUrl"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&url)],
            )?;
            Ok(())
        });
        if let Err(e) = opened {
            tracing::warn!("Couldn't open {url}: {e}");
        }
    }

    fn open_folder(&self, folder: &Path) {
        tracing::info!("Folder: {}", folder.display());
    }

    fn open_file(&self, path: &Path) {
        tracing::info!("File: {}", path.display());
    }

    fn opens_files(&self) -> bool {
        false
    }

    fn launches_processes(&self) -> bool {
        false
    }

    fn has_army(&self) -> bool {
        true
    }

    fn limits_tabs(&self) -> bool {
        true
    }

    // A phone's screen is too small for the tiles (`clients` ignores `TabRequest::Grid`).
    fn has_grid_view(&self) -> bool {
        false
    }

    fn is_touch(&self) -> bool {
        true
    }

    fn has_hotkeys(&self) -> bool {
        false
    }

    fn uploads_crash_reports(&self) -> bool {
        crate::crash_uploads::available()
    }

    fn set_crash_uploads(&self, on: bool) {
        crate::crash_uploads::set_enabled(on);
    }

    fn ask_crash_uploads(&self, question: &Question) -> bool {
        let Some((vm, class)) = BRIDGE.get() else {
            return false;
        };
        let asked = crate::android::attach(vm, |env: &mut Env| -> jni::errors::Result<bool> {
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
            env.call_static_method(
                class,
                jni_str!("askCrashUploads"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z"),
                &[
                    JValue::Object(&title),
                    JValue::Object(&message),
                    JValue::Object(&note),
                    JValue::Object(&link),
                    JValue::Object(&link_url),
                    JValue::Object(&no),
                    JValue::Object(&yes),
                ],
            )?
            .z()
        });
        asked
            .inspect_err(|e| tracing::warn!("Couldn't ask about crash reports: {e}"))
            .unwrap_or(false)
    }

    fn shares_text(&self) -> bool {
        true
    }

    fn share_text(&self, subject: &str, text: &str) {
        let Some((vm, class)) = BRIDGE.get() else {
            return;
        };
        let _ = crate::android::attach(vm, |env: &mut Env| -> jni::errors::Result<()> {
            let subject = env.new_string(subject)?;
            let text = env.new_string(text)?;
            env.call_static_method(
                class,
                jni_str!("shareText"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
                &[JValue::Object(&subject), JValue::Object(&text)],
            )?;
            Ok(())
        });
    }

    fn installs_packages(&self) -> bool {
        true
    }

    /// `Bridge.installPackage(path)`: Android's installer (`PackageUpdate.kt`).
    fn install_package(&self, path: &Path) -> Result<(), String> {
        let (vm, class) = BRIDGE.get().ok_or("the extension isn't started")?;
        let error =
            crate::android::attach(vm, |env: &mut Env| -> jni::errors::Result<Option<String>> {
                let path = env.new_string(path.to_string_lossy())?;
                let error = env
                    .call_static_method(
                        class,
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
