# Proposal: Turu's own host SWF

Status: R0 done (`host/`, crate `turu_host`); R1 done (`turu.swf` starts the game, run only with
`TURU_HOST_SWF=turu`); R2 done (the generic calls, parity mode `TURU_HOST_PARITY=1`); R3a done
(auras, monsters, combat); R3b done (inventory, shop, player, world); R3c done (server
list, login); R3d done (options, modules, the "Use Turu's Host SWF" setting: `turu.swf`
answers every callback Turu calls); R4 done (Turu Mas runs `turu.swf`, checked on the emulator with
four tabs); R5 written (packets from the player), awaiting its checks in game; the rest are
proposals. Replaces the plan to fork VibeSkua's
`skua.swf` (formerly `docs/SKUA_SWF_PROPOSAL.md`). Touches a new `host/` (AS3 source and crate
`turu_host`), `bot/assets/`, `bot/src/flash.rs`, `events.rs`, `background.rs`, `perf.rs`,
`api/wait.rs`, `api/player.rs`, `api/options.rs`, `api/send.rs`, `butler/`, `air_patch/`,
`air/app/src/` (Turu Mas's host SWF), `air/ane/rust/src/` (`android.rs`'s `patched`,
`clients.rs`), `app/src/player.rs`. Reference: `docs/VIBESKUA_SPEC.md` (VibeSkua's Flash bridge:
the behaviour to match), `docs/EVENT_WAITS_PROPOSAL.md` (waits that wake on packets),
`docs/PACKET_BUTLER_PROPOSAL.md`, `docs/TURU_MAS.md` (the AIR patcher), `docs/STATUS.md` (the
measurements and workarounds cited below).

## Where things stand

Turu embeds VibeSkua's `skua.swf` unchanged (`bot/assets/skua.swf`, ~3,000 lines of AS3). It
hosts the game client and registers 86 `ExternalInterface` callbacks, of which Turu calls 58
through `bot/src/flash.rs`. Everything Turu needs beyond them is worked around in Rust or patched
at run time:

- **Layout.** skua sets `stage.align = TOP`, so a window taller than 958:550 puts all the empty
  space under the game. `app/src/player.rs` forces the stage centred.
- **Cutscenes.** Skip Cutscenes removes the cutscene but leaves `world.visible` off (a black
  screen), and removes it before the game sees it, so maps like /lichwar hold back their
  monsters. `api/options.rs` works around the first; the second is open (STATUS.md).
- **Log noise.** skua's quest-wiki module reads `game.ui.ModalStack` before login and throws
  every 100 ms (STATUS.md, Known gaps).
- **Packets twice.** Every received packet reaches Rust as `pext` (`JSON.stringify` of the
  packet) and again as `packetFromServer` (the debug line, prefixes stripped). Turu ignores the
  second (`events.rs`). Capture works by keeping `sfc.debug = true`, so SmartFox builds a debug
  string for every packet both ways, and skua turns it back on every frame.
- **Polling.** Every wait polls Flash (`wait::for_true`, 100 ms): ~200 calls a second for one
  farming tab, growing with tabs x waits on one thread. EVENT_WAITS_PROPOSAL wakes waits on
  server packets, but some conditions have no packet at all: the map SWF finished loading, the
  loading screen closed, the player arrived after walking, a cutscene or dialog appeared.
- **One value per call.** Each `getGameObject` walks a dotted path from `game`; hot paths read
  3 to 6 values in a row, one round trip each. Item checks take two calls
  (`selectArrayObjects` for one column, then `getArrayObject` for the match).
- **Work the game asks of the player.** The display-list hit test (`run_mouse_pick`) was 13% of
  the player's time at 30 FPS, and `gotoAndStop` ~25% of a frame (STATUS.md). ruffle-aqw's
  dormant marks skip clips that are stopped, but the game keeps many decorative clips playing,
  and Lag Killer's `world.visible = false` hides clips while they still run their frames.
  Filters and full quality cost render passes on phones and in Grid View.
- **Turu Mas (AIR).** skua.swf is rewritten at run time for AIR (see "Turu Mas today").
- **Unused and risky.** Turu never calls `injectScript` (loads and runs any SWF from a URL) or
  the `lnk*`/`fc*` remote registry, whose objects live until `lnkDestroy`.
- **Errors.** A callback that throws answers as if it returned nothing, so Rust can't tell
  "null" from "threw". AS3 errors in the game client are never reported.
- **License.** VibeSkua publishes no license. Turu ships its compiled SWF; it can't build on its
  source.

### Turu Mas today

Turu Mas bundles the same `bot/assets/skua.swf` (`air/app/build.sh` copies it into the APK). The
host SWF (`TuruAirHost`) reads it once, then has the engine patch it for each tab
(`nativePatch` → `turu_air_patch`, `Target::Skua`, `Options { shared_stage, client }`) and loads
the result with `loadBytes`. On VibeSkua's SWF, the shared-stage patch makes
16 changes (6 swaps, 1 drop, 9 stage sites):

