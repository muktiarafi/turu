# Turu

Cross-platform (macOS, Windows, Linux, Android) AdventureQuest Worlds bot client: a port of
VibeSkua (C#/Avalonia, Windows only) onto **Ruffle + Mugetsu's AQW patches**, with the
bot engine in **Rust** and scripts in **TypeScript**. The egui UI keeps VibeSkua's layout and flow;
a native shell (Android's, in Compose: the tabs, the rail and the daily windows) follows its platform and its approved design
pass in `docs/UI.md` ("Native shells"), with every feature reachable and behaving the same.

Read first: `docs/ARCHITECTURE.md` and `docs/STATUS.md` (what works, known gaps,
tooling quirks). Continuing the Mugetsu 2.0.0 update or the process tabs (M6):
`docs/MUGETSU_2_M6_HANDOVER.md` (state, open items, decisions, traps). The Android port: `docs/ANDROID_HANDOVER.md` (start here: state,
branches, setup, testing, pitfalls), `docs/ANDROID_PROPOSAL.md` (plan) and `docs/ANDROID.md`
(build, run); what of Turu's UI is native Android there, and why: `docs/ANDROID_NATIVE_UI.md`. Native
windows on Android (Compose pages over view models, branch `native-ui`): `docs/NATIVE_UI_PROPOSAL.md`,
"L4 and L5b so far" and its "Handover".
Turu Mas, the Android app on Adobe AIR: `docs/TURU_MAS.md` (build, release,
license; how it came about: `docs/AIR_PROPOSAL.md`). Reference: `docs/VIBESKUA_SPEC.md` (VibeSkua's
behaviour, Flash bridge, API and UI, surveyed from its source), `docs/SCRIPTING.md`
(writing scripts), `docs/UI.md`, `docs/HEADLESS.md` (running without a window: flags,
service, monitoring; keep it in step with the startup flags and the control API).

## Layout

Turu builds on **ruffle-aqw** (`ruffle/`, submodule `muktiarafi/ruffle-aqw`, branch `aqw`): Ruffle
`f22d29d9` plus Mugetsu 2.0.0's patches. This repo holds only Turu's own code.

| Path | What |
|---|---|
| `bot/` | crate `turu_bot`: the engine. No dependency on Ruffle or the app crate. TLS is a feature (`native-tls` default, `rustls-tls`). |
| `bot/js/turu.ts` | **The script API contract** (runtime + types, embedded). Rust implements each `"module.method"` it calls in `bot/src/api/<module>.rs`. Change both sides together. |
| `host/` | crate `turu_host`: Turu's own host SWF, `turu.swf`, the default (`docs/HOST_SWF_PROPOSAL.md`): its AS3 (`as3/`, package `turu.host`, written from `docs/VIBESKUA_SPEC.md`, not from VibeSkua's source) and its build with Ruffle's `asc.jar`. Rebuild and commit `bot/assets/turu.swf` after changing `as3/`; CI checks they match. |
| `app/` | crate `turu`: the app. Started from ruffle-aqw's `desktop/` (Ruffle's frontend + Mugetsu's changes, marked `AQW build:`); Turu's code is the rest. |
| `shell/` | crate `turu_shell`: the shells' logic without a window (docs/NATIVE_UI_PROPOSAL.md, L1). Android's: `slots::SlotShell`, the tab list over the slots (`android/src/slot_shell.rs` carries out its actions; Android takes the crate with `default-features = false`). The desktop's (the default `desktop` feature): the tabs' clients started and restarted, the tab list, Grid View, the clients' pictures on the shell's GPU and their pacing, the dialog queue, updates, Army launches, the monitor's peers. `app/src/shell.rs` is its winit front (the window, its GPU choice, input, rfd); the front carries out the `Action`s the shell returns. Takes `turu_frontend` without `ruffle`; tested with a fake client (`tests/shell.rs`). |
| `frontend/` | crate `turu_frontend`: what frontends share, with no winit/rfd (shared with both Android apps: `docs/ANDROID.md`, `docs/TURU_MAS.md`). The app re-exports its modules (`crate::turu`, ...). |
| `android/` | crate `turu_android` + the Gradle app (`docs/ANDROID.md`): `TuruActivity` (AndroidX's GameActivity) and `TuruService` (foreground service: starts the engine, also alone after a kill; notification; wake lock), the engine thread on `turu_frontend::engine` drawing the game and `TuruUi` with wgpu (`screen.rs`), setup from `turu-args.txt`. Submodule `android/vendor/rustls-platform-verifier` (HTTPS checks). `android/shared`: the Kotlin both Android apps share (crash records and uploads, the text field above the keyboard); Turu Mas's extension includes it by path. |
| `air/`, `air_patch/` | Turu Mas (`docs/TURU_MAS.md`): crate `turu_air` (`air/ane/rust`, the engine as an AIR native extension, with its Kotlin in `air/ane/android`), the host SWF and `app.xml` (`air/app`), and crate `turu_air_patch` (patches AQW's client and the SWFs it loads for AIR's sandbox at run time). Built with the AIR SDK: `air/app/build.sh`. |
| `frontend/src/turu.rs`, `frontend/src/turu/ruffle_host.rs` | Tabs and `spawn_action`; `ruffle_host.rs` holds the Flash host (`TuruFlashHost`, bot -> player via the event loop) and the ExternalInterface provider (player -> bot). |
| `frontend/src/gui/turu/` | egui UI: nav bar, Script Loader, Options, Skills, logs (VibeSkua layout, Mugetsu palette). |
| `scripts/` | Submodule `muktiarafi/turu-scripts`: TypeScript scripts (`core/` CoreBots/CoreFarms ports, `nation/`, `farm/`, ...), `tsconfig.json`, `types/turu.ts` (copy of `bot/js/turu.ts`), `tools/build-index.mjs` (the index Search Scripts reads). Commit there first. |
| `ruffle/` | Submodule ruffle-aqw: Ruffle's crates (`core`, `render`, `frontend-utils`, `video`) and Mugetsu's patched crates (`ruffle/patches/`, wired up by `[patch.crates-io]` in the root `Cargo.toml`). Change it when the fix belongs in the player: a missing or wrong Flash API, the player's performance, or AQW-specific behaviour Turu can't reach from outside. Turu's own logic stays in this repo, not in Ruffle. Commit in ruffle-aqw (marked `AQW build:` / `Turu:`), then bump the submodule. |

Sibling checkouts used as references (read-only): `../VibeSkua` (C# source of truth for
behaviour: `Skua.Core/Scripts/*.cs`, `Skua.AS3/skua/src`), `../Scripts` (the C# scripts we
port), `../mugetsu` (patch source).

Updating Ruffle or Mugetsu happens in ruffle-aqw: re-apply Mugetsu on the new Ruffle there, format
it before re-applying Turu's commits (ruffle-aqw's `.github/CONTRIBUTING.md`), tag it, bump
`ruffle/`, then port what changed in ruffle-aqw's `desktop/` into `app/`
(`git -C ruffle diff <old>..<new> -- desktop`). Keep the root `Cargo.toml`'s
`[workspace.dependencies]`, `[patch.crates-io]` and profiles in step with `ruffle/Cargo.toml`
(Cargo reads patches and profiles only from the root workspace).

## Build, run, test

```bash
cargo build -p turu --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
./target/debug/turu                      # starts turu.swf -> AQW; log in with the game's own screen
cargo nextest run -p turu_bot            # engine + script runtime tests (fake Flash host): ~25 s, `cargo test` ~30 s (see below)
cargo nextest run -p turu_shell          # the desktop shell's logic with fake clients, no window: a few seconds
cargo run -p turu_host                   # rebuild bot/assets/turu.swf from host/as3 (Java); `-- --check` as CI
cargo test -p turu -p turu_frontend --no-default-features --features turu/software_video,turu/external_video,turu/lzma,turu/jpegxr,turu/fontconfig-dlopen   # app + bot windows (see STATUS.md); cargo test here, not nextest (see below)
cargo clippy -p turu_bot -p turu_frontend -p turu_shell -p turu --all-targets --no-default-features --features turu/software_video,turu/external_video,turu/lzma,turu/jpegxr,turu/fontconfig-dlopen -- -D warnings -A clippy::redundant_clone   # see STATUS.md
cd android && ./gradlew assembleDebug   # Android APK (SDK, NDK 27, cargo-ndk: docs/ANDROID.md)
air/app/build.sh --no-install           # Turu Mas APK (AIR SDK, JDK 17: docs/TURU_MAS.md)
docker build -t turu .                  # headless image (Dockerfile, compose.yaml: docs/HEADLESS.md, "Docker")
cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .    # type-check scripts
cd scripts && npx -y prettier@3.6.2 --write "**/*.{ts,mjs}"   # format scripts (their CI checks it)
```

Run the engine's tests with cargo-nextest (`.config/nextest.toml`): each test in its own process,
every test binary at once. `cargo nextest run -p turu_bot <name>` runs the tests whose names
contain `<name>` (`<file>::` for one `bot/tests/` file: they are modules of one test binary,
`bot/tests/main.rs`, where a new file needs its `mod` line), `--run-ignored only` the ignored ones.
Install it with `brew install cargo-nextest` or `cargo install cargo-nextest --locked`; without
it, use `cargo test`. Keep `cargo test` for the app and frontend on macOS: the system checks each
newly built test program at launch, and nextest launches one per test (over 100 s against
seconds).
CI runs all four crates (`turu_bot`, `turu`, `turu_frontend`, `turu_shell`) in one `cargo nextest run --profile ci` (ci.yml).

Release builds: Mugetsu's `../mugetsu/source/build-*.sh` flow (dist profile, PGO) applies;
on macOS the feature set above is the one Mugetsu uses (Metal, fontconfig loaded at runtime).
Data lives in `dirs::data_dir()/Turu` (override with `TURU_DATA_DIR`).

## Rules that keep it working

* **No winit, rfd or window in `frontend/` or `shell/`** (CI checks `turu_shell`'s dependencies). Reach the app through `turu::set_tab_requests`,
  `TuruFlashHost::new` and `platform::platform()`; desktop-only pieces stay in `app/`.
* **Never call into Flash from the winit/UI thread.** `Flash::call` marshals to the event
  loop and blocks; from the UI thread it would wait for itself (the host refuses). UI actions
  that touch the game go through `turu::spawn_action`.
* **Never block or call Flash inside `Bot::on_flash_call`** (it runs while the player is
  locked). Post `events::Work` or update state instead.
* Bot calls on the script thread honour Stop/Pause through `ScriptControl::check`; long
  loops must call `bot.sleep`/`Flash::call` (or check `bot.should_exit()`) so Stop works.
* Kotlin and Rust meet only through each Android app's `java.rs` (`android/src/`,
  `air/ane/rust/src/`): every Kotlin method Rust calls and every `external` it implements is
  bound there (`bind_java_type!`) and checked when the app starts. Change both sides together and
  start the build once (docs/ANDROID_NATIVE_UI.md, "Platform calls").
* **What a native page sends and reads is typed** (the models in `frontend/src/models/`). A
  model's actions are one `<Name>Act` enum, added with `acts` in `models::register()`; what an
  action answers is a record, never `json!`; its name is `models::Model`. No action takes the raw
  body (`body: &Json`), and no Android page reads or builds JSON by hand (`org.json`): it sends
  `client.act(Api.<model>, <Name>Act...)` from the types `turu-typegen` generates. What the
  shell's activity and its Rust say over JNI is typed the same way (`turu_frontend::native_shell`:
  `ShellAction`, `SlotMessage`, the Accounts' records). The `models_stay_typed` test (in CI) and
  the Compose hook refuse the untyped forms. Adding a model:
  docs/NATIVE_UI_PROPOSAL.md, "Handover", the steps a page takes.
* Behaviour follows VibeSkua's C# by default (timeouts in 100 ms "tries", `ActionDelay` 800 ms,
  `world.lock` cooldowns, packet formats). When unsure, read the C#. Turu differs only where
  there is a reason you can state (the server refused it, it hung, the platform differs): mark
  the code `Turu:` with the reason (as `bot/` does: `// Turu: VibeSkua spins here without
  sleeping.`), and add what a user or script would notice to STATUS.md's "Differences from
  VibeSkua". Never remove a marked difference to match the C#.
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

