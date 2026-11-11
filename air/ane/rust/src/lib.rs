//! Spike A0's bridge benchmark (docs/AIR_PROPOSAL.md, "The bridge: an ANE").
//!
//! The engine calls the game the way `AirFlashHost` will: from its own thread, it asks Kotlin's
//! `Bridge.dispatch` to send AIR a status event ("call", id), AS3 runs the call on AIR's thread
//! and answers with `reply(id, value)` (Kotlin's `Bridge.nativeReply`), and the engine waits for
//! that answer. [`bench`] times such round trips, and plain JNI calls into Kotlin for comparison,
//! and sends the summary as a "done" status event.

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{Global, JClass, JString, JValue};
use jni::sys::jint;
use jni::{Env, EnvUnowned, JavaVM, jni_sig, jni_str};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The JVM and Kotlin's `Bridge` class: a native thread can't look the app's classes up itself.
static BRIDGE: OnceLock<(JavaVM, Global<JClass<'static>>)> = OnceLock::new();

/// Where AS3's replies go while a benchmark waits for them.
static REPLIES: Mutex<Option<Sender<(i32, String)>>> = Mutex::new(None);

/// How long one call may wait for AS3 (as the Ruffle host's timeout).
const TIMEOUT: Duration = Duration::from_secs(5);

/// `Bridge.nativeBench(calls)`: times `calls` round trips on a thread of its own.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeBench<'local>(
    mut unowned_env: EnvUnowned<'local>,
    class: JClass<'local>,
    calls: jint,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            if BRIDGE.get().is_none() {
                let _ = BRIDGE.set((env.get_java_vm()?, env.new_global_ref(&class)?));
            }
            let calls = calls.max(1) as usize;
            let _ = std::thread::Builder::new()
                .name("Turu Bench".into())
                .spawn(move || bench(calls));
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Bridge.nativeReply(id, value)`: AS3's answer to a call.
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_muktiarafi_turu_air_Bridge_nativeReply<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jint,
    value: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let value = value.try_to_string(env)?;
            if let Some(replies) = REPLIES.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                let _ = replies.send((id, value));
            }
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

fn bench(calls: usize) {
    let Some((vm, class)) = BRIDGE.get() else {
        return;
    };
    let (sender, replies) = mpsc::channel();
    *REPLIES.lock().unwrap_or_else(|e| e.into_inner()) = Some(sender);
    let result = vm.attach_current_thread(|env: &mut Env| -> jni::errors::Result<String> {
        // Plain JNI calls into Kotlin (`Bridge.echo`): the floor under any bridge.
        let started = Instant::now();
        for i in 0..calls {
            env.call_static_method(
                class,
                jni_str!("echo"),
                jni_sig!("(I)I"),
                &[JValue::Int(i as jint)],
            )?;
        }
        let echo = started.elapsed() / calls as u32;

        let summary = match round_trips(env, class, &replies, calls)? {
            Ok(times) => summarize(calls, &times),
            Err(error) => error,
        };
        Ok(format!(
            "{summary}; JNI call {:.1} us",
            echo.as_secs_f64() * 1e6
        ))
    });
    *REPLIES.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let summary = result.unwrap_or_else(|e| format!("bench failed: {e}"));
    let _ = vm.attach_current_thread(|env: &mut Env| dispatch(env, class, "done", &summary));
}

/// Each call's time, or why the run stopped.
fn round_trips(
    env: &mut Env,
    class: &JClass,
    replies: &Receiver<(i32, String)>,
    calls: usize,
) -> jni::errors::Result<Result<Vec<Duration>, String>> {
    let mut times = Vec::with_capacity(calls);
    for i in 0..calls {
        let started = Instant::now();
        dispatch(env, class, "call", &i.to_string())?;
        loop {
            match replies.recv_timeout(TIMEOUT) {
                Ok((id, _)) if id as usize == i => break,
                Ok(_) => continue,
                Err(_) => {
                    return Ok(Err(format!(
                        "call {i} timed out after {} s",
                        TIMEOUT.as_secs()
                    )));
                }
            }
        }
        times.push(started.elapsed());
    }
    Ok(Ok(times))
}

fn summarize(calls: usize, times: &[Duration]) -> String {
    let total: Duration = times.iter().sum();
    let mut sorted = times.to_vec();
    sorted.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
    format!(
        "{calls} round trips in {:.0} ms: median {:.2} ms, p95 {:.2} ms, max {:.2} ms",
        ms(total),
        ms(at(0.5)),
        ms(at(0.95)),
        ms(*sorted.last().unwrap_or(&Duration::ZERO)),
    )
}

/// `Bridge.dispatch(code, level)`: a status event to AS3.
fn dispatch(env: &mut Env, class: &JClass, code: &str, level: &str) -> jni::errors::Result<()> {
    let code = env.new_string(code)?;
    let level = env.new_string(level)?;
    env.call_static_method(
        class,
        jni_str!("dispatch"),
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
        &[JValue::Object(&code), JValue::Object(&level)],
    )?;
    Ok(())
}
