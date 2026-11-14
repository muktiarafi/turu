# Turu Mas

Turu Mas is Turu's second Android app: the same bot engine, scripts and windows, but AQW's client
runs in the **Adobe AIR runtime** instead of Ruffle. It's an app of its own, next to Turu, in
gold. How it was built and measured: `docs/AIR_PROPOSAL.md` (spike A0, phases A1–A3).

The name: *mas*, from *emas*, is Javanese for gold, with *turu*, sleep; hence its gold. It isn't
"Turu AIR" because the AIR SDK's license keeps Adobe's trademark out of an app's title (see
"The AIR SDK's license" below). "Works with Adobe® AIR®" is the allowed way to say what it runs on.

## For users

* Install `turumas-<version>-android-arm64.apk` from
  [Releases](https://github.com/muktiarafi/turu/releases) (arm64, Android 8+). It installs next
  to Turu (`com.muktiarafi.turu`) as `com.muktiarafi.turumas`.
* Its data folder is its own (`Android/data/com.muktiarafi.turumas/files`): settings, accounts
  and scripts aren't shared with Turu. Search Scripts downloads the scripts.
* It starts with the AIR SDK's splash screen for about 1.6 s (the SDK's free tier).
* Tabs, Army and Butler as in Turu on Android: the tab strip's "+", Army > New Tab, Army >
  Accounts ("In a tab") and Army Control, up to Options > Application > Tab Limit (default 4,
  at most 4; about 200 MB a client), reopened after Android stops the app
  (`android-tabs.json`). No Grid View. No HotKeys or file dialogs.
* Army > Accounts keeps passwords as Turu on Android does: encrypted with a key in the Android
  Keystore, in the app's internal storage (not the shared data folder; not Turu's), so an
  account opened in a tab logs in by itself.
* Updates: as Turu on Android, it checks the releases at start and every 6 hours (Options >
  Application) and shows Update in the bar; the window's Download APK fetches Turu Mas's APK
  for Android to install over it (same signature, so data stays). A release without a Turu Mas
  APK (those before it) offers its release page instead.
* Crash reports (Sentry) are sent only if you say yes at first start, as in Turu; change it in
  Options > Application. [PRIVACY.md](../PRIVACY.md) says what's sent.

## What's where

