# Proposal: a process for each tab (desktop and Android)

Status: being built (2026-11-17). M0 is built, M1's spike ran on the emulator, M2 is built, M3
(Android) and M5 (desktop) are built, M6 is built (one way of doing tabs: as-built section under
it); M4 (phones) is not started. Before M6, without the switch or `--shell`, every tab was
a player on one thread of one process, on the desktop and on Android. This proposal keeps the tabs as the user sees them, but runs each tab's game and bot in a process of its
own, so that Android killing one for memory, a crash or a lost GPU device ends one account instead
of all of them. Turu Mas is out of scope (see "Turu Mas").

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
  Tab Limit). Hidden slots' bindings waive priority (`BIND_WAIVE_PRIORITY`): done in M3 (see
  "M3 so far", "Which slot Android stops first"); whether Android then kills a hidden slot first
  under real memory pressure is M4's.

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
  the shown one. Done in M3: hidden slots' bindings waive priority (`Context.BIND_WAIVE_PRIORITY`)
  and only the slot on screen is bound important, so Android should pick a hidden slot first
  (adj 200 against 0 on the emulator; the low-memory run on a phone is M4's).
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
(M5: the shell owns them). Since done in M5: sound in the client, the shell drawing several
clients, `Tabs` / `SelectTab` / dialogs through the shell, the monitor and updates in the shell
(each client with a control API of its own), and shared textures on Windows and Linux (built, not run there; see "M5 so
far"). M1's results are on `spike/m1-slots`, M2's on `m2-client-mode`: this
file differs between the two until they're merged.

### M3: Android process tabs

* The slot pool in the manifest; slot services with grouped notifications and per-slot wake locks;
  the activity as the shell (surface hand-over with the detach answer, input, the text field,
  Platform calls: the file picker, sharing, installing updates); slot records and restart after a
  kill; Accounts, New Tab and Army Control opening and selecting slots; `memory::trim` on hide;
  Tab Limit from the phone's memory. The in-process tabs stay behind an Options switch until M4
  (on by default since 2026-11-17, ahead of M4: "On by default for the beta" below).
* **Done when** on the emulator: open, switch, close, two accounts in game, Butler following
  across slots, an army script with `bot.army.barrier` across two slots, one slot killed and back
  in game with its script while the other kept farming.

#### M3 so far (2026-11-16, branch `m3-android-tabs`, on M5 and the M1 spike)

Built: process tabs behind Options > Application > Process Tabs (experimental), off by default
then (on since 2026-11-17: "On by default for the beta" below) and read at start (saved as
`turu-process-tabs.txt` in the data folder). Turning it off stops the slots at the next start
(their records stay, so their tabs come back if it's turned on again). Checked on the emulator:
the switch, found from a slot's Options search, turned off removed the file (it now writes `off`),
and after a restart both slots had stopped and the app ran its engine itself.

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
  slot is muted (no window shows its game, so `lib.rs` mutes it as a backgrounded app; the slot
  given the surface is heard) and frees its spare textures, as a backgrounded app does today. Each slot gets
  Android's memory levels itself (`onTrimMemory` in `SlotService`; the activity's say nothing about
  the slots') and frees what it can spare.
* **Which slot Android stops first**: the shell binds every slot waiving priority
  (`BIND_WAIVE_PRIORITY`), so a hidden slot stays at its foreground service's importance (adj 200),
  and binds the one on screen important too (`BIND_AUTO_CREATE | BIND_IMPORTANT`; without
  `BIND_AUTO_CREATE` Android didn't count it), lifting it to the activity's (adj 0). Checked on the
  emulator (`dumpsys activity oom`): the shown slot at 0 and the hidden at 200, swapped by a
  switch; a trim level sent to a hidden slot freed its textures.
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
* **What only the activity does**, through the shell: a link (`OpenUrl`), the share sheet (`Share`),
  the "open" and "save" pickers (`PickFile`, `SaveFile`, answered with `DocumentPicked` /
  `DocumentCreated` to the slot that asked, which copies or writes the file itself), and an
  update's install (`Install`, answered with `Installed`; from any slot, so the monitor's can
  install while hidden). The clipboard a slot uses itself (`SlotService.copyText` / `clipText`:
  Android lets any of the app's processes while the app has the focus). Checked on the emulator:
  the web monitor's Open started the browser, Logs' Share opened the share sheet with the log, and
  Logs' Save wrote the file through the save picker. An update installed from a slot (host GPU,
  a debug 0.3.1 over 0.3.0, served from the Mac through `--releases-url`): slot 1, which runs the
  update checks, found it, its Menu offered "Update to 0.3.1", Download & install fetched and
  checked the APK in the slot and sent `Install` to the shell, whose activity installed it with no
  confirmation (Android 12+, an app updating itself); Android stopped every process, and the app
  opened again came back on 0.3.1 with both slots logged in and no update offered. The "open"
  picker: Script Loader's Load Script in slot 1 opened Android's picker through the shell, and the
  file picked was copied into `scripts/Imported/` and loaded in the slot. The clipboard: Copy of the
  script log in a slot put it on Android's clipboard (pasted into the field above the keyboard).
  Not run: the slot reading the clipboard itself (the game's Ctrl+V: a focused game field shows
  the native field, which pastes on its own), text a keyboard commits without keys (`adb input`
  sends keys only), and the soft keyboard for an egui box (none needs it). Once, after Ctrl+A and
  Delete sent as keys to that field, one Back closed it and sent the app to the background too;
  done again without them, Back only closed the field.
* **Crash reports from slots**: each slot's service starts Sentry in its process and reads
  Android's exit records, as the app's service does, and its Rust side reaches them (a panic's
  message, the GPU's warnings, Options' Send Crash Reports). The processes that start together
  read the exit records one at a time (a file lock in `crash/`, the last exit seen written before
  it's let go), so each exit is reported once, and a process starting keeps the memory and tabs
  logs of the app's processes still running. Every report is tagged `process` (`main`, `c1` ...
  `c4`), an exit's with the process that ended. The crash question is Turu's own dialog in a slot,
  and an answer reaches the other slots at their next start. Checked on the emulator (host GPU):
  SIGSEGV sent to slot 2 left a tombstone, slot 2 came back in 1.5 s and Sentry had one fatal event
  tagged `c2`; four earlier kills went once each, tagged with the slot that died, though the
  activity sent them; both live slots kept their memory logs; Options showed the switch in a slot.
* **Headless Mode across tabs** (the game option, every tab's at once as in VibeSkua's tabbed
  host): `turu::start_headless_mode_sync` reaches only its own process's tabs, so with process tabs
  it stayed the tab's that changed it (seen on a phone). A tab now sends a change to the shell
  (`ToShell::HeadlessMode`, from `client_link`'s status thread, each second); the shell keeps it
  and sends it to the other tabs and to a tab that connects (`ToClient::HeadlessMode`); a value
  got from the shell isn't sent back, and a tab's own at start isn't sent. The same messages in
  Android's shell (`slots::SlotShell`) and the desktop's (`app/src/shell.rs`). Checked on the
  emulator: on from slot 1 reached slot 3 within 3 s, off from slot 3 reached slot 1, and a tab
  opened afterwards started with it on. The desktop shell's side compiles; not run.
* **The notifications' summary**: the tabs' notifications (one per slot, IDs 101 to 104) have a
  group summary over them (ID 100), one line per tab and "n tabs". Any slot posts it again after
  its own notification changes, from the app's notifications up (the other slots', whichever
  process posted them, and its own as it stands now), so the slots share no state for it; a slot
  that ends takes its line out, and the last takes the summary. Checked on the emulator: three tabs
  gave a summary of three lines; closing one left "2 tabs" and two lines, shown by Android as one
  "Turu • 2 tabs" group.
* **Alerts from slots** (Notify Drop, a stopped script): each slot sets up `Alerts` in its process,
  numbering its notifications apart from the other slots' (from 100 000 times its slot), and posts
  while the app isn't on screen, which the shell tells every slot (`MSG_ON_SCREEN`, from the
  activity's `onResume` and `onPause`, and to a slot when it binds); without a shell, or once it
  dies, the app isn't. Checked on the emulator: a stop with the app in the background posted
  (ID 100000), with it on screen didn't, and after the shell's process was killed did (100001).
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

* **Keys outside the native fields**: the activity gets them (a hardware keyboard's, and
  GameActivity's text input), reads them with the device's key map, which a slot has no activity
  for, and sends them to the slot on screen (`AndroidKey`, `Text`); Back and the volume stay the
  system's. A slot's egui text box that isn't typed through the native field asks the shell for
  the soft keyboard (`SoftKeyboard`, only the slot on screen's is shown; a tab switch hides it).
  Checked on the emulator, in game: Enter as a hardware key opened the game's chat and its field
  above the keyboard, and Back then closed it and sent the app to the background, the slot
  running; with process tabs off, the same Enter still opened the field in process. Not run: a
  committed text (voice, an emoji), and the soft keyboard for an egui box (every Turu text box goes
  through the native field today).

Not done yet:
* **Tab Limit** from the phone's memory (with M4's numbers). Meanwhile 4 by default, up to the
  nine slots since 2026-11-22, raising it above 4 asking first with the phone's memory.
Accounts' "In a tab" in a slot, checked on the emulator (host GPU): Start on a saved account opened
slot 3 in a process of its own and showed it (the slot on screen let go in 6 ms); its record held
the login and server, no password; it logged in with Accounts' password and was in game 5.6 s
after Start, a third tab on the strip.

Army Control across slots, checked on the emulator (host GPU): each slot's lists both, the other
from its process (its script, map and HP); "Show its tab" from slot 1 showed slot 2 (slot 1 let go
in 5 ms); Start and Stop on slot 1's row in slot 2's Army Control started and stopped slot 1's
script.

Two accounts in game across slots, checked on the emulator (2026-11-16, Twig; scripts started
through each slot's control API):
* **Butler:** slot 2's account followed slot 1's (`/butler/follow`); slot 1 joined Yulgar and slot 2
  was there within 5 s.
* **Army barrier:** a script in both slots met at `bot.army.barrier` three times, one account
  arriving 5 s after the other: the first waited each time and both went on within 0.1-0.3 s of the
  second's arrival.
* **A hidden slot killed while both farmed** (`kill -9`, slot 2 on screen): slot 1's new process
  started within 1 s, was in game at 7 s, started its script again by itself at 9 s and was
  farming at 14 s; slot 2 kept its process and killed a monster every 2-5 s throughout.
* **The slot on screen killed while both farmed:** it came back with a new surface, logged in,
  started its script again and farmed 70 s after the kill (its login took 38 s, against 3.5 s for
  the hidden slot's; twice); slot 1 kept farming, more slowly alone in the room. The slow login is
  the shown slot's engine stalling, not the network: twice for 14-16 s, it sat in
  `vkAcquireNextImageKHR` (in the emulator's `vkAcquireImageANDROID`, waiting on its host GPU pipe:
  a thread dump during the stall), so the game loaded its login screen and the server list late,
  and the bot's calls waited. A slot started shown with no other running did the same (16.5 s);
  hidden, the login screen came 0.08 s after the game loaded, and in process it didn't stall.
  It was the emulator's software renderer: with the emulator on the host GPU (`-gpu host`), the
  same kill of the shown slot showed the login screen 0.04 s after the game loaded, was in game
  6 s after the kill and ran its script again at 7.6 s. For M4: time a shown slot's login on
  phones all the same (`--perf-log`'s Acquire line).
* Seen meanwhile: the monitor's slot often found the other slot's control API slow to answer
  (over its 3 s for a peer: `PEER_TIMEOUT`) while both farmed on the emulator's CPU renderer, and
  showed it offline for that poll; to read again on phones. The test script's own stalls (a hunt
  waiting in one cell, a join after a relogin) were the script's: both slots answered, stopped
  and moved at once.

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
steps (startup, the first pictures after textures were freed), not only between ticks. The same
busy engine made a drag in a slot in game miss scrolling on the emulator (its touches reached the
engine 0.3-1.3 s late, all in one frame); at the login screen, or in-process on a light scene, the
same drag scrolled: the CPU renderer's cost, not the slots'. Holding the
surface until a new slot has drawn was set aside (it shows a frozen tab rather than a black one).

### M4: Android on phones

* **Done when** on real phones (a 4 GB one and a vendor with its own manager): two or three slots
  farm for an hour with the screen off; a low-memory kill of a hidden slot, the others going on; a
  GPU loss in one slot, the others going on; the native text field and the file picker through
  the shell. Then process tabs become the default and Sentry's A/B/C issues are tagged with the
  slot, to compare with M0.

#### On by default for the beta (2026-11-17)

The owner turned Process Tabs on by default on every platform (Android, macOS, Windows, Linux) so
that the beta's users try them, ahead of M4's phone runs and of any build or run of desktop
process tabs on Windows or Linux. The label keeps "(experimental)".

* **The switch** (`turu_frontend::slots::switched_on`, read by the desktop's `shell.rs`, Android's
  `slots.rs` and `SlotShell.switchedOn` in Kotlin): on unless `turu-process-tabs.txt` says `off`;
  turning it off writes `off` (before, it removed the file). An `on` file from before stays on; a
  user who had turned it off (no file) gets it on again: the two can't be told apart.
* **The way back on the desktop**: `--in-process` starts the window with its tabs in its own
  process for that start, whatever the switch says, for when the shell or its clients don't
  start and Options (drawn by a client) can't be reached. Writing `off` into the file does the
  same for good. `--headless` and clients Army > Accounts launches stay in process, as before.
  Android has no such flag: Options > Application in a slot, or the file. Not built: turning the
  switch off by itself when slots keep dying at start.
* **Arriving from in-process tabs on Android**: the first start with process tabs on (no slot
  records yet) opens the tabs `android-tabs.json` lists in slots 2 to 4 after `turu-args.txt`'s
  first tab (`saved_tabs::tab_args`), as the app would have reopened them itself; before, they
  were left behind. A script the app was running for its first tab when Android last killed it
  isn't brought back by that start (the app's restart record isn't a slot's).
* **Options' description** now says it's on while it's tried out and how to go back (`--in-process`
  on the desktop), instead of the gaps fixed since (Android's file picker, sharing and updates
  through the shell; Map Travel and Jump to Player for the shell's other tabs).

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
* **Testing hook**: `TURU_SHELL_TEST=<file>` runs `wait`, `open`, `select`, `close`, `kill`, `click`, `fullscreen`,
  `title` and `dump` lines in the shell.

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

Then (2026-11-16, on `m3-android-tabs`): shared GPU surfaces, sound, the clipboard, device fonts
and file dialogs.

* **Pictures as IOSurfaces on macOS** (`app/src/shared_surface.rs`): the shell checks in a Mach
  service and names it to its clients (`TURU_SHELL_SURFACES`). A client draws the tab straight into
  one of three IOSurface-backed Metal textures (`client_screen::SharedSurfaces`, the pool
  `shell::SHARED_POOL`), sends each surface's Mach port to that service once with its process id
  and the surface's id, and after the GPU has finished a picture says which surface holds it
  (`ToShell::Surface`). There's no readback and no copy through the CPU. The shell opens the same
  surface as a texture of its own device and keeps the newest three of each client. It gives a
  surface back (`ToClient::Release`) once a newer one has come and its own GPU work with the older
  one is done (`on_submitted_work_done`). A surface whose port hasn't come yet (another channel)
  is drawn when it comes. A hidden tab's client drops its surfaces, and so does the shell. Without
  the variable, or if a surface can't be made or sent, the client sends pixels as before
  (Windows, Linux, `fake_shell.py`).
* **The shell draws on the GPU** in every case: pixel pictures are uploaded into a texture, and the
  window is drawn with `movie_texture::Blit`. Grid View's tiles are drawn as they are, and the
  active tab's picture over them with premultiplied alpha. The CPU compose is gone. The window takes
  any format the surface offers (BGRA, else RGBA, else the first), no longer only BGRA.
* **A click on another tile** is decided by the active tab's client (it knows where its windows
  are), which asks for that tab with `SelectTab`. Before, the shell read the picture's alpha on the
  CPU. `TURU_SHELL_TEST`'s `dump` reads the window back from the GPU.
* **Sound** in a client: its player has the window's audio (CPAL, Options' output device), at
  the window's volume and mute while it's the tab on screen (in Grid View, the active one), and
  muted while hidden or another tab's tile, as the window's in-process tabs are. `--headless` stays
  silent. Checked on the Mac: each client opened a CoreAudio output of its own (macOS's power
  assertions listed one, then two); which one is heard wasn't listened to.
* **The clipboard and device fonts** in a client: its game has a UI backend of its own
  (`backends::ClientUiBackend`) with the window's device fonts (`DeviceFonts`, taken out of
  `DesktopUiBackend`) and the system clipboard. Before, the null backend gave a client neither:
  AQW's device-font text (combat text, "AQW Recovered Helvetica Neue", Calibri) was missing, and
  the game's Ctrl/Cmd+C and V did nothing. Turu's text boxes get Cut, Copy and Paste as egui-winit
  makes them in a window (`client_screen::Clipboard`), and what egui copies goes to the clipboard.
  A link the game opens goes to the shell unless Options deny it: a client has no "Open website?"
  question to ask. Checked on the Mac (`TURU_SHELL_TEST`'s new `key cmd+V` line): a text on the
  clipboard pasted into Script Loader's Find box and into the game's Username field; Cmd+A and
  Cmd+C in the Find box put it back on the clipboard. The client loaded its device fonts (Arial
  for both names), with no "Fallback font not found". Found on the way, in the window too: the
  Find box lost the focus after its first character, because the count shown before it changed
  the box's egui id; it has a fixed id now.
* **File dialogs** in a client: `Platform::pick_file` / `save_file` send `ToShell::FileDialog`. The
  shell shows rfd's dialog over its window, one at a time, and answers with `ToClient::Answer`
  (a path, or none when cancelled or another dialog is open).

Checked on a Mac (Apple M5, debug build, login screen, no account): both clients said "Pictures go
to the shell as shared surfaces". Two tabs' dumps read back from the shell's GPU showed each
client's login screen and tab strip. After `kill -9` of tab 2 its new process handed over new
surfaces, and the dump showed it. The Scripts menu opened in a client after many pictures, so the
surfaces were given back and drawn into again. In Grid View two tiles showed side by side, each from
its own client's surface, the active outline on tab 2. A click on tab 1's tile made tab 1 active (the
dump showed it), and the toggle went back to one tab. Script Loader's Load Script in a client
opened the shell's "Load Script" dialog as a sheet of the window (a file wasn't picked: the answer's
way back isn't run). Tests (the new messages' round trips) and clippy pass. Android's and Turu Mas's
crates check. Resizing (`TURU_SHELL_TEST`'s `resize`, the window's size asked of macOS: no mouse
here, so AppKit's live resize while the corner is held wasn't run): the pictures followed each
size, from 900x560 to 1485x852 points, without a stretched or stale one left; a burst of 40 sizes
20 ms apart, five times each way in Grid View with two tabs, left no error, Grid View re-tiled at
each size, and the shell and clients' memory stayed flat (about 172 and 260 MB). Not run: an Intel
Mac (its surfaces are made `Managed`, Apple's chips `Shared`), Windows and Linux (below).

**Army > Accounts' processes of their own from a tab** (2026-11-17): "In its own window" and
"Without a window" used to start the clients from the tab's client, so they were its children:
only its Army Control had them, the shell's web monitor and Report To didn't list them, and they
ended when that client restarted. A tab now asks the shell (`ToShell::Launch`, the launches with
their passwords, as `OpenTab`), whose Army starts them: they outlive any tab's client, and the
shell's monitor hub (its Army's) lists them. The shell sends their list to every tab when it
changes and to a tab that connects (`ToClient::ArmyClients`: each one's control API and token, how
it ended): Army Control lists them (`Peer::ShellClient`; commands, Logs and Gear through their
control APIs), each tab's monitor hub polls them under their Army ids, and Accounts counts them as
running; Restart and Close go to the shell (`ToShell::ArmyClient`). Checked on the Mac (scratch
data folder, a made-up account "Without a window", no password): Start Selected in tab 1 made the
shell start it (its parent the shell's process, not the tab's client); tab 1's Army Control listed
it ("no window", login screen); after `kill -9` of tab 1's client it kept its process, and the
restarted tab's Army Control listed it again; Close there ended it and took its row away. Not
changed: Army Queue's launches from a tab are still the tab's client's (with Accounts' "In its own
window" or "Without a window"; with "In a tab" its squads open as tabs since 2026-11-21, below).

**`OpenTab` carries a playlist and a Script Options storage** (2026-11-17): a tab asked for with
them (`Startup`'s `playlist`, `run_playlist`, `options_storage`) now starts with
`--load-playlist`/`--run-playlist` and `--script-options-storage`
(`slots::playlist_args`), on the desktop and in Android's slots. Nothing asks for a tab with them
yet (Accounts, the strip's "+" and the saved tabs give the login, server and script; Army Queue,
which has a storage, launches processes), so it was a gap waiting for its first caller.

**Army Queue's squads in tabs** (2026-11-21): Army Queue is that first caller. With Accounts' "In a
tab" it opens each account with `OpenTab` and its storage, plus `exit_on_stop` (new: the tab gets
`--exit-on-script-stop`). Such a tab logs out once its script stops and sends `CloseTab` for itself
rather than quitting: the shell starts a tab whose client quit again, with its flags, so the script
would run once more. The desktop shell's only tab closing itself ends the window: that is
`turu --exit-on-script-stop` with a window (Accounts' "In its own window", which Army Queue waits
on), whose client used to quit and be restarted. `TabInfo` now carries each tab's login (`user`, from its `--user`, both
shells), which `army::queue::TabLauncher` follows: an account is done once its tab is gone; one
whose script never showed busy within the queue's timeout has its tab closed ("Didn't Start"). Unit
tests only (`bot/tests/army_queue.rs`, `slots.rs`); not run in the window with real accounts.

**Map Travel for the shell's other tabs** (2026-11-17): its jumps already went to every row, as
Army commands do, but it read a client's location under its Army id only, while a shell's other
tab is `tab:<n>` in the monitor hub: "Use current" for a checked shell tab found nothing, and the
other tabs' visits never reached Recent destinations. Each row now carries its hub id, and the
other tabs are polled while Map Travel or its pickers are open too, not only while Army Control
is drawn. Checked on the Mac with two accounts in game: with Map Travel open in tab 3 (in battleon),
tab 2 sent to Yulgar through its control API showed in tab 3's Recent destinations as
"yulgar-1 - Enter, Spawn".

**Turu's updates through the shell** (2026-11-17): the shell checks for updates (its clients,
started with `--exit-with-parent`, don't), but it has no UI, so what it found reached only the web
monitor and the log. It now sends its update state to every tab when it changes and to a tab that
connects (`ToClient::Updates`: the release, the check's time and error, the install's progress),
which the client shows as its own: the bar's Update button, the Update window, Options' last
check. A client's Check Now and Download & restart (or Update when I quit) go to the shell
(`ToShell::CheckUpdates`, `InstallUpdate`), which installs the update and starts again with its
arguments, its tabs with it; an update kept for when Turu quits is put in place when the shell
quits. Checked on the Mac against a local release server (`TURU_RELEASES_URL`, a made-up 9.9.9):
the shell found it, the client's bar showed "Update 9.9.9", its Download & restart made the
shell download the package and check it against `SHA256SUMS.txt`, and the install stopped where a
program outside `Turu.app` must ("update it by hand"), the client's Update window showing that.
Not run: an install that completes (it needs Turu run from `Turu.app`, or a Windows or Linux
build) and the restart onto the new version, which is the window's own, unchanged.

**Options > Application > Process Tabs (experimental)** on the desktop too (2026-11-17): saved
as `turu-process-tabs.txt` in the data folder, as on Android, and read at start: on, a plain start
of Turu's window runs as the shell (`shell::switched_on_for_this_start`); `--headless` and the
clients Army > Accounts launches (`--exit-with-parent`) stay as they are. The switch is in the
window's Options and in every client's (Options lives in the clients), so it's turned off where
it's on. Checked on the Mac (scratch data folder): with the file, a plain `turu` logged "Process
tabs are on: this window is the shell" and started its client, while `--headless` reached the
login screen and an `--exit-with-parent` start opened the window as before; the switch shown on in
a client's Options, turned off there, removed the file, and the next start was the window again.
`TURU_SHELL_TEST` takes `text <string>` (typed).

**Shared surfaces on Windows and Linux** (2026-11-17, `app/src/shared_gpu.rs`; written without
those machines, to be tried on them). The route follows the backend the client's GPU runs, which
on Windows may be Vulkan or DX12 (Turu's default is wgpu's primary backends):
* **Vulkan, Linux**: each surface is an image with exportable memory
  (`VK_KHR_external_memory_fd`, opaque fd, a dedicated allocation, device-local); the client sends
  the fd with the memory's size and type over the shell's Unix socket (`SCM_RIGHTS`, in
  `$XDG_RUNTIME_DIR`, mode 0600), and the shell makes the same image and imports the memory, with
  the exporter's size and type as the spec asks. The format is checked first for exportable and
  importable memory.
* **Vulkan, Windows**: the same with the memory as a Win32 handle (`VK_KHR_external_memory_win32`,
  opaque), its value in `ToShell::SharedHandle`; the shell copies it out of the client it started
  (`DuplicateHandle` on the child's process handle).
* **DX12, Windows**: a committed resource on a shared heap, simultaneous-access (each process
  tracks its states blind to the other's, and such a texture decays to common after each
  `ExecuteCommandLists`), its shared handle copied the same way and opened with
  `OpenSharedHandle`.
* **Both sides on one GPU**: the shell now picks its GPU as the clients do (Options' backends,
  high performance, through Ruffle's `request_adapter_and_device`), and each client says which it
  has (`ToShell::Gpu`). On another GPU or backend, on a surface the shell can't open, or with a
  handle it can't copy, the shell tells the client (`ToClient::NoSurfaces`) and it sends pixels.
  `TURU_NO_SHARED_SURFACES=1` for the shell starts with pixels.

Checked: the module type-checks and passes clippy for `x86_64-unknown-linux-gnu` and
`x86_64-pc-windows-msvc` against the patched wgpu-hal (alone, with stand-ins for the three
`turu_frontend` items it uses: the whole app doesn't cross-compile here, its C dependencies
want those systems' compilers); the shell's and client's Windows and Linux branches aren't
compiled here. On the Mac the fallback ran: `TURU_SHELL_TEST`'s `nosurfaces` made the shell refuse
the next surface, the client said "sending pixels" and its tab went on at 30 pictures a second,
drawn right; killed, its new process was back on IOSurfaces at 30.

To check on each machine (a debug build is enough), `turu --shell`, then open a second tab:
1. The log: `Shell graphics: <backend> on <GPU>`, `tab N's client draws on <the same>`, and
   `Pictures go to the shell as shared surfaces (vulkan-fd | vulkan-win32 | dx12)`, or why pixels
   (`no route for ...`, `the GPU can't share ...`, `surfaces can't be used (...)`).
2. The picture: the game and Turu's UI with right colours (blue sky, not orange), moving, no
   garbage or flicker; switch tabs, Grid View, resize, `kill` a client. Garbage rather than an
   error would be an image layout or resource state the other process doesn't expect: report it,
   and `TURU_NO_SHARED_SURFACES=1` meanwhile.
3. On Windows both routes: `turu --shell --graphics dx12` and `--graphics vulkan` (the clients get
   the flag too).
4. CPU against pixels: the tab on screen with and without `TURU_NO_SHARED_SURFACES=1`.

With accounts (2026-11-17, a Mac, debug build, two of the data folder's saved accounts on Twig,
`TURU_SHELL_TEST_FOLLOW` driving the running shell):
* **Army > Accounts' "In a tab" through the shell**: Start Selected in tab 1's client opened both
  accounts as tabs 2 and 3, each its own process, logged in with Accounts' passwords and in game.
  Their device-font text (names, chat) showed.
* **A client killed** (`kill -9` of tab 3, on screen): the shell showed tab 1, started tab 3 again
  after 1 s with its login, and it was back in game (its chat: battleon joined again) within
  seconds.
* **Butler across tabs**: tab 3 followed tab 2's account (`/butler/follow` on its control API); tab
  2 joined Yulgar, and tab 3 was there 7 s later (by way of a house, battleon-1 being full).
* **Army barrier across tabs**: a script in both met at `bot.army.barrier` three times, one
  arriving 5 s later; the first waited each time, and both went on within 0.2 s of the second's
  arrival.
* **Army Control** in tab 2 listed all three tabs, tab 3 from its process, with its script and map.
* **Switching**: a tab already drawn showed its first picture 125-150 ms after it was selected; an
  in-game tab shown for the first time took 1.6 s (its scene drawn the first time).

Release build, the same two accounts in game, against today's window with them as in-process tabs
(each process's CPU from its CPU time over 20-30 s; memory resident; `TURU_SHELL_TEST`'s `rate`):

| | Today's window | Process tabs (shell + 3 clients) |
|---|---|---|
| Seen: CPU | 167% | 143% (2 + 0 + 1 + 140) |
| Seen: memory | 1357 MB | 1097 MB (72 + 76 + 198 + 751) |
| Covered or minimized: CPU | 13% | 13% (0 + 1 + 2 + 10) |
| Pictures a second, tab on screen | 30 draws | 30 |

Grid View with the three tabs live: the active tab 30 pictures a second, the in-game tile 8 (the
tiles' rate), the login screen's tile 30 (not throttled at the login screen); 159%, 44% and 5%
CPU. Measuring found three things, fixed:
* **Hidden clients spun**: a hidden client's screen still asked to be called when its last UI
  repaint was due, already past, so its engine loop never slept: ~80% CPU each, at the login
  screen too. Now 1%. The same spin while a shown tab waits for the shell to give a surface back.
* **The UI alone drew up to 120 pictures a second** while egui animated (Accounts' spinner): the
  30-a-second cap was only on the next wake-up, not on drawing.
* **Covered**: today's window draws nothing while covered or minimized; a client drew on. The shell
  now tells its clients (`ToClient::Covered`, from winit's `Occluded` and a zero size), and they
  draw nothing meanwhile (the game, sound and pace go on). And the game had the mouse on its stage
  whenever its tab was shown, so every update hit-tested the whole stage (`run_mouse_pick`, most of
  a covered client's 57%): it has it now only while the pointer is over the window and it's seen,
  as the window's tabs do.

**Windows and Linux: gaps to fix** (found reading the code, 2026-11-17; 1 to 4 and the socket fixed
since; none built or run on Windows or Linux yet). The window (`app/src/gui/controller.rs`, `app/src/app.rs`,
`app/src/mugetsu.rs`) has safeguards the shell (`app/src/shell.rs`) and its clients
(`app/src/client.rs`) didn't use. In order of what they break:

1. **The GPU choice was cruder than the window's.** Fixed 2026-11-17 (built and run on the Mac;
   not built or run on Windows or Linux). The shell and its clients now choose as the window does:
   `select_wgpu_backend` (`app/src/gui/controller.rs`, now shared) tries Options' backends in
   turn, then the others (DX12, Metal, GL, the rest), with Options' power
   (`graphics_power_preference`), and both call `mugetsu::set_adapter_ids` (the client also
   `set_video_memory_adapter`, off Windows). The shell gives it its window as the display (GL on
   Linux needs it). A client has none: its instances have no display handle, so on Linux with GL
   it tries EGL without one and goes on to the other backends if that lists no GPU; if the chosen
   backend lists one but gives no device, the client asks the other backends for one ("Client: no
   GPU on …; trying the other backends") instead of failing. Both log the choice with the GPU's
   type (`Shell graphics:`, `Client graphics:`). Checked on the Mac: with `--shell` and with
   `--shell --power low`, both took Metal on the same GPU (the Mac has one, so the power choice
   isn't tested) and the client still sent shared surfaces (IOSurface); with
   `--shell --graphics gl`, both logged that OpenGL wasn't there, fell back to Metal and shared
   surfaces. The device fallback hasn't run.
   * Still to check on Linux: `turu --shell --graphics gl` opens and its tabs draw (pixels:
     there's no GL route), and which backend the client logs; `--power low` on a laptop with two
     GPUs logs the integrated one on both lines.

2. **Mugetsu's Vulkan-crash safety net wasn't armed.** Fixed 2026-11-17 (not built or run on
   Windows). The shell (`make_gpu`) and the client (`gpu`) call
   `mugetsu::vulkan_startup_begin(wanted.contains(VULKAN))` before choosing the GPU, and
   `mugetsu::frame_presented()` after each picture (the shell's present, the client's picture
   sink), which ends the window 10 s after the first. A hidden or covered client draws no
   pictures, so a timer ends its window 30 s after its GPU was set up at the latest (otherwise a
   driver crash hours later in a hidden tab would count as a Vulkan startup crash and move the PC
   to DX12). A client marks itself
   (`windows::set_client`): Windows' crash handler then shows no message box and doesn't start
   Turu again (nobody could answer it, and "start again" would start a stray client); after a
   Vulkan driver crash while starting it saves the marker and ends, and the client the shell
   starts next reads it in `mugetsu_args` (its arguments are the shell's, without `--graphics`
   unless the user gave it) and takes DX12. The shell keeps the window's message box and
   restart. The shell, already on Vulkan, isn't restarted: with the client on DX12 the adapter
   keys differ and that tab's pictures go as pixels (`ToShell::Gpu`) until the next start, which
   takes DX12 for both.
   * Check on Windows: `RUFFLE_TEST_NATIVE_CRASH=driver` (`windows::test_native_crash`, now also
     called in the client's `gpu`, not in the shell's) with `RUFFLE_TEST_DRIVER_FILE` naming the
     DLL and Vulkan chosen: the marker is written, no box shows, and the restarted client logs
     `Using DirectX 12: the Vulkan driver crashed on this PC before` and
     `Using preferred backend DirectX 12`, then crashes again before its `Client graphics:` line
     (the environment variable still asks; that crash isn't Vulkan's, so no box and no new
     marker), and the shell stops restarting it after five deaths.

3. **The graphics switch couldn't be reached in shell mode.** Fixed 2026-11-17 (not built or run
   on Windows or Linux). On Windows the shell's window gets the window's title-bar menu
   (`windows::add_graphics_menu`); choosing there restarts the shell as it restarts the window
   (`start_again`, then `exit`), and its clients quit when it goes. On both, a client's Options >
   Application has **Graphics** (Automatic, Vulkan, DirectX 12, and OpenGL on Linux;
   `Platform::graphics_choices`, offered only by a client: the window keeps the game's right-click
   menu). A choice goes to the shell (`ToShell::ChooseGraphics`), which asks over its window as
   it does file dialogs (without blocking its loop), saves it (`mugetsu::save_graphics`), leaves
   its loop, stops its tabs and starts again (`run`), every tab with it.
   * The rest of the game's right-click Mugetsu menu, which a client lacks, is in a client's
     Options > Application too (2026-11-17; `Platform::flash_edges`, `show_fps`, `full_screen`,
     `open_log_folder`, offered only by a client): **Edge Smoothing** (Flash-style or MSAA,
     applied to the tab at once and saved), **Show FPS** (the window's badge, now shared:
     `widgets::fps_badge`, counting the client's pictures of the game a second), **Full Screen**
     (`ToShell::FullScreen`: the shell's window borderless; Esc leaves it and still reaches the
     tab, as in the window; the tabs hear of each change, the system's own button included, by
     `ToClient::FullScreen`), and **Log Folder**. Frame Interpolation, already there, is now
     saved by a client too. Edges, Show FPS and interpolation reach the other tabs at their next
     start (each process reads them once). Windows' title-bar menu says "More settings:
     Options > Application" in shell mode. Checked on the Mac: with Show FPS saved on, a client
     drew the counter over its game; `fullscreen on` / `off` (`TURU_SHELL_TEST`) took the shell's
     window full screen and back, and the shell logged telling the tabs. The new Options rows
     weren't looked at on screen.
   * Check on Windows: the title-bar menu's DirectX 12 restarts the shell, which logs
     `Shell graphics: dx12`, and its first tab's `Client graphics: dx12`; the same from a tab's
     Options. On Linux: OpenGL from a tab's Options restarts both on GL (pixels).

4. **The shell's window lacked the window's touches.** Fixed 2026-11-17 (run on the Mac; not on
   Linux). The window's attributes are one function (`app::window_attributes`), used by both:
   Turu's icon (and the Dock's on macOS), the title with Mugetsu's notes (`mugetsu::window_title`,
   now also the shell's "<title> – <tab> (<n> tabs)"), and on Linux the app id and the
   startup-notification token. Its size is the window's too (2026-11-17; `app::window_size`,
   shared): turu.swf's stage under the bar, or `--width` / `--height` (Mugetsu's 958x584 off
   macOS), at least `app::MIN_WINDOW_SIZE` and no bigger than the screens, shown once sized;
   `--fullscreen` makes it full screen once shown (macOS ignores it on a hidden window). It used
   to open at a fixed 1180x640. Checked on the Mac with `TURU_SHELL_TEST`'s new `title` (the
   window's title and size, for the log): `--shell` gave "Turu 0.3.0 – Login" at 958x584 points,
   `--shell --width 1600` 800x494 points on a 2x screen, and `--shell --fullscreen` went full
   screen. Not run on Linux (the app id and token, Wayland's grouping) or Windows.

5. **Small ones.**
   * Linux: the shell's Unix socket for shared surfaces (`shared_gpu::listen`,
     `$XDG_RUNTIME_DIR/turu-shell-<pid>.sock`) is now removed when the shell's loop ends
     (`shell::run`). One left by a shell that crashed or was killed is removed by the next shell
     when it starts listening (`shell_sockets::remove_stale`, 2026-11-17): every
     `turu-shell-<pid>.sock` but its own that refuses a connection (a running shell's accepts;
     the connection ends at once, with no surface on it). Tested on the Mac (the module is built
     for tests on any Unix: a dead shell's socket, a reused pid's and a running listener's); not
     run on Linux.
   * Windows and Wayland report no `Occluded`: a covered shell window keeps its clients drawing
     (a minimized one on Windows is caught by its zero size). The window has the same limit, so it
     matches today.
   * The shell's and client's Windows and Linux branches (`ToShell::SharedHandle`'s handling,
     `listen_for_surfaces`, `shared_gpu::Sender::connect` in `client.rs`), and the fixes above
     (the client's crash handling in `windows.rs`, `choose_graphics`, the title-bar menu), aren't
     compiled on those systems yet: the first build there is their compile check (`shared_gpu.rs`
     alone is type-checked for both targets; a cross-check from a Mac stops at C dependencies:
     `jpegxr`, `rquickjs-sys`, `openssl-sys`).

Not done yet, of "done when": the clipboard without a display handle on Wayland (not tried); Windows and Linux, shared surfaces included (above: fixed but not built or run there). A down tab's monitor entry goes offline at the hub's next poll
of it (within 2 s: a client that doesn't answer is offline at once, not after 15 s;
`peers_are_polled_through_their_control_api`), and a monitor the shell reports to with Report To
drops it at the shell's next report (`Hub::record_report` keeps only the bots a report lists).

**The shell's own `--api`** (2026-11-17): it was ignored with a warning (each client had one, on
a port of the shell's choosing). The shell now serves the same API over its tabs
(`control_api::start_forwarding`, started in `shell::run`): the same token (`TURU_API_TOKEN` or
`api-token`), `/tabs` (the shell's tab numbers and labels), `/tabs/<id>/<route>` and the plain
routes for the first tab, each passed on to the tab's client's control API with that tab's
token, its answer streamed back (`/events` too). A tab whose client doesn't answer (starting
again) is `503 game_not_running`; the default route never falls back to another tab. A window
that became the shell through Options > Application used to start the in-process API on its
address, over a process with no game: `main.rs` now leaves `--api` to `shell::run` on both paths
(not in the clients, which see the same switch and serve their own). Checked with the switch
saved on and `--api` without `--shell`: the shell served it and its client its own.
A client's `/update` used to answer `not_supported` (a client leaves updates to the shell); it now
asks the shell, as Download & restart does, after its own check that no script runs. Tested in
`bot/tests/control_api.rs` (two real control APIs with tokens of their own and a dead port behind
it). Checked on the Mac with `--shell --api`: no token 401, `/tabs` listed both tabs, plain and
`/tabs/<id>` routes and `/events` answered, and a client killed by the test hook came back on a
new port with `/tabs/2/status` answering from it (its restart was quicker than the next request,
so the 503 wasn't seen there). Not run: `POST /update` through it (the client's new installer).

### M6: one way of doing tabs

* Remove the in-process tabs of the Ruffle frontends (the thread-local swapping in `players.rs`
  goes with them, as each client has one player). `turu_frontend`'s tab list (`turu::TABS`,
  `turu::add_tab`) stays: Turu Mas keeps several bots in one process through it
  (`air/ane/rust/src/clients.rs`). Update docs/STATUS.md, docs/ANDROID.md, docs/UI.md,
  docs/HEADLESS.md and docs/GPU_RECOVERY_PROPOSAL.md (a lost device becomes "restart that
  client").
* **Done when** CI passes without the in-process path and the docs describe one model.

#### M6 as built (2026-11-17, on `main`, before M4's phones and M5's Windows and Linux runs)

* **Desktop:** every start of Turu's window is the shell (`shell::runs_as_shell`), Army >
  Accounts' "In its own window" clients included; the switch and `--in-process` are gone (the
  flag is accepted and ignored). The window keeps one player, for a movie given on the command
  line (plain Ruffle), without tabs or Grid View of its own.
* **Clients first got what the window had:** the memory watch and Mugetsu 2.0.0's memory limit
  (`app/src/memory.rs`, every 5 s), Performance mode, Memory Limit and Free Memory Now in
  Options > Application, the window's player setup (default fonts, letterbox, align, quality),
  the least tessellation size by GPU, the GPU lost (the client ends and the shell starts it
  again; `TURU_TEST_GPU_LOSS=1`), the cursor's shape (`ToShell::Cursor`), the perf log and the
  GPU's release at close.
* **Android:** the activity is always the shell; the app's process runs no engine, and
  `TuruService` keeps only the statics every process calls. Each slot's notification has Pause
  or Resume, Stop and Start; slots write the tabs log; the Tab Limit counts the shell's tabs; the
  shell sends the display's refresh rate (`ToClient::RefreshRate`) and the slot on screen asks
  it for a display mode (`ToShell::PreferRefreshRate`).
* **`players.rs`** holds one player, without the thread-state swapping; ruffle-aqw dropped its
  two commits behind it (`PlayerThreadState`, `segments::swap`), so its history is closer to
  Mugetsu's. Windows' present thread follows Mugetsu's default again (clients never use it).
* **Not done:** frame interpolation's in-between pictures and uncapped pacing in clients (done
  since, 2026-11-18: STATUS.md, "Frame interpolation in a tab's client"), IME,
  gamepads, drag-and-drop, Flash's right-click items in a client (done since, 2026-11-18:
  STATUS.md, "Tabs' input"); TuruActivity's in-process
  fallbacks for the text field and documents (unreachable now) are still there (removed since,
  2026-11-18, with the natives and the Rust activity paths behind them). Checked on the
  Mac (shell, a client's GPU loss and restart, the memory limit in a client) and the emulator
  (shell and two slots start, the bindings checked in each process); not on Windows, Linux or
  phones.

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
  Built on Android 2026-11-22 (docs/ANDROID.md, "Tabs closed for memory"): a slot tells a kill for
  memory from other restarts by Android's exit record for its process, waits up to 20 s while the
  phone is low on memory before it logs in, and after more than 3 kills in 30 minutes comes back
  held, at the login screen with its script not started, saying why in its notification and tab
  row with Start again. Back-off between restarts stays Android's own (`START_STICKY`).
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
* Native windows. The client draws the whole tab, Turu's egui windows included.
  `docs/NATIVE_UI_PROPOSAL.md` (withdrawn on 2026-11-16, reopened in steps on 2026-11-20) plans
  native windows in the shell on top of this design: first the Android shell's tab strip and
  Accounts in Compose, the bot windows staying in the clients' egui until its checkpoint.
* A thread per tab within one process (more cores, no containment).
