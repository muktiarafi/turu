# Turu

Cross-platform (macOS, Windows, Linux) AdventureQuest Worlds bot client: a port of
VibeSkua (C#/Avalonia, Windows only) onto **Ruffle + Mugetsu's AQW patches**, with the
bot engine in **Rust** and scripts in **TypeScript**. Same UI/UX and flow as VibeSkua.

Read first: `docs/ARCHITECTURE.md` and `docs/STATUS.md` (what works, known gaps,
tooling quirks). The Android port: `docs/ANDROID_HANDOVER.md` (start here: state,
branches, setup, testing, pitfalls), `docs/ANDROID_PROPOSAL.md` (plan) and `docs/ANDROID.md`
(build, run). Turu Mas, the Android app on Adobe AIR: `docs/TURU_MAS.md` (build, release,
license; how it came about: `docs/AIR_PROPOSAL.md`). Reference: `docs/VIBESKUA_SPEC.md` (VibeSkua's
behaviour, Flash bridge, API and UI, surveyed from its source), `docs/SCRIPTING.md`
(writing scripts), `docs/UI.md`, `docs/HEADLESS.md` (running without a window: flags,
service, monitoring; keep it in step with the startup flags and the control API).

## Layout

Turu builds on **ruffle-aqw** (`ruffle/`, submodule `muktiarafi/ruffle-aqw`, branch `aqw`): Ruffle
`fda41800` plus Mugetsu 1.6.0's patches. This repo holds only Turu's own code.

| Path | What |
|---|---|
| `bot/` | crate `turu_bot`: the engine. No dependency on Ruffle or the app crate. TLS is a feature (`native-tls` default, `rustls-tls`). |
| `bot/js/turu.ts` | **The script API contract** (runtime + types, embedded). Rust implements each `"module.method"` it calls in `bot/src/api/<module>.rs`. Change both sides together. |
| `host/` | crate `turu_host`: Turu's own host SWF, `turu.swf`, the default (`docs/HOST_SWF_PROPOSAL.md`): its AS3 (`as3/`, package `turu.host`, written from `docs/VIBESKUA_SPEC.md`, not from VibeSkua's source) and its build with Ruffle's `asc.jar`. Rebuild and commit `bot/assets/turu.swf` after changing `as3/`; CI checks they match. |
| `app/` | crate `turu`: the app. Started from ruffle-aqw's `desktop/` (Ruffle's frontend + Mugetsu's changes, marked `AQW build:`); Turu's code is the rest. |
| `frontend/` | crate `turu_frontend`: what frontends share, with no winit/rfd (for Android later: `docs/ANDROID_PROPOSAL.md`). The app re-exports its modules (`crate::turu`, ...). |
| `android/` | crate `turu_android` + the Gradle app (`docs/ANDROID.md`): `TuruActivity` (NativeActivity) and `TuruService` (foreground service: starts the engine, also alone after a kill; notification; wake lock), the engine thread on `turu_frontend::engine` drawing the game and `TuruUi` with wgpu (`screen.rs`), setup from `turu-args.txt`. Submodule `android/vendor/rustls-platform-verifier` (HTTPS checks). |
| `air/`, `air_patch/` | Turu Mas (`docs/TURU_MAS.md`): crate `turu_air` (`air/ane/rust`, the engine as an AIR native extension, with its Kotlin in `air/ane/android`), the host SWF and `app.xml` (`air/app`), and crate `turu_air_patch` (patches AQW's client and the SWFs it loads for AIR's sandbox at run time). Built with the AIR SDK: `air/app/build.sh`. |
| `frontend/src/turu.rs` | Flash host (bot -> player via the event loop), ExternalInterface provider (player -> bot). |
| `frontend/src/gui/turu/` | egui UI: nav bar, Script Loader, Options, Skills, logs (VibeSkua layout, Mugetsu palette). |
| `scripts/` | Submodule `muktiarafi/turu-scripts`: TypeScript scripts (`core/` CoreBots/CoreFarms ports, `nation/`, `farm/`, ...), `tsconfig.json`, `types/turu.ts` (copy of `bot/js/turu.ts`), `tools/build-index.mjs` (the index Search Scripts reads). Commit there first. |
| `ruffle/` | Submodule ruffle-aqw: Ruffle's crates (`core`, `render`, `frontend-utils`, `video`) and Mugetsu's patched crates (`ruffle/patches/`, wired up by `[patch.crates-io]` in the root `Cargo.toml`). Change it when the fix belongs in the player: a missing or wrong Flash API, the player's performance, or AQW-specific behaviour Turu can't reach from outside. Turu's own logic stays in this repo, not in Ruffle. Commit in ruffle-aqw (marked `AQW build:` / `Turu:`), then bump the submodule. |

