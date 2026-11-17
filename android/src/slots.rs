//! Process tabs (docs/TAB_PROCESSES_PROPOSAL.md, M3): each tab in a slot, a foreground service in a
//! process of its own (`SlotService.kt`: `Slot1Service` ... `Slot4Service` in `:c1` ... `:c4`). While
//! they're on (unless `turu-process-tabs.txt` in the data folder says `off`:
//! `turu_frontend::slots::switched_on`), the activity's process runs no engine: it's the
//! shell (`SlotShell.kt`, `slot_shell.rs`). These are a slot process's side: the engine's start
//! from the slot's record (`turu_frontend::slots`), the surface handed over and taken back,
//! touches, the shell's messages (keys among them), and the slot's notification.

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::sys::jint;
use jni::{Env, EnvUnowned, JavaVM};
use ndk::native_window::NativeWindow;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use turu_bot::accounts::SecretStore;
use turu_frontend::gui::turu::native_field::Contents;
use turu_frontend::shell::{FieldId, ToClient, ToShell};
use turu_frontend::slots::SlotRecord;
use turu_frontend::turu;

use crate::input::Touch;
use crate::java::SlotService;

/// This process's slot (1 to 4), in a slot process.
static SLOT: OnceLock<u32> = OnceLock::new();
/// The slot process's JVM, for calls into `SlotService`.
static VM: OnceLock<JavaVM> = OnceLock::new();
/// The activity's density, which a slot process has no activity to ask (`attach_surface`).
static DENSITY: Mutex<f32> = Mutex::new(1.0);
/// The slot's control API (its address and token), for its hello to the shell.
static API: Mutex<Option<(String, String)>> = Mutex::new(None);

/// This process's slot, in a slot process.
pub(crate) fn slot() -> Option<u32> {
    SLOT.get().copied()
}

/// The activity's density, as the slot was told it with the surface.
pub(crate) fn density() -> f32 {
    *DENSITY.lock().unwrap_or_else(|e| e.into_inner())
}

/// The Keystore's name for the password slot `slot` logs in with, when Accounts doesn't keep it
/// (no AQW name starts with `@`).
pub(crate) fn password_key(slot: u32) -> String {
    format!("@slot:{slot}")
}

/// The slot's startup flags: the app's own from `turu-args.txt`, its record's, the login's password (Accounts', else the shell's copy
/// for this slot), and a control API of its own on a free local port.
pub(crate) fn args(root: &Path, slot: u32) -> Vec<OsString> {
    let record = SlotRecord::read(&turu_frontend::slots::dir(root), slot).unwrap_or_else(|| {
        tracing::warn!("Slot {slot}: no record; at the login screen");
        SlotRecord::default()
    });
    // The app's own flags (graphics, logs) from turu-args.txt, then the tab's.
    let (_, _, app) = turu_frontend::slots::split_args(&crate::args_lines(root));
    let mut args: Vec<OsString> = app.iter().chain(&record.args).map(OsString::from).collect();
    if let Some(username) = record.username() {
        let password = turu_bot::accounts::password(username)
            .ok()
            .flatten()
            .or_else(|| {
                crate::storage::KeystoreStore
                    .get(&password_key(slot))
                    .ok()
                    .flatten()
            });
        match password {
            Some(password) => args.extend(["--password".into(), password.into()]),
            None => tracing::info!(
                "Slot {slot}: no saved password for {username}; log in on the game's screen"
            ),
        }
    }
    let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr());
    match (port, turu_bot::control_api::new_token()) {
        (Ok(address), Ok(token)) => {
            args.push(format!("--api={address}").into());
            // SAFETY: before the engine's threads exist (`start_engine`, early).
            unsafe { std::env::set_var("TURU_API_TOKEN", &token) };
            // For the slot that runs the web monitor, when no shell lists the slots.
            let file = turu_frontend::slots::SlotAddress {
                api: address.to_string(),
                token: token.clone(),
            };
            if let Err(e) = file.write(&turu_frontend::slots::dir(root), slot) {
                tracing::warn!("Slot {slot}: couldn't write where its control API is: {e}");
            }
            *API.lock().unwrap_or_else(|e| e.into_inner()) = Some((address.to_string(), token));
        }
        (Err(e), _) => tracing::warn!("Slot {slot}: no control API: {e}"),
        (_, Err(e)) => tracing::warn!("Slot {slot}: no control API: {e}"),
    }
    args
}

