# Mugetsu ruffle.patch analysis
Source: /Users/aryamuktiaraafi/projects/mugetsu/source/patches/ruffle.patch (22,948 lines, 110 files, vs Ruffle fda41800, Ruffle 0.7.0-nightly.2026.10.2). Mugetsu VERSION = "1.6.0".

KEY FINDING: this is a GAME-PLAYER + PERFORMANCE fork. It contains NO ExternalInterface / fscommand / JS-bridge / socket changes. Most of the patch (core, render) is perf work (shape caching, atlas, batching, GC of library, interpolation replay).

## 1. Files touched (+added/-removed; computed from diff lines)
### Workspace root
| File | +/- | Purpose |
|---|---|---|
| Cargo.toml | +8/0 | `[patch.crates-io]` to ./patches/* (see sec. 4) |
| Cargo.lock | +32/-10 | adds cty, libmimalloc-sys, mimalloc; drops registry source/checksum for patched crates; new deps on desktop/core/render |
### core (ruffle_core)
| File | +/- | Purpose |
|---|---|---|
| core/src/aqw_bench.rs | +193 | NEW load benchmark test (env AQW_CORPUS, AQW_REPEAT) |
| core/src/aqw_retention.rs | +101 | NEW heap census / retention-path debug of GC objects |
| core/src/avm2.rs | +76 | AVM2 changes (SWF-version gate: `show_all_apis()` exposes newer APIs; perf) |
| core/src/avm2/amf.rs | +1/-1 | minor |
| core/src/avm2/class.rs | +4 | perf/minor |
| core/src/avm2/events.rs | +84/-10 | event dispatch perf |
| core/src/avm2/function.rs | +4 | minor |
| core/src/avm2/globals/avmplus.rs | +2/-2 | minor |
| core/src/avm2/globals/flash/display/display_object.rs | +76 | AS3 DisplayObject property setters instrumented with `aqw_trace` (target ruffle_aqw_trace), likely perf/dirty flags |
| core/src/avm2/globals/flash/events/event_dispatcher.rs | +8/-2 | dispatch perf |
| core/src/avm2/globals/flash/geom/transform.rs | +12 | perf |
| core/src/avm2/globals/flash/utils/Dictionary.as | +2/-1 | Dictionary tweak |
| core/src/avm2/globals/flash/utils/dictionary.rs | +16 | Dictionary perf |
| core/src/avm2/object.rs | +6/-3 | registers aqw_inspect module |
| core/src/avm2/object/aqw_inspect.rs | +68 | NEW object describe() for retention debug (lists e.g. "SocketObjectData" type name only) |
| core/src/avm2/object/dictionary_object.rs | +89/-4 | Dictionary perf |
| core/src/avm2/object/function_object.rs | +5 | `aqw_shared` etc |
| core/src/avm2/object/loaderinfo_object.rs | +1 | minor |
| core/src/avm2/object/script_object.rs | +68 | perf |
| core/src/avm2/object/soundchannel_object.rs | +29/-9 | sound channel fix/perf |
| core/src/avm2/property_map.rs | +135/-1 | property lookup perf |
| core/src/avm2/script.rs | +4 | minor |
| core/src/avm2/vtable.rs | +172/-18 | vtable lookup perf |
| core/src/bitmap/operations.rs | +1 | minor |
| core/src/context.rs | +14/-1 | adds `aqw_hovered_chain`, `aqw_rollout_chain` to mouse data |
| core/src/debug_ui/avm2.rs | +1/-1 | minor |
| core/src/display_object.rs | +469/-21 | cache-as-bitmap placement/oversize logging, cache step sizing, perf |
| core/src/display_object/avm1_button.rs | +5 | `aqw_shared` |
| core/src/display_object/avm2_button.rs | +9 | `aqw_shared` |
| core/src/display_object/container.rs | +1 | minor |
| core/src/display_object/edit_text.rs | +64 | text perf/shared |
| core/src/display_object/graphic.rs | +959/-57 | shape render caching / shared shape handles / deferred shapes |
| core/src/display_object/interactive.rs | +64 | hit-test/rollout chain |
| core/src/display_object/interp.rs | +423 | NEW frame interpolation (frame_interpolation) |
| core/src/display_object/morph_shape.rs | +123/-16 | morph shape caching |
| core/src/display_object/movie_clip.rs | +139/-3 | perf |
| core/src/display_object/replay.rs | +699 | NEW interpolation replay (recorded draws) |
| core/src/display_object/stage.rs | +4 | quality log |
| core/src/display_object/text.rs | +5 | `aqw_shared` |
| core/src/drawing.rs | +64/-10 | drawing API perf |
| core/src/font/glyph.rs | +65/-7 | glyph caching |
| core/src/frame_lifecycle.rs | +2 | minor |
| core/src/html/layout.rs | +30 | text layout perf |
| core/src/input.rs | +15/-1 | input handling |
| core/src/lib.rs | +43 | pub `set_show_all_apis`, `show_all_apis`, `set_shape_wait`, `set_cache_size_steps`, `frame_interpolation` mod |
| core/src/library.rs | +282/-13 | library GC of unused characters (backoff), freed-movie tracking |
| core/src/loader.rs | +10 | logs "loaded SWF: {url}" (ruffle_aqw_perf) |
| core/src/player.rs | +449/-9 | memory report, GC census, hover/press pick logging, rollout chain, `aqw_memory_report()` |
| core/src/tessellation_cache.rs | +294/-16 | shape tessellation cache |
### render (ruffle_render + ruffle_render_wgpu)
| File | +/- | Purpose |
|---|---|---|
| render/Cargo.toml | +1 | dep |
| render/src/backend.rs | +87 | new RenderBackend trait methods (trim memory, atlas, etc.) |
| render/src/commands.rs | +39/-25 | command list changes |
| render/src/filters.rs | +9 | filter batch support |
| render/src/tessellator.rs | +1239/-13 | parallel tessellation + `set_flash_aa` "Positive" fill-rule Flash-style edge AA (uses patched lyon) |
| render/src/transform.rs | +25/-11 | transform tweaks |
| render/wgpu/Cargo.toml | +5/-1 | deps (rayon, wgpu-core) |
| render/wgpu/shaders/bitmap.wgsl, gradient.wgsl | +5/-2 each | shader tweaks |
| render/wgpu/shaders/filter/batch_blur.wgsl | +70 | NEW batched blur |
| render/wgpu/shaders/filter/batch_glow.wgsl | +78 | NEW batched glow |
| render/wgpu/src/arena.rs | +292 | NEW GPU arena allocator |
| render/wgpu/src/backend.rs | +941/-59 | backend: atlas, async encode, `aqw_trim_memory` |
| render/wgpu/src/blend.rs | +11 | minor |
| render/wgpu/src/buffer_builder.rs | +18 | minor |
| render/wgpu/src/buffer_pool.rs | +250/-24 | pooling |
| render/wgpu/src/cache_batch.rs | +172 | NEW cache batch |
| render/wgpu/src/context3d/mod.rs | +5 | minor |
| render/wgpu/src/descriptors.rs | +78/-19 | descriptors/pipelines |
| render/wgpu/src/filters.rs | +61 | filter batching |
| render/wgpu/src/filters/batch.rs | +467 | NEW filter batcher |
| render/wgpu/src/filters/{bevel,blur,color_matrix,displacement_map,glow}.rs | +3/-1 each | batch hooks |
| render/wgpu/src/lib.rs | +432/-8 | globals/switches (`set_atlas_enabled`, `set_cache_batch_enabled`, ...) |
| render/wgpu/src/mesh.rs | +213/-139 | mesh storage |
| render/wgpu/src/packing.rs | +107 | NEW packing |
| render/wgpu/src/pipelines.rs | +1/-1 | minor |
| render/wgpu/src/pixel_bender.rs | +2 | minor |
| render/wgpu/src/segments.rs | +100 | NEW segments |
| render/wgpu/src/shaders.rs | +42 | shaders registered |
| render/wgpu/src/surface.rs | +437/-198 | surface passes |
| render/wgpu/src/surface/commands.rs | +1062/-56 | command execution, merging |
| render/wgpu/src/surface/target.rs | +101/-1 | targets |
| render/wgpu/src/utils.rs | +1 | minor |
### desktop
| File | +/- | Purpose |
|---|---|---|
| desktop/Cargo.toml | +23/-6 | deps mimalloc, libmimalloc-sys, rustc-demangle, wgpu-core, reqwest(blocking), serde_json, `windows` (DXGI), more windows-sys features; Windows resource metadata rebrand |
| desktop/assets/favicon.ico | binary | replaced icon |
| desktop/assets/mugetsu-icon-64.rgba | binary | NEW 64x64 window icon |
| desktop/src/mugetsu.rs | +772 | NEW: central Mugetsu module (title, settings persistence, GPU choice, update check, tips) |
| desktop/src/main.rs | +247/-70 | mimalloc allocator, AQW default args, panic hook, log filter |
| desktop/src/app.rs | +708/-14 | perf log, memory watch, FPS counter, `apply_setting`, window icon/title, context menu hookup, raw device events off |
| desktop/src/custom_event.rs | +2 | `RuffleEvent::MugetsuSetting(Setting)` |
| desktop/src/gui.rs | +5/-3 | `show_context_menu(menu, settings, close_event)`, `mod mugetsu_menu` |
| desktop/src/gui/context_menu.rs | +313/-3 | NEW custom-styled right-click settings panel (all egui colors) |
| desktop/src/gui/mugetsu_menu.rs | +77 | NEW "Mugetsu" menu in Ruffle menu bar (--gui only) |
| desktop/src/gui/menu_bar.rs | +2 | adds the Mugetsu menu button |
| desktop/src/gui/controller.rs | +137/-5 | Vulkan-start crash tracking, adapter ids, `ui_visible` skip-present optimisation |
| desktop/src/backends/ui.rs | +96/-34 | message dialog title "Mugetsu"; device-font fallback chain (Helvetica->Arial etc.) |
| desktop/src/heap_census.rs | +415 | NEW allocation census (flag file `heap-census`) |
| desktop/src/linux.rs | +33 | NEW Linux restart + graphics choice dialog |
| desktop/src/windows.rs | +395 | NEW Windows: EcoQoS/priority, crash handler, title-bar (system) menu for Graphics |
| desktop/src/player.rs | +1/-1 | minor |
### frontend-utils / tests
| File | +/- | Purpose |
|---|---|---|
| frontend-utils/Cargo.toml | +4/-1 | deps |
| frontend-utils/src/backends/navigator.rs | +36/-10 | refactors request building into `build_request` closure; macOS: retry GET once on connect error (750 ms) |
| tests/framework/src/runner.rs | +1 | test harness |
| tests/framework/src/runner/image_test.rs | +19 | test harness |
| tests/tests/environment.rs | +4/-3 | test env |

## 2. Desktop / egui specifics
### 2.1 Window setup (app.rs)
```rust
// AQW build: Mugetsu's icon (64 px so the taskbar copy stays sharp).
let icon_bytes = include_bytes!("../assets/mugetsu-icon-64.rgba");
let icon = Icon::from_rgba(icon_bytes.to_vec(), 64, 64).expect("App icon should be correct");
...
    .with_visible(false)
    .with_title(crate::window_title()) // AQW build
    .with_window_icon(Some(icon))
    .with_min_inner_size(min_window_size);
```
- Title: `mugetsu::window_title()` = "Mugetsu", or "Mugetsu – {notes joined by ' · '}" (notes: graphics API name, "DirectX 12: Vulkan driver crashed on this PC", "update X available", "... · right-click here to change").
- Title reset after movie close: `set_title(&crate::window_title())`. Dialog title: "Mugetsu".
- `event_loop.listen_device_events(DeviceEvents::Never)` (1000Hz mouse perf).
- Size comes from CLI defaults (below): 960x580 physical px (not macOS), min size logic unchanged.

### 2.2 Default CLI / AQW URL (main.rs `mugetsu_args()`)
Exact URL: `const AQW_LOADER_URL: &str = "https://game.aq.com/game/gamefiles/Loader3.swf?ver=a";`
Triggered only when no movie URL positional is given (`Opt::try_parse_from(&args).map_or(true, |opt| opt.movie_url.is_some())` -> plain Ruffle if movie given). Then it builds `[exe, AQW_LOADER_URL, defaults...(skipping any the user already gave), user args...]`:
```rust
let mut defaults = vec![
    ("--tcp-connections", None, Some("allow")),
    ("--player-version", None, Some("9")),
];
#[cfg(not(target_os = "macos"))]
defaults.extend([("--width", None, Some("960")), ("--height", None, Some("580"))]);
defaults.extend([
    ("--power", Some("-p"), Some("high")),
    ("--no-gui", None, None),                    // unless --gui passed
    ("--graphics", Some("-g"), Some(graphics)),  // vulkan | dx12 | metal
]);
```
Graphics selection order: user `--graphics` > saved choice file > macOS "metal" > Intel/Qualcomm adapter => "dx12" > Vulkan-crashed-before marker => "dx12" > "vulkan".
Extra flags: `--no-obs` (sets env DISABLE_VULKAN_OBS_CAPTURE=1), `--gui` (shows Ruffle menu bar, stripped before clap parse).
main(): `ruffle_core::set_show_all_apis(true);` (AQW since 2026-10-10 uses APIs newer than loader's SWF version), `Opt::parse_from(mugetsu_args())`, `windows::install_crash_handler()`, `windows::aqw_tune_process()`.
Also startup per-AQW: `ruffle_core::frame_interpolation::set_enabled(saved_interpolation())`, `ruffle_render::tessellator::set_flash_aa(saved_flash_edges().unwrap_or(true))`.
Note: `--tcp-connections allow` is what permits SmartFox raw sockets; `--player-version 9` pins the reported version.

### 2.3 Removal of Ruffle default menu / open dialog
- No code is deleted; default is `--no-gui` => `GuiController` `no_gui` true, menu bar hidden ("No menu bar: the window is just the game"). `--gui` brings back Ruffle's menu bar (+ a "Mugetsu" menu added in menu_bar.rs).
- No Open-file dialog on startup since a movie URL is always passed. Panic hook's "report on GitHub" flow removed (`truncate_to_line_boundary`, `webbrowser` crash URL, `MessageDialogResult`, `Url` imports removed); replaced by an OK-only rfd dialog titled "Mugetsu" with GPU-reset or generic text, and backtrace in ruffle.log.
- Right-click: Ruffle's context menu is replaced when settings is Some: `app.rs` always passes `Some(MenuSettings::now())`, and `show_context_menu` shows menu if `!menu.is_empty() || settings.is_some()`; `ContextMenu::show` redirects to `show_mugetsu`.

### 2.4 egui theme (NO custom fonts, NO global egui Visuals/theme override)
There is no theme.rs change and no font loading; everything is custom-painted with `FontId::proportional(size*z)` using egui's default font. The "theme" is the set of constants in desktop/src/gui/context_menu.rs:
```rust
pub(crate) const WIDTH: f32 = 268.0;
pub(crate) const PANEL: Color32 = Color32::from_rgba_premultiplied(17, 19, 26, 242);
pub(crate) const EDGE: Color32 = Color32::from_rgba_premultiplied(58, 62, 78, 255);
pub(crate) const ROW_HOVER: Color32 = Color32::from_rgba_premultiplied(40, 44, 58, 255);
pub(crate) const TRACK: Color32 = Color32::from_rgba_premultiplied(48, 52, 66, 255);
pub(crate) const ACCENT: Color32 = Color32::from_rgb(124, 108, 240);
pub(crate) const TEXT: Color32 = Color32::from_rgb(232, 234, 242);
pub(crate) const MUTED: Color32 = Color32::from_rgb(140, 146, 166);

pub(crate) fn panel_style(style: &mut Style) {
    style.visuals.override_text_color = Some(TEXT);
    style.spacing.button_padding = vec2(0.0, 0.0);
    style.interaction.selectable_labels = false;
}
```
Panel frame (scale `z = (content_rect.height()/1000.0).clamp(0.6, 1.0)`):
```rust
Area::new(Id::new("context_menu")).order(Order::Foreground).fixed_pos(self.position...)
  .constrain_to(egui_ctx.content_rect()).interactable(true).show(egui_ctx, |ui| {
    panel_style(ui.style_mut());
    Frame::new().fill(PANEL).stroke(Stroke::new(1.0, EDGE))
      .corner_radius(CornerRadius::same((12.0 * z) as u8))
      .inner_margin(Margin::same((10.0 * z) as i8))
      .shadow(Shadow { offset: [0, (6.0*z) as i8], blur: (24.0*z) as u8, spread: 0, color: Color32::from_black_alpha(140) })
      .show(ui, |ui| { ui.set_width(WIDTH*z); ui.spacing_mut().item_spacing = vec2(0.0, 2.0*z); ... })
```
Fonts (all `FontId::proportional`): header "Mugetsu" 16, version "v1.6.0" 12 (MUTED, right), section heading 11 (MUTED + hline TRACK), row title 14.5, hint 11.5 (MUTED), segmented label 13, action-row right glyph 13. Rounding: row hover 8, toggle track = height/2 (19*z tall, 34*z wide, knob radius-3 white circle, TRACK->ACCENT lerp via `animate_bool_responsive`), segmented track 13, pill 11. Row heights: toggle 40, action 30, title 36, segmented 30.
Widgets (all in context_menu.rs, pub(crate), reused by mugetsu_menu.rs): `section`, `row_title`, `toggle_row`, `segmented`, `action_row`, `lerp_color`, `graphics_options()`.
Menu layout: header; movie's own enabled items as `action_row`s (AQW has none); GRAPHICS: "Edge smoothing" segmented [Flash-style | MSAA]; "Frame interpolation" toggle; "Graphics API" segmented (Auto/Vulkan/DirectX 12; Linux Auto/Vulkan/OpenGL; macOS Auto/Metal; "Changing it restarts Mugetsu") ; DISPLAY: "Show FPS" toggle, "Full screen" toggle (sends EnterFullScreen/ExitFullScreen); HELP: "Open log folder" ↗. Closes on click-away, Esc, or any click. Clicks send `RuffleEvent::MugetsuSetting(Setting::{FlashEdges(bool),Interpolation(bool),Graphics(Option<Graphics>),ShowFps(bool),OpenLogFolder})`, handled by `MainWindow::apply_setting` in app.rs.
Menu-bar variant (`--gui`): gui/mugetsu_menu.rs, same widgets with z=1.0, `ui.menu_button("Mugetsu", ...)`, no Full screen row.

### 2.5 Title bar menu (Windows only) — windows.rs `graphics_menu`
Appends to the window's system menu (right-click title bar) via `GetSystemMenu/AppendMenuW`, subclasses WndProc (`SetWindowLongPtrW GWLP_WNDPROC`) and handles `WM_SYSCOMMAND`. Items: "Graphics: Automatic (recommended)" (0x1000), "Graphics: Vulkan" (0x1010), "Graphics: DirectX 12" (0x1020), grayed "More settings: right-click the game" (0x1040), with check marks. Choosing shows MessageBoxW OK/Cancel "Mugetsu will restart to use {name} graphics...", saves, `start_again()` (CreateProcessW same cmdline), `exit(0)`. Public fns: `add_graphics_menu(window)`, `choose_graphics`, `sync_menu_checks`. Linux: linux.rs uses an rfd dialog instead. Other windows.rs: `aqw_tune_process` (HIGH priority + opt out of EcoQoS), `install_crash_handler` (SetUnhandledExceptionFilter; if crash module is a GPU driver in driver store/System32 and within 10 s of first present, writes marker and offers restart on DX12), `test_native_crash` (env RUFFLE_TEST_NATIVE_CRASH=driver|own).

### 2.6 Settings persistence (mugetsu.rs) — plain flag/marker files, NOT Ruffle preferences.toml
Dir: `dirs::config_local_dir()/ruffle` (= %LOCALAPPDATA%\ruffle). Files:
- `mugetsu-graphics.txt` ("vulkan" | "dx12" | "metal" | "gl"; absent = automatic)
- `mugetsu-interpolation.txt` (exists = on; default off)
- `mugetsu-edges.txt` ("flash" | "msaa"; absent => default Flash-style; `flash-aa` exe-flag)
- `mugetsu-show-fps.txt` (exists = on)
- `mugetsu-vulkan-crashed.txt` (module path crash record, invalidated by driver file mtime)
- `mugetsu-low-quality-tip-shown`
- Logs: `<cache_dir>/log/ruffle.log` ("Open log folder" opens it).
Exe-adjacent flag files (`exe_flag(name)`): `verbose-log`, `no-update-check`, `safe-drawing`, `no-shared-shapes`, `no-pass-merge`, `slow-multiply`, `no-atlas`, `no-glow-batch`, `no-cache-steps`, `no-replay`, `no-shape-eviction`, `wait-for-shapes`, `heap-census`, `test-vram-mb.txt`.
Default log filter: `warn,ruffle_desktop=info,ruffle_aqw_perf=info,ruffle_core=warn,ruffle_core::avm2=off,ruffle_core::stub=off,avm_trace=off` (verbose-log restores `warn,ruffle=info,avm_trace=info`; RUST_LOG overrides). Reason noted: AQW trace output includes login response (email/username).
Update check: GET `https://api.github.com/repos/mugetsu-aqw/mugetsu/releases/latest` (reqwest blocking), title note + once-per-version dialog to open `https://github.com/mugetsu-aqw/mugetsu/releases/latest`.
Other mugetsu.rs: FPS counter (`SHOWN_FPS` atomic f32 bits), DXGI `video_memory()`, `check_gpu_choice` (tip when running on iGPU with dGPU present), `low_quality_tick` (tip to set AQW quality LOW).

### 2.7 Other desktop changes
- Custom global allocator `CountingAlloc` wrapping `mimalloc::MiMalloc` (non-tracy) + heap_census hooks.
- controller.rs: `ui_visible` flag so GUI-only repaints that draw nothing don't re-present; gui repaints capped to ~60/s when `no_gui`.
- ui.rs: `device_font_candidates(name,bold,italic)`: name -> strip " Bold/ Italic/ Oblique/ Regular" suffixes -> Helvetica*/Arial MT/Geneva => Arial; Times=>Times New Roman; Courier=>Courier New -> Arial; substitutes registered under requested bold/italic flags.
- Win resource metadata: ProductName/FileDescription "Mugetsu...", CompanyName "Mugetsu", OriginalFilename "Mugetsu.exe", ProductVersion "1.6.0", FileVersion "1.6.0 (Ruffle 0.7.0-nightly.2026.10.2)".

