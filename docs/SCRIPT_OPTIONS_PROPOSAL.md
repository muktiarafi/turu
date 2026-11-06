# Proposal: Script Options for runs with no one at the window

Status: A + C1 built (see "As built"); C2 isn't. Gap #48 in `docs/PORTING_GAPS.md`: the Script Options window never
opens by itself for `--headless`, control API or relogin runs. It does for `--run-script` and
Army Control's Start, as in VibeSkua. This doc says what VibeSkua does on each start path, what
Turu does today, and what "opening by itself" should mean where there is no window.

## How it works today

### VibeSkua

Everything goes through `ScriptManager.StartScript()` (`Skua.Core/Scripts/ScriptManager.cs`):

1. It compiles the script and calls `LoadScriptConfig`, which sets `_configured[storage]` to the
   script's `DontPreconfigure` (or `false` when it has `Options` but no `DontPreconfigure`). This
   runs on **every** start, so "configured" never survives from one run to the next.
2. `needsConfig = hasOptions && !_configured[storage]`. If so and `SilentConfig` is false, it
   calls `Config.Configure()`, which shows the dialog (`_dialogService.ShowDialog`) and blocks
   the start until it closes.
3. Inside the script, CoreBots' `SetOptions()` calls `Bot.Config.Configure()` when the script has
   `SkipOptions` and it isn't ticked (`Scripts/CoreBots.cs:252`). `Configure()` doesn't look at
   `SilentConfig`, so **this dialog shows on silent runs too**.

`SilentConfig` is set in only two places:

* `SkuaStartupHandler` (`Skua.App.Avalonia/Services/SkuaStartupHandler.cs:123`): with
  `--run-script` **and** `--script-options-storage`, it sets `OverrideStorage` and
  `SilentConfig = true`, and never sets it back, so it holds for the rest of that client's life
  (relogin restarts included).
* The Scheduler (`ScriptSchedulerViewModel.cs:432`): `true` around each entry's `StartScript()`,
  then `false`.

VibeSkua has no headless mode and no HTTP API, so it never has to choose what to do with no
window: there is always a dialog, and a start blocks on it until someone clicks.

### Turu

