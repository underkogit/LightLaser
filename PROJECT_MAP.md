# PROJECT_MAP

A navigation map of the LightLaser codebase: what runs, in which order, where each concern
lives, and where to make changes. [README.md](README.md) covers usage; this document covers
structure. [REFACTOR_REPORT.md](REFACTOR_REPORT.md) covers structural debt and a cleanup plan.

## Snapshot

| Item | Value |
| --- | --- |
| Language, edition | Rust 2024, binary crate (no `lib` target) |
| Size | 82 files, 13,828 lines under `src/` |
| UI | `eframe` / `egui` 0.36, `egui_extras` image loaders, `egui-phosphor` icons |
| Hardware | `serialport` → GRBL-compatible controller |
| Imports | `image`, `resvg` + `tiny-skia`, `gerber_parser` + `clipper2` + `evalexpr` |
| Persistence | `serde` + `toml` (projects and app config), flat TOML locales |
| Threads | 2: eframe UI thread and one streamer thread |
| Baseline | `cargo test`: 44 passed, 1 ignored |

Area split by lines: `ui` 7,154 (52%), `core` 5,678 (41%), `comm` 788 (6%), app entry
points 208 (1%). The three largest files are `core/image/gerber.rs` (832),
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

### `src/comm` — controller link (788 lines)

| Path | Responsibility |
| --- | --- |
| `comm/grbl.rs` | `GrblConnection`: handshake, `$$` settings (`$130`/`$131` work area, `$30` full-scale S, `$32` laser mode), `send_and_wait_ok`, jog, home, unlock, feed hold, cycle start, soft reset; `GrblEvent` and `GrblSettings` definitions |
| `comm/serial.rs` | `SerialConnection`: port open (DTR/RTS pulse), line-oriented read with 100 ms timeout, raw writes |
| `comm/status.rs` | `GrblStatus::parse` for `<State|MPos:…|WPos:…|FS:…>` reports |
| `comm/streamer.rs` | `Streamer` thread: command loop, 120-byte RX window, `ok` accounting, abort on `error:`/`ALARM:`, stall recovery, 250 ms status polling |

### `src/core` — domain model and job generation (5,678 lines)