## 3. ExternalInterface / JS bridge / fscommand / networking
- Grep of the whole patch for `ExternalInterface`, `fscommand`, `external_interface`, `javascript`, `XMLSocket`, `socket`: ZERO functional hits (only the type-name string "SocketObjectData" in avm2/object/aqw_inspect.rs, a debug description list). No change to core/src/external.rs, no desktop ExternalInterface provider, no changes to core/src/socket.rs or desktop socket backend, no URLLoader changes except below.
- => Desktop uses stock Ruffle behavior: desktop has no JS, so `ExternalInterface.available` is false/ `call` is a no-op in stock desktop (stock `ExternalInterface` in desktop only supports registered callbacks from the Rust side via `Player::call_internal_interface`/`add_external_interface` with no JS host). To talk to an AS3 bot SWF you will have to add an `ExternalInterfaceProvider` in desktop (impl `ruffle_core::external::ExternalInterfaceProvider`, set via `PlayerBuilder::with_external_interface(...)` -- verify in current fda41800 tree) yourself; this patch does not provide it.
- Networking: `--tcp-connections allow` default in `mugetsu_args()` is the only enabler for XMLSocket / flash.net.Socket (AQW SmartFox). No socket code modified.
- frontend-utils navigator.rs: refactor + macOS-only retry of GET once on `is_connect()` errors after 750 ms (POST not retried, "could repeat a login"). No URLLoader/cookie/header/proxy/TLS changes for other platforms. `reqwest` gets `blocking` feature in desktop only for update check.
- Related core: `loader.rs` adds info log `loaded SWF: {url}` (target ruffle_aqw_perf); `show_all_apis` gate in avm2.rs means AS3 code sees all APIs regardless of SWF version (useful for a bot SWF too).
- Player version forced to 9 via `--player-version 9` default (affects Capabilities.version/AVM1 behaviors).

