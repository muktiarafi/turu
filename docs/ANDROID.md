# Turu on Android

How to build, run and check the Android app. The design and the plan are in
`docs/ANDROID_PROPOSAL.md`.

> **Status (phase 4):** the game on screen (Vulkan) with Turu's nav bar and bot windows over it,
> touch, keys, the soft keyboard and sound, full screen, and kept running in the background: a
> foreground service with the script's state and Stop / Pause in its notification, a wake lock
> while a script runs, a one-time prompt to leave Turu out of battery optimisation, and a
> restart without a window after Android kills the process (it logs back in and runs
> `turu-args.txt`'s script). On an Android 16 arm64 emulator (2026-11-03): scripts found,
> loaded, configured, started and stopped by touch; farming went on with the screen off in deep
> Doze; Pause and Resume from the notification worked; after the process was killed the engine
> came back alone, logged in and farmed, and the app showed it when opened. Not yet run on a
> phone.
>
> **Known gaps:** the soft keyboard covers the game's own text fields (close it to see the
> text; Turu's text boxes near the top stay visible); no file dialogs (Load Script and the
> other dialogs do nothing: scripts come from Search Scripts, or `adb push` into `scripts/`);
> the windows are the desktop's, at the phone's density (no drawer or full-screen sheets yet);
> OpenGL ES draws only parts of the game (use Vulkan, the default); after a restart only
> `turu-args.txt`'s script runs (not one started by hand); phone makers' own battery managers
> (Xiaomi, Samsung, Huawei) can still kill the service: see their settings.

## What's where

| Path | What |
|---|---|
| `android/Cargo.toml`, `android/src/lib.rs` | crate `turu_android`: `android_main`, the engine thread, logcat, `turu-args.txt`. `screen.rs` (the GPU, the game's texture, Turu's egui UI over it, who gets a touch or a key), `input.rs` (touch, keys), `audio.rs` (AAudio, from ruffle-android), `ui.rs` (the game's soft keyboard), `platform.rs` (the bot windows' `Platform`: the browser; no file dialogs yet). Empty on other systems. |
| `android/src/notify.rs` | `TuruService`'s native methods (start the engine, the notification's buttons) and the status it shows, sent every 2 s when it changed. |
| `android/app/` | the Gradle app module: manifest (landscape), `TuruActivity` (a NativeActivity: full screen, Back to the background, asks once for notifications and battery optimisation, starts the service), `TuruService` (the foreground service: starts the engine, also alone after a kill; the notification; the wake lock), the `cargoNdk` task that builds the crate into `app/src/main/jniLibs/`. |
| `android/rustls-platform-verifier/` | Gradle module building the Kotlin half of `rustls-platform-verifier` (HTTPS certificate checks against Android's trust store) from the submodule `android/vendor/rustls-platform-verifier`. |
| `frontend/src/engine.rs` | the windowless player loop, shared with the desktop's `--headless`. |

## Build

Needs the Android SDK with platform 36, build-tools 36, NDK 27.3.13750724, JDK 17 or newer,
`cargo-ndk`, the Rust targets, and libclang (rquickjs generates its QuickJS bindings for Android
with bindgen). On macOS:

```bash
brew install --cask android-commandlinetools
```

```bash
sdkmanager --sdk_root=$HOME/Library/Android/sdk "platform-tools" "build-tools;36.0.0" "platforms;android-36" "ndk;27.3.13750724"
```

```bash
rustup target add aarch64-linux-android x86_64-linux-android && cargo install cargo-ndk --locked
```

```bash
git submodule update --init android/vendor/rustls-platform-verifier
```

libclang comes from the Command Line Tools on macOS (`xcode-select --install`); elsewhere set
`LIBCLANG_PATH`. Then, with `sdk.dir` in `android/local.properties`:

```bash
cd android && ./gradlew assembleDebug
```

The APK is `android/app/build/outputs/apk/debug/app-debug.apk`. It builds arm64-v8a and x86_64
by default; `ndkAbis=arm64-v8a` in `android/local.properties` builds one (faster). The Rust
library is always a release build: a debug engine is too slow to play.

## Releases

The release workflow (`.github/workflows/release.yml`, on a `v*` tag) builds
`turu-<version>-android-arm64.apk` with `./gradlew assembleRelease -PturuVersion=<version>`.
`turuVersion` sets the APK's version name, and its version code comes from it (0.2.0-alpha.1 is
200101: see `versionCode` in `app/build.gradle.kts`), so each release updates the one before.

