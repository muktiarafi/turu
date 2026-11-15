//! The Kotlin classes the Rust side calls, and is called from, bound with jni's
//! `bind_java_type!`: each method's name and signature are written once, here, and checked against
//! the class when the extension starts ([`check`], from `Bridge.nativeStart`), not when the method
//! is first called. As the Ruffle build's `android/src/java.rs`.

// The macro's expansion calls `AtomicBool::fetch_update`, deprecated in newer Rust; Java methods
// with many parameters (`askCrashUploads`, `status`) get Rust wrappers with as many.
#![allow(deprecated, clippy::too_many_arguments)]

use jni::Env;
use jni::refs::LoaderContext;

jni::bind_java_type! {
    rust_type = Bridge,
    rust_type_vis = pub(crate),
    java_type = com.muktiarafi.turu.air.Bridge,
    native_methods_export = false,
    methods {
        static fn dispatch(code: JString, level: JString),
        static fn status(
            title: JString,
            text: JString,
            running: jboolean,
            paused: jboolean,
            startable: jboolean,
            awake: jboolean,
        ),
        static fn game_insets(left: jint, top: jint),
        static fn show_ui_field(text: JString, start: jint, end: jint, kind: jint, hint: JString),
        static fn update_ui_field(text: JString, start: jint, end: jint),
        static fn hide_ui_field(),
        static fn soft_keyboard(shown: jboolean),
        static fn open_url(url: JString),
        static fn share_text(subject: JString, text: JString),
        static fn copy_text(text: JString),
        static fn ask_crash_uploads(
            title: JString,
            message: JString,
            note: JString,
            link: JString,
            link_url: JString,
            no: JString,
            yes: JString,
        ) -> jboolean,
        static fn install_package(path: JString) -> JString,
        static fn alert(kind: jint, title: JString, text: JString),
        static fn open_document(mime_types: JString, folder: JString) -> jboolean,
        static fn create_document(mime_type: JString, file_name: JString) -> jboolean,
        static fn write_document(uri: JString, path: JString) -> JString,
        static fn crash_uploads_available() -> jboolean,
        static fn set_crash_uploads(on: jboolean),
        static fn panicked(message: JString),
    },
    native_methods {
        static raw fn native_start {
            sig = (context: android.content.Context, files_dir: JString) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeStart,
        },
        static raw fn native_flash_call {
            sig = (name: JString, args: JString, client: jint) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeFlashCall,
        },
        static raw fn native_reply {
            sig = (id: jlong, value: JString, run_micros: jint) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeReply,
        },
        static raw fn native_patch {
            sig = (swf: jbyte[], target: jint) -> jbyte[],
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativePatch,
        },
        static raw fn native_fetch {
            sig = (id: jlong, url: JString) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeFetch,
        },
        static raw fn native_fetch_result {
            sig = (id: jlong) -> jbyte[],
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeFetchResult,
        },
        static raw fn native_surface {
            sig = (surface: android.view.Surface, density: jfloat) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeSurface,
        },
        static raw fn native_touch {
            sig = (phase: jint, x: jfloat, y: jfloat) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeTouch,
        },
        static raw fn native_on_ui {
            sig = (x: jfloat, y: jfloat) -> jboolean,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeOnUi,
        },
        static raw fn native_text {
            sig = (text: JString) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeText,
        },
        static raw fn native_key {
            sig = (key: jint) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeKey,
        },
        static raw fn native_insets {
            sig = (keyboard: jint, cutout_left: jint, cutout_right: jint) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeInsets,
        },
        static raw fn native_field_place {
            sig = (top: jint, left: jint) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeFieldPlace,
        },
        static raw fn native_ui_field_edited {
            sig = (text: JString, start: jint, end: jint) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeUiFieldEdited,
        },
        static raw fn native_ui_field_enter {
            sig = () -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeUiFieldEnter,
        },
        static raw fn native_ui_field_closed {
            sig = () -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeUiFieldClosed,
        },
        static raw fn native_action {
            sig = (action: JString) -> void,
            fn = crate::notify::Java_com_muktiarafi_turu_air_Bridge_nativeAction,
        },
        static raw fn native_crash_uploads_answer {
            sig = (answer: jint) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativeCrashUploadsAnswer,
        },
        static raw fn native_package_installed {
            sig = (status: jint, message: JString) -> void,
            fn = crate::android::Java_com_muktiarafi_turu_air_Bridge_nativePackageInstalled,
        },
        static raw fn native_document_picked {
            sig = (path: JString) -> void,
            fn = crate::saf::Java_com_muktiarafi_turu_air_Bridge_nativeDocumentPicked,
        },
        static raw fn native_document_created {
            sig = (uri: JString, path: JString) -> void,
            fn = crate::saf::Java_com_muktiarafi_turu_air_Bridge_nativeDocumentCreated,
        },
    },
}

jni::bind_java_type! {
    rust_type = Passwords,
    rust_type_vis = pub(crate),
    java_type = com.muktiarafi.turu.air.Passwords,
    methods {
        static fn save_password(name: JString, password: JString),
        static fn password(name: JString) -> JString,
        static fn delete_password(name: JString),
    },
}

/// Looks up every bound class's methods and registers its native methods, through `loader` (a
/// native thread's own class loader doesn't see the app's classes). Each mismatch is logged with
/// what Java said, and the app stops: a mismatch shows the first time a build is started.
pub(crate) fn check(env: &mut Env, loader: &LoaderContext) {
    let checked = [
        ("Bridge", BridgeAPI::get(env, loader).map(drop)),
        ("Passwords", PasswordsAPI::get(env, loader).map(drop)),
    ];
    let count = checked.len();
    let failures: Vec<String> = checked
        .into_iter()
        .filter_map(|(class, result)| {
            let e = result.err()?;
            Some(format!("{class}: {}", describe(env, e)))
        })
        .collect();
    if failures.is_empty() {
        tracing::info!("Kotlin bindings checked: {count} classes");
        return;
    }
    for failure in &failures {
        tracing::error!("Kotlin and Rust disagree on {failure}");
    }
    panic!("Kotlin and Rust disagree: {}", failures.join("; "));
}

/// `e`, with the pending Java exception's text (a `NoSuchMethodError` names the method), cleared.
fn describe(env: &mut Env, e: jni::errors::Error) -> String {
    let Some(exception) = env.exception_occurred() else {
        return e.to_string();
    };
    env.exception_clear();
    env.call_method(
        &exception,
        jni::jni_str!("toString"),
        jni::jni_sig!("()Ljava/lang/String;"),
        &[],
    )
    .and_then(|text| text.l())
    .and_then(|text| env.cast_local::<jni::objects::JString>(text))
    .and_then(|text| text.try_to_string(env))
    .unwrap_or_else(|_| e.to_string())
}
