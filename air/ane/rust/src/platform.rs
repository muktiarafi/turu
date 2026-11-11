//! What the bot windows can do under AIR (`turu_frontend::platform::Platform`): links and the share
//! sheet through Kotlin's `Bridge`; one game client (AIR runs one root SWF), so no Army or tabs;
//! no file dialogs yet (scripts come from Search Scripts).

use crate::host::BRIDGE;
use jni::objects::JValue;
use jni::{Env, jni_sig, jni_str};
use std::path::Path;
use turu_frontend::platform::{Answer, FileDialog, Platform};

pub(crate) struct AirPlatform;

impl Platform for AirPlatform {
    fn pick_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
    }

    fn save_file(&self, _dialog: FileDialog, _answer: Answer) -> bool {
        false
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

    fn has_file_dialogs(&self) -> bool {
        false
    }

    fn opens_files(&self) -> bool {
        false
    }

    fn launches_processes(&self) -> bool {
        false
    }

    fn has_army(&self) -> bool {
        false
    }

    fn is_touch(&self) -> bool {
        true
    }

    fn has_hotkeys(&self) -> bool {
        false
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
}
