# PROJECT_MAP

A navigation map of the LightLaser codebase: what runs, in which order, where each concern
lives, and where to make changes. [README.md](README.md) covers usage; this document covers
structure. [REFACTOR_REPORT.md](REFACTOR_REPORT.md) covers structural debt and a cleanup plan.

## Snapshot

| Item | Value |
| --- | --- |
| Language, edition | Rust 2024, binary crate (no `lib` target) |
| Size | 78 files, 11,902 lines under `src/` |
| UI | `eframe` / `egui` 0.36, `egui_extras` image loaders, `egui-phosphor` icons |
| Hardware | `serialport` → GRBL-compatible controller |
| Imports | `image`, `resvg` + `tiny-skia`, `gerber_parser` + `clipper2` + `evalexpr` |
| Persistence | `serde` + `toml` (projects and app config), flat TOML locales |
| Threads | 2: eframe UI thread and one streamer thread |
| Baseline | `cargo test`: 13 passed, 1 ignored; `cargo check --all-targets`: 53 warnings |

Area split by lines: `ui` 6,682 (56%), `core` 4,264 (36%), `comm` 748 (6%), app entry
points 208 (2%). The three largest files are `core/image/gerber.rs` (832),
`ui/widget/preview_window.rs` (753), and `ui/panels/canvas/input.rs` (700).

## Runtime topology

```
                 main.rs
                    │
                    │ creates logger, config, streams
                    ▼
   ┌─────────────────────────────────────────┐          ┌──────────────────────────┐
   │ UI thread: LaserApp (eframe::App)       │          │ streamer thread          │
   │  ├─ Project (documents, layers, params) │  cmd     │  run_loop (streamer.rs)  │
   │  ├─ CanvasState + caches                │ ───────► │   ├─ GrblConnection      │
   │  ├─ History, Selection, ToolsState      │ mpsc     │   ├─ SerialConnection    │
   │  ├─ Streamer (sender handle)            │ ◄─────── │   └─ RX/TX window logic  │
   │  └─ AppConfig, log buffer (Arc<Mutex>)  │  events  └────────────┬─────────────┘
   └─────────────────────────────────────────┘                       │ serial
                                                                     ▼
                                                          GRBL controller
```

* UI → streamer: `StreamerCommand` over an `mpsc` channel.
* Streamer → UI: `GrblEvent` over a second `mpsc` channel, drained once per frame by
  `LaserApp::drain_grbl_events` (`src/ui/mod.rs`).
* Logs: `MultiLogger` writes to stderr and to an `Arc<Mutex<VecDeque<LogEntry>>>` that the
  in-app console renders.

## Startup sequence (`src/main.rs`)

1. Build `MultiLogger`, install it, set the max level from the `RUST_LOG`-style filter.
2. Resolve the config directory: `<directory of the executable>/config` (`src/dirs.rs`).
3. Read `config/app_config.toml`, then apply `app_locale` to `rust-i18n`.
4. Create the events channel and spawn the streamer thread (`Streamer::spawn`).
5. Open a resizable 1200×720 viewport titled `LightLaser - v<version>`.
6. Install image loaders, fonts (`src/ui/fonts.rs`), spacing tweaks, and construct
   `LaserApp` with every long-lived state object.

`LaserApp::on_exit` saves `AppConfig`; the settings window also saves when it is dismissed.

## Module map

### Entry points and infrastructure

| Path | Responsibility |
| --- | --- |
| `src/main.rs` | Logger, config, streamer, viewport, `LaserApp` construction |
| `src/dirs.rs` | Config directory resolution (`<exe>/config`) |
| `src/logger.rs` | `MultiLogger`: stderr plus in-memory ring buffer (2000 entries) |
| `src/i18n_backend.rs` | `rust-i18n` backend that loads `locales/<code>.toml` next to the executable |
| `build.rs` | Copies `locales/*` into `target/<profile>/locales` |

### `src/comm` — controller link (748 lines)

| Path | Responsibility |
| --- | --- |
| `comm/grbl.rs` | `GrblConnection`: handshake, `$130`/`$131` work area, `send_and_wait_ok`, jog, home, unlock, feed hold, cycle start, soft reset; `GrblEvent` definition |
| `comm/serial.rs` | `SerialConnection`: port open (DTR/RTS pulse), line-oriented read with 100 ms timeout, raw writes |
| `comm/status.rs` | `GrblStatus::parse` for `<State|MPos:…|WPos:…|FS:…>` reports |
| `comm/streamer.rs` | `Streamer` thread: command loop, 120-byte RX window, `ok` accounting, abort on `error:`/`ALARM:`, stall recovery, 250 ms status polling |

