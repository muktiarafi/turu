# Proposal: a process for each tab (desktop and Android)

Status: proposed, not built (2026-11-16). Today every tab is a player on one thread of one
process (`frontend/src/players.rs`, `frontend/src/engine.rs`; docs/STATUS.md, "Tabs share one
thread and one process"), on the desktop and on Android. This proposal keeps the tabs as the user
sees them, but runs each tab's game and bot in a process of its own, so that Android killing one
for memory, a crash or a lost GPU device ends one account instead of all of them. Turu Mas is out
of scope (see "Turu Mas").

## Summary

* A **shell** process owns the window (the desktop's winit window, Android's activity), launches
  and watches the **clients**, and routes input to the one on screen. It runs no game.
* Each tab is a **client** process: one player and one bot, the engine loop the desktop's
  `--headless` and Android already share (`engine::run_loop_paced` with one tab), plus the
  screen code Android already has (`android/src/screen.rs`: the game's texture, Turu's egui UI over
  it). The client draws the **whole** tab, Turu's UI included, so the bot windows stay in-process
  with their bot and nothing in `frontend/src/gui/turu/` has to talk to a bot in another process.
* The client on screen draws straight to the shell's surface: on Android, the activity's
  `Surface`, handed over Binder; on the desktop, a GPU texture shared across processes. Hidden
  clients draw nothing.
* Army, Butler and Army Control work between processes already (file-based sync, the control API
  on loopback): the desktop's Army > Accounts "In its own window" and "Headless" modes run that
  way today.
* Milestone 0 is a measurement gate: whether enough of the memory kills Sentry shows had two or
  more accounts open for this to help.

## Why

### What Sentry shows (90 days to 2026-11-16)

| Issue | Events / users | What processes would change |
|---|---|---|
| TURU-ANDROID-B, "The GPU stopped working: Out of memory" | 92 / 18 | **Containment only.** The newest is a 12 GB POCO with 3 GB free, in the foreground: more GPU devices won't make that rarer. With processes the lost device takes down one client, and restarting that client (relogin, script) is the recovery `GPU_RECOVERY_PROPOSAL.md` builds by hand today. |
| TURU-ANDROID-A, "Android closed Turu to free memory" | 17 / 11 (2 of them Turu Mas) | **Only with two or more accounts open.** Android kills one process instead of the whole app. A single-account user gains nothing here (and pays for a shell process: see "Cost"). |
| TURU-ANDROID-C, "The system killed Turu (signal 9)" | 6 / 5 | As A, where the kill was for memory. |
| TURU-ANDROID-4 (ScreenOffCPUCheckKill), -D (o-kill:6, CPU in the background), -F (OneKeyClean, Turu Mas) | 165 / 26 | **Unknown.** Vendor managers may kill per process or per package. No claim until a phone shows which. |
| TURU-DESKTOP-6, GPU out of memory | 3 / 0 | Containment, as B. |

**The reports can't tell one account from several.** Exit reports (`CrashReports.kt`) carry the
reason, RSS/PSS at the end and the memory log (`android/src/memory.rs`: a reading a minute), but
not how many tabs were open, and memory alone doesn't tell either: one report read in full
(TURU-ANDROID-A, a Samsung A16 with 7.6 GB, beta.6, up 30 hours) logs 400-460 MB PSS over its
last 2.5 hours, but 200-690 MB swapped out besides (PSS leaves swap out), so anywhere from one
client to two. Milestone 0 adds the tab count.

### What a process gives that threads don't

A thread per tab (one player per thread, one process) would use more cores at the lowest memory,
but a panic (release builds abort), a lost GPU device or a low-memory kill would still end every
account. Only a process boundary contains them. Memory kills being the reported problem is why this
proposal picks processes over threads.

## Cost

Measured on a Mac (release build, all clients at the login screen, `footprint`; 2026-11-16):

| Run | Memory |
|---|---|
| A window with 1 tab | 279 MB |
| A window with 2 tabs | 316 MB (+37 MB for the second tab) |
| `--headless` alone (`NullRenderer`: no GPU) | 71-72 MB |

These numbers don't give the cost of the client this design needs. That client has its own GPU
device and draws Turu's UI, so it sits between the 72 MB headless process and the 279 MB window.
The window's own memory lines put 87 MB in video memory (24 MB of bitmaps, a 12 MB layer pool) and
60 MB in the Rust heap; the rest is the window, egui, fonts and the binary. In game, each
client adds a few hundred MB whichever design is used.

On Android each process also has its own Java runtime and its own Vulkan device (the driver keeps
memory per device). Neither is measured. **Milestone 1 measures them on phones**, and what's
affordable decides the slot count per phone (or whether to go on).

What a hidden client must give back, so that hidden clients cost less than shown ones: the layer
and cache texture pools and unused shapes (`memory::trim` already frees them on Android's
`onTrimMemory`; call it on hide), the surface, and its sound stream.

## Design

### Roles

```
                 shell (window or activity; no game)
                  │  launches, watches, restarts clients
                  │  routes input to the client on screen
                  │  answers Platform calls: file dialogs, open URL, share, install
   ┌──────────────┼───────────────┐
client 1        client 2        client 3      (a process each: player, bot, Turu's UI)
on screen:      hidden:         hidden:
draws to the    draws nothing,  draws nothing,
shell's surface ticks at the    ticks at the
                hidden rate     hidden rate
   └──── army files, Butler state files, control API on loopback ────┘
```

* **The client** is `turu_frontend`'s engine loop with one tab, the screen (generalised from
  `android/src/screen.rs` into `frontend/`: wgpu and egui only, no window type, so the rule "no
  winit, rfd or window in `frontend/`" holds) and a `Platform` that sends dialogs, URLs and the
  like to the shell. It draws the tab strip too, from a tab list the shell sends; tapping another
  tab asks the shell to switch.
* **The shell** draws only what shows when no client can: "starting", "this tab closed: Restart".
  On the desktop it's the `turu` binary in a shell mode (winit, rfd: in `app/`). On Android it's
  the activity's process, with no engine and no GPU device.
* **Messages** between them (input, show and hide, size and scale, the tab list, Platform
  calls and their answers, "detached") go over a local socket in one format on both platforms,
  so the Rust on each side is shared. Only Android's `Surface` goes over Binder.

### What's reused

* `army::Army::launch` and `Client` (`bot/src/army.rs`): starting `turu` with VibeSkua's flags,
  the password in `TURU_PASSWORD`, `--exit-with-parent`, a control API on a free port with its own
  token, a `/snapshot` poll every 2 s into the monitor hub. On the desktop a process tab is this
  plus a surface.
* `LaunchMode` (`bot/src/accounts.rs`): `Tab` becomes a process tab; `Window` and `Headless` are
  unchanged.
* Army sync (`bot/src/api/army.rs`: logs, `army/.<name>.lock`, the barrier, the room from the
  data folder's path) and Butler (`butler/<username>.state`) already work between processes.
* `Platform` (`frontend/src/platform.rs`): the seam for what the client asks of the shell.
* Android: `screen.rs`'s `attach` / `detach` (a new surface for the same player and renderer),
  `restart.rs` (a script and login back after a kill), `memory.rs`'s readings and `trim`, the crash
  and exit reports.

### Android

* **Slots, not spawning.** The manifest declares a fixed pool of foreground services, one per
  slot, each in its own process (`android:process=":c1"` ... `":c4"`, type `specialUse` as
  `TuruService`). A slot runs `start_engine` with one account, as the service already does without
  a window after a kill. Forking a binary is not an option: a forked process has no app
  context for JNI, the Keystore or a foreground service, and Android 12+ may kill an app's child
  processes.
* **The surface.** The activity hands its `Surface` to the selected slot over Binder; the slot
  attaches it (`ANativeWindow_fromSurface`, then `screen.rs`'s `attach`). Switching is: slot A
  detaches and **answers once it has stopped drawing**, only then is the surface given to slot B (a
  surface has one producer; drawing after the window went away is the TURU-ANDROID-9 kind of bug).
* **Input** comes to the activity (GameActivity, `input.rs`'s translation) and goes to the slot on
  screen as messages. The text field above the keyboard (`SurfaceField`) stays in the activity;
  its text goes to the slot.
* **Sound**: each slot has its own AAudio stream; only the one on screen is unmuted.
* **After a kill** a slot comes back on its own (`START_STICKY`) and reads its own record (account,
  server, script, options: `restart.rs`'s file, one per slot) to log in and start the script. The
  other slots never stopped. `android-tabs.json` is replaced by the slot records.
* **Which slot Android kills.** The slot on screen is bound by the activity (to raise its
  importance); hidden slots stay at a foreground service's. Under pressure Android should then kill
  a hidden slot first. To verify in milestone 1.
* **Notifications**: each slot's foreground service needs one; grouped under one summary, one line
  per account (as the notification has today). To verify: IDs across processes.
* **Tab Limit** stays, its default from the phone's memory (milestone 1's numbers).

To verify on phones (milestone 1), not assumed: starting a slot's foreground service while Turu is
in the background (Turu asks to be left out of battery optimisation, which Android lists as an
exemption), `START_STICKY` restarting a single slot process, which process the low-memory killer
picks among them, the memory of a slot process (Java runtime, Vulkan device) shown and hidden,
and file locks between processes on the shared-storage data folder (the army's locks and barrier
rely on them; today's Android tabs only use them within one process).

### Desktop

* The window process becomes the shell. A new tab starts a client process (`Army::launch` with a
  shell-mode flag naming the shell's socket); Accounts' "In a tab" does the same.
* **Getting the client's picture into the window.** Two ways, a decision below:
  * **Shared GPU textures everywhere**: the client renders into a texture the shell can open
    (macOS IOSurface, Windows DX12 shared handles, Linux dmabuf / Vulkan external memory) and
    the shell draws it in the tab's area. One design, three backends, through `wgpu-hal`
    (ruffle-aqw patches `wgpu-hal`).
  * **Native embedding where it exists**: Windows and X11 can put another process's window
    inside yours (VibeSkua's tabbed host does it); macOS and Wayland can't (docs/STATUS.md). That
    would mean two mechanisms.
  * Copying frames through shared memory is fine for bring-up only. The client draws Turu's UI
    too, so a frame is the window's physical size (a 2560x1600 window: 16 MB a frame), not the
    game's 958x550.
* **Grid View**: the shell draws each client's texture in its tile; the clients render at the
  tile's size.
* **Who owns what**: the shell runs the Army poll and the web monitor (one per machine, as today);
  Army Control in a client asks the shell's control API for the army. Settings are written by
  several processes, as already happens with "In its own window" clients: writes take a lock and
  read the file again first.
* A crashed client's tab shows "closed" with Restart, and restarts by itself with its login and
  script (auto-relogin's rules).

### Turu Mas

Not in this proposal. Turu Mas's player is the Adobe AIR runtime, not Ruffle: its tabs are several
AQW clients on **one AIR stage in one runtime** (docs/AIR_ARMY_SPIKE.md, docs/TURU_MAS.md), and
AIR draws into its own activity, so there is no surface to hand to another process. Two clients
cost about 1.4x one there because the runtime and shared assets are paid once; separate processes
would lose that. Whether a second AIR runtime can run in another process of the same app (a second
activity with `android:process`) is **unknown**, not known to be impossible: a separate spike if
Turu Mas's kills call for it. Milestone 0's tab count goes into Turu Mas's reports too.

## Milestones

Each ends with a check. Two tracks: **Android** (M0, M1 gate M3 and M4) and **desktop** (M2, M5),
which doesn't wait for the phone gates: its case is crash containment and cores, not phone
memory. M0 and M1 can run together; M2 can start at any time. M6 comes after both tracks.

### M0: know how many kills had several accounts (gate for M3)

Built 2026-11-16 (not yet released): `turu_frontend::tabs_log` writes `crash/tabs-<pid>.log`
from both Android apps' notification threads; `CrashReports.kt` adds it and the memory class to the
exit report and tags the Sentry event `tabs` / `tabs_most`; Turu's memory readings carry the tab
count. Search Sentry with `exit_reason:LOW_MEMORY tabs_most:[2,3,4]` (tags are text, so no `>`);
reports from earlier releases have no `tabs_most`.

* Android and Turu Mas: the memory log's readings and the exit report record how many tabs were
  open, which one was shown, and each one's state (login screen, in game, script running); the
  report adds the phone's memory class.
* **Done when** a release with it has sent enough low-memory and signal-9 exits to say what share
  had two or more accounts open (the owner sets how many is enough).
* **Gate**: if nearly all are single-account kills, the work that helps is lowering one client's
  memory, and this proposal waits. Otherwise go on.

### M1: two slots on phones (spike, gate for M3)

* Two slot services in their own processes on Android, each with one account and no UI changes:
  a debug switch moves the activity's `Surface` between them; input goes to the one shown.
* **Done when**, on a 4 GB and an 8 GB phone, these are written down: each slot's PSS shown and
  hidden (login screen, in game, farming) against one tab's; the activity process's PSS; the time
  to switch; `kill -9` of one slot leaving the other farming, and the killed one back in game with
  its script; which process Android kills when another app fills memory; background starts,
  `START_STICKY` per slot and file locks between the slots (the unverified points in "Android").
* **Gate**: the owner decides from the numbers whether the extra memory per account is worth it,
  and the slot count per memory class.

### M2: client mode and shell messages (shared, built on the desktop)

* `frontend/`: the screen from `android/src/screen.rs`, made to draw to any wgpu target; the
  message format and both ends; a client loop (one tab, the screen, Turu's UI); a `Platform`
  that forwards to the shell; the tab list.
* **Done when** `cargo test` covers the messages and a fake shell drives a client (input in,
  Platform calls out, show and hide); a desktop shell shows a client's picture by frame copy;
  **one backend (Metal, through IOSurface) shows a client's texture shared across processes**, the
  desktop's riskiest piece, before M5 relies on it; Android's single-tab build still works the same.

### M3: Android process tabs

* The slot pool in the manifest; slot services with grouped notifications and per-slot wake locks;
  the activity as the shell (surface hand-over with the detach answer, input, the text field,
  Platform calls: the file picker, sharing, installing updates); slot records and restart after a
  kill; Accounts, New Tab and Army Control opening and selecting slots; `memory::trim` on hide;
  Tab Limit from the phone's memory. The in-process tabs stay behind an Options switch until M4.
* **Done when** on the emulator: open, switch, close, two accounts in game, Butler following
  across slots, an army script with `bot.army.barrier` across two slots, one slot killed and back
  in game with its script while the other kept farming.

### M4: Android on phones

* **Done when** on real phones (a 4 GB one and a vendor with its own manager): two or three slots
  farm for an hour with the screen off; a low-memory kill of a hidden slot, the others going on; a
  GPU loss in one slot, the others going on; the native text field and the file picker through
  the shell. Then process tabs become the default and Sentry's A/B/C issues are tagged with the
  slot, to compare with M0.

### M5: desktop process tabs

* The window as the shell (per the frame decision), Grid View from client textures, the army and
  web monitor in the shell, settings writes under a lock, a closed client's tab with Restart and
  automatic restart.
* **Done when** on macOS, Windows and Linux: tabs open, switch and close as today; Grid View with
  three clients; a client killed (`kill -9`) shows as closed and comes back logged in while the
  others play; Army Control, Butler and an army script work across tabs.

### M6: one way of doing tabs

* Remove the in-process tabs (the thread-local swapping in `players.rs` goes with them, as
  each client has one player). Update docs/STATUS.md, docs/ANDROID.md, docs/UI.md,
  docs/HEADLESS.md and docs/GPU_RECOVERY_PROPOSAL.md (a lost device becomes "restart that client").
* **Done when** CI passes without the in-process path and the docs describe one model.

## Testing

* `cargo test`: the message format (round trips, unknown messages ignored), a fake shell
  driving a client, slot records and restart, the tab list after a client exits.
* Devices: the checks in each milestone's "Done when", with made-up account names in anything
  written down.

## Risks

* **Fewer accounts per phone.** Each process carries its own runtime and GPU device. M1 measures
  it; it may lower the Tab Limit on 4 GB phones.
* **Vendor managers** (Xiaomi, OPPO, Transsion) may kill the whole package, which no process split
  helps.
* **Restart storms.** A killed slot that comes straight back while memory is still short can push
  out another slot. Restarts need a policy (wait, back off, give up after a few, tell the user).
* **Two ways of doing tabs** live side by side from M3 to M6.
* **Text input across processes** (the keyboard, IME composition, the field above the keyboard)
  is the part most likely to feel worse.
* **Shared files written by several processes**: settings today; anything else that assumes one
  writer needs the same lock.
* **Desktop texture sharing** is per graphics backend, in `wgpu-hal`, which ruffle-aqw patches.

## Decisions to make

1. **M0's threshold**: what share of multi-account kills justifies going on.
2. **M1's threshold**: how much extra memory per account is acceptable, and the slot count per
   phone memory class.
3. **Desktop pictures**: shared GPU textures on every OS (recommended: one design), or native
   embedding on Windows and X11 plus textures on macOS and Wayland.
4. **A single account**: always a client process behind the shell (recommended: one model, and
   in the background Android can drop the shell's activity first), or the first tab in the
   shell's own process with only the others in processes.
5. **Settings ownership**: every process writes under a lock (recommended), or the shell owns
   settings and clients ask it.

## Not in this proposal

* Lowering one client's memory. That is what helps single-account kills, and M0 may show it
  matters more.
* Turu Mas (above).
* A thread per tab within one process (more cores, no containment).