## The owner's standing requests

* **UI work follows Impeccable**: load its skill, run `impeccable context --target <file>` once,
  read its platform reference and craft-floor before editing, `impeccable detect <file>` after,
  and look at the result (Android: emulator screenshots at font scale 1.0 and 1.3, and with the
  keyboard up). Impeccable's own hook skips every file here (`skipped: native-platform`:
  PRODUCT.md's platform is `adaptive`) and its detector has no Compose rules, so the owner chose
  (2026-11-20) a hook of Turu's beside it: `.claude/hooks/compose-design-check.py` checks each
  edited Kotlin screen (colours outside `ShellTheme.kt`, text under 12 sp or in dp, touch targets
  under 48 dp, gradients, glyph icons, em dashes, suppressed deprecations...) plus `impeccable
  detect`, and makes a deep pass on Stop; wire it in `.claude/settings.local.json` beside
  Impeccable's entries. Act on its findings; waive a deliberate one on its line
  (`// impeccable-disable-line <rule>: <why>`). Run it by hand with `--check <files>`. Don't
  edit `.impeccable/config.json` to force Impeccable's. Android's shell looks like
  `ShellTheme.kt` and docs/UI.md's pass, not DESIGN.md (the web monitor's).
* **Always format, and clear the warnings** (owner, 2026-11-22): every change ends with its code
  formatted (STATUS.md, "Formatting": the bot crate, the UI windows at 120 columns, and in
  `ruffle/` its workspace and `patches/` crates) and with the clippy command above, and the
  build, showing no warning, Ruffle's crates included. Fix a warning rather than allow it; where
  the fix is wrong (clippy's suggestion doesn't compile, a stub must keep a signature), allow it on
  that item with a `reason`. Prefer `allow` to `expect` in Ruffle's patched crates: a newer rustc
  can make an `expect` unfulfilled.
