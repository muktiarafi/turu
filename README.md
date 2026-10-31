<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/logo/turu-logo-dark.svg">
    <img src="docs/logo/turu-logo-light.svg" alt="Turu" width="320">
  </picture>
</p>

# Turu

*Turu* is Javanese for "sleep": let the bot play while you do.

A cross-platform (macOS, Windows, Linux) bot client for AdventureQuest Worlds.

Turu works like [VibeSkua](https://github.com/NinjaXz/VibeSkua) (same windows, same flow, same
game-side `skua.swf`), but runs anywhere:

- **Flash player:** [Ruffle](https://ruffle.rs) with
  [Mugetsu](https://github.com/mugetsu-aqw/mugetsu)'s AQW patches, drawing with Metal, Vulkan or
  DirectX 12.
- **Bot engine:** Rust (`bot/`).
- **Scripts:** TypeScript with full type information. No compiling step: the app loads
  `.ts` files directly.

The Flash player comes from [ruffle-aqw](https://github.com/muktiarafi/ruffle-aqw) (Ruffle with
Mugetsu's patches), the `ruffle/` submodule. Turu builds on its crates and keeps its own app
(`app/`) and engine (`bot/`).

## Download

[Releases](https://github.com/muktiarafi/turu/releases) has packages for Windows (x86-64),
macOS (one app for Apple silicon and Intel) and Linux (x86-64). They're alpha builds. macOS
doesn't know the app (it isn't notarized): if it won't open the first time, allow it in
System Settings > Privacy & Security, or run `xattr -dr com.apple.quarantine Turu.app`.

To build it yourself, read on.

## Get the code

```bash
git clone --recursive https://github.com/muktiarafi/turu.git
```

Both submodules are needed: `ruffle/` ([ruffle-aqw](https://github.com/muktiarafi/ruffle-aqw),
the Flash player's crates) for every build, `scripts/`
([turu-scripts](https://github.com/muktiarafi/turu-scripts)) for the tests and the type-check.
In a clone made without `--recursive`, run `git submodule update --init`.

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
cargo run -p turu --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
```

Windows and Linux:

```bash
cargo run -p turu
```

The window opens with VibeSkua's top bar and loads AQW. Then:

1. Log in with the game's own login screen.
2. Open **Scripts > Search Scripts**, find a script (e.g. "0MaxBags") and click **Start**. It is
   downloaded, with the files it imports, from
   [turu-scripts](https://github.com/muktiarafi/turu-scripts). Or open **Scripts > Script
   Loader**, click **Load Script**, pick a `.ts` file and click **Start Script**.
3. Progress shows in the Script Loader's SCRIPT LOG.

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
cargo build -p turu --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen

# Release build, as Mugetsu ships it (LTO, much faster in game; takes a while)
cargo build -p turu --profile dist --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen
```

Binaries land in `target/debug/turu` and `target/dist/turu` (`.exe` on Windows).

Releases are built by `.github/workflows/release.yml`: pushing a tag such as `v0.1.0-alpha.2`
tests the engine and scripts, builds the `dist` profile for each platform, makes the macOS
`.app` and publishes a GitHub release (a tag with a `-` makes a pre-release). Mugetsu's speed
profile (PGO) isn't used yet.

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
[docs/SCRIPTING.md](docs/SCRIPTING.md). CoreBots helpers are in
[scripts/core/CoreBots.ts](scripts/core/CoreBots.ts).

## Where things are

| Path | What |
|---|---|
| `bot/` | Bot engine (`turu_bot`): Flash bridge, script API, runners, TypeScript runtime |
| `bot/js/turu.ts` | The script API (types + runtime) |
| `bot/assets/skua.swf` | VibeSkua's game-side SWF, embedded in the app |
| `app/src/turu.rs` | Connects the bot to the Flash player |
| `app/src/gui/turu/` | The UI (top bar, Script Loader, options, logs, ...) |
| `app/` | The app (`turu`): window, player tabs, UI; started from Ruffle's desktop frontend |
| `scripts/` | TypeScript scripts (`core/`, `nation/`, ...) |
| `ruffle/` | [ruffle-aqw](https://github.com/muktiarafi/ruffle-aqw): Ruffle with Mugetsu's patches |
| `docs/` | [Architecture](docs/ARCHITECTURE.md), [UI](docs/UI.md), [scripting](docs/SCRIPTING.md), [VibeSkua reference](docs/VIBESKUA_SPEC.md), [status](docs/STATUS.md) |

## Data folder

The bot options (`options.json`), the CoreBots Options (`options/CBO_Storage(<username>).txt`),
the skill sets (`UserAdvancedSkills.json` is yours, `AdvancedSkills.json` the shipped ones) and
the `skua.swf` copy live in:

| OS | Folder |
|---|---|
| macOS | `~/Library/Application Support/Turu` |
| Windows | `%APPDATA%\Turu` |
| Linux | `~/.local/share/Turu` |

The player's log is in Ruffle's cache folder (`~/Library/Caches/ruffle/log` on macOS,
`%LOCALAPPDATA%\ruffle\log` on Windows, `~/.cache/ruffle/log` on Linux).

## Status

Alpha. The engine, auto-relogin, the main windows, Search Scripts, Army and the web monitor
work, and the ported scripts are being tested in game. Several of VibeSkua's tools (the skills
editor, Scheduler, Loadouts, ...) aren't built yet. [docs/STATUS.md](docs/STATUS.md) lists what
works and what's missing.

## License

Turu is GPL-3.0-or-later ([LICENSE](LICENSE)). It builds on Ruffle, MIT / Apache-2.0
([LICENSE-RUFFLE.md](LICENSE-RUFFLE.md)), and Mugetsu's changes, GPL-3.0.
Turu isn't affiliated with or endorsed by Artix Entertainment, the Ruffle project or Mugetsu.
