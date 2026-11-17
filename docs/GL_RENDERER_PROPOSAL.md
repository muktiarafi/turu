# Proposal: OpenGL that draws the whole game, then draws it faster

Status: G0 built (2026-11-18, below); G1 to G4 proposed, not built. Before G0, OpenGL ES on Android drew only parts of the
game (docs/ANDROID_HANDOVER.md, "Known gaps"), and on the desktop the app warns that OpenGL "is
much slower" (`app/src/gui/controller.rs:1018`). This proposal makes OpenGL correct first, then
cuts what it costs per render pass, as Mugetsu did for D3D12 and Vulkan.

## Summary

Mugetsu never touched OpenGL. Its wgpu-hal patch changes only `dx12/`, `vulkan/`,
`metal/adapter.rs`, `lib.rs` and `dynamic/`. ruffle-aqw's copy of wgpu-hal's GLES backend
(`ruffle/patches/wgpu-hal/src/gles/`) has no `AQW build:` or `Turu:` change at all. So
"optimize OpenGL like Mugetsu" can't mean porting code. It means using Mugetsu's method on the GL
backend: measure a crowded frame, find what each of Ruffle's thousands of render passes costs
there, and make each pass cheaper without changing what the GPU draws.

The milestone has four steps, in order. **G0** makes the picture complete. It blocks the rest,
because numbers measured on half a frame mean nothing. **G1** measures. **G2 and G3** are the
levers, ranked by what G1 shows. **G4** makes OpenGL a fallback Turu can offer.

## Where things stand

### What OpenGL already gets from Mugetsu

Most of Mugetsu's speed work is above the GPU API, so GL already has it:

- All of `ruffle/render/wgpu`: the shape arena (`arena.rs`), atlases and packing, batched blurs
  and glows (`filters/batch.rs`), cacheAsBitmap batching (`cache_batch.rs`), buffer pools, and
  tessellation spread over all cores.
- wgpu-core's backend-agnostic patches: no "Pre Pass" command buffer before every render pass
  (`patches/wgpu-core/src/command/render.rs:2723`), finished command buffers dropped on a
  background thread (`device/life.rs:438`), and the `merge_command_buffers` hook
  (`command/mod.rs:737`). GL doesn't implement the hook, so it does nothing there.

### What does nothing on OpenGL

| Mugetsu change | Backend | Why GL doesn't need it, or can't use it |
|---|---|---|
| D3D12 recorder (`dx12/record.rs`, `MERGES_COMMAND_BUFFERS = true`) | D3D12 | It replaced a real `ID3D12GraphicsCommandList` per pass (a `Reset` and `Close` in the driver, ~60% of encoding) with a recording. GL's backend already records: a command buffer is a `Vec<Command>` replayed in `Queue::submit`. There's no driver object per buffer to save. |
| Texture-kind allocators (`aqw_texture_kind`, 32 MB blocks for cacheAsBitmap pictures) | D3D12, Vulkan | GL has no app-side memory allocator. The driver places textures itself. |
| `vkQueueSubmit` in batches of 512 | Vulkan | AMD's Vulkan driver failed with out-of-memory on 3,000+ command buffers in one submit. GL has no such call. |
| `memory_budget()` | Metal, Vulkan | GL has no portable equivalent. See G3. |
| Lean command pools (`RUFFLE_LEAN_POOLS`, new in 2.0.0), with its pool counters and test switches | Vulkan | A pool never gives a command buffer back, and NVIDIA's driver holds ~28 KB per buffer, so a pool over the limit is destroyed and made anew once the fence's timeline semaphore says the GPU is done with it. A GL command buffer is a Rust `Vec`, taken out of the encoder at `end_encoding` and freed when wgpu-core drops it, so nothing piles up. |

### How Mugetsu optimized Vulkan, and what carries over

Mugetsu's Vulkan patch (`mugetsu/source/patches/wgpu-hal.patch`, `src/vulkan/`) has four
changes, compared here against 2.0.0. The `descriptor.rs` hunk only changes line endings.