Every release must be signed with the same key, or a phone refuses the update. The key lives in
the repository's Actions secrets: `ANDROID_KEYSTORE` (the keystore, base64),
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`. A tag build fails
without them; a run by hand signs with a debug key. Make the keystore once and keep a copy
somewhere safe (losing it means users uninstall to update):

```bash
keytool -genkeypair -keystore turu-release.jks -alias turu -keyalg RSA -keysize 4096 -validity 10000
```

```bash
base64 -i turu-release.jks | gh secret set ANDROID_KEYSTORE --repo muktiarafi/turu
```

```bash
gh secret set ANDROID_KEYSTORE_PASSWORD --repo muktiarafi/turu
```

```bash
gh secret set ANDROID_KEY_ALIAS --repo muktiarafi/turu --body turu
```

```bash
gh secret set ANDROID_KEY_PASSWORD --repo muktiarafi/turu
```

keytool makes a PKCS12 keystore, whose key password is the keystore's: give both secrets the
same one. `gh secret set` without `--body` asks for the value, so it stays out of the shell
history.

Locally, the same environment variables the workflow sets (`TURU_KEYSTORE` with the file's
path, `TURU_KEYSTORE_PASSWORD`, `TURU_KEY_ALIAS`, `TURU_KEY_PASSWORD`) sign `assembleRelease`
with it; without them it's signed with the debug key.

## Run on an emulator

```bash
sdkmanager --sdk_root=$HOME/Library/Android/sdk "emulator" "system-images;android-36;google_apis;arm64-v8a" "cmdline-tools;latest"
```

```bash
$HOME/Library/Android/sdk/cmdline-tools/latest/bin/avdmanager create avd -n turu_api36 -k "system-images;android-36;google_apis;arm64-v8a" -d pixel_7
```

```bash
$HOME/Library/Android/sdk/emulator/emulator -avd turu_api36 -gpu host
```

```bash
adb install -r android/app/build/outputs/apk/debug/app-debug.apk && adb shell am start -n com.muktiarafi.turu/.TuruActivity
```

## Setup: `turu-args.txt`

The data folder is the app's files folder on shared storage,
`/sdcard/Android/data/com.muktiarafi.turu/files` (Turu's usual data folder layout: `scripts/`,
`options/`, `options.json`, ...). The app reads `turu-args.txt` there at start: the desktop's
startup flags (`docs/HEADLESS.md`), **one argument per line**, `#` for comments, plus
`--graphics auto|vulkan|gl` (auto: Vulkan, else OpenGL ES). Without the file the game starts at
its login screen:

```
--user
YourName
--password
your password
--server
Twilly
--run-script
farm/GoldFarm
--api=0.0.0.0:8791
```

```bash
adb push turu-args.txt /sdcard/Android/data/com.muktiarafi.turu/files/
```

Restart the app after changing it (`adb shell am force-stop com.muktiarafi.turu`, then start it).
The password sits in that file in the clear, readable with `adb`; this is for testing until
phase 3 keeps it in the Android Keystore.

## Watch it

The log is in logcat under the tag `turu`, with the desktop's default filter:

```bash
adb logcat -s turu
```

## Crash reports

logcat's buffer is gone by the time someone says "it crashed", so Turu keeps its own record
in the data folder's `crash/` (`/sdcard/Android/data/com.muktiarafi.turu/files/crash/`):

* `exit-<time>-<reason>.txt`: at each start, `CrashReports.kt` reads Android's record of how
  the earlier processes ended (`ActivityManager.getHistoricalProcessExitReasons`, Android 11 and
  later) and writes one for each exit nobody asked for: low memory, a native crash, a hang
  (ANR, with its trace), a kill by the system or a battery saver (`SIGNALED`, `OTHER`), with the
  memory at the end, the phone and whether battery optimisation is off. Force stop, an update and
  a clean exit are left out. A native crash (Android 12 and later) comes with its tombstone,
  read into text by `Tombstone.kt` (signal, abort message, backtraces, the end of the log), and
  the raw `-tombstone.pb` next to it. Android keeps these records across reboots and updates, but
  not across an uninstall: a tester who saw a crash should install the new build over the old one.
* `panic-<time>.txt`: a Rust panic's message and backtrace, written by the panic hook before the
  abort (the abort then shows up as an `exit-...-crash_native` report too).