/// Once the engine runs in a slot: its tab requests and status go to the shell, and its
/// notification follows the tab.
pub(crate) fn start_link(root: &Path) {
    turu_frontend::client_link::install(std::sync::Arc::new(to_shell), || true);
    // The other slots, from their files (each rewritten when its slot starts: the shell's last list
    // goes stale when the shell dies and a slot restarts on a new port), named as the shell's list
    // names them.
    let dir = turu_frontend::slots::dir(root);
    let me = slot().unwrap_or(0);
    turu::set_peer_source(Box::new(move || {
        let named = turu::shell_peers();
        let mut peers = turu_frontend::slots::peers_from_files(&dir, me);
        for peer in &mut peers {
            if let Some(listed) = named.iter().find(|listed| listed.id == peer.id) {
                peer.label.clone_from(&listed.label);
            }
        }
        peers
    }));
    // One slot runs the web monitor and the update checks: the first to take its lock.
    turu::start_monitor_election();
    // A shell that bound while the engine started up got a hello without the control API.
    hello();
    start_status();
}

/// Says hello to the shell (dropped while none is bound), with the control API once the engine
/// set it up.
fn hello() {
    let api = API.lock().unwrap_or_else(|e| e.into_inner()).clone();
    to_shell(ToShell::Hello {
        token: String::new(),
        pid: std::process::id(),
        version: turu::VERSION.to_owned(),
        api: api.as_ref().map(|(address, _)| address.clone()),
        api_token: api.map(|(_, token)| token),
    });
}

/// Puts `text` on the clipboard (`SlotService.copyText`).
pub(crate) fn copy_text(text: &str) {
    with_service(|env| {
        let text = env.new_string(text)?;
        SlotService::copy_text(env, &text)
    });
}

/// The clipboard's text (`SlotService.clipText`).
pub(crate) fn clip_text() -> Option<String> {
    let vm = VM.get()?;
    crate::attach(vm, |env: &mut Env| {
        let text = SlotService::clip_text(env)?;
        crate::storage::optional_string(env, text.into())
    })
    .inspect_err(|e| tracing::warn!("Slot {:?}: couldn't read the clipboard: {e}", slot()))
    .ok()
    .flatten()
}

/// Sends a message to the shell, when one is bound (`SlotService.toShell`).
pub(crate) fn to_shell(message: ToShell) {
    let Ok(json) = serde_json::to_string(&message) else {
        return;
    };
    with_service(|env| {
        let json = env.new_string(&json)?;
        SlotService::to_shell(env, &json)
    });
}

/// Runs `work` with `SlotService`'s class on this thread; nothing before the slot started.
fn with_service(work: impl FnOnce(&mut Env) -> jni::errors::Result<()>) {
    let Some(vm) = VM.get() else { return };
    if let Err(e) = crate::attach(vm, work) {
        tracing::warn!("Slot {:?}: couldn't reach SlotService: {e}", slot());
    }
}

