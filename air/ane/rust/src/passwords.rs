//! Account passwords for the Accounts window (`turu_bot::accounts`), in place of the desktop's
//! keychain, as the Ruffle build's `KeystoreStore` (`android/src/storage.rs`): Kotlin's
//! `Passwords` encrypts each with an AES-GCM key in the Android Keystore, into the app's internal
//! storage.

use jni::objects::{Global, JClass, JString, JValue};
use jni::refs::Reference;
use jni::{Env, JavaVM, jni_sig, jni_str};
use std::sync::OnceLock;
use turu_bot::accounts::SecretStore;

/// The JVM and `Passwords`' class, looked up by [`install`] (an engine thread can't: its class
/// loader is the system's, which doesn't see the extension's classes).
static PASSWORDS: OnceLock<(JavaVM, Global<JClass<'static>>)> = OnceLock::new();

/// Makes the Accounts window keep passwords with [`KeystoreStore`]. From `nativeStart` (a Java
/// thread, so the extension's classes are found), after `Passwords.init` and before any password
/// is read.
pub(crate) fn install(env: &mut Env) -> jni::errors::Result<()> {
    if PASSWORDS.get().is_none() {
        let class = env.find_class(jni_str!("com/muktiarafi/turu/air/Passwords"))?;
        let _ = PASSWORDS.set((env.get_java_vm()?, env.new_global_ref(&class)?));
    }
    if !turu_bot::accounts::set_secret_store(Box::new(KeystoreStore)) {
        tracing::warn!("A password store was already in use; passwords aren't kept on this phone");
    }
    Ok(())
}

/// Runs `work` with `Passwords`' class on this thread (attached to the JVM). A Java exception
/// comes back as its error.
fn with_passwords<T>(
    work: impl FnOnce(&mut Env, &JClass) -> jni::errors::Result<T>,
) -> Result<T, String> {
    let (vm, class) = PASSWORDS
        .get()
        .ok_or("the password store isn't set up yet")?;
    crate::android::attach(vm, |env: &mut Env| work(env, class)).map_err(|e| e.to_string())
}

/// See the module's documentation.
struct KeystoreStore;

impl SecretStore for KeystoreStore {
    fn set(&self, username: &str, password: &str) -> Result<(), String> {
        with_passwords(|env, class| {
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
        with_passwords(|env, class| {
            let name = env.new_string(username)?;
            let password = env
                .call_static_method(
                    class,
                    jni_str!("password"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[JValue::Object(&name)],
                )?
                .l()?;
            if password.is_null() {
                return Ok(None);
            }
            let password = env.cast_local::<JString>(password)?;
            Ok(Some(password.try_to_string(env)?))
        })
        .map_err(|e| format!("couldn't read the password from the Android Keystore: {e}"))
    }

    fn delete(&self, username: &str) -> Result<(), String> {
        with_passwords(|env, class| {
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