| What skua does | What the patch makes it | Why |
|---|---|---|
| `ExternalInterface.addCallback` / `call` | `TuruExternal<n>` | AIR on Android has no `ExternalInterface`. |
| `new Loader()` ×4 (the game, `injectScript`, two for the custom background) | `TuruLoader<n>` | A SWF loaded from the web lands in a remote sandbox that can't share code; `TuruLoader` fetches through Turu's disk cache, patches the client, and `loadBytes` it. |
| `Security.allowDomain('*')` | dropped | Throws in AIR's application sandbox. |
| `stage.removeChildAt(0)` when the game loads | `TuruStage.removeOwn(this)` | Child 0 of a shared stage is whichever tab is shown. |
| `stage.addChild(game)`, one `getChildAt(0)` | `TuruStage.stageAddChild` / `rootOf` | The game goes into its own tab's layer. |
| The game's and the quantity selector's key listeners | `TuruStage.keys` gate | Only the tab on screen takes keys. |
| `stage.mouseX` / `mouseY` ×4 (quantity selector) | the game's own | The host scales each game to fit AIR's view. |

What that costs, beyond the rules themselves:

- **The client number is baked into class names.** Each tab's skua is patched separately to
  call `TuruExternal0`…`3` and `TuruLoader0`…`3`, eight near-empty classes compiled into the
  host SWF. That is why Turu Mas has at most four tabs (`clients.rs`'s `MAX`), and why the patch
  cache holds four copies of skua (`skua0`…`skua3`).
- **The host guesses whose code is calling.** `TuruStage.clientOfDomain` finds a tab from the
  caller's `ApplicationDomain` by looking for the `Game` class and comparing it with each tab's
  game (or its loader while the game is still starting); skua never says which tab it is.
- **The host does skua's job for the game.** `TuruLoader` recognises the client by its URL
  (`/gamefiles/Game*.swf`) and copies `gameParams` (`sFilePath`) into the game's `params` on
  `init`, because skua only copies its parameters after `complete`, too late for the client.
- **skua sets the shared stage's `scaleMode` and `align`** for every tab at once; the host sets
  them back (`TuruAirHost.fit`).
- **Every call is an AIR status event.** A round trip through the extension is ~1.6 ms on screen
  and 2.7 ms median (27 ms p95) off screen, about 650 calls a second at most on screen
  (AIR_PROPOSAL, "bridge benchmark"). One farming tab polling ~200 calls a second uses about a
  third of that; four tabs would pass it. On Ruffle the hop is far cheaper.
- **Rules match instructions, not names**, so they survive VibeSkua moving code around, but a
  change in how skua reaches the stage or loads a SWF can silently miss a site
  (`Report::stage_missed`), and `lib.rs`'s hash invalidates the cache on every rule change.

The game client's rules (its `AirLoader`, `Loader`s, the shared-stage hooks for keys, sound and
popups in the client and every SWF it loads, and a parent for the client's own
`new ApplicationDomain()`s, `TuruStage.assetsParent`, so its assets and game menu find
`TuruStage`) are AQW's code: a host SWF of Turu's own can't remove those. The last one leans on
the host SWF: the parent it hands out is the domain of the tab's host SWF (`mains[client]`).

## Goal

- Turu owns its host SWF, `turu.swf`: written for Turu, GPL like the rest, built from source in
  this repo, reproducibly, in CI.
- **Same answers as VibeSkua.** Every callback Turu or its scripts call keeps its name,
  arguments and answer format, so `flash.rs`, scripts and the fake-host tests don't change.
  New behaviour comes under new names, asked for by Rust after a version check.
- One SWF for every host: desktop, Ruffle Android and Turu Mas, with no patching of its own.
- Fewer Flash calls and less latency: events and batched reads instead of polling.
- Less work in the player when nobody is looking.
- Each milestone measured with `perf.rs` (calls/s, game-thread ms/s) before and after, and
  switchable off without a rebuild where it changes behaviour.

Not in scope: the game client itself, `ActionDelay` and the server's pacing ("too soon"), and
replacing the bot's packet-based state (drops, quests) with SWF state.

## Approach: write it, not fork or patch it

Three ways to get a host SWF Turu can change:

1. **Patch VibeSkua's SWF at load**, as `turu_air_patch` does for AIR. Keeps VibeSkua's binary,
   but every change is bytecode surgery: fine for one-line fixes, impractical for new modules.
2. **Fork VibeSkua's AS3** (tried as "M0" and dropped). It builds (Ruffle's `asc.jar` compiles
   it), and upstream changes would merge. But VibeSkua has no license, so its source can't
   become Turu's; and most of this plan reshapes it anyway (errors in every callback, packets,
   the host interface, fixes, removals).
3. **Write Turu's own** (this proposal). Same names and answers, its own code, designed for both
   hosts from the start.

How it's written:

- **From the behaviour, not the code.** `turu.swf` is written from `docs/VIBESKUA_SPEC.md`,
  what `flash.rs` and its tests expect back, and the game client itself. VibeSkua's `.as` files
  are not translated. Where those sources leave a behaviour open, it is settled by watching
  VibeSkua's SWF answer (the parity mode, R2), not by reading its code. VibeSkua's commit log
  can be read to learn of game updates its fixes followed.
