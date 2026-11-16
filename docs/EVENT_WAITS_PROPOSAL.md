# Proposal: waits that wake on game events

Status: partly built, as M5a of `docs/HOST_SWF_PROPOSAL.md` (`120e4943a`; `kicked` added in
`13da2f73a`), woken by `turu.swf`'s events (`world`, `mapLoaded`, `cellChanged`, `arrived`,
`loading`, `kicked`) rather than by the packet topics below. Built: phase 1's `for_signal`
(`forItemBuy`, `forItemSell` and `forBankLoad` sleep until their packet); phase 2's pulse (`Topics`,
`fire`, `mark`, `sleep_until` in `control.rs`, with no Options switch); phase 3 for `forMapLoad`,
`forCellChange` and the background checks, and `forPlayerPosition` too (the table below left it
polling) (`wait::for_true_on`); a `waits:` line in the perf log. Not built: the computed
`forActionCooldown`, Butler's class name on events, the other phase 3 waits, phases 4 and 5. The
rest of this document is the proposal as written, when every wait polled. Touches
`bot/src/control.rs`, `api/wait.rs`, `api/packets.rs`, `events.rs`, `api/player.rs`,
`background.rs`, `api/kill.rs`, `api/hunt.rs`, `perf.rs`, `butler/mod.rs` and `script/runtime.rs`.
Reference: VibeSkua `Skua.Core/Scripts/ScriptWait.cs` and `ScriptInterface.cs` (packet cases),
`Skua.AS3/skua/src/skua/Main.as` (`onExtensionResponse`), and `docs/HEADLESS_MONITORING_HANDOFF.md`
(measurements, "too soon").

## Where things stand

Every wait in the engine polls the game, as VibeSkua's `ForTrue` does: `wait::for_true`
evaluates a predicate, sleeps 100 ms (20 ms for `forMonsterDeath`, 50 ms for the quest waits)
and tries again. Most predicates start with `player::playing`.

Much of the cost of that has already been cut, without changing what the waits do:

- `isLoggedIn` and the avatar's `intState` (behind `playing`, `alive`, `inCombat`) are read at
  most every 100 ms per client and shared by every thread (`api::player::shared`).
- The background timer checks "still in game?" every 100 ms and the loading screen every 250 ms
  while in game (VibeSkua: 20 ms and 100 ms). Those checks were about half of all calls.
- `forDrop` / `forPickup` check the packet-built drop stack before `playing`. `player::loaded`
  reads `items.length`, not the inventory.
- `perf.rs` counts every Flash call by thread and callback, with the caller's wait and the
  player's run time (`GET /debug/perf`, `TURU_PERF_LOG`).

Measured after those fixes (release, headless, one farming client): 1-4% of a core, the game
thread busy 8-30 ms a second, of which bot calls are 2-8 ms; about 190-210 calls a second.

So **for one tab on a desktop, CPU is no longer the reason to do this.** What polling still
costs:

1. **Latency.** A wait notices its condition up to one poll late (100 ms). With the shared
   reads it can be up to 100 ms more, because the value it reads may be that old. A
   `kill.monster` -> `forMonsterDeath` -> next action chain pays this on every kill.
2. **Scale.** The calls grow with tabs x waits, all on one thread: nine tabs, Grid View
   (tiles at 8 FPS, `grid::TILE_FPS`), slower Android phones, and the AIR host, where
   `AIR_PROPOSAL.md` expects an ANE round trip to cost more than Ruffle's event-loop hop. A
   wait that sleeps until a packet arrives costs nothing while nothing happens.
3. **Waits that aren't events.** `forItemBuy` / `forItemSell` / `forBankLoad` wake every 20 ms
   to look at a flag (`for_signal`); in VibeSkua they are `AutoResetEvent.WaitOne`.
4. **Handlers inside engine waits** (`STATUS.md`, Known gaps): `bot.handlers` don't run during
   one long Rust-side wait (`forMapLoad`, the `kill.monster` death wait, ...).
