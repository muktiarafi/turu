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
* **Decided 2026-11-16**: go on. The owner judged process tabs worth doing without waiting for
  the released reports; they still come in, to compare with M4 (tagged by slot).

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
* **Decided 2026-11-16**: the extra memory per account is acceptable (on the emulator's ~110 MB
  for two accounts). Still open: the slot count per memory class (from the phone runs, M4's
  Tab Limit), and whether hidden slots' bindings waive priority (`BIND_WAIVE_PRIORITY`) so that
  Android kills a hidden slot first.

#### M1 results so far (2026-11-16, emulator; phones open)

Built on branch `spike/m1-slots`: `SlotService.kt` (`Slot1Service` in `:c1`, `Slot2Service` in
`:c2`), `SlotShell.kt` (the activity as the shell: binds both, hands the surface over through a
`Messenger` and waits for the slot to let go, forwards touches), `android/src/slots.rs` (the slot's
natives), switched on by `turu-slots.txt`; each slot reads `turu-args-c<N>.txt` and skips the
restart, saved tabs, web monitor and update checks. adb controls are in `TuruActivity`
(`--ei slot N`, `--ei lock N --ei lockAction A`, `--ei restartSlot N --ei restartDelay S`).
`tools/slots/run.sh` runs the checks below on a phone with two accounts.

On the Android 16 arm64 emulator (one account in game, one at the login screen; debuggable APK;
Vulkan on llvmpipe, a CPU renderer, so "GPU" memory is in each process's RAM, unlike a phone):

| | PSS |
|---|---|
| Tabs, 1 account in game, shown | 359 MB |
| Tabs, the same in game but never drawn + a 2nd tab at the login screen, shown | 360-368 MB |
| Slots, the same states: shell + slot 1 (in game, never drawn) + slot 2 (login, shown) | 47 + 259-265 + 163-167 = **~475 MB** |
| Slots, other states: slot 1 in game once drawn, shown / hidden | 346 / 334-337 MB |
| Slots, other states: slot 2 at the login screen, never shown | 117 MB |
| Slots, app in the background: shell / slot 1 / slot 2 | 41 (+29 swap) / 334 / 116 (+58 swap) MB |

In the same states, two accounts cost about 365 MB as tabs and about 475 MB as slots: **about
110 MB more**, of which the shell is ~45 MB. Which client was drawn matters as much as that (an
in-game client is ~260 MB never drawn, ~340 MB once drawn), so only matched states compare. llvmpipe
keeps GPU memory in each process's RAM, so the phone runs decide.

Mechanics, all on the emulator:

* **Hand-over works**: 11 switches over two runs, 25-326 ms each; the slot drops its wgpu surface
  (swapchain) before answering, and neither slot logged "Couldn't set up the window" or "Couldn't
  draw in the new window" (a refused swapchain). Slot 2's login
  screen and slot 1's game showed in turn; touches reach the shown slot (a field took the focus,
  Menu opened). Keys and the soft keyboard aren't forwarded (spike).
* **A slot killed** (`kill -9`, the other slot on screen): back in 2 s (`START_STICKY`, restart
  scheduled after 1 s) and logged back in by its own args; the other slot kept its process.