- **What it answers.** The 58 callbacks Turu calls, and every name turu-scripts passes to
  `bot.flash.call` (`availableMonsters`, `rejectExcept`, `walkTo`, `getArrayObject`,
  `buyItemByID`), with VibeSkua's names, arguments and formats, including the JSON the generic
  calls make of game objects. All five game-side modules stay: the three Turu's options toggle
  (Hide Players, Disable FX, Disable Collisions) and the two always on in the quest window (item
  drop rates, wiki links). Not carried over: the `lnk*`/`fc*` remote registry, `injectScript`,
  and the 13 other callbacks nothing calls; a call to a name `turu.swf` doesn't have answers an
  error ("not in Turu's host SWF") instead of an empty string.
- **Deliberate differences**, each listed in the parity comparator's allowlist with why: the
  game centred (no `align = TOP`), no quest-wiki error before login, no monster-click wiki link
  (`openWebsite`: Turu only logs it, and the game's own wiki buttons open the browser), a custom-background timer that gives up, the old game removed by reference,
  `packetFromServer` only on request, errors reported (`error`), "Turu" as the login screen's
  title (VibeSkua's says "AURAS!!!"), the Bank hotkey ignored while typing in a text field,
  `loadClient` taking the frame rate, and the header's frame rate at 24, the game's own (Turu
  sets 24 when the client loads anyway, `background.rs`; Turu Mas's stage is the host's 24).
  From R5, on the Ruffle builds, `sfc.debug` left as the game sets it (packets come from the
  player).
- **Build.** `host/as3` (package `turu.host`) is compiled by crate `turu_host` with Ruffle's
  `ruffle/tools/asc/asc.jar` against Ruffle's playerglobal (which it builds itself, so nothing
  depends on where `ruffle_core`'s build put it). Java is already a build dependency there; the
  AIR SDK's compilers aren't on desktop CI. `cargo run -p turu_host` writes
  `bot/assets/turu.swf`, committed so a Rust-only build needs no Java; CI's `host` job runs
  `-- --check`, which fails if it isn't what the source builds. asc's output is deterministic.
  What asc does differently from Flex, found compiling skua.swf (the dropped fork):
  - Each `-in` file compiles on its own and can't see the others, so every file goes into one
    unit, an `include` wrapper with each class after the class it extends (asc rejects a
    forward reference to a base class).
  - That unit is one script, so every class's static initializer runs at load in the wrapper's
    order (Flex runs each class's when first used): a static initializer must not use another
    class.
  - `-swf` writes no End tag and a `DoABC`, so `turu_host` keeps only the ABC and writes the
    SWF: version 43, zlib, `FileAttributes` (AS3, useNetwork), `ScriptLimits` 1000/60, a black
    background, a `FrameLabel`, `DoABC2`, `SymbolClass` for `turu.host.Main`, 958x550, 24 fps.
  - asc emits `callproperty; pop` where Flex has `callpropvoid`, and `findpropstrict x;
    getproperty x` for `getlex x`: patch rules written against Flex's output (`air_patch`'s for
    skua) miss them. `turu.swf` needs no such rules (R4).

### Moving over

Both SWFs are embedded until the switch. `TURU_HOST_SWF=turu|vibeskua` picks one; from R3, once
`turu.swf` answers every call, so does a setting, Options > Application > Host SWF (VibeSkua /
Turu). VibeSkua stays the default until parity passes (R3); Turu is then the default for one
release with VibeSkua still selectable, and VibeSkua's SWF is removed after that (R6).

### Parity

Two SWFs can't each host the same game, and recorded answers can't be replayed against a game
that has moved on. So parity compares both on the same game, as black boxes, in Rust:

- In parity mode (desktop, a debug setting), `turu.swf` is the root and loads VibeSkua's
  `skua.swf` as a child. VibeSkua's SWF loads and hosts the game as usual and stays
  authoritative: Rust uses its answers.
- `turu.swf` loads no game of its own. It finds VibeSkua's game on the stage (the stage child
  with an `sfc`) and registers its callbacks with a prefix (`turu.getGameObject`, ...), and its
  events too (`turu.pext`, ...). Ruffle keeps one callback table per player, so the prefix keeps
  the two apart.
- VibeSkua's SWF takes stage child 0 off the stage when the game loads, which is `turu.swf`'s
  root: its code keeps running, so it keeps its own references and listens on the stage.
- `Flash::call` calls both `name` and `turu.name`, answers with VibeSkua's, and logs and counts
  each difference not on the allowlist (`parity:` lines in the debug log, a count in the perf
  log). Events are compared per name, in order and payload.
- The bar to switch: zero unlisted mismatches over the standard farms (FarmerJoe DoAll at
  Icestorm Arena, GoldFarm, a quest chain), with the same kills and no new "too soon".

## Milestones

Each is a branch `host-rN` from the `host` feature branch, fast-forwarded into it.

### R0: The build

- This proposal; `host/` with crate `turu_host` and a stub `turu.host.Main`; `bot/assets/turu.swf`
  committed but not yet used; CI's `host` job (format, clippy, tests, `--check`).
- Done when: `cargo run -p turu_host -- --check` passes locally and in CI, and the SWF reads
  back in Adobe's `swfdump`.

### R1: The skeleton

- `turu.host.Main` starts as VibeSkua's does (spec §1): `Security.allowDomain('*')` (skipped
  where it throws), the `String.prototype.trim` patch, the game's version, then the client
  loaded into `turu.swf`'s domain. Its `params` (`turu.swf`'s own parameters, `vars`, `sURL`,
  `sBG`, `sTitle`, `isEU`, `loginURL`) are set on `init`, before the client reaches the stage.
  The game then takes `turu.swf`'s place on the stage, `SHOW_ALL`, centred.
