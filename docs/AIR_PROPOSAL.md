# Proposal: Adobe AIR as a second game client on Android

Status: proposal; spike A0 started (the patcher, see "Spike A0" below). Written 2026-11-07 against Turu `5263a0a17` (0.2.0-beta.2 on
Android), `../you-mad-bro` `c7be9c4` and VibeSkua's `Skua.AS3`.

Read `docs/ARCHITECTURE.md` (the Flash bridge, `FlashHost`) and `docs/ANDROID_HANDOVER.md`
first.

## Summary

* **Yes, it can be done, and the bot engine barely changes.** `turu_bot` reaches the game
  only through `FlashHost::call` and `Bot::on_flash_call`, and `skua.swf` reaches the bot only
  through its `Externalizer` class (`addCallback` and `call`, 159 lines). Swap both ends of that
  bridge and the scripts, the API and the background loops run as they do now.
* **But it isn't a drop-in "other player".** You Mad Bro runs on **Adobe AIR** (Harman AIR 51,
  captive runtime), not on a Flash Player you can embed. AIR brings its own activity, its own
  APK packaging (ADT), and the AIR application sandbox. Three things follow:
  1. AIR on Android has **no `ExternalInterface`**. The bridge becomes an AIR Native Extension
     (ANE) that loads Turu's Rust library.
  2. The AIR sandbox **rejects AQW's client as shipped**: `Security.allowDomain` throws (#3207),
     and maps loaded with `Loader.load` can't share code with the game. The game SWF has to be
     **bytecode-patched**. You Mad Bro does this at build time and ships a patched `Game.swf` in
     the APK. Turu should patch at run time, in Rust, so a game update doesn't need a new APK.
  3. Turu's egui windows are drawn in the same wgpu surface as Ruffle's game today. Under AIR,
     AIR owns the screen, so the UI needs an **overlay view** of its own.
* **Don't copy You Mad Bro's code.** The repository has **no license file**, so legally it is
  all-rights-reserved. Use it as a map of *what* to patch and *which* Android settings work
  (it farms 24/7 in the background on AIR), and write Turu's code ourselves. Or ask its author
  for permission.
* **Recommendation: a two-week spike first, with a go/no-go at the end.** Patch `Game.swf` with
  Rust, load it under AIR with a modified skua, log in, and run for an hour on the phones that
  fail on Ruffle today (Redmi 10, Infinix HOT 11S / Hot 30i). If AIR draws correctly and stays
  up where Ruffle doesn't, build the real thing (phases A1 to A4, roughly 6 to 9 weeks). If not,
  the time goes back into Ruffle's memory and driver fixes.

## Spike A0: what we know so far (2026-11-09)

Against the live client `Game4000r8.swf` (`sVersion` R0048). Built: the `air_patch/` crate
(`turu_air_patch`), its tests, and two examples: `patch` (patch a SWF and list what changed) and
`dump` (print a method's bytecode with names resolved). No AIR runtime test yet (no AIR SDK on
this machine).

**AQW's client already has an AIR mode of its own.** This changes most of the plan below:

* `types.AirLoader` extends `Loader`. When `Security.sandboxType == "application"`, its `load`
  fetches the SWF with a binary `URLLoader`, runs it through `SwfLoaderPatch.patch(bytes, domain,
  url)` and loads it with `loadBytes` and `allowCodeImport = true`. Otherwise it falls back to
  `super.load`.
* `SwfLoaderPatch` is Artix's run-time ABC patcher, in AS3. It turns each `flash.display::Loader`
  in a loaded SWF into `types::AirLoader` and flips SharedObjects to secure. So maps, and the
  assets they load, are handled at any depth.
* The client already builds an `AirLoader` in about 30 places, including `World.loadMap`,
  `World.loadNextHouseItem`, `Game.startGameMenuLoad`, `loadExternalAssets`, `showBook`,
  `frame43` (character select) and `qRewardPrev`.
* `serverFilePath` comes from `params.sFilePath` when the host sets it (`Game.onAddedToStage`), and
  only falls back to `loaderInfo.url` otherwise. Nothing to patch: skua-air sets the parameter.
* `Security.allowDomain("*.aq.com")` in `Game`'s constructor sits in a try/catch, so it's harmless.
* `Mobile.isAIR` and `Mobile.isTouchDevice` exist (the mobile touch UI, `liteAssets.mobile`).

You Mad Bro doesn't use any of this. Its patcher (written against an older client, or ignoring
this mode) redirects `World.loadMap` and the others to its own loader. Most of its ~1,900 lines
aren't needed for today's client.

**What `turu_air_patch` changes in `Game4000r8.swf`** (0.2 s, 2.81 MB to 2.83 MB):

| Rule | Sites |
|---|---|
| `findpropstrict Loader; constructprop Loader, 0` (flash.display) becomes the client's own `types::AirLoader`, as `SwfLoaderPatch` does to loaded SWFs | 8: `Avatar/loadPet` (pets: the one that matters), `MailWindow/buildImageBlock`, `mcPopup_355/loadMap`, `mcPopup_355/loadNews`, `JoyArt/get`, `cnt_deathcounter_version2_188/loadAd`, `FBListener`, `WildTangentAPI/loadVex` |
| `allowDomain`/`allowInsecureDomain` calls dropped (arguments popped, `undefined` pushed if the result is used) | 2: `FBListener` (unguarded, unused), `Game/Game` (already in a try/catch; dropped anyway) |