## 4. Cargo.toml / Cargo.lock hunks
Cargo.toml (appended after `lto = "off"` at line ~206):
```toml
# AQW build: wgpu-core with O(used) usage-scope reset (see patches/wgpu-core/src/track/metadata.rs).
[patch.crates-io]
wgpu-core = { path = "patches/wgpu-core" }
wgpu-hal = { path = "patches/wgpu-hal" }
gc-arena = { path = "patches/gc-arena" }
# AQW build: a `Positive` fill rule for Flash-style edges (see render/src/tessellator.rs).
lyon_path = { path = "patches/lyon_path" }
lyon_algorithms = { path = "patches/lyon_algorithms" }
```
(paths are relative to the Ruffle repo root: ./patches/<crate>; those directories are separate and not in this diff.)
Cargo.lock: adds `cty 0.2.2`, `libmimalloc-sys 0.1.49` (deps cc, cty), `mimalloc 0.1.52`; removes `source`/`checksum` lines in 5 packages (wgpu-core, wgpu-hal, gc-arena, lyon_path, lyon_algorithms -- path-patched); adds deps to ruffle_desktop (libmimalloc-sys, mimalloc, reqwest, rustc-demangle, serde_json, wgpu-core, windows 0.62.2), ruffle_core (hashbrown 0.17.1), ruffle_render_wgpu (rayon, wgpu-core).
desktop/Cargo.toml: `mimalloc` 0.1 (no default features), `rustc-demangle` 0.1, `libmimalloc-sys` 0.1 (features ["extended"]), `wgpu-core = "30.0.1"`, `reqwest` workspace + "blocking", `serde_json` workspace; windows: `windows = { version = "0.62", features = ["Win32_Graphics_Dxgi"] }`, windows-sys 0.61.2 extra features Win32_System_Threading/Foundation/Diagnostics_Debug/Kernel/LibraryLoader/Environment/UI_WindowsAndMessaging.
frontend-utils/Cargo.toml +4/-1, render/Cargo.toml +1, render/wgpu/Cargo.toml +5/-1 (rayon, wgpu-core, etc.).

## Takeaways for reuse
- To reuse the egui look: copy the constants + helper widgets from context_menu.rs lines (section 2.4) and `mugetsu_menu.rs`.
- The AQW URL/default flags are all in `mugetsu_args()` in desktop/src/main.rs; settings are file markers in %LOCALAPPDATA%\ruffle.
- ExternalInterface for an AS3 bot must be added; the patch gives nothing.
