# Proposal: OpenGL that draws the whole game, then draws it faster

Status (2026-11-18): G0 to G4 built on branch `gl-renderer` (here and in ruffle-aqw) and checked
on the Android emulator; OpenGL ES is now Android's default. Not yet run on a real phone, on
desktop GL, or with G3's NVIDIA/AMD extensions. Before G0, OpenGL ES on Android drew only parts of the
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
levers, ranked by what G1 shows. **G4** makes OpenGL a fallback Turu can offer. **G5** lists what
else of Mugetsu's carries over, from a survey of its D3D12, Metal and wgpu-core changes.

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

- **Android:** since G4, `--graphics auto` (the default) tries GL, then Vulkan
  (`android/src/screen.rs`), and `--graphics gl` forces it. Before G4 it tried Vulkan first. Phones whose Vulkan driver crashes or runs out of memory (the
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

Battleon with 10 players on the emulator (GLES 3.0 over Metal, Apple M5), with `--perf-log 5`,
once the room had settled (14 perf-log periods after the start):

| | Vulkan | OpenGL ES, levers off | OpenGL ES, levers on (G2) |
|---|---|---|---|
| Pictures drawn | 29.0/s | 25.6/s | 27.4/s |
| Render per picture | 18.9 ms | 28.3 ms | 27.5 ms |
| `submit` per picture | 8.5 ms | 22.3 ms | 21.2 ms |

A first run measured while the room was still loading (shapes tessellating, 5-10 MB/s of
uploads) gave GL 5.6 pictures/s and a 100-150 ms submit. Don't compare numbers taken before a
room settles.

The counts below are from that first, loading run. Per draw they match the settled run (20 calls
per draw). GL replay, per second: 17.7 submits, 3,235 command buffers, 1,047 passes, 18,247 draws, and
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

Battleon on the emulator, A/B once the room had settled (about 81,500 draws/s in both runs):

| | `none` | `fbo,state` |
|---|---|---|
| GL calls | 1,692,000/s (20.7 per draw) | 917,000/s (11.2 per draw) |
| Calls skipped | 0 | 714,000/s |
| Time in `submit` | 605 ms/s | 569 ms/s (-6%) |
| Pictures drawn | 25.6/s | 27.4/s |

Calls halve, but time barely moves. The emulator's GLES encoder buffers calls and sends them in
batches, so a call it never sends saves little. The cost there is in uploads, flushes and the
context lock. A phone's driver validates each call itself, so the levers should be worth more
there. That needs a phone to show. An earlier A/B that reported 40 -> 22 µs per draw caught the
two runs at different load and is withdrawn.

G2's exit target, GL's frame CPU time within 1.5x of Vulkan's, holds on the emulator once
settled: 27.5 ms against 18.9 ms of render per picture, 1.45x. With the levers off it's 1.49x.
Calls per frame halved, which was the other target.

**GLES 3.1 (`-gpu swiftshader_indirect`: ANGLE on SwiftShader).** Phones take this path, and
the 3.0 emulator doesn't: vertex attributes go through `glVertexAttribFormat`/`glBindVertexBuffer`.
The levers cut calls per draw from 16.5 to 11.9. Once settled, the picture matched the levers-off
one; the first frames show AQW's white fade-in on joining. A fix that came out of reviewing this
path is in ruffle-aqw `1c83f96ce`: on 3.1+, `glVertexAttribPointer`/`Format`/`Divisor` also
change vertex buffer bindings, so the shadow drops what it knew of those bindings. SwiftShader
draws on the CPU, so it gives no timing. `base_vertex` (GLES 3.2, most phones) has still never
run on GL here.

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

## G5: More from Mugetsu (survey of 2026-11-19)

"How Mugetsu optimized Vulkan" above covers its Vulkan patch. This survey covers the rest of
Mugetsu 2.0.0: its D3D12 and Metal hal changes (`mugetsu/source/patches/wgpu-hal.patch`), its
wgpu-core changes (`wgpu-core.patch`), and the places in `ruffle/render/wgpu` that act by
backend. Nothing here is built. Every gain needs a real phone to measure: the emulator's GLES
sends every call to the host and buffers them, so it overstates call counts and hides flushes
and memory bandwidth.

### Candidates, best first

**G5a: Pipeline warm-up on GL** (Mugetsu's D3D12 warm-up). On D3D12, building a render
pipeline compiles its shaders with FXC, and Ruffle built ~90 the first time it drew: freezes of
up to 2.8 s entering Yulgar, and 50-570 ms per page blend mode seen for the first time.
`Descriptors::warm_up` (`render/wgpu/src/descriptors.rs:112`) builds them all on background
threads while AQW loads, but only on D3D12 (`descriptors.rs:119`: "Vulkan doesn't need this").
GL pays the same cost: a pipeline's first use translates its shaders to GLSL (naga), then
compiles and links them, which phone drivers are known to be slow at. So GL phones very likely
freeze as D3D12 did.

- **The catch:** the GL backend takes its program cache with `try_lock().expect(...)`
  (`patches/wgpu-hal/src/gles/device.rs:424`), so two threads building pipelines at once
  panic. Mugetsu's one thread per job would crash on GL: warm up on a single thread, or make
  that a plain `lock()` (an `AQW build:` change). Each compile also holds the GL context lock,
  which competes with the engine thread; acceptable during AQW's loading screen.
- **Size:** small. Android draws with one sample (Flash-style edges, no MSAA), so one set of
  pipelines per format.
- **To check:** whether Android keeps compiled programs between runs (its EGL blob cache, set
  up per app process) for slot processes. If it does, warm-up costs little after the first run.
- **Measure:** the first entry into a crowded room and the first skill effects, with and
  without; a "GL pipelines ready after N s" line as D3D12 logs.

**G5b: Keep the EGL context current on the engine thread.** Every lock of the GL context
makes it current and releases it (`gles/egl.rs:317`, `make_current`/`unmake_current`), and the
release flushes. G1 counted ~400 locks/s, `eglMakeCurrent` 20% inclusive of the engine thread on
the emulator. On a tiled phone GPU a flush in mid-frame can cost more than the call. The only
other thread that locks is wgpu-core's background drop of finished command buffers
(`drop_finished_encoders`, `patches/wgpu-core/src/device/life.rs`), itself a Mugetsu change.

- **Plan:** on GL, drop encoders inline (`WGPU_INLINE_ENCODER_DROP` already does it), so the
  engine thread is GL's only user. Then the lock keeps the context current, releasing it only
  when another thread waits or at a frame's end. This is the "every GL call on one thread"
  design that "Two GL-only costs from G1" above asks for.
- **First experiment, no code:** `--env WGPU_INLINE_ENCODER_DROP=1 --env RUFFLE_GL_STATS=1`, and
  check that the `GL replay` line's locks by thread show only the engine thread.
- **Risk:** a context current on one thread can't be made current on another (EGL_BAD_ACCESS),
  so a missed caller deadlocks or fails. Inventory every thread that touches the device first
  (surface configure, the encoder drop, anything on rayon's workers). Biggest potential of the
  three; needs a design of its own.

**G5c: Reuse command buffers' memory** (Mugetsu's D3D12 `recycle_ops`). Every GL command
buffer starts from empty `commands` and `data_bytes` (`end_encoding` takes them,
`gles/command.rs:274`), ~3,200 buffers/s each growing them again, and `reset_all` drops the
buffers it gets back. Keep a few emptied ones per encoder, within Mugetsu's limits: 4 per
encoder, 256 entries at most. Keeping more or bigger ones grew to gigabytes over a session on
D3D12. Small and low risk; a small gain.

Already planned above and waiting for a phone profile: G2c (one FBO per target), G2d (merging
command buffers, Mugetsu's D3D12 hook) and merging uploads.

### What doesn't carry over

| Mugetsu change | Backend | Why not on GL |
|---|---|---|
| Device-wide pool of command lists, created ahead on a background thread | D3D12 | GL has no command list object; a GL command buffer is a `Vec`. |
| One sampler index buffer per distinct sampler list | D3D12 | A GL bind group creates no GL object. |
| FXC blob copied out on the compiling thread | D3D12 | An FXC thread-safety bug. |
| `d3d12.dll` never unloaded | D3D12 | A D3D12 shutdown crash. |
| Texture-kind allocators, submits in batches of 512, lean pools | D3D12, Vulkan | See "What does nothing on OpenGL". |
| Present thread | Vulkan, D3D12 | Presenting on another thread needs the context there, against G5b. |

### Not from Mugetsu: worth a look on tiled phone GPUs

- **The stencil is stored after every pass and loaded in the next.** `stencil_attachment`
  (`render/wgpu/src/surface/target.rs:430-436`) always uses `StoreOp::Store` and, after the first
  pass, `LoadOp::Load`. That keeps masks across a target's passes, but on a tiler each such pass
  writes the stencil to memory and reads it back. `StoreOp::Discard` on a target's last pass
  would let GL skip the write: the GL backend already turns discarded attachments into
  `glInvalidateFramebuffer`.
- **Two full-screen copies per picture.** wgpu's EGL present blits the surface's texture to the
  window every frame (`gles/egl.rs:1155`), on top of Turu's own blit of the game into the
  surface (`android/src/screen.rs`). Vulkan has one. A 720p-to-1440p screen at 60-120 Hz pays
  that in fill rate and bandwidth.

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