* **Search Scripts wherever a script is chosen**: every screen that picks a script offers
  "Search scripts…" opening Search Scripts picking for it, never only the Script Loader's
  (docs/UI.md, "Search Scripts everywhere a script is chosen"; Android's `ScriptPick`). Not as an
  Android rail item: one was tried and taken off to keep Jump.
* **Kotlin uses no deprecated API**, and none is hidden with `@Suppress("DEPRECATION")`: use the
  AndroidX compat replacement. Gradle prints no `w:` for the files touched.
* **Made-up player names** in tests, fixtures, examples and docs ("Questwalker"), never the
  owner's account or character names from logs and screenshots; replace them when copying a
  packet or log line.
* **Nothing relogs on a crowded server**: a check script or test plan that relogs names a quiet
  server, never Twilly (Turu's default relogin server), Artix or Alteon.
* **The emulator renders on the host GPU**: `emulator -avd turu_api36 -gpu host` (the AVD alone
  gives llvmpipe, which skews memory and timing); check the "Graphics:" log line.
* **On the emulator the tabs are the owner's real accounts**: start no script on them unasked,
  test with made-up accounts and remove them after, and set back any setting or data a check
  changed (docs/NATIVE_UI_PROPOSAL.md, "Handover", has the details).
* **Never push or open a pull request unasked.**
