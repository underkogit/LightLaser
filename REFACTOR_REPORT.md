# REFACTOR_REPORT

Structural review of LightLaser at revision `dc41548` (working tree: the uncommitted release
profile in `Cargo.toml`). Scope: everything under `src/`, plus `build.rs` and the manifests.
No production code was changed by this review; this is an assessment with a staged plan.

## Baseline

Measured on Windows with the project's own toolchain:

| Check | Result |
| --- | --- |
| `cargo test` | 13 passed, 1 ignored, 0 failed (14 tests total) |
| `cargo check --all-targets` | success, 53 warnings (38 dead-code items), 4 unused dependencies |
| Source size | 78 files, 11,902 lines |
| UI share | 6,682 lines (56%): `ui/panels` 3,278, `ui/widget` 1,898, `ui` root 1,506 |
| Core share | 4,264 lines (36%), of which `core/image` is 2,289 |
| Communication | 748 lines (6%) |
| Largest files | `core/image/gerber.rs` 832, `ui/widget/preview_window.rs` 753, `ui/panels/canvas/input.rs` 700, `ui/panels/canvas/draw.rs` 699 |

There is no CI configuration, no `lib` target, and no integration test directory. The only
tests cover G-code line parsing (3) and SVG size parsing (10); one performance test is
`#[ignore]`d.

Severity scale: **P0** produces wrong machine output or loses user data; **P1** will produce
wrong output or heavy maintenance cost as features are added; **P2** hygiene and ergonomics.

## Executive summary

The application is small and readable, but it has three copies of its most important
algorithm — turning a layer into G-code geometry — and they have already drifted. A layer can
preview differently from what the machine burns. Around that core, state is addressed by
`Vec` indices, three different placement representations coexist on `Layer`, and G-code is
assembled by string formatting in seven places. The code compiles and the tested parts work;
the debt is concentrated rather than spread out.

Top five, in order:

1. Vector layers are exported at twice their canvas position (P0).
2. Layer → segments exists three times, and preview/canvas/job disagree on which layers and
   offsets they use (P1, the root cause of several smaller divergences).
3. Passes are interpreted at three levels, and raster pass comments lie about what was cut (P1).
4. `dither` is a stored, exposed, serialised setting that no code reads (P1, silent no-op).
5. Undo does not cover the layer list, and undo itself is 100 full-document clones (P1/P2).

## P0 — wrong output

### 1. Vector layers are emitted at double their offset

Evidence:

* Dragging a layer writes the delta to `layer.offset` **and** to every node
  (`ui/panels/canvas/input.rs:447-463`).
* The properties panel calls `Layer::translate`, which does the same
  (`ui/panels/right_panel/properties.rs:103`, `core/layer/layer.rs:248-261`).
* Vector export takes the layer offset as the origin
  (`ui/widget/job.rs:13-19`) and adds it to node coordinates that already contain the delta
  (`core/vector_gcode.rs:58-90`).
* The canvas draws vector nodes without adding the offset (`ui/panels/canvas/draw.rs:566-597`).

Impact: draw a rectangle, move it 20 mm right, then `Start` or `Export`; the job runs 40 mm
right of where the canvas shows it. The same applies to vector shapes created inside an
existing layer after any move, and to pasted copies.