The first start after a new report shows "Turu closed unexpectedly"; Diagnostics > Logs >
Crashes lists them, with Share (Android's share sheet: mail, a chat, Drive), Copy and Delete. The
Logs tabs have Share too. From a computer: `adb pull
/sdcard/Android/data/com.muktiarafi.turu/files/crash`, or `adb shell dumpsys activity exit-info
com.muktiarafi.turu` for Android's own list.

The APK's library is stripped, so Turu's frames show only an address (`pc 0000000001e8d904
.../base.apk`). Read them with the unstripped library of the same build: the release workflow
keeps it as the `symbols-android-arm64` artifact (90 days), a local build has it in
`android/app/src/main/jniLibs/arm64-v8a/`:

```bash
$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin/llvm-symbolizer --obj=libturu_android.so 0x1e8d904 0x1e89568
```

### Sent to Sentry

Builds made with a Sentry DSN also send crash reports to the developers, once the user agrees:
the first start asks "Send crash reports?" (closing it asks again next time), and Options >
Application > Send Crash Reports changes it (`sendCrashReports` in `settings.json`, applied at
once). Nothing is sent before a yes; builds without a DSN never ask. `CrashUploads.kt` starts
Sentry (`io.sentry:sentry-android`, auto-init off in the manifest):

* Native and Java crashes are caught as they happen (Sentry's NDK handler), so a phone that
  keeps its tombstones to itself (the Redmi 10's MIUI) still reports, and merged with the
  tombstone where Android gives one (Android 12+, raw tombstone attached). ANRs come from
  Android's exit records. Reports are sent at the next start.
* A Rust panic's message goes with the native crash (`TuruService.panicked`, from the panic
  hook: a breadcrumb and the `panic` extra).
* Exits Sentry doesn't catch (low memory, a kill by the system or a battery saver) are sent as
  `CrashReports`' report, a warning tagged `exit_reason`.
* No personal data: `sendDefaultPii` off (no IP address), no screenshots, no logs. Sessions are
  counted (crash-free rate) under an anonymous install ID.

Setup, in the GitHub repository: the `SENTRY_DSN` secret (the project's DSN; the release build
passes it to Gradle), and for readable native frames the `SENTRY_AUTH_TOKEN` secret (an
organization auth token) with the `SENTRY_ORG` and `SENTRY_PROJECT` variables: the release
workflow then uploads `libturu_android.so`'s symbols with `sentry-cli`. A local build takes the
DSN from `-PsentryDsn=...`, `SENTRY_DSN` or `sentryDsn` in `android/local.properties`.

## GPU memory

A phone's GPU shares its RAM, and running out of GPU memory aborts Turu: a Redmi 10 (Mali-G52,
4 GB) crashed logging in, wgpu panicking on a 64-byte buffer that came back invalid
(`Globals::new`, from `Player::render`). Mugetsu's desktop memory watch reads the GPU's own
budget, which Android doesn't give, so `android/src/memory.rs` works from the phone's memory:

* Idle pooled textures are capped at Mugetsu's low-memory budget, 64 MB, for the whole run
  (`set_memory_pressure(true)`; the "idle texture budget" log line still prints the normal one).
* Every minute, the geometry of shapes not drawn for 5 minutes is freed (AQW keeps every SWF it
  loaded; on a desktop this grew ~500 MB an hour).
* Spare textures and the shapes not drawn for 2 minutes are freed when the window goes away,
  when Android calls `onTrimMemory` at `RUNNING_LOW` or above (the service passes it on), and
  every 30 s while `/proc/meminfo`'s `MemAvailable` is under 15% of the phone's memory.

Multisampling is already off: Flash-style edge smoothing (`set_flash_aa`, on for Android) draws
with one sample at every stage quality. To test a trim: `adb shell am send-trim-memory
com.muktiarafi.turu RUNNING_LOW` (with the app in the background); the log says "freed spare
textures".

The control API (`--api`) works as on a desktop. From the computer:

```bash
adb forward tcp:8791 tcp:8791
```

Its token file (`api-token` in the data folder) is made readable by the app only, which `adb`
can't read on shared storage. Put your own token there before starting the app (16 characters or
more), and it's used as is:

```bash
openssl rand -hex 24 > api-token && adb shell rm -f /sdcard/Android/data/com.muktiarafi.turu/files/api-token && adb push api-token /sdcard/Android/data/com.muktiarafi.turu/files/
```

```bash
curl -H "Authorization: Bearer $(cat api-token)" http://127.0.0.1:8791/status
```

Windowless (phase 1) the app used ~164 MB (PSS) and ~4% of a CPU at AQW's title screen, and
~410 MB and 4-16% while farming. On screen at the login screen (emulator, `-gpu host`): Vulkan
~199 MB and 16-23%. The emulator translates both APIs to the Mac's GPU, so a phone will differ.