5. **Re-reads that could follow events.** Butler's broadcast reads the whole inventory every
   2 s for the class name: ~25 calls a second, 15% of all, with one account
   (`HEADLESS_MONITORING_HANDOFF.md` §6).

### What it won't buy

- **Farming speed is set mostly by pacing, not by waits.** A farm loop spends most of its time
  in `ActionDelay` (800 ms), the skill timer and the server's own limit (~1 s between skills,
  "Please slow down. Last action was too soon!"). None of that changes here, so a loop gets
  faster by at most the poll latency of its waits: likely a few percent. Phase 0 measures it
  before anything is built on the promise.
- **Scripts poll too.** `bot.wait.forTrue` in `turu.ts` and CoreBots' loops
  (`Core.huntMonster`'s `while (!checkInventory)`, ...) poll through `__turu`, each check one
  or more Flash calls. Engine waits that wake on packets don't touch those. Phase 0 measures
  how many calls they make; if they are a large share, the script-side API (open questions) is
  worth more than the later engine phases.

## Goal

- Waits return when the packet that satisfies them arrives, and poll rarely while nothing
  happens.
- **Same answers as VibeSkua.** Up to phase 3 a packet only *wakes* a wait early: the wait
  still decides with VibeSkua's predicate, read from the game.
- **Same timeouts under load.** A wait that times out lasts at least as long as today's, also
  when Flash calls are slow (see "The timeout keeps its length").
- **No faster action pacing.** The server already answers "Please slow down. Last action was
  too soon!" to skills less than ~1 s apart and to repeated quest loads (handoff §3, §4).
  `ActionDelay`, the other fixed sleeps and the skill timer stay as they are, and the
  server's `warning` count (now logged through `Bot::note`) is a pass/fail measure for each
  phase.
- **Cheap, answer-preserving wins first** (phase 1), before the per-wait work they don't need.
- Measured before and after with `perf.rs`, and switchable off in game without a rebuild.

Not in scope: `ActionDelay` and other fixed sleeps, the skill timer's cadence, and waits on
things no packet reports (walking: `forPlayerPosition`; a map's SWF finishing loading).

## Design

### Packets wake, Flash confirms

`skua.swf` forwards every server extension response through `onExtensionResponse` ->
`ExternalInterface.call('pext', ...)` (`Main.as`) from the moment the game loads. Turu receives
it in `Bot::on_flash_call` while the player is locked, inside the game's socket event.

A woken thread checks its predicate with a Flash call. That call queues on the event loop and
runs after the current event, so after the game's own handler for the packet. The read
therefore sees the game *after* the packet. That holds for what the game applies in the same
event (`ct`, `dropItem`, `acceptQuest`, ...), not for what it defers (the map SWF after
`moveToArea`): those waits keep polling for the deferred part.

The wake comes from `pext`, not from the Interceptor's socket capture (`capture.rs`). The
capture sees bytes before the game has handled them, so a read woken from there could see the
old state.

The rule up to phase 3: **a packet is a hint, never an answer.** A spurious wake costs one
check. A packet that never arrives costs only time, because the fallback poll still finds the
result.

### Phase 1: wins that need no topics

Three changes that keep every answer and need neither the topic table nor the new `for_true`:

- **`for_signal` sleeps until signalled.** `ScriptControl` gets one generation counter under
  its existing mutex, bumped (with `notify_all`) by `packets.rs` right after it sets a
  `state.signals` flag. `for_signal` reads the generation, then the flag, then sleeps until the
  generation moves, the deadline, or Stop/Pause. Reading the generation first means a signal
  between the flag check and the sleep isn't lost. `forItemBuy` / `forItemSell` /
  `forBankLoad` become real event waits, as in VibeSkua. This counter is the pulse below with a
  single topic; phase 2 generalises it.
- **`forActionCooldown` computes its wait.** Read `world.lock.<action>` once, sleep until
  `ts + cd` (or the timeout), then confirm with `is_action_available`, retrying the 100 ms poll
  only if the confirmation fails (the game reset the lock meanwhile). 1-2 calls instead of up
  to 40, same answer and same timeout.
- **Butler reads the class name on events.** Read it at login and after equip/unequip replies
  (new `equipItem` / `unequipItem` cases in `packets.rs`, which has none yet, set a "class may
  have changed" flag the broadcast checks; phase 0's equip packet log names their fields), not from the whole inventory every 2 s. The heartbeat file write stays on
  its timer. Expected: ~25 calls a second fewer per account.

### `Pulse`: per-topic generations on `ScriptControl`'s lock (phase 2)

`ScriptControl` already has the mutex and condvar that Stop and Pause use to wake a sleeping
script. Add generations to the same lock:

```rust
bitflags! {
    pub struct Topics: u32 {
        const SESSION  = 1 << 0; // loginResponse, logout, connectionLost, GameLoaded
        const MAP      = 1 << 1; // moveToArea
        const CELL     = 1 << 2; // moveToCell (sent); uotls with strFrame for us
        const SELF     = 1 << 3; // ct with our own p entry (HP, MP, intState)
        const COMBAT   = 1 << 4; // any ct (monsters' HP and state, auras)
        const MONSTERS = 1 << 5; // monster respawn / state packets (phase 0 names them)
        const DROPS    = 1 << 6; // dropItem, getDrop
        const ITEMS    = 1 << 7; // addItems, removeItem, equipItem / unequipItem replies
        const SHOP     = 1 << 8; // loadShop, buyItem, sellItem
        const BANK     = 1 << 9; // loadBank, bank transfer replies
        const QUESTS   = 1 << 10; // acceptQuest reply, ccqr, getQuests
    }
}

impl ScriptControl {
    /// From packet handlers (player thread): bumps the topics' generations and wakes every
    /// sleeper. Holds the lock for a few instructions and never waits, so it is allowed in
    /// `on_flash_call`.
    pub fn fire(&self, topics: Topics);

    /// The generations now; taken before a predicate is evaluated.
    pub fn mark(&self) -> Mark;

    /// Sleeps until `deadline`, Stop / Pause, or any of `topics` fired after `mark`; true when
    /// a topic woke it. Off the script thread it never fails with Stopped (the background
    /// loops and Butler use it too).
    pub fn sleep_until(&self, deadline: Instant, topics: Topics, mark: &Mark) -> BotResult<bool>;
}
```

- **No lost wake-ups:** a wait takes `mark()` *before* it evaluates the predicate. A packet
  between the evaluation and the sleep has already bumped the generation, so `sleep_until`
  returns at once.
- `api/packets.rs` and `events.rs` call `bot.control.fire(...)` next to the existing
  `emit`/state updates, one line per packet case. `state.signals` stays; phase 1's single
  counter becomes the SHOP and BANK topics.
- **The shared reads must not hide a wake.** A wait woken by a packet about us would read
  `playing` from a cache up to 100 ms old and go back to sleep. So `fire(SESSION | SELF)` also
  clears `logged_in_read` / `avatar_state_read`. SELF fires only for a `ct` that carries our
  own entry, not every combat tick, so the sharing still works while fighting.

### `for_true` with topics (phase 3)

```rust
pub fn for_true(bot, predicate, loop_fn, timeout, sleep_ms, wake: Wake) -> BotResult<bool>

pub struct Wake {
    topics: Topics,    // empty = today's polling
    fallback_ms: u64,  // poll interval when nothing fires
}
```

- **`loop_fn` keeps its cadence:** `forMonsterDeath`'s `untargetSelf` still runs every 20 ms,
  as in VibeSkua. Those waits gain the early wake, not fewer calls.
- `Wake::poll()` keeps every caller not listed below exactly as it is, including its timeout.

#### The timeout keeps its length

Today a timeout is `timeout` tries, and each try is a sleep of `sleep_ms` *plus* the time its
predicate (and `loop_fn`) spends in Flash calls. That second part is not small where it
matters: with nine tabs, Grid View, a 2 FPS headless player or a slow phone, each call waits
its turn on the one game thread. So today's waits time out later than `timeout x sleep_ms`
exactly when the game is slow.

Once wakes come early and the fallback poll is rarer, counting tries no longer measures time,
but a plain wall-clock deadline of `timeout x sleep_ms` would cut those slow-case timeouts
short and turn slow successes into new failures. So the deadline is:

```
deadline = start + timeout x (sleep_ms + mean check time of this wait so far)
```

re-computed after each check, never shorter than `timeout x sleep_ms`. That is the length the
polling wait would have had at the same call cost. The predicate is checked once more at the
deadline, as today's last try does. A wait that *times out* lasts as long as before; only a
wait that succeeds ends sooner. Phase 0's wait diagnostics record today's real timeout lengths
under load, and the fake-game tests check the new ones against them with a slowed fake host.

### Which wait wakes on what

| Wait | Phase | Wakes on | Fallback poll | Notes |
|---|---|---|---|---|
| `forItemBuy` / `forItemSell` / `forBankLoad` | 1 | the signal | none | like VibeSkua's `AutoResetEvent` |
| `forActionCooldown` | 1 | none | exact | read `world.lock.<action>` once, sleep until `ts + cd`, confirm |
| `forMonsterDeath` | 3 | COMBAT, SESSION | 20 ms (its `untargetSelf` loop) | wakes early only; the biggest latency win (every kill) |
| `forDrop` / `forPickup` | 3 | DROPS, SESSION | 1000 ms | the answer is already packet state |
| `forQuestAccept` / `forQuestComplete` | 3 | QUESTS, SESSION | 500 ms | `acceptQuest` reply (`bSuccess`, `QuestID`), `ccqr` (both in `packets.txt`) |
| `kill`: `ForItemToken` wait | 3 | COMBAT, DROPS, ITEMS, CELL, SESSION | 500 ms | the token already queues those events |
| `forMapLoad` | 3 | MAP, SESSION | name: 500 ms; then `loaded`: 100 ms | the map SWF loads after `moveToArea`, so the second half keeps polling |
| `forCellChange` | 3 | CELL, SESSION | 500 ms | `moveToCell` is sent by the client, which sets `strFrame` in the same frame |
| `forCombatExit` | 3 | SELF, SESSION | 500 ms | our `intState` is in `ct` |
| `forMonsterSpawn` | 3 | MONSTERS, MAP, SESSION | 500 ms | the respawn packet is named in phase 0 |
| `forFullyRested` | 3 | SELF, SESSION | 1000 ms | HP/MP regen arrives in `ct`/`uotls` (confirm in phase 0) |
| `forItemEquip` | 3 | ITEMS, SESSION | 500 ms | `equipItem` reply |
| `forPlayerPosition` | - | none | 100 ms | unchanged: walking is client-side |
| `hunt` sleeps (`bot.sleep(200)`) | 3 | MONSTERS, COMBAT | 200 ms | sleep on the pulse instead of a fixed sleep |

The phase 3 rows are ordered by expected gain; if phase 0 shows a row doesn't matter, it can
stay polling. Every phase 3 wait also wakes on SESSION, so a logout or lost connection ends it
at once.

### Other pollers (phase 3)

- **Background timer:** in game it sleeps on SESSION with today's 100 ms "still in game?"
  check as the fallback, so login, logout and `connectionLost` are seen at once. Out of game
  it keeps checking on every tick, as now.

### Phase 4: state from packets, checked in shadow mode first

After phase 3 each check is still a Flash call, just a rarer one. The remaining frequent reads
could come from packets:

| Value | Today | From packets |
|---|---|---|
| `playing` (logged in, alive) | 2-3 calls, shared 100 ms | SESSION + our `intHP`/`intState` in `ct`, cleared at logout |
| our HP / MP / max | 1 call each | `ct` `p[<me>]` |
| target alive (`has_target`) | `getTargetMonster` (returns the whole monster) | the target from `combat::attack` / `cancelTarget`, its HP from `ct` `m[<mapId>]` |
| current cell / pad | 1 call each | `moveToCell` sent; `uotls` for us |

These would be the first values *answered* from packets instead of only woken by them, so
they need checking in game, which is Turu's weakest area. The game applies some packets after
`pext` sees them, so a packet value can differ from the game's own for a moment, in ways that
are hard to notice. Do this phase only if the scale cases (many tabs, Android, AIR) still need
fewer calls after phase 3:

1. **Shadow mode** (Options > Application > Diagnostics, "Check packet state"): each read
   computes both values and logs mismatches (`packet state: hp cache=1200 flash=1180 (ct 3 ms
   ago)`), counted per value in `/debug/perf` and the monitor. It changes no behaviour.
2. Run the farmrun setup (`HEADLESS_MONITORING_HANDOFF.md`) in shadow mode: kill farms, quest
   farms, a merge, a relogin. A value graduates once its mismatches are zero, or explained by
   a known lag that a short grace window covers.
3. Graduated values answer from the cache, with Flash as the fallback while the cache is unset
   (just after login, or after a missed packet). The shadow switch stays.

### Phase 5: handlers inside engine waits

`bot.handlers` run between bot calls (`turu.ts` `runDueHandlers` after each `__turu` reply) and
during `bot.sleep`, but not inside one long engine wait. With the pulse:

- `__turu`'s closure in `script/runtime.rs` takes the `Ctx` (rquickjs passes it to a Rust
  function that asks for it). For the length of the dispatch it installs a scoped thread-local
  hook that calls the JS `runDueHandlers(names)`.
- `sleep_until` also stops at `bot.handlers.next_due()` (new, beside `take_due`). When a
  handler is due, the wait calls the hook, then goes back to waiting until its own deadline.
- **Reentrancy:** a handler's bot calls re-enter `__turu` on the same thread, which QuickJS
  allows. Handlers don't run from inside a handler (the existing `running` flag, plus a depth
  guard on the hook). A handler's `ScriptStopped` ends the wait with `Stopped`; any other error
  is logged, as `runDueHandlers` does now. This is closer to VibeSkua than today: its handlers
  run on a timer thread at any moment.

This is a correctness fix, worth doing whatever phase 0 shows about speed.

## Phases

| Phase | What | Done when |
|---|---|---|
| **0. Measure** | Built: `perf.rs`. Add wait diagnostics like VibeSkua's `RecordWaitDiagnostics` (per wait: completed, timed out, checks, time, and time from the satisfying packet to the return) to `/debug/perf`. Split each farm loop's time into waits, pacing sleeps (`ActionDelay`, skill timer) and the rest, and count the calls made by script-side polling (script thread, outside an engine wait). Record real timeout lengths under load. Capture packet logs (`turu::packet`, headless) for a kill, a respawn, equip, buy, sell, bank, map and cell changes and resting; `packets.txt` covers only a quest turn-in. Keep them as test fixtures | baseline for 1 tab, 4 tabs and Grid View; the expected gain of phase 3 written down (loop time and calls); the topic table confirmed or corrected |
| **1. Quick wins** | `for_signal` on a generation counter; `forActionCooldown` computed; Butler's class name on events | existing tests pass; fewer calls in `/debug/perf`; buy/sell/bank waits return within ~1 ms of their packet |
| **2. Pulse** | `Topics`, `fire`, `mark`, `sleep_until`; `fire` calls in `packets.rs`/`events.rs`; clearing the shared reads; Options > Application "Event waits" (off = today's polling) | unit tests below; every test passes with it on and off |
| **3. Waits** | `for_true` with `Wake` and the load-aware deadline; the table above, in its order; kill token and hunt sleeps; the background timer | fake-game tests below; a farmrun with it on: fewer calls, lower wait latency, timeouts no shorter, server "too soon" warnings not up |
| **4. Packet state** | only if the scale cases still need it: the cache, shadow mode, graduating one value at a time | per value, zero unexplained mismatches over a few hours of farming |
| **5. Handlers in waits** | the scoped hook and `next_due` | a handler fires on time during `forMapLoad` and a long `kill.monster` (test); the `STATUS.md` gap removed |

Phases 1-3 don't change any answer. Phase 4 is the only one that needs in-game proof before
shipping. Phase 1 needs nothing else; phase 5 needs only phase 2. If phase 0 shows the gain of
phase 3 is small and script-side polling dominates, do phases 1, 2 and 5, then the script-side
API, and keep phase 3 to its first rows.

## Testing

- **Phase 1:** a signal between the flag check and the sleep still wakes `for_signal`; Stop
  and Pause still wake it; `forActionCooldown` returns at `ts + cd` with 1-2 calls and still
  waits out a lock the game resets; Butler broadcasts the new class after an equip packet.
- **Pulse:** `fire` wakes a sleeper on its topics and not on others; Stop and Pause still wake
  it; a `fire` between `mark` and `sleep_until` returns at once; `fire` never blocks.
- **Fake game:** `tests/engine_a_packets.rs` and others already push `pext` through
  `Bot::on_flash_call`. For each wait in the table: it returns within ~10 ms of its packet
  (not ~100 ms) and makes at most N Flash calls during 2 s of silence (counted with
  `bot.flash.perf()`). A wait whose packet never comes still succeeds through the fallback.
- **Timeouts:** with a fake host whose calls take 0 ms and 30 ms, a wait that never succeeds
  lasts at least as long with "Event waits" on as with it off.
- **Replay:** phase 0's packet logs replayed into the fake game, so the topic table is tested
  against real packets.
- **Off switch:** the suite also runs with "Event waits" off, so the polling path keeps working.

## Risks

- **Faster isn't always the same.** A wait that returned up to 100-200 ms after its condition
  now returns at once, so the next action follows sooner. The server already refuses some
  actions sent too soon. Mitigations: pacing sleeps are untouched; the server's warnings are
  counted before and after; the switch turns it off; if a wait needs it, give that wait a small
  settle delay rather than slowing every wait.
- **Shorter timeouts under load.** A wall-clock deadline would end waits sooner than today's
  tries when Flash calls are slow, the very cases this is for. The load-aware deadline and the
  slowed-host test cover it.
- **The gain may be small.** Pacing, not waits, sets a farm loop's speed, and scripts poll on
  their own. Phase 0 sets the expected gain before phase 3; phases 1 and 5 are worth doing
  either way.
- **A packet that never comes, or comes under another name,** costs only time: the fallback
  answers. Phase 0's logs and shadow mode find these.
- **Lock discipline:** `fire` runs on the player thread and takes only the control mutex, which
  sleepers release in the condvar wait and nobody holds across a Flash call. Clearing the
  shared reads takes `bot.state`, which `on_flash_call` already takes.
- **Phase 5 reentrancy:** a handler that waits while the script is inside a wait. The depth
  guard keeps it to one level.

## Open questions

- **Script-side waits.** `bot.wait.forTrue(cond, timeout, interval, { wakeOn: ["drops",
  "items"] })` would let CoreBots' loops (`Core.huntMonster`'s `while (!checkInventory)`) wake
  on packets, through the same pulse. Phase 0's count of script-side polling calls decides
  whether this comes before phase 3 or after it.
- Fallback intervals (500 ms / 1000 ms) are guesses; phase 0's numbers should set them.
- Does the skill timer gain anything from SELF (probably not: cooldowns end client-side)?
