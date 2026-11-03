# Turu on Android

How to build, run and check the Android app. The design and the plan are in
`docs/ANDROID_PROPOSAL.md`.

> **Status (phase 1):** no UI. The app starts the game and the bot without a window, the way
> `--headless` does on a desktop, set up from a file. Tested on an Android 16 arm64 emulator up to
> AQW's login screen: the game loads over HTTPS, Search Scripts downloads a script, it loads and
> starts (QuickJS), and the control API answers. A login and a farm haven't been run yet. The
> process only lives while Android lets it: there's no foreground service yet (phase 4), so keep
> the app on screen.

## What's where

| Path | What |
|---|---|
| `android/Cargo.toml`, `android/src/lib.rs` | crate `turu_android`: `android_main` (NativeActivity), the engine thread, logcat, `turu-args.txt`. Empty on other systems. |
| `android/app/` | the Gradle app module: manifest (NativeActivity, landscape), the `cargoNdk` task that builds the crate into `app/src/main/jniLibs/`. |
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

## Run on an emulator

```bash
sdkmanager --sdk_root=$HOME/Library/Android/sdk "emulator" "system-images;android-36;google_apis;arm64-v8a" "cmdline-tools;latest"
```

```bash
$HOME/Library/Android/sdk/cmdline-tools/latest/bin/avdmanager create avd -n turu_api36 -k "system-images;android-36;google_apis;arm64-v8a" -d pixel_7
```

```bash
$HOME/Library/Android/sdk/emulator/emulator -avd turu_api36 -no-window -no-audio
```

```bash
adb install -r android/app/build/outputs/apk/debug/app-debug.apk && adb shell am start -n com.muktiarafi.turu/android.app.NativeActivity
```

## Setup: `turu-args.txt`

The data folder is the app's files folder on shared storage,
`/sdcard/Android/data/com.muktiarafi.turu/files` (Turu's usual data folder layout: `scripts/`,
`options/`, `options.json`, ...). The app reads `turu-args.txt` there at start: the desktop's
startup flags (`docs/HEADLESS.md`), **one argument per line**, `#` for comments:

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

At AQW's title screen on the emulator the app used ~164 MB (PSS) and ~4% of a CPU.
