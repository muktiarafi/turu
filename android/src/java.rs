//! The Kotlin classes the Rust side calls, and is called from, bound with jni's
//! `bind_java_type!`: each method's name and signature are written once, here, and checked against
//! the class when the app starts ([`check`]), not when the method is first called.

// The macro's expansion calls `AtomicBool::fetch_update`, deprecated in newer Rust; Java methods
// with many parameters (`askCrashUploads`, `status`) get Rust wrappers with as many.
#![allow(deprecated, clippy::too_many_arguments)]

use jni::Env;
use jni::refs::LoaderContext;

jni::bind_java_type! {
    rust_type = Storage,
    rust_type_vis = pub(crate),
    java_type = com.muktiarafi.turu.Storage,
    native_methods_export = false,
    methods {
        static fn save_password(name: JString, password: JString),
        static fn password(name: JString) -> JString,
        static fn delete_password(name: JString),
        static fn write_document(uri: JString, path: JString) -> JString,
    },
    native_methods {
        static raw fn native_init {
            sig = () -> void,
            fn = crate::storage::Java_com_muktiarafi_turu_Storage_nativeInit,
        },
    },
}

jni::bind_java_type! {
    rust_type = TuruActivity,
    rust_type_vis = pub(crate),
    java_type = com.muktiarafi.turu.TuruActivity,
    native_methods_export = false,
    methods {
        fn show_game_field(text: JString, start: jint, end: jint, password: jboolean, max: jint),
        fn update_game_field(text: JString, start: jint, end: jint),
        fn hide_game_field(),
        fn show_ui_field(text: JString, start: jint, end: jint, kind: jint, hint: JString),
        fn update_ui_field(text: JString, start: jint, end: jint),
        fn hide_ui_field(),
        fn open_url(url: JString),
        fn share_text(subject: JString, text: JString),
        fn copy_text(text: JString),
        fn clip_text() -> JString,
        fn install_package(path: JString) -> JString,
        fn ask_crash_uploads(
            title: JString,
            message: JString,
            note: JString,
            link: JString,
            link_url: JString,
            no: JString,
            yes: JString,
        ),
        fn prefer_refresh_rate(hz: jfloat),
        fn open_document(mime_types: JString, folder: JString),
        fn create_document(mime_type: JString, file_name: JString),
    },
    native_methods {
        static raw fn native_document_picked {
            sig = (path: JString) -> void,
            fn = crate::saf::Java_com_muktiarafi_turu_TuruActivity_nativeDocumentPicked,
        },
        static raw fn native_document_created {
            sig = (uri: JString, path: JString) -> void,
            fn = crate::saf::Java_com_muktiarafi_turu_TuruActivity_nativeDocumentCreated,
        },
        static raw fn native_keyboard_height {
            sig = (pixels: jint) -> void,
            fn = crate::ui::Java_com_muktiarafi_turu_TuruActivity_nativeKeyboardHeight,
        },
        static raw fn native_cutout {
            sig = (left: jint, right: jint) -> void,
            fn = crate::ui::Java_com_muktiarafi_turu_TuruActivity_nativeCutout,
        },
        static raw fn native_refresh_rate {
            sig = (hz: jfloat) -> void,
            fn = crate::ui::Java_com_muktiarafi_turu_TuruActivity_nativeRefreshRate,
        },
        static raw fn native_package_installed {
            sig = (status: jint, message: JString) -> void,
            fn = crate::platform::Java_com_muktiarafi_turu_TuruActivity_nativePackageInstalled,
        },
        static raw fn native_crash_uploads_answer {
            sig = (answer: jint) -> void,
            fn = crate::platform::Java_com_muktiarafi_turu_TuruActivity_nativeCrashUploadsAnswer,
        },
        static raw fn native_game_field_edited {
            sig = (text: JString, start: jint, end: jint) -> void,
            fn = crate::game_field::Java_com_muktiarafi_turu_TuruActivity_nativeGameFieldEdited,
        },
        static raw fn native_game_field_enter {
            sig = () -> void,
            fn = crate::game_field::Java_com_muktiarafi_turu_TuruActivity_nativeGameFieldEnter,
        },
        static raw fn native_game_field_closed {
            sig = () -> void,
            fn = crate::game_field::Java_com_muktiarafi_turu_TuruActivity_nativeGameFieldClosed,
        },
        static raw fn native_ui_field_edited {
            sig = (text: JString, start: jint, end: jint) -> void,
            fn = crate::ui_field::Java_com_muktiarafi_turu_TuruActivity_nativeUiFieldEdited,
        },
        static raw fn native_ui_field_enter {
            sig = () -> void,
            fn = crate::ui_field::Java_com_muktiarafi_turu_TuruActivity_nativeUiFieldEnter,
        },
        static raw fn native_ui_field_closed {
            sig = () -> void,
            fn = crate::ui_field::Java_com_muktiarafi_turu_TuruActivity_nativeUiFieldClosed,
        },
    },
}

jni::bind_java_type! {
    rust_type = TuruService,
    rust_type_vis = pub(crate),
    java_type = com.muktiarafi.turu.TuruService,
    native_methods_export = false,
    methods {
        static fn crash_uploads_available() -> jboolean,
        static fn set_crash_uploads(on: jboolean),
        static fn panicked(message: JString),
        static fn breadcrumb(category: JString, message: JString),
        static fn alert(kind: jint, title: JString, text: JString),
        static fn gpu_lost(why: JString),
        static fn status(
            title: JString,
            text: JString,
            running: jboolean,
            paused: jboolean,
            startable: jboolean,
            awake: jboolean,
        ),
    },
    native_methods {
        static raw fn native_start {
            sig = (context: android.content.Context, files_dir: JString) -> void,
            fn = crate::notify::Java_com_muktiarafi_turu_TuruService_nativeStart,
        },
        static raw fn native_action {
            sig = (action: JString) -> void,
            fn = crate::notify::Java_com_muktiarafi_turu_TuruService_nativeAction,
        },
        static raw fn native_restarted {
            sig = () -> void,
            fn = crate::restart::Java_com_muktiarafi_turu_TuruService_nativeRestarted,
        },
        static raw fn native_trim_memory {
            sig = (level: jint) -> void,
            fn = crate::memory::Java_com_muktiarafi_turu_TuruService_nativeTrimMemory,
        },
    },
}

/// Looks up every bound class's methods and registers its native methods, through `loader` (a
/// native thread's own class loader doesn't see the app's classes). Each mismatch is logged with
/// what Java said, and the app stops: a mismatch shows the first time a build is started.
pub(crate) fn check(env: &mut Env, loader: &LoaderContext) {
    let checked = [
        ("TuruActivity", TuruActivityAPI::get(env, loader).map(drop)),
        ("TuruService", TuruServiceAPI::get(env, loader).map(drop)),
        ("Storage", StorageAPI::get(env, loader).map(drop)),
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