- The `Host` seam (`addCallback`, `call`, `load`, `show`), with one implementation,
  `ExternalHost` (`ExternalInterface` and the player's own stage: desktop and Ruffle Android).
  Turu Mas's comes in R4, where it can be tested.
- Events: every one Rust handles (`requestLoadGame`, `pre-load`, `loaded`, `pext`, `packet`,
  `packetFromServer`, `connectionLost`, `userIntentionalAction`, `debug`), plus `error`. Logout
  and Change Server in the options menu (`mcOption`) and a server picked from the login list
  (`mcLogin.sl.iList`) are seen by one capture-phase click listener on the game, with no frame
  handler. The Bank hotkey (`litePreference.data.keys.Bank`).
- Callbacks: `loadClient(frameRate?)`, `setBackgroundValues`, `hostInfo`, `setStrictErrors`,
  `setPacketEcho`, `catchPackets`, and `throwForTest` (unlisted: debug builds of Turu only).
- `hostInfo()`: `{"name": "turu", "version": 1, "features": [...], "callbacks": [...]}`. Rust
  asks for it in `Work::LoadClient`, before `loadClient`, and keeps it per tab until the next
  `requestLoadGame`; VibeSkua's SWF answers nothing, and calls go through as before. With it,
  `Flash::call` refuses a name it doesn't list, without calling Flash
  (`BotError::NotInHostSwf`: "`name` is not in Turu's host SWF").
- Errors: every callback in `try/catch`. Its throws and the uncaught errors of `turu.swf` and
  of the client reach Rust as `error(message, stack)`, logged to the debug log, the same message
  at most once a minute with a count. No Sentry: the bot crate has none, the client's errors are
  AQW's, and stacks can carry player data. `setStrictErrors(true)` makes a throw answer
  `{"error": "..."}`; off until `flash.rs` handles it.
- Packets as VibeSkua's: read from SmartFox's debug messages, with `sfc.debug` kept on.
  SmartFox's client (`it.gotoandplay.smartfoxserver.SmartFoxClient`) is sealed and its socket
  private, so AS3 can't wrap its sends; R5 replaces this on the Ruffle builds. `pext` once per
  packet; `packetFromServer` only after `setPacketEcho(true)`; a new SmartFox client is followed.
- `TURU_HOST_SWF=turu` chooses it (`turu_bot::HostSwf`; `bot::paths` writes `turu.swf`). No
  setting yet: with only R1's callbacks, Turu's background loops would fill the log with
  "not in Turu's host SWF".
- Not carried over: the monster-click wiki link (`openWebsite`), dropped in R3b (allowlisted).
- Done when, by hand on desktop with `TURU_HOST_SWF=turu`: the client loads and you log in; the
  same hand-played sequence gives the same outgoing packet log and `pext` count as with
  VibeSkua's; `throwForTest` (and `throwForTest("later")`, uncaught) shows its stack in the
  debug log; `packetFromServer` no longer arrives.

### R2: The generic calls and parity mode

- The calls that reach into the game by path (`GameCalls`): `getGameObject`, `getGameObjectS`
  (statics, from the game's own domain), `getGameObjectKey`, `setGameObject`,
  `setGameObjectKey`, `getArrayObject`, `setArrayObject`, `callGameFunction`,
  `callGameFunction0`, `selectArrayObjects`, `isNull`, `isTrue`. Paths are split once and kept,
  the cache cleared past 1,000. A callback that throws answers what VibeSkua's does (nothing;
  `""` for `selectArrayObjects`).
- Parity mode, `TURU_HOST_PARITY=1` (desktop): Rust starts `turu.swf` with `turuParity=1` and
  `turuSkua` (VibeSkua's SWF's URL) in its query. `turu.swf` loads VibeSkua's onto the stage
  in its own domain, loads no game, finds VibeSkua's (the stage child with an `sfc`) and uses
  `turu.` names both ways.
- `Flash::call` asks both SWFs for each read Turu's lists (`turu.hostInfo`) in one request
  (`FlashHost::call_pair`: back to back, no frame between) and answers with VibeSkua's; writes
  go to VibeSkua's only. `pext`, `packet`, `connectionLost` and `userIntentionalAction` are
  matched per name, in order; one with no partner after 5 s counts as missing. Rust acts on
  VibeSkua's events only.
- Exact string equality; the allowlist with reasons is `bot/src/parity.rs` (`ALLOWED`; `pext`
  compares only `params`). Each unlisted difference is a `parity:` line in the debug log (once
  a minute per call and arguments, with a count); the perf log has counts per call; a script's
  end logs a summary.
- Done when: GoldFarm for 15 minutes and one quest chain in parity mode end with no unlisted
  mismatch and no `turu.swf` error. Before login, 190 reads compare equal (headless).

### R3: The API

The rest of the callbacks Turu and its scripts call (about 44) and the five modules, in four
parts, each a branch (`host-r3a` ...) fast-forwarded in after its own checks:

- **R3a**: auras (`Auras`: `GetPlayerAura`, `GetMonsterAuraByID`/`ByName`,
  `GetAuraSnapshots`, `GetAurasValue`, `HasAnyActiveAura`), monsters and combat (`Combat`:
  `availableMonsters`, `getMonsters`, `getTargetMonster`, `attackMonsterName`/`ID`,
  `attackPlayer`, `untargetSelf`, `canUseSkill`, `useSkill`, `infiniteRange`, `magnetize`).
  VibeSkua's 5 s aura-array rebuild isn't carried over (Turu drops expired snapshots itself).
- **R3b**: inventory, shop, player and world (`buyItemByID`/`ByName`,
  `getUnmetPurchaseRequirements`, the inventory checks, `isFavoriteItem`, `rejectExcept`,
  `gender`, `getAvatar`, `jumpCorrectRoom`, `walkTo`, `isLoggedIn`, `isKicked`,
  `getActiveModalMessage`, `sendClientPacket`). Parity showed VibeSkua's monster-click wiki link
  (`openWebsite("https://aqwwiki.wikidot.com/FrogZard")`); it's dropped and allowlisted, as Turu
  only logs `openWebsite`.
- **R3c**: server and login (`Servers`: `clickServer`, `connectToServer`, `killLoginModals`,
  `loadPolicyFile(s)`, `resetConnectionState`). `connectToServer` makes the stock server list's
  own checks and messages (the client's `ServerList.onServerClick`: offline, full, chat limits,
  member and testing servers, confirmed email) before `connectTo`; its dialogs' buttons do
  nothing, as the list's callbacks are private to it. All actions: checked by
  `host/checks/r3c-login.ts` (two relogins) on each SWF.
- **R3d**: options and modules (`Options`: `disableDeathAd`, `killLag`, `skipCutscenes`,
  `modEnable`/`modDisable` for Hide Players, Disable FX and Disable Collisions, re-applied every
  100 ms and restored when turned off), and Options > Application > "Use Turu's Host SWF" (off by
  default, read at launch; `TURU_HOST_SWF` overrides it). Not carried over: Lag Killer's custom
  background (Turu never sets one), and the quest window's two modules (`QuestItemRates`,
  `QuestRequirementWiki`), which Turu never toggles; their names do nothing. With R3d,
  `turu.swf` answers every callback Turu and its scripts call. Checked by
  `host/checks/r3d-options.ts` on each SWF, and FarmerJoe DoAll on `turu.swf` alone.

