# Proposal: getting the picture back after a lost GPU device

Status: proposed, not built (2026-11-08). Today a lost device leaves the game running without a
picture until Turu restarts (docs/ANDROID.md, "A lost GPU device"; docs/STATUS.md, "A lost GPU
device (desktop)"). This proposal rebuilds the game on a new device and logs back in.

## Summary

When the GPU drops the device, Turu throws the dead game away and starts a fresh one: it copies
the login and the running script out of the old game (which still runs, only its drawing is
dead), stops the script, drops the player, creates a new GPU device and player, attaches the
window, and lets auto-relogin log back in and restart the script. The user sees a few seconds of
black screen and a fresh login, as after a disconnect. Android first; the desktop after, for all
its tabs at once.

Moving the running game onto a new device instead isn't practical (see "Why not keep the game
session").

## Why

The device is lost when the driver gives up: a reset (Windows' TDR after ~2 s without an answer
from the GPU), a driver crash, or the GPU's memory running out. A reset drops the device in every
program at once. A device can't come back, and everything the game drew with (shapes, bitmaps,
glyphs) lived on it.

What Sentry has shown so far:

| Issue | What |
|---|---|
| TURU-ANDROID-7, TURU-ANDROID-E | Redmi 10 2022 (Mali-G52, 4 GB), beta.6: the device was lost and Turu aborted (`Device::poll`, then reading an image back). Fixed in beta.7: Turu goes on without a picture. |
| TURU-ANDROID-B | The same Redmi, beta.8, 2026-11-08: lost 5.5 minutes after the start, in the foreground ("Unknown: Device is lost"), no out-of-memory warning before it. Turu survived, but the screen stayed frozen. The third loss on that phone. |
| TURU-DESKTOP-5 | Windows 11, beta.8: two clients in their own windows panicked in the same second when getting the window's next picture failed. A driver reset. Fixed after beta.8: the window stops drawing instead. |

Going on without a picture keeps a farming script alive, but on a phone in someone's hand, or a
desktop window someone watches, a frozen screen until a restart is a poor result, and a phone
like that Redmi hits it every few minutes.

## Why not keep the game session

Keeping the session means re-creating every GPU object the game holds on the new device. Ruffle
keeps them in at least seven places, each made lazily and cached:

| Where | What it holds |
|---|---|
| `ruffle/core/src/bitmap/bitmap_data.rs` | `BitmapData`'s texture (`bitmap_handle`), and `DirtyState::GpuModified` pixels that only the GPU has |
| `ruffle/core/src/drawing.rs` | the drawing API's shape (`render_handle`) |
| `ruffle/core/src/font/glyph.rs` | each glyph's shape |
| `ruffle/core/src/display_object/morph_shape.rs` | morph shapes and their blends |
| `ruffle/core/src/tessellation_cache.rs` | shared tessellations |
| cacheAsBitmap caches on display objects | their textures |
| ruffle-aqw's atlases and texture pools (`ruffle/render/wgpu`) | packed bitmaps, spare textures |

`Player` also has no way to swap its renderer. Even with every cache found, whatever the game
last drew on the GPU (skill icons made with `BitmapData.draw`, `GpuModified` bitmaps) is gone:
only the dead device had it. A missed cache means a handle from the old device used with the new
one, a validation error, and a panic. It would be a large change to ruffle-aqw, hard to keep
correct as Ruffle changes.

A new player avoids all of it: the library, its shapes, glyphs and bitmaps are the player's own,
so a new player starts with none.

## Design

### The flow

1. **Notice the loss.** As today: `ruffle_render_wgpu::device_lost()` turns `Some` (ruffle-aqw's
   device-lost callback). The engine (Android: `memory::tick`; desktop: `GuiController::render`)
   says so once, as now, then asks for a rebuild instead of only freezing.
2. **Save what the new game needs, from the old one.** The game still runs and answers calls:
   * the login: if `bot.servers.login_info` is empty (a login by hand), copy the game's
     `loginInfo.strUsername` and `loginInfo.strPassword` into it (`servers::set_login_info`).
     Memory only, never written to disk;
   * the server, as auto-relogin keeps it;
   * whether a script runs, and which (auto-relogin's `was_running` and `start_silent` already
     cover restarting the loaded script).
3. **Stop the script** and wait for it to end, as `servers::auto_relogin` does.
4. **Drop the old player and its screen.** Its textures and buffers belong to the dead device;
   dropping them there is fine (ruffle-aqw tolerates every GPU error once the device is lost).
