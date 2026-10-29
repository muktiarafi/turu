# Turu handoff (2026-10-29)

Where the first session stopped and what to do next. Read `CLAUDE.md` and
`docs/turu/ARCHITECTURE.md` first.

## Goal

Run `../Scripts/Nation/Materials/0MaxBags.cs`, ported to TypeScript, on Turu: a Rust/Ruffle
(Mugetsu-patched) port of VibeSkua with VibeSkua's UI layout in egui.

## Done and verified

- Branch `turu` at Ruffle `fda41800` plus Mugetsu 1.6.0's patches. This is one commit, `583a33e5f`, made
  without being asked; squash or keep as you like. `patches/` holds the five patched crates.
- `bot/` crate `turu_bot`:
  - Flash bridge (`flash.rs`, `host.rs`).
  - Stop and pause (`control.rs`): the next bot call fails, and a QuickJS interrupt handler
    also stops scripts that loop without calling the bot.
  - Event bus, options, paths and logs.
  - Script runtime: oxc strips the types, QuickJS runs the result, imports are relative or
    `"turu"`, and there is a `__turu` dispatch.
  - `cargo test -p turu_bot --test runtime` passed. It runs TS with imports, stops a busy
    script, and reports errors with locations.
- `bot/js/turu.ts`: the typed script API contract, with every Bot member the 0MaxBags
  closure uses.
- Desktop: `desktop/src/turu.rs` marshals bot calls through `RuffleEvent::TuruFlashCall`, and
  the ExternalInterface provider is wired in `player.rs`/`app.rs`. The app starts the embedded
  `skua.swf` instead of Loader3.
  - **Verified live:** `skua.swf` boots, calls `requestLoadGame`, the bot answers
    `loadClient`, and AQW loads ("Game client loaded" in the log).
- Known noise: skua.swf's `QuestRequirementWiki` module logs a `ModalStack` null error every
  100 ms before login (Flash's release player hid it). It's harmless; filter it if wanted.

## Second session (same day): everything implemented, not yet live-tested

All four agents finished. On disk, not committed:

- **Engine (`bot/`)**: every one of the 209 `call("x.y")`s in `turu.ts` has a Rust arm.
  `cargo test -p turu_bot` passes (45 tests, all against a fake host).
- **Scripts (`scripts/`)**: CoreBots, CoreFarms and CoreNation (the parts 0MaxBags reaches)
  plus `nation/materials/0MaxBags.ts`, 6.2k lines. `tsc --noEmit` is clean, every file
  transpiles (`--test scripts_transpile`), and 0MaxBags loads in QuickJS and stops cleanly
  (`--test runtime max_bags`).
- **UI (`desktop/src/gui/turu/`)**: the VibeSkua nav bar, Script Loader, Game Options, Logs,
  Logger (opt-in via `logs.packet_logging`), Stats, Skills (saved sets), Current Drops, and
  Auto Attack. See `docs/turu/UI.md`. Entries not built yet are greyed out ("Coming soon").
- The app builds, boots AQW and doesn't panic.

### Next steps

1. **Live test.** Run the app, log in, open Scripts > Script Loader, try a small script (see
   `docs/turu/SCRIPTING.md`), then `scripts/nation/materials/0MaxBags.ts`. Watch
   Diagnostics > Logs. Expect fixes in game paths and timings.
2. Known gaps reported by the agents:
   - No `servers` API, so no auto-relogin or watchdog. `Core.relogin` logs out, waits and
     stops the script.
   - `FlashArg` has no object variant, so equipping uses the `equipItem` packet. Check
     consumables in game.
   - No `Handlers` in the engine; Core polls its handlers inside `Core.sleep`/`waitForTrue`.
   - No counter-attack pause (`StopAttacking`).
   - No boosts or reputation APIs (Core reads `world.myAvatar.factions`).
   - Quest rewards and shop items lack some fields; Core reads them from raw game objects.
   - `turu.ts` could expose `invHelper`, `registerQuests` with reward IDs, and a pads list.
3. Later: Script Repo browser, Skills editor, Scheduler, HotKeys, Application options, and
   release packaging (adapt `../mugetsu/source/build-*.sh` to Turu).

## Reference material

- `docs/turu/VIBESKUA_SPEC.md`: VibeSkua survey (bridge, every callback, API behaviour, UI).
- `docs/turu/handoff/maxbags-closure.md`: the full 0MaxBags dependency closure, 329
  members, with the C# copied verbatim from `../Scripts` (GPL; that repo's license).
  Delete it once the port is done.
- `docs/turu/handoff/mugetsu-report.md`: what Mugetsu's patch changes, including the egui palette.
