# Proposal: Turu's own skua.swf

Status: proposal, nothing built. Touches `bot/assets/skua.swf` (and a new AS3 source tree),
`bot/src/flash.rs`, `events.rs`, `background.rs`, `perf.rs`, `api/wait.rs`, `api/player.rs`,
`api/options.rs`, `api/send.rs`, `butler/`, `air_patch/`, `air/app/src/` (Turu Mas's host
SWF), `air/ane/rust/src/` (`android.rs`'s `patched`, `clients.rs`), `app/src/player.rs`. Reference:
VibeSkua `Skua.AS3/skua/src/skua/*.as` (the SWF as it is), `docs/EVENT_WAITS_PROPOSAL.md` (waits
that wake on packets), `docs/PACKET_BUTLER_PROPOSAL.md`, `docs/TURU_MAS.md` (the AIR patcher),
`docs/STATUS.md` (the measurements and workarounds cited below).

## Where things stand

Turu embeds VibeSkua's `skua.swf` unchanged (`bot/assets/skua.swf`, ~3,000 lines of AS3). It
hosts the game client and registers ~70 `ExternalInterface` callbacks that `bot/src/flash.rs`
calls. Everything Turu needs beyond them is worked around in Rust or patched at run time:

- **Layout.** `Main.as` sets `stage.align = TOP`, so a window taller than 958:550 puts all the
  empty space under the game. `app/src/player.rs` now forces the stage centred.
- **Cutscenes.** `World.skipCutscenes` removes the cutscene but leaves `world.visible` off (a
  black screen), and removes it before the game sees it, so maps like /lichwar hold back their
  monsters. `api/options.rs` works around the first; the second is open (STATUS.md).
- **Log noise.** `QuestRequirementWiki` reads `game.ui.ModalStack` before login and throws
  every 100 ms (STATUS.md, Known gaps).
- **Packets twice.** Every received packet reaches Rust as `pext` (`JSON.stringify` of the
  packet) and again as `packetFromServer` (the debug line, prefixes stripped). Turu ignores the
  second (`events.rs`). Capture works by keeping `sfc.debug = true`, so SmartFox builds a debug
  string for every packet both ways, and `maintainPacketCapture` turns it back on every frame.
- **Polling.** Every wait polls Flash (`wait::for_true`, 100 ms): ~200 calls a second for one
  farming tab, growing with tabs x waits on one thread. EVENT_WAITS_PROPOSAL wakes waits on
  server packets, but some conditions have no packet at all: the map SWF finished loading, the
  loading screen closed, the player arrived after walking, a cutscene or dialog appeared.
- **One value per call.** Each `getGameObject` splits a dotted path and walks it from `game`;
  hot paths read 3 to 6 values in a row, one round trip each. Item checks take two calls
  (`selectArrayObjects` for one column, then `getArrayObject` for the match).
- **Work the game asks of the player.** The display-list hit test (`run_mouse_pick`) was 13% of
  the player's time at 30 FPS, and `gotoAndStop` ~25% of a frame (STATUS.md). ruffle-aqw's
  dormant marks skip clips that are stopped, but the game keeps many decorative clips playing,
  and Lag Killer's `world.visible = false` hides clips while they still run their frames.
  Filters and full quality cost render passes on phones and in Grid View.
- **Turu Mas (AIR).** skua.swf is rewritten at run time for AIR (see "Turu Mas today" below).
- **Unused and risky.** Turu never calls `injectScript` (loads and runs any SWF from a URL),
  the `lnk*`/`fc*` remote registry, or relies on the per-frame login/server-list monitors after
  they've done their job. Remote-registry objects live until `lnkDestroy`.
- **Errors.** Most callbacks have no `try/catch`; a missing path throws inside
  `ExternalInterface`, and Rust can't tell "null" from "threw". AS3 errors in the game client
  are never reported.

### Turu Mas today

Turu Mas bundles the same `bot/assets/skua.swf` (`air/app/build.sh` copies it into the APK). The
host SWF (`TuruAirHost`) reads it once, then has the engine patch it for each tab
(`nativePatch` → `turu_air_patch`, `Target::Skua`, `Options { shared_stage, client }`) and loads
the result with `loadBytes`. On VibeSkua's SWF, the shared-stage patch makes
16 changes (6 swaps, 1 drop, 9 stage sites):

| What skua does | What the patch makes it | Why |
|---|---|---|
| `ExternalInterface.addCallback` / `call` (`Externalizer`) | `TuruExternal<n>` | AIR on Android has no `ExternalInterface`. |
| `new Loader()` ×4 (`loadGame`, `injectScript`, two in `initCustomBackground`) | `TuruLoader<n>` | A SWF loaded from the web lands in a remote sandbox that can't share code; `TuruLoader` fetches through Turu's disk cache, patches the client, and `loadBytes` it. |
| `Security.allowDomain('*')` | dropped | Throws in AIR's application sandbox. |
| `this.stg.removeChildAt(0)` in `onComplete` | `TuruStage.removeOwn(this)` | Child 0 of a shared stage is whichever tab is shown. |
| `this.stg.addChild(game)`, one `getChildAt(0)` | `TuruStage.stageAddChild` / `rootOf` | The game goes into its own tab's layer. |
| `game.stage.addEventListener(KEY_DOWN, key_StageGame)`, `QtySelectorMC`'s key listener | `TuruStage.keys` gate | Only the tab on screen takes keys. |
| `stage.mouseX` / `mouseY` ×4 (`QtySelectorMC`) | the game's own | The host scales each game to fit AIR's view. |

What that costs, beyond the rules themselves:

- **The client number is baked into class names.** Each tab's skua is patched separately to
  call `TuruExternal0`…`3` and `TuruLoader0`…`3`, eight near-empty classes compiled into the
  host SWF. That is why Turu Mas has at most four tabs (`clients.rs`'s `MAX`), and why the patch cache
  holds four copies of skua (`skua0`…`skua3`).
- **The host guesses whose code is calling.** `TuruStage.clientOfDomain` finds a tab from the
  caller's `ApplicationDomain` by looking for the `Game` class and comparing it with each tab's
  game (or its loader while the game is still starting); skua never says which tab it is.
- **The host does skua's job for the game.** `TuruLoader` recognises the client by its URL
  (`/gamefiles/Game*.swf`) and copies `gameParams` (`sFilePath`) into the game's `params` on
  `init`, because skua only copies its own `loaderInfo.parameters` after `complete`, too late for
  the client.
- **skua sets the shared stage's `scaleMode` and `align`** in `onComplete`, for every tab at once;
  the host sets them back (`TuruAirHost.fit`).
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
`TuruStage`) are AQW's code: an owned skua can't remove those. The last one leans on skua: the
parent it hands out is the domain of the tab's skua (`mains[client]`).

## Goal

- Turu owns its host SWF: built from source in this repo, reproducibly, in CI.
- **Same behaviour as VibeSkua by default.** Every existing callback keeps its name, arguments
  and answers, so `flash.rs`, scripts and the fake-host tests don't change until a milestone
  says so. New behaviour is opt-in, asked for by Rust after a version check.
- Fewer Flash calls and less latency: events and batched reads instead of polling.
- Less work in the player when nobody is looking.
- Each milestone measured with `perf.rs` (calls/s, game-thread ms/s) before and after, and
  switchable off without a rebuild where it changes behaviour.

Not in scope: the game client itself, `ActionDelay` and the server's pacing ("too soon"), and
replacing the bot's packet-based state (drops, quests) with SWF state.

## Approach: fork, or patch at load

Two ways to change the SWF:

1. **Fork the AS3 into this repo** (`skua/src`, built with `ruffle/tools/asc/asc.jar`, which the
   playerglobals build already uses; Java is already a build dependency there). Clean diffs,
   new classes are easy, reviewable. Costs: a Java step in the build (or a committed SWF checked
   by CI), and following VibeSkua's upstream changes by hand.
2. **Patch VibeSkua's SWF at load**, as `turu_air_patch` does for AIR. Keeps the original as
   the source, but every change is bytecode surgery; fine for one-line fixes, impractical for
   new modules.

**Recommendation: fork (1)**, keeping VibeSkua's files and history as the first commit so
upstream changes can be merged. Commit the built `skua.swf` to `bot/assets/` as today (so a
Rust-only build needs no Java), and have CI rebuild it and fail on a difference. `air_patch`
keeps working on the result until milestone 6 makes its skua rules unnecessary.

Turu Mas needs nothing extra: `air/app/build.sh` already copies `bot/assets/skua.swf`, so the
committed SWF ships in the APK too. The AIR SDK's `amxmlc` could compile skua, but desktop CI has
no AIR SDK, so skua stays on `asc.jar` and both builds use the one SWF.

## Milestones

Each milestone is one PR (or a few), shippable on its own, in this order.

### M0: Build from source, unchanged

- Import `Skua.AS3/skua/src` into `skua/` with VibeSkua's history (or as one commit naming the
  VibeSkua commit it came from).