/// The slot's notification (with its Pause, Stop or Start) and wake lock, every 2 s when they
/// changed: the CPU stays awake while the tab's script runs. And the tabs log for crash reports.
fn start_status() {
    let spawned = std::thread::Builder::new()
        .name("Turu Slot Status".into())
        .spawn(|| {
            let mut shown = None;
            let crash_dir = turu_frontend::crash_reports::dir();
            loop {
                // The tabs log, for a crash report from this slot (as the app's service wrote it).
                turu_frontend::tabs_log::record(&crash_dir);
                let now = crate::notify::status();
                if shown.as_ref() != Some(&now) {
                    with_service(|env| {
                        let title = env.new_string(&now.title)?;
                        let text = env.new_string(&now.text)?;
                        SlotService::status(env, &title, &text, now.running, now.paused, now.startable, now.awake)
                    });
                    shown = Some(now);
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    if let Err(e) = spawned {
        tracing::error!(
            "Slot {:?}: couldn't start the notification updates: {e}",
            slot()
        );
    }
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
            let _ = VM.set(env.get_java_vm()?);
            crate::notify::set_slot_vm(env.get_java_vm()?);
            tracing::info!("Slot {slot}: process {}", std::process::id());
            crate::java::check(env, &jni::refs::LoaderContext::FromObject(&context));
            let context = env.new_local_ref(&context)?;
            if let Err(e) = rustls_platform_verifier::android::init_with_env(env, context) {
                log::error!("Couldn't set up certificate checks; HTTPS will fail: {e}");
            }
            crate::start_engine(std::path::PathBuf::from(files_dir.try_to_string(env)?));
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `SlotService.nativeShellBound()`: a shell bound this slot: it says hello (with its control
/// API) and its status again.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeShellBound<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    hello();
    turu_frontend::client_link::resend_status();
}

/// `SlotService.nativeFromShell(json)`: a message from the shell (`turu_frontend::shell`).
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeFromShell<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    json: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let json = json.try_to_string(env)?;
            match serde_json::from_str::<ToClient>(&json) {
                Ok(ToClient::Tabs { tabs, shown, you }) => {
                    turu_frontend::client_link::set_tabs(tabs, shown, you);
                    crate::run_on_engine(|_| crate::with_screen(|screen| screen.request_repaint()));
                }
                // The text fields above the keyboard are the shell's activity's.
                Ok(ToClient::FieldEdited {
                    field,
                    text,
                    start,
                    end,
                }) => match field {
                    FieldId::Game => crate::game_field::edited(text, start as usize, end as usize),
                    FieldId::Ui => crate::ui_field::edited(Contents {
                        text,
                        start: start as usize,
                        end: end as usize,
                    }),
                },
                Ok(ToClient::FieldEnter { field }) => match field {
                    FieldId::Game => crate::game_field::entered(),
                    FieldId::Ui => crate::ui_field::entered(),
                },
                Ok(ToClient::FieldClosed { field }) => match field {
                    FieldId::Game => crate::game_field::closed(),
                    FieldId::Ui => crate::ui_field::closed(),
                },
                Ok(ToClient::Insets {
                    keyboard,
                    cutout_left,
                    cutout_right,
                }) => {
                    // Read at each frame, as the activity's own are.
                    crate::ui::set_insets(keyboard, cutout_left, cutout_right);
                    crate::run_on_engine(|_| crate::with_screen(|screen| screen.request_repaint()));
                }
                // The answers to what the shell's activity did for this slot.
                Ok(ToClient::DocumentPicked { path }) => {
                    turu_frontend::documents::picked(path.map(std::path::PathBuf::from));
                }
                Ok(ToClient::DocumentCreated { uri, path }) => turu_frontend::documents::created(
                    uri,
                    path.map(std::path::PathBuf::from),
                    crate::saf::write_document,
                ),
                Ok(ToClient::Installed { status, message }) => {
                    turu::package_install_finished(status, message);
                }
                // Keys the shell's activity got, read with its device's key map.
                Ok(ToClient::AndroidKey {
                    code,
                    down,
                    meta,
                    typed,
                    letter,
                }) => {
                    let read = crate::input::KeyRead {
                        code,
                        down,
                        meta,
                        typed,
                        letter,
                    };
                    if let Some(key) = read.key() {
                        crate::run_on_engine(move |player| {
                            crate::with_screen(|screen| screen.key(player, &key))
                        });
                    }
                }
                // The display's refresh rate, from the shell's activity.
                Ok(ToClient::RefreshRate { millihertz }) => crate::ui::set_refresh_millihertz(millihertz),
                // Another tab turned Headless Mode on or off.
                Ok(ToClient::HeadlessMode { on }) => {
                    turu_frontend::client_link::set_headless_mode(on);
                }
                // What the soft keyboard committed without keys (voice, an emoji).
                Ok(ToClient::Text { text }) => {
                    let keys: Vec<_> = text.chars().flat_map(crate::input::typed).collect();
                    crate::run_on_engine(move |player| {
                        crate::with_screen(|screen| {
                            keys.iter().for_each(|key| screen.key(player, key))
                        })
                    });
                }
                // The surface, touches and size come over Binder (`SlotService`), not as messages.
                Ok(_) => {}
                Err(e) => tracing::warn!(
                    "Slot {:?}: a message from the shell I can't read: {e}",
                    slot()
                ),
            }
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

/// How long a slot's engine has to let go of the surface: a slot still starting (building its
/// player, loading turu.swf) gets to it only after that step. The caller waits off the main thread.
const DETACH_LIMIT: std::time::Duration = std::time::Duration::from_secs(30);

/// `SlotService.nativeDetach()`: stops drawing in the surface and lets it go. 1 once the engine
/// has dropped it (the surface can then go to another slot), 0 when it didn't in [`DETACH_LIMIT`].
/// Blocks until then: called on a thread of its own.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_SlotService_nativeDetach<'local>(
    _unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jint {
    let started = std::time::Instant::now();
    let answered = crate::detach_surface(DETACH_LIMIT);
    tracing::info!(
        "Slot {:?}: surface detached (answered: {answered}, after {} ms)",
        slot(),
        started.elapsed().as_millis()
    );
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
