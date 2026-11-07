# Android port: handover

For whoever continues the Android port (a person or an agent). It says where things stand on
2026-11-03, how to pick up, and what was learned the hard way. The design and the plan are in
`docs/ANDROID_PROPOSAL.md` (with an "As built" section per phase); building and running are in
`docs/ANDROID.md`. Read `CLAUDE.md` first: its rules hold here too.

## Where it stands

Phases 0 to 4 of the proposal are built and committed, and checked on an emulator only:

| Phase | What works (emulator, Android 16, arm64) | Commit |
|---|---|---|
| 0 | `turu_frontend` crate split out of the app; `Platform`, `SecretStore`, TLS feature on `turu_bot` | `5d1bf2307` |
| 1 | The engine without a UI (null renderer), set up from `turu-args.txt`; logged in and farmed 20 min | `21baaba70`, `19d8b804b` |
| 2 | The game on screen (Vulkan), touch, keys, soft keyboard, AAudio sound, foreground service | `661e8091e` |
| 3 | Turu's nav bar and bot windows (egui) over the game; scripts found, loaded, configured, started, stopped by touch | `2442a71db`, `9c99a2256` |
| 4 | Background: notification with Pause/Resume/Stop, wake lock, battery prompt, restart without a window after a kill | `d755a7339` |

**Real phones:** a Redmi 10 (Mali-G52, 4 GB) has run it since; it crashed logging in on GPU
memory, fixed by `22d14f3f0` (docs/ANDROID.md, "GPU memory"). A full farming run on a phone
isn't recorded yet, which is the most useful next step (see "Next").

Since phase 4 (`6a35ed1e4`): **Cosmetics** works on the emulator (catalog download, equip,
outfits, colors, reset). A short window (under 480 points of content, as a phone held sideways)
gets a compact layout: the quick bar becomes a "Copy Look" tab, Reset and the status sit in the
tab row, the category chips become a drop-down. egui also gets Android's fonts from
`/system/fonts` (Roboto, symbols) after its own, as the desktop adds the system's: arrows like
the sort's "↓" were boxes before.

**Releases** build the APK too (from v0.2.0-alpha.1): the `android` job of
`.github/workflows/release.yml`, signed with the keystore in the repository's secrets (see
`docs/ANDROID.md`, Releases).