1. **Submits in batches of 512 command buffers**, because AMD's driver ran out of memory on a
   crowded frame's ~3,400.
2. **Texture-kind allocators**: pooled layers and cacheAsBitmap pictures get gpu-allocator blocks
   of their own, so freeing the pooled ones empties whole blocks.
3. **Lean command pools**: see the table above.
4. **`memory_budget()`** from `VK_EXT_memory_budget`, so Ruffle's trimming knows how much memory
   it has.

Each of these fixes a driver object or memory the app manages, none of which GL has. So no code
ports. What does carry over is the method:

- **Count what the hal does and show it in the perf log.** The pools have
  `AQW_COMMAND_BUFFERS` and `aqw_pool_stats()` in wgpu-core, printed in the memory line. G1 adds
  GL's counters the same way.
- **Give every lever an env switch** for A/B runs (`RUFFLE_LEAN_POOLS`, `RUFFLE_POOL_*`). G2
  uses `RUFFLE_GL_LEVERS`.
- **Fix the cost where the frame pays it.** On Vulkan that was the driver's memory. On GL it's
  the replay's calls in `Queue::submit` (below).
- **A memory budget where the API has one.** G3 does this with `GL_NVX_gpu_memory_info` and
  `GL_ATI_meminfo`.

### What OpenGL costs per render pass today

A crowded room's first frame has ~1,700 render passes (Mugetsu's note in its Vulkan submit
patch), and wgpu-core gives each pass a command buffer of its own. On GL, `Queue::submit`
(`gles/queue.rs:1903`) replays each one on the thread that submits, holding the GL context
lock. It does this per command buffer, before the pass's own draws:

- `reset_state` (`queue.rs:88`): 10 GL calls (program, framebuffer, 6 disables, index buffer).
- A debug group push and pop, when the buffer has a label and the driver has debug functions.

It does this per render pass:

- `ResetFramebuffer` (`queue.rs:1107`) binds the one shared `draw_fbo`, then **detaches every
  color slot up to `max_color_attachments` plus depth-stencil**. That's 9 or more
  `glFramebufferTexture2D` calls, even though Ruffle binds one color attachment and sets
  `max_color_attachments = 4` (`render/wgpu/src/backend.rs:2073`). The driver then checks the
  FBO is complete again for every pass. On phones (Mali, Adreno), changing an FBO's attachments
  can also flush tiles.
- `BindAttachment` attaches the pass's target, `SetDrawColorBuffers`, then resets color, depth
  and stencil masks and three enables.

It does this per draw:

- On GLES 3.x without base-instance draws (most phones), the first instance is a uniform
  (`queue.rs:269`). Ruffle picks each draw's transform by instance index (`color.wgsl`), so
  nearly every draw sets that uniform. When the first instance changes, `prepare_draw`
  (`command.rs:210`) also rebinds vertex attributes for instance-stepped buffers.

On D3D12 this kind of cost was the driver's. On GL it's ours: wgpu-hal's replay makes the
calls, and it can skip the redundant ones. That replay is where the work goes.

### Measurements so far

The only numbers so far are from the Android emulator at the login screen (docs/ANDROID_PROPOSAL.md,
phase 2):

| | PSS | CPU | Picture |
|---|---|---|---|
| Vulkan | ~199 MB | 16-23% | complete |
| OpenGL ES | ~196 MB | 12-16% | only the logo's outline, no error logged |

GL's lower CPU proves nothing, because it drew almost nothing. There are no in-game GL numbers on any
platform.

### Where OpenGL is used

