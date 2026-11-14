# Turu on Android

How to build, run and check the Android app. The design and the plan are in
`docs/ANDROID_PROPOSAL.md`.

> **Status (phase 4):** the game on screen (Vulkan) with Turu's nav bar and bot windows over it,
> touch, keys, the soft keyboard and sound (muted while no window shows the game; Notify
> Drop's beep still sounds), full screen, and kept running in the background: a
> foreground service with the script's state and Start / Pause / Stop in its notification, a wake lock
> while a script runs, a one-time prompt to leave Turu out of battery optimisation, and a
> restart without a window after Android kills the process (it logs back in and runs
> `turu-args.txt`'s script). On an Android 16 arm64 emulator (2026-11-03): scripts found,
> loaded, configured, started and stopped by touch; farming went on with the screen off in deep
> Doze; Pause and Resume from the notification worked; after the process was killed the engine
> came back alone, logged in and farmed, and the app showed it when opened. On a phone: a Redmi
> 10 (Mali-G52, 4 GB) ran out of GPU memory logging in, which led to the limits in "GPU memory"
> below; a full farming run on a phone isn't recorded yet.
>
> **Tabs, Army and Butler (phase 5, 2026-11-07; run on a MuMu Player 12 emulator, Android 15 x86_64):**
> several game clients in tabs, as on the desktop: the tab strip's "+", Army > Accounts ("In a
> tab" only; "Log in by hand" without a saved password) and Army Control's Show open and select
> them; one engine thread runs them all (`turu_frontend::players`), the active tab drawn and
> heard, the others muted and ticked at the hidden tab's rate. Options > Application > Tab Limit
> (default 4, at most 4) refuses more, with a log line. The Army menu and Plugins > Butler show
> (`Platform::has_army`), Butler and Army Control as full-screen sheets with finger-sized
> buttons. The wake lock is held while any tab's script runs and the notification has a line per
> tab (Pause and Stop act on the active one). The tabs after the first are written to
> `android-tabs.json` in the data folder (account, server, script, whether it ran; no password)
> and reopened when the engine starts, after `turu-args.txt`'s.
>
> **Checked on MuMu (2026-11-07, at the login screen, no account logged in):** a second tab
> from Army > New Tab, switching between the two, closing one, and the second reopened from
> `android-tabs.json` after a restart (two clients at the login screen: ~245 MB PSS); the
> Army menu, Army Control's sheet and its Map Travel window, a map preset saved to
> `army-control-jump.json`; Butler and its Send to player pop-up; Accounts' server list with
> each server's ping; Advanced Stats, Notification Center ("Script started/stopped: name"),
> Junk Items and Game Options at phone width; Load Script through the SAF picker (the script
> copied to `scripts/Imported/`); HotKeys from MuMu's keyboard (F9 opened Load Script, F10
> started the loaded script, Escape closed a dialog). Army Queue is hidden on Android: its
> squads run as processes of their own, which Android can't start.
>
> **Checked on MuMu, logged in (2026-11-07):** typing the username and password into the
> game's login fields with MuMu's keyboard (the soft keyboard is now shown on the UI thread:
> Android 15 refused it from the engine thread); a script joining a private room
> (`greenguardwest-93101`), killing Slimes with the class's saved rotation, `monsterKilled`,
> `itemDropped` and `extensionPacket` events in the script, and the drop grabber asking for
> its drops (the server refused them: already full or banked on that account), and
> `bot.drops.pickup` landing one (Bludrut's Dark Energy: `getDrop` succeeded, an AC item,
> into the bank); a death counted once (`playerDeath`, at Ultra Engineer); Fast Travel to
> Tercessuinotlim (it asks for room 111111, as VibeSkua; the server put it in another room,
> 111111 being full); Army Control > Map Travel jumping to a saved preset (`yulgar`), the
> recent list filling in.
>
> **Unverified on a device:** Butler with a master and a follower and an army script across
> two tabs (they need a second account); using a boost (the lookup runs: no boost in that
> account's inventory, none active); sound switching with
> the active tab; the notification's lines and the wake lock with two tabs; a script that
> starts once its tab is logged in by hand; `onTrimMemory` stopping hidden tabs' scripts (not
> done).
>
> **Known gaps:** the soft keyboard covers the game's own text fields (close it to see the
> text; a native text field above it holds what they hold, with Android's caret, selection
> handles and text toolbar); the windows are the desktop's, at the phone's
> density: a window too big for the screen opens as a full-screen sheet, and the side bar hides
> (its logo, or Menu > Hide Side Bar; the `›` tab at the left edge brings it back; it stays
> hidden when Turu opens again: `sideBarHidden` in `settings.json`); OpenGL ES draws only parts of the game (use
> Vulkan, the default); after a restart a Scheduler playlist started by hand doesn't come back
> (a script does); phone makers' own battery managers (Xiaomi, Samsung, Huawei) can still kill
> the service: see their settings.
>
> **Quick controls (2026-11-08, checked on MuMu at the login screen):** the notification has
> Start while the active tab's script is loaded and stopped (silent: the saved options, no
> Script Options window), besides Pause and Stop while it runs. The Script Loader's ★ beside the
> script's name makes it a favourite; once there are favourites or recently started scripts
> (the last 6, any start: by hand, the notification, the Scheduler, the API), a tap on the side
> bar's script chip lists them under the Script Loader, and a tap on one loads and starts it.
> They're kept in `quick-scripts.json` in the data folder, shared by the tabs. Checked: the
> side bar hidden with its logo stayed hidden after a force stop and a new start, and came back
> with the `›` tab; the chip's list started a recent script (`turu_long`), its ★ in the Script
> Loader moved it under Favourites; the notification read "turu_long loaded" with Start, which
> started it (then Pause and Stop), and Stop brought Start back.
>
> **Touch controls (2026-11-08, removed 2026-11-11):** Turu's own joystick and action bar
> buttons over the game (Menu > Show / Edit Touch Controls, `touch-controls.json`) are gone:
> AQW's own mobile joystick and buttons (`liteAssets.mobile`) do the same, in Turu and in Turu
> Mas (docs/TURU_MAS.md). A `touch-controls.json` left in the data folder is unused. The rail's Menu and the
> script chip's list now scroll: on MuMu both were taller than the screen, which hid Hide Side
> Bar, and the chip list's top.
>
> **Scripts for the map (2026-11-08):** joining a map puts a badge on the side bar's script
> chip with how many scripts are made for it, and the chip's list has them under "On <map>"
> (scrolling), the scripts naming the map most first; a tap loads and starts one, and one not
> downloaded yet (marked ↓, from the index's `maps`) is downloaded first. A script's maps are
> the ones its source names in a call that takes a map (`huntMonster`, `killMonster`, `join`,
> ...; `turu_bot::map_scripts`, and the scripts repository's `tools/build-index.mjs` the same
> way); a script that takes its map from a variable or its options isn't found, and maps
> scripts only pass through (`whitemap`, `battleon`, `yulgar`, `house`) list none. The
> scripts folder is read once in the background (MuMu: 1869 files, 809 maps, 636 ms) and again
> after Search Scripts changes it. On MuMu, `tercessuinotlim` showed 24 on the chip and the
> list "On tercessuinotlim · 24" under the Script Loader; a download from it is not checked on
> a device yet.
>
> **High frame rates (2026-11-08):** Options > Game > Set FPS goes up to 240, with a ▾ of the
> usual rates (Off, 24, 30, 60, 90, 120, 144, 165, 240). Above 60 the screen asks the activity
> for a display mode that keeps up (`TuruActivity.preferRefreshRate`: the slowest refresh rate
> at least that fast, at the same resolution, else the fastest; at 60 or less the system
> chooses again), since many 90 and 120 Hz phones hold an app at 60 Hz otherwise. When the game
> still ticks faster than the display, the screen shows the newest picture once a refresh
> instead of waiting for vsync after every tick, so the game keeps its rate. On MuMu (one
> 480 Hz mode, so the mode switch itself isn't exercised; the request is logged): 120 FPS ran
> at 121-123 in an empty room (`whitemap`), 24 FPS at 23.9; crowded Battleon (10 players) ran
> at 14-16 FPS whatever the setting, the engine thread taking a whole core of the emulator.
>
> **Without a computer (2026-11-07, compiled, not yet tried on a device):** file dialogs are the
> system picker (`saf.rs`): Load Script, Junk Items' and Loadouts' import and export, the
> Scheduler, the Interceptor's export and the logs' Save work as on a desktop (a picked script
> is copied into `scripts/Imported/`, other files into the app's cache). Accounts keeps
> passwords in the Android Keystore (`storage.rs`, `Storage.kt`) and logs in by itself (once
> tabs bring back the Army menu). A Turu
> text box is typed in through a native field above the keyboard, as the game's are (`ui_field.rs`,
> docs/ANDROID_HANDOVER.md). After a kill the
> script started by hand comes back too, with its options and the account (`restart.rs`).

## What's where

| Path | What |
|---|---|
| `android/Cargo.toml`, `android/src/lib.rs` | crate `turu_android`: `android_main`, the engine thread, logcat, `turu-args.txt`. `screen.rs` (the GPU, the game's texture, Turu's egui UI over it, who gets a touch or a key), `input.rs` (touch, keys), `audio.rs` (AAudio, from ruffle-android), `ui.rs` (the game's soft keyboard), `platform.rs` (the bot windows' `Platform`: the browser; no file dialogs yet). Empty on other systems. |
| `android/src/storage.rs`, `android/app/.../Storage.kt` | account passwords (Accounts' `SecretStore`): AES-GCM with a key in the Android Keystore, in the app's internal storage (`noBackupFilesDir/passwords`, not the shared data folder); copying and writing the picker's documents. |
| `android/src/restart.rs` | the script started by hand, kept in the data folder's `android-restart.txt` while it runs (its options storage, the account and server; the password, when Accounts hasn't it, with the Keystore's key), run again when the service comes back without an activity after a kill (`turu-args.txt`'s own script wins). |
| `android/src/notify.rs` | `TuruService`'s native methods (start the engine, the notification's Start, Pause and Stop) and the status it shows (a line per tab with several), sent every 2 s when it changed; alerts (a watched drop, a stopped script) as notifications of their own (`alert`, docs/ANDROID_NATIVE_UI.md, "Alerts"). |
| `frontend/src/saved_tabs.rs` | `android-tabs.json`: the tabs after the first, written when they change and reopened when the engine starts (Turu Mas keeps its own the same way). |
| `android/app/` | the Gradle app module: manifest (landscape), `TuruActivity` (a GameActivity, AndroidX's: full screen, Back to the background, asks once for notifications and battery optimisation, starts the service), `TuruService` (the foreground service: starts the engine, also alone after a kill; the notification; the wake lock), the `cargoNdk` task that builds the crate into `app/src/main/jniLibs/`. |
| `android/shared/` | Gradle module `:shared` (package `com.muktiarafi.turu.shared`): the Kotlin Turu Mas's extension uses too (`docs/ANDROID_NATIVE_UI.md`): `CrashReports` and `Tombstone` (how earlier runs ended), `CrashUploads` (Sentry), `CrashQuestion` ("Send crash reports?"), `NativeField` (the text field above the keyboard; Turu's is `SurfaceField`), each app describing itself with a `CrashApp` (Turu's in `TuruService.kt`). Java 8 class files, for AIR's `d8`. |
| `android/rustls-platform-verifier/` | Gradle module building the Kotlin half of `rustls-platform-verifier` (HTTPS certificate checks against Android's trust store) from the submodule `android/vendor/rustls-platform-verifier`. |
| `frontend/src/engine.rs`, `frontend/src/players.rs` | the windowless player loop with its tabs (shared with the desktop's `--headless`, one tab there), and the tabs' players and thread state (shared with the desktop's window). |

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
`--graphics auto|vulkan|gl` (auto: Vulkan, else OpenGL ES), `--safe-drawing` (as Options >
Application > Safe Drawing: "Misdrawing" below), `--test-gpu-loss` (destroys the GPU device
20 s after the start: "A lost GPU device" below) and `--perf-log <seconds>` ("Watch it" below). Without the file the game starts at its login
screen:

```
--user
YourName
--password
your password
--server
Twilly
--run-script
farm/GoldFarm
--api=127.0.0.1:9021
```

```bash
adb push turu-args.txt /sdcard/Android/data/com.muktiarafi.turu/files/
```

Restart the app after changing it (`adb shell am force-stop com.muktiarafi.turu`, then start it).
The password sits in that file in the clear, readable with `adb`: for testing. On the phone
itself, log in on the game's screen and start the script: after a kill it comes back with
the login, kept with the Android Keystore's key. Accounts keeps its passwords the same way
(Army > Accounts, hidden on Android until tabs work).

## Watch it

The log is in logcat under the tag `turu`, with the desktop's default filter:

```bash
adb logcat -s turu
```

`--perf-log <seconds>` in `turu-args.txt` is the desktop's `TURU_PERF_LOG` (an app can't be
started with environment variables): every `seconds` the log has each tab's Flash calls and ticks
(`docs/HEADLESS.md`), and, from `android/src/frame_perf.rs`, the engine thread's time while there
is a window: game frames per second and stutters (gaps of 1.5 frames), draws (and the ones skipped,
below), and per draw rendering the game, Turu's UI, getting the next picture and presenting, then
ruffle-aqw's render timers and counters per rendered picture (the desktop's `ruffle_aqw_perf`
lines: render passes, blend layers, cacheAsBitmap redraws, new textures).

```bash
adb logcat -s turu | grep -E "engine thread|render per|per rendered"
```

`--releases-url <url>` in `turu-args.txt` is the desktop's `TURU_RELEASES_URL`: where client
updates are looked for, such as a test feed for the update window (docs/ANDROID_NATIVE_UI.md,
"Updates"). Turu Mas reads it too.

**Crowded rooms** (2026-11-08, emulator `-gpu host` on an Apple M5, battleon on Alteon, ~300 render
passes a frame from the avatars' multiply and overlay blends, the same as on the desktop): one
render of the game took ~45-50 ms (the desktop: ~8 ms), nearly all of it on the CPU in recording
and submitting those passes (`gpu_wait` 0.1 ms); the game's frames ~4-9 ms, the UI and presenting
under 1 ms. As the screen was drawn after every tick and Mugetsu's catch-up
(`Player::max_frames_per_tick`) leaves no time when a render alone is longer than a frame, the game
ran one frame per render: 14-19 FPS instead of 30, in slow motion. Now a tick's draw is skipped
while more than half a frame of game time is already waiting (`Screen::behind`; drawn at least
every 250 ms), which a render that fits in half a frame never does: 25-28 game FPS with 16-18
pictures a second there. Also, cacheAsBitmap textures freed by objects that changed size are reused
at the same size for ~1 s (ruffle-aqw `TextureRecycler`): ~4.5 new textures a frame, ~1 ms each on
the emulator's Vulkan, of which ~25% are now reused (`texture_create` 6-9 -> ~3 ms a render). A
phone's balance between the CPU's command recording and the GPU's drawing (`gpu_wait`) isn't
measured yet.

## Crash reports

logcat's buffer is gone by the time someone says "it crashed", so Turu keeps its own record
in the data folder's `crash/` (`/sdcard/Android/data/com.muktiarafi.turu/files/crash/`):

* `exit-<time>-<reason>.txt`: at each start, `CrashReports.kt` reads Android's record of how
  the earlier processes ended (`ActivityManager.getHistoricalProcessExitReasons`, Android 11 and
  later) and writes one for each exit nobody asked for: low memory, a native crash, a hang
  (ANR, with its trace), a kill by the system or a battery saver (`SIGNALED`, `OTHER`), with the
  memory at the end (PSS only when the phone's memory could hold it: an OPPO reported 20 million
  MB), the phone and whether battery optimisation is off. Force stop, an update and
  a clean exit are left out. A native crash (Android 12 and later) comes with its tombstone,
  read into text by `Tombstone.kt` (signal, abort message, backtraces, the end of the log), and
  the raw `-tombstone.pb` next to it. Android keeps these records across reboots and updates, but
  not across an uninstall: a tester who saw a crash should install the new build over the old one.
* `panic-<time>.txt`: a Rust panic's message and backtrace, written by the panic hook before the
  abort (the abort then shows up as an `exit-...-crash_native` report too).
* `memory-<pid>.log`: the running process's memory and CPU share, a reading a minute (GPU memory,
  below). An
  exit report gets its process's log under "Memory over the run", and the logs of earlier
  processes are then deleted.

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
the first start asks "Send crash reports?" in Android's own dialog (`CrashQuestion.kt`, through
`Platform::ask_crash_uploads`; the desktop asks in Turu's window; closing it asks again next
time), and Options >
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

The desktop app sends its own to a separate project (docs/STATUS.md, "Crash reports to Sentry").

## Misdrawing

A driver that draws garbage says nothing: no error, no crash report. An Infinix Hot 30i (PowerVR
GE8320, going by its spec sheet) showed streaks, stripes and blocks of colour over the login
screen and bands around the game's menus (2026-11-07). So the debug log (Diagnostics > Logs >
Debug, and its Share) starts with the GPU and its driver, as
`Graphics: Vulkan on <name> (<type>, vendor 0x...., device 0x....); driver <name> <version>`,
plus a `Graphics: ... failed` line for each API that couldn't start.

MSAA isn't the cause there: Android always draws with Flash-style edge smoothing (no MSAA, at
any of AQW's Quality settings). What can be turned off is Mugetsu's drawing speed-ups: Options >
Application > Safe Drawing (or `--safe-drawing` in `turu-args.txt`) sets the desktop's
`safe-drawing` switches (`turu_frontend::platform::SAFE_DRAWING`: no shared shapes, atlas,
cacheAsBitmap batching or pass merging, plain multiply) at the next start, saved as
`safe-drawing.txt` in the data folder; the debug log then says `Safe Drawing: ...`. If it fixes a
phone, the next step is to find which switch does it and turn it on for that GPU alone.

## GPU memory

A phone's GPU shares its RAM, and running out of GPU memory aborted Turu: a Redmi 10 (Mali-G52,
4 GB) crashed logging in, and an Infinix HOT 11S (6 GB) 1 to 6 minutes into a session (Sentry
TURU-ANDROID-3, 0.2.0-beta.2), both with wgpu panicking on a 64-byte buffer that came back invalid
(`Globals::new`, from `Player::render`). The GPU had refused the buffer and ruffle-aqw's
out-of-memory handler tolerated that, but wgpu's `create_buffer_init` then `expect`s the invalid
buffer's mapping, a panic the handler can't catch. ruffle-aqw now creates such buffers with its
own `utils::create_buffer_init`, which leaves a buffer that failed unfilled (it draws nothing),
and `TexturePool::get_globals` frees idle textures and retries once when the GPU is out of memory
for one. ruffle-aqw's GPU warnings (`ruffle_aqw_perf`, at most 100) also go to Sentry as
breadcrumbs, so a later crash report shows the out-of-memory errors before it. Mugetsu's desktop
memory watch reads the GPU's own budget, which Android doesn't give, so `android/src/memory.rs`
works from the phone's memory:

* Idle pooled textures are capped at Mugetsu's low-memory budget, 64 MB, for the whole run
  (`set_memory_pressure(true)`; the "idle texture budget" log line still prints the normal one).
* Every minute, the geometry of shapes not drawn for 5 minutes is freed (AQW keeps every SWF it
  loaded; on a desktop this grew ~500 MB an hour).
* Spare textures and the shapes not drawn for 2 minutes are freed when the window goes away,
  when Android calls `onTrimMemory` at `RUNNING_LOW` or above (the service passes it on), and
  every 30 s while `/proc/meminfo`'s `MemAvailable` is under 15% of the phone's memory.
* The same trim runs, at most every 2 minutes, while Turu itself is over its budget
  (`process_memory::over_budget`): RSS above the larger of 1 GB and 20% of the phone's memory, or
  more than 30 000 mappings (half the kernel's usual limit, which apps can't read). Going over and
  back are warnings, so they reach Sentry as breadcrumbs. On the emulator (with a test budget of
  300 MB) it trimmed as meant but RSS hardly moved: most of a farming client's ~470 MB is the
  game's own, which a trim can't free.
* Every 10 s the engine polls the GPU without waiting, so finished GPU work's command buffers are
  let go though the game may submit nothing new for minutes in the background (wgpu only looks
  after a submit or a poll), and ruffle-aqw's Vulkan encoders keep at most 16 spare command
  buffers: 3904 command buffers had been kept alive for good after a few minutes of farming on
  the emulator, ~500 with the fix.
