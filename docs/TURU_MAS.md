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
* One game client: no Army, no New Tab. No HotKeys or file dialogs.
* Updates: as Turu on Android, it checks the releases at start and every 6 hours (Options >
  Application) and shows Update in the bar; the window's Download APK fetches Turu Mas's APK
  for Android to install over it (same signature, so data stays). A release without a Turu Mas
  APK (those before it) offers its release page instead.
* Crash reports (Sentry) are sent only if you say yes at first start, as in Turu; change it in
  Options > Application. [PRIVACY.md](../PRIVACY.md) says what's sent.

## What's where

| Path | What |
|---|---|
| `air_patch/` | crate `turu_air_patch`: patches AQW's client and VibeSkua's `skua.swf` for AIR's sandbox, at run time. |
| `air/ane/rust/` | crate `turu_air` (`libturu_air.so`): the engine (`android.rs`), `AirFlashHost` (`host.rs`), Turu's windows over the game (`overlay.rs`), `AirPlatform` (`platform.rs`), crash reports (`crash_uploads.rs`). |
| `air/ane/android/` | the extension's Kotlin: `TuruAirExtension` (AIR's calls), `TuruAirService` (the foreground service), `TuruOverlay` (the view Turu's windows draw in). |
| `air/ane/as3/`, `air/ane/extension.xml` | the extension's AS3 face (`TuruAirNative`) and descriptor. `air/ane/build.sh` builds the `.ane`. |
| `air/app/` | the app: the host SWF (`src/TuruAirHost.as`, `TuruExternal`, `TuruLoader`), `app.xml` (ID, name, manifest additions), `build.sh` (the APK). Icons: `res/` (the adaptive one: gold `#D4A72C`, dark eyes) and `icons/` (PNGs from `turu-air-symbol.svg`: `qlmanage -t -s 512`, then `sips -z`). |
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

`air/ane/rust/src/crash_uploads.rs`, Turu Mas's side of the desktop's `app/src/crash_uploads.rs`:
the same question at first start and the same switch (`sendCrashReports` in `settings.json`,
through `Platform`). Sentry's own panic hook sends a Rust panic as it happens; Turu's hook then
keeps it in the data folder's `crash/panic-<time>.txt` and waits (2 s at most) for the report to
leave, as a release build aborts on a panic. They go to Turu's Android project (the release
job reuses its `SENTRY_DSN` secret and `SENTRY_PROJECT` variable): filter by release
`turu-mas@<version>` or the tag `client: air`. No IP address and no device name.

Not caught: crashes in AIR's runtime (Java or its own native code) and Android's exit reasons
(low memory, a kill), which Turu's `CrashReports.kt` reports. They'd need the Kotlin side of
Turu's Android build (`io.sentry:sentry-android`) in the extension.

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
  no charge. Turu is GPL-3.0-or-later, and the APK carries AIR's runtime: see "Open question".
* **Development (6.1, 8).** Only documented ways into the runtime (an ANE is one), no reverse
  engineering of the SDK. Turu patches AQW's and VibeSkua's SWFs, not AIR.
* **The build tools phone home (6.4).** ADT sends Wipro the OS, the app ID, packaging settings,
  the SDK version and a machine ID (`airsdk.wxlift.com`): CI's builds too. The runtime in the app
  sends nothing about users.
* **Audits (4), indemnity (5), governing law (15):** English law for a licensee in Indonesia.

**Open question: GPL and the runtime.** The APK combines Turu's GPL code (`libturu_air.so`,
the extension) with AIR's proprietary runtime. GPLv3's "System Libraries" exception likely
doesn't cover a runtime bundled in the app, so a recipient could argue the whole APK must be
GPL, which AIR's license forbids. The usual fix is an additional permission (GPLv3 section 7)
from Turu's copyright holders, for example: "As an additional permission, you may convey Turu
Mas combined with the Adobe AIR runtime and SDK components that ADT packages into it, without
those components being covered by this License." Not added yet: the license is the author's
call (decision 4 in docs/AIR_PROPOSAL.md).

## Known gaps

From docs/AIR_PROPOSAL.md (A1–A3), still open:

* `loaded_swf` (Grabber's map items, the map pads): `None` on AIR.
* Stop and Pause in the notification (the API does it).
* A tap on the game doesn't close an open Turu menu.
* 6 hours screen-off, and the phones that fail on Ruffle (Redmi 10, Infinix): not run yet.
* Crashes in AIR's runtime aren't reported (see "Crash reports").
* The release job hasn't run in CI yet: the AIR SDK's download on Ubuntu and ADT's packaging
  there are untested.