- **Android:** `--graphics auto` tries Vulkan, then GL (`android/src/screen.rs:122`), and
  `--graphics gl` forces it. Phones whose Vulkan driver crashes or runs out of memory (the
  Redmi 10's Mali-G52 in docs/GPU_RECOVERY_PROPOSAL.md) have nothing to fall back to while GL is
  broken.
- **Linux** without Vulkan, and **Windows** when neither Vulkan nor D3D12 works
  (`app/src/gui/controller.rs:797`).
- **Not macOS.** wgpu has no GL backend there, so nothing in this proposal can be checked on
  the main dev machine.

## G0: The whole picture

**Result (2026-11-18): the hypothesis held on the emulator, and the fix is in.** The Android
emulator's GLES is 3.0 (OpenGL ES Translator over Metal, Apple M5), so it has no `base_vertex`.
With `--graphics gl` and a restored session in Battleon:

- Without the guard: almost nothing drew, only the dialog and a few black skill-button shapes. No
  background, characters or UI plates.
- With the guard: the whole scene drew (town, characters, effects, portrait, chat, skill bar, Game
  Menu).

The fix is ruffle-aqw `ed14ee86c`: `Descriptors::base_vertex` is set from
`DownlevelFlags::BASE_VERTEX`, and `render_shape` takes the shared-buffer path only when it's set
(otherwise the per-slice branch). `RUFFLE_NO_BASE_VERTEX=1` forces the fallback, for testing on a
3.2 device. Turu bumped the submodule in `388001546`.

Still open for G0: one emulator and one room only. Not yet checked on a real Mali or Adreno phone,
`-gpu swiftshader_indirect`, a crowded room, or Linux Mesa. In the guarded run some Game Menu
button plates and a couple of skill slots looked missing or dim (they were fine in an earlier
guarded run, so possibly still loading; not investigated). The sections below are the original
plan.

**Logging (2026-11-18, ruffle-aqw `7e1c4b3ac`).** The `base_vertex draws` line was missing from
logcat because it was logged on `ruffle_render_wgpu`, which Android's filter holds at warn. It's
now a `Device:` line on `ruffle_aqw_perf` with the API version, the downlevel flags,
`base_vertex`, and the limits Ruffle depends on. Android also puts it in Turu's debug log
(Diagnostics > Logs) beside the `Graphics:` line. On the emulator it says `OpenGL ES 3.0 (4.1
Metal)`, `base_vertex draws: false`, a 65,536 B uniform binding (200 transforms per draw), 2D
textures up to 16,384, and 4 color attachments. A second guarded run in Battleon drew the whole
scene, with the Game Menu and every skill slot. `turu-args.txt` takes `--env NAME=value` for
Ruffle's switches.

**Leading hypothesis: draws from the shared shape buffer need `base_vertex`, and GLES 3.0/3.1
can't do it.** ruffle-aqw binds the shape arena's whole vertex and index buffers once, and starts
each draw at its own vertices with `base_vertex` (`render/wgpu/src/surface/commands.rs:404-429`,
an `AQW build:` change). GLES has `glDrawElementsInstancedBaseVertex` only from 3.2
(`gles/adapter.rs:402` sets `DownlevelFlags::BASE_VERTEX` for 3.2 and up only). Nothing checks
the flag on the way. ruffle-aqw doesn't look at it, and wgpu-core has no `BASE_VERTEX` check in
its draw path (`grep` of `patches/wgpu-core/src`), although the hal's comment at `queue.rs:285`
assumes it does. Shapes in the arena would then draw wrongly or not at all. Lines and bitmaps,
which bind their own buffers, would still draw, which would fit "only the logo's outline".

This is still a hypothesis. A missing GL function more often crashes than draws half a picture,
and the emulator's GLES may well be 3.2. So the first step is a test, not a fix:

1. **Log what the device is.** At startup, with the backend line that's already logged: GL
   version, `downlevel_flags`, and the limits Ruffle asks for (`max_uniform_buffer_binding_size`,
   which sets `transforms_per_draw`; `max_texture_dimension_2d`; `max_color_attachments`).
2. **Test with one switch.** Add an env var (`RUFFLE_NO_BASE_VERTEX=1`) that always takes the
   per-shape-slice branch (`commands.rs:431`). Run `--graphics gl` on the emulator. If the
   picture is complete, the hypothesis holds.
3. **Then fix:** keep `downlevel_flags.contains(BASE_VERTEX)` in `Descriptors` and use the arena
   path only when it's there. The fallback branch already exists.
4. **If it isn't that**, these are the other suspects, checked in this order:
   - Filter batching (`filters/batch.rs:280`): instanced draws with instance-stepped vertex
     attributes, which GL emulates by rebinding attributes at an offset (`command.rs:134`).
     There's no switch for it yet. Add one next to `set_cache_batch_enabled` (`lib.rs:401`),
     and try that one and `set_atlas_enabled` off too.
   - The uniform transform array: `transforms_per_draw` comes from the adapter's
     `max_uniform_buffer_binding_size` (`dynamic_transforms.rs:13`). A GLES driver that reports
     more than it can really bind would lose transforms past some index.
   - Atlas and cache textures bigger than the device's `max_texture_dimension_2d`, or in
     formats GLES can't render to.
   - Turn on wgpu's validation and GL debug output (`KHR_debug`) and read the log. Uncaptured
     wgpu errors may not reach Turu's log on Android.

**Done when:** on the Android emulator with `--graphics gl` (and with
`-gpu swiftshader_indirect` for a second GLES), the login screen, Battleon and a crowded room
look the same as on Vulkan in screenshots taken at the same moment. On Linux with Mesa
(llvmpipe, GL 4.5, or the Docker image with a virtual display), the same rooms look right too.
If desktop GL draws correctly and GLES doesn't, that's evidence for base vertex.

## G1: Measure

**Result (2026-11-18, ruffle-aqw `48a682ce8`).** Every state-setting call the replay makes now
goes through `gles::aqw::Replay`, which counts it by kind. With `RUFFLE_GL_STATS=1` (on
Android, `--env RUFFLE_GL_STATS=1`), one `GL replay` line every 5 s on `ruffle_aqw_perf`
gives:

- submits, command buffers, passes and draws per second;
- GL calls per second by kind;
- uploads;
- time in `submit`;
- context locks per second by thread.

Battleon with 10 players on the emulator (GLES 3.0 over Metal, Apple M5), with `--perf-log 5`:

| | Vulkan | OpenGL ES |
|---|---|---|
| Pictures drawn | 24/s | 5.6/s |
| Render per picture | 32 ms | 160-190 ms |
| `submit` per picture | 10.7 ms | 100-150 ms (520 ms of every second) |

GL replay, per second: 17.7 submits, 3,235 command buffers, 1,047 passes, 18,247 draws, and
**360,000 GL calls**:

| Kind | Calls/s | Note |
|---|---|---|
| vertex | 148,000 | ~8 per draw. On GLES 3.0 the first instance is emulated, so every draw re-specifies its instance-stepped attributes: bind `ARRAY_BUFFER`, enable, pointer, divisor, for each attribute. Only the pointer's offset changes. |
| state | 85,000 | Masks and enables reset per command buffer and per pass, then set again by the pipeline. |
| buffer | 22,600 | |
| texture | 21,100 | Unit, bind, and the two level parameters for every bind. |
| uniform | 18,200 | The emulated first instance, one per draw. |
| fbo | 15,900 | 5 calls to detach per pass, then attach. |
| copy | 11,000 | |
| program | 10,000 | |
| sampler | 9,600 | |

Other numbers from the same run:

- **Uploads:** 2,605 per second (5 MB/s), each a `glBufferSubData` from wgpu's CPU-side staging
  buffers.
- **Context locks:** 403 per second, all on the engine thread except 1 on `wgpu encoder drop`.
- **simpleperf** (`adb root`, `-e cpu-clock`) on the engine thread: 42% in the emulator's GL pipe
  (`writel`), with `glBufferSubData` at 28% and `eglMakeCurrent` at 20% inclusive. Each lock of
  the EGL context makes it current and releases it again, and the release flushes.

What the counters point to:

- **G2b first:** about two-thirds of the calls set state that's already set.
- **Then G2a:** 5 detaches per pass.
- **Two GL-only costs the plan didn't list:** the context lock's `eglMakeCurrent` pair, and
  per-write uploads. On the emulator every GL call crosses a pipe to the host, so it
  exaggerates call counts compared with a phone's driver. The ranking should still hold.

The sections below are the original plan.

Use the same tools the D3D12 and Vulkan work used, plus what GL needs:

- **The perf log on GL**: frame time, `finish_encoding`, and submit time. On GL the GL calls
  happen in `Queue::submit`, not while encoding. Look there first.
- **GL counters**, behind `RUFFLE_GL_STATS=1` in `gles/queue.rs`: command buffers per submit,
  render passes, draws, and GL calls by kind (framebuffer attach/detach, program, texture,
  sampler, buffer binds, uniform sets, state toggles). One line every 5 s, like the GPU timing
  line.
- **GPU time:** `RUFFLE_GPU_TIMING=1` works on desktop GL only. The backend enables timestamps
  only with `GL_ARB_timer_query` (`gles/adapter.rs:530`). On GLES, add `EXT_disjoint_timer_query`
  if the emulator or a phone has it. Otherwise use frame spacing and `adb shell dumpsys gfxinfo`.
- **Against Vulkan on the same machine**, in the same rooms: the login screen, Battleon, a
  crowded room with skill animations on, and a farm with a bot running.

**Done when:** STATUS.md has a "GL renderer" entry with the numbers for each room and backend
(emulator, Linux Mesa, and a Windows or Linux machine with a real GPU). It should also say
which of G2's levers the counters point to.

## G2: Cheaper passes (the GL counterpart of the D3D12 recorder)

**Result (2026-11-18, ruffle-aqw `12e89403c`): G2a and G2b are built and on by default.**
`gles::aqw::Replay` keeps a shadow of the state it sets during one submit:

- program and the emulated first instance;
- enables, masks, blend, stencil, depth, viewport and scissor;
- draw buffers and the draw framebuffer's binding;
- texture units, samplers and uniform buffer ranges;
- vertex attributes and buffers.

It skips any call whose value is already in place. Two things keep the shadow from going stale.
It starts empty at each submit, and the submit holds the context lock, so nothing else uses the
context meanwhile. It is also forgotten at every command it doesn't model (copies, resolves,
barriers, the sRGB clear workaround, per-draw-buffer blend). The draw framebuffer's attachments
and the textures' level parameters survive those commands, because only the replay's own
commands change them.

