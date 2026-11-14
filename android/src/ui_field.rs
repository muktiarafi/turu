//! Turu's own focused text box (egui's) as a native Android text field just above the soft
//! keyboard, as the game's are (`game_field.rs`): Kotlin's `NativeField`, with Android's caret,
//! selection handles, text toolbar, suggestions, autocorrect and autofill. egui alone gets only
//! keys from the keyboard.
//!
//! The text boxes say what they hold each frame (`turu_frontend::gui::turu::native_field`); the
//! screen keeps Kotlin's field in step after each frame ([`UiField::sync`]). Kotlin's edits come
//! back here as the whole text and selection and reach the box in the next frame; its Send is
//! Enter in the box, and Back takes the focus off it.

use crate::platform::with_activity;
use jni::objects::{JClass, JString, JValue};
use jni::sys::jint;
use jni::{EnvUnowned, jni_sig, jni_str};
use turu_frontend::gui::turu::native_field::{self, Change, Contents, Kind, NativeField};

/// Kotlin's field for Turu's text boxes, as last shown or edited.
#[derive(Default)]
pub struct UiField {
    field: NativeField,
}

impl UiField {
    /// After a UI frame: shows, updates or hides Kotlin's field to match the focused text box.
    /// True while it's up (egui's own keyboard path then stays down).
    pub fn sync(&mut self, ctx: &egui::Context) -> bool {
        match self.field.sync(native_field::take_focused(ctx)) {
            Some(Change::Show(focused)) => show(&focused.contents, focused.kind, &focused.hint),
            Some(Change::Update(contents)) => update(&contents),
            Some(Change::Hide) => hide(),
            None => {}
        }
        self.field.is_shown()
    }

    /// Whether Kotlin's field is up.
    pub fn is_shown(&self) -> bool {
        self.field.is_shown()
    }
}

/// `TuruActivity.showUiField(text, start, end, kind, hint)`.
fn show(contents: &Contents, kind: Kind, hint: &str) {
    let kind = match kind {
        Kind::Line => 0,
        Kind::Password => 1,
        Kind::Lines => 2,
        Kind::Number => 3,
    };
    call("showUiField", |env, activity| {
        let text = env.new_string(&contents.text)?;
        let hint = env.new_string(hint)?;
        env.call_method(
            activity,
            jni_str!("showUiField"),
            jni_sig!("(Ljava/lang/String;IIILjava/lang/String;)V"),
            &[
                JValue::Object(&text),
                JValue::Int(contents.start as jint),
                JValue::Int(contents.end as jint),
                JValue::Int(kind),
                JValue::Object(&hint),
            ],
        )?;
        Ok(())
    });
}

/// `TuruActivity.updateUiField(text, start, end)`.
fn update(contents: &Contents) {
    call("updateUiField", |env, activity| {
        let text = env.new_string(&contents.text)?;
        env.call_method(
            activity,
            jni_str!("updateUiField"),
            jni_sig!("(Ljava/lang/String;II)V"),
            &[
                JValue::Object(&text),
                JValue::Int(contents.start as jint),
                JValue::Int(contents.end as jint),
            ],
        )?;
        Ok(())
    });
}

/// `TuruActivity.hideUiField()`.
fn hide() {
    call("hideUiField", |env, activity| {
        env.call_method(activity, jni_str!("hideUiField"), jni_sig!("()V"), &[])?;
        Ok(())
    });
}

/// Runs `work` with the activity, logging a failure of `what`.
fn call(
    what: &str,
    work: impl FnOnce(&mut jni::Env, &jni::objects::JObject) -> jni::errors::Result<()>,
) {
    let Some(app) = crate::app() else {
        return;
    };
    if let Err(e) = with_activity(&app, work) {
        tracing::warn!("{what} failed: {e}");
    }
}

/// `TuruActivity.nativeUiFieldEdited(text, start, end)`, on the UI thread.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeUiFieldEdited<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    text: JString<'local>,
    start: jint,
    end: jint,
) {
    let text = unowned_env
        .with_env(|env| -> jni::errors::Result<String> { text.try_to_string(env) })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    let contents = Contents {
        text,
        start: start.max(0) as usize,
        end: end.max(0) as usize,
    };
    crate::run_on_engine(move |_| {
        crate::with_screen(|screen| {
            let (field, ctx) = screen.ui_field();
            field.field.edited(ctx, contents);
        });
    });
}

/// `TuruActivity.nativeUiFieldEnter()`, on the UI thread.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeUiFieldEnter<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    crate::run_on_engine(|_| {
        crate::with_screen(|screen| native_field::enter(screen.ui_field().1));
    });
}

/// `TuruActivity.nativeUiFieldClosed()`, on the UI thread.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_TuruActivity_nativeUiFieldClosed<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    crate::run_on_engine(|_| {
        crate::with_screen(|screen| {
            let (field, ctx) = screen.ui_field();
            field.field.closed(ctx);
        });
    });
}