* The rules match instructions, not method names or code shapes. A client update that moves code
  is still patched. `Report::game_problem` refuses a client without `AirLoader` and
  `SwfLoaderPatch`, or one with bodies it can't read.
* Branches, `lookupswitch` tables and exception ranges are relinked after an edit, with unit tests.
  The whole client re-reads after patching (7,656 method bodies decode), and patching it a second
  time finds nothing more. `TURU_GAME_SWF=<path> cargo test -p turu_air_patch` runs that check.
* Not checked yet: AIR's verifier accepting the result. The edits keep stack heights (`max_stack`
  +1 where a call is dropped) and touch no types (`AirLoader` is a `Loader`), so a VerifyError
  would be a surprise.

**What this means for the design:**

* No hook class and no load redirection in skua-air.swf. Its changes shrink to: load the game with
  `loadBytes` and `allowCodeImport` (bytes from `turu_air_patch`), set `params.sFilePath` (and
  `isWeb = false`, as You Mad Bro does), and drop its own `Security.allowDomain('*')`. Map
  unloading (`unloadAndStop`, a GC nudge) is still ours to add if AIR's memory grows like
  Ruffle's: check in the spike.
* The patch is small enough that a future client might need none. If Artix removes the AIR mode,
  `game_problem` says so and Turu stays on Ruffle.