G2a: a pass detaches only what an earlier pass of the same submit attached and it doesn't. It
also skips attaching what's already attached. The first pass of each submit detaches everything,
as upstream does, so a texture deleted between submits can't come back under a reused name.

`RUFFLE_GL_LEVERS` picks the levers: `fbo` and `state`. Unset turns both on; `none` gives
upstream's replay.

Battleon on the emulator, A/B in the same session:

| | `none` | `fbo,state` |
|---|---|---|
| GL calls per draw | 20.1 | 10.7 |
| Calls skipped | 0 | 238,000/s (44%) |
| `submit` per draw | 40 µs | 22 µs |
| Pictures drawn | 3.4/s | 7.0/s |

The picture count also depends on how loaded the room was, so compare the per-draw rows. The
screenshot matched G0's.

Not built:

- **G2c (FBO cache).** One attach and a framebuffer check per pass remain (~4 fbo calls per
  pass). Wait for a Mali or Adreno profile before taking on the lifetime risk.
- **G2d (merging command buffers).** `reset_state` per command buffer is now mostly skipped
  calls.

Two GL-only costs from G1, also not built:

- **Keeping the EGL context current.** Each lock makes it current and releases it; G1 counted
  ~400 locks/s, nearly all on the engine thread. Leaving the context current is only safe if
  no other thread ever locks it. `wgpu encoder drop` does, about once a second. EGL can't make
  a context current on thread B while thread A holds it, and A can't be made to release while
  it's idle (the app in the background) without risking a deadlock. That needs a design of its
  own: perhaps every GL call on one thread.
