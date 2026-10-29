# Turu

Cross-platform (macOS, Windows, Linux) AdventureQuest Worlds bot client: a port of
VibeSkua (C#/Avalonia, Windows only) onto **Ruffle + Mugetsu's AQW patches**, with the
bot engine in **Rust** and scripts in **TypeScript**. Same UI/UX and flow as VibeSkua.

Read first: `docs/turu/ARCHITECTURE.md`. Reference: `docs/turu/VIBESKUA_SPEC.md` (VibeSkua's
behaviour, Flash bridge, API and UI, surveyed from its source), `docs/turu/SCRIPTING.md`
(writing scripts), `docs/turu/UI.md`.

## Layout

This repo is a Ruffle fork (branch `turu`, based on Ruffle `fda41800` = Mugetsu 1.6.0's base).

| Path | What |
|---|---|
| `bot/` | crate `turu_bot`: the engine. No dependency on Ruffle or the desktop crate. |
| `bot/js/turu.ts` | **The script API contract** (runtime + types, embedded). Rust implements each `"module.method"` it calls in `bot/src/api/<module>.rs`. Change both sides together. |
| `bot/assets/skua.swf` | VibeSkua's AS3 host (`Skua.AS3/skua/bin/skua.swf`), unchanged. |
| `desktop/src/turu.rs` | Flash host (bot -> player via the event loop), ExternalInterface provider (player -> bot). |
| `desktop/src/gui/turu/` | egui UI: nav bar, Script Loader, Options, Skills, logs (VibeSkua layout, Mugetsu palette). |
| `scripts/` | TypeScript scripts: `core/` (CoreBots/CoreFarms ports), `nation/`, `types/`, `tsconfig.json`. |
| `patches/` | Mugetsu's patched crates (`[patch.crates-io]`). Don't edit; re-apply from Mugetsu instead. |
| Everything else | Upstream Ruffle + Mugetsu's changes (marked `AQW build:`). Keep Turu changes out of `core/` and `render/` unless a Flash API is missing. |

Sibling checkouts used as references (read-only): `../VibeSkua` (C# source of truth for
behaviour: `Skua.Core/Scripts/*.cs`, `Skua.AS3/skua/src`), `../Scripts` (the C# scripts we
port), `../mugetsu` (patch source).

## Build, run, test

```bash
cargo build -p ruffle_desktop --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
./target/debug/turu                      # starts skua.swf -> AQW; log in with the game's own screen
cargo test -p turu_bot                   # engine + script runtime tests (fake Flash host)
cargo clippy -p turu_bot -p ruffle_desktop --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
cd scripts && npx -y -p typescript@5.9.3 tsc --noEmit -p .    # type-check scripts
```

Release builds: Mugetsu's `../mugetsu/source/build-*.sh` flow (dist profile, PGO) applies;
on macOS the feature set above is the one Mugetsu uses (Metal, fontconfig loaded at runtime).
Data lives in `dirs::data_dir()/Turu` (override with `TURU_DATA_DIR`).

## Rules that keep it working

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
* Match the surrounding code's style. Rust: `cargo fmt`. Upstream Ruffle files keep their
  conventions; mark Turu changes in them with `// Turu:` comments.
* Don't commit unless asked.
