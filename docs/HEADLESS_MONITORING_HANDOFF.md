# Headless run monitoring: handoff

What a long monitored `--headless` run of FarmerJoe DoAll (`scripts/other/farmjoekits/0FarmerJoeDoAll.ts`)
found on 2026-11-08/09, what was fixed, what is still open, and how to pick it up. The account
was `turu_next` on Twig (`turu-args.txt`), a fresh character taken from level 1 to 35 over about
14 hours across several restarts, on macOS (Apple silicon, AC power).

Start with **Open issues** and **Next steps**; **How to run and watch** has the tooling and its
pitfalls.

## Committed in this work

| Commit | What |
|---|---|
| `61f0fdc2e` | Headless runs at a hidden tab's 2 FPS in game, and its player has no mouse (`app/src/headless.rs`); Android drops the pointer while no window shows the game (`android/src/lib.rs`, `set_touchable`). |
| `6cf2f8faf` | `tools/farmrun/`: run a script headless, sample `/status` every minute, live stuck alerts, `summarize.py` report. |
| `fb2f6b4c2` | farmrun samples resources (`tools/farmrun/sample.py`): CPU user/system, RSS, footprint/PSS, threads, open files, network, disk (Linux), log size. |
| `7774797b2` | `TURU_PACKET_LOG=1` logs every packet the client sends (`turu::packet`); the summarizer lists the packets before each "Please slow down" and AFK stretches. |
| ruffle-aqw `15c114985` + Turu `60ec41011` | `memory biggest:` line: the loaded files holding the most memory, with copies (`Player::aqw_movie_sizes`). |
| ruffle-aqw `cc12f1908` | Monster-retention diagnostics (below). |

**ruffle-aqw's `aqw` branch is 2 commits ahead of origin: push it before pushing Turu**, or
Turu's submodule pointer names commits nobody else can fetch.

Docs: `docs/HEADLESS.md` (farmrun, `TURU_PACKET_LOG`, `memory biggest:`), `docs/STATUS.md`
(headless frame rate and mouse), `docs/ANDROID.md` (pointer while hidden).

## Monster-retention diagnostics (ruffle-aqw `cc12f1908`)

Six files in `ruffle/core/src`, built and committed; **the class-chain part has never run**:

- `aqw_retention.rs`: `movie_holders()`, every reachable `MovieClipShared` and `TranslationUnitData`
  made from given movies.
- `display_object/movie_clip.rs` `aqw_shared_movie()`, `avm2/script.rs` `aqw_unit_movie()`,
  `display_object.rs` re-export: read a holder's movie pointer.
- `player.rs` `log_movie_holders()`, run from `log_loader_retention` (`TURU_RETAIN_URL`): for
  copies of the movie that no live `LoaderInfo` points at, logs per copy how many holders it has,
  the shortest chain that isn't its own library, and (new, untested) the chain to its linked AS3
  class.
- `library.rs`: `RUFFLE_AQW_LIBRARY_WHY=<URL part>` logs why the library cleanup keeps a tested
  library (`kept: an AS3 class linked to its symbol N is alive` / `its code ... is alive` /
  `instances of its symbols [...] are alive`), and `Library::aqw_classes_of()`.

They're behind env vars, at no cost otherwise; keep or drop them once the leak is fixed. The
remaining `rustfmt --check` hunks in those files are older code, not from this work.

## Open issues (most valuable first)

### 1. Monster SWFs pile up with every map join (memory leak)

**Evidence.**
- The `memory census:` and `memory biggest:` lines show monster files with one live copy per join of their map, kept after leaving:
  - Sandsea's monsters at x4 after 4 joins, with the character elsewhere;
  - Icestorm's at x3.
- FarmerJoe's Blacksmithing rep loop (`CoreFarms.blacksmithingREP`, non-gold) alternates Greenguard East/West every 15-40 s (69 joins in 30 min). It reached x25-26 per monster:
  - live SWFs 117 -> 385 in 25 min;
  - Rust heap +170 MB/h;
  - footprint 394 MB.