- **Merging uploads.** There are 3,700 `glBufferSubData` per second, each from a write's own
  staging buffer. Concatenating adjacent writes to the same buffer would cut the calls but add
  copies. Profile on a phone first.

These levers are candidates, listed by expected value. Each is a change in ruffle-aqw's
`patches/wgpu-hal/src/gles/` marked `AQW build:`. Each sends the GPU the same draws in the same
order and only drops calls that change nothing. Each also gets a switch (`RUFFLE_GL_LEVERS`) for
A/B runs, as M7a of docs/HOST_SWF_PROPOSAL.md did.

- **G2a: Detach only what was attached.** `ResetFramebuffer` remembers which slots the last
  pass attached and clears only those. That's 1-2 calls instead of 9+. Small and safe, so it
  goes first.
- **G2b: Skip state that's already set.** Shadow the state each pass resets (masks, enables,
  draw buffers) and the bindings each draw sets (program, textures and samplers per unit,
  uniform buffer ranges, vertex buffers, the first-instance uniform). Skip a call when the value
  is already in place. `reset_state` writes into the same shadow, so the "default state at the
  start of each buffer" the encoder relies on stays true. Ruffle's own `bound` cache
  (`commands.rs:299`) already does this per pass. This does it across passes and inside the
  hal.
- **G2c: One FBO per target.** Keep a small cache of FBOs keyed by attachment set (texture,
  level, layer). Evict entries when a texture is destroyed, and limit the cache size. A pass
  then binds a framebuffer the driver has already validated instead of re-attaching. On tiled
  phone GPUs this is likely the biggest of the three, but it's also the riskiest (lifetimes, a
  driver keeping FBO state per context). Do it only if G1 shows attach calls or FBO checks in
  the profile after G2a.