* Unexplained, on the emulator only so far: 1-2 minutes after going to the background the heap
  jumps ~820 MB (~1600 new 1 MB blocks, allocated by the emulator's Vulkan driver,
  `vulkan.ranchu.so`, when a command buffer begins), in every run, and stays. It isn't the game's
  drawing: in the background ruffle-aqw records ~50 render passes per 10 s (the skill bar's 50x50
  icons, redrawn with `BitmapData.draw`), with no burst at the jump. It isn't AQW's own quality
  steps (High, Medium, Low, a minute apart as the frame rate stays at 2), which come at about the
  same time: with them blocked the jump still came. A phone's per-minute memory log will say
  whether real drivers do the same.

None of that says what grows. A Pixel 8 (8 GB, 0.2.0-beta.2) aborted after 27 minutes in the
background on a failed allocation (`handle_alloc_error` in a mouse pick's tiny matrix cache, just
the allocation that hit the limit) at 1.9 GB of RSS and 2.5 GB of PSS, with 3 GB of the phone's
memory still free: a limit of the process (the allocator's regions, or the number of mappings)
rather than the phone's. So the engine logs the process's memory every minute
(`turu_frontend::process_memory`, `memory.rs`): RSS and its parts, PSS, the `malloc` heap
(`mallinfo`: bytes handed out, above anonymous RSS while pages are untouched), swap, address
space, mappings and threads, its share of a CPU core since the reading before (from
`/proc/self/stat`) with the 4 busiest threads' shares by name (`/proc/self/task/*/stat`, e.g.
`CPU 94% of a core (Turu Engine 60%, Script Thread 25%)`; threads that call into Java attach
under their own name, `crate::attach`, as Android otherwise renames them "Thread-N", and rayon's
workers are "Turu Rayon N", not their maker's name), and every 5 minutes the largest
mappings by name (e.g. `[anon:scudo:primary_reserve]`, the GPU driver's device). It goes to logcat, to
`crash/memory-<pid>.log` (the last 150 readings) for that process's exit report, and every 5
minutes to Sentry as a breadcrumb. On the emulator at the login screen: RSS ~230-300 MB, ~3500
maps; 152% of a core in the first minute (the game loading), 3% in the background.

The CPU share is for kills like Sentry's TURU-ANDROID-4: an OPPO A9 2020 (ColorOS, Android 11,
beta.2) closed Turu with `EXCESSIVE_RESOURCE_USAGE` "o-kill:6" while it ran as a foreground
service with battery optimisation off. Android's own CPU check spares foreground services, so
that was ColorOS's background manager; on OPPO, realme and OnePlus phones the exit report's hint
now points to ColorOS's own switches (App battery management: background activity, auto launch).
TURU-ANDROID-D (the same phone, beta.7, 5 kills in 8 hours) killed it at ~20% of a core, 3.5
hours in: the log showed 11-17% in the background at 2 FPS (the emulator: ~3%) and stretches of
~94% for 10 minutes, but not whose; the thread shares are there to say. Since then the player
also has no pointer while no window shows the game (`set_touchable` in `src/lib.rs`): Ruffle
hit-tested the whole display list for it on every update, about 50 a second whatever the frame
rate, which was 75% of the player's time at 2 FPS (measured with `--headless`, which runs the
same loop; not yet measured on a phone).

