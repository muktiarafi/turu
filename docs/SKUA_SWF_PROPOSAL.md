# Proposal: Turu's own skua.swf

Status: proposal, nothing built. Touches `bot/assets/skua.swf` (and a new AS3 source tree),
`bot/src/flash.rs`, `events.rs`, `background.rs`, `perf.rs`, `api/wait.rs`, `api/player.rs`,
`api/options.rs`, `api/send.rs`, `butler/`, `air_patch/`, `app/src/player.rs`. Reference:
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
- **Several clients on one stage.** Turu Mas rewrites skua's bytecode at run time
  (`turu_air_patch`: `ExternalInterface` to `TuruExternal<n>`, `stage.getChildAt(0)` to
  `TuruStage.rootOf`, `stage.removeChildAt(0)` to `TuruStage.removeOwn`).
- **Unused and risky.** Turu never calls `injectScript` (loads and runs any SWF from a URL),
  the `lnk*`/`fc*` remote registry, or relies on the per-frame login/server-list monitors after
  they've done their job. Remote-registry objects live until `lnkDestroy`.
- **Errors.** Most callbacks have no `try/catch`; a missing path throws inside
  `ExternalInterface`, and Rust can't tell "null" from "threw". AS3 errors in the game client
  are never reported.

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

## Milestones

Each milestone is one PR (or a few), shippable on its own, in this order.

### M0: Build from source, unchanged

- Import `Skua.AS3/skua/src` into `skua/` with VibeSkua's history (or as one commit naming the
  VibeSkua commit it came from).
- `skua/build.sh`: compile with `asc.jar` against the playerglobals; output
  `bot/assets/skua.swf`.
- CI job: rebuild and compare. The SWF won't be byte-identical to VibeSkua's (different
  compiler), so the check for this milestone is behavioural: `cargo test -p turu_bot`,
  `air_patch`'s `patches_the_real_skua`, and a login-to-Battleon run on desktop and Turu Mas.
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
- Done when: calls/s on a standard farm (FarmerJoe DoAll, GoldFarm) drop measurably with the
  same kills per minute and the same answers (a debug mode that does both and compares).

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
- Done when: calls/s idle and farming drop, wait latency (perf log) drops, and the warning
  count ("too soon") doesn't rise. Measured with one tab and with nine in Grid View.

### M6: Several clients built in

- `loadClient(clientId)` (or a flash var): skua namespaces its callbacks and calls by the id and
  finds its game by reference, never `stage.getChildAt(0)`.
- Turu Mas then needs no skua rules in `air_patch` (the game client's rules stay). Keep them for
  an old SWF (version 0) until the bundled one is the only one shipped.
- Done when: Turu Mas runs several clients on the new SWF with `air_patch`'s skua target
  disabled.

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

**Levers**, most expected value first:

- **Feed the dormant marks.** `stopAnimations(true)`: stop decorative and idle clips (other
  players' idle loops, map decorations such as flags and torches, monsters not in combat),
  restoring them on `false`. A dormant clip is skipped by a frame's three walks only while
  stopped with nothing queued (Battleon 2.2 -> 1.3 ms, /fotia 4.9 -> 1.95 ms), so every clip
  stopped here is one ruffle-aqw's mark can skip. Beyond Lag Killer's `world.visible = false`,
  which hides clips but leaves them running their frames.
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

## Open questions

- Fork with VibeSkua's history, or one squashed import?
- Should the built SWF stay committed, or be built by `build.rs` when Java is present?
- Do scripts get the events too (`bot.events.on("mapLoaded", ...)` in `bot/js/turu.ts`), or only
  the engine's waits?
- Does anything (custom scripts, `injectScript` users) depend on the remote registry?
