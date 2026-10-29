<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/turu/logo/turu-logo-dark.svg">
    <img src="docs/turu/logo/turu-logo-light.svg" alt="Turu" width="320">
  </picture>
</p>

# Turu

*Turu* is Javanese for "sleep": let the bot play while you do.

A cross-platform (macOS, Windows, Linux) bot client for AdventureQuest Worlds.

Turu works like [VibeSkua](../VibeSkua) (same windows, same flow, same game-side `skua.swf`),
but runs anywhere:

- **Flash player:** [Ruffle](https://ruffle.rs) with [Mugetsu](../mugetsu)'s AQW patches,
  drawing with Metal, Vulkan or DirectX 12.
- **Bot engine:** Rust (`bot/`).
- **Scripts:** TypeScript with full type information. No compiling step: the app loads
  `.ts` files directly.

This repository is a Ruffle fork (branch `turu`). Ruffle's own readme is
[README.ruffle.md](README.ruffle.md).

## Requirements

- [Rust](https://rustup.rs) 1.99 or newer (`rustup toolchain install stable`)
- Java 17+ (Ruffle compiles its Flash classes with it)
- Node.js 18+ (only to type-check scripts)
- Linux only:
  ```bash
  sudo apt install build-essential pkg-config libasound2-dev libudev-dev libfontconfig-dev libfreetype6-dev libssl-dev
  ```

## Run

macOS:

```bash
cargo run -p ruffle_desktop --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
```

Windows and Linux:

```bash
cargo run -p ruffle_desktop
```

The window opens with VibeSkua's top bar and loads AQW. Then:

1. Log in with the game's own login screen.
2. Open **Scripts > Script Loader** and click **Load Script**.
3. Pick a `.ts` file, e.g. `scripts/nation/materials/0MaxBags.ts`.
4. Click **Start Script**. Progress shows in the SCRIPT LOG.

**Stop Script** stops it; **Pause Script** pauses it.

Useful environment variables:

| Variable | Effect |
|---|---|
| `RUST_LOG=warn,turu=info` | Bot messages in the terminal (`turu=debug` for more) |
| `TURU_DATA_DIR=/some/dir` | Use another data folder (handy for a second account) |

## Build

The commands below are for macOS. On Windows and Linux, drop
`--no-default-features --features …`.

```bash
# Debug build: fast to compile, slower to run
cargo build -p ruffle_desktop --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen

# Release build, as Mugetsu ships it (LTO, much faster in game; takes a while)
cargo build -p ruffle_desktop --profile dist --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
```

The package is `ruffle_desktop`; the program it builds is `turu`. Binaries land in
`target/debug/turu` and `target/dist/turu` (`.exe` on Windows).

Packaging, including the macOS `.app`, the speed profile (PGO) and the Windows resources,
follows Mugetsu's scripts in `../mugetsu/source/build-*.sh` / `build-windows.ps1`. A Turu
version of those scripts isn't written yet.

## Test

```bash
cargo test -p turu_bot                                   # engine + script runtime (fake game, no login needed)
cargo test -p turu_bot --test scripts_transpile          # every script in scripts/ loads
cargo clippy -p turu_bot
cargo fmt -p turu_bot
```

## Scripts

```bash
cd scripts
npx -y -p typescript@5.9.3 tsc --noEmit -p .             # type-check all scripts
```

A minimal script:

```ts
import { bot } from "turu";

export default function main(): void {
    bot.map.join("greenguardwest", "West4", "Left");
    bot.skills.startAdvanced();
    bot.kill.monster("Frogzard");
    bot.log("done");
}
```

Calls are synchronous, so there's no `await`. The API with its docs is in
[bot/js/turu.ts](bot/js/turu.ts). The guide is
[docs/turu/SCRIPTING.md](docs/turu/SCRIPTING.md). CoreBots helpers are in
[scripts/core/CoreBots.ts](scripts/core/CoreBots.ts).

## Where things are

| Path | What |
|---|---|
| `bot/` | Bot engine (`turu_bot`): Flash bridge, script API, runners, TypeScript runtime |
| `bot/js/turu.ts` | The script API (types + runtime) |
| `bot/assets/skua.swf` | VibeSkua's game-side SWF, embedded in the app |
| `desktop/src/turu.rs` | Connects the bot to the Flash player |
| `desktop/src/gui/turu/` | The UI (top bar, Script Loader, options, logs, ...) |
| `scripts/` | TypeScript scripts (`core/`, `nation/`, ...) |
| `patches/` | Mugetsu's patched crates |
| `docs/turu/` | [Architecture](docs/turu/ARCHITECTURE.md), [UI](docs/turu/UI.md), [scripting](docs/turu/SCRIPTING.md), [VibeSkua reference](docs/turu/VIBESKUA_SPEC.md), [status](docs/turu/HANDOFF.md) |

## Data folder

Settings, the options file, script options, the skill sets (`UserAdvancedSkills.json` is yours, `AdvancedSkills.json` the shipped ones) and the `skua.swf` copy live in:

| OS | Folder |
|---|---|
| macOS | `~/Library/Application Support/Turu` |
| Windows | `%APPDATA%\Turu` |
| Linux | `~/.local/share/Turu` |

Ruffle's own log is in its cache folder (`~/Library/Caches/ruffle/log` on macOS,
`%LOCALAPPDATA%\ruffle\log` on Windows, `~/.cache/ruffle/log` on Linux).

## Status

The engine, the UI and the 0MaxBags port are complete but not yet tested in a live game.
Auto-relogin, the script browser and the skills editor aren't built yet.
[docs/turu/HANDOFF.md](docs/turu/HANDOFF.md) has the details.

## License

Ruffle is MIT / Apache-2.0. Mugetsu's changes are GPL-3.0, so builds of Turu are GPL-3.0.
Turu isn't affiliated with or endorsed by Artix Entertainment, the Ruffle project or Mugetsu.
