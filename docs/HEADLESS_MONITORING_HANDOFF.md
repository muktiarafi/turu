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

## Monster-retention diagnostics (ruffle-aqw `cc12f1908`, extended)

Behind env vars, at no cost otherwise; kept for the next leak:

- `aqw_retention.rs`: `movie_holders()`, every reachable `MovieClipShared` and `TranslationUnitData`
  made from given movies.
- `display_object/movie_clip.rs` `aqw_shared_movie()`, `avm2/script.rs` `aqw_unit_movie()`,
  `display_object.rs` re-export: read a holder's movie pointer.
- `player.rs` `log_movie_holders()`, run from `log_loader_retention` (`TURU_RETAIN_URL`): for
  copies of the movie that no live `LoaderInfo` points at, logs per copy how many holders it has,
  the shortest outside chain and the chain to its linked AS3 class. The walk now leaves those
  copies' own libraries out (`Library::aqw_hide_libraries`), so the chains show what holds them
  from outside, not the library itself; and a `class exported as <ns>::<name> by the domain of
  <url|stage>` line says which domain exports each linked class (`Library::aqw_domains_exporting`).
- `library.rs`: `RUFFLE_AQW_LIBRARY_WHY=<URL part>` logs why the library cleanup keeps a tested
  library (`kept: an AS3 class linked to its symbol N is alive` / `its code ... is alive` /
  `instances of its symbols [...] are alive`), and `Library::aqw_classes_of()`.

The remaining `rustfmt --check` hunks in those files are older code, not from this work.

## Fixed after the handoff (2026-11-09)

Uncommitted at the time of writing: Turu (`bot/`, `frontend/`, docs) and ruffle-aqw
(`core/src/library.rs`, `core/src/player.rs`).

### 1. Monster SWFs piled up with every map join: fixed (ruffle-aqw)

**Cause.** AQW loads all of a map's monsters into **one shared `ApplicationDomain`**: the domain
report showed `Mogzard_v2`'s classes exported by the domain of `OgugStoneaxe_v2.swf`, later of
`Dreadspider2_v2.swf`. Each of those libraries holds that domain (`MovieLibrary.avm2_domain`) and
the domain exports every monster's classes, so library A kept B's classes alive and B kept A's.
The library cleanup (`Library::sweep_movie_libraries`) tested libraries one at a time, each on its
own backoff, so the members of a group were never tested in the same cycle: each was always found
"in use" through the others. A copy of each monster stayed per join.

**Fix.** `sweep_movie_libraries` also picks, for each library chosen for testing, every library
sharing its domain (not the root movie's domain; groups up to 64), so a whole map's monsters are
tested together and freed together once nothing outside holds them. Libraries still in use are
revived as before, and `finalize_movie_libraries` runs again after a revival, so a group member
whose class only became reachable through a revived library is kept.

**Measured** (release, a 25-round Greenguard East/West loop like `blacksmithingREP`'s non-gold
method: `Core.huntMonster` Wolf / Slime with quest 2777, 50 joins, then 5 min in Oaklore):

| | live SWFs, round 1 -> 20 | Rust heap | census |
|---|---|---|---|
| before (`20261109-062337-*`) | 29 -> 123 | 147 -> 145 MB | every monster x6 at round 20 |
| after (`20261109-065804-*`) | 21 -> 29 (24 at round 25) | 136 -> 130 MB (-45 MB/h) | monsters x1-2 |

503 movies freed in 32 min, no `was freed but is still used`, 575 kills/h. Not yet re-measured
over hours or in FarmerJoe's full run.

### 2. Auto-relogin gave up for good after 5 attempts: fixed (Turu)

The log showed the engine *does* restart the script after a successful relogin
(`auto_relogin` in `bot/src/api/servers.rs`; CoreBots' "Bot stopped due to player logout" is that
stop, logged on the way). The network drop lost the run only because the relogin gave up. Now,
with Retry Relogin on (the default; VibeSkua reads it nowhere), a failed round of `ReloginTries`
attempts is followed by another after 1, 2, 5, then every 10 minutes; a login from the game's
screen meanwhile counts as the relogin (and the script restarts), and `/login` or `/relogin`
cancel the wait instead of answering 409. Test: `a_failed_auto_relogin_waits_to_try_again`
(`bot/tests/servers.rs`). **Not yet tested in game** by cutting the network.

The `reloginServer` = `Twilly` target is documented behaviour (`ReloginServer` wins over the server
you were on); left as is.

### 3. Quest loads sent `getQuests` 3-6 times: fixed (Turu)

`quests::load` (`bot/src/api/quests.rs`) leaves out IDs it asked for less than 1 s ago
(`LOAD_REPEAT_GAP`). In game with `TURU_PACKET_LOG=1`: one `getQuests` for quest 2777 per
registration. Test: `quest_loads_skip_ids_asked_for_within_a_second` (`bot/tests/loader_grabber.rs`).

## Open issues (most valuable first)

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

1. Commit: ruffle-aqw first (its `aqw` branch was already 2 commits ahead of origin: push it
   before Turu), then bump `ruffle/` and commit Turu's changes.
2. Re-measure issue 1 over hours: FarmerJoe DoAll with farmrun (live SWFs and heap should stay
   flat; the save-state run grew 46 -> 128 SWFs in 4 h before).
3. Test the relogin retry in game: cut the network (Wi-Fi off) for 3+ minutes mid-run; expect
   "Re-login failed; trying again in 1 min (Retry Relogin)." and the script restarting after.
4. Add reputation/XP/CP/gold to `/status` and to farmrun's samples.
5. Optional: Butler's inventory reads, the skill 1 s gap, the no-op `gotoAndStop` cost.