- **G2d: Merge command buffers** (the hook Mugetsu added for D3D12). Concatenate one submit's
  buffers: shift each buffer's `data_bytes` ranges and query indices, and insert a reset command
  where a buffer started. On GL this saves only the per-buffer `reset_state` and debug group, and
  G2b makes those cheap. Do it only if G1 still shows per-buffer cost after G2b.

**Done when:** each lever that's kept has numbers from G1's rooms showing it pays (GL calls per
frame and submit time, on the emulator and on desktop GL), and the picture is unchanged in G0's
screenshot comparison. Exit targets, to confirm once G1 has numbers: GL calls per frame halved
in a crowded room, and GL's frame CPU time within 1.5x of Vulkan's on the same machine.

## G3: Memory on OpenGL

**Built (2026-11-18), not verified.** The GLES backend's `Adapter::memory_budget` reads
`GL_NVX_gpu_memory_info` (total and available) or `GL_ATI_meminfo`. ATI gives free memory only,
so the budget is what was free when the adapter was made, and the usage is how much less is
free now. `adapter_memory` asks GL on Linux after Vulkan, and on Android.

Android stays at `None` because GLES has no such extension; open question 2 is still the
user's to answer. It compiles for Android and Linux. It hasn't run on an NVIDIA or AMD desktop,
the only place it can return anything.

Ruffle's pools and trimming read the GPU's memory budget through `adapter_memory`
(`render/wgpu/src/backend.rs:90`). That only asks Vulkan (Linux) and Metal (macOS), so on GL it
gets `None`. It doesn't ask on Android either, even on Vulkan, because `target_os = "android"`
isn't handled.

- GL: `GL_NVX_gpu_memory_info` (NVIDIA) and `GL_ATI_meminfo` (AMD) on the desktop, read through
  a `memory_budget()` in `gles/adapter.rs`, as Mugetsu added for Metal and Vulkan.
- Android: GLES has no extension for this, so this needs a decision (see "Open questions").
  Answering with `None` keeps today's behaviour.

**Done when:** the perf log's memory line shows a budget on desktop GL with NVIDIA or AMD, and
Ruffle's trimming acts on it.

## G4: OpenGL as a real fallback

**Result (2026-11-18): on Android, OpenGL ES is now the default, by the user's decision.** This
goes further than the fallback planned below.