`turu_bot::script::start` and `start_silent` (`bot/src/script/mod.rs:496`) set `silent` and
start the script thread, which calls `preconfigure` (the window unless `dontPreconfigure`) and
then runs. `ScriptManager::configure` (also used by `bot.config.configure`, which CoreBots'
`setOptions()` calls until `skipOptions` is ticked) returns at once, without a word, when the
script has no options, the run is silent, or `interactive` is false. `interactive` is set from
`!headless()` (`frontend/src/turu.rs:129`) and by the egui UI once it is up
(`frontend/src/gui/turu/mod.rs:194`). With a window, a request in a tab you aren't looking at
waits until you open it (the tab's "!", and Army Control says so).

When the window isn't shown, the script reads what `ScriptOptions::load` found: the values in
`options/<storage>.cfg` (or `options/Queue_<name>_<id>.cfg` for a playlist entry), and the
declared defaults for anything missing, including when the file doesn't exist. Nothing logs
which of these it was, and nothing says the window was skipped. A message box in the same
situation does log (`[Message box, no one to answer] ...`); the options window doesn't.

### Start paths

| Start path | VibeSkua | Turu today | Values used when the window doesn't show |
|---|---|---|---|
| Script Loader / nav bar Start, Search Scripts Start | Window (unless `DontPreconfigure`); CoreBots' window unless `SkipOptions` | Same (`script::start`) | — |
| `--run-script` (and Army > Accounts' auto-start) | Window, as above | Same (`startup::auto_login` → `script::start`) | — |
| `--run-script` + `--script-options-storage` | Silent start with that storage, for the client's whole life; CoreBots' window still shows | Flag not built (gap #32) | — |
| `--run-script` with `--headless` | (no headless) | `script::start`, but `interactive` is false: silent, no log | Saved `.cfg`, else defaults |
| Army Control Start (all tabs / clients) | Start All Scripts → each client's Start toggle: window in each | `POST /script/start?configure=1` to each client: window in each desktop client | Headless clients: saved, else defaults, no log |
| Control API `POST /script/start` | (no API) | `start_silent` unless `configure=1` | Saved, else defaults, no log |
| Web monitor Start (`/api/bots/<id>/script/start`) | (no monitor) | Control API route without `configure`: silent, even for a desktop tab | Saved, else defaults, no log |
| Auto-relogin restart | `StartScript()` non-silent: window again (unless `SilentConfig` from the startup flags), and CoreBots' window again | `start_silent` (`api/servers.rs:1299`) | Saved, else defaults (the same values as the run it restarts) |
| Scheduler entry (window, `--run-playlist`, `/scheduler/start`) | Silent start with the entry's storage; CoreBots' window still shows | `start_silent` with `options/Queue_*.cfg`; CoreBots' window doesn't show either | The entry's `.cfg`, else defaults |
| Loadout enhancer (`loadouts.rs:559`) | `StartScript()` on a generated script with no options | `start_silent`, generated script with no options | — |

So Turu matches VibeSkua wherever both have a window, with two deliberate differences: the
relogin restart is silent, and a silent run also silences `bot.config.configure` (CoreBots'
`setOptions()`). Both stop an unattended client from stalling behind a dialog nobody will see.

## What's actually wrong

The behaviour on the headless paths, start with what's saved, is the only one that works without
a window. What's missing is around it:

1. **Nobody is told.** A headless run, an API start or a monitor Start uses defaults silently
   when no `.cfg` was copied over, and the first sign is the script doing the wrong thing.
2. **Setting options needs a desktop.** `docs/HEADLESS.md` says to set them on a desktop and copy
   `options/<storage>.cfg` to the server. There's no way to see or change a script's options
   through the control API or the monitor.
3. **`configure=1` means different things.** Army Control sends it to every client; a desktop
   client shows the window, a headless one quietly doesn't.

## Options for "opening by itself" without a window

### A. Start with saved or default values, and say so

Keep starting at once, but log and surface what was used whenever the window would have shown
and didn't (the script has options, `preconfigure` or `bot.config.configure` wanted the window,
and the run is silent or has no UI):

* Script log: `Script Options not shown (headless): using options/Farm.cfg.` or `... no
  options/Farm.cfg, using the defaults.` once per run (the preconfigure and the in-script
  `configure` share one flag, so CoreBots doesn't log twice).
* A monitor event (`EventKind::ScriptOptionsDefaults` or a field on `ScriptStarted`) when it fell
  back to defaults, so the web monitor and webhooks can flag it.
* `POST /script/start` answers `{"started": ..., "options": "saved" | "defaults" | "none"}`, so
  Army Control can show "defaults" next to a headless client.
* At `--headless` startup with `--run-script`, warn before the game loads when the script has
  options and no `.cfg`.

Small, changes no behaviour, and fixes problem 1. It doesn't fix problem 2.

### B. Refuse to start until options are saved

A silent or windowless start of a script that would have shown the window, with no `.cfg` for
its storage, fails: `409 "Farm.ts has Script Options that were never saved; set them or pass
defaults=1"`, and `--run-script` logs the same and doesn't start.

This is the safest against a wrong run, but it breaks setups that work today (most scripts are
fine on defaults, and nearly every turu-scripts script uses `dontPreconfigure` plus CoreBots'
`skipOptions`, so "would have shown the window" means asking CoreBots' option before the script
runs). It also can't apply to relogin restarts or playlists without stopping a running farm.
Not recommended.

### C. Show and submit the options through the API and the monitor

Make the options reachable without a window, in two steps:

* **C1, read and write** (no waiting): control API routes on the loaded script:
  * `GET /script/options`: the declaration (storage, groups, each option's name, display name,
    description, kind, choices, default) and the current values, and whether a `.cfg` exists.
  * `POST /script/options`: body `{"name": value}` or `{"Group": {"name": value}}`, checked as
    the window checks them (`ScriptOptions::set`, `set_in`), then saved (`save_options`).
    `?reset=1` puts the defaults back first.
  * Same for a playlist entry: `GET/POST /scheduler/options?id=`.
  * The web monitor gets an Options button on a card that renders the declaration as a form
    (toggle, number, text, choice) and posts it. It goes through the monitor's control
    allow-list like the other commands.
* **C2, the window over the network:** when a run on a client with no UI wants the window and
  someone can answer remotely (`configure=1`, or `--headless` with the monitor's control on),
  the script waits as it does for the egui window: `ScriptManager::configure` posts its request,
  `/status` and the snapshot report `waitsFor: "Script Options"`, the monitor card shows the
  form, and `POST /script/options?submit=<request>` (or `cancel`) answers it via
  `finish_configure`. Army Control would show "waiting for Script Options" for that client,
  as it already does for a hidden desktop tab.

C1 fixes problem 2 and is useful for everyone (change a server's options from a phone). C2 is the
closest to "opens by itself", but it makes a headless start block on a person, which is the
thing headless mode exists to avoid, and it needs a timeout or a "nobody is coming" rule for
clients nobody is watching.

### Recommendation

**A, plus C1.** Start with what's saved, say so in the log, the monitor and the API reply, and
let the API and the monitor read and set a script's (and a playlist entry's) options. That
removes the "copy `.cfg` files from a desktop" step from `docs/HEADLESS.md` without ever making
an unattended client wait. Leave C2 until someone asks for it; if it's built, only for
`configure=1` (Army Control's Start), never for `--run-script --headless` or relogins.

`configure=1` on a headless client then means "start with the saved values, and report
`options: "defaults"` if there were none", which is the answer Army Control needs to show.

## Relogin

VibeSkua re-shows the window on an auto-relogin restart: `Relogin` calls `StartScript()`
(`Skua.Core/Scripts/ScriptInterface.cs:874`), which isn't silent unless the client was started
with `--script-options-storage`; `LoadScriptConfig` resets "configured" on every start, so a
script without `DontPreconfigure` shows it again, and CoreBots' `SetOptions()` shows its own
again unless `SkipOptions` is ticked. On an unattended client the restarted script then waits
behind that dialog until someone closes it. (`RestartScriptAsync`, stop + 5 s + `StartScript`,
isn't called anywhere in VibeSkua.)

Turu should **keep the relogin restart silent**, as now: the user already chose the options for
this run, the restart reloads the same storage, and a dialog after an unattended disconnect is a
stall, not a feature. VibeSkua's own `--script-options-storage` clients behave this way too,
since their `SilentConfig` never resets. With A, each restart logs its "not shown" line
again (one per run), so the log says which values the restarted run uses. Record the divergence in
`docs/STATUS.md`'s existing "Script options" entry (it already names the relogin case); no
behaviour change.

## What each option touches

| | Files | API / CLI | Size |
|---|---|---|---|
| A | `bot/src/script/mod.rs` (one "not shown" note per run in `configure`/`preconfigure`, with whether `ScriptOptions::path()` exists), `bot/src/script/options.rs` (remember whether the file was found in `load`), `bot/src/monitor/` (event or `ScriptStarted` field), `bot/src/control_api.rs` (`options` in the start reply), `bot/src/army.rs` + `frontend/src/gui/turu/army.rs` (show "defaults" per client), `frontend/src/turu.rs` (startup warning), tests in `bot/tests/`, `docs/HEADLESS.md`, `docs/STATUS.md` | `/script/start` reply gains `options`; no new flags | ~100 lines + tests, half a day |
| B | `bot/src/script/mod.rs` (refuse in `start_with`), `bot/src/control_api.rs` (`defaults=1`), `bot/src/startup.rs`, docs | `/script/start?defaults=1`; maybe `--script-defaults` | ~60 lines + tests, but needs CoreBots' `skipOptions` read before the run |
| C1 | `bot/src/script/options.rs` (`Serialize` for `Declaration`, `OptionDecl`, `Kind`; a JSON view of the values), `bot/src/script/mod.rs` (reuse `options()`/`save_options`), `bot/src/scheduler.rs` (an entry's options), `bot/src/control_api.rs` (routes + doc table), `bot/src/monitor/server.rs` (allow-list) and the monitor page (form), tests, `docs/HEADLESS.md` (routes; drop or shorten the copy-files table) | `GET/POST /script/options`, `GET/POST /scheduler/options?id=` | ~250 lines Rust + ~250 lines page JS, 1–2 days |
| C2 | C1, plus `bot/src/script/mod.rs` (a `remote` answerer next to `interactive`, timeout), `/status` and the snapshot (`waitsFor`), Army Control's per-client status, the monitor card | `POST /script/options?submit=` / `?cancel=` | +~200 lines, 1–2 days, and a decision on timeouts |


## As built

* **A** (`bot/src/script/mod.rs`): `ScriptManager::configure` (the preconfigure and
  `bot.config.configure`) notes once a run, when the window can't show, `Script Options not shown
  (no window | this start doesn't show it): using the options saved in options/X.cfg`, or `there is
  no options/X.cfg, so the defaults are used` plus a monitor `scriptOptions` event (a new
  `EventKind`, with a `#[serde(other)]` fallback for later kinds). Relogin restarts note it too. The
  note only comes when the window was wanted: a script with `dontPreconfigure` whose CoreBots
  `skipOptions` is ticked says nothing. `/script/start` answers `options` (`none`, `saved`,
  `defaults`, `failed`) and `optionsWindow`; Army Control's outcome line and the monitor's toast say
  "with the default Script Options (none saved)" when a client started without the window on
  defaults. `--run-script` without a window warns at startup (`startup::configure`); the first tab
  is now marked interactive before the startup flags, as `add_tab` does.
* **C1** (`bot/src/control_api.rs`, `ScriptOptions::to_json` / `apply_json`): `GET/POST
  /script/options[?reset=1]` and `GET/POST /scheduler/options?id=[&reset=1]`. A POST sets all the
  values or none and saves (`save_options`, which refuses if another script was loaded meanwhile).
  Editing a playlist entry's options isn't refused while a script runs (the window refuses it, as in
  VibeSkua): reading them doesn't touch the running script.
* **Monitor**: `GET`/`POST /api/bots/<id>/script/options` pass through (POST needs Allow Control and
  takes the body to tabs, Army clients and remote machines: `RemoteCommand.body`). The detail view
  has a Script Options tab: Saved/Defaults pill, each category's options as a checkbox, number,
  text field or menu, changed rows tinted, Save (only with changes and valid numbers), Discard and
  Reset to defaults (confirmed). It reads once on open, after a save, and when another script is
  loaded with nothing edited, so the 2 s refresh never drops typing. Checked in a browser against a
  mock server (desktop and phone widths, dark and light), not against a live client.
* Tests: `bot/tests/control_api.rs` (`reads_and_sets_script_options`), `bot/tests/runtime.rs`
  (`a_run_without_the_window_says_which_options_it_uses`), `bot/tests/scheduler.rs` (entry
  options over the API), `script::options` (`json_sets_all_or_nothing`), `monitor::hub`
  (`checks_routes`).

## Decisions

1. A + C1 (2026-11-06); C2 left out.
2. `--headless --run-script` with options and nothing saved warns and runs on the defaults.
3. The monitor's Start stays silent on a desktop tab; the monitor's Script Options tab sets them.