- `skua/build.sh`: compile with `asc.jar` against the playerglobals; output
  `bot/assets/skua.swf`.
- CI job: rebuild and compare. The SWF won't be byte-identical to VibeSkua's (different
  compiler), so the check for this milestone is behavioural: `cargo test -p turu_bot`,
  `air_patch`'s `patches_the_real_skua` (the same 6 swaps and 1 drop; with the shared stage, the
  same 9 stage sites: `cargo run -p turu_air_patch --example patch -- --skua --shared-stage`),
  and a login-to-Battleon run on desktop and on Turu Mas with two tabs.
- Done when: the new SWF passes all of that and replaces the old one.

### M1: Version handshake and error reporting

- `skuaInfo()`: returns `{"version": n, "features": [...]}`. VibeSkua's SWF doesn't have it, so
  a failed call means "version 0". `flash.rs` stores the answer per client; every later
  milestone's Rust side checks its feature first, so an old SWF (a custom one, a test host)
  keeps working.
- Forward AS3 errors: listen to `loaderInfo.uncaughtErrorEvents` on skua and on the game
  client's `LoaderInfo`; send `error(message, stack)` to Rust, logged to the debug log and
  (rate-limited, deduplicated) to Sentry.
- Every callback wrapped in `try/catch`. Behind a flag (`setStrictErrors(true)`), a throw
  answers `{"error": "..."}` instead of an empty string, so `flash.rs` can return a real error.
  Off by default until callers handle it.