### `src/core` — domain model and job generation (4,264 lines)

| Path | Responsibility |
| --- | --- |
| `core/layer/project.rs` | `Project` (`version`, `name`, `layers`, `machine`); `PROJECT_VERSION = 1`; layer order operations |
| `core/layer/layer.rs` | `Layer`, `LayerParams`, `LayerSource`, `VectorShape`, `EngraveMode`, `LayerUnit`; bounds and translate helpers |
| `core/config.rs` | `MachineConfig` (port, baud, work area, jog step/feed, job passes) |
| `core/vector/` | `CurvePoint`, `Bezier*`, `PathNode`, `VectorPath`, `StrokeStyle`; flattening and bounds |
| `core/tools/` | Drawing tools behind the `Tool` trait: select, line, curve, rect, circle; tool events and previews |
| `core/vector_gcode.rs` | Vector layer → `GcodeCommand` list (flatten, laser on/off, passes) |
| `core/gcode/parser.rs` | Line parser into `GcodeCommand` fields (only `raw` is consumed downstream) |
| `core/gcode/segments.rs` | G-code text → XY segments, mirroring, normalization to origin |
| `core/gcode/stream.rs` | `GcodeStream`: line iterator (not used by the app) |
| `core/image/loader.rs` | `image::open` wrapper with aspect ratio |
| `core/image/raster.rs` | `RasterizeOptions`, `RasterImage`, resize to px/mm, vertical flip, `is_dark` |
| `core/image/engraver.rs` | Raster → horizontal runs (`EngraveSegment`) and → `GcodeCommand` list |
| `core/image/svg_size.rs` | Reads `width`/`height`/`viewBox` from SVG headers; unit and PCB profiles |
| `core/image/svg_raster.rs` | Renders SVG into an exact pixel box with uniform scale |
| `core/image/svg_trace.rs` | Rasterizes SVG, Zhang-Suen thinning, path tracing, RDP simplification, G-code |
| `core/image/svg_vector.rs` | SVG path extraction and outline G-code (alternate, unused path) |
| `core/image/gerber.rs` | Gerber document walk, apertures, macros, arcs, clipper2 union/stroke → segments |
| `core/image/palette.rs` | Palette extraction (unused) |
| `core/bounding_box.rs` | `BoundingBox` accumulator used for fit-to-view |

### `src/ui` — application shell (6,682 lines)

| Path | Responsibility |
| --- | --- |
| `ui/mod.rs` | `LaserApp` state, keyboard shortcuts, drag-and-drop routing, panel layout, copy/paste/duplicate |
| `ui/app_config.rs` | `AppConfig` (`app_config.toml`): locale, scale, grid, console font, UI font, SVG profile, window size, theme |
| `ui/project_io.rs` | New/open/save/save-as, layer import helpers, G-code bounds scan |
| `ui/history.rs` | Undo/redo stacks of whole `Project` clones (limit 100) |
| `ui/selection.rs` | Index-based selection state and hit-testing helpers |
| `ui/tools_state.rs` | Active tool, tool dispatch, fixed 1 mm snapping |
| `ui/canvas_transform.rs` | World ↔ screen mapping (Y up) |
| `ui/line_raster.rs` | Software line rasterizer and texture sizing for canvas/preview caches |
| `ui/panels/` | Top bar (menus, connection, state), bottom bar (version, MPos), left tool rail, right panel (layers, properties, machine controls, jog crosshair) |
| `ui/panels/canvas/` | Canvas panel: background, layers, overlays, input handling, `CanvasState`, segment cache |
| `ui/widget/` | Dialogs and windows: connection, export, settings, console, preview, G-code viewer; job builder |

## Data model

```
Project
├── version: u32            (currently 1)
├── name: String
├── machine: MachineConfig  (port, baudrate, work_area_mm, jog_feed, jog_step, passes)
└── layers: Vec<Layer>
    ├── name, color: [u8;3]
    ├── source: Image{path} | Svg{path} | Gcode{path} | Gerber{path} | Vector
    ├── is_visible (canvas), is_enabled (job/export)
    ├── params: LayerParams
    │   ├── power_percent, feed_rate, passes, dynamic_power
    │   ├── pixels_per_mm, threshold, dither, invert
    │   └── engrave_mode: Raster | Lines (SVG only)
    ├── bounds: Option<[min_x, min_y, max_x, max_y]>   world millimetres
    ├── shapes: Vec<VectorShape>                       vector layers only
    ├── offset: [dx, dy]                               placement for segment/vector sources
    └── mirror_x, mirror_y, mirror_center
```

