# Turu on Android

How to build, run and check the Android app. The design and the plan are in
`docs/ANDROID_PROPOSAL.md`.

> **Status (phase 3):** the game on screen (Vulkan) with Turu's nav bar and bot windows over it,
> touch, keys, the soft keyboard and sound, full screen. A foreground service keeps it running,
> and online, in the background; Back sends the app to the background like Home. On an Android
> 16 arm64 emulator (2026-11-03): a script was found in Search Scripts, loaded, its options
> window opened, and it was started and stopped from the Script Loader, all by touch. Not yet
> run on a phone.
>
> **Known gaps:** the soft keyboard covers the game's own text fields (close it to see the
> text; Turu's text boxes near the top stay visible); no file dialogs (Load Script and the
> other dialogs do nothing: scripts come from Search Scripts, or `adb push` into `scripts/`);
> the windows are the desktop's, at the phone's density (no drawer or full-screen sheets yet);
> OpenGL ES draws only parts of the game (use Vulkan, the default); the notification has no
> actions and no wake lock is held (phase 4).

## What's where

| Path | What |
|---|---|
| `android/Cargo.toml`, `android/src/lib.rs` | crate `turu_android`: `android_main`, the engine thread, logcat, `turu-args.txt`. `screen.rs` (the GPU, the game's texture, Turu's egui UI over it, who gets a touch or a key), `input.rs` (touch, keys), `audio.rs` (AAudio, from ruffle-android), `ui.rs` (the game's soft keyboard), `platform.rs` (the bot windows' `Platform`: the browser; no file dialogs yet). Empty on other systems. |
| `android/app/` | the Gradle app module: manifest (landscape), `TuruActivity` (a NativeActivity: full screen, Back to the background, starts `TuruService`, the foreground service), the `cargoNdk` task that builds the crate into `app/src/main/jniLibs/`. |
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
