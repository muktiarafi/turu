# Proposal: Turu on Android

Status: phases 0 and 1 built (see "As built"); phase 1 logged in and farmed on an emulator. Build and run: `docs/ANDROID.md`. Written 2026-11-03 against Turu `db698ca36`, ruffle-aqw on
Ruffle `fda41800` + Mugetsu 1.6.0, and `ruffle-rs/ruffle-android` `main` of 2026-11-02.

Read `docs/ARCHITECTURE.md` first: this doc assumes its thread model (one event loop thread
owns every `Player`; bot threads reach Flash only through `FlashHost`) and the headless mode
(`app/src/headless.rs`), which is most of what Android needs.

## Summary

* **Feasible, and cheaper than it looks.** The engine (`bot/`, ~44 k lines) has no
  dependency on the window, Ruffle or the OS beyond three spots (keychain, TLS, process
  spawning). The headless loop already runs a player without winit, with its own executor and
  `FlashHost`. What's new is a surface, touch and soft-keyboard input, Android's lifecycle and a
  touch layout for the tool windows.
* **Don't depend on ruffle-android; use it as a donor.** It's an app (`cdylib` + Kotlin
  shell), not a library, and it builds Ruffle `master` from git, while Turu needs ruffle-aqw.
  Copy its Gradle/cargo-ndk setup, AAudio backend, rustls init and JNI helpers (MIT/Apache,
  compatible with Turu's GPL-3.0) into a new `android/` crate, and write the rest against
  Turu's own player code.
* **Scope a phone Turu around running scripts.** Script Loader, Search Scripts, options, skills,
  Auto, logs, relogin, drops and the web monitor all carry over. Army (one process per
  account), the packet tools, Grabber and other script-writing windows don't fit, or are
  better on a desktop.
* **The bot must live in a foreground service, not the activity.** Android destroys the
  surface whenever the app leaves the screen. The player keeps ticking (headless-style) and
  draws only while a surface is attached.
* **Before building any of it,** check whether "watch and steer bots from my phone" is the
  real goal: the web monitor (`docs/WEB_MONITOR_PROPOSAL.md`) already does that in the
  phone's browser, with no port.

## Should Turu use ruffle-android?

What ruffle-android is (from its `Cargo.toml`, `src/` and `CONTRIBUTING.md`):

| | ruffle-android |
|---|---|
| Shape | One Rust `cdylib` (`ruffle_android`, ~1,100 lines: `lib.rs`, `audio.rs`, `java.rs`, `navigator.rs`, `keycodes.rs`) loaded by a Kotlin/Compose app (`rs.ruffle`). |
| Activity | `android-activity` 0.6 with **GameActivity**; `android_main` runs the player loop on GameActivity's native thread. |
| Ruffle | `ruffle_core`, `ruffle_render_wgpu`, `ruffle_frontend_utils`, ... as **git deps on `branch = "master"`**. |
| Renderer | `WgpuRenderBackend::for_window_unsafe(..., wgpu::Backends::GL, ...)`: OpenGL ES only; the surface is rebuilt on `InitWindow`/`Resume` (`recreate_surface_unsafe`). |
| Audio | Its own AAudio backend over `ndk` (`audio.rs`); CPAL/Oboe were dropped. |
| TLS | `rustls-tls` + `ring` + `rustls-platform-verifier` (initialised with the JNI env). |
| Storage | `DiskStorageBackend` in the app's data dir (from JNI). |
| Input | Touch as mouse, hardware keyboard. **No soft keyboard** (on its TODO list), no gamepad. |
| Build | Gradle 9.8 / AGP 9.5, cargo-ndk Gradle plugin, NDK 27, minSdk 26, targetSdk 37, ABIs armeabi-v7a, arm64-v8a, x86, x86_64. |
| State | "very early stage"; 114 open issues; actively maintained (commits on 2026-11-02). |

Ways to use it:

| Option | How | Verdict |
|---|---|---|
| A. Fork it and add Turu | Fork the repo, point its Ruffle deps at ruffle-aqw, add `turu_bot` and the UI. | No. Two forks to keep in step (ruffle-aqw and the Android fork), and its `lib.rs` is one 800-line `android_main` that we'd rewrite anyway to get the service, egui and multiple players. |
| B. Depend on it | Use it as a crate. | Not possible: it's a `cdylib` app with no library API, and its git deps on Ruffle `master` clash with `[patch.crates-io]` and ruffle-aqw's paths. |
| **C. Donor (recommended)** | New `android/` crate in this repo, built with the same tooling. Copy `audio.rs`, `java.rs`'s JNI helpers, the rustls setup and the Gradle files; credit them in the files' headers. | Yes. One Ruffle (ruffle-aqw), one workspace, and Turu's own player builder and `FlashHost`. We follow ruffle-android's commits for Android fixes (surface handling, AAudio, AGP bumps) the way `app/` follows ruffle-aqw's `desktop/`. |

## What carries over

| Layer | Today | On Android |
|---|---|---|
| `bot/` engine, `bot/js/turu.ts`, scripts | Pure Rust + QuickJS (`rquickjs`, C via `cc`) + `oxc`. | Builds for `aarch64-linux-android` with the NDK. Three changes, below: secrets, TLS, `army.rs`. |
| `skua.swf` + the Flash bridge | Embedded, written to the data dir, `ExternalInterface`. | Unchanged. |
| Ruffle + Mugetsu core/render patches | wgpu (Metal/Vulkan/DX12). | Core patches are backend-agnostic. Render patches (arena, atlas, batched filters) are wgpu-level; the `wgpu-hal` patches touch only `dx12/` and `vulkan/` (plus `lib.rs`, `dynamic/`), so they apply to Vulkan on Android and are inert on GLES. |
| Player building, executor, `FlashHost` | `app/src/player.rs` (window), `app/src/headless.rs` (no window). | `headless.rs`'s pieces (`HeadlessExecutor`, the `mpsc` loop, `DesktopFlashHost::headless`) are the model: the Android loop is that loop plus "render when a surface is attached". |
| Turu UI (`frontend/src/gui/turu/`) | egui; since phase 0 no winit, `rfd` or `webbrowser` (file dialogs and links go through `Platform`). | egui runs on Android through `egui-wgpu` with input fed by hand. The windows need a touch layout (below). |
| Control API, web monitor | hyper on its own thread. | Works. Useful both ways: steer the phone's bot from a desktop, or show the phone's bots on a desktop monitor (`--report-to`). |

## Feature matrix

**Port** = same behaviour; **Adapt** = works with an Android-specific change; **Drop** = not in
the Android build (stays on desktop); **Later** = possible, not in the first release.

| Feature (VibeSkua / Turu name) | Decision | Notes |
|---|---|---|
| Game client, login with the game's screen | Adapt | Needs the soft keyboard for Flash text fields (below). |
| Script Loader, Start/Stop/Pause, script options window | Port | Full-screen sheet instead of a floating window. |
| Search Scripts (repo download, update, pin) | Port | Over rustls. |
| Your own scripts | Adapt | Scripts folder in app-specific external storage, so a file manager or `adb push` can reach it; import a `.ts` through the system picker (SAF). No in-app editor in v1. |
| Scheduler / playlists | Port | |
| Options > Game, Application, CoreBots | Port | |
| Skills (skill sets, rotations) | Port | |
| Auto Attack / Auto Hunt, Jump | Port | Jump's arrow-key cell stepping (gap #19) needs buttons. |
| Logs, Script logs, Current Drops, Stats, Runtime | Port | |
| Auto-relogin, relogin watchdog | Port | More important on phones (network switches, Doze). |
| Webhooks (Discord) | Port | |
| Web monitor and control API | Port | Off by default; binding on LAN needs a clear on/off switch and the key, as on desktop. |
| Accounts (saved logins) | Adapt | Passwords in the Android Keystore instead of `keyring`. |
| Tabs (several clients in one process) | Later | Already in-process, so it works in principle; memory decides (one client is ~420 MB with a window on desktop). v1: one client; phase 5: two, at most four ("Plan: tabs, Army and Butler"). |
| Army (one `turu` process per account, `army.rs`) | Adapt (phase 5) | Android apps don't spawn copies of themselves: Accounts launches into tabs, and army scripts (`bot.army`) sync between tabs as between processes. Army Control of *desktop* clients from the phone is the monitor's job. |
| Butler | Later (phase 5) | Its file-based sync (`butler/<user>.state`) works within one process's tabs; useful only with tabs. |
| Cosmetics | Later | Works; large catalog UI needs its own layout. |
| Grabber, Loader, Console, Interceptor, Packet Logger, Spammer, Junk Items editor | Drop (v1) | Script-writing and debugging tools: keyboard and big screen. They compile; hide them from the phone nav. Revisit for tablets. |
| Mugetsu settings (edge smoothing, frame interpolation, show FPS) | Port | Graphics API choice becomes Auto/Vulkan/OpenGL ES. |
| Mugetsu's update check, Windows crash handler, Linux restart dialog, Dock icon, heap census | Drop | Desktop-only; already behind `cfg`s. |
| Gamepad (`gilrs`) | Later | Android gamepads come through `InputEvent` instead. |
| Headless (`--headless`), CLI flags | Adapt | No command line: the service is the headless mode. "Run in background" switch; start a script from a notification action or from the control API. |

## Architecture

### Crates

```
android/                 new crate `turu_android` (cdylib): android_main, the engine thread,
                         surface/input/egui glue, JNI, AAudio (from ruffle-android)
  app/                   Gradle project: Kotlin TuruActivity (GameActivity), TuruService
                         (foreground service), manifest, icons; cargo-ndk plugin
app/                     desktop app: window, headless loop, Mugetsu, rfd dialogs
frontend/                lib crate `turu_frontend` (built): what desktop and Android share
  turu.rs                tabs, TuruFlashHost, ExternalInterface, spawn_action; reaches the
                         player loop only through closures the app hands in
  platform.rs            `Platform`: file dialogs, browser, file manager
  capture.rs, map_items.rs, map_pads.rs, grid.rs
  gui/turu/              the egui windows
```

The seam is two closures rather than an enum of senders: `turu::set_tab_requests` (where tab
requests go) and `TuruFlashHost::new(tab, send)` (where a tab's Flash calls go). The desktop
wraps its `EventLoopProxy`, the headless loop its `mpsc::Sender`, and Android will pass its engine
thread's channel. A shared `PlayerBuilder` setup for window, headless and Android isn't done yet:
it belongs to phase 1, when the Android loop needs it.

What the egui windows ask of the OS (`frontend/src/platform.rs`):

```rust
pub trait Platform: Send + Sync {
    fn pick_file(&self, dialog: FileDialog, answer: Answer) -> bool;  // rfd / SAF
    fn save_file(&self, dialog: FileDialog, answer: Answer) -> bool;  // rfd / SAF
    fn open_url(&self, url: &str);                                    // webbrowser / Intent
    fn open_folder(&self, folder: &Path);                             // file manager / share sheet
    fn open_file(&self, path: &Path);                                 // open, xdg-open / Intent
}
```

Dialogs never block: `answer` (`Box<dyn FnOnce(Option<PathBuf>) + Send>`) is called on any
thread when the user picks, and the window polls a channel (`files::Pending`). That fits rfd's
futures on the desktop and Android's `Intent` + `onActivityResult` alike. The app installs its
platform once (`platform::set`); without one (headless, tests) nothing opens.

### Threads and lifecycle

```
Kotlin                                    Rust (one process)
──────                                    ─────────────────────────────────────────────
TuruService (foreground, notification) ─► engine thread (started once per process)
  keeps the process alive                   owns Player(s), tokio runtime, executor
                                            loop: tick; answer TuruFlashCall; poll tasks;
                                                  if surface: render player + egui
TuruActivity (GameActivity)            ─► android_main thread
  InitWindow / TerminateWindow              forwards to the engine thread:
  input, soft-keyboard text                   SurfaceAttached(NativeWindow, size)
                                              SurfaceDetached, Input(..), TextInput(..)
                                          script thread, background loops, control API,
                                          monitor: unchanged (bot/)
```

* ruffle-android runs the player on `android_main`, so the player dies with the activity. Turu
  can't allow that: a farm must survive the screen turning off or switching apps. The engine
  thread is started on first launch and owned by the process; `android_main` is a thin
  forwarder. `TuruService` starts with the first script (or with "Run in background") and
  stops when the user stops it.
* With no surface the loop is `headless.rs`'s loop: `Player::tick` doesn't need a renderer
  surface. On `SurfaceDetached` the renderer stays and only `render()` is skipped; on
  `SurfaceAttached` call `recreate_surface_unsafe` (as ruffle-android does on `Resume`). Also
  set the game's Headless Mode frame rate while hidden, as `state.hidden_tab` does for desktop
  tabs.
* All the existing rules hold: no Flash calls from `android_main` or JNI callbacks (they go
  through `spawn_action`), none inside `Bot::on_flash_call`.

### Rendering

* **wgpu backend:** try Vulkan first, GLES as fallback, with a user override (Mugetsu's
  "Graphics API" segment: Auto / Vulkan / OpenGL ES). ruffle-android forces GLES, most
  likely because Android Vulkan drivers vary; Mugetsu's Vulkan patches (memory blocks for
  cacheAsBitmap, command buffer merging) are where its desktop speed comes from. Measure both
  on a few devices (an Adreno, a Mali, an emulator) in phase 1 before choosing the default.
  Reuse Mugetsu's "Vulkan crashed before → fall back" marker idea (`mugetsu-vulkan-crashed.txt`).
* **Stage and orientation:** skua.swf's stage is 958×550, close to a landscape phone. Lock
  the activity to landscape (`sensorLandscape`), letterbox on, keep the stage scale.
* **egui on top:** `egui-wgpu` draws into the same surface pass as the movie, as the desktop
  already does. Scale egui's `pixels_per_point` from the display density, with a minimum
  touch target of ~44 dp.
* **Fonts:** no fontconfig/freetype on Android. Turn on `ruffle_core/default_font` and load
  `/system/fonts` into Ruffle's device-font fallback (Mugetsu's Helvetica→Arial chain maps to
  Roboto/Noto).