- A 4-hour run with only save states (`/whitemap` round trips every 30-60 min) grew:
  - live SWFs 46 -> 128;
  - heap 150 -> 253 MB;
  - footprint 222 -> 308 MB.
- Map files themselves are freed about 4 minutes after leaving (`maps/...` back to x1). Map SWFs are 0.4-1.7 MB uncompressed; monster SWFs ~0.1-0.2 MB each. The cost is mostly the AS3 objects built from them (GC objects +25-40k/h).

**What's known about the cause** (logs: `tools/farmrun/out/20261109-holders-retain*.log`):

- Old copies have **no live `LoaderInfo`** (`loader retention: 0`), so it isn't the load-manager bug fixed for death ads.
- Each copy has ~40 holders: 39 are its own library's symbols (expected while the library is kept). The one outside chain is `GcRootData -> DomainData -> ScriptData -> TranslationUnitData`. Each load gets a child domain (`Avm2Domain::movie_domain` in `loader.rs` when the game passes no `LoaderContext`), which the copy's own library holds (`MovieLibrary.avm2_domain`, inline in the root). So that chain is only the library again.
- `RUFFLE_AQW_LIBRARY_WHY=mon/Mogzard_v2.swf` on a controlled test (log in, no script, 8 joins alternating `greenguardwest`/`greenguardeast` via `POST /map/join`, then leave for Oaklore):
  - the cleanup (`Library::finalize_movie_libraries`) keeps the old copy with **"an AS3 class linked to its symbol 54 is alive"**, still 4 minutes after leaving;
  - its backoff doubles 0 -> 1 -> 3 -> 7 (test interval, capped at 63 GC cycles; a cycle is ~10 s, `RUFFLE_GC_CYCLE_SECONDS`).
- The controlled test left only **1** old copy, the real farm 25. The difference is probably combat: monsters fought and killed, `world.monsters`, `pMC`, death animations, drops. That's not confirmed.

**Next step.** Find what holds that AS3 class:
1. Run the class-chain report: `TURU_RETAIN_URL=mon/Mogzard_v2.swf RUFFLE_AQW_LIBRARY_WHY=mon/Mogzard_v2.swf TURU_PERF_LOG=30`, and read the `class #n` lines.
2. Do it in the real loop, not only with manual joins: run FarmerJoe until `blacksmithingREP` ("Farming rank 4", "Using Non-Gold Method"), or call `Farm.blacksmithingREP()` from a small script.
3. Candidates: the game's own caches (AS3 objects holding the monster class, e.g. a dictionary of loaded monster classes keyed by file), or Ruffle keeping `ClassObject`s through prototypes/vtables of live instances.

Flash itself frees an unreferenced child domain's classes, so if the holder is Ruffle-internal it's a ruffle-aqw fix (as with `LoadManager`). If it's the game's AS3, the fix may be an AQW-specific workaround.

### 2. Auto-relogin gives up for good after 5 attempts; the script isn't restarted

At 04:11 the Mac lost its network (every request failed: `game.aq.com`, `content.aq.com`).
- `Bot stopped due to player logout`: CoreBots stops the script on logout. VibeSkua restarts it after a relogin (`Bot.Manager.RestartScriptAsync`); **not ported** (STATUS lists it).
- Relogin: 5 attempts in ~95 s, then `[Relogin] Unable to reconnect within configured retries.` and nothing more (`bot/src/api/servers.rs` ~line 1120). The client sat logged out ~35 min until logged in by hand (`POST /login?server=Twig`, then `POST /script/start`).
- The `retryRelogin` option (default true, `bot/src/options.rs`) is **read nowhere**. VibeSkua has it only in the options UI and interface, so it's dead there too.
- The relogin aimed at **Twilly**, not Twig where it was, because the saved `reloginServer` option is `Twilly` (`options.json`).