| Path | What |
|---|---|
| `air_patch/` | crate `turu_air_patch`: patches AQW's client and the SWFs it loads for AIR's sandbox, at run time. |
| `air/ane/rust/` | crate `turu_air` (`libturu_air.so`), a Cargo workspace of its own ("Licenses"): the engine (`android.rs`), `AirFlashHost` (`host.rs`), Turu's windows over the game (`overlay.rs`), `AirPlatform` (`platform.rs`), crash reports' engine side (`crash_uploads.rs`). |
| `air/ane/android/` | the extension's Kotlin: `TuruAirExtension` (AIR's calls), `TuruAirService` (the foreground service), `TuruOverlay` (the view Turu's windows draw in), `GameField` (the game's focused text field as a native Android field above the keyboard, kept in step by the host's `gameTyping` and status event "typing": as Turu's, but a view right of the rail, as a window of its own would take AIR's focus), `Passwords` (Accounts' passwords, with `passwords.rs`: the Ruffle build's `Storage.kt` and `KeystoreStore`), `CrashUploads` (Sentry), `CrashReports` and `Tombstone` (how earlier runs ended). |
| `air/ane/as3/`, `air/ane/extension.xml` | the extension's AS3 face (`TuruAirNative`) and descriptor. `air/ane/build.sh` builds the `.ane`. |
| `air/app/` | the app: the host SWF (`src/TuruAirHost.as`, `TuruExternal`, `TuruLoader`, and `TuruHost` for Turu's own `turu.swf`), `app.xml` (ID, name, manifest additions), `build.sh` (the APK). Icons: `res/` (the adaptive one: gold `#D4A72C`, dark eyes) and `icons/` (PNGs from `turu-air-symbol.svg`: `qlmanage -t -s 512`, then `sips -z`). `build.sh` adds rustls-platform-verifier's network security config to `res/` (plain HTTP only to the certificate authorities' hosts, for revocation lists). |
| `air/ane/rust/src/clients.rs`, `air/app/src/TuruStage.as` | Tabs: game clients 0-3 on AIR's one stage, each with its own bot (below). |
| `frontend/` (`air` feature) | the gold accent (`theme.rs`: `ACCENT`, and `ON_ACCENT`, the dark text and icons on it); `turu_frontend` without its `ruffle` feature. |

The code keeps "air" in its names (crate `turu_air`, package `com.muktiarafi.turu.air`, the
`air` feature): they name the runtime, not the app.

## Build

Needs the AIR SDK 51.5+ (`AIR_HOME`), a **JDK 17** (`AIR_JAVA_HOME`: ADT packages with its own
Gradle 8.13, which fails on newer JDKs), the Android SDK with the NDK (`ANDROID_HOME`), cargo-ndk
and libclang, as Turu's Android build (docs/ANDROID.md, "Build"). Then, with a phone on adb:

```bash
air/app/build.sh
```

`--no-install` only builds. Output: `target/air-app/TuruMas.apk`.

* **Version:** `TURU_VERSION` (the release workflow passes the tag), else the root `Cargo.toml`'s.
  Android's version code is the one Turu's Gradle build gives the same version (0.2.0-beta.3 is
  200203): AIR computes the code from `versionNumber` (major × 1,000,000 + minor × 1,000 + patch),
  so `build.sh` writes it as `0.200.203`. `versionLabel` (what Android shows) is the version.
* **Signing:** `MAS_KEYSTORE` (a PKCS#12 file) and `MAS_KEYSTORE_PASSWORD`; without them, a
  development key made in `target/air-app/dev.p12`. An APK signed with one key doesn't install
  over one signed with another.
* **Crash reports:** `TURU_MAS_SENTRY_DSN` when building; without it the app never asks.
* `air/ane/build.sh` keeps the library with its symbols in `target/air-ane/jni/arm64-v8a/` and
  packs a copy without debug info.
* **Perf log:** `--perf-log <seconds>` in `turu-args.txt` (in the app's data folder) is the
  desktop's `TURU_PERF_LOG`: each tab's calls to `turu.swf` per second, every one a round trip
  through the extension, and the busiest, in logcat (`turu_perf`), and `turu.swf`'s gaps between
  frames (`frames:`).

## Releases

The release workflow's `turu-mas` job (`.github/workflows/release.yml`) builds it on every tag
with Turu's other packages and publishes `turumas-<version>-android-arm64.apk` in the same
release. The name doesn't start with `turu-`: Turu's updater takes any
`turu-*-android-arm64.apk` as Turu's own APK, and installed versions already work that way.
Turu Mas's own updater looks for `turumas-*` (`turu_bot::updates`: the engine calls
`set_turu_mas`, so `Target::current()` is `TuruMasArm64` and `app_name()` "Turu Mas").

The job gets the AIR SDK with `joshtynjala/setup-adobe-air-action` (`accept-license: true`
accepts the SDK's license; free tier), JDK 17 and 21, the NDK, cargo-ndk and libclang.

**Setup, once,** in the repository's Actions secrets and variables:

| Name | Kind | What |
|---|---|---|
| `MAS_KEYSTORE` | secret | Turu Mas's release key (PKCS#12), base64. A tag build fails without it; a run by hand signs with a development key. |
| `MAS_KEYSTORE_PASSWORD` | secret | Its password. |

Crash reports and symbols need nothing more: the job uses Turu's Android Sentry project
(`SENTRY_DSN`, `SENTRY_PROJECT`, `SENTRY_AUTH_TOKEN`, `SENTRY_ORG`).

The release key is Turu Mas's own, not Turu's (`turu-release.jks`): the two are separate apps.
Make it once with a JDK 17's keytool and keep a copy somewhere safe (losing it means users
uninstall to update). keytool asks for the password, so it stays out of the shell history:

```bash
keytool -genkeypair -storetype PKCS12 -keystore turu-mas-release.p12 -alias turumas -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=Turu Mas"
```

```bash
base64 -i turu-mas-release.p12 | gh secret set MAS_KEYSTORE --repo muktiarafi/turu
```

```bash
gh secret set MAS_KEYSTORE_PASSWORD --repo muktiarafi/turu
```

`*.p12` is in `.gitignore`. To sign a local build with it: `MAS_KEYSTORE=turu-mas-release.p12
MAS_KEYSTORE_PASSWORD=... air/app/build.sh`.

## Crash reports

Sentry runs in the extension's Kotlin (`CrashUploads.kt`, with `io.sentry:sentry-android` and its
NDK part), as in Turu's Android app: the same question at first start and the same switch
(`sendCrashReports` in `settings.json`; the engine's `crash_uploads.rs` asks and answers through
`Bridge`). It catches Java and native crashes as they happen, Turu's and AIR's runtime's alike,
and hangs (ANRs), merged with the tombstone where Android keeps one (Android 12+), and sends them at
the next start. A Rust panic: the panic hook writes `crash/panic-<time>.txt`, hands the message to
Sentry (`Bridge.panicked`), and the release build's abort is then reported as one native crash
with it. They go to Turu's Android project (the release job reuses its `SENTRY_DSN` secret, as
`TURU_MAS_SENTRY_DSN`, which the extension's Gradle build reads): filter by release
`turu-mas@<version>` or the tag `client: air`. No IP address, no screenshots, no logs.

**How earlier runs ended** (Android 11+): the extension's `CrashReports.kt`, Turu's own ported
(with `Tombstone.kt`), reads Android's record of each exit Turu Mas didn't ask for at the next
start (before the engine, and in the service) and writes `crash/exit-<time>-<reason>.txt`: a
crash in Turu or in AIR's runtime (Java or native, with its tombstone on Android 12+), a hang, a
low-memory kill, a battery saver's kill. Force stop, an update and a clean exit write none.
Diagnostics > Logs > Crashes lists them and a notice shows each new one, as in Turu. The exits
Sentry doesn't catch itself (low memory, a kill) are sent too, one issue per reason, with the
report attached.

**Packaging:** an ANE gives AIR one jar and its native libraries, so the extension's `aneLibs`
task collects what it needs at run time (Sentry, the AndroidX parts it uses, Kotlin's standard
library: Java 8 class files, which the AIR SDK's old d8 can read) and Sentry's `libsentry.so` and
`libsentry-android.so`; `air/ane/build.sh` merges and packs them (about 1.7 MB more in the APK).
On Linux (the release job) ADT packages its "legacy" way, dexing with the AIR SDK's own d8
1.2.52; on a Mac with an Android SDK it builds through Gradle 8.13 and AGP 8.9.1, whose d8 is
current, so a jar can pass locally and fail in CI. That old d8 crashes on `MethodParameters`
entries without a name, which some of Sentry's classes have (`v0.3.0-beta.4`'s job:
"NullPointerException ... visitParameter"), so `air/ane/strip-parameters.py` removes the
attribute from the merged classes (it only names parameters for reflection). The legacy route also
dexes without Android's classes, so it can't desugar default interface methods (AndroidX's
`DefaultLifecycleObserver`, which Sentry implements: "its super class `java.lang.Object` is
missing"); it dexes for the manifest's minimum SDK, AIR's 21 unless `app.xml` says otherwise, and
from 24 up nothing needs desugaring. So `app.xml` declares Turu Mas's real minimum, 26 (Android
8).

Symbols: the release workflow builds the library with line tables, uploads it with `sentry-cli`
and keeps it as the `symbols-turu-mas-android-arm64` artifact (90 days).

## The AIR SDK's license

Read 2026-11-11: the agreement shipped in the SDK (`$AIR_HOME/AIR SDK license.pdf`,
`AdobeAIR_SDK License-en_US-20190731`, now Wipro's, formerly HARMAN's) and the pricing page
(`airsdk.wxlift.com/pricing`). Not legal advice; what matters for Turu Mas:

* **Fees (section 4).** Free tier: under $50k a year in revenue, and the runtime injects a
  splash screen at start. Paid tiers (per seat, per year): Basic $199 (to $100k), Professional
  $799 (to $500k), Enterprise $1,199. Turu earns nothing: free tier. `app.xml` picks the dark
  splash (`<splash>dark</splash>`, AIR 51.5.1).
* **Trademark (section 14).** No "AIR" or "Adobe" in the app's title (hence the name). "Works
  with Adobe® AIR®" or "for Adobe® AIR®" is allowed. No HARMAN or Adobe names to market it
  (3.3(b)).
* **Distribution (3.3).** The runtime may be shipped only as ADT puts it in the app (the captive
  runtime: what `build.sh` does). Never with a pre-release (beta) SDK (section 10).
* **Privacy (6.4).** A privacy policy, posted, and linked conspicuously in the app:
  [PRIVACY.md](../PRIVACY.md), linked from the "Send crash reports?" question.
* **Licenses on the SDK (3.2, 3.3(iv)).** "In no event" may the SDK be made subject to a license
  that requires it be distributed as source, licensed for derivative works, or redistributable at
  no charge. Turu is GPL-3.0-or-later, and the APK carries AIR's runtime: see "Licenses" below.
* **Development (6.1, 8).** Only documented ways into the runtime (an ANE is one), no reverse
  engineering of the SDK. Turu patches AQW's and VibeSkua's SWFs, not AIR.
* **The build tools phone home (6.4).** ADT sends Wipro the OS, the app ID, packaging settings,
  the SDK version and a machine ID (`airsdk.wxlift.com`): CI's builds too. The runtime in the app
  sends nothing about users.
* **Audits (4), indemnity (5), governing law (15):** English law for a licensee in Indonesia.

## Licenses

The APK combines Turu's GPL code (`libturu_air.so`, the extension) with AIR's proprietary
runtime. GPLv3's "System Libraries" exception doesn't cover a runtime bundled in the app, so
without more the GPL would ask for the whole APK under it, which AIR's license forbids (3.2). So:

* **An additional permission (GPLv3 section 7):** [LICENSE-EXCEPTION](../LICENSE-EXCEPTION) lets
  Turu's code be combined with AIR's runtime and the SDK parts ADT packages, and the result
  conveyed. It's the FSF's wording for linking with a library under other terms, without the
  part that asks for the library's source. Only copyright holders can grant it, for their own
  code, so every contributor of code in the APK has to agree (`git shortlog -sn -- bot frontend
  air air_patch`): muktiarafi; leeteryx (`bot/`, `frontend/`): **asked, not yet agreed**. New
  contributions come with it (CONTRIBUTING.md). Release Turu Mas only once everyone has.
* **No Mugetsu in the APK.** Mugetsu's changes are GPL-3.0 and its author's, so the permission
  can't cover them. The root workspace patches wgpu-core and wgpu-hal with them (Ruffle's
  drawing); Turu Mas only draws egui with wgpu, so `turu_air` is a workspace of its own
  (`air/ane/rust/Cargo.toml`, with its own `Cargo.lock`) built with crates.io's wgpu-core and
  wgpu-hal. Its other patch, egui-wgpu, is Turu's (MIT / Apache-2.0). `frontend`'s `pacing`
  (Mugetsu's frame interpolation pacing) builds only with the `ruffle` feature. To check:
  `cd air/ane/rust && cargo tree --target aarch64-linux-android -e normal | grep ruffle/` lists
  no `wgpu-core` or `wgpu-hal`.
* The rest of the APK is permissive: Ruffle's `swf` and `http-cache` (MIT / Apache-2.0), Sentry
  (MIT), AndroidX and Kotlin's library (Apache-2.0).

Not legal advice.

## Tabs

AIR runs one root SWF on one stage, so Turu Mas's tabs are game clients side by side on that
stage, not players of their own as in Ruffle. How it was found to work: docs/AIR_ARMY_SPIKE.md.

* **Whose game.** AQW's client and nearly every SWF it loads find the game as
  `stage.getChildAt(0)`. `turu_air_patch`'s `Options::shared_stage` rewrites each of those to
  `TuruStage.rootOf(x)`, the game `x` belongs to: in the client when it's patched, and in every
  other SWF as the engine downloads it (`shared_stage_swf`, on the download thread).
  The tab on screen has its game at stage child 0 and the others hidden (`visible`, so AIR doesn't
  draw them). Every 60 s the Debug log says if any call couldn't tell its tab (none so far).
* **Whose calls.** The host loads Turu's `turu.swf` for every tab, the same unpatched bytes, its
  tab number in the `LoaderContext`'s parameters (`turuClient`). It reaches the host through
  `TuruHost` (calls, loading through `TuruLoader`, showing its game; docs/HOST_SWF_PROPOSAL.md,
  R4). Its calls reach the engine with `n` (`nativeFlashCall`), which hands them to that tab's
  bot (`clients::bot`); the bot's calls name `n` too (`AirFlashHost { client }`), so they run its
  `turu.swf`'s callbacks.
* **Opening and closing.** `clients.rs` serves the UI's tab requests: a new tab gets a free client
  number and a bot, and the host SWF loads a skua for it (status event "tab", `open`); `select`
  shows it; `close` disconnects its game and unloads its SWFs (`unloadAndStop`). Client 0, the
  first tab, loads at start.
* **Fitting the view.** With several tabs the tab strip pushes AIR's view down, and AIR keeps
  its stage at the screen's size when its view gets shorter (as for a soft keyboard), so the game
  would run off the bottom and right. Kotlin reports the view's size (status event "layout"); the
  host reads AIR's own size (the stage in `NO_SCALE` for a moment, the resize kept from the
  games) and scales and moves each game to fit (`TuruAirHost.fit`, `TuruStage.place`: 0.903x on
  a 2400x1080 screen). For the game's code that reads the stage's mouse, `stage.mouseX` and
  `mouseY` are rewritten to the game's own. With one tab nothing moves.
* **The map's SWF for the bot.** Grabber's map items and Jump's pads read the map the game
  loaded (`FlashHost::loaded_swf`). AIR keeps its movies to itself, so `AirFlashHost` reads the
  copy the game was served from the disk cache (`game_files::served_swf`: the newest of the last
  64 SWFs `fetch` served whose path ends in the map's). With the cache off, or the file gone
  from it, the bot downloads the map itself, as before.
* **No hidden-tab slowdown.** Ruffle slows a hidden tab's game to 2 FPS with `stage.frameRate`;
  on AIR that's every client's rate, so Turu Mas doesn't (`turu::set_hidden_tabs_slowed(false)`).
  Hidden games aren't drawn, and AIR slows the whole app off screen.
* **Only the tab on screen hears and is heard.** The stage and the sound mixer are every client's,
  so `Options::shared_stage` also routes, in every SWF, what would reach them all through
  `TuruStage`, with the calling code's own `ApplicationDomain.currentDomain`: each client's skua
  has a domain of its own and its game and maps load in or below it, so the `Game` class found
  from there (skua's main class before the game) names the tab (`TuruStage.clientOfDomain`).
  * *Keys:* the listener of each `addEventListener(KeyboardEvent.KEY_DOWN or KEY_UP, f, ...)`
    (on the stage or anything else; 91 sites in the client) becomes a gate that passes keys only
    while its tab is shown (`keys`, and `keysRemoved` so `removeEventListener` finds it).
    Switching tabs takes the focus off a hidden game's text field.
  * *Sound:* each `play(...)`'s result goes through `TuruStage.sound`: a hidden tab's
    `SoundChannel` is silenced, and follows its tab from then on; timeline sounds (maps,
    monsters, skills) through the hidden game's own `soundTransform`.
  * *Popups:* `addChild`, `removeChild` and `getChildByName` shortly after a `stage` (22, 7 and 10
    sites) go through `stageAddChild` and its kin: a child for the stage goes into its tab's
    layer, a sprite just above its game that's fitted, shown and hidden with it (`rootOf` maps
    it back to the game). The game and skua themselves stay on the stage.
  * The host's Debug log says, when it changes, how many keys were held back, sounds silenced,
    popups put in a layer, and hooks that couldn't tell their tab (none so far).
* **The client's own domains.** The client loads its assets and the game menu (the treasure
  chest) into a `new ApplicationDomain()`, whose only parent is the system's, where those SWFs,
  patched to call `TuruStage`, wouldn't find it (the menu never opened). `Options::shared_stage`
  gives each such domain a parent, `TuruStage.assetsParent`: the domain of the client's skua,
  which has `TuruStage` and names the tab, without the game's classes over the assets' own.
* **World Camera** resets the game's root to scale 1 at (0, 0) when it closes, which undid the
  fit with several tabs; the host puts the fit back each frame it finds the shown game there
  (`TuruStage.keepFitted`).

## Known gaps

* Tabs: a stage kept in a variable and used later (`MinigameSkip`, the keybinds' `onClose`)
  isn't seen by the popup rule, so those few still show over whichever tab is on screen.

From docs/AIR_PROPOSAL.md (A1–A3), still open:

* 6 hours screen-off, and the phones that fail on Ruffle (Redmi 10, Infinix): not run yet.