Since `b08c1f176`: **typing in the game's text fields.** While the soft keyboard is up for one,
a strip just above it shows the field's text with its caret and selection (a password as dots),
scrolled to the caret, and a count to 150 (the game's longest message; typing stops there).
`TuruActivity` passes the keyboard's height from the window's IME insets (`ui.rs`), with
`SOFT_INPUT_ADJUST_NOTHING` (set in code, see "Things that bit us"). The strip (its panel, not only
its text) keeps clear of the display cutout's left and right insets, as the keyboard does (`nativeCutout`): held sideways, a punch-hole camera (Galaxy
S24 FE) sits at the side, at about the strip's height. Backspace, Delete, the arrows, Home/End and
Ctrl+A/C/V/X now edit the field (Ruffle's `TextControl`, as the desktop sends it).

## Branches and workflow

* `android` is the feature branch, cut from `main` at `db698ca36`. Each phase gets its own
  branch from `android` (`android-phase-N`, or a name like `android-touch-fix`) and is merged
  back with `git merge --ff-only`, never a merge commit. Rebase the phase branch first if
  `android` moved on.
* `main` has none of this. Desktop `--headless` needs the ruffle-aqw fix that is on `android`
  (below); `main` wants that submodule bump at some point.
* Nothing of Turu's is pushed. ruffle-aqw's `aqw` branch is pushed (it holds `0b7de76a3`, which
  `android` points the submodule at).
* `CLAUDE.md`: don't commit unless asked. The owner says when to commit and merge.
* Untracked files in the working tree that aren't part of this work: `packets.txt`,
  `docs/PORTING_GAPS.md`. Leave them out of commits.

## Code map

| Path | What |
|---|---|
| `frontend/` | crate `turu_frontend`: what desktop and Android share (no winit, rfd or window). `engine.rs` is the windowless player loop (`EngineEvent`, `Executor`, `NoDialogs`, `run_loop`, `run_loop_with`, `run_loop_paced` (Android: draws between ticks for frame interpolation), `flash_host`), used by desktop `--headless` and Android. `pacing.rs` is frame interpolation's pacing, shared by the desktop window and Android's screen. |
| `android/src/lib.rs` | `android_main` (forwards window and input events to the engine), `start_engine` (once per process, from the activity or the service), the engine thread, `turu-args.txt`, `--graphics`. |
| `android/src/screen.rs` | The GPU (`gpu`, set up without a window), `MovieTexture` the player draws into, `Screen` (surface, blit, egui painting `TuruUi`), who gets a touch or a key. |
| `android/src/notify.rs` | `TuruService`'s native methods (`nativeStart`, `nativeAction`) and the status sent to the notification every 2 s. |
| `android/src/input.rs`, `audio.rs`, `ui.rs`, `platform.rs` | Touch and keys; AAudio (from ruffle-android); the game's soft keyboard; the bot windows' `Platform` (browser via `Intent`, no file dialogs). |
| `android/app/src/main/kotlin/com/muktiarafi/turu/` | `TuruActivity` (NativeActivity: full screen, Back to background, one-time prompts, starts the service) and `TuruService` (foreground service: starts the engine, notification, wake lock, `START_STICKY`). |
| `android/rustls-platform-verifier/` + submodule `android/vendor/rustls-platform-verifier` | The Kotlin half of HTTPS certificate checks, pinned to `252e2516`. A Gradle check compares it with `Cargo.lock`. |
| `android/app/build.gradle.kts` | The `cargoNdk` task (runs `cargo ndk` into `app/src/main/jniLibs/`, sets the bindgen env). |

## This machine's setup (macOS, Apple Silicon)

* Android SDK at `~/Library/Android/sdk` (platform 36, build-tools 36, NDK 27.3.13750724,
  emulator, `cmdline-tools;latest`, arm64 Android 36 image), Homebrew's
  `android-commandlinetools`, Gradle 9.8.2, `cargo-ndk`, Rust targets `aarch64-linux-android`
  and `x86_64-linux-android`.
* `adb` and `emulator` aren't on the PATH: call `~/Library/Android/sdk/platform-tools/adb`
  and `~/Library/Android/sdk/emulator/emulator`, or add those folders to the PATH.
* AVD `turu_api36` (Pixel 7 profile). Start it with a window and the Mac's GPU:
  `emulator -avd turu_api36 -gpu host`. Without `-gpu host` (or with `-no-window`) it renders
  in software (lavapipe, SwiftShader): fine for logic, useless for performance numbers.
* Stylus handwriting is turned off on that emulator (`settings put secure
  stylus_handwriting_enabled 0`): the Mac's mouse arrived as a stylus and Gboard showed its
  handwriting bar instead of the keyboard.
* `android/local.properties` (git-ignored) has `sdk.dir` and `ndkAbis=arm64-v8a`.
* The app on the emulator has the owner's `turu-args.txt` (auto-login and GoldFarm) and a
  control API token we put there (see "Testing").

## Build, install, run

```bash
cd android && ./gradlew assembleDebug
```

```bash
adb install -r android/app/build/outputs/apk/debug/app-debug.apk && adb shell am start -n com.muktiarafi.turu/.TuruActivity
```

Checks (desktop and Android):

```bash
cargo test -p turu -p turu_frontend --no-default-features --features turu/software_video,turu/external_video,turu/lzma,turu/jpegxr,turu/fontconfig-dlopen
```

```bash
cd android && ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/27.3.13750724 LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$HOME/Library/Android/sdk/ndk/27.3.13750724/toolchains/llvm/prebuilt/darwin-x86_64/sysroot" cargo ndk --platform 26 -t arm64-v8a clippy -p turu_android --release
```

Known clippy noise: Ruffle's and wgpu's own warnings; "initializer for `thread_local` value
can be made `const`" on thread-locals that already are (`screen.rs`, `lib.rs`,
`bot/src/control.rs`): a false positive.

## Testing on the emulator