Frame interpolation (Options > Application > Frame Interpolation, off by default; saved as
`mugetsu-interpolation.txt` in the data folder) is paced as on a desktop (`turu_frontend::pacing`):
the engine loop (`engine::run_loop_paced`) calls `Screen::step` after each tick and at the times it
asks for, and the screen is presented on every refresh of the display (FIFO present waits for
vsync), with a new in-between picture on every n-th refresh, as many as the engine thread can
afford. The activity sends the display's refresh rate (`nativeRefreshRate`, again when the display
changes mode). Without interpolation the screen is drawn once after each tick, as before.

Multisampling is already off: Flash-style edge smoothing (`set_flash_aa`, on for Android) draws
with one sample at every stage quality. To test a trim: `adb shell am send-trim-memory
com.muktiarafi.turu RUNNING_LOW` (with the app in the background); the log says "freed spare
textures".

The control API (`--api`) works as on a desktop. Keep it on `127.0.0.1` and reach it through
`adb forward`, which connects to the phone's own localhost. `--api=0.0.0.0:9021` would serve it
in plain HTTP to everyone on the Wi-Fi, who could read the token off the network. To steer the
phone from another machine without adb, report to that machine's web monitor instead
(`--report-to`, `docs/HEADLESS.md`): the phone connects out, so it opens no port. From the
computer:

```bash
adb forward tcp:9021 tcp:9021
```

Its token file (`api-token` in the data folder) is made readable by the app only, which `adb`
can't read on shared storage. Put your own token there before starting the app (32 characters or
more), and it's used as is:

```bash
openssl rand -hex 24 > api-token && adb shell rm -f /sdcard/Android/data/com.muktiarafi.turu/files/api-token && adb push api-token /sdcard/Android/data/com.muktiarafi.turu/files/
```

```bash
curl -H "Authorization: Bearer $(cat api-token)" http://127.0.0.1:9021/status
```

Windowless (phase 1) the app used ~164 MB (PSS) and ~4% of a CPU at AQW's title screen, and
~410 MB and 4-16% while farming. On screen at the login screen (emulator, `-gpu host`): Vulkan
~199 MB and 16-23%. The emulator translates both APIs to the Mac's GPU, so a phone will differ.

## A lost GPU device

The GPU can drop the device: wgpu marks it lost when the driver runs out of memory or resets.
That aborted Turu: the next `Device::poll` (the engine's every 10 s, or ruffle-aqw's) panicked,
as wgpu treats any poll error but a timeout as fatal (Sentry TURU-ANDROID-7 on beta.6, a Redmi 10
2022 with 4 GB, "Error in Device::poll: ... Parent device is lost"). A device can't come back, and
the game's shapes and bitmaps live on it, so Turu now goes on without a picture:

* ruffle-aqw's wgpu-core patch makes a lost device's poll return as if its queue were empty, and
  its error handler tolerates every GPU error once the device-lost callback fired
  (`ruffle_render_wgpu::device_lost`). Frames aren't drawn, and reading an image back
  (`BitmapData.draw`) gives transparent black instead of waiting for a submission that failed.
* The screen stops drawing, so the activity shows its last picture or black. The game, its
  scripts and the notification's Stop and Pause go on.
* `memory.rs` says so once: in logcat, the debug log, the notification ("Graphics stopped:
  restart Turu to see the game") and to Sentry as a warning of its own (`gpu: lost`, one issue
  whatever the driver's message, which it carries).

Checked on the emulator with `--test-gpu-loss`: in game, the device destroyed, Turu kept running
(its memory readings went on) and the notification said so. On a phone: a Redmi 10 2022 lost its
device 5.5 minutes in on beta.8, in the foreground (Sentry TURU-ANDROID-B, "Unknown: Device is
lost"), and Turu went on with a frozen screen, as meant; it was that phone's third loss. Getting
the picture back without a restart is proposed in docs/GPU_RECOVERY_PROPOSAL.md (a new device
and player, then auto-relogin).