- Done when: `perf.rs` shows no change, and an error thrown on purpose in the client shows up in
  the debug log with its stack.

### M2: Fixes that don't change behaviour

- Remove `stage.align = TOP` (the game centres by default); then drop the forced align in
  `app/src/player.rs`.
- `QuestRequirementWiki`: return when `game.ui` is null (no more error every 100 ms).
- `onGameClick`: fix `"…/" + strMonName || "monsters"` (the fallback never applies; a missing
  name opens `…/null`).
- `initCustomBackground`: stop its 100 ms timer after a limit when `game.mcLogin` never
  appears.
- `onComplete`: remove the old game by reference, not `stage.removeChildAt(0)` (after a reload
  child 0 may not be it). This is also what Turu Mas's `removeOwn` rule does.
- `Externalizer.call`: today it passes `rest` as one array. Keep that (Rust unpacks it), but
  document it; changing it would touch every inbound handler for no gain.
- Done when: tests pass and STATUS.md's log-noise and alignment entries are gone.

### M3: Cheaper packets and frame handlers

- `packetFromServer`: off by default, on by `setPacketEcho(true)` (VibeSkua's C# uses it; Turu
  doesn't).
- Outgoing capture by wrapping `sfc.sendXtMessage` / `sfc.send` instead of reading debug lines,
  so `sfc.debug` can stay as the game set it. Keep the old path behind a flag until the
  wrapper is checked against every packet type (`xt`, `xml`, `str`, `json`) in game.
- `maintainPacketCapture`, `monitorLoginScreen`, `monitorServerList`: remove themselves once
  their job is done, or listen to the game's events instead of `ENTER_FRAME`.
- Cache split paths in `_getObjectS` (a small map from path string to its parts).
- Done when: `perf.rs` and the profiler show the game thread's packet cost down (measure a
  farm before and after: packets/s, ms/s in `onDebugMessage` and `JSON.stringify`), and the
  packet log is the same line for line.

