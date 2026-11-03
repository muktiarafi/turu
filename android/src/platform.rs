//! What the bot windows ask of Android (`turu_frontend::platform`). There are no file dialogs
//! yet (the system picker answers through the activity, which needs Java of ours): Load Script
//! and the other dialogs say so, and scripts come from Search Scripts. Web pages open in the
//! browser; folders and files only get logged.

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
                if let Err(e) = open_in_browser(app, url) {
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
}

/// `activity.startActivity(new Intent(ACTION_VIEW, Uri.parse(url)))`.
fn open_in_browser(app: &AndroidApp, url: &str) -> jni::errors::Result<()> {
    // SAFETY: android-activity hands out the process's JavaVM.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    // A global reference: it can only be borrowed, not wrapped as a local one.
    let activity_raw = app.activity_as_ptr() as jobject;
    vm.attach_current_thread(|env| {
        // SAFETY: the activity outlives this call.
        let activity = unsafe { env.as_cast_raw::<JObject>(&activity_raw) }?;
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
            &*activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&intent)],
        )?;
        Ok(())
    })
}