Fixes:
- While `retryRelogin` is on, keep retrying with growing gaps (1, 2, 5, 10 min) after the attempts run out.
- Port CoreBots' restart after relogin, so unattended runs survive network drops.
- Consider dropping `reloginServer` when it differs from the server the run was started on, or at least logging it.

### 3. Quest loads send `getQuests` 3-6 times; the server says "too soon"

CoreBots' `ensureLoad` (`scripts/core/CoreBots.ts` ~3044) calls `bot.quests.load(id)` every 100 ms until the quest shows; C# does the same (`Wait.ForTrue(..., Quests.Load, 20)`, `WAIT_SLEEP` 100). The reply takes ~300 ms, so every quest step sends 3 identical packets (6 for Sandsea's 5 registered quests) and the server answers "Please slow down. Last action was too soon!", several per phase change (5 in 5 s at Sandsea).
Fix (engine, all scripts benefit): in `bot/src/api/quests.rs` `load()`, skip `world.showQuests` for IDs requested less than ~1 s ago. `ensureLoad` keeps polling every 100 ms, so loading isn't slower.

### 4. Skills sent less than 1 s apart are sometimes rejected

Over 1,577 skill uses (auto-attacks aside): 4 of 74 sent 750-1000 ms after the previous skill drew "too soon", 1 of 226 at 1250-1500 ms, 0 of 1,277 otherwise. So the server seems to have a ~1 s gap between skills that the client's `canUseSkill` doesn't enforce; the rejected skill's damage is lost. Small (~5 lost skills/h). Likely VibeSkua's 100 ms skill loop too. Possible fix: a 1 s minimum between skill packets in the skill timer (`bot/src/skills.rs`); measure kill rate before and after.

### 5. CPU: what's left on the game thread

Release build, headless, after the fixes: **1-4% of a core**, game thread 8-30 ms/s (ticks 6-22, bot calls 2-8), ~190-210 bot calls/s. The debug build overstates everything ~3-4x: measure on release.
A debug-build profile (`sample <pid> 30`, files `tools/farmrun/out/20261108-profile-debug-*.txt`) splits the busy time:
- frames ~39%;
- answering bot calls ~20%;
- AS3 timers ~12%;
- the game's socket ~10%;
- GC ~7%.

Inside frames, **85% of ENTER_FRAME time is AS3 `gotoAndStop` to the frame a clip is already on**: `MovieClip::no_op_goto` still runs `run_inner_goto_frame` over the whole stage (`ruffle/core/src/frame_lifecycle.rs`). It comes from AQW's own code, not skua.swf (skua's modules run at most every 100 ms). It's upstream Ruffle behaviour that emulates Flash. An optimization would need care (count no-op gotos per frame and the clips first).

### 6. Smaller things

