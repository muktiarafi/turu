//! M1 spike (docs/TAB_PROCESSES_PROPOSAL.md): each account in a process of its own. While
//! `turu-slots.txt` is in the data folder, the activity's process runs no engine (the shell:
//! `TuruActivity`, `android_main`), and each slot service (`SlotService.kt`, processes `:c1` and
//! `:c2`) runs one: one tab, set up from `turu-args-c<N>.txt`, drawing in the activity's surface
//! while it's the slot on screen. These are the slot process's natives: the engine's start, the
//! surface handed over and taken back, touch, and a probe of file locks between processes.

use jni::EnvUnowned;
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::sys::jint;
use ndk::native_window::NativeWindow;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::input::Touch;

/// The switch file in the data folder that turns the spike on.
pub(crate) const SWITCH_FILE: &str = "turu-slots.txt";

/// This process's slot (1 or 2), in a slot process.
static SLOT: OnceLock<u32> = OnceLock::new();
/// The activity's density, which a slot process has no activity to ask (`attach_surface`).
static DENSITY: Mutex<f32> = Mutex::new(1.0);
/// The lock file held by [`Java_com_muktiarafi_turu_SlotService_nativeLockProbe`].
static HELD: Mutex<Option<File>> = Mutex::new(None);

/// This process's slot, in a slot process.
pub(crate) fn slot() -> Option<u32> {
    SLOT.get().copied()
}

/// Whether the spike is on (the activity's process is then only the shell).
pub(crate) fn spike_on(root: &Path) -> bool {
    root.join(SWITCH_FILE).exists()
}

/// The activity's density, as the slot was told it with the surface.
pub(crate) fn density() -> f32 {
    *DENSITY.lock().unwrap_or_else(|e| e.into_inner())
}

/// The slot's startup flags, instead of `turu-args.txt`.
pub(crate) fn args_file(root: &Path) -> Option<PathBuf> {
    slot().map(|slot| root.join(format!("turu-args-c{slot}.txt")))
}

/// `SlotService.nativeStartSlot(context, filesDir, slot)`: starts this slot process's engine.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeStartSlot<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
    files_dir: JString<'local>,
    slot: jint,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            crate::init_logging();
            let _ = SLOT.set(slot as u32);
            tracing::info!("Slot {slot}: process {}", std::process::id());
            crate::java::check(env, &jni::refs::LoaderContext::FromObject(&context));
            let context = env.new_local_ref(&context)?;
            if let Err(e) = rustls_platform_verifier::android::init_with_env(env, context) {
                log::error!("Couldn't set up certificate checks; HTTPS will fail: {e}");
            }
            crate::start_engine(PathBuf::from(files_dir.try_to_string(env)?));
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `SlotService.nativeAttach(surface, density)`: draws in the activity's surface from now on.
/// 1 when the slot took it.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeAttach<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    surface: JObject<'local>,
    density: f32,
) -> jint {
    unowned_env
        .with_env(|env| -> jni::errors::Result<jint> {
            *DENSITY.lock().unwrap_or_else(|e| e.into_inner()) = density;
            // SAFETY: a live `android.view.Surface` from the caller, on this thread's env.
            let window = unsafe {
                NativeWindow::from_surface(env.get_raw().cast(), surface.as_raw().cast())
            };
            let Some(window) = window else {
                tracing::warn!("Slot {:?}: no window from the surface", slot());
                return Ok(0);
            };
            tracing::info!(
                "Slot {:?}: surface attached ({}x{})",
                slot(),
                window.width(),
                window.height()
            );
            *crate::lock_window() = Some(window);
            crate::run_on_engine(crate::attach_surface);
            Ok(1)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `SlotService.nativeDetach()`: stops drawing in the surface and lets it go. 1 once the engine
/// has dropped it (the surface can then go to another slot), 0 when it didn't answer in time.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeDetach<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jint {
    let answered = crate::detach_surface();
    tracing::info!("Slot {:?}: surface detached (answered: {answered})", slot());
    jint::from(answered)
}

/// `SlotService.nativeResize()`: the activity's surface changed size.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeResize<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    crate::run_on_engine(crate::resize);
}

/// `SlotService.nativeTouch(kind, id, x, y)`: a finger in window pixels, forwarded by the shell
/// (kind 0 down, 1 move, 2 up, 3 cancel).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeTouch<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    kind: jint,
    id: jint,
    x: f32,
    y: f32,
) {
    let (x, y) = (f64::from(x), f64::from(y));
    let touch = match kind {
        0 => Touch::Down(id, x, y),
        1 => Touch::Move(id, x, y),
        2 => Touch::Up(id, x, y),
        _ => Touch::Cancel,
    };
    crate::run_on_engine(move |player| crate::with_screen(|screen| screen.touch(player, touch)));
}

/// `SlotService.nativeLockProbe(action)`: 1 takes `army/.spike.lock` with `File::try_lock` (as
/// `api::army`'s locks) and holds it, 2 lets it go. Answers 0 held, 1 another process holds it,
/// 2 let go, -1 an error.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeLockProbe<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    action: jint,
) -> jint {
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    if action == 2 {
        *held = None;
        tracing::info!("Slot {:?}: lock probe let go", slot());
        return 2;
    }
    let dir = turu_bot::paths::Paths::new().root.join("army");
    let result = std::fs::create_dir_all(&dir).and_then(|()| {
        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(".spike.lock"))
    });
    let file = match result {
        Ok(file) => file,
        Err(e) => {
            tracing::warn!("Slot {:?}: lock probe couldn't open the file: {e}", slot());
            return -1;
        }
    };
    let answer = match file.try_lock() {
        Ok(()) => {
            *held = Some(file);
            0
        }
        Err(TryLockError::WouldBlock) => 1,
        Err(TryLockError::Error(e)) => {
            tracing::warn!("Slot {:?}: lock probe failed: {e}", slot());
            -1
        }
    };
    tracing::info!(
        "Slot {:?}: lock probe answered {answer} (0 held, 1 busy)",
        slot()
    );
    answer
}