### M4: Batched reads

- `getMany({key: path, ...})`: one JSON object with every value; a path that throws or is null
  gives `null` for that key.
- `selectArrayObjectsWhere(path, field, value)`: only the matching items, replacing the
  `selectArrayObjects` + `getArrayObject` pair in `inventory::matching_by`.
- `getSkillStates()`: every skill's cooldown left, cost, range and whether it's usable, for the
  skill loop (one call instead of `canUseSkill` per skill).
- `getSelfSnapshot()` / `getTargetSnapshot()`: HP, MP, state, cell, auras in one call.
- Rust: `flash.rs` gets `get_many`; the kill loop, skill rules, `api::player::shared` and item
  checks use them when the feature is there.
- Turu Mas gains most: each round trip saved is ~1.6 ms of AIR's thread on screen and more off
  screen ("Turu Mas today"), against well under that on Ruffle.
- Done when: calls/s on a standard farm (FarmerJoe DoAll, GoldFarm) drop measurably with the
  same kills per minute and the same answers (a debug mode that does both and compares), on
  desktop and on Turu Mas (the extension's round trips per second, on and off screen).

### M5: Events and watched values

The large one; it completes EVENT_WAITS_PROPOSAL for the conditions packets can't report.

- `subscribe(names)` / `unsubscribe(names)`; skua then calls `event(name, json)` only on change:
  - `mapLoaded` (the map SWF finished; the game's own load completing), `loadingDone` (the
    loading screen closed), `cellChanged`;
  - `arrived` (walking finished at the asked position);
  - `cutscene` (a child added to `mcExtSWF`), `modal` (a dialog opened or closed);
  - `bankOpen`, `shopLoaded`, `questLoaded`;
  - `self` / `target` changes (HP, MP, state, target changed or died), `aura` added/removed,
    `inventory` changed.
- `watch(id, paths, minIntervalMs)` / `unwatch(id)`: skua reads the paths once per frame (cheap
  inside AS3) and sends only the ones that changed, no more often than asked. `api::player::
  shared` becomes a cache fed by it, never more than a frame old.
- Events go through `Bot::on_flash_call`, which must not block or call Flash: they update state
  and notify waits (`control.rs`), as packets do in EVENT_WAITS_PROPOSAL.
- Waits that wake on them: `forMapLoad`, `forBankLoad`, `forItemBuy`/`forItemSell`,
  `forPlayerPosition`, the loading-screen check in `background.rs`. Each wait keeps its polling
  path as the fallback and its VibeSkua timeout; an event only wakes it early.
- Cutscenes: with `cutscene`, Skip Cutscenes can wait until the game registered the overlay
  before removing it, fixing the /lichwar hold-back; `skipCutscenes` also restores
  `world.visible` unless Lag Killer is on.
- On Turu Mas, events arrive through `TuruExternal.call` as status events like any other call
  from skua; there are far fewer of them than the polls they replace, and they cost AIR's
  thread nothing while waiting. With the screen off, AIR slows the app (27 ms p95 per round
  trip), so a poll there is both slow and late; an event is only late.
- Done when: calls/s idle and farming drop, wait latency (perf log) drops, and the warning
  count ("too soon") doesn't rise. Measured with one tab and with nine in Grid View, and on Turu
  Mas with four tabs, screen on and off.

### M6: One SWF for every host, Turu Mas included

skua finds out at start which host it runs under and talks to it directly, so the same SWF runs
unpatched on desktop, Android (Ruffle) and Turu Mas. It depends only on M0–M2, so it can come
straight after them if Turu Mas needs it first.

- **A host interface, found at run time.** skua looks for a `TuruHost` class in its own
  `ApplicationDomain` (`ApplicationDomain.currentDomain.hasDefinition`): the host SWF compiles
  it in, as it compiles `TuruExternal` today. With it, `Externalizer` calls
  `TuruHost.addCallback(client, ...)` and `TuruHost.call(client, ...)`; without it (desktop,
  Ruffle Android), `ExternalInterface` as now. No class name carries a client number.
- **The tab says who it is.** The host passes the client number in the `LoaderContext`'s
  `parameters` when it loads skua (`turuClient`); skua reads it from `loaderInfo.parameters`
  and passes it with every host call. Then it registers itself:
  `TuruHost.register(client, main, gameDomain)` once the game's domain is known.
  `TuruStage.clientOfDomain` gets an exact answer instead of matching `Game` classes, and its
  "couldn't tell its tab" counter should stay at zero by construction. That includes the client's
  asset domains (`TuruStage.assetsParent`): `register` records skua's domain as their parent, and
  skua must still load in a domain under the host's, or the game menu and assets lose `TuruStage`
  and the menu silently never opens.
- **Loading through the host.** `loadGame` and the custom-background loads call
  `TuruHost.load(client, url, kind)`, which does what `TuruLoader` does now (disk cache, patch
  the client or the asset, `loadBytes` with `allowCodeImport`), and gives the `Loader` back. skua
  copies its parameters and `sFilePath` into the game's `params` on `init` itself, on every
  host, so `TuruLoader`'s URL check and `gameParams` copy go. `injectScript` keeps a plain
  `Loader` (or goes in M9), and refuses under AIR.
- **The sandbox.** `Security.allowDomain('*')` only when `Security.sandboxType` isn't
  `Security.APPLICATION` (AIR), so it never throws there.
- **The stage.** skua swaps its game by reference (M2), and under a host it hands the game to
  `TuruHost.showGame(client, game)` instead of adding it to the stage, and leaves `scaleMode` and
  `align` to the host. Its key listener checks `TuruHost.isShown(client)`; `QtySelectorMC` reads
  the game's `mouseX`/`mouseY` instead of the stage's (correct on every host: the game is
  scaled on AIR and letterboxed on desktop).
- **The patcher.** With the bundled SWF reporting the feature (`skuaInfo`, M1), the host loads
  skua as is: no `nativePatch` for skua, no `skua0`…`skua3` in the cache. Then remove
  `Target::Skua`, `Options::client`, the `stg`/`removeChildAt` rules, `TuruExternal0`…`3` and
  `TuruLoader0`…`3`. skua is bundled in the APK (`app:/skua.swf`), never a user's file, so
  there is no old SWF to keep supporting once the APK ships the new one. The game client's rules
  and `shared_stage_swf` for the SWFs it loads stay.
- **More tabs.** With no per-client classes, `clients.rs`'s `MAX` of 4 is only a setting; what
  limits tabs on a phone becomes memory and the extension's calls a second (M4, M5).
- Done when: Turu Mas runs four tabs on the unpatched SWF (log in, change maps, switch tabs: keys,
  sound and the quantity selector go to the shown tab) with `Target::Skua` deleted; desktop and
  Ruffle Android are unchanged; `TuruStage`'s debug counters show no call it couldn't place.

### M7: Less work for the player

skua can't make Ruffle's own code faster (the interpreter, the display-list walks, the
renderer: those change in ruffle-aqw). It can cut the work the game asks of Ruffle. These
change what the game does, so each is a switch, off by default, measured in game.

Where things stand (STATUS.md): after ruffle-aqw's goto-settle and dormant marks, a frame's game
logic is ~1.3-2 ms, the player's ticks ~65 ms/s on a farm, the bot's calls 2-8 ms/s. On a desktop
with one tab the headroom is modest; it matters most on phones, Turu Mas, many tabs, Grid View,
and while the game is shown and drawing.

**Measure first** (a step of its own, before any switch). The GPU's time per frame is in the
perf log now with `RUFFLE_GPU_TIMING=1` (STATUS.md, "GPU time per frame"):

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
  - Hide Players that takes other avatars off the display list instead of `hideMC()`, and puts
    them back on `false` (check the game re-adds them on cell changes).
- **Fewer event-loop wakes.** Not a switch: M4's batched reads and M5's events cut the calls,
  and each call wakes the app's event loop (190-250 ticks a second with a bot running, against
  30 frames). Counted here with the rest so the total is measured together.