- **Butler broadcast** makes ~25 Flash calls/s (15% of all) even with one account. Every 2 s it reads the whole inventory (`world.myAvatar.items`, grows with the inventory) just for the class name (`bot/src/butler/mod.rs` `broadcast`). Re-read the class on equip/item events instead.
- **Item added** (`bot/src/background.rs` `item_added`) re-reads the whole inventory per drop.
- **Quest watchdog false alarms** (inherited from C#): "completable without a turn-in for N s" measures time since the last turn-in, and fires 0.1-0.3 s after the turn-in packet went out, before the reply. It cycles the quest log needlessly.
- `checkClassRank` logs class points as "Rank: 302500" (C# does too).
- Every monster SWF is live **x2** right after a map loads, the map file x2 for a few minutes, and `moveToCell Enter Spawn` is sent twice per join (the game's own and `join`'s jump, as in VibeSkua). Harmless.
- The tutorial badges step sleeps ~1.5 s per badge even when all are done (~15 s per start).
- Deaths: 158 in a run, frequent while ranking up a fresh class; you said that's expected while XP/CP grow.

### 7. Tooling gaps

- `/status` has no reputation, XP, class points or gold. Long rep grinds (2 h at Sandsea to rank 10) can't be followed except by quest counts. Adding them would let farmrun report progress per hour.
- macOS `nettop` sums only open sockets: farmrun's network figure there is a lower bound. There's no per-process disk I/O on macOS without root.
- The census lists only the top 8 URLs by copies; `memory biggest:` the top 8 by size.

## Fixed in this work (measured)

- **Headless ran the game at full frame rate with a phantom mouse.** Ruffle starts with `mouse_in_stage: true`; headless never cleared it, so every update (~50/s, for timers and the socket, whatever the frame rate) hit-tested the whole display list (`run_mouse_pick`): 13% of the player's time at 30 FPS, 75% at 2 FPS. Now 2 FPS in game, no mouse. FarmerJoe at Icestorm, debug build, 5 min each:
  - 30% of a core at 30 FPS;
  - 17% at 2 FPS;
  - 13% with both fixes;
  - as many kills.

  Android drops the pointer while no window shows the game (type-checked and clippy'd only, not run on a device).

## How to run and watch

- **Run:**

  ```bash
  BUILD=0 BIN=target/release/turu TURU_PACKET_LOG=1 caffeinate -is tools/farmrun/run.sh scripts/other/farmjoekits/0FarmerJoeDoAll.ts 0
  ```

  `0` means until the script stops. `caffeinate` keeps the Mac awake: Turu holds no sleep assertion of its own. Closing the lid still sleeps the Mac.
- **Report:**

  ```bash
  python3 tools/farmrun/summarize.py tools/farmrun/out/<run>.log
  ```

  It works mid-run.
- **Control API:** farmrun's port is 9031. Its token is generated per run and lives only in the process environment: `ps eww -p <pid> | tr ' ' '\n' | grep TURU_API_TOKEN`.
- **Pitfalls hit:**
  - farmrun passes `--exit-on-script-stop`, so `POST /script/stop` quits Turu. For experiments, start Turu without a script; the launcher used was `--headless --user ... --server ... --api=127.0.0.1:9031` with `TURU_PASSWORD` and `TURU_API_TOKEN` set.
  - Don't edit `tools/farmrun/run.sh` while a run uses it: bash reads the script as it goes.
  - Monitor pipelines must be line-buffered all the way (`cut` at the end buffers; use `awk ...; fflush()`).
  - Measure on release (debug overstates 3-4x).
  - `RUST_LOG` isn't needed: `turu::packet` and `ruffle_aqw_perf` show at the default headless filter.
- **Logs** (`tools/farmrun/out/`, git-ignored):

  | File | What |
  |---|---|
  | `20261108-182240-*` | Release, packets: getQuests and skill "too soon" analyses. |
  | `20261108-205338-*` | ~7 h, release: Sandsea, Blacksmithing x25 leak, the network drop and relogin. |
  | `20261109-holders-retain*.log` | Retention experiments. |
  | `20261108-profile-*.txt` | `sample` profiles. |
  | `20261108-debug-build-first-run.log` | The first debug run. |

## Next steps

1. Finish issue 1: run the class-chain report in the Blacksmithing loop, find the class holder, fix it in ruffle-aqw (or work around it), and re-measure with farmrun (live SWFs and heap should stay flat across joins). Then keep or drop the diagnostics.
2. Issue 2: relogin retry with backoff, and port the CoreBots restart after relogin. Test by cutting the network (e.g. turning Wi-Fi off) for 3+ minutes mid-run.
3. Issue 3: de-duplicate `quests.load`; check with `TURU_PACKET_LOG` that a quest step sends one `getQuests` and the "too soon" count drops.
4. Add reputation/XP/CP/gold to `/status` and to farmrun's samples.
5. Optional: Butler's inventory reads, the skill 1 s gap, the no-op `gotoAndStop` cost.
