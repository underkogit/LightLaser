# LightLazer

![LightLazer](images/LightLaser_aQjJS8K7E0.png)
![LightLazer](images/LightLaser_kVOATA2Uyo.png)

LightLazer is a desktop laser engraving and cutting control application written in Rust. It provides a graphical interface for composing jobs from images, SVG files, G-code files, and vector drawing tools, then streaming the resulting G-code to a GRBL-based laser controller over a serial port.

## Features

- Cross-platform GUI built with `eframe` / `egui`.
- GRBL communication over serial with a streaming sender that respects the controller RX buffer.
- Live status polling (machine position, work position, feed, spindle).
- Job controls: start, pause, resume, stop, home, unlock.
- Jog controls with configurable step and feed rate.
- Layer system:
    - Image layers with raster engraving parameters (power, feed rate, passes, pixels per millimeter, threshold, dither, invert).
    - Vector layers drawn with line, curve, rectangle, and circle tools.
    - SVG and G-code layers as imported sources.
- Layer visibility, enable/disable, reordering, renaming, color editing, duplication, and deletion.
- Project save/load in TOML format with version checking.
- G-code export of the full job to a `.nc` file.
- Undo/redo history for project edits.
- Configurable work area, grid, axes, snap-to-grid, and theme.
- Internationalization with English and Russian locales.
- Logging to console and an in-app log panel.

## Requirements

- Rust toolchain with edition 2024 support.
- A GRBL-compatible laser controller connected via a serial port (optional for offline job preparation and export).

## Building

```
cargo build --release
```

The build script copies locale files from `locales/` into the target output directory so they are available at runtime next to the executable.

## Running

```
cargo run --release
```

## Usage Overview

1. Open the application. The main window contains a tool panel on the left, the canvas in the center, the layer/parameters panel on the right, and the log panel at the bottom.
2. Add content:
    - `File > Add image` to create image layers.
    - `File > Add SVG` to import SVG files.
    - `File > Add G-code` to import existing G-code.
    - Use the drawing tools to create vector shapes directly on the canvas.
3. Select a layer in the right panel and adjust its parameters (power, feed rate, passes, and image-specific options where applicable).
4. Toggle layer visibility and enabled state as needed. Only enabled layers are included in jobs and exports.
5. Connect to the machine via the connection dialog, selecting the serial port and baud rate.
6. Use `Start` to stream the job to the controller, or `File > Export G-code` to save it to a file.
7. Save the project with `File > Save project` (TOML format).

## Project Files

Projects are stored as TOML and include:

- Project version and name.
- Machine configuration (port, baud rate, work area, jog settings, laser mode, soft limits, homing).
- Layers with source type, color, visibility, enabled state, parameters, bounds, vector shapes, and offset.

## Configuration

Application settings are stored in `config/app_config.toml` next to the executable. Settings include window size, locale, theme, grid step, and visibility toggles for grid, axes, and work area.

## Locales

Locale files are TOML dictionaries located in `locales/`. The following locales are supported:

- `en.toml` (English)
- `ru.toml` (Russian)

## Keyboard Shortcuts

- `Ctrl+Z` — undo
- `Ctrl+Shift+Z` or `Ctrl+Y` — redo
- `Ctrl+S` — save project
- `Ctrl+Shift+S` — save project as
- `Ctrl+O` — open project
- `Ctrl+N` — new project
- `Ctrl+E` — export G-code
- `Ctrl+D` — duplicate selection
- `Delete` / `Backspace` — delete selection
- `Escape` — cancel current tool action
- `Enter` — finish current tool action

## Project Structure

- `src/comm` — serial connection, GRBL protocol, status parsing, and job streamer.
- `src/core` — geometry, bounding boxes, project and layer model, image rasterization and engraving, G-code parsing and generation, vector paths, and drawing tools.
- `src/ui` — application state, panels, canvas rendering and input, dialogs, history, selection, and widgets.
- `src/i18n_backend.rs` — custom locale backend that loads TOML translation files.
- `src/logger.rs` — logger that writes to both console and an in-memory buffer for the UI.
- `src/dirs.rs` — resolution of the configuration directory.
- `locales/` — translation files.
- `build.rs` — copies locale files to the output directory.

## License

Not specified.