- `--graphics auto`, which is also used when no flag is given, tries GL first, then Vulkan.
- Options > Application > Graphics offers Automatic, OpenGL ES and Vulkan. The choice is saved
  as `graphics.txt` in the data folder, and each tab takes it at its next start.
  `--graphics` in `turu-args.txt` overrides it.

Checked on the emulator, with no `--graphics` flag:

- The `Graphics:` line says `gl`.
- Battleon draws completely at 7.8 pictures/s with the levers on.
- Home and back, and switching between two tabs (each slot has its own GL context), keep
  drawing, with no errors in logcat.
- `graphics.txt` with `vulkan` starts on Vulkan.

Not done:

- The "Vulkan crashed last time" marker is moot while GL is first. The reverse case, GL
  crashing at startup and falling back to Vulkan, isn't handled. A GL that fails to create
  falls through to Vulkan, but a GL crash after that point doesn't.
- The 30-minute farm.
- A real phone.

Only after G0 to G2:

- **Android:** if Vulkan crashed at startup last time, start on GL the next time (Mugetsu's
  "Vulkan crashed before" marker, which docs/ANDROID_PROPOSAL.md already plans). Options gets an
  Auto / Vulkan / OpenGL ES choice.
- **Desktop:** reword `aqw_warn_vulkan_fallback`'s note to match the G1/G2 numbers.

**Done when:** a phone or emulator with Vulkan off starts on GL by itself, and a 30-minute farm
on GL draws the whole picture with no errors in the log.

## Where the work lands

- `ruffle/` (ruffle-aqw, branch `aqw`): `patches/wgpu-hal/src/gles/` (G1 counters, G2, G3) and
  `render/wgpu/src` (G0's flag check and logging), committed there and marked `AQW build:` /
  `Turu:`, then the submodule bumped here.
- This repo: the env vars in docs/STATUS.md and docs/ANDROID.md, and G4's Android
  settings.
- Branches: `gl-g0`, `gl-g1`, ... in ruffle-aqw and here, each fast-forwarded once its
  "done when" holds.

## Testing

- **None of it runs on macOS.** Use the Android emulator (`-gpu host`, and
  `-gpu swiftshader_indirect` for a second GLES driver), Linux with Mesa (llvmpipe; the Docker
  image with Xvfb), and Windows or Linux with a real GPU for G1's desktop numbers.
- Screenshot comparisons against Vulkan for every lever, in G0's rooms.
- `cargo test -p turu_bot` and the app tests don't touch GL. Ruffle's own image tests could run
  under `WGPU_BACKEND=gl` on Linux to catch drawing changes. That's worth adding to CI if it
  stays fast.

## Risks

- **G0's cause is unknown.** If base vertex isn't the cause, G0 could take longer than G1 to G3
  together. The order of the suspects keeps each check cheap.
- **Shadowed state goes stale.** A missed invalidation (something outside wgpu touching GL state:
  egui-wgpu shares the device, and so does the surface's present blit) gives a wrong picture
  only on some drivers. `reset_state` at each buffer's start and screenshot comparisons limit
  it.
- **The FBO cache leaks or keeps a deleted texture alive.** Limit its size, evict on destroy,
  and keep it behind its switch.
- **wgpu updates.** The GLES patches add to the rebase work when ruffle-aqw moves to a new
  Ruffle or wgpu. Keep each lever small and marked.
- **The payoff may be small.** If G1 shows GL frames already close to Vulkan once the picture is
  complete, stop after G0 and G4.

## Open questions

1. Is GL worth more than a correct fallback? Who runs it: phones with bad Vulkan drivers, old
   Linux machines, or no one? Sentry's backend tags could answer this before G2.
2. Android memory budget for G3: use `ActivityManager.getMemoryInfo` through `java.rs` (system
   RAM, not GPU memory, but phones share it), or leave it at `None`?
3. Should the GL replay changes go upstream to wgpu (G2a and G2b are general) instead of staying
   ruffle-aqw patches?