- **Less packet work.** Also not a switch: M3's `sfc.debug` off and no `packetFromServer`.
- `setFrameRate(n)` inside skua, so the game's own FPS setting and Turu's (`background.rs`'s
  `stage.frameRate`) don't fight.

**What skua can't reach**, noted so it's not expected of this milestone:

- The game client's own code: its classes are sealed, so skua can't replace their methods. The
  ~165 `gotoAndStop`s a second (half to the frame a clip is already on) could be skipped in the
  client's bytecode (a guard on `currentFrame`), which `turu_air_patch` already knows how to
  rewrite: a separate proposal, for desktop as well as AIR.
- Rendering's fixed costs (tessellation, GPU passes, Metal/D3D presentation): Ruffle and wgpu.

Done when: each lever has its before/after on the STATUS.md benchmarks (FarmerJoe DoAll at
Icestorm Arena and a /fotia farm, 5 minutes each, release build; plus a phone run for the drawing
levers) with the same kills and no new "too soon" warnings, nothing visible breaks when a tab is
shown again or a switch is turned off, and only the levers that paid off are kept.

### M8: Packet hooks for Butler

- With M3's wrapper: `holdPackets(filter)` / `releasePackets()` and `dropPackets(filter)`, so
  Butler can keep the client from acting while it drives the character, and a skill can be held
  rather than sent too soon.
