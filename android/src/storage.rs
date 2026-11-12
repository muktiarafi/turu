//! The Java side's `Storage` object (`Storage.kt`): account passwords kept with a key in the
//! Android Keystore, and the documents the system picker chose (`saf.rs`). Its `nativeInit`
//! hands this module the class, which a native thread can't look up itself.

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{Global, JClass, JObject, JString, JValue};
use jni::refs::Reference;
use jni::{Env, EnvUnowned, JavaVM, jni_sig, jni_str};
use std::sync::OnceLock;
use turu_bot::accounts::SecretStore;

/// The JVM and `Storage`'s class, once the activity or the service called `Storage.init`.
static STORAGE: OnceLock<(JavaVM, Global<JClass<'static>>)> = OnceLock::new();

/// `Storage.nativeInit()`: `Storage.init(context)` ran (in the activity or the service).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_Storage_nativeInit<'local>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            if STORAGE.get().is_none() {
                let _ = STORAGE.set((env.get_java_vm()?, env.new_global_ref(&class)?));
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Runs `work` with `Storage`'s class on this thread (attached to the JVM). A Java exception
/// comes back as its error.
pub(crate) fn with_storage<T>(
    work: impl FnOnce(&mut Env, &JClass) -> jni::errors::Result<T>,
) -> Result<T, String> {
    let (vm, class) = STORAGE.get().ok_or("Android's storage isn't set up yet")?;
    vm.attach_current_thread(|env: &mut Env| work(env, class))
        .map_err(|e| e.to_string())
}

/// A `String` a Java method returned; `None` for null.
pub(crate) fn optional_string(
    env: &mut Env,
    object: JObject,
) -> jni::errors::Result<Option<String>> {
    if object.is_null() {
        return Ok(None);
    }
    let text = env.cast_local::<JString>(object)?;
    Ok(Some(text.try_to_string(env)?))
}

/// Account passwords for the Accounts window (`turu_bot::accounts`), in place of the desktop's
/// keychain: `Storage.savePassword` encrypts each with an AES-GCM key in the Android Keystore,
/// into the app's internal storage.
pub struct KeystoreStore;

impl SecretStore for KeystoreStore {
    fn set(&self, username: &str, password: &str) -> Result<(), String> {
        with_storage(|env, class| {
            let name = env.new_string(username)?;
            let password = env.new_string(password)?;
            env.call_static_method(
                class,
                jni_str!("savePassword"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
                &[JValue::Object(&name), JValue::Object(&password)],
            )?;
            Ok(())
        })
        .map_err(|e| format!("couldn't save the password in the Android Keystore: {e}"))
    }

    fn get(&self, username: &str) -> Result<Option<String>, String> {
        with_storage(|env, class| {
            let name = env.new_string(username)?;
            let password = env
                .call_static_method(
                    class,
                    jni_str!("password"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[JValue::Object(&name)],
                )?
                .l()?;
            optional_string(env, password)
        })
        .map_err(|e| format!("couldn't read the password from the Android Keystore: {e}"))
    }

    fn delete(&self, username: &str) -> Result<(), String> {
        with_storage(|env, class| {
            let name = env.new_string(username)?;
            env.call_static_method(
                class,
                jni_str!("deletePassword"),
                jni_sig!("(Ljava/lang/String;)V"),
                &[JValue::Object(&name)],
            )?;
            Ok(())
        })
        .map_err(|e| format!("couldn't remove the password: {e}"))
    }
}

/// Makes the Accounts window keep passwords with [`KeystoreStore`]; before any is read.
pub(crate) fn install() {
    if !turu_bot::accounts::set_secret_store(Box::new(KeystoreStore)) {
        tracing::warn!("A password store was already in use; passwords aren't kept on this phone");
    }
}