Layer sources split into two families:

* **Raster-backed** (`Image`, `Svg` in raster mode): the source is a file, the layer's
  `bounds` are authoritative for placement and size; `offset` is not used when drawing.
* **Segment-backed** (`Gcode`, `Gerber`, `Vector`, `Svg` in lines mode): geometry is
  generated from the file or from `shapes`; `bounds` describes the world extent, and
  `offset` is applied on top when drawing and when emitting G-code.

Other long-lived UI state: `CanvasState` (zoom, pan, drag mode, texture/segment caches,
preview and G-code windows), `Selection` (layer/shape/node/handle indices), `History`,
`ToolsState`, `RightPanelState`, `StatusBar`, `AppConfig`.

## Units and coordinates

* Everything in the document is millimetres, origin at the work-bed lower-left, Y up.
* Screen mapping is in `ui/canvas_transform.rs`:
  `sx = rect.min.x + x * scale + offset.x`, `sy = rect.min.y - y * scale + offset.y`.
* Snapping is always on at 1 mm (`tools_state::SNAP_MM`); holding `Shift` while dragging
  sets the step to zero.
* The display grid step is independent (`AppConfig::grid_step_mm`), and the work area comes
  from `MachineConfig::work_area_mm` (controller `$130`/`$131`, default 300 × 300).

## Job build pipeline (`ui/widget/job.rs`)

1. `start_job` calls `build_job_gcode`; the result is a `Vec<String>` of raw G-code lines.
2. Header: `G21`, `G90`, `G17`.
3. Outer loop over `machine.passes` repeats the whole job.
4. For each `is_enabled` layer (visibility is ignored here), compute origin and size and
   branch on `LayerSource`:
   * `Image` → `load_image` → `rasterize` (px/mm, threshold, invert) → `engrave_layer`.
   * `Svg`, `EngraveMode::Lines` → `svg_trace::rasterize_svg_to_image` (512–4096 px) →
     `trace_image_to_paths(128)` → `traced_paths_to_gcode` with X/Y scale factors.
   * `Svg`, raster mode → `svg_raster_box` (64–4096 px) → `rasterize_svg_into_box` →
     `rasterize` → `engrave_layer`.
   * `Gcode` → read text → segment parser in this file → mirror → normalize to min → `G0`/`G1`.
   * `Gerber` → `parse_gerber_segments` → `chain_segments` into polylines → `G0`/`G1`.
   * `Vector` → `export_vector_layer` → flatten at 0.05 mm → `G0`/`G1`.
5. After each layer: `M5`. After the job: `G0 X0 Y0`, `M5`.
6. `StreamerCommand::Start { lines }` hands the lines to the streamer thread.

Laser power is always mapped to `S0..S1000` with `M4` (dynamic) or `M3` (constant).
`export_job_gcode` uses the same builder and joins the lines with `\n`; the export dialog
and the G-code viewer both call it.

## Canvas pipeline (`ui/panels/canvas`)

1. `ui_canvas` allocates a painter, applies a pending "fit view", builds `CanvasTransform`.
2. `render_background`: grid, axes, work-area rectangle with dimensions.
3. `render_layers`: skips invisible layers; per source either
   * draws an image/SVG texture (`texture_cache`, keyed by source path), or
   * asks `cache::get_or_build` for segments and draws them, choosing a rasterized texture
     when the cached texture resolution is sufficient or a culled line pass otherwise.
4. Machine position marker from `GrblStatus::mpos`.
5. `input::handle_interaction`: hover readout, wheel zoom, pan (middle button), selection,
   rubber band, layer/node/handle dragging, tool dispatch, Delete/Backspace.
6. `render_overlays`: active tool preview and rubber band.
7. Zoom buttons, mouse coordinate readout, and the canvas context menu (preview, G-code,
   copy, delete, mirror, round).

## Preview pipeline (`ui/widget/preview_window.rs`)

`open_preview` marks the window dirty; the next frame calls `rebuild_preview`, which walks
visible **and** enabled layers, collects world segments with `build_layer_segments`, and
computes overall bounds. Segments are cached as world-space textures (`PreviewRaster`) keyed
by view (`PreviewWindow::revision` plus `preview_view_key`); a stale texture is stretched
while the view moves and re-rendered when it settles. The progress slider re-bakes only the
processed prefix of the segment list.

