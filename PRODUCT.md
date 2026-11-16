# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

Desktop (macOS, Windows, Linux) and Android, drawn with egui on top of the Ruffle player, plus
Turu Monitor, a web dashboard Turu serves itself. The Android app runs the game in Ruffle and is
the default; Turu Mas is a second Android app that runs it in the Adobe AIR runtime instead, the
alternative for when Ruffle misbehaves on Android (AIR is much more stable at running Flash,
though not as fast as Ruffle). Design work should follow each OS's own conventions where they
matter (Android especially), rather than one identical look everywhere.

## Users

Primary: AdventureQuest Worlds players who already know VibeSkua/Skua. They pick a script, start
it, and leave it running (overnight, while they do something else, often several accounts at
once), then check in now and then to see that it is still farming, read the log, or restart it.
Turu now lets them do this off Windows too: on a Mac, a Linux server (windowless), or a phone.

## Product Purpose

A cross-platform AQW bot client. It runs the game in Ruffle with Mugetsu's AQW patches, drives
it with a Rust bot engine through Turu's own host SWF, `turu.swf` (written from a survey of
VibeSkua's `skua.swf`, answering the same calls), and runs TypeScript scripts (ports of the
community's C# scripts). The name is Javanese for "sleep": the bot plays while you don't.
Success is a script that keeps farming unattended, recovers from disconnects on its own, and is
easy to check on from wherever the user is.

## Positioning

The VibeSkua experience (same windows, same flow, same script library) on every platform,
including Android and headless servers, with scripts in typed TypeScript loaded directly with no
compile step, and built-in monitoring and control (Turu Monitor, control API,
Army) instead of a separate relay app.

## Operating Context

- Mostly unattended: long runs, auto-relogin, Scheduler playlists, multiple tabs/accounts (Army).
- Check-ins are short: glance at status, logs, drops and stats; stop, restart or relog.
- Desktop window with floating tool windows over the game; Android phone screen with a soft
  keyboard; windowless mode on a server watched through the terminal or Turu Monitor.
- Script authors edit `.ts` files in VS Code and load them from the Script Loader.

## Capabilities and Constraints

- Feature set and terminology follow VibeSkua (see `docs/VIBESKUA_SPEC.md`, `docs/UI.md`);
  behaviour follows VibeSkua's C# by default, and differs only for a stated reason
  (`docs/STATUS.md`, "Differences from VibeSkua").
- UI lives in `frontend/src/gui/turu/` (egui), shared by desktop and Android; no winit/rfd in
  `frontend/`. UI code only reads cached bot state and never calls into Flash directly.
- The game view stays primary: bot UI must never crowd it out; tool windows float over the game
  and fit small windows and phone screens.
- Turu Monitor is served by the app itself (`docs/WEB_MONITOR_PROPOSAL.md`).
- Not binding for future design work (the user did not commit to them): keeping VibeSkua's exact
  window layout and sizes, and keeping Mugetsu's current palette (dark panel, `#7C6CF0` accent).
  They describe the current implementation, not a requirement.

## Brand Commitments

- Name **Turu** and its meaning, "sleep" (let the bot play while you do).
- The existing logo and symbol in `docs/logo/` (light/dark logotype, symbol, macOS and mono
  variants) and the app icons derived from it.

## Evidence on Hand

- Logo files: `docs/logo/*.svg`; app icons in `app/assets/` and the Android app.
- README and docs describe features; releases are beta builds. Android runs on real
  phones (docs/ANDROID.md names a Redmi 10 and a Pixel 8; Sentry has crash reports from users'
  phones).
- No user counts, testimonials, benchmarks beyond the headless CPU/RAM note in
  `docs/STATUS.md`, or press. Do not invent any.

## Product Principles

1. The game comes first: bot UI supports it and gets out of its way.
2. Built for leaving it alone: unattended runs are the norm, so state must be readable at a
   glance when the user comes back.
3. Familiar to Skua users: same names, features and flow, so nothing has to be relearned.
4. At home on each platform: one product, adapted to the OS it runs on.