Fix: choose one placement representation. For vector layers, node coordinates are already
world coordinates, so `export_vector_layer` must use `(0, 0)` as origin (or, better, the
unified geometry builder from P1 #3). Add a regression test that translates a vector layer
and asserts emitted coordinates.

### 2. `dither` is a no-op

Evidence: `LayerParams::dither` (`core/layer/layer.rs:71-75`), the checkbox and tooltip
(`ui/panels/right_panel/properties.rs:185-186`), and the parameter comparison
(`:201`). `RasterizeOptions` has no dither field (`core/image/raster.rs:3-8`), and
`engrave_segments` only uses `threshold`/`invert` through `RasterImage::is_dark`
(`core/image/engraver.rs:54-115`). The value is serialised into projects and exposed in the
UI, but it is absent from the canvas cache key (`ui/panels/canvas/cache.rs:14-25`), so
toggling it does not even refresh what is drawn.

Impact: the UI promises a different engraving and the machine does the same thing.

Fix: implement threshold dithering (Floyd–Steinberg or ordered 4×4) inside rasterization, or
remove the field, the checkbox, the locale key, and the comparison branch. Removing is the
cheaper correct option for the next release.

## P1 — divergence and structural risk

### 3. Three independent layer → geometry pipelines

| Pipeline | Entry point | Used by |
| --- | --- | --- |
| Live job | `ui/widget/job.rs:39-157` (`build_job_gcode`) | Start, export, G-code viewer |
| Canvas cache | `ui/panels/canvas/cache.rs:96-190` (`build_segments`) | Canvas drawing |
| Preview | `ui/widget/preview_window.rs:223-355` (`build_layer_segments`) | Preview window |

They duplicate per-source rasterization, origin computation, mirroring, normalization, and
offset application. Already visible drift:

* **Visibility semantics.** The job includes any `is_enabled` layer
  (`ui/widget/job.rs:53-56`); preview requires `is_enabled && is_visible`
  (`ui/widget/preview_window.rs:182-184`); the canvas requires `is_visible`
  (`ui/panels/canvas/draw.rs:192-195`). A hidden but enabled layer is drawn nowhere, yet
  burned.
* **Placement.** Preview adds `layer.offset` for G-code and Gerber segments
  (`ui/widget/preview_window.rs:338-342`); the canvas cache does not and leaves the offset to
  the draw call (`ui/panels/canvas/draw.rs:329-330`); the job normalizes again against its
  own minimum (`ui/widget/job.rs:196-241`).
* **Trace threshold.** All three trace SVG lines with the hard-coded `128`
  (`ui/widget/job.rs:100`, `ui/panels/canvas/cache.rs:137`,
  `ui/widget/preview_window.rs:289`) and ignore `params.threshold`, while the raster branch
  honours it.
* **Mirroring.** Mirror is implemented for vector shapes and, in two separate places, for
  G-code (`ui/panels/canvas/mod.rs:221-293`, `ui/widget/job.rs:176-194`,
  `core/gcode/segments.rs:60-95`). For image, SVG, and Gerber it only rewrites bounds and
  offset, which can move the layer instead of mirroring it; previews never mirror at all.

Impact: the preview cannot be trusted as a preview; every new source type or parameter must
be implemented three times and kept in sync by hand.

Fix (the main refactor): one geometry function, for example

```rust
pub struct LayerGeometry { pub segments: Vec<Segment>, pub bounds: Option<Rect> }
pub fn geometry_for(layer: &Layer, mode: GeometryMode) -> Result<LayerGeometry, LayerError>;
```

with `GeometryMode` expressing the one intended policy (job vs display) instead of three.
The job then only adds program framing and passes; the canvas only rasterizes or draws the
segments; the preview only renders them.

### 4. Pass semantics are split three ways and the comments are wrong

* `machine.passes` repeats the whole job (`ui/widget/job.rs:46-51`).
* `layer.params.passes` is consumed inside each generator
  (`core/image/engraver.rs:54`, `core/vector_gcode.rs:47`, `ui/widget/job.rs:220`,
  `ui/widget/job.rs:273`).
* `engrave_layer` additionally emits `; --- pass N ---` comments
  (`core/image/engraver.rs:142-144`) even though all passes were already emitted by
  `engrave_segments`, so comments do not correspond to the cuts that follow.

Impact: two different "passes" controls in the UI, confusing output, and comments that
mislead anyone reading an exported file. With `machine.passes = 3` and per-layer passes = 3,
the job does 9 passes.

Fix: keep one job-level pass loop and one layer-level pass count; emit a comment before each
layer pass block from the code that actually repeats the block; document the multiplication
in README.

### 5. G-code is assembled by string formatting in six places

`GcodeCommand` carries parsed fields (`core/gcode/command.rs`), but the job consumes only
`cmd.raw` (`ui/widget/job.rs:145-147`), and the warning report confirms the parsed variants
are never constructed. Construction helpers are copied verbatim:

| Helper | Copies |
| --- | --- |
| `make_cmd` / `mk` closure | `core/image/engraver.rs:157`, `core/vector_gcode.rs:112`, `core/image/svg_vector.rs:111`, `core/image/svg_trace.rs:343`, `ui/widget/job.rs:388` |
| Laser-on header (`S_MAX = 1000`, `M3`/`M4`) | `core/image/engraver.rs:126-133`, `core/vector_gcode.rs:38-39,72-76`, `core/image/svg_vector.rs:107-108,146-150`, `core/image/svg_trace.rs:339-341,379-381`, `ui/widget/job.rs:206-212,260-266` |
| `G21`/`G90`/`G1 F` preamble | the same five places plus `ui/widget/job.rs:42-44` |
| `layer_origin` | `ui/widget/job.rs:13`, `ui/panels/canvas/cache.rs:192`, `ui/widget/preview_window.rs:215` |

Impact: power/feed changes must be made in six places; a future `S_MAX` or laser-mode change
will miss at least one path.

Fix: `core/gcode/builder.rs` with a `Program` type (`header`, `laser_on(power, dynamic)`,
`rapid`, `cut`, `laser_off`, `comment`), and `Segment` as a named type instead of
`(f64, f64, f64, f64)` quadruples. Keep `GcodeCommand` only if parsed fields are actually
consumed; otherwise delete it and emit `String` lines.

### 6. Three G-code parsers with three behaviours

| Parser | Location | Behaviour |
| --- | --- | --- |
| Bounds scan | `ui/project_io.rs:292-336` | Reads every `X`/`Y` token, including rapid moves, so imported-layer bounds are too large |
| Segment parser | `core/gcode/segments.rs:5-51` | Skips rapid moves, tracks `G0`/`G1`, treats `G2`/`G3` as straight lines |
| Job parser | `ui/widget/job.rs:340-386` | Same as above, copied |

`core/gcode/parser.rs` parses `G2`/`G3` (`:68-77`) and `I`/`J` (`:83-84`) but nothing uses the
result. No parser interpolates arcs, and relative (`G91`) coordinates are not handled.

Impact: imported G-code with arcs is burned as chords; layer bounds and fit-to-view can be
wrong; three copies to fix for every G-code dialect detail.

Fix: one parser producing `Segment`s with an explicit motion model, including a documented
decision about arcs (either interpolate or reject with a clear log line). `project_io` should
reuse it for bounds.

### 7. Undo does not cover the layer list or parameter edits

Evidence: delete, reorder, and visibility/enabled toggles mutate `Project` directly without
`push_history` (`ui/panels/right_panel/layers.rs:89-130`), while canvas delete does push
(`ui/panels/canvas/input.rs:532-553`, `ui/panels/right_panel/properties.rs:7-25`). In the
properties panel only the position edit pushes (`ui/panels/right_panel/properties.rs:99-104`);
name, colour, power, feed, passes, threshold, and the other parameters are written straight
into the layer (`:55-64`, `:122-206`). Drag and mirror push once per gesture, which is correct.

Impact: removing a layer with the trash button or changing power cannot be undone, and the
undo stack no longer matches the document.

Fix: make document mutation go through `Project` methods that take `&mut History`, or add a
single `commit` wrapper. Until then, add `push_history` to the layer-list branches and to the
parameter commit block.

### 8. `LayerParams` edit uses clone-and-compare-nine-fields

Evidence: `ui/panels/right_panel/properties.rs:28-40` clones params, `:191-206` compares nine
fields manually and only then assigns `layer.params = new_params`.

Impact: any new parameter added to `LayerParams` is silently discarded unless someone also
extends the comparison. The clone also churns memory every frame the panel is open.

Fix: edit `&mut layer.params` in place (borrow split is manageable by extracting the panel
body into a helper taking `&mut LayerParams`), or give `LayerParams` a `revision`/dirty flag.
The colour and name fields already write directly, so in-place editing is consistent with
the existing style.

### 9. Index-based selection is fragile

Evidence: `Selection` stores `usize` indices (`ui/selection.rs:4-9`); removal renumbers them
by hand (`ui/panels/right_panel/layers.rs:105-113`), and the properties panel re-checks bounds
every frame (`:208`). Reordering layers invalidates `selection.layers` silently.

Impact: stale indices select the wrong layer after list edits; every new operation must
repeat the fixup.

Fix: `LayerId(u64)` on `Layer`, `Vec<Layer>` plus `HashMap<LayerId, usize>` or an id→index
lookup; selection stores ids, resolved at the frame boundary. This is a mechanical change
that can be done source by source.

### 10. UI strings bypass localisation

Evidence: 18 hard-coded `ui.label`/`ui.button`/`ui.checkbox` calls, including panel tabs
(`ui/panels/right_panel/mod.rs:51`), properties labels (`properties.rs:56,157`), machine panel
(`machine.rs:12,44,118,149,161,173`), canvas context menu
(`ui/panels/canvas/mod.rs:84-134`), tool hints (`core/tools/tool.rs:25-33`), and window
toolbars (`preview_window.rs:111-115`, `gcode_window.rs:35-43`). The backend also tries to
load `fr.toml` (`src/i18n_backend.rs:19`), which does not exist, and the settings window lists
only `en`/`ru` (`ui/widget/settings_window.rs:17`).

Impact: Russian UI is partially English; the `fr` reference is dead weight.

Fix: add keys for the visible strings (locale files currently have exact 138/138 parity, so
keep them in lockstep), remove `fr`, and derive the locale list from one constant shared by
the backend and settings.

### 11. Settings that persist but do nothing

| Setting | Evidence | Reality |
| --- | --- | --- |
| `app_theme` | `ui/widget/settings_window.rs:150-174`, no other read | Never applied; only the selector reads it |
| `window_width`/`window_height` | written each frame `ui/mod.rs:191-192`, default `app_config.rs:22-27` | Never used; viewport is fixed at 1200×720 (`main.rs:64`) |

Impact: users change a setting, it is saved, and nothing happens. README already lists the
theme as a known limitation; the window size is undocumented.

Fix: apply the theme in `LaserApp::ui` (a `match` on `AppTheme` setting `ctx.set_visuals`)
and restore window size in the viewport builder, or remove both fields.

## P2 — hygiene, dead code, packaging

### 12. Dead code and unused dependencies

`cargo check` reports 53 warnings: 38 dead-code, 14 unused imports, and 1 deprecated call. The
largest clusters:

| Item | Location |
| --- | --- |
| Entire unused modules | `core/image/palette.rs` (no caller), `core/image/svg_vector.rs` (alternate SVG path never called), `core/gcode/stream.rs` (`GcodeStream`) |
| Dead parser API | `core/gcode/parser.rs:27` `parse_file`, `parse_line`; `GcodeCommand` methods |
| Dead geometry | `core/geometry.rs` `Rect` and most `Point2d` methods; `BoundingBox::update_from_point`/`update_from_bounds`/`get_center` |
| Dead layer API | `LayerUnit` (`core/layer/layer.rs:6`), `Layer::is_raster` |
| Dead UI state | `DragMode::Pan` (`ui/panels/canvas/state.rs:14`), `Selection::hit_node`/`hit_handle`, `History::clear`, `GRID_STEPS`, `PATH_LAYER`, unused `LaserApp` fields (`events_tx`, `config_folder`) |
| Dead local | `new_center` computed and discarded in `ui/panels/canvas/mod.rs:233-239` |
| Parsed but never shown | `GrblStatus::wpos`, `feed`, `spindle` (`comm/status.rs:50-53`); only `mpos` reaches the UI (`ui/panels/bottom_bar.rs:18`) |

Unused dependencies flagged by Cargo: `directories`, `dirs`, `lyon`, `palette` (the last is
unrelated to the internal `palette` module). `dirs` is flagged although `src/dirs.rs` exists,
because `use dirs::get_config_dir` (`main.rs:7`) resolves to the local module and the crate is
never linked. The local module also duplicates the executable-directory logic in
`src/i18n_backend.rs:13-17`.

Impact: review noise, longer builds, and misleading dependency footprint (23 direct deps for
an application this size).

Fix: delete unused modules/items, remove the four dependencies, rename `src/dirs.rs` to
`src/paths.rs`, and share one `exe_dir()` helper. Target: zero warnings.

### 13. Error handling is uneven

* 24 event sends ignore failure (`let _ = events_tx.send(...)`), and there are 47 `let _ =`
  sites in total. If the UI thread dies, the streamer keeps streaming; if the streamer dies,
  the UI only logs "streamer thread is dead" (`comm/streamer.rs:53-57`).
* `GrblEvent::Error` is used for informational text: `"Connecting to {port}..."` is sent as an
  error (`comm/streamer.rs:91`) and logged as one by the UI (`ui/mod.rs:150-152`).
* Panics: `log::set_boxed_logger(...).unwrap()` (`main.rs:31`), `clipboard.as_ref().unwrap()`
  (`ui/mod.rs:390`), `pending.pop_front().unwrap()` (`comm/streamer.rs:238`), `expect` in the
  ignored benchmark.
* File operations report errors only through the log buffer; the user-facing windows often
  show nothing (for example a failed project save exits silently after logging).

Fix: add `GrblEvent::Info` (or reuse `Connected`), replace the two reachable `unwrap`s with
`if let`, and surface save/import failures in a status line or modal instead of only the log.

### 14. `build.rs` and config location

`build.rs:5` hard-codes `target/<profile>` and ignores `CARGO_TARGET_DIR`, then `i18n_backend`
loads locales from the executable directory. `AppConfig` and projects live in `<exe>/config`
(`src/dirs.rs`), which fails under Program Files or a read-only installation; the
`directories` crate was evidently intended for this and is currently unused.

Impact: packaging and portable-install scenarios break; settings cannot be saved after an
installer places the app in a protected directory.

Fix: copy locales into `OUT_DIR` and embed them with `include_str!` (removing the runtime file
dependency and `build.rs` copy entirely), or honour `CARGO_TARGET_DIR` and document the
portable layout. Move `app_config.toml` to a platform config directory via `directories`,
keeping a portable fallback next to the executable.

### 15. Large files with mixed responsibilities

`core/image/gerber.rs` (832 lines) mixes document walking, aperture and macro resolution,
primitive scaling, clipper2 conversion, arc tessellation, and segment extraction. It has no
tests. `ui/widget/preview_window.rs` (753) mixes window chrome, geometry rebuilding, view
math, raster caching, and texture upload. `ui/panels/canvas/input.rs` (700) mixes selection,
dragging, tools, hit-testing, and deletion.

Impact: hard to review and to test; all three are the files most likely to grow.

Fix (after the geometry unification): split gerber into `walk`/`primitives`/`clip`/`segments`
submodules; split preview into `view`/`raster`/`build`; move canvas editing operations
(delete/duplicate/move) into `core`-level document commands.

### 16. Test coverage is concentrated on two pure parsers

There are no tests for engraving (pixel → runs), vector export, Gerber import, segment
normalisation, project round-trip, selection/hit-testing, or the streamer state machine. The
one rasterizer test is an ignored benchmark.

Priority tests to add before the refactor:

1. `vector_gcode`: translate a layer, assert absolute coordinates (catches P0 #1).
2. `engraver`: 4×4 known bitmap → expected runs; passes multiply segments; invert/threshold.
3. `project_io`: `Project` → TOML → `Project` equality; version mismatch path.
4. `segments`: G0 skipped, G1 kept, arcs either interpolated or rejected, relative mode.
5. Geometry golden tests shared by job/canvas/preview once P1 #3 lands; the test is the
   contract that keeps the three consumers honest.
6. Streamer state machine with a mock transport: ack window, abort on `error:`, stall
   recovery, pause/resume/stop.

## Duplication matrix

| Concept | Copies | Locations |
| --- | --- | --- |
| Layer → segments | 3 | `job.rs:39-157`, `cache.rs:96-190`, `preview_window.rs:223-355` |
| G-code text parsing | 3 | `segments.rs:5-51`, `job.rs:340-386`, `project_io.rs:292-336` |
| G-code command construction | 5 | `engraver.rs:157`, `vector_gcode.rs:112`, `svg_vector.rs:111`, `svg_trace.rs:343`, `job.rs:388` |
| Laser power header | 6 | see P1 #5 |
| `layer_origin` | 3 | `job.rs:13`, `cache.rs:192`, `preview_window.rs:215` |
| Bounds min/max scan | 5 | `job.rs:196`, `project_io.rs:262`, `cache.rs:76`, `preview_window.rs:190`, `canvas/mod.rs:192` |
| Mirror transform | 3 | `segments.rs:60-95`, `canvas/mod.rs:221-293`, `job.rs:176-194` |

## Refactoring plan

Each phase is independently shippable. Do not start a phase before its tests exist.

### Phase 0 — safety net (0.5–1 day)

* Add the tests listed above for vector export, engraver, project round-trip, and segments.
* Add `cargo clippy --all-targets` to whatever local script will later become CI.
* Record the current warning count as a baseline.

Verification: `cargo test` green with the new suites added.

### Phase 1 — correctness fixes (1–2 days)

* Vector origin: use `(0, 0)` in `export_vector_layer` or remove `offset` from vector math.
* Implement or delete `dither` (delete is recommended for this cycle).
* Fix pass comments: emit one comment per repeated block, from the loop that repeats it.
* Unify visibility policy: decide whether a hidden layer is burned; make job, preview, and
  canvas agree, then document it in README.
* Reuse one parser for `gcode_bounds` so imported bounds ignore rapid moves.
* Add `push_history` to layer-list delete/reorder and to the parameter commit block, or route
  mutations through `Project`.
* Apply or remove `app_theme` and the window-size fields.

Verification: new regression tests, no behaviour change outside the listed issues, manual
smoke test with a real or simulated controller.

### Phase 2 — one geometry pipeline (2–4 days, highest value)

* Introduce `core/render.rs`: `Segment`, `Rect`, `LayerGeometry`, `geometry_for(layer, mode)`.
* Port `cache.rs`, `preview_window.rs`, and `job.rs` to it; delete their private copies.
* Introduce `core/gcode/builder.rs` for framing, laser power, rapid/cut moves, comments.
* Replace the six power headers and five `make_cmd` copies with builder calls.
* Keep `job.rs` as orchestration (passes, layer comments, final home) only.

Verification: golden geometry tests; exported G-code for a sample project diffed before/after
and explained line by line; preview and canvas identical for the same layer.

### Phase 3 — document identity and undo (3–5 days)

* Add `LayerId`, switch `Selection` to ids, remove manual index fixups.
* Make `Project::layers` private with explicit mutators; every mutator takes the history (or
  returns a change token) so undo coverage is structural rather than remembered.
* Consider snapshot-on-edit only for small documents, or a command enum for large ones;
  measure before choosing. 100 full clones of a vector-heavy document are not free.

Verification: undo/redo property test over a randomized edit sequence.

### Phase 4 — hygiene and packaging (1–2 days)

* Delete dead modules/items and the four unused dependencies; rename `dirs` to `paths`.
* Localise the 18 hard-coded strings; remove `fr`; share the locale list.
* Fix the event-error misuse and the reachable `unwrap`s.
* Decide the config/locale location policy (`OUT_DIR` embedding or platform directories).

Verification: zero compiler warnings, README and PROJECT_MAP updated, new user-facing strings
present in both locales.

### Phase 5 — optional splits

* `gerber.rs` into walker/primitives/clip/segments; add its first tests with a tiny synthetic
  Gerber document.
* `preview_window.rs` into view/raster/build.
* `canvas/input.rs` editing operations into core commands.

## What not to touch

* The GRBL character-counting protocol and stall recovery in `comm/streamer.rs`: it is the
  part with real hardware semantics and no test harness; change it only with a mock serial
  transport and a real machine session.
* `core/image/svg_size.rs`: it is the best-tested module (10 tests) and handles real vendor
  quirks; the refactor should not rewrite it.
* The texture/LOD rendering strategy in `canvas/draw.rs` and `line_raster.rs`: it exists for
  performance with huge jobs; measure before changing, and keep the ignored benchmark as the
  yardstick.

## Acceptance criteria for the whole effort

1. `cargo check --all-targets` emits zero warnings.
2. One function produces geometry for a layer; `grep` finds no second implementation.
3. `cargo test` covers vector export, engraver, segments, project round-trip, and the
   streamer state machine.
4. Preview, canvas, and exported G-code agree for a project containing one layer of every
   source type, including hidden/disabled combinations and a moved vector layer.
5. README has no claim that the code contradicts.