How each is checked:

- Reads join parity's compared list (`parity.rs`'s `READS`). Actions (attack, use a skill, buy,
  walk, connect, the modules) go to VibeSkua's SWF only in parity mode, or they would happen
  twice; they're checked by running `turu.swf` alone (`TURU_HOST_SWF=turu`). Real scripts need
  every part (CoreBots asks `isLoggedIn` first), so until R3d each part's actions run from a
  small script that calls only that part (`host/checks/`, e.g. `r3a-combat.ts`), compared
  packet for packet with VibeSkua's SWF; from R3d FarmerJoe DoAll at Icestorm Arena keeps its
  kills per minute with no new "too soon".
- Behaviour comes from the spec, then the C# that uses each answer, then `flash.rs`, then
  parity; where the spec is silent on an answer's shape, parity shows VibeSkua's and `turu.swf`
  matches it. VibeSkua's AS3 isn't read.
- Where VibeSkua's SWF throws (its quest-wiki module before login, `infiniteRange` without
  skills, Disable Collisions without a map), `turu.swf` guards instead; the differences that
  show go on the allowlist with their reason.
- Done when, for each part: a parity run over GoldFarm and a script that uses the part has no
  unlisted mismatch, and the `TURU_HOST_SWF=turu` checks above pass.

### R4: Turu Mas

- `TuruAirHost` compiles in `TuruHost`, static methods by tab (`client`): `addCallback`,
  `call` (to `TuruExternal`), `load(client, url):Loader` (a `TuruLoader` for the client: disk
  cache, the client patched for AIR, `loadBytes`), `showGame(client, main, game)` (`TuruStage`
  takes the host SWF off and the game on), `isShown(client)`.
- `turu.swf` uses `AirHost` when its domain has `TuruHost`, with its tab from
  `loaderInfo.parameters.turuClient`, which the host passes in the `LoaderContext`. `Host.load`
  became `load(url):Loader`, the host making the loader. Under AIR (`Host.ownsStage()` false)
  `turu.swf` leaves the shared stage's frame rate, scale and alignment to the host, and its Bank
  key works only while its tab is on screen (`isShown`).
