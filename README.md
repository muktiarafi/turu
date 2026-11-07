<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/logo/turu-logo-dark.svg">
    <img src="docs/logo/turu-logo-light.svg" alt="Turu" width="320">
  </picture>
</p>

# Turu

*Turu* is Javanese for "sleep": let the bot play while you do.

A cross-platform (macOS, Windows, Linux, Android) bot client for AdventureQuest Worlds. It works
like [VibeSkua](https://github.com/NinjaXz/VibeSkua) (same windows, same flow), but runs on
[Ruffle](https://ruffle.rs) with [Mugetsu](https://github.com/mugetsu-aqw/mugetsu)'s AQW
patches, with a Rust bot engine and TypeScript scripts.

## Download

Beta builds for Windows, macOS, Linux and Android (arm64, 8+) are on
[Releases](https://github.com/muktiarafi/turu/releases). Turu updates itself.

- **macOS:** the app isn't notarized. If it won't open, allow it in System Settings > Privacy &
  Security, or run `xattr -dr com.apple.quarantine Turu.app`.
- **Android:** no Army, Butler or HotKeys yet ([docs/ANDROID.md](docs/ANDROID.md)).
- **Crash reports** (Sentry) are sent only if you say yes at first start; change it in Options >
  Application.

## Use

1. Log in with the game's own login screen.
2. **Scripts > Search Scripts**, pick a script (e.g. "0MaxBags"), **Start**. Or **Scripts >
   Script Loader > Load Script** for a local `.ts` file.
3. Watch the SCRIPT LOG; **Stop** or **Pause** any time.

Running on a server without a window: [docs/HEADLESS.md](docs/HEADLESS.md).

## Build

Needs [Rust](https://rustup.rs) 1.99+, Java 17+, and on Linux
`build-essential pkg-config libasound2-dev libudev-dev libfontconfig-dev libfreetype6-dev libssl-dev`.

```bash
git clone --recursive https://github.com/muktiarafi/turu.git
cd turu
cargo run -p turu                 # Windows, Linux
cargo run -p turu --no-default-features --features software_video,external_video,lzma,jpegxr,fontconfig-dlopen   # macOS
cargo test -p turu_bot            # engine + script runtime
```

Add `--profile dist` for a release build. Android: [docs/ANDROID.md](docs/ANDROID.md).

## Scripts

```ts
import { bot } from "turu";

export default function main(): void {
    bot.map.join("greenguardwest", "West4", "Left");
    bot.skills.startAdvanced();
    bot.kill.monster("Frogzard");
}
```

Calls are synchronous (no `await`). API: [bot/js/turu.ts](bot/js/turu.ts). Guide:
[docs/SCRIPTING.md](docs/SCRIPTING.md). Scripts live in
[turu-scripts](https://github.com/muktiarafi/turu-scripts).

## Docs

[Architecture](docs/ARCHITECTURE.md) · [Status](docs/STATUS.md) · [UI](docs/UI.md) ·
[Headless](docs/HEADLESS.md) · [Android](docs/ANDROID.md) ·
[VibeSkua reference](docs/VIBESKUA_SPEC.md)

## Credits

- [Skua](https://github.com/auqw/skua) and [Scripts](https://github.com/auqw/Scripts) by the auqw
  team (SharpTheNightmare, wtffidy, omega13-engr, BrenoHenrike, PUNK3DAF, LordExelot, l0newolf12
  and contributors): the original framework, script API and the C# scripts Turu's scripts port.
- [VibeSkua](https://github.com/NinjaXz/VibeSkua) by NinjaXz, building on
  [ArrowDev's Avalonia port](https://github.com/ArrowDev123/Skua): the client Turu reproduces,
  and its `skua.swf`, which Turu ships unchanged.
- [Ruffle](https://ruffle.rs) and [Mugetsu](https://github.com/mugetsu-aqw/mugetsu): the Flash
  player and its AQW patches.

## License

Turu is GPL-3.0-or-later ([LICENSE](LICENSE)). Ruffle is MIT / Apache-2.0
([LICENSE-RUFFLE.md](LICENSE-RUFFLE.md)); Mugetsu is GPL-3.0. Skua, VibeSkua and Scripts publish
no license, so the GPL doesn't cover `skua.swf` or the ported scripts; their authors keep their
rights (open an issue if you're one of them and want something changed). Turu isn't affiliated
with Artix Entertainment or any of the projects above.