**Harman's license.** The AIR SDK site (`airsdk.harman.com`) now redirects to a Wipro one
(`airsdk.wxlift.com`), and its pricing page renders in the browser only. The last terms we could
read are Harman's January 2020 ones: free under $50 k a year, with a splash screen; Professional
$799 a year up to $500 k
([PDF](https://airsdk.harman.com/assets/pdfs/Adobe%20AIR%20-%20Updated%20Pricing.pdf)). Read
today's terms by hand before going further.

**Under AIR (2026-11-09, emulator, Android 16 arm64, AIR SDK 51.5.1):** `air/spike/` (now `air/app/`) is a bare
host (`TuruAirHost.as`, `app.xml`, `build.sh`). It loads the live client, patched by
`turu_air_patch` and bundled as `app:/Game-air.swf`, with `loadBytes` and `allowCodeImport` into
a child `ApplicationDomain`. `params` (with `sFilePath`) are set before the game reaches the
stage. Result: sandbox `application`, client constructed in 70 ms, login screen and title art
drawn, no uncaught errors, `serverFilePath`/`serverGamePath` = AQW's URLs. Build notes:
* ADT packages through its own Gradle 8.13, which fails on Java 25 ("Unsupported class file major
  version 69"). It needs a JDK 17 (`AIR_JAVA_HOME` in `build.sh`) and `ANDROID_HOME`.
* A free-tier splash screen shows for ~1.6 s at start (`AIRRuntimeCheck: showing Splash Screen`).
* APK: 25 MB (captive runtime, arm64, with the 2.8 MB patched client).

Logged in by hand on the emulator: battleon drawn with its NPCs, the news popup's art
(`mcPopup_355/loadNews`, one of the swapped loaders), the avatar's gear; `/join yulgar` loaded
the next map. 275 MB PSS in battleon. Two problems, neither from the patch:
* **The game disconnected within seconds of going to the background.** AIR suspends a
  deactivated app by default. The host now sets `NativeApplication.executeInBackground` and
  `SystemIdleMode.KEEP_AWAKE`; Android's own limits (network cut ~17 s after Home without a
  foreground service, docs/ANDROID_HANDOVER.md) still call for the service in the ANE.
* **`softKeyboardBehavior` `pan` left the stage drawn ~325 px low** (with stale pixels above it)
  after tapping the chat box, though no keyboard showed (the emulator's is hidden while the Mac's
  counts as hardware). It drew right again after a trip to the background. To try: `none` and the
  client's own handling of `stage.softKeyboardRect`.
* Back sends an AIR app to the background (its default), as `TuruActivity` does.

With `executeInBackground` the game still lost its connection in 2 minutes off screen: Android
froze the process (`ActivityManager: freezing <pid> air.com.muktiarafi.turu.airspike`, 71 s after
Home; ~10 s in the first run) and swapped ~200 MB of it out. AIR's switch can't stop the cached
apps freezer: only a foreground service can (as `TuruService` does for Turu today). So the
background test waits for the minimal ANE with the service.

**The minimal ANE** (`air/ane/`, `com.muktiarafi.turu.air`): `TuruAirExtension`
(`startService`, `stopService`), `TuruAirService` (a `specialUse` foreground service with an
ongoing notification and a partial wake lock, as `TuruService`), in Kotlin, and the AS3
`TuruAirNative`. The Kotlin side is a standalone Gradle library (`air/ane/android`, Turu's
version catalog and Gradle wrapper, AGP's built-in Kotlin, `FlashRuntimeExtensions.jar`
compile-only). An ANE gives AIR one jar per platform, so `build.sh` merges the library's classes
with the Kotlin standard library: the ANE is 1.7 MB, nearly all stdlib (R8 could shrink it).
`acompc` for the SWC; `adt -target ane`. The service is declared in `app.xml`'s manifest additions (full class
name: the app's package is `air.<id>`). On the emulator the host starts it at launch: foreground,
type `specialUse`, notification posted.

**Background with the service: it holds.** Logged in, Home for 5 minutes, back: no `freezing`
line in logcat, same process, still in battleon with the other players (6 in the room), no
disconnect. 226-246 MB PSS while off screen, CPU ~0% (AIR slows a deactivated app; a bot script
will need to check it keeps its pace).

**First real phone (2026-11-11): Samsung Galaxy S24 FE** (Exynos 2400e, Xclipse 940), the spike
APK from `air-spike` rebased on main. Logged in, changed maps, went to a house (house items load
through the client's `AirLoader`), and stayed connected through screen off and back. Nothing
went wrong. This phone runs Turu's Ruffle build too (after the flicker fixes), so it says AIR
works on a phone, not yet that it fixes the phones Ruffle fails on.

**The bridge benchmark (2026-11-11, emulator, at the login screen): fast enough.** The design
above, built: crate `turu_air` (`air/ane/rust`, `libturu_air.so` by cargo-ndk) on a thread of
its own calls Kotlin's `Bridge.dispatch`, which sends AIR a status event ("call", id); the host
answers on AIR's thread with a small JSON of game state (as `getGameObject` would) through the
extension's `reply`, which Kotlin hands to `Bridge.nativeReply`; Rust waits on a channel.

| Where | Round trips | Median | p95 | Max |
|---|---|---|---|---|
| On screen | 1,000 in 1,540 ms | 1.60 ms | 1.95 ms | 19 ms |
| Off screen (foreground service) | 100 in 918 ms | 2.69 ms | 27.5 ms | 303 ms |

A plain JNI call into Kotlin is 0.5 us, so the cost is AIR delivering the event, not JNI. AIR
delivers status events right away, not once a frame (that would be ~42 ms at 24 fps): ~650 calls a
second on screen. Off screen AIR slows the app, which shows in the tail. The Ruffle host's hop
is cheaper, but VibeSkua's own ExternalInterface calls aren't free either; a script making tens of
calls per action is fine. Bursts (`lnk*` reads of many objects) should batch, as skua's helpers
already do.

On the S24 FE the 1,000-call run overlapped logging in and battleon loading: 1,000 in 32.6 s,
median 1.64 ms, p95 87.6 ms, max 261 ms. A call is answered between the game's frames, on AIR's
thread, so while it loads a character and a map (or draws a heavy frame) calls wait; the Ruffle
host has the same rule (its calls wait for the player's event loop). In battleon, off screen: 100
in 1.03 s, median 2.04 ms, p95 63 ms, max 98 ms. Judged good enough to go on; an idle in-game
run on screen, side by side with Ruffle on the same phone, would settle it. Packaging: an ANE's `Android-ARM64/libs/arm64-v8a/*.so` lands in the APK's
`lib/arm64-v8a/` (900 KB here) and `System.loadLibrary` finds it.

**One APK or two: one is possible, though not through ADT's APK targets.** (2026-11-11,
emulator.)
* ADT refuses a `LAUNCHER` category in `app.xml`'s manifest additions ("error 404: Android
  attribute android:name is not allowed to be overridden", and the same for `android:exported`),
  and refuses any entry for its own `AIRAppEntry`. An activity with `MAIN` only, or no filter,
  is accepted.
* `adt -target android-studio` exports a plain Gradle project instead (AGP 8.9.1, Gradle 8.13;
  AIR's runtime as `AppEntry.jar`, `runtimeClasses.jar` and `jniLibs/<abi>/libCore.so`; our
  extension's jar and `libturu_air.so`; the SWFs as assets). Its manifest is ours to edit.
  `air/spike/studio.sh` (removed in A3, which chose two apps: see git history) declared `TuruLauncher` (in the extension, Kotlin) with `MAIN`, exports,
  moves the `LAUNCHER` category from `AIRAppEntry` to `TuruLauncher`, and builds with Gradle.
  On the emulator the launcher starts AIR's activity, and the game, the service and the bridge
  run as in the ADT build (1,000 round trips, median 1.55 ms).
* `AIR_NOANDROIDFLAIR=true` drops the `air.` prefix: the app ID is `app.xml`'s `<id>` as is
  (`com.muktiarafi.turu.airspike` here), so an AIR build can carry `com.muktiarafi.turu` and
  update today's app.
* So the one-app design works by folding the exported project into Turu's Gradle build:
  `TuruLauncher` starts Ruffle's `TuruActivity` or AIR's `AIRAppEntry` by the user's choice.
  The export carries AIR's native runtime for four ABIs (88 MB debug APK); an `abiFilters` to
  arm64 cuts that, as Turu's build already does. The export's Gradle and AGP lag Turu's (8.13 and
  8.9.1 vs 9.8 and 9.5): folding in means taking its libraries and manifest entries, not its
  build files.

**Left for the spike:**
* The phones that fail on Ruffle (Redmi 10, Infinix HOT 11S / Hot 30i): 50 map changes, an hour
  idle, memory curve. This decides go/no-go.
* The bridge benchmark in game (logged in, a busy map) and on a phone.
* A longer background run (an hour, screen off) with a memory reading.
* Pets, the Book of Lore and char select (through `SwfLoaderPatch`).
* Today's AIR SDK license terms (Wipro).

## A1: the engine on AIR (started 2026-11-11, before the failing-phone runs)

**As built:**
* **skua.swf stays unchanged, patched at run time** (`turu_air_patch`, `Target::Skua`): its
  `ExternalInterface` becomes the host's `TuruExternal` (same static `addCallback` and `call`),
  each `new Loader()` the host's `TuruLoader`, and its `allowDomain` is dropped (6 swaps, 1 drop,
  3 ms). No skua-air.swf to build or keep in step with VibeSkua: the "skua-air.swf" section above
  is superseded. The game's client is patched at run time too (`Target::Game`, as skua loads it),
  so a game update needs no new APK. Other SWFs (backgrounds, injected scripts) are patched as
  assets: cleaned, not required to have the AIR mode.
* **`turu_frontend` builds without Ruffle**: a `ruffle` feature (default on: the desktop and
  Android builds are unchanged) gates `engine.rs`, `players.rs`, `capture.rs` and the Ruffle host,
  now `turu/ruffle_host.rs` (re-exported, same paths). Startup flags, tabs, the bot, the API and
  the monitor are shared. The crate's tests run without Ruffle (93 pass), which they couldn't
  (`ruffle_core`'s Java build).
* **`turu_air`** (`air/ane/rust`, `libturu_air.so`): `nativeStart` sets up logging, rustls
  (`ring`, rustls-platform-verifier over JNI), the data folder (the app's external files folder,
  as the Ruffle build's) and `turu-args.txt`'s flags, then the API and the monitor; client updates
  are left out (they install the Ruffle build's APK). `AirFlashHost` sends each call as a status
  event (`{"id","name","args"}`) and waits up to 20 s for `nativeReply`; a call from AIR's own
  thread is refused. `nativeFlashCall` hands skua's calls to `Bot::on_flash_call`. `nativePatch`
  patches over JNI and refuses a client without its AIR mode. `loaded_swf` (Grabber's map items,
  the map pads) isn't there yet: `None`.
* **The extension** (`air/ane`): Kotlin functions `startEngine`, `flashCall`, `reply`,
  `patchSwf` (ByteArray in and out), the service; rustls-platform-verifier's Kotlin part comes
  from the Ruffle build's module (`android/rustls-platform-verifier`, which now resolves its paths
  from its own folder so another Gradle build can include it). The extension is 7.3 MB, the APK
  39 MB (arm64).
* **The host** (`air/app/src`): `TuruAirHost` starts the service and the engine, loads the
  patched skua.swf into a child domain at the stage's bottom (skua swaps itself for the game),
  keeps its log strip on top. `TuruExternal` answers the engine's calls with `ExternalInterface`'s
  stringification (as the Ruffle host's `from_external`). `TuruLoader` fetches, patches,
  `loadBytes` with `allowCodeImport`, and sets the client's `params` (`sFilePath`, `isWeb`) on
  `init`, before skua's `complete` puts it on the stage.

**Checked on the emulator (2026-11-11):** skua.swf patched and loaded, `Externalizer::init done.`
reached the bot, `requestLoadGame` -> `loadClient` -> the live client fetched, patched (277 ms) and
loaded, `Game client loaded`. Logged in by hand; `/status` (`--api`) read the game (map, cell,
level, HP). `AirSmokeTest.ts` (`air/app`) loaded and started through the API: read the player,
`bot.map.join("yulgar")`, then logged map, cell, HP and gold every 3 s. Pause held it, Resume went
on, Stop ended it within a second. `/relogin`: "Relogin by IP successful" in 7 s, back on the same
server.

A trap for testing: a folder `adb` creates in the app's data folder (`files/scripts` pushed
before the app made it) is invisible to the app (Android's storage layer goes by the creator),
so its scripts aren't found and Search Scripts can't write there. Let the app create its folders
(a first start does), then push into them.

**Left for A1:** 6 hours screen-off on a phone (the "done when"); `loaded_swf`; Stop/Pause in the
notification (the API does it now); scripts without adb (Search Scripts in the UI is A2); an
in-game bench on a phone.

## A2: Turu's windows over the game (2026-11-11)

**As built:**
* **`overlay.rs`** (crate `turu_air`): the nav rail and the bot windows (`TuruUi::show_bar` and
  `show_windows`, the Ruffle build's code) drawn with wgpu of its own (Vulkan, else GL) into a
  see-through surface (`CompositeAlphaMode::PreMultiplied` where offered), cleared to transparent,
  on a thread of its own: at once after input, else when egui asks, at least every 200 ms (logs
  change without input). egui's fonts as the Ruffle build's (Roboto, the symbols).
* **`TuruOverlay`** (Kotlin): a `SurfaceView` over AIR's (`setZOrderOnTop`, translucent), in the
  activity's content layout. At a finger's down it asks the engine (`nativeOnUi`: the rail and
  bar, or a non-background egui layer, as the Ruffle build's `Screen::touch`); a gesture it doesn't
  take falls through to AIR's view below. It's the soft keyboard's target while egui wants text
  (`platform_output.ime`): `commitText`, deletes and Enter become egui events.
* **The game right of the rail:** the engine sends the rail's and bar's size in pixels
  (`gameInsets`) whenever they change (the rail hidden, a cutout); Kotlin sets them as AIR's view's
  margins, and AIR's stage scales into what's left (`SHOW_ALL`).
* **`AirPlatform`**: one client (no Army, no New Tab), touch-sized controls, no hotkeys, no file
  dialogs yet; links and the share sheet through Kotlin, copy to the clipboard.
* **HTTPS:** rustls-platform-verifier's Kotlin half fetches revocation lists over plain HTTP from
  the CAs' hosts, which its module allows with a network security config (merged into the Ruffle
  build's manifest). Without it every certificate came back "Revoked" (no Search Scripts). `app.xml`
  sets `usesCleartextTraffic` for the AIR app for now; the verifier's own per-host config, as a
  resource in the extension, is the tighter fix.
* The host's log strip hides 20 s after skua loads (it covered the game's bar); an error shows it.
* The APK is 56 MB (the engine with wgpu and egui: the extension's `.so` grew).

**Checked on the emulator:** the rail drawn, the game moved right of it (AIR's view 2232 of 2400
px: the 64-point rail at 2.625); Menu, Scripts, Search Scripts opened by touch; Search Scripts
listed 1,800 scripts and downloaded them (Auto Update Scripts); its search box took typing through
the soft keyboard ("babysitter": 1 found); a tap on the game's username field reached AIR (the
keyboard then served `AIRWindowSurfaceView`).

**First phone test (Galaxy S24 FE), and the fixes:**
* *Crash on coming back from the background, idle or running a script.* Not reproduced on the
  emulator; the likely cause, fixed: `surfaceDestroyed` handed the surface's loss to the drawing
  thread and returned at once, and Android may free a surface once that returns, while a frame
  could still be drawing into it. `set_window` now waits (up to 2 s) until the drawing thread has
  let the old surface go. Panics now also go to the data folder's `crash/panic-<time>.txt`.
* *The UI came back empty after the background* (emulator, found while checking the above): a
  new egui renderer per surface lost egui's textures, which it uploads once ("Missing texture:
  Managed(0)"). The renderer now lives with the GPU device, across surfaces.
* *No typing in Search Scripts.* The overlay asked for a text editor, and some keyboards
  (Samsung's) then send composing text, which a plain connection drops; Gboard happened to commit
  text. Now `TYPE_NULL` (key events only, as the Ruffle build's `KeyInput`), with
  `onKeyDown`/`onKeyMultiple` into egui, deletes as Backspace/Delete.
* *No typing strip above the keyboard.* The Ruffle build's strip for the game's fields, now drawn
  by the overlay (`GameTyping`): the host SWF reads AIR's `stage.focus` each frame (text, caret,
  selection, password, `maxChars`) and sends it when it changes; the keyboard's height comes from
  the window's insets. `softKeyboardBehavior` is `none` (AIR's pan moved the stage oddly). Turu's
  own text boxes move above the keyboard as the Ruffle build's (`pan.rs`, ported).
* *The rail under the notch.* The display cutout's left inset now widens the rail
  (`set_rail_inset`, as the Ruffle build), and the game moves with it.
* *Sound in the background.* The host sets `SoundMixer.soundTransform` to 0 on deactivate, 1 on
  activate.
* Also: the host's stage setup ran only on `ADDED_TO_STAGE`, which the main SWF may never get (it's
  on the stage already): it now runs at once when there's a stage.

Checked on the emulator: three trips to the background and back (same process, the UI drawn);
the rail clear of an emulated camera hole; typing and Backspace in Search Scripts by key events;
the strip showing the game's username field's text and caret over AIR's keyboard.

**Confirmed on the Galaxy S24 FE (2026-11-11):** all five fixed (no crash coming back from the
background, idle or running; typing in Search Scripts; the strip above the keyboard; the rail
clear of the notch; silent in the background), and a script farmed correctly, found, started and
watched from Turu's own windows on the phone.

**Left for A2:** a tap on the game doesn't close an open egui menu (egui only sees the UI's
touches); Turu's touch controls (joystick, skills) over AIR; the notification's Stop/Pause.

## A3: Turu AIR, an app of its own (2026-11-11)

Two apps, not one with a switch: **Turu AIR** next to Turu.
* `air/app/` (was `air/spike/`): `app.xml` names it "Turu AIR", ID `com.muktiarafi.turuair`,
  version 0.2.0; `build.sh` packages with `AIR_NOANDROIDFLAIR=true`, so the ID is that, not
  `air.com.muktiarafi.turuair`. Its data folder is `Android/data/com.muktiarafi.turuair/files`,
  apart from Turu's: settings, accounts and scripts aren't shared.
* Its look: `turu_frontend`'s `air` feature (on in `turu_air`) makes the accent Adobe AIR-like red
  (`#E0262D`, hover `#E84C52`, pressed `#BE1C22`) instead of Turu's violet, and the rail's open
  item a dark red; the rail's logo tile follows the accent. The launcher icon is Turu's symbol in
  that red (`air/app/icons`, rendered from `turu-air-symbol.svg` with macOS's QuickLook). The
  danger red (`#EF4444`, Stop, Delete) is close to the accent there; it can move toward orange in
  Turu AIR if they're taken for each other.
* The notification's title is the app's name ("Turu AIR").
* Signing: `AIR_KEYSTORE` and `AIR_KEYSTORE_PASSWORD` (PKCS#12); without them a development key in
  `target/air-app/`. The one-APK experiment (`studio.sh`, `TuruLauncher`) is gone.

**Left for A3:** the release workflow's job for Turu AIR (the AIR SDK in CI:
`joshtynjala/setup-adobe-air-action` with `accept-license`, JDK 17, cargo-ndk, the keystore from
the secrets), a release keystore of its own, Sentry for Turu AIR, docs (docs/ANDROID.md or one of
its own), and the AIR SDK's license terms read.

## Why consider it: what Ruffle costs on Android

From `docs/ANDROID.md` and the crash reports so far:

| Problem | Seen on | Would AIR fix it? |
|---|---|---|
| GPU out of memory, wgpu panics (`Globals::new`) | Redmi 10 (Mali-G52, 4 GB), Infinix HOT 11S | Probably. AIR's renderer is Adobe's own, tuned on phones for a decade, with no wgpu/Mugetsu atlas layer. Spike measures it. |
| Garbage drawing, no error | Infinix Hot 30i (PowerVR GE8320) | Probably. AIR draws the real Flash player's way. |
| OpenGL ES draws only parts of the game | every GLES device | Yes. Not a concern under AIR. |
| Process aborted at 1.9 GB RSS after 27 min | Pixel 8 | Unknown. AIR has its own GC, and AQW keeps every SWF it loaded under any player. You Mad Bro unloads each map (`unloadAndStop`, its own `ApplicationDomain`) and calls `System.gc()` every 90 s. Turu would do the same. |
| ColorOS kill for CPU use | OPPO A9 2020 | No. That's the phone maker's battery manager, whatever the player. AIR may use less CPU, so it may help. |
| Soft keyboard, typing strip, `TextControl`, touch routing for the joystick | all | Mostly. AIR handles Flash text fields, the IME and multitouch natively. |
| Ruffle's AVM2 gaps (a game feature that misbehaves) | case by case | Yes. AIR is the reference implementation. |

What AIR doesn't fix: background network limits, Doze, vendor battery managers, AQW's own
memory growth. The foreground service, wake lock and relogin logic stay.

## What You Mad Bro is

| | You Mad Bro |
|---|---|
| Runtime | Harman AIR SDK 51.1+, captive runtime in the APK, arm64 only. `app.xml` with `renderMode auto`, landscape, full screen. |
| Client | `Loader.swf` (~10 k lines of AS3: `loader/src`). Its own bot engine (command list, AutoBattle, AutoQuest), UI in AS3. |
| Game | `Game.swf` downloaded **at build time**, disassembled with RABCDAsm, patched by regex on the assembly (`patcher/Patcher.java`, ~1,900 lines), reassembled, shipped in `app:/gamefiles/`. Same for `spiderbook3.swf`, `Map-UI_r38.swf`, `charselect.swf`. |
| Patches that make AQW run under AIR | strip `Security.allowDomain` (throws #3207 in the app sandbox); hard-code `serverFilePath`/`serverGamePath` (they come from `loaderInfo.url`, which is `app:/`); route `World.loadMap`, `loadNextWith`, `loadNextHouseItem(B)` to `queueLoadViaBytes`/`loadMapViaBytes` (URLLoader, then `loadBytes` with `allowCodeImport`, each map in its own child `ApplicationDomain`); raise `maxstack` where it adds calls. |
| Patches for its bot or UI (not needed by Turu) | `SmartFoxClient.handleMessage` public; hero menu; `mcPopup`; world map; char select; quest reward preview. |
| Android side | One ANE (`com.aqw.battery`, Java): foreground service (`specialUse|dataSync`), wake lock, Wi-Fi lock, battery-optimization prompt, notifications, Picture-in-Picture, SAF file picker. |
| Build | AIR SDK + JDK 17 + D compiler + RABCDAsm; GitHub Actions with `joshtynjala/setup-adobe-air-action`. |
| License | **None in the repository.** |

The lesson for Turu is that AQW runs under AIR on Android with a handful of loader patches, and
it keeps running in the background. Its approach of shipping a frozen patched client, though,
means every AQW client update needs a new APK.

## Architecture

```
AIR activity (AppEntry, Harman)                          Turu's library (Rust, in the ANE)
────────────────────────────────                          ──────────────────────────────────
AIR runtime ── skua-air.swf ── patched Game.swf           turu_bot: Bot, scripts, timers
   │            │                                            ▲          │
   │            │ Externalizer.call("packet", ...)           │          │ FlashHost::call(name, args)
   │            └──► ext.call("ei", name, json) ──JNI──► Bot::on_flash_call (never blocks)
   │                                                                     │
   │   StatusEvent("call", id) ◄── dispatchStatusEventAsync ◄── AirFlashHost: queue + wait reply
   │            │ runs the callback on AIR's thread                     ▲
   │            └──► ext.call("reply", id, result) ──JNI─────────────────┘
   │
   └─ content view ── overlay SurfaceView (ANE) ── egui TuruUi drawn with wgpu
                      touches outside Turu's windows fall through to AIR
TuruService (foreground service, notification, wake lock: today's Kotlin, moved into the ANE)
```

### The bridge: an ANE

* **AS3 to bot.** `Externalizer.call(name, ...rest)` becomes
  `context.call("ei", name, JSON.stringify(rest))`. Java's `FREFunction` passes it over JNI to
  Rust, which calls `Bot::on_flash_call` exactly as `TuruExternalInterface::call_method` does
  now. The same rule applies: it never blocks.
* **Bot to AS3.** A new `AirFlashHost: FlashHost` puts `{id, name, args}` in a queue, asks Java
  to `dispatchStatusEventAsync("call", id)`, and waits on a reply channel, as the Ruffle host
  waits on its event loop. skua's `StatusEvent` listener takes the request
  (`context.call("next")`), runs the registered callback on AIR's thread, and answers with
  `context.call("reply", id, result)`. It keeps the same timeouts and `HostError`s. Calling from
  AIR's own thread is refused, as calling from the event loop is today.
* **Why not a loopback socket?** `flash.net.Socket` to `127.0.0.1` would need no JNI for the
  bridge and would work with AIR on a desktop too. But calls would be async on the AS3 side, a
  port is open to every app on the phone (it needs a token), and we need an ANE anyway for the
  service and the overlay. Keep the socket as the fallback if ANE events turn out slow; the spike
  measures round-trip time.

### skua-air.swf

(Superseded in part by "Spike A0": with the client's own `AirLoader`, change 2 needs no map
hook. Only the patched bytes, `loadBytes` with `allowCodeImport`, and `params.sFilePath`.)

Built from `../VibeSkua/Skua.AS3` with three changes (CLAUDE.md says `skua.swf` stays unchanged.
This is a second SWF next to it, for AIR only):

1. `Externalizer.addCallback`/`call` go through the ANE (above). The callback table stays as is.
2. `Main.loadGame` fetches the game SWF's bytes, asks the bot for the patched bytes
   (`context.call("patchGame", version, bytes)`, cached per version), and loads them with
   `loadBytes` and `allowCodeImport = true`. It also exposes the hook the patched `World` calls
   for maps and house items (child `ApplicationDomain`, `unloadAndStop` on the old map, a GC
   nudge).
3. No `Security.allowDomain('*')` (`Main.as` line 94).

### Patching the game in Rust, at run time

(As built, the rules are smaller: see "Spike A0". Paths come from `params.sFilePath`, and map loads
already go through the client's `AirLoader`.)

Ruffle's `swf` crate (already a `turu_frontend` dependency) reads and **writes** SWF files and
ABC bytecode. A new module (`bot/src/air_patch.rs`, or its own small crate) would:

* strip `Security.allowDomain`/`allowInsecureDomain` calls (`getlex Security; push...;
  callpropvoid allowDomain`);
* replace `serverFilePath`/`serverGamePath` with AQW's URLs;
* redirect `ldr.load(req, ctx)` in `World.loadMap`, `loadNextWith`, `loadNextHouseItem(B)` to
  skua's hook, and recompute `max_stack`;
* do the same for the other SWFs that load code (`spiderbook3`, `Map-UI`, `charselect`) as the
  spike finds them;
* find each patch site by method name and an instruction pattern. If a site is missing (AQW
  changed its client), refuse with a clear message ("this game version isn't supported in AIR
  mode yet; switch to Ruffle") instead of loading a half-patched client.

Patched SWFs are cached in the data folder by game version (`gameversion`'s `sFile`). This
replaces You Mad Bro's D + Java + RABCDAsm toolchain with code we own and test
(`cargo test` with a saved `Game.swf`), and a game update needs no release.

### The UI over AIR

AIR's `AppEntry` owns the activity and draws into its own view. The ANE gets the activity
(`FREContext.getActivity()`) and adds a transparent `SurfaceView` over AIR's view
(`setZOrderMediaOverlay(true)`, translucent format). Turu draws `TuruUi` into it with wgpu,
reusing `android/src/screen.rs`'s egui painting without the movie texture. A container view
decides who gets each touch: a touch inside the nav rail or a Turu window goes to egui (today's
rule in `screen.rs`), and anything else is passed down to AIR. This keeps VibeSkua's layout. The
fallback, if overlay drawing or passing touches through misbehaves on some phones, is a separate
Turu activity for the bot windows (switch between the game and Turu, as with any app).

### Crates and packaging

* `turu_frontend` gets a `ruffle` feature (on by default). `turu.rs`, `capture.rs`, `engine.rs`
  and Mugetsu's options sit behind it, so the bot windows build without `ruffle_core`.
* A new `turu_air` cdylib (or a feature of `turu_android`): `turu_bot`, the egui overlay, the
  JNI half of the ANE, `AirFlashHost`. No Ruffle, so it is much smaller than today's library.
* The ANE: Java/Kotlin (`TuruService`, `CrashUploads`, the overlay, the bridge functions), the
  `.so`, and the manifest additions (service, permissions). ADT builds the APK from `app.xml`,
  the SWFs and the ANE.
* **One APK or two?** The user's "choose Ruffle or Flash" can be:
  * **Two apps** from one repo: `Turu` (today's Gradle build, Ruffle) and `Turu AIR` (ADT). It's
    the simplest build, but a separate app ID means a separate data folder, and users choose by
    installing.
  * **One app** with a choice in Options > Application ("Game client: Ruffle / Adobe AIR", taking
    effect at the next start). It would be an ADT-built APK whose launcher is Turu's own activity
    (from the ANE's manifest additions). That activity starts either `TuruActivity` (Ruffle) or
    AIR's `AppEntry`. It carries both libraries (bigger APK), and depends on ADT accepting a
    launcher that isn't `AppEntry`, which the spike has to prove.
  * Recommendation: aim for one app, and find out in the spike whether ADT allows it. If it
    doesn't, ship two.

## What changes for users in AIR mode

| Feature | AIR mode | Notes |
|---|---|---|
| Scripts, Script Loader, Search Scripts, options, skills, Auto, relogin, monitor, control API | Same | Through `AirFlashHost`. |
| Logging in with the game's screen, typing | Better | AIR's own text fields and soft keyboard. The typing strip isn't needed. |
| Interceptor, Packet Logger edits (`capture.rs` wraps Ruffle's socket) | Adapt | A loopback proxy like VibeSkua's `CaptureProxy`, or skua's `catchPackets` for reading only. Already hidden on phones (v1 "Drop"), so it can wait. |
| Grabber's map items, Jump's pads (`FlashHost::loaded_swf`) | Adapt | `AirFlashHost::loaded_swf` downloads the map SWF itself (same URL, cached). |
| Frame interpolation, Safe Drawing, GPU memory trims, Graphics API choice | None | Ruffle only. AIR has `renderMode` (auto/gpu/cpu/direct) and Stage quality instead. |
| Tabs, Army in tabs, Butler (proposal phase 5) | Not in AIR mode | One AIR app runs one root SWF, and skua is a singleton (`Main.instance`). |
| Restart without a window after a kill | Worse | The service can restart Turu's engine, but the game needs AIR's activity, and Android 10+ restricts starting activities from the background. After a kill: a notification "tap to resume". |
| Picture-in-Picture | New, possible | You Mad Bro shows it works with AIR (it patches the AIR SDK's template). |

## Risks

| Risk | Mitigation |
|---|---|
| **Harman's license.** Harman's January 2020 pricing put the free tier under $50 k a year in income, with a splash screen injected at start; paid tiers remove it. Those terms may have changed since. | Read today's terms before the spike ends. A Harman splash screen in a free tool may be fine. |
| **GPL-3.0 and a proprietary runtime in one process.** Turu's library would run inside AIR's process, next to closed code. | Turu's own code can carry an additional permission (GPL section 7) for linking with the AIR runtime. Check that nothing GPL from third parties is in `turu_air` (Ruffle is MIT/Apache; check Mugetsu's terms for anything that would come along). Get a clear answer before shipping. |
| **You Mad Bro has no license.** | Read it for knowledge, write our own patcher and loader. Credit it in docs. Ask the author if we want any of its code. |
| AQW changes its client, the patch sites move. | Pattern-based finding, clear refusal, Ruffle still there. Tests run against each new `Game.swf` (a CI job can fetch the current version daily and try the patcher). |
| AIR also runs out of memory or is slow on 4 GB phones. | That's what the spike measures, on those phones, before any other work. |
| ANE call round trips are slower than Ruffle's event-loop hop (the bot makes hundreds of calls a second at times). | Spike benchmark: 1,000 `getGameObject` calls. Fallbacks: batch calls (`lnk*`/`fc*` already exist in skua), the socket bridge. |
| The overlay `SurfaceView` misdraws or eats touches on some phones. | A separate Turu activity for the windows as the fallback. |
| Two clients to support. | The bot and UI are shared. Only the host, the loader and the patcher differ. AIR mode can stay labelled experimental until it has a crash-free record. |
| Harman drops or changes Android AIR. | Ruffle stays the default and the fallback. AIR is optional. |
| Play Store rules (16 KB pages, target SDK) need a recent AIR SDK. | Turu ships APKs from GitHub releases. Check the AIR SDK's Android target before shipping. |
| Artix treats AIR clients differently. | You Mad Bro sets `isWeb = false` and works. Behave as it does, and as Turu does today. |

## Plan

| Phase | Goal | Done when | Rough size |
|---|---|---|---|
| A0. Spike (go/no-go) | Rust patcher prototype on the `swf` crate (strip `allowDomain`, paths, map loads). A bare AIR app with a modified `skua.swf` (bridge stubbed with `trace`) loads the patched game. Bridge benchmark with a minimal ANE. Check ADT with a custom launcher. Read Harman's terms. | On a Redmi 10 and an Infinix: log in, change maps 50 times, idle in a map for an hour, without a crash or misdrawing; memory curve recorded. Bridge round trip measured. A yes or no on one APK. | 2 weeks |
| A1. Engine on AIR | `turu_frontend` `ruffle` feature, `turu_air` cdylib, `AirFlashHost`, the ANE bridge, `TuruService` in the ANE, skua-air.swf built in CI, the patcher in `turu_bot` with tests and a cache. | A script runs from `turu-args.txt` in AIR mode, Stop/Pause work, relogin works, 6 hours screen-off on one phone. | 2–3 weeks |
| A2. UI overlay | egui over AIR, touch passing, the nav rail, windows as today. | Load, configure, start and stop a script by touch in AIR mode. | 1–2 weeks |
| A3. Choice and release | One APK with the client switch (or two apps), the release workflow's AIR job (AIR SDK action, signing), Sentry in AIR mode, docs. | A release ships with both clients; switching keeps the data folder and settings. | 1–2 weeks |
| A4. Gaps | `loaded_swf` by download, the capture proxy, "tap to resume" after a kill, PiP if wanted. | Per row of the table above. | 1 week+ |

A0 should also say plainly which Ruffle problems it fixes. If AIR is clearly more stable on the
failing phones, it may deserve to become Android's default client, with Ruffle as the
alternative, rather than the reverse.

## Decisions to make

1. Run the spike? It's the cheapest way to answer the question for real.
2. One app with a switch, or two apps? (After the spike's ADT check.)
3. Harman's terms and the splash screen: acceptable for Turu?
4. Add a GPL linking permission for the AIR runtime to Turu's license?
5. Ask You Mad Bro's author about reuse, or write everything ourselves?
6. If AIR is clearly more stable: which client is the default on Android?