Sibling checkouts used as references (read-only): `../VibeSkua` (C# source of truth for
behaviour: `Skua.Core/Scripts/*.cs`, `Skua.AS3/skua/src`), `../Scripts` (the C# scripts we
port), `../mugetsu` (patch source).

Updating Ruffle or Mugetsu happens in ruffle-aqw: re-apply Mugetsu on the new Ruffle there, tag
it, bump `ruffle/`, then port what changed in ruffle-aqw's `desktop/` into `app/`
(`git -C ruffle diff <old>..<new> -- desktop`). Keep the root `Cargo.toml`'s
`[workspace.dependencies]`, `[patch.crates-io]` and profiles in step with `ruffle/Cargo.toml`
(Cargo reads patches and profiles only from the root workspace).

## Build, run, test

```bash
cargo build -p turu --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
./target/debug/turu                      # starts turu.swf -> AQW; log in with the game's own screen
cargo test -p turu_bot                   # engine + script runtime tests (fake Flash host)
cargo run -p turu_host                   # rebuild bot/assets/turu.swf from host/as3 (Java); `-- --check` as CI
cargo test -p turu -p turu_frontend --no-default-features --features turu/software_video,turu/external_video,turu/lzma,turu/jpegxr,turu/fontconfig-dlopen   # app + bot windows (see STATUS.md)
cargo clippy -p turu_bot -p turu_frontend -p turu --all-targets --no-default-features --features turu/software_video,turu/external_video,turu/lzma,turu/jpegxr,turu/fontconfig-dlopen -- -A clippy::redundant_clone   # see STATUS.md
cd android && ./gradlew assembleDebug   # Android APK (SDK, NDK 27, cargo-ndk: docs/ANDROID.md)
air/app/build.sh --no-install           # Turu Mas APK (AIR SDK, JDK 17: docs/TURU_MAS.md)
cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .    # type-check scripts
cd scripts && npx -y prettier@3.6.2 --write "**/*.{ts,mjs}"   # format scripts (their CI checks it)
```

Release builds: Mugetsu's `../mugetsu/source/build-*.sh` flow (dist profile, PGO) applies;
on macOS the feature set above is the one Mugetsu uses (Metal, fontconfig loaded at runtime).
Data lives in `dirs::data_dir()/Turu` (override with `TURU_DATA_DIR`).

## Rules that keep it working

* **No winit, rfd or window in `frontend/`.** Reach the app through `turu::set_tab_requests`,
  `TuruFlashHost::new` and `platform::platform()`; desktop-only pieces stay in `app/`.
* **Never call into Flash from the winit/UI thread.** `Flash::call` marshals to the event
  loop and blocks; from the UI thread it would wait for itself (the host refuses). UI actions
  that touch the game go through `turu::spawn_action`.
* **Never block or call Flash inside `Bot::on_flash_call`** (it runs while the player is
  locked). Post `events::Work` or update state instead.
* Bot calls on the script thread honour Stop/Pause through `ScriptControl::check`; long
  loops must call `bot.sleep`/`Flash::call` (or check `bot.should_exit()`) so Stop works.
* Behaviour follows VibeSkua's C# exactly (timeouts in 100 ms "tries", `ActionDelay` 800 ms,
  `world.lock` cooldowns, packet formats). When unsure, read the C#.
* Scripts are synchronous TypeScript: no `async`/`await` in the API. Types are stripped by
  oxc at load (no runtime type check); `tsc --noEmit` in `scripts/` is the type check.
* Keep `bot/js/turu.ts` readable for humans who write scripts: JSDoc on every member,
  VibeSkua names in camelCase, `string | number` for "name or ID".
* Match the surrounding code's style. Rust: `cargo fmt` for `bot/`; `frontend/src/gui/turu/`
  files use `max_width = 120`, formatted one file at a time (see STATUS.md). Upstream Ruffle files keep their
  conventions; mark Turu changes in them with `// Turu:` comments.
* Pull requests run `.github/workflows/ci.yml` (rebased on main, format, clippy, tests) and
  are merged by rebase only: see `CONTRIBUTING.md`.
* Don't commit unless asked.