- The host loads the same `turu.swf` bytes for every tab, unpatched, in a domain under its own
  (so the game menu's assets still find `TuruStage`), and `turu.swf` has its game loaded into a
  domain under its own (`AirHost.load`), as VibeSkua's SWF does: the game's domain then holds the
  tab's `turu.host::Main`, so `TuruStage` knows the tab while the game is still being built
  (without it, two `assetsParent` calls a start couldn't tell their tab). `TuruStage` needed no change: it finds a tab
  by its host SWF's class (`mains[client]`), whichever it is. VibeSkua's SWF keeps going through
  `Target::Skua`, `TuruExternal<n>` and `TuruLoader<n>` until R6.
- Which SWF: Options > Application's "Use Turu's Host SWF", read by the host from settings.json in
  the data folder the engine's start returns (no new native call). `build.sh` packages both.
- `clients.rs`'s `MAX` stays 4: the per-tab classes no longer set it, but a phone's memory and
  the extension's calls a second do, which M4 and M5 measure.
- Checked on the emulator (Android 16, four accounts): four tabs logged in, each its own game;
  switching showed each one's, with 264 keys held back from hidden tabs; the game menu opened;
  `TuruStage` placed every call ("couldn't tell their tab: none"); GoldFarm on one tab moved only
  that tab's maps and turned in 26 quests in 5.2 minutes (5.0 a minute, VibeSkua's desktop pace)
  with no "too soon".
- Done when: on a phone with the setting on, four tabs log in and change maps; switching tabs
  moves keys, sound and the quantity selector to the shown tab; the game menu opens in Battleon;
  `TuruStage`'s `fallbacks` and `hookSummary` show no call it couldn't place; GoldFarm on one tab
  keeps VibeSkua's pace on Turu Mas.

### R5: Packets from the player

On the Ruffle builds (desktop, Ruffle Android) the bot's packets come from ruffle-aqw, below the
game client, instead of from AS3. Turu controls that player, so it can watch the game's socket
without touching SmartFox.

- **Outgoing:** when a `flash.net.Socket` flushes, ruffle-aqw hands Rust the messages written
  (SmartFox ends each with a null byte): what the client sent, when it sent it.
- **Incoming:** ruffle-aqw hands Rust the messages a `socketData` event carried *after* that
  event's dispatch returns. SmartFox's handler runs synchronously (it parses the message, raises
  `onExtensionResponse`, and the game updates), so the client has applied the packet by then,
  which waits (EVENT_WAITS_PROPOSAL) need. That is the difference from `capture.rs`, which reads
  the network before the client does and stays the Interceptor's raw view.
- Built as `ruffle_core::socket::SocketObserver` (`PlayerBuilder::with_socket_observer`), told
  from `Socket.flush` and after each `socketData` dispatch, and when a socket closes; the
  frontend's `TuruSocketObserver` hands a tab's to its bot (`Bot::on_socket`,
  `bot/src/player_packets.rs`). Every AVM2 `flash.net.Socket` is watched (the game opens one,
  SmartFox's), each with its own buffer split on null bytes; `XMLSocket` and policy files aren't.
- Rust builds `packet` (outgoing `%xt%zm%...` strings, VibeSkua's filter) and `pext` (incoming
  extension responses) from them, as the client's SmartFox builds `onExtensionResponse`'s
  `params` (`ExtHandler.handleMessage`): a string message's fields after `xt`, a JSON one's
  `b.o`, with `type` `str` or `json`. XML and system messages aren't the bot's. They go to the
  same handlers as the SWF's events, so the bot, scripts and their events don't change.
- `turu.swf` then stops its own capture there (`setPacketCapture(false)`, asked by Rust after
  `hostInfo` when `FlashHost::player_packets()`): no `sfc.debug` kept on, so SmartFox stops
  building a debug string for every packet, and no `JSON.stringify` of every received packet.
  VibeSkua's SWF can't be asked; with it both run, and Rust drops its `packet` and `pext`.
- No client patching on desktop or Ruffle Android, and nothing in `game_files.rs` changes. AQW or
  SmartFox changing their code doesn't reach it: it reads the socket, not their methods.
- Hold and drop (M8): ruffle-aqw can hold a flush or drop a message. To the client it looks like
  a slow network, which it already copes with.
- Turu Mas: AIR owns its socket, so it keeps the debug messages. A hook in SmartFox's
  `writeToSocket`/`handleMessage` there (Turu Mas already patches the client, `turu_air_patch`) is
  a later, separate step, if M4/M5's measurements show Turu Mas's packet cost matters.
- Parity: in parity mode the player's `packet` and `pext` are compared with VibeSkua's. Two
  captures don't start together (VibeSkua's at login), so events pair by content, in either
  order, and one left alone for 5 s is reported with the side that saw it. Over GoldFarm and a
  relogin (about 50,000 compared) every unlisted one was the player's alone: about 5% of every
  kind, `packet` and `pext`, and none VibeSkua's alone. VibeSkua's capture misses them (sent
  packets come from SmartFox's debug messages, which the game's logger keeps turning off; for
  received ones it logs nothing), so packets only the player saw are allowlisted; one only
  VibeSkua's saw is still reported.
- Done when: over the standard farms the player's packets match the SWF's (parity), and `perf.rs`
  shows the packet cost down (the ms/s in SmartFox's debug messages and `JSON.stringify` gone), on
  desktop and on Ruffle Android.

### R6: The switch

- Turu's SWF becomes the default on every host; VibeSkua's stays selectable for one release,
  then goes, with `Target::Skua`, `Options::client`, the skua rules (`stg`/`removeChildAt`),
  `TuruExternal0`…`3` and `TuruLoader0`…`3`. The game client's rules and `shared_stage_swf` for
  the SWFs it loads stay.
- Docs: `docs/VIBESKUA_SPEC.md` (where Turu's SWF differs), `docs/ARCHITECTURE.md`,
  `docs/SCRIPTING.md` (calls that now error), STATUS.md, the README's license section.

### M4: Batched reads

- `getMany({key: path, ...})`: one JSON object with every value; a path that throws or is null
  gives `null` for that key.
- `selectArrayObjectsWhere(path, field, value)`: only the matching items, replacing the
  `selectArrayObjects` + `getArrayObject` pair in `inventory::matching_by`.
- `getSkillStates()`: every skill's cooldown left, cost, range and whether it's usable, for the
  skill loop (one call instead of `canUseSkill` per skill).
- `getSelfSnapshot()` / `getTargetSnapshot()`: HP, MP, state, cell, auras in one call.
- Rust: `flash.rs` gets `get_many`; the kill loop, skill rules, `api::player::shared` and item
  checks use them when `hostInfo` lists the feature.
- Turu Mas gains most: each round trip saved is ~1.6 ms of AIR's thread on screen and more off
  screen ("Turu Mas today"), against well under that on Ruffle.
- Done when: calls/s on a standard farm drop measurably with the same kills per minute and the
  same answers (a debug mode that does both and compares), on desktop and on Turu Mas (the
  extension's round trips per second, on and off screen).

### M5: Events and watched values

It completes EVENT_WAITS_PROPOSAL for the conditions packets can't report.

- `subscribe(names)` / `unsubscribe(names)`; the SWF then calls `event(name, json)` only on
  change:
  - `mapLoaded` (the map SWF finished; the game's own load completing), `loadingDone` (the
    loading screen closed), `cellChanged`;
  - `arrived` (walking finished at the asked position);
  - `cutscene` (a child added to `mcExtSWF`), `modal` (a dialog opened or closed);
  - `bankOpen`, `shopLoaded`, `questLoaded`;
  - `self` / `target` changes (HP, MP, state, target changed or died), `aura` added/removed,
    `inventory` changed.
- `watch(id, paths, minIntervalMs)` / `unwatch(id)`: the SWF reads the paths once per frame
  (cheap inside AS3) and sends only the ones that changed, no more often than asked.
  `api::player::shared` becomes a cache fed by it, never more than a frame old.
- Events go through `Bot::on_flash_call`, which must not block or call Flash: they update state
  and notify waits (`control.rs`), as packets do in EVENT_WAITS_PROPOSAL.
- Waits that wake on them: `forMapLoad`, `forBankLoad`, `forItemBuy`/`forItemSell`,
  `forPlayerPosition`, the loading-screen check in `background.rs`. Each wait keeps its polling
  path as the fallback and its VibeSkua timeout; an event only wakes it early.
- Cutscenes: with `cutscene`, Skip Cutscenes can wait until the game registered the overlay
  before removing it, fixing the /lichwar hold-back, and it restores `world.visible` unless Lag
  Killer is on.
- On Turu Mas, events arrive through `TuruHost.call` as status events like any other call from
  the SWF; there are far fewer of them than the polls they replace, and they cost AIR's thread
  nothing while waiting. With the screen off, AIR slows the app (27 ms p95 per round trip), so
  a poll there is both slow and late; an event is only late.
- Done when: calls/s idle and farming drop, wait latency (perf log) drops, and the warning
  count ("too soon") doesn't rise. Measured with one tab and with nine in Grid View, and on Turu
  Mas with four tabs, screen on and off.

### M7: Less work for the player

The SWF can't make Ruffle's own code faster (the interpreter, the display-list walks, the
renderer: those change in ruffle-aqw). It can cut the work the game asks of Ruffle. These change
what the game does, so each is a switch, off by default, measured in game.

Where things stand (STATUS.md): after ruffle-aqw's goto-settle and dormant marks, a frame's game
logic is ~1.3-2 ms, the player's ticks ~65 ms/s on a farm, the bot's calls 2-8 ms/s. On a desktop
with one tab the headroom is modest; it matters most on phones, Turu Mas, many tabs, Grid View,
and while the game is shown and drawing.

**Measure first** (a step of its own, before any switch). The GPU's time per frame is in the
perf log with `RUFFLE_GPU_TIMING=1` (STATUS.md, "GPU time per frame"):

- The perf log counts dormant against awake clips per frame, and of the awake ones, how many are
  awake only because they're playing (no script queued, no child changed). That number bounds
  the stop-animations lever.
- Frame time with filters on and off, and with `stage.quality` high and low, on a phone and on
  a desktop with a window (headless uses the null renderer, so drawing costs nothing there).
- Hit-test time per pick in a shown tab with the pointer over the game, as STATUS.md's
  0.35 ms (farm map) / ~1 ms (Yulgar).
- On Turu Mas, AIR's own renderer and VM do the work, not Ruffle and wgpu: the dormant marks
  and `RUFFLE_GPU_TIMING` don't apply there. Measure frame time with AIR's `Event.ENTER_FRAME`
  spacing (or `adb shell dumpsys gfxinfo`) for the drawing levers, with one tab and four.

**Levers**, most expected value first:

- **Feed the dormant marks.** `stopAnimations(true)`: stop decorative and idle clips (other
  players' idle loops, map decorations such as flags and torches, monsters not in combat),
  restoring them on `false`. A dormant clip is skipped by a frame's three walks only while
  stopped with nothing queued (Battleon 2.2 -> 1.3 ms, /fotia 4.9 -> 1.95 ms), so every clip
  stopped here is one ruffle-aqw's mark can skip. Beyond Lag Killer's `world.visible = false`,
  which hides clips but leaves them running their frames. On Turu Mas a stopped clip still
  saves AIR its frame scripts and timeline work, so this helps there too, without the marks.
  With several tabs it matters more on AIR than on Ruffle: hidden tabs aren't drawn, but every
  tab's game runs at the one stage's frame rate (TURU_MAS.md, "No hidden-tab slowdown"), so
  `stopAnimations(true)` on hidden tabs is AIR's stand-in for Ruffle's 2 FPS.
- **Smaller hit tests in a shown tab.** `setMouseEnabled(...)`: `mouseChildren = false` on
  layers nobody clicks (map art, other players while a bot runs), and on `game.world` entirely
  while a bot runs in a hidden tab or headless. Hidden tabs already skip picking
  (`give_mouse`); this shrinks the walk where it still happens.
- **Cheaper drawing** (a window or Android only):
  - `stripFilters(true)`: remove `filters` (glows, drop shadows on names and effects) from
    avatars and effects, each an extra render pass in wgpu; restore on `false`.
  - `setQuality(...)` for Grid View tiles (lower `stage.quality`), full quality on the shown
    tab.
  - `cacheAsBitmap` on a map's static background layers, so their vectors aren't drawn again
    each frame (check Ruffle's wgpu support and its memory cost on phones).
  - Hide Players that takes other avatars off the display list instead of hiding them, and puts
    them back on `false` (check the game re-adds them on cell changes).
- **Fewer event-loop wakes.** Not a switch: M4's batched reads and M5's events cut the calls,
  and each call wakes the app's event loop (190-250 ticks a second with a bot running, against
  30 frames). Counted here with the rest so the total is measured together.
- **Less packet work.** Also not a switch: R5's packets from the player (`sfc.debug` left alone,
  no `JSON.stringify` per packet) and `packetFromServer` only on request (R1).
- `setFrameRate(n)` inside the SWF, so the game's own FPS setting and Turu's (`background.rs`'s
  `stage.frameRate`) don't fight.

**What the SWF can't reach**, noted so it's not expected of this milestone:

- The game client's own code: its classes are sealed, so the host SWF can't replace their
  methods. The ~165 `gotoAndStop`s a second (half to the frame a clip is already on) could be
  skipped in the client's bytecode (a guard on `currentFrame`), patched as Turu Mas already
  patches the client (`turu_air_patch`): a separate proposal, for desktop as well as AIR.
- Rendering's fixed costs (tessellation, GPU passes, Metal/D3D presentation): Ruffle and wgpu.

Done when: each lever has its before/after on the STATUS.md benchmarks (FarmerJoe DoAll at
Icestorm Arena and a /fotia farm, 5 minutes each, release build; plus a phone run for the drawing
levers) with the same kills and no new "too soon" warnings, nothing visible breaks when a tab is
shown again or a switch is turned off, and only the levers that paid off are kept.

### M8: Packet hooks for Butler

- With R5's packets from the player (the Ruffle builds; Turu Mas would need the SmartFox hook
  first): `holdPackets(filter)` / `releasePackets()` and `dropPackets(filter)`,
  so Butler can keep the client from acting while it drives the character, and a skill can be
  held rather than sent too soon.
- `outgoing(packet)` events carry what the client sent, for PACKET_BUTLER_PROPOSAL's mirror.
- Done when: Butler's proposal uses them and the warning count doesn't rise.

## Risks

- **Different answers.** Scripts and behaviour are checked against VibeSkua's C#; a rewritten
  SWF can answer differently in ways the fake host doesn't show. Mitigation: parity mode with an
  explicit allowlist, the switch only at zero unlisted mismatches, VibeSkua selectable for a
  release after it.
- **Game updates.** VibeSkua follows AQW's client changes in its SWF; Turu's must follow them
  itself. Watch `git -C ../VibeSkua log -- Skua.AS3` for what changed in the game, and keep
  parity mode working so a regression shows up as a mismatch.
- **Independence.** The rewrite is written from the spec, Rust's expectations and the game, not
  from VibeSkua's files; its authors have read those files, so this is a reimplementation, not
  a clean room. If the license question matters beyond that, ask someone qualified.
- **Compiler.** `asc.jar` isn't Flex; its differences (above) are handled in `turu_host`, and
  the one-script rule (no static initializer using another class) has to hold as code grows.
- **Events in `on_flash_call`.** Many events per frame must stay cheap there (no Flash, no
  locks held long); M5 measures the player's time spent in it.
- **Game-side switches (M7, M8)** can break things only seen in game; they stay off by default
  until checked.
- **Two hosts to check.** Ruffle and AIR's AS3 VMs differ (sandbox rules,
  `LoaderContext.parameters` with `loadBytes`, timing of `init` and `complete`). Each milestone's
  "done when" includes a Turu Mas run from R4 on, and `TuruHost` stays small so the desktop path
  (no host class) stays the plain `ExternalInterface` one. A call that can't resolve under AIR
  fails silently (the client catches the error), as the game menu did when its domain couldn't
  see `TuruStage`, so that run includes opening the game menu (the treasure chest) in Battleon.

## Open questions

- Do scripts get M5's events too (`bot.events.on("mapLoaded", ...)` in `bot/js/turu.ts`), or
  only the engine's waits?
- What can CI check of `turu.swf` beyond the build, without a game login (a Ruffle harness with
  a stub client)?