* **Repeated kills back off**: 1 s, 4 s, 16 s (Android's own schedule): the restart policy can
  lean on it.
* **The shell killed**: both slots kept running. The first time, slot 2 was still drawing into the
  dead activity's surface and the emulator's GPU layer aborted (`Failed to unbox VkDescriptorSet`,
  in the emulator, not Turu; possibly caused by that, from one run each way). Each slot now watches the shell's Binder (`linkToDeath`) and lets
  the surface go; rerun: both let go, the emulator stayed up, and opening the app again showed slot
  1's game at once (same process, no relogin). Exit reports of slot processes show in the app's
  crash list too (one record list per package).
* **No shell at all**: `START_STICKY` alone restarts a killed slot (2 s), without any binding.
* **Background starts**: a slot's foreground service started 70 s after Home was allowed, with
  battery optimisation on and off alike; the reason read (exempt run) was `PROC_STATE_FGS`: the
  other slot's foreground service keeps the app's uid in that state.
* **Priority**: a hidden slot killed while the activity was in front died as `vis BTOP`: the
  shell's plain binding (`BIND_AUTO_CREATE`) lifts every slot while the app is on screen, not only
  the shown one. Before the phones' low-memory run, decide whether hidden slots' bindings should
  waive priority (`Context.BIND_WAIVE_PRIORITY`), so that Android picks a hidden slot first.
* **File locks between processes** on the shared-storage data folder (`File::try_lock`, as the
  army's): slot 1 held, slot 2 refused; slot 1 let go, slot 2 got it; and back. They work.
* **Notifications**: each slot's foreground service has its own (ids 101, 102), side by side.

Not tried: which process the low-memory killer picks (no memory pressure on the emulator: the
script's last step on a phone), phones' GPUs, a 4 GB phone.

### M2: client mode and shell messages (shared, built on the desktop)

* `frontend/`: the screen from `android/src/screen.rs`, made to draw to any wgpu target; the
  message format and both ends; a client loop (one tab, the screen, Turu's UI); a `Platform`
  that forwards to the shell; the tab list.
* **Done when** `cargo test` covers the messages and a fake shell drives a client (input in,
  Platform calls out, show and hide); a desktop shell shows a client's picture by frame copy;
  **one backend (Metal, through IOSurface) shows a client's texture shared across processes**, the
  desktop's riskiest piece, before M5 relies on it; Android's single-tab build still works the same.

#### M2 as built (2026-11-16, branch `m2-client-mode`)

Narrowed on one point: Android's `Screen` stays where it is (about ten Android hooks, and no
phone to check a move on); only its window-free pieces moved to `frontend/`
(`movie_texture.rs`: `Blit`, `MovieTexture`), and the client draws with a screen of its own
(`client_screen.rs`) that reuses them. Merging the two belongs with M3.

* **Messages** (`frontend/src/shell.rs`, no Ruffle, no window): `ToClient` (show, hide, pointer,
  wheel, key, text, focus, tabs, answer, dump) and `ToShell` (hello, frame, open URL, select tab),
  4-byte length plus JSON, a frame's BGRA pixels after its header, unknown types skipped; the
  client connects to the shell's loopback port with `TURU_SHELL_TOKEN`; `serve` hands messages to
  a `ClientEnd`; `FrameSender` sends on its own thread, frames newest-wins.
* **Client** (`turu --client=<address>`, `app/src/client.rs`): `--headless`'s loop and player
  (`headless::build_player`, now taking the renderer) with a wgpu renderer on a GPU set up without
  a window; `ClientScreen` draws the game and Turu's UI into a BGRA texture at the shell's size and
  scale, reads it back without making the engine wait (map, then look next step), and routes input
  as the desktop does (a press egui wants stays egui's until release, else the game's; keys go to
  egui while one of its text boxes has the focus). `Platform::open_url` goes to the shell; file
  dialogs aren't offered yet. The client quits when the shell's connection closes. No sound.
* **Shell** (`turu --shell`, `app/src/shell.rs`): a winit window that starts one client with its
  own flags, accepts it by token, copies each picture into the window's surface (`COPY_DST`) and
  sends the window's mouse, wheel, keys, text, focus and size.
* **Shared texture** (`app/examples/iosurface_share.rs`, macOS): the parent checks a unique name in
  with the bootstrap server, the child looks it up, writes a pattern into an IOSurface-backed
  Metal texture through wgpu (`wgpu::hal::metal::Device::texture_from_raw`, already in the patched
  `wgpu-hal`) and sends the surface's Mach port; the parent opens the same surface as a texture of
  its own wgpu device.

Checked against "done when" (debug builds, on a Mac, at the login screen, no account):

* `cargo test -p turu_frontend`: the messages round-trip, unknown ones are skipped, an oversized
  one is refused, a fake shell on loopback drives a client end (hello with the token, then its
  messages), and frames are newest-wins while messages aren't dropped (`OpenUrl` included); with
  `-p turu -p turu_frontend` and Ruffle's features, the client's key mapping too.
* A real client driven by `tools/shell/fake_shell.py` (no window, a scratch data folder, the login
  screen): ~29 pictures a second at 1280x720; a click opened the Scripts menu, a click on the
  game's username field and End and "xyz" typed into it, a new size and scale (1600x900 at 1.25)
  redrew at that size; `hide` stopped the pictures (one readback under way still went out) and
  `show` resumed them; the client quit when the shell closed. A URL opened from the UI wasn't
  triggered in a run: `ClientPlatform::open_url` queues `ToShell::OpenUrl` on the `FrameSender`
  that test covers.
* `turu --shell` on a Mac: the window started its client, which connected by token, and drew 600
  of its pictures in 21 s; the client quit with the shell.
* The shared texture: all 230 400 pixels the child's GPU wrote read back on the parent's GPU, no
  CPU copy, no change under `ruffle/`.
* Android's single-tab build on the emulator: in game, drawn through the moved `MovieTexture` and
  `Blit`, no panic. Turu Mas's crate checks (`cargo ndk check`). `--headless` and the normal
  window still reach the login screen after the `build_player` and `main.rs` changes.

Defined but not used yet: `Tabs` (the client draws no tab strip), `Answer` (no file dialogs through
the shell), `SelectTab`. Not done: sound in the client, the shell drawing more than one client, the
IOSurface route inside the shell (M5), Windows and Linux shared textures, the debug-only epaint
check Android's `Screen` would also hit (it ships release builds only). The shell and its client
both start the control API, web monitor and update checks, so they collide when one is turned on
(M5: the shell owns them). M1's results are on `spike/m1-slots`, M2's on `m2-client-mode`: this
file differs between the two until they're merged.

### M3: Android process tabs

* The slot pool in the manifest; slot services with grouped notifications and per-slot wake locks;
  the activity as the shell (surface hand-over with the detach answer, input, the text field,
  Platform calls: the file picker, sharing, installing updates); slot records and restart after a
  kill; Accounts, New Tab and Army Control opening and selecting slots; `memory::trim` on hide;
  Tab Limit from the phone's memory. The in-process tabs stay behind an Options switch until M4.
* **Done when** on the emulator: open, switch, close, two accounts in game, Butler following
  across slots, an army script with `bot.army.barrier` across two slots, one slot killed and back
  in game with its script while the other kept farming.

#### M3 so far (2026-11-16, branch `m3-android-tabs`, on M5 and the M1 spike)

Built: process tabs behind Options > Application > Process Tabs (experimental), off by default and
read at start (saved as `turu-process-tabs.txt` in the data folder, as Safe Drawing is). Turning it
off stops the slots at the next start (their records stay, so their tabs come back if it's turned
on again). Checked on the emulator: the switch, found from a slot's Options search, turned off removed
the file, and after a restart both slots had stopped and the app ran its engine itself.

* **Slot records** (`turu_frontend::slots`): `slots/c<N>.json`, a tab's startup flags, written when
  it opens and removed when it closes; never a password (`--password` in either form is taken out
  of `turu-args.txt`'s flags for the first tab), and never the app's own flags (`--graphics`,
  `--safe-drawing`, the logs'), which every slot reads from `turu-args.txt`. A slot logs in with
  Accounts' password, else the Keystore's copy the shell kept for that slot (`@slot:<N>`). A
  slot's script is brought
  back after a kill as the app's is (`restart.rs`, with its own `android-restart-c<N>.txt`).
* **The shell's logic** is plain Rust, tested on the desktop (`slots::SlotShell`): the tab list from
  the records and the slots bound, open (the first free slot of four), select, close (never the
  last), a dead slot listed as starting. `slot_shell.rs` runs it in the activity's process and
  answers each call with the activity's actions; `SlotShell.kt` carries them out (start, stop,
  bind, hand the surface over, pass messages on). The first tab comes from `turu-args.txt`, its
  `--password` moved to the Keystore.
* **Shell and slots talk in M2's messages** (`turu_frontend::shell`), carried by the Binder
  `Messenger` the surface already uses (not a socket: Binder tells each side when the other dies,
  and a slot needs no address to find a new shell). A slot says hello with its control API (a
  free port and its own token), so the strip, Army Control and the monitor hub of each slot see
  the others as on the desktop. The tab's side is `turu_frontend::client_link`, shared with the
  desktop's clients.
* **Slots**: four in the manifest (`:c1` ... `:c4`), each with its own notification (grouped) and a
  wake lock held only while its script runs; a closed tab's process ends with its service. A hidden
  slot is muted and frees its spare textures, as a backgrounded app does today.
* **The surface after a death**: a slot that dies while drawing stays the window's producer, and the
  next one's Vulkan surface on it is refused (`ERROR_NATIVE_WINDOW_IN_USE_KHR`, which wgpu panics
  on). The shell makes a new surface (its view hidden and shown) when that happens, and when it
  stops a slot that didn't let go.
* **The hand-over is asynchronous**: the shell asks the slot on screen to let go and returns; the
  slot's engine lets go when it gets to it (up to 30 s, waited for on a thread of the slot's own),
  and its answer moves the surface to the slot chosen last. Meanwhile the old slot keeps the
  screen, and nothing waits on the activity's main thread (only when the surface is destroyed or
  a slot is stopped, where it must be gone first). A slot that dies before answering counts as
  having let go; one that doesn't let go in 30 s keeps the screen, and the Rust side is told.
* **Typing in a slot**: the text fields above the keyboard stay the activity's (`SurfaceField`): a
  slot's game field or Turu text box that takes the focus sends `FieldShow` / `FieldUpdate` /
  `FieldHide` to the shell (only the slot on screen's are shown), and the field's edits, Send and
  closing go back as `FieldEdited` / `FieldEnter` / `FieldClosed`; the window's keyboard height and
  cutout go to every slot (`Insets`). Choosing another tab closes the field and takes the focus off
  the text box. Checked on the emulator: the game's Username field and Application Options'
  search box, typed in through the activity's field, closed with Back.
* **While no slot draws** (a tab starting, a slot coming back after a kill, a new surface on its
  way) a plain Android view over the game area says "Starting the tab…" with a spinner, instead of
  black: it shows until a slot whose engine is up (its hello carries its control API) has the
  surface. Checked on the emulator at the first tab's start and right after the slot on screen was
  killed; it went once the slot drew.
* **The app's once-only jobs** (decided 2026-11-16, after how VibeSkua does it: its tab host, a
  process without a game that's always there, downloads scripts; its Manager checks for client
  updates; skill sets are refreshed in every process, made safe by atomic writes). Android has no
  such process (the activity can go), so jobs are sorted by kind (`turu_bot::shared_job`):
  * **Launch updates** (scripts, skill sets): whichever process comes first, under a lock in
    `jobs/`, with the time they were done beside it; a shell's tab skips them when another did
    them in the last 10 minutes. On the desktop the shell does them (its clients leave them to it).
  * **The web monitor, Report To, client update checks and `monitor.json`**: one slot, the first to
    take `jobs/monitor.lock` (`turu::start_monitor_election`, tried every 3 s); the system lets the
    lock go when that process ends, and another slot takes it. That slot polls the others all along
    (their addresses from `slots/c<N>.api.json`, which each slot rewrites when it starts: the
    shell's list goes stale once the shell is gone) and follows Options changed in theirs
    (`turu::follow_settings_file`, the desktop shell's too). On the desktop the shell runs them.
  * A slot says hello again once its engine has set up its control API (a shell that bound earlier
    got one without it).

Checked on the emulator (Android 16, one account on Twig):
* **Starting:** the first start wrote slot 1's record without the password, and slot 1 logged in
  from it.
* **Opening a second tab** (adb, as the strip's "+") started slot 2 at the login screen, shown.
  Its strip listed both tabs from the shell's list.
* **Switching:** a tap on the strip's other tab switched to slot 1 in 65 ms, still in game.
* **Kills:**
  * Slot 2 killed while hidden: Android started it again within a second, and the shell bound it
    again.
  * Slot 1 killed while on screen and in game: the shell made a new surface, and slot 1 came back,
    took it and logged in again. The first try, before the new surface, crash-looped on "window in
    use".
  * The shell killed, then slot 1: slot 1 came back with no shell and logged in again; the app
    opened afterwards showed it in game.
  * The shell's process killed: both slots kept running and let its surface go. Opening the app
    again rebuilt both tabs from their records, with slot 1 still in game and no relogin.
* **Closing tab 2** stopped its service, removed its record and ended its process, and slot 1 was
  shown again.
* **Sound:** a hidden slot was muted, and its sound came back when it was shown again.
* **The switch off:** the app ran as before (one process, in-process tabs).

Not done yet:
* **Keys outside the native fields:** egui's own text boxes that aren't typed through the native
  field (GameActivity's text input) and hardware keys don't reach a slot.
* **The other Platform calls through the shell:** the file picker, sharing, links and installing
  updates do nothing in a slot.
* **Notifications:** a group summary.
* **`BIND_WAIVE_PRIORITY`** on hidden slots.
* **Memory and limits:** Android's memory levels to the slots, and Tab Limit from the phone's memory.
* **Runs not done yet:** Accounts' "In a tab" and Army Control, and two accounts in game with Butler
  and an army script across slots.

Checked: on the Mac, the desktop shell ran the launch updates and the monitor (both clients
listed) and its clients ran neither. On the emulator, slot 1 took the monitor's lock and ran the
monitor and updates, and slot 2 skipped them ("another tab updated the scripts lately"). With the
shell killed and then slot 1, slot 2 took the monitor over within seconds and listed slot 1
(restarted, in game) from its address file. An update found by the monitor's slot is installed
only from the activity. Also: the Keystore's key is made the first time a password is
kept; two processes doing that at once could each make one (the shell keeps the first tab's, so
it's made before the slots start). On the emulator, switching back one second after opening a tab
now works (slot 1 let go after 3.9 s, busy while slot 2 started, and kept the screen meanwhile), and
three switches 0.4 s apart ended on the last one. A slot shown again after its textures were freed
can take seconds to let go (9.7 s once on the emulator's CPU renderer): its first pictures back
rebuild them; to measure on phones. Another let-go took 14 s on the emulator, a slot at the
login screen while another started, both on the CPU renderer. For M4: read the "let go (… ms)"
lines on phones; if seconds are common there, have the engine look for the request during long
steps (startup, the first pictures after textures were freed), not only between ticks. Holding the
surface until a new slot has drawn was set aside (it shows a frozen tab rather than a black one).

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

#### M5 so far (2026-11-16, branch `m5-desktop-tabs`, on M2)

Built: the shell runs several clients as tabs.

* **The shell** (`app/src/shell.rs`) keeps a client process per tab, matched to its connection by
  process id: the tab on screen gets `Show` and the window's input, the others `Hide`; every
  client gets the list (`Tabs`: id, label, busy, why it's down); the title follows the tab on
  screen. A strip's click, "×" and "+" come back as `SelectTab`, `CloseTab` and `OpenTab` (Army >
  Accounts' "In a tab" too: its login goes in the message, the password on to the new client in
  `TURU_PASSWORD`). Each client reports its label and whether it's busy (`Status`).
* **A client that dies** is started again with its flags after 1, 4, 16, then 60 s; after 5 deaths
  in 10 minutes its tab stays "closed" while another tab runs (with none running it goes on trying
  every 60 s). When the tab on screen dies, a running one is shown instead. A click on a closed tab
  starts it again; a tab without a client isn't switched to (it takes no input) until it connects.
  A client that never connects is restarted after 60 s. The first tab gets the shell's
  `TURU_PASSWORD`.
* **The tab strip** (`gui/turu/tab_strip.rs`) shows the shell's list in a client (`turu::shell_tabs`,
  a down tab with its reason and the warning badge); the bar's height counts it
  (`turu::tab_strip_count`).
* **Testing hook**: `TURU_SHELL_TEST=<file>` runs `wait`, `open`, `select`, `close`, `kill`, `click`
  and `dump` lines in the shell.

Checked on a Mac (debug build, login screen, no account), through the hook with the window up:
a second tab opened from the shell and a third from the client strip's "+"; each client's strip
showed all tabs with the right one active; a click on the strip selected tab 1; `kill -9` of a
client restarted it after 1 s (4 s and 16 s for the next deaths) while the others ran, its tab
reading "Login (restarting)" meanwhile; the tab on screen killed: the shell showed tab 1; five
deaths left tab 2 "closed" while tab 1 ran, and a strip click on it started it again (shown once
connected); closing a tab removed it and, with one left, the strip; the first client had
`TURU_PASSWORD`; no client outlived the shell. Tests (with round trips of the new messages, and an
older tab list without `busy` and `down`) and clippy pass; Android's and Turu Mas's crates check,
and Android's in-process tab strip (refactored here) showed two tabs and switched on a tap on the
emulator.

Then (same branch): the monitor, Army Control, settings and Grid View across processes.

* **Each client has a control API** on a free local port with its own token (`--api`,
  `TURU_API_TOKEN`), as Army > Accounts' clients do; the shell passes on its flags but `--api`,
  `--monitor` and `--report-to` (`shell::client_args`). Before, every client got the shell's
  `--api` and all but the first failed to listen; and the shell's monitor showed one empty tab of
  its own and none of the clients.
* **The monitor's hub has peers** (`monitor::hub::PeerSource`): tabs in other processes, polled
  through `GET /snapshot` every 2 s by a "Turu Monitor Peers" thread and shown as tabs
  (`tab:<n>`, the shell's number); commands go to them over HTTP (`Control::Peer`). The shell's
  hub lists every client and has no tab of its own, so its web monitor and Report To cover all
  tabs, and keeps `monitor.json` (a client's hub keeps none). Each client's hub lists the other
  tabs while its Army Control is open (the tab list carries each one's API and token, and which
  tab the client is); otherwise only the shell polls, as every poll reads the client's game and its
  CPU "since the last poll". Options > Application is in the clients, which leave the monitor to
  the shell: the shell reads `settings.json` again every 2 s and applies a change to the monitor or
  Report To.
* **Army Control** in a client lists the shell's other tabs (`Peer::Shell`): their game from the
  hub, commands and Logs through their control APIs, Gear from the shared gear files, "Show its tab" through the shell; no
  Restart or Close (the shell supervises them). The hotkeys' "every client" commands reach them too.
* **Settings writes merge** (`Paths::merge_settings`): under a lock file, the file is read again
  and only the settings this process changed since it last read or wrote them are replaced, so
  two processes changing different settings keep both (`bot/tests/settings_merge.rs`). Changes
  another process makes are taken up at this one's next save, not at once.
* **Grid View**: the bar's Grid toggle in a client asks the shell (`ToShell::Grid`, with what
  Turu's UI takes of the window). The shell lays the tiles out as `grid::layout` does and sends each
  client its part (`Show` with `grid`): the active tab draws the whole window with its game in its
  tile and the rest transparent, plus the tiles' labels and outlines; the others draw only their
  game at their tile's size and run at the tiles' frame rate. The shell puts the tiles under the
  active tab's picture (on the CPU, as the pictures are still copies), and a press on another tile
  where the active picture is transparent shows that tab. For a frame or so after such a click, the
  window shows the tiles without the bar, until the newly active client's picture of the window comes.

Checked on a Mac (debug build, login screen, no account): with `--shell --api --monitor` and two
tabs, each client listened on its own port and the shell's monitor (`/api/state`) listed `tab:1`
and `tab:2`, online with their process readings; a log read and a Stop through the monitor reached
tab 2; after a `kill -9` it came back on a new port and the monitor followed. Army Control in tab 1
listed both tabs, the other one as "login screen" from its process. Grid View: the toggle tiled
two tabs side by side and three in 2x2 with labels and the active outline; a click on tab 2's tile
made it active (strip and outline followed); the toggle went back to one tab. With three tiles: the
active tab killed, the shell made tab 2 active and tab 1 came back as a tile; a tile clicked became
active; closing a tile re-tiled the other two; closing down to one tab left one tab without the strip
or the toggle. Monitor settings written to `settings.json` (as a client's Options would) started the
web monitor in the shell within 2 s. Tests: the hub
polling a peer through a real control API, settings merged from two writers, the flags clients
get, the new messages.

Not done yet, of "done when": the clients Army > Accounts launches "In its own window" from a tab
stay that tab's client's (its Army Control has them), and the shell's monitor and Report To don't
list them, and they end when that client restarts; the IOSurface route in the shell (pictures are copied, and Grid
View composes them on the CPU: fine for a few tiles, slow in debug builds); a tab coming back
logged in after a kill, and Army Control, Butler and an army script across tabs while in game (no
account here: Butler and `bot.army` already work across processes through files, as with Army >
Accounts' clients); Windows and Linux. Not built: Map Travel and Jump to Player in Army Control
for the shell's tabs (they read in-process tabs and Army clients only); the shell's own `--api`
(ignored with a warning: each client has one). Built but not run: Army > Accounts' "In a tab"
through the shell. `OpenTab` doesn't carry a login's playlist or options storage yet. A client's
Options > Application > Check Now for updates is off (`--exit-with-parent` marks it launched; the
shell checks). A down tab keeps its last monitor entry online for up to 15 s.

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

1. ~~**M0's threshold**~~: decided 2026-11-16, go on.
2. **M1's threshold**: the extra memory per account is acceptable (decided 2026-11-16); the slot
   count per phone memory class is still open.
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