- `outgoing(packet)` events carry what the client sent, for PACKET_BUTLER_PROPOSAL's mirror.
- Done when: Butler's proposal uses them and the warning count doesn't rise.

### M9: Cleanup

- Remove `injectScript` (or keep it behind a setting nobody turns on by accident).
- Remove the `lnk*`/`fc*` remote registry if nothing uses it by then; otherwise cap it or free
  objects by age.
- Update `docs/VIBESKUA_SPEC.md` (where Turu's SWF now differs), `docs/ARCHITECTURE.md` (the
  SWF is built here), `docs/SCRIPTING.md` if scripts gain anything, and STATUS.md.

## Risks

- **Drifting from VibeSkua.** Scripts and behaviour are checked against VibeSkua's C#; a
  different SWF can answer differently. Mitigation: existing callbacks unchanged, new ones
  opt-in, the fake host in `turu_bot`'s tests updated with each feature.
- **Upstream changes.** VibeSkua keeps changing `skua.swf`. With the history imported, its
  changes merge like any other; check `git -C ../VibeSkua log -- Skua.AS3` when updating.
- **Compiler differences.** `asc.jar` isn't Adobe's Flex compiler; M0 exists to catch that
  before anything else changes.
- **Events in `on_flash_call`.** Many events per frame must stay cheap there (no Flash, no
  locks held long); M5 measures the player's time spent in it.
- **Game-side switches (M7, M8)** can break things only seen in game; they stay off by default
  until checked.
- **Two hosts to check.** Every milestone runs under Ruffle and under AIR, whose AS3 VMs differ
  (sandbox rules, `LoaderContext.parameters` with `loadBytes`, timing of `init` and `complete`).
  Each milestone's "done when" includes a Turu Mas run, and `TuruHost` stays small so the
  desktop path (no host class) stays the plain `ExternalInterface` one. A rewritten call that
  can't resolve fails silently (no `Report::stage_missed`; the client catches the error), as the
  game menu did when its domain couldn't see `TuruStage`, so that run includes opening the game
  menu (the treasure chest) in Battleon.

## Open questions

- Fork with VibeSkua's history, or one squashed import?
- Should the built SWF stay committed, or be built by `build.rs` when Java is present?
- Do scripts get the events too (`bot.events.on("mapLoaded", ...)` in `bot/js/turu.ts`), or only
  the engine's waits?
- Does anything (custom scripts, `injectScript` users) depend on the remote registry?
- Should M6 come right after M2, ahead of the call savings, because it lifts Turu Mas's
  four-tab cap and removes the skua rules from `air_patch`?