5. **Reset the per-device and per-thread state:**
   * ruffle-aqw's `gpu_oom` lost flag (`LOST`), which today stays set for good: reset it when a
     new device's error handler is installed (`install_error_handler`), or key it by device;
   * the thread's `ruffle_core::PlayerThreadState` (deferred shapes, replay's change count,
     frame interpolation) and the renderer's command segments (`ruffle_render_wgpu::segments`):
     start the new player with fresh ones, as a new desktop tab does.
6. **Create a new device and player.** Android has the pieces: `screen::gpu` (device, blit,
   renderer), `build_player`, `attach_surface` (`android/src/lib.rs`, `screen.rs`). The new player
   loads skua.swf, which loads the game, as at start-up.
7. **Log back in and restart the script** through auto-relogin (`servers::auto_relogin`: stop,
   log in with `login_info`, rejoin, `script::start_silent`), started once the new game is at the
   login screen. The bot (`Arc<Bot>`) lives outside the player, so its options, logs, scheduler,
   and Butler state carry over.

### Changes by area

| Area | Change |
|---|---|
| ruffle-aqw (`ruffle/render/wgpu/src/lib.rs`, `gpu_oom`) | Reset the lost flag for a new device. |
| `frontend/src/engine.rs` | `run_loop_paced` holds one player for good. Let it return when a rebuild is due (an `EngineEvent` or a return value of `step`), so the caller can build a new player and loop again. |
| `android/src/lib.rs` | An outer loop in `engine`: build GPU and player, run, and on a rebuild drop them and go round again. Steps 2-3 before dropping, step 7 after. |
| `android/src/memory.rs` | On a loss, ask for a rebuild (within the limits below), else freeze as today. |
| `bot/src/api/servers.rs` | A way to run auto-relogin for a rebuild (it normally starts from a logout or disconnect event), and to save the game's login into `login_info`. |
| Desktop, later (`app/src/gui/controller.rs`, `frontend/src/turu.rs`, `app/src/player.rs`) | The device is shared by every tab, so every tab is rebuilt: the same steps per tab, then a new `Descriptors` for the window. Tabs already make and drop players on one thread. |

### Limits

* At most 2 rebuilds an hour. A device that dies again right after a rebuild (within 2 minutes)
  stops the rebuilds for this run. Past either limit, freeze as today, and say so.
* No rebuild while Turu is closing, or when nobody logged in (then only the screen comes back;
  the login screen needs no relogin).
* `--headless` draws nothing and needs none of this.

### What the user sees

A few seconds of black screen, then the game's loading screen, then the login and the script
restarting from the top, as after an auto-relogin. The debug log, the notification (Android) or
the title (desktop) say "Graphics restarted after the GPU stopped working". Sentry gets the loss
as today (`gpu: lost`) plus whether the rebuild worked.

## Testing

* Android: the emulator's `--test-gpu-loss` (`turu-args.txt`) destroys the device 20 s after the
  start. Log in, start a script, wait for the loss: the picture comes back, the login happens,
  the script runs again; a second `--test-gpu-loss` within 2 minutes leaves it frozen with the
  message. Check memory after a few rebuilds (`crash/memory-<pid>.log`): the old player must be
  freed.
* Desktop: `TURU_TEST_GPU_LOSS=1` does the same, with two tabs.
* Real devices: the Redmi 10 2022 that loses its device every few minutes.

## Risks

* **Something keeps the old player alive** (a future, a callback, the Flash host's channel),
  which keeps the old game's socket and memory: watch the memory log after rebuilds.
* **A rebuild loop on a phone whose driver keeps failing:** the limits above stop it; the cause
  stays the driver's.
* **A script that can't simply start again** (one counting progress in memory): the same as after
  an auto-relogin today, which VibeSkua's scripts are written for.
* **The password is read out of the game** for a login by hand. It stays in memory, where the game
  already keeps it, and is only used for this login.

## Decisions to make

1. Rebuild by default, or behind an option (Options > Application)?
2. Desktop in the same change, or after Android has run on the Redmi?
3. After the limits, freeze (scripts keep farming) or restart Turu fully?

## Meanwhile

On a phone that keeps losing its device, try Options > Application > Safe Drawing (Mugetsu's
drawing speed-ups off). If the losses stop, one of those speed-ups is the trigger, and it can be
turned off for that GPU alone (docs/ANDROID.md, "Misdrawing").
