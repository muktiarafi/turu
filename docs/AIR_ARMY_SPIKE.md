# Spike: several game clients in Turu Mas

Can Turu Mas (docs/TURU_MAS.md) run more than one game client, so that Army and Butler (hidden
there by `AirPlatform::has_army`) can work? AIR on Android runs one app, one root SWF, one stage
(no `NativeWindow`), so the clients would have to share it. This spike loads two clients on that
stage. Started 2026-11-12, on the arm64 emulator.

## The problem: everything finds the game at stage child 0

AQW's client and nearly every SWF it loads find the game as `MovieClip(stage.getChildAt(0))`.
Counted with a scan of the bytecode (`stage`, then `getChildAt(0)`):

| SWF | Sites |
|---|---|
| `Game4000r8.swf` | 125 in 111 methods (popups, AvatarMC, MonsterMC, menus, ...) |
| a map (`town-battleon`) | 91 |
| the interface assets (`Assets_20261016r1.swf`) | 47 |
| a class (`EvolvedWorshipperOfNulgath`) | 37 |
| hair, helms, capes, weapons | 2-11 each |
| monsters, pets, grounds | 0 |
| skua.swf | 1 (`ModalMC`), plus `stg.removeChildAt(0)` (removing itself) |

On a shared stage each of those would find whichever game is at child 0, so the rewrite has to
cover every SWF as it's downloaded, not only the client.

## What the spike does

* **`turu_air_patch::Options::shared_stage`** (`air_patch/src/lib.rs`): `x.stage.getChildAt(0)`
  becomes `TuruStage.rootOf(x)`, and `stage` through the scope (`getlex`) becomes
  `findpropstrict stage` (the object that has it) and the same. `rootOf` walks `x`'s parents to the
  stage's child: the game `x` belongs to. In skua, `this.stg.removeChildAt(0)` becomes
  `TuruStage.removeOwn(this)`. A site that a branch lands inside is left alone and counted
  (`Report::stage_missed`). Tests: `shared_stage_*` (`TURU_STAGE_SWFS=<dir>` runs it on real SWFs).
* **The host** (`air/app/src/TuruAirHost.as`, `TuruStage.as`, `TuruExternalMute.as`): `CLIENTS = 2`.
  Client 1 is the engine's, as today. Client 2 loads once client 1's game is on the stage. Its skua's
  `ExternalInterface` is `TuruExternalMute`, which answers `requestLoadGame` with `loadClient`
  itself. A "Client 1/2" button at the top switches the client on screen: `visible`, and the
  active game moves to stage child 0, so anything still asking the stage gets the visible one.
  Every 30 s the Debug log says how many `rootOf` calls couldn't tell whose game they meant.
* **The engine** (`air/ane/rust/src/android.rs`): patch targets take bits 0x10 (shared stage) and 0x20
  (mute). Once the host asks for a shared stage, every SWF downloaded for the game is rewritten on
  the download's thread, not AIR's.

## Results so far

* Every site is rewritten: Game 124 of 125 (the one left is in Flash's `UIComponent`, its result
  unused), skua 1 + 1, and every cached map, class and item, with 0 missed and 0 undecodable.
  Patching again changes nothing.
* Cost on the emulator: Game ~0.45 s, once per download, on the download thread (then the usual
  0.06 s patch). Generic2 7 ms. On a Mac in release: the 8 MB interface assets 0.44 s, a map 36 ms,
  most items under 1 ms.
* Two clients start: two skuas, two games on the stage, each with its own working login
  screen, switching works, **0 `rootOf` fallbacks**.
* Memory at the login screen with both: 189 MB PSS (emulator).
* Client 2's title has no background: the engine sends `setBackgroundValues` only to its own skua.
  That comes from muting it, not from the shared stage.

## In game (2026-11-12, emulator, two accounts)

* Both accounts logged in at once with the game's own screens and stayed in: client 1 in its
  house, client 2 in its house and then battleon-2 (7 players: NPCs, quest markers, avatars, gear).
* 61 downloaded SWFs (maps, classes, armour, pets, the interface assets) rewritten, 0 missed;
  **0 `rootOf` fallbacks** and no `UNCAUGHT` error over the whole session, switching back and forth.
* Client 1's inventory opened on client 1 and didn't show through on client 2.
* The engine follows client 1 only: its log has client 1's account's packets and none of client
  2's. With client 2 on screen, Jump listed client 1's cells and moved it to r1 (seen after
  switching back).
* Memory: **392 MB PSS** with both in game (189 MB at the login screens), against ~250-275 MB for
  one client in battleon on a phone: about 1.4x, not 2x (AIR's runtime and the shared assets are
  paid once).
* Patching downloads costs nothing visible: the interface assets ~0.5 s and a map ~45 ms on the
  emulator, on the download threads.

## Verdict

**Go.** The one thing that looked like a blocker, the game finding itself at stage child 0, is
handled by a bytecode rule that covered every site in every SWF seen, with no fallbacks in play.
What's left is ordinary engineering (below), not unknowns. Not covered: a real phone (the 4 GB
ones, where two clients may not fit), hours of play, and a script running on client 1 while
client 2 also plays.

## Built (2026-11-12)

Tabs with a bot each, Army and Butler, as described in docs/TURU_MAS.md ("Tabs"); the spike's
second muted client and its switch button are gone. Checked on the emulator at the login
screen: New Tab, switching, closing (the client unloaded), a tab opened again on the freed
client number, and the second tab reopened from `android-tabs.json` after a restart. In game,
with two accounts: each tab's bot follows its own account (the log tags each line with its
account's name, as the tab names show), the game fits the view below the tab strip (`fit`:
0.903x), taps land (the login field, the inventory), Butler on one tab lists the other's account
and follows it (same room found from its state file), and Army Control lists both tabs.

## To make it Army and Butler (the plan)

1. **A bot per client.** Client IDs through the bridge: `AirFlashHost` (one per client),
   the status events and `reply`, `TuruExternal`'s callbacks and `TuruLoader` per client (skua
   patched to a per-client external, or a client ID passed in), and the engine running one bot
   per client as `turu_frontend`'s tabs do. `TuruExternalMute` goes.
2. **Tabs on AIR.** `AirPlatform`: `has_army` true, `limits_tabs` true (Tab Limit 2 by default),
   no Grid View. New Tab and closing a tab load and unload a client (`unloadAndStop`, and drop it
   from `TuruStage.games`). The tab strip replaces the spike's button.
3. **The gaps below**, then Butler needs nothing of its own: its sync files already work between
   clients sharing a data folder.

Rough size: 1-2 weeks, plus a phone run.

## Known gaps (not in the spike)

* **One engine, one bot.** Client 2 has no `Bot`. Army and Butler need a bot per client:
  `AirFlashHost` and the extension's calls need a client ID, `TuruExternal` its callbacks per client,
  and the engine one bot per client (as tabs in `turu_frontend`).
* ~~**`stage.addChild`**, **stage keyboard listeners**, **sound**~~: done 2026-11-12, by each
  calling code's `ApplicationDomain` (docs/TURU_MAS.md, "Tabs": "Only the tab on screen hears
  and is heard").
* **Only two clients.** `TuruExternalMute`'s callbacks are static, so more muted clients would
  share them; with a bot per client they'd go anyway.