* **Video:** `ruffle_video_external` downloads OpenH264 at runtime; drop it and use
  `software_video` only (AQW barely uses video).

### Input

* **Touch → mouse:** tap is click, drag is mouse move with the button down (as in
  ruffle-android's `MotionEvent` handling). Touches that land on egui go to egui first, as the
  desktop does with the mouse.
* **Soft keyboard: the hard part.** AQW's login, chat and every text field are Flash
  `TextField`s, and ruffle-android hasn't solved it. Ruffle already calls
  `UiBackend::open_virtual_keyboard` / `close_virtual_keyboard` when an editable text field
  gets or loses focus (`ruffle/core/src/backend/ui.rs:139`); Turu's Android UI backend
  implements them with GameActivity's `GameTextInput` (`AndroidApp::show_soft_input` and the
  text input state of `android-activity`'s `game-activity` feature), and turns its edits into
  `PlayerEvent::TextInput` and key events. egui text fields use the same path when egui wants
  keyboard input.
* **Hardware keyboards** (Bluetooth, DeX, Chromebooks) work as in ruffle-android via
  `keycodes.rs`; skill hotkeys come with them.
* **Back button:** closes the top egui sheet, then the nav drawer; never quits while a script
  runs.

### UI on a phone

The desktop layout (34 px nav bar, floating egui windows over a 958×550 game) doesn't fit a
6-inch screen. Keep the windows' contents and change their frames:

* The nav bar becomes a collapsible edge strip (or a floating button) that opens a drawer with
  VibeSkua's menu groups (Scripts, Options, Tools, Combat, Bank, Plugins), minus the dropped
  windows.
* Tool windows open as full-screen sheets over the game with a close button, one at a time.
  A sheet is the existing window body, so the window code gains a "sheet" frame, not a second
  layout. Tablets (≥ 840 dp wide) can keep floating windows.
* A status chip over the game: script name, running/paused, with Stop. It mirrors the
  persistent notification.
* The notification (required for the service): script name and runtime, with Stop and
  Pause actions that call `spawn_action`.

### Storage, secrets and network

| Concern | Desktop | Android |
|---|---|---|
| Data dir (`Paths`) | `dirs::data_dir()/Turu` or `TURU_DATA_DIR` | `Context.getFilesDir()/Turu`, passed from JNI before the bot starts (set the same override `TURU_DATA_DIR` sets). |
| `scripts/` | in the data dir | `getExternalFilesDir(null)/scripts`: reachable by file managers and `adb`, no storage permission needed. |
| Logs | `<cache_dir>/log/ruffle.log` | Same file under `getCacheDir()`, plus `android_logger` to logcat. "Open log folder" becomes "Share log". |
| Account passwords | `keyring` (Keychain, Credential Manager, Secret Service) | `keyring` has no Android store. Add a small `SecretStore` trait in `bot/src/accounts.rs` with a `keyring` implementation and an Android one: an AES-GCM key in the Android Keystore (via JNI), encrypting one file in the data dir. |
| TLS (`reqwest` in `bot/` and `ruffle_frontend_utils`) | `native-tls` | `rustls` + `ring` + `rustls-platform-verifier` as in ruffle-android. Make the TLS backend a feature of `turu_bot` instead of hard-coding `native-tls`. |
| `monitor/push.rs` hostname (`Command::new("hostname")`) | shell | `Build.MODEL` from JNI, or the user's device name setting. |
| Game socket, HTTP | Ruffle navigator | Same; manifest needs `INTERNET` and `ACCESS_NETWORK_STATE`. |

### Running in the background

Android's limits are the biggest behavioural difference from a desktop:

* **Foreground service** with a visible notification. Android 14+ requires a type: use
  `specialUse` (with the `PROPERTY_SPECIAL_USE_FGS_SUBTYPE` explanation). `dataSync` is capped
  at 6 hours a day on Android 15, too short for farms; `mediaPlayback` would be a lie.
  Needs `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_SPECIAL_USE`, and `POST_NOTIFICATIONS`
  (asked at first start on Android 13+).
* **Partial wake lock** while a script runs, released on Stop.
* **Battery optimisation:** offer the "ignore battery optimisations" prompt
  (`REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`) and link to the vendor-specific guides (Xiaomi,
  Samsung, Huawei kill foreground services anyway). Without it, Doze cuts the network and the
  relogin watchdog has to recover.
* **Thermals and battery:** with the screen off nothing is drawn; the game's frame rate drops
  to the Headless Mode rate. With the screen on, offer an FPS cap.
* **Memory:** if the system kills the process, the service restarts it (`START_STICKY`) and
  Turu relogs and restarts the last script, as auto-relogin does after a crash. Respond to
  `onTrimMemory` with Mugetsu's `aqw_trim_memory`.

### Build and distribution

* `cargo ndk` through the Gradle plugin, NDK 27, Rust targets `aarch64-linux-android` and
  `x86_64-linux-android` (emulators). Skip armeabi-v7a and x86: a 32-bit address space is
  tight for an AQW client, and they double build time.
* minSdk 26 (AAudio, like ruffle-android); Vulkan needs API 24+ anyway. targetSdk follows
  ruffle-android.
* Features: `--no-default-features --features software_video,lzma,jpegxr`; no fontconfig,
  freetype, external_video. `mimalloc` builds for Android but is desktop tuning; leave it off
  at first and measure.
* CI: an Android build job and `cargo ndk clippy -p turu_android`; `cargo test -p turu_bot`
  stays host-only. ruffle-aqw gains an Android check so a Ruffle update doesn't break it.
* **Distribution:** signed APKs on GitHub Releases (works with Obtainium). Google Play isn't a
  realistic channel for a game bot (Play policy and Artix's terms), so `specialUse` review and
  Play's target-API deadlines matter less, but keep targetSdk current anyway.

## As built: phase 0

* **`turu_frontend`** (`frontend/`): `app/src/turu.rs`, `capture.rs`, `map_items.rs`,
  `map_pads.rs`, `grid.rs` and `app/src/gui/turu/` moved there unchanged in their module names;
  the app re-exports them at its root (`use turu_frontend::{capture, grid, platform, turu}`), so
  `crate::turu::...` still works in `app/`. The frontend depends on `turu_bot`, `ruffle_core`,
  `swf`, `egui`, `qrcode` and a few small crates: no winit, rfd, webbrowser or wgpu.
* **Seams:** `turu::set_tab_requests` and `TuruFlashHost::new` (was `DesktopFlashHost` with
  `new(EventLoopProxy)` and `headless(Sender)`); `turu::window_title(title)` and
  `turu::start_perf_log(main_thread)` take what came from `mugetsu` and `thread_perf`. The bar
  heights and Mugetsu's palette the windows use are in `turu_frontend::gui`; the app's
  `context_menu.rs` takes the palette from there.
* **`Platform`** as above; `app/src/desktop_platform.rs` implements it with rfd, `webbrowser`
  and Mugetsu's `open_folder`. The windows no longer take a `FilePicker` parameter.
* **`SecretStore`** (`bot/src/accounts.rs`): `set`/`get`/`delete` by lowercased username; the
  keychain (`keyring`) by default, `accounts::set_secret_store` to replace it. `keyring` isn't
  built for Android, where passwords aren't saved until the app installs its store.
* **TLS feature** on `turu_bot`: `native-tls` (default) or `rustls-tls`.
* Not done: `monitor/push.rs`'s `hostname` command (phase 3).

## As built: phase 1

* **Shared loop:** `turu_frontend::engine` holds the windowless pieces (`EngineEvent`,
  `Executor`, `NoDialogs`, `run_loop`, `flash_host`), taken from `app/src/headless.rs`, which now
  only maps the desktop's options onto the `PlayerBuilder`. A shared `PlayerBuilder` setup wasn't
  worth it: the two frontends' option sources differ and the builder calls are a dozen lines.
* **`turu_android`** (`android/`): NativeActivity (no Java of our own) and `android_main`, which
  starts the engine thread once per process; the thread outlives the activity. Setup comes from
  `turu-args.txt` (the desktop's flags, one argument per line) through
  `turu::take_startup_args`, so `--user`, `--run-script`, `--api` and the rest work unchanged.
  The data folder is the app's files folder on shared storage (`paths::set_default_root`), so
  `adb` reaches scripts, options and logs; phase 3 moves the private parts to internal storage.
* **TLS:** reqwest's `rustls-no-provider` with `ring`, certificates checked by
  `rustls-platform-verifier` (Kotlin half from the submodule `android/vendor/rustls-platform-verifier`,
  pinned to the commit the locked crate came from; a Gradle check compares it with
  `Cargo.lock`). AGP 9's built-in Kotlin needs the sources listed as `kotlin`, not only `java`.
* **QuickJS:** rquickjs has no pre-generated bindings for Android; its `bindgen` feature makes
  them at build time (libclang plus the NDK's sysroot, set by the `cargoNdk` Gradle task).
* **Logging:** `tracing` with `log`/`log-always` into `android_logger` (tag `turu`) with the
  desktop's default filter.
* **Checked on an Android 16 arm64 emulator:** the game loads to the title screen over HTTPS;
  `--load-script farm/GoldFarm` downloads it from the repository and loads it; the control API
  answers `/status`; `/script/start` runs CoreBots in QuickJS, which stops at "Please log-in".
  ~164 MB PSS and ~4% CPU at the title screen; the APK is 21 MB (debug, one ABI).
* **Logged in and farmed** (2026-11-03, emulator): GoldFarm for 20 minutes, 168 kills (~8 a
  minute, steady), no deaths or relogins, ~405-416 MB PSS without growth, 4-16% CPU. The first
  logins all timed out ("avatar state: 0"): with AQW's default settings (a new phone has no saved
  `AQWUserPref`) the game draws the stage into a bitmap when a map loads, the null renderer
  couldn't, and the error stopped the avatar's setup. Desktop `--headless` had the same bug,
  hidden by settings saved from playing in a window; the fix is in ruffle-aqw's
  `core/src/bitmap/operations.rs` (a draw the renderer can't do leaves the bitmap as it is).
* **Not done:** the full hour (stopped at 20 minutes), a real phone, and keeping the process
  alive in the background (phase 4).

## As built: phase 2

* **Surface:** the engine thread waits for the first window and builds the player with wgpu on
  it (`--graphics auto|vulkan|gl`; auto tries Vulkan, then OpenGL ES). `android_main` forwards
  the activity's events as `EngineEvent::Run` closures (new in `turu_frontend::engine`, with
  `run_loop_with`'s after-tick hook that draws). A new window gets a new surface
  (`recreate_surface_unsafe`); the window going away stops drawing first (acknowledged before
  `TerminateWindow` returns) and marks the bot's `hidden_tab`, so the game slows down like a
  hidden desktop tab and speeds up again on return.
* **Input** (`input.rs`): touch as the mouse; keys named from the Android key code, their
  character from the device's key character map (Shift works, unlike ruffle-android's table).
  **Soft keyboard** (`ui.rs`): Ruffle's `open_virtual_keyboard`/`close_virtual_keyboard` call
  `show_soft_input`/`hide_soft_input`; NativeActivity gets the keyboard's typing as key events.
* **Sound:** ruffle-android's AAudio backend, logging instead of panicking; the desktop's
  decoders (mp3, aac, nellymoser) and `default_font` for `_sans` and friends.
* **Foreground service, pulled forward from phase 4:** without one, Android 16 cut the app's
  network about 17 s after Home and the game dropped. `TuruActivity` (a NativeActivity) starts
  `TuruService` (`specialUse`, a notification, `START_NOT_STICKY`); the wake lock, battery
  prompt and notification actions stay in phase 4.
* **Checked on the emulator** (Android 16, arm64, `-gpu host`): the game draws with Vulkan
  (login screen, HUD, chat); GoldFarm kept farming through 60 s in the background (kills 2 to
  9, no relogin) and drew again on return; tapping the login screen's Username field opened the
  soft keyboard and typing reached it with capitals.
* **Vulkan vs OpenGL ES** (login screen, emulator): Vulkan ~199 MB PSS, 16-23% CPU; OpenGL ES
  ~196 MB, 12-16%, but it drew only the logo's outline, with no error logged. Mugetsu's
  renderer changes were made on Metal, Vulkan and DX12; GLES needs work before it can be a
  fallback. Default: Vulkan.
* **Not done:** a phone; logging in by hand on the game's screen (typing works; the login itself
  was done with `--user`); the soft keyboard covers the field (NativeActivity has no text view
  to scroll into view; phase 3's egui can show a text box above it); device fonts from
  `/system/fonts`.

## As built: phase 3

* **Screen** (`android/src/screen.rs`), as the desktop's `gui/controller.rs` does it: one wgpu
  device; the player draws into a texture of its own (`MovieTexture`, the desktop's
  `MovieView`); each frame draws it below Turu's nav bar (the desktop's `blit.wgsl`) and paints
  `TuruUi` (`show_bar`, `show_windows`, Headless Mode's cover) over it with `egui_wgpu`. A new
  window only gets a new surface (one per window: Vulkan refuses a second); the player's
  renderer never changes. The engine thread is marked as the UI thread
  (`turu::init_main_thread`), so a window calling Flash directly fails instead of waiting for
  itself.
* **Who gets input:** a touch that starts on the nav bar or an egui window is the UI's (until
  it lifts), else the game's, shifted below the bar; keys are the UI's while one of its text
  boxes has the focus, which also shows the soft keyboard. egui runs at the display's density
  (2.625 on the emulator's Pixel 7 profile: the nav bar is 89 px).
* **Not built from the plan:** the drawer and full-screen sheets (the desktop windows fit the
  landscape screen at that density, so they come later); SAF import (no file dialogs:
  `platform.rs` refuses them, and scripts come from Search Scripts); a text box above the soft
  keyboard for the game's own fields.
* **Also:** full screen (Android 15+ draws apps under the status bar, which covered the nav
  bar); Back moves the app to the background instead of closing the activity (with target SDK
  36 Back goes to the activity's back callback, not to native code); links open in the
  browser through an `Intent` (JNI).
* **Checked on the emulator:** the nav bar's menus open by touch; Search Scripts lists the
  repository, its search box takes the soft keyboard's typing; from the Script Loader GoldFarm
  was stopped and started, BuyScrolls was loaded from Search Scripts and its Script Options
  window opened (toggle, list, number); the engine and the UI's state survived the activity
  being destroyed and started again.

## As built: phase 4

* **The engine starts without a window:** `start_engine` runs once per process, from
  `android_main` or from `TuruService.nativeStart` (JNI), whichever comes first. The GPU is set
  up without a surface (`screen::gpu`), the player is built at once (skua.swf's stage size), and
  the first window gets the screen. The soft keyboard, the key maps and the browser use the
  activity's `AndroidApp` when there is one.
* **`TuruService`** (Java): loads the library and starts the engine in `onCreate`;
  `START_STICKY`, so after Android kills the process it comes back alone, the engine logs in
  again and runs `turu-args.txt`'s script. Its notification shows the script and its state
  ("GoldFarm running", "zykran is farming") with Pause/Resume and Stop, which call
  `nativeAction` (`turu::spawn_action`, then `script::set_paused` / `script::stop`). A partial
  wake lock is held while a script runs. `notify.rs` sends the state every 2 s when it changed,
  through a global reference to the service's class (a native thread can't find app classes).
* **`TuruActivity`** asks once (remembered in `SharedPreferences`) for notifications (Android
  13+; the service posts its notification again when it's granted) and for leaving Turu out of
  battery optimisation.
* **Checked on the emulator:** both prompts; the notification and its buttons (Pause, then
  Resume); 3 minutes with the screen off in forced deep Doze (kills 3 to 26, no relogin); the
  process killed with `kill -9` while in the background: the service restarted it within 1 s,
  the engine came up without a window, logged in and started GoldFarm, and opening the app
  later attached a screen to it.
* **Found on the way:** the notification thread made the bot before `start_engine` set the data
  folder (the bot then wrote to a relative `Turu/`, which Android refuses), so it starts after
  the engine now.
* **Not done:** a 6-hour run on two phone brands (the phase's "done when"); restarting a script
  started by hand (only `turu-args.txt`'s); guides for vendors' battery managers.

## Plan: tabs, Army and Butler (phase 5)

Written 2026-11-04 against `script-loader-layout` (`55bce1e5b`). Steps 1-4, 6 and 7 built on
2026-11-07 (branch `wt/android-tabs`), checked with `cargo ndk check` only; step 5 (Keystore)
and step 8 (a phone) are open. As built: the tabs after the first are reopened at every engine
start (not only after a kill: Android doesn't tell them apart for the service), muted tabs keep
their AAudio streams open (paused streams would replay stale sounds), Army Control's Load
script uses the loaded script without file dialogs, and HotKeys stay hidden (keys don't reach
egui outside a text box). `Army::launch` refuses on Android with `cfg!` rather than
`#[cfg]`, so the process code (and what only it uses) still compiles without dead-code
warnings.

### Why tabs, not processes

Army scripts (`bot.army`, `bot/src/api/army.rs`) and Butler (`bot/src/butler/sync.rs`) find
each other through files in the data folder: the army's sync logs, lock files and barrier, its
room number (a hash of the folder's path), and each account's `butler/<username>.state`. They
don't care whether the clients are processes or tabs; Butler's sync already tells tabs of one
process apart (`beat=` instead of VibeSkua's PID check). The desktop's tabs already hold one bot
and one player each, Army > Accounts has an "In a tab" launch mode (`LaunchMode::Tab`), and Army
Control steers tabs through `control_api::local`.

So on Android the multi-account story is several tabs in the one engine thread. Starting one
process per account (`android:process` services, each loading `libturu_android.so`) would cost
a whole runtime per account and is not planned.

What's missing is on Android's side: `android/src/lib.rs`'s `engine` builds one player and runs
`engine::run_loop_with(&player, ...)`, nothing calls `turu::set_tab_requests` (so the tab
strip's "+", Accounts' "In a tab" and Army Control's Select do nothing), and every Flash call
goes to that one player whatever its tab.

### Memory decides how many

One client is ~410 MB PSS while farming (phase 1, emulator). A phone holds 2 clients, maybe 3,
before the system starts killing a foreground service. Enough for Butler (you and one or two
followers) and small army scripts; not for a 4-6 account army, which stays a desktop or server
job (`docs/HEADLESS.md`). The limit is a setting (default 2, at most 4), checked on open.

### Steps

1. **Share the multi-player bookkeeping.** The desktop's `PlayerController` (`app/src/player.rs`)
   keeps, per tab: its player, its `ThreadState` (Ruffle's `PlayerThreadState` and the wgpu
   renderer's unsubmitted `segments`, swapped into the thread while a player that isn't the
   active one runs: `SwappedIn`, `TabPlayer`) and `hidden_tick` (`HIDDEN_TICK_MAX`, 50 ms,
   between a hidden tab's socket polls). Move `ThreadState`, `SwappedIn`, `TabPlayer` and the
   hidden-tick timing into a `turu_frontend::players` module with no winit, and have
   `PlayerController` use it, so the desktop and Android swap the same state the same way.
   Desktop behaviour unchanged (tabs, Grid View, closing a tab).
2. **A tabbed engine loop.** `engine::run_loop_with` takes the tab list: tick the active
   player at its frame rate and the others at the hidden tab's rate (`set_hidden_tab_fps`),
   answer `TuruFlashCall` with `call.tab()`'s player (as `app.rs`'s `user_event` does), run
   `EngineEvent::Run` on the active one, and add `EngineEvent::Tab(TabRequest)`. The desktop's
   `--headless` keeps one tab and goes through the same code.
3. **Tab requests on Android.** `engine` calls `turu::set_tab_requests` with a sender of
   `EngineEvent::Tab`. Open builds a player with `build_player` (`turu::add_tab` first, its bot
   given `engine::flash_host`'s host for that tab); Select swaps which player draws to the
   surface and which one AAudio plays (the others muted, as `set_tab_volumes`); Close detaches
   the bot and drops the player. Open over the limit is refused with a log line. The tab strip
   (`frontend/src/gui/turu/tab_strip.rs`, in `TuruUi` already) then shows up by itself with two
   tabs; it needs a touch size (taller tabs, "×" only on the active one).
4. **Accounts launch into tabs.** On Android, Army > Accounts offers only "In a tab" and
   "Log in by hand"; `army::Army::launch` (process spawning) is behind
   `#[cfg(not(target_os = "android"))]`, so nothing can start a process.
5. **Passwords in the Android Keystore.** `accounts.rs`'s Android `SecretStore` is a stub, so
   Accounts can't log in a tab by itself and `bot.army.login` fails. Implement it over JNI: an
   AES key in the Keystore, the passwords encrypted with it in internal storage (not the shared
   data folder). Needed for any unattended multi-account run.
6. **Service and notification.** The wake lock is held while any tab's script runs; the
   notification lists each tab's state (one line per tab, Pause/Stop for the active one); after
   a kill the service reopens the tabs that were open (their accounts, written to
   `android-tabs.json` on each open/close) and starts each one's script, extending phase 4's
   `turu-args.txt` restart.
7. **Touch layouts for Butler and Army Control.** Both are 520-560 px floating windows today;
   give them the sheet frame and enlarge the roster rows and Summon/Follow toggles.
8. **Run it on a phone.** Two tabs (a Butler master and a follower) for an hour with the screen
   off; an army script (e.g. one using `bot.army.barrier`) on two tabs; `onTrimMemory` at
   `TRIM_MEMORY_RUNNING_LOW` logs and, at critical, stops the hidden tabs' scripts instead of
   letting the system kill the service.

### Effort

| Step | Files | Estimate |
|---|---|---|
| 1-3. Shared player bookkeeping, tabbed loop, tab requests | `frontend/src/engine.rs`, new `frontend/src/players.rs`, `app/src/player.rs`, `android/src/{lib,screen,audio}.rs` | 1-2 weeks |
| 4. Accounts into tabs | `bot/src/army.rs`, `frontend/src/gui/turu/accounts.rs` | 1-2 days |
| 5. Keystore `SecretStore` | `bot/src/accounts.rs`, `android/` (JNI, Java) | 3-4 days |
| 6. Service, notification, restart | `android/src/notify.rs`, `TuruService`, `android/src/lib.rs` | 2-3 days |
| 7. Butler and Army Control layouts | `frontend/src/gui/turu/{butler,army,tab_strip}.rs` | 2-3 days |
| 8. Phone testing | | ongoing |

About 3-4 weeks in all. Steps 1-2 are the risky part (the thread-state swap is what keeps two
players' renderers apart); do them first and check the desktop's tabs and Grid View still work
before any Android code.

### Not in this plan

* **A phone and a desktop in one army or party.** The sync files only reach clients sharing a
  data folder, so a phone can't follow a desktop master or join a desktop army. That needs the
  army logs, barrier and Butler state carried over the network (the web monitor's hub, which
  already collects every client's snapshot, is the natural place). Worth its own design note: it
  would also let one army span several machines.
* **Steering desktop armies from the phone.** Already done by the web monitor in the phone's
  browser (Start / Stop / Relog / Join map on one bot or several).
* **Grid View** on a phone: the tiles would be too small; tablets only, later.

## Phases

| Phase | Goal | Done when |
|---|---|---|
| 0. Split (built) | `turu_frontend` crate; the event-loop seam; `Platform` trait; `SecretStore`; TLS feature on `turu_bot`. Desktop only. | Desktop and headless behave as before; `cargo test -p turu_bot` and clippy pass. |
| 1. Headless spike (built; farmed 20 min on an emulator) | `android/` with no UI: engine thread, null renderer, a script started from `turu-args.txt`, logcat output, control API on. | On a phone, `turu_bot` logs in (credentials from the control API or a file) and runs a farm for an hour. Answers: does QuickJS/oxc run, memory use, socket stability. |
| 2. Game on screen (built; checked on an emulator) | wgpu surface (Vulkan vs GLES measured), touch, AAudio, soft keyboard for Flash fields, surface detach/attach without restarting the game. | Log in with the game's own screen on a phone; switch apps and back mid-fight without losing the game. |
| 3. Turu UI (built; checked on an emulator, without the drawer, sheets or SAF) | egui on Android, drawer nav, sheet frames, the "Port" rows of the matrix, SAF import. | Load, configure, start and stop a script without a computer. |
| 4. Background (built; checked on an emulator) | Foreground service, notification actions, wake lock, battery prompt, restart after kill. | A 6-hour farm with the screen off survives on two device brands. |
| 5. Later | Tabs (2 clients), Army scripts and Butler between them ("Plan: tabs, Army and Butler"), Cosmetics, tablet layout, gamepad. | Tabs: a Butler master and follower farm for an hour with the screen off on a phone; an army script runs on two tabs. Others per row. |

Phases 0 and 1 are cheap and answer the risky questions (does the engine run, how much memory,
does the socket survive) before any UI work.

## Risks

| Risk | Mitigation |
|---|---|
| Soft keyboard into Flash text fields doesn't work well (ruffle-android hasn't done it). | Phase 2 tests it first. Fallback: an egui text box that sends its text to the focused field, and `--user`-style auto-login from saved accounts, so the game's login screen is optional. |
| Vulkan drivers crash or misrender with Mugetsu's patches. | GLES fallback and the "crashed before" marker; device test matrix in phase 2. |
| OEM battery managers kill the service. | Documented settings per vendor; auto-relogin and restart-last-script on process restart. |
| Memory: one client ~400 MB, phones kill big background apps. | One client in v1; trim memory on `onTrimMemory`; measure in phase 1. |
| Two upstreams drift (ruffle-aqw, ruffle-android). | ruffle-android is only a reference; follow its fixes by hand, like `desktop/`. |
| UI sprawl: two layouts for ~30 windows. | One body per window, two frames (window, sheet); phone nav lists only the "Port" windows. |
| Artix's terms and account bans apply as on desktop; phones add no new protection. | Same as desktop; nothing in this design changes how the client talks to the game. |

## Decisions to make

1. Is the goal "run bots on a phone" or "watch and steer bots from a phone"? The second is
   the web monitor, already built; this proposal is for the first.
2. Phone only, or tablets too (floating windows above 840 dp)?
3. Default renderer after phase 2's measurements: Vulkan or GLES?
4. Ship tabs (multi-account in one app) in v1, or one client only?
5. ABIs: arm64-v8a + x86_64 only?