| Path | Responsibility |
| --- | --- |
| `core/job.rs` | `build_job(project, spindle_max, laser_mode)` → `(Vec<String>, Vec<LayerReport>)`: the whole document as one G-code program plus a per-layer diagnostic (power, feed, moves, cut length) |
| `core/layer/geometry.rs` | The single source of layer geometry: `layer_segments(layer)` → mirrored, origin-anchored segments for image, SVG, G-code, Gerber, and vector layers; `placement(layer)` → where those segments sit on the bed |
| `core/mirror.rs` | Mirroring primitives: `flip_image` (raster), `flip_segments` (flips about the geometry's own centre), `bounds` |
| `core/layer/project.rs` | `Project` (`version`, `name`, `layers`, `groups`, `machine`); `PROJECT_VERSION = 1`; group and layer ordering, id allocation, `normalize` |
| `core/layer/group.rs` | `LayerGroup` (id, name, colour, collapsed, `visible`, `enabled`), `GroupId`, `DropTarget` |
| `core/layer/layer.rs` | `Layer` (with stable `id` and `group`), `LayerParams`, `LayerSource`, `VectorShape`, `EngraveMode`, `LayerUnit`; bounds and translate helpers |
| `core/config.rs` | `MachineConfig` (port, baud, work area, jog step/feed, job passes) |
| `core/vector/` | `CurvePoint`, `Bezier*`, `PathNode`, `VectorPath`, `StrokeStyle`; flattening and bounds |
| `core/tools/` | Drawing tools behind the `Tool` trait: select, line, curve, rect, circle; tool events and previews |
| `core/vector_gcode.rs` | Vector layer → `GcodeCommand` list (flatten, laser on/off, passes) |
| `core/gcode/parser.rs` | Line parser into `GcodeCommand` fields (only `raw` is consumed downstream) |
| `core/gcode/segments.rs` | G-code text → XY segments, normalization to origin |
| `core/gcode/stream.rs` | `GcodeStream`: line iterator (not used by the app) |
| `core/image/loader.rs` | `image::open` wrapper with aspect ratio |
| `core/image/raster.rs` | `RasterizeOptions`, `RasterImage`, resize to px/mm, vertical flip, `is_dark` |
| `core/image/engraver.rs` | Raster → horizontal runs (`EngraveSegment`) |
| `core/image/svg_size.rs` | Reads `width`/`height`/`viewBox` from SVG headers; unit and PCB profiles |
| `core/image/svg_raster.rs` | Renders SVG into an exact pixel box with uniform scale |
| `core/image/svg_trace.rs` | Rasterizes SVG, Zhang-Suen thinning, path tracing, RDP simplification |
| `core/image/svg_vector.rs` | SVG path extraction and outline G-code (alternate, unused path) |
| `core/image/gerber.rs` | Gerber document walk, apertures, macros, arcs, clipper2 union/stroke → segments |
| `core/image/palette.rs` | Palette extraction (unused) |
| `core/bounding_box.rs` | `BoundingBox` accumulator used for fit-to-view |

### `src/ui` — application shell (7,154 lines)

| Path | Responsibility |
| --- | --- |
| `ui/mod.rs` | `LaserApp` state, keyboard shortcuts, drag-and-drop routing, panel layout, copy/paste/duplicate |
| `ui/app_config.rs` | `AppConfig` (`app_config.toml`): locale, scale, grid, console font, UI font, SVG profile, window size, theme |
| `ui/project_io.rs` | New/open/save/save-as, layer import and file replacement via `FileKind` (dialog filter, source, bounds), G-code and Gerber bounds scans |
| `ui/history.rs` | Undo/redo stacks of whole `Project` clones (limit 100) |
| `ui/selection.rs` | Index-based selection state and hit-testing helpers |
| `ui/tools_state.rs` | Active tool, tool dispatch, fixed 1 mm snapping |
| `ui/canvas_transform.rs` | World ↔ screen mapping (Y up) |
| `ui/line_raster.rs` | Software line rasterizer and texture sizing for canvas/preview caches |
| `ui/panels/` | Top bar (menus, connection, state), bottom bar (version, MPos), left tool rail, right panel (layers, properties, machine controls, jog crosshair) |
| `ui/panels/canvas/` | Canvas panel: background, layers, overlays, input handling, `CanvasState`, segment cache |
| `ui/panels/right_panel/layers/` | Layer tree: `mod.rs` (list, drag lifecycle, `LayerAction` and its application), `row.rs` (group header and layer row widgets, `LayerRow`/`GroupRow`), `drop.rs` (pointer → `DropTarget`, insertion line, drag ghost) |
| `ui/widget/` | Dialogs and windows: connection, export, settings, console, preview, G-code viewer; job builder |

## Data model

```
Project
├── version: u32            (currently 1)
├── name: String
├── machine: MachineConfig  (port, baudrate, work_area_mm, jog_feed, jog_step, spindle_max, passes)
├── groups: Vec<LayerGroup>  order = panel order
│   ├── id: GroupId (u64)
│   ├── name: String
│   ├── collapsed: bool
│   ├── visible: bool       switch for the whole group
│   └── enabled: bool       switch for the whole group
└── layers: Vec<Layer>      order = z-order and engraving order
    ├── id: u64             stable, allocated by `Project`
    ├── group: GroupId      always names an existing group
    ├── name: String
    ├── source: Image{path} | Svg{path} | Gcode{path} | Gerber{path} | Vector
    ├── is_visible (canvas), is_enabled (job/export)
    ├── params: LayerParams
    │   ├── power_percent, feed_rate, passes, dynamic_power
    │   ├── pixels_per_mm, threshold, dither, invert
    │   └── engrave_mode: Raster | Lines (SVG only)
    ├── bounds: Option<[min_x, min_y, max_x, max_y]>   world millimetres
    ├── shapes: Vec<VectorShape>                       vector layers only
    └── mirror_x, mirror_y                             content mirror flags
```

Groups are blocks, not tags: every group's layers occupy one contiguous range of `layers`, and
the blocks follow the order of `groups`. `Project::compact_groups` enforces it after a move.
Groups may be empty, so a group can exist before a layer is dropped into it; `group_block`
returns `None` for those and `group_insert_index` finds where their layers belong.
`Project::normalize` runs on load: it allocates missing ids, gives nameless groups a name, and
puts orphan layers into one new group, which is how pre-group projects are upgraded. Group ids
and layer ids come from one counter, `Project::next_id`, derived from the largest id in the
document, so ids survive save/load without storing the counter.

A group's `visible`/`enabled` flags are independent of the flags on its layers:
`Project::layer_visible` and `Project::layer_enabled` combine both, and every consumer (canvas,
hit testing, preview, job builder, export count) goes through them. The panel's group
checkboxes write only the group flags; the "hide/show layers" toggle in the group menu writes only
the layer flags.

Layer sources split into two families:

* **File-backed** (`Image`, `Svg`, `Gcode`, `Gerber`): `core::layer::geometry::layer_segments`
  turns the file into segments starting at the origin, and `placement(layer)` puts them on the
  bed at the lower-left corner of `bounds`. Moving a layer only moves `bounds`, so the geometry
  is cached across drags and rebuilt only when the file, the parameters, or the mirror flags
  change.
* **Vector**: the node coordinates themselves are the world geometry, so `shapes` are drawn
  and emitted as they are and `placement` is `(0, 0)`; `Layer::translate` moves the nodes.

Mirroring is part of the geometry: `Image` and `Svg` flip their raster before rasterizing or
tracing, `Gcode` and `Gerber` flip their parsed segments about the geometry's own centre, and
vector layers mirror their nodes. `Layer::mirror` toggles the flag and mirrors `bounds` about the
selection centre, so a multi-layer selection mirrors as one picture while each layer flips in
place.

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

## Job build pipeline (`core/job.rs`, called from `ui/widget/job.rs`)

1. `start_job` calls `core::job::build_job(&project, machine.spindle_max, grbl_laser_mode)`;
   it returns the G-code lines plus one `LayerReport` per emitted layer.
2. Header: `G21`, `G90`, `G17`.
3. Outer loop over `machine.passes` repeats the whole job.
4. For each layer whose `Project::layer_enabled` is true (visibility is ignored here), take
   `core::layer::geometry::layer_segments` and add `placement` to every coordinate:
   * `Image` → `load_image` → flip if mirrored → `rasterize` (px/mm, threshold, invert) →
     `engrave_segments`.
   * `Svg`, `EngraveMode::Lines` → `svg_trace::rasterize_svg_to_image` (512–4096 px) → flip if
     mirrored → `trace_image_to_paths(128)` → segments scaled by X/Y factors.
   * `Svg`, raster mode → `svg_raster_box` (64–4096 px) → `rasterize_svg_into_box` → flip if
     mirrored → `rasterize` → `engrave_segments`.
   * `Gcode` → segment parser → flip if mirrored → normalize to min.
   * `Gerber` → `parse_gerber_segments` → flip if mirrored → `chain_segments` into polylines.
   * `Vector` → `export_vector_layer` → flatten at 0.05 mm → `G0`/`G1`.
5. `Image`, `Svg`, and `Gcode` emit one laser-on header and `G0`/`G1` per segment; `Gerber`
   switches the laser per polyline. After each layer: `M5`. After the job: `G0 X0 Y0`, `M5`.
6. If the controller reported `$32=0`, every `M4` is rewritten to `M3`: without laser mode the
   blade of dynamic power does nothing and `M3` (spindle CW) is the command that actually
   switches a diode module on.
7. `StreamerCommand::Start { lines }` hands the lines to the streamer thread.

`LayerReport` (`index`, `name`, `kind`, `laser_power`, `power_percent`, `feed_rate`, `passes`,
`moves`, `cut_length`) is logged once per job, so a layer that would not burn can be recognised
from the console alone: `is_silent()` is true when a layer emits no moves or only zero length
ones, and the log prints the percentage, `S`, feed rate and an energy figure
(`power_percent / feed_rate`) that makes two layers of one group comparable at a glance.

Laser power is a percentage mapped onto the controller's full-scale S value: `$30` is fetched with
the rest of the settings when the controller connects and stored in `machine.spindle_max`
(default 1000, which is the GRBL default). `LayerParams::spindle_value` performs that mapping, so
`M4`/`M3` carry `S` in the controller's own range instead of a fixed `S0..S1000`.
`export_job_gcode` uses the same builder and joins the lines with `\n`; the export dialog
and the G-code viewer both call it.

## Canvas pipeline (`ui/panels/canvas`)

1. `ui_canvas` allocates a painter, applies a pending "fit view", builds `CanvasTransform`.
2. `render_background`: grid, axes, work-area rectangle with dimensions.
3. `render_layers`: skips layers that `Project::layer_visible` rejects; per source either
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

* **Project** (`*.toml`, `Project`): version, name, machine, groups, layers with id, group,
  source, colour, flags, params, bounds, shapes, mirror flags. Version mismatch only
  logs a warning; `load_project_from` calls `Project::normalize` afterwards.
* **App config** (`<exe>/config/app_config.toml`, `AppConfig`): window size, locale, theme,
  grid/axes/work-area toggles, grid step, UI scale, console font size, optional UI font
  file, SVG import profile.
* **Locales** (`locales/en.toml`, `locales/ru.toml`): flat `"key" = "value"` maps, 151 keys
  each, loaded from `<exe>/locales` at startup. `LaserApp` resolves them with `t!`; the
  auto-generated group name uses `ui.layer_group.name` as its base word and falls back to
  `Group` when the dictionary is missing.

## Where to change what

| Task | Touch |
| --- | --- |
| Add an imported file type | `LayerSource` and `Layer::is_raster` (`core/layer/layer.rs`); `ui/project_io.rs`; drag-and-drop match in `ui/mod.rs`; import menu in `ui/panels/top_bar.rs`; a branch in `core/layer/geometry.rs` (mirroring included) |
| Add a layer parameter | `LayerParams` (`core/layer/layer.rs`), properties UI (`right_panel/properties.rs`), the parameter application block there, and any consumer (`geometry.rs`, `vector_gcode.rs`, `job.rs`) |
| Add a drawing tool | `core/tools/<name>.rs` + `ToolKind`/icon/tooltip in `core/tools/tool.rs` + `ToolsState` (`ui/tools_state.rs`) + input dispatch (`canvas/input.rs`) |
| Change emitted G-code for a source | `core/job.rs` for how segments become `G0`/`G1`; `core/layer/geometry.rs` for what the source generates; `core/vector_gcode.rs` for vector layers |
| Change what the canvas shows | `ui/panels/canvas/draw.rs`; segment generation in `core/layer/geometry.rs` |
| Add a UI string | `locales/en.toml` and `locales/ru.toml` (keep keys identical), then `t!("…")` at the call site |
| Add an app setting | `ui/app_config.rs` field + default + settings UI (`widget/settings_window.rs`), then read it from `LaserApp` |
| Add a controller command | `comm/grbl.rs` method, `StreamerCommand` + handler in `comm/streamer.rs`, button in `right_panel/machine.rs` or `jog_crosshair.rs` |
| Change undo behaviour | `ui/history.rs` plus every `push_history` call site (search `push_history`) |
| Add a group action | `LayerAction` + `apply_layer_actions` in `ui/panels/right_panel/layers/mod.rs`, the matching `Project` method in `core/layer/project.rs`, and the header widget in `group_header` (`layers/row.rs`) |
| Add a layer action | `LayerAction` + `apply_layer_actions` (`layers/mod.rs`) and a button or context-menu entry in `layer_row` (`layers/row.rs`). Layer rows get `LayerSource` and decide there whether a file action applies |
| Change group ordering | `Project::group_block` / `group_order` / `group_insert_index` / `move_layers_to` / `compact_groups` in `core/layer/project.rs`; the panel only reports a `DropTarget` |
| Change how rows drag | `ui/panels/right_panel/layers/`: `row.rs` starts a `LayerRowDrag` on a press, `drop.rs` maps the pointer to a `DropTarget` and paints the indicator and ghost, `mod.rs` turns the release at the end of `ui_layer_list` into a `LayerAction::Drop`. Nothing relies on egui's built-in drag-and-drop payloads |

## Invariants and traps

* Selection is index-based: `project.layers[i]`, `layer.shapes[j]`. Layers also carry a
  stable `id`, and the layer panel converts between the two (`selected_layer_ids` /
  `select_layer_ids`) before touching the document, because a drop reorders indices under the
  selection. Any other code that reorders or removes layers must do the same.
* Every layer names an existing group; a group may be empty. `group_block` returns `None` for
  an empty group and `group_insert_index` places its first layer where the group sits in
  `groups` order. `remove_layer` leaves the emptied group in place, `remove_group` drops its
  layers. `move_layers_to` resolves the drop anchor before it removes the dragged layers and
  then shifts the index by how many of them sat above it, so a group's block stays contiguous.
* Group visibility is two flags deep: `LayerGroup::visible`/`enabled` are switches for the whole
  group and are deliberately not copied into the layers. All drawing, hit testing, job building,
  and counting go through `Project::layer_visible` / `Project::layer_enabled`, which combine the
  group flag with the layer flag.
* Selecting a group means selecting all of its layer indices; the canvas drag path
  (`collect_drag_states`) then moves them together with no extra code.
* There is one segment builder, `core::layer::geometry::layer_segments`; the job builder, the
  canvas cache, and the preview all call it, so a layer looks the same everywhere. Placement is
  a separate step (`placement`), which is what makes dragging cheap: the geometry is cached by
  source, parameters, size, and mirror flags, and only the lower-left corner changes.
* A layer carries its world position in `bounds` (and, for vector layers, in its node
  coordinates). `Layer::translate` moves both; `Layer::mirror` flips the mirror flags, mirrors
  `bounds` about the selection centre, and for vector layers mirrors the nodes and recomputes
  the bounds.
* A zero length `G1` never fires the laser in laser mode, because GRBL only energises while
  moving. `engrave_segments` widens a single-pixel dark run to one pixel of travel for that
  reason, and `LayerReport::is_silent` flags a layer whose whole toolpath collapsed to points.
* `$32=0` (no laser mode) makes GRBL interpret `M4` as "spindle CCW, dynamic power off"; a diode
  module wired to the direction pin then stays dark, so the job builder rewrites `M4` to `M3`
  when the controller reports laser mode off.
* `Project::duplicate_group` inserts the copy into `groups` right after the original and then
  appends its layers through `add_layer_to_group`, so the new block lands next to the source
  block. The copy gets a fresh group id, a unique name (`X copy`, then `X copy 2`), and new
  layer ids; layer names, parameters, and transforms are copied unchanged. The panel then
  translates the copy by one snap step, which is a UI concern, not part of the model.
* `FileKind` in `ui/project_io.rs` is the single mapping between a `LayerSource` variant, its
  file filter, its bounds rule, and its constructor. Import and replacement both go through it,
  and `FileKind::of` returning `None` is what makes a vector layer unreplaceable. Replacement
  keeps the previous lower-left corner (Gerber excepted, its geometry is absolute) and, for
  images, the previous width.
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
4. `src/core/layer/geometry.rs` — how any layer becomes segments, mirrored and anchored.
5. `src/ui/widget/job.rs` — how a document becomes G-code.
6. `src/ui/panels/canvas/mod.rs` and `input.rs` — how drawing and editing feel.
7. `src/comm/streamer.rs` — how bytes reach the controller.