* **Crashes:** reports in the data folder's `crash/` and Diagnostics > Logs > Crashes
  (docs/ANDROID.md, "Crash reports"). A native crash to test them: `adb shell run-as
  com.muktiarafi.turu kill -11 <pid>`, then start the app.
* **Log:** `adb logcat -s turu` (tag `turu`; the desktop's default filter). For AQW's script
  errors, change `ruffle_core::avm2=off` to `warn` in `init_logging_now` for one run (keep
  `avm_trace` off: it logs the login response with the account's details).
* **Control API:** `--api=127.0.0.1:9021` in `turu-args.txt`, then `adb forward tcp:19021
  tcp:9021` (adb reaches the phone's localhost; `0.0.0.0` would expose it to the Wi-Fi). The token file the app makes is owner-only and `adb` can't read it on shared
  storage: delete it and push your own (32+ chars) before starting the app. `/status`,
  `/snapshot`, `/script/start`, `/script/stop`, `/debug/perf` all work.
* **Screens and input:** `adb exec-out screencap -p > shot.png`; `adb shell input tap X Y`
  (screen pixels, 2400x1080 landscape), `input text`, `input keyevent`. `dumpsys input_method |
  grep mInputShown` tells whether the soft keyboard is up.
* **Background:** `input keyevent KEYCODE_HOME`; screen off `KEYCODE_SLEEP`; Doze `dumpsys
  battery unplug` + `dumpsys deviceidle force-idle` (undo: `unforce`, `battery reset`). A kill
  like Android's: `adb shell run-as com.muktiarafi.turu kill -9 <pid>`; the service restarts it
  within ~1 s.
* **Notification:** `dumpsys notification --noredact` (title, text, actions); `dumpsys power |
  grep turu:script` (wake lock).
* **The owner's account:** `turu-args.txt` at the repo root (git-ignored) holds their login.
  Never print the password, never type it into the game yourself (use the file, or
  `TURU_PASSWORD` for desktop runs), and stop scripts you started once a test is done. Don't
  start scripts that spend the account's gold or items (BuyScrolls was only loaded, never run).

## Things that bit us (keep these in mind)

* **Windowless login needs ruffle-aqw `0b7de76a3`.** With AQW's default settings (no saved
  `AQWUserPref`, as on a new phone or server) the game draws the stage into a bitmap when a map
  loads; the null renderer threw, the avatar was never made, and logins timed out with
  "avatar state: 0". Desktop with saved settings hid it.
* **Background network:** Android 16 cuts a background app's network ~17 s after Home unless a
  foreground service runs. That's why `TuruService` came in phase 2.
* **Start order:** anything that calls `turu::bot()` before `start_engine` sets the data folder
  makes the bot with a relative `Turu/` path, which Android refuses (the notification thread
  did). Start new threads after `start_engine`.
* **Vulkan allows one surface per window:** `Screen::attach` skips a window it already has a
  surface on (`ERROR_NATIVE_WINDOW_IN_USE_KHR` otherwise).
* **egui's root UI is a full-screen background layer:** touch routing counts only the nav bar
  and layers above `Order::Background`, or the game never gets a tap.
* **The game has one mouse, and AQW's touch controls use it:** the mobile joystick and skill
  buttons (`liteAssets.mobile` in the client) listen for `MOUSE_DOWN` on the control, then
  `MOUSE_MOVE`/`MOUSE_UP` on the stage, so a second finger pressing a skill used to move the mouse
  and release it, which ended the joystick's drag. `turu_frontend::touch::GameTouches` routes each
  finger by its pointer ID: the first finger on the game holds the mouse, a later one is a quick
  tap (release, tap, then press again where the holder came down and move to where it is now, so
  the joystick keeps its centre and its direction).
* **AQW's dash and keys need Flash touch events:** they dash in the joystick's direction, so the tap
  above (which lets the joystick go) dashed nowhere and only spent the stamina. They also listen for
  `TouchEvent.TOUCH_BEGIN` from a non-primary touch point when `Multitouch.supportsTouchEvents`.
  ruffle-aqw has that for Turu (`// Turu:`): `UiBackend::supports_touch_events` (true on Android),
  the native `Multitouch.supportsTouchEvents` getter, and `Player::handle_touch`, which sends a
  bubbling `touchBegin`/`touchMove`/`touchEnd` to the object under the finger. `Screen::touch`
  gives a finger that comes while another holds the mouse to `handle_touch` first, and taps with
  the mouse only when no listener took it. egui follows one finger at a time.
* **Android makes a new activity in the same process** for some configuration changes (an
  assets-path change right after boot; `configChanges` can't opt out of it). Its `android_main`
  runs again: `lib.rs`'s `APP` is replaced each time, or the keyboard and the browser go through
  the destroyed one ("Can't show soft input after NativeActivity has been destroyed").
* **egui must get touches as touches:** `input.rs` sends each touch as an `egui::Event::Touch`
  as well as the mouse events it stands for (as egui-winit does). With mouse events only, egui
  takes the screen for a mouse's, and `ScrollArea`'s drag-to-scroll (on touch screens only)
  never ran: no window scrolled with a finger. A drag that starts on a DragValue changes the
  number instead of scrolling, so every number box in Turu's windows is a `widgets::number`
  (a DragValue with a pointer, tap to type on a touch screen; `NumberFormat` for decimals or a
  label for 0). Don't add a bare `egui::DragValue`.
* **Text fields need `TextControl`:** Ruffle edits a field from `PlayerEvent::TextControl`
  (Backspace, arrows, Ctrl+V), not from `KeyDown`. With Ctrl, Android's key character map types
  nothing, so `input.rs` takes the key's unmodified letter for Ctrl+A/C/V/X.
* **A tap on the keyboard's delete did nothing until delete was held:** without a text view,
  Android gives the keyboard a stand-in connection that turns typed text into keys but drops
  "delete before the cursor", which a keyboard sends while it thinks there's text. `KeyInput.kt`
  (an invisible focused view, `TYPE_NULL`: key events only) is the keyboard's connection now; it
  accepts NativeActivity's show/hide (`checkInputConnectionProxy`) and turns delete requests into
  keys. No suggestions or swipe typing, as in a terminal.
* **The keyboard came up at every start and return from the background:** NativeActivity's
  `onCreate` sets the window's soft input mode to "unspecified, resize", over the manifest's, and
  with `KeyInput` focused as a text editor Android shows the keyboard whenever the window comes
  to the front (`SHOW_AUTO_EDITOR_FORWARD_NAV` in `dumpsys input_method`). `TuruActivity` sets
  "always hidden, nothing" after `super.onCreate`; check it with `dumpsys window windows`
  (`sim={state=always_hidden adjust=nothing}`).
* **The emulator's soft keyboard** stays hidden (an IME of zero height) while the Mac's keyboard
  counts as a hardware one: turn on "show on-screen keyboard" (`settings put secure
  show_ime_with_hard_keyboard 1`) and restart Gboard or the emulator.
* **Back with target SDK 36** goes to the activity's back callback, not to native code:
  `TuruActivity` handles it (moves the task back).
* **Edge to edge** (Android 15+): the status bar drew over the nav bar; the activity runs
  immersive full screen.
* **The game flickered, as if stretching to the whole screen** (a phone, after logging in): with
  the default cutout mode (Android 14 and older) the window kept clear of a camera on a short
  edge only while the system bars were hidden, so the keyboard or a swipe from an edge changed
  the window's size and the cutout's inset (the rail's width), and with them the game's area,
  whose texture is made anew and empty on a resize. `TuruActivity` sets the cutout mode to
  short edges and `setDecorFitsSystemWindows(false)`, and `Screen::draw` renders the game in a
  frame that resized it. A Samsung S24 FE still flickered: every window event (resized, content rect or
  configuration changed, as insets come and go) reconfigured the surface, which drops its
  pictures; `Screen::resize` now does that only for a new size, and draws right away.
* **The app's code is Kotlin, called from Rust over JNI** (package `com.muktiarafi.turu`, so the
  native functions are `Java_com_muktiarafi_turu_<Class>_<method>`). What Rust calls or
  implements must stay `@JvmStatic` in the class's `companion object` (`TuruService.status`,
  `panicked`, `crashUploadsAvailable`, `setCrashUploads`, the `native*` externals), the methods
  Rust calls not `internal` (their JVM names get mangled), and with the same parameter types, or
  the call fails at run time (`NoSuchMethodError`, `UnsatisfiedLinkError`), not at build time.
  Keep the app ID (`applicationId`) as it is: a new one is a separate app to Android, installed
  next to the old one with its own empty data folder, and no update for people who have it.
* **AGP 9's built-in Kotlin** compiles only `kotlin` source folders: the verifier module lists
  the upstream sources as both `java` and `kotlin`.
* **rquickjs** has no pre-made Android bindings: its `bindgen` feature, libclang from the
  Command Line Tools, the NDK's sysroot in `BINDGEN_EXTRA_CLANG_ARGS` (the Gradle task sets it).
* **TLS:** reqwest's `rustls-no-provider` + `ring` (aws-lc-rs needs CMake), and
  `rustls-platform-verifier` initialised over JNI before any HTTP request.
* **Logging:** `tracing` needs `log` and `log-always` to reach `android_logger`.
* **A notification posted before POST_NOTIFICATIONS is granted is dropped:** the activity calls
  `TuruService.refresh()` when it is.
* **`ruffle_core` can't build alone here:** its build script compiles playerglobals with Java,
  which fails with Java 25 (`ScriptCompiler` not found); build it through the app or Android
  crate, whose cached build is reused. `cargo test -p turu_frontend` alone hits it.
* **`cargo fmt -p turu` reformats ~50 upstream files:** don't. Format bot with `cargo fmt`, Turu
  UI files one by one with `rustfmt --edition 2024 --config max_width=120 <file>`; the
  Android crate was hand-formatted.

## Known gaps

* No real phone yet; no 6-hour run (phase 4's "done when").
* The strip above the keyboard is for the game's fields only; Turu's own text boxes (egui) can
  still be under the keyboard when a window sits low.
* OpenGL ES draws only parts of the game, with no error logged. Mugetsu's renderer changes were
  made on Metal, Vulkan and DX12. Vulkan is the default.
* No file dialogs (`platform.rs` refuses them): scripts come from Search Scripts or `adb push`.
  SAF import needs Kotlin in the activity (`ACTION_OPEN_DOCUMENT`, `onActivityResult`, a native
  callback).
* The desktop windows at the phone's density work, but there is no drawer or full-screen sheet
  layout yet.
* After a restart (kill), only `turu-args.txt`'s script runs, not one started by hand.
* Passwords for Accounts: `SecretStore` has no Android implementation (Keystore via JNI). Its
  `NoStore` says `keeps_passwords() == false`, so Accounts shows no password field there; a
  Keystore store brings the field back by keeping the trait's default.
* The Army menu and Plugins > Butler are hidden on Android (`Platform::has_army`) until tabs
  work there (phase 5). Accounts would start accounts only in tabs (`launches_processes`: the
  executable is app_process, not a `turu`), with the loaded script (no file dialogs).
* The data folder is on shared storage (for `adb`); private files (tokens, passwords) belong in
  internal storage.
* No device fonts from `/system/fonts` (Ruffle's `default_font` covers `_sans`).
* Phone makers' battery managers (Xiaomi, Samsung, Huawei) can still kill the service.

## Next

1. **Run it on a phone** (arm64, Android 8+): Vulkan drawing, touch and the keyboard with a
   finger, a long screen-off farm, a vendor battery manager.
2. Bring the ruffle-aqw fix to `main` (desktop `--headless` on a fresh server has the same login
   bug).
3. Log in by hand on the game's screen on a phone, now that typing shows above the keyboard.
4. SAF import and the Keystore `SecretStore` (phone use without a computer).
5. Proposal phase 5: tabs (several clients), Army scripts and Butler (steps and estimate in
   `ANDROID_PROPOSAL.md`, "Plan: tabs, Army and Butler"), a tablet layout, gamepads (Cosmetics is
   done, above).

A different direction the owner pointed at: `../aqw-python` is a bot with no Flash at all (it
speaks AQW's socket protocol and keeps its own game state). That would be far lighter on a
phone, but Turu's whole script API reads the running client, so it would mean reimplementing
most of `bot/src/api/`. Not started; worth a design note before any work.