## Controller protocol (`comm/streamer.rs`)

* Opening: `\r\n\r\n`, then `?` after 700 ms, accept a banner/status/`ok` within 5 s.
* Streaming: classic GRBL character-counting with a 120-byte window; each `ok` releases the
  oldest in-flight line; `error:` or `ALARM:` aborts the job with `JobFinished { success: false }`.
* Stall: after 10 s without `ok`, query status; if the controller reports `Run`/`Idle`,
  resend the front line once, otherwise soft reset (`0x18`) and fail the job.
* Status polling: every 250 ms while running and while idle with nothing in flight; the
  loop sleeps 2 ms between iterations.
* `Pause` sends `!`, `Resume` sends `~`, `Stop` sets `Stopping` and the next loop sends `0x18`.

## Persistence and localisation

* **Project** (`*.toml`, `Project`): version, name, machine, layers with source, colour,
  flags, params, bounds, shapes, offset, mirror fields. Version mismatch only logs a warning.
* **App config** (`<exe>/config/app_config.toml`, `AppConfig`): window size, locale, theme,
  grid/axes/work-area toggles, grid step, UI scale, console font size, optional UI font
  file, SVG import profile.
* **Locales** (`locales/en.toml`, `locales/ru.toml`): flat `"key" = "value"` maps, 138 keys
  each, loaded from `<exe>/locales` at startup. `LaserApp` resolves them with `t!`.

## Where to change what

| Task | Touch |
| --- | --- |
| Add an imported file type | `LayerSource` and `Layer::is_raster` (`core/layer/layer.rs`); `ui/project_io.rs`; drag-and-drop match in `ui/mod.rs`; import menu in `ui/panels/top_bar.rs`; builder branch in `ui/widget/job.rs`; `cache.rs` and `preview_window.rs` |
| Add a layer parameter | `LayerParams` (`core/layer/layer.rs`), properties UI (`right_panel/properties.rs`), the parameter application block there, and any consumer (`engraver.rs`, `vector_gcode.rs`, `job.rs`) |
| Add a drawing tool | `core/tools/<name>.rs` + `ToolKind`/icon/tooltip in `core/tools/tool.rs` + `ToolsState` (`ui/tools_state.rs`) + input dispatch (`canvas/input.rs`) |
| Change emitted G-code for a source | `ui/widget/job.rs` for live jobs; `core/image/engraver.rs`, `core/vector_gcode.rs`, `core/image/svg_trace.rs` for the source generators |
| Change what the canvas shows | `ui/panels/canvas/draw.rs`; segment generation in `canvas/cache.rs` |
| Add a UI string | `locales/en.toml` and `locales/ru.toml` (keep keys identical), then `t!("…")` at the call site |
| Add an app setting | `ui/app_config.rs` field + default + settings UI (`widget/settings_window.rs`), then read it from `LaserApp` |
| Add a controller command | `comm/grbl.rs` method, `StreamerCommand` + handler in `comm/streamer.rs`, button in `right_panel/machine.rs` or `jog_crosshair.rs` |
| Change undo behaviour | `ui/history.rs` plus every `push_history` call site (search `push_history`) |

## Invariants and traps

* Selection is index-based: `project.layers[i]`, `layer.shapes[j]`. Any reorder or removal
  needs the manual index fixups seen in `right_panel/layers.rs`.
* There are three independent segment builders (live job, canvas cache, preview). They must
  produce the same geometry for the same layer; today they do not in all cases.
* A layer carries world position in `bounds`, `offset`, and (for vector layers) node
  coordinates at once. `Layer::translate` updates all three.
* `GcodeCommand` stores parsed fields, but downstream code only reads `raw`.
* Passes exist at three levels: `machine.passes` (whole job), `layer.params.passes`
  (source generator), and per-source comment blocks.
* Canvas and preview skip invisible layers; jobs and exports skip disabled layers only.
* The config directory lives next to the executable, so an install under a read-only
  directory cannot save settings or projects there.

## Suggested reading order

1. `src/main.rs` — how the process starts.
2. `src/ui/mod.rs` — the `LaserApp` state and frame loop.
3. `src/core/layer/layer.rs` + `project.rs` — the document model.
4. `src/ui/widget/job.rs` — how a document becomes G-code.
5. `src/ui/panels/canvas/mod.rs` and `input.rs` — how drawing and editing feel.
6. `src/comm/streamer.rs` — how bytes reach the controller.
