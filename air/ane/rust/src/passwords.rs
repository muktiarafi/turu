//! Account passwords for the Accounts window (`turu_bot::accounts`), in place of the desktop's
//! keychain, as the Ruffle build's `KeystoreStore` (`android/src/storage.rs`): Kotlin's
//! `Passwords` encrypts each with an AES-GCM key in the Android Keystore, into the app's internal
//! storage.

use crate::java::Passwords;
use jni::refs::Reference;
use jni::{Env, JavaVM};
use std::sync::OnceLock;
use turu_bot::accounts::SecretStore;

/// The JVM, kept by [`install`] (`Passwords`' methods are bound in `java.rs`).
static PASSWORDS: OnceLock<JavaVM> = OnceLock::new();

/// Makes the Accounts window keep passwords with [`KeystoreStore`]. From `nativeStart` (a Java
/// thread, so the extension's classes are found), after `Passwords.init` and before any password
/// is read.
pub(crate) fn install(env: &mut Env) -> jni::errors::Result<()> {
    if PASSWORDS.get().is_none() {
        let _ = PASSWORDS.set(env.get_java_vm()?);
    }
    if !turu_bot::accounts::set_secret_store(Box::new(KeystoreStore)) {
        tracing::warn!("A password store was already in use; passwords aren't kept on this phone");
    }
    Ok(())
}

/// Runs `work` with `Passwords`' class on this thread (attached to the JVM). A Java exception
/// comes back as its error.
fn with_passwords<T>(work: impl FnOnce(&mut Env) -> jni::errors::Result<T>) -> Result<T, String> {
    let vm = PASSWORDS
        .get()
        .ok_or("the password store isn't set up yet")?;
    crate::android::attach(vm, work).map_err(|e| e.to_string())
}

/// See the module's documentation.
struct KeystoreStore;

impl SecretStore for KeystoreStore {
    fn set(&self, username: &str, password: &str) -> Result<(), String> {
        with_passwords(|env| {
            let name = env.new_string(username)?;
            let password = env.new_string(password)?;
            Passwords::save_password(env, &name, &password)?;
            Ok(())
        })
        .map_err(|e| format!("couldn't save the password in the Android Keystore: {e}"))
    }

    fn get(&self, username: &str) -> Result<Option<String>, String> {
        with_passwords(|env| {
            let name = env.new_string(username)?;
            let password = Passwords::password(env, &name)?;
            if password.is_null() {
                return Ok(None);
            }
            Ok(Some(password.try_to_string(env)?))
        })
        .map_err(|e| format!("couldn't read the password from the Android Keystore: {e}"))
    }

    fn delete(&self, username: &str) -> Result<(), String> {
        with_passwords(|env| {
            let name = env.new_string(username)?;
            Passwords::delete_password(env, &name)?;
            Ok(())
        })
        .map_err(|e| format!("couldn't remove the password: {e}"))
    }
}
