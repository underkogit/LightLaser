use rust_i18n::t;

use crate::comm::StreamerCommand;
use crate::core::gcode::GcodeCommand;
use crate::core::image::{
    RasterizeOptions, engrave_layer, load_image, rasterize, rasterize_svg_into_box,
    svg_raster_box, svg_trace,
};
use crate::core::layer::{EngraveMode, Layer, LayerSource};
use crate::core::vector_gcode::export_vector_layer;
use crate::ui::LaserApp;

fn layer_origin(layer: &Layer) -> (f64, f64) {
    if layer.is_vector() {
        (layer.offset[0], layer.offset[1])
    } else {
        layer.bounds().map(|b| (b.0, b.1)).unwrap_or((0.0, 0.0))
    }
}

impl LaserApp {
    pub(crate) fn start_job(&mut self) {
        let lines = match self.build_job_gcode() {
            Ok(lines) => lines,
            Err(e) => {
                log::error!("{}", t!("ui.error.build_job", e = e));
                return;
            }
        };

        if lines.is_empty() {
            log::warn!("{}", t!("ui.warn.empty_job"));
            return;
        }

        self.streamer.send(StreamerCommand::Start { lines });
    }

    fn build_job_gcode(&self) -> Result<Vec<String>, String> {
        let mut lines: Vec<String> = Vec::new();

        lines.push("G21 ; mm".to_string());
        lines.push("G90 ; absolute".to_string());
        lines.push("G17 ; XY plane".to_string());

        // `machine.passes` repeats the whole job. Each layer's own pass count still applies
        // within a single job pass.
        for pass in 0..self.project.machine.passes.max(1) {
            if pass > 0 {
                lines.push(format!("; --- job pass {} ---", pass + 1));
            }

            for (i, layer) in self.project.layers.iter().enumerate() {
                if !layer.is_enabled {
                    continue;
                }

                lines.push(format!("; --- layer #{}: {} ---", i + 1, layer.name));

                let origin = layer_origin(layer);
                let width_mm = layer
                    .bounds()
                    .map(|b| (b.2 - b.0).max(1.0))
                    .unwrap_or(100.0);
                let height_mm = layer
                    .bounds()
                    .map(|b| (b.3 - b.1).max(1.0))
                    .unwrap_or(100.0);

                let layer_cmds: Vec<GcodeCommand> = match &layer.source {
                    LayerSource::Image { path } => {
                        let loaded = load_image(std::path::Path::new(path))
                            .map_err(|e| format!("image '{}': {}", path, e))?;

                        let raster = rasterize(
                            &loaded.dynamic,
                            RasterizeOptions {
                                pixels_per_mm: layer.params.pixels_per_mm,
                                threshold: layer.params.threshold,
                                invert: layer.params.invert,
                            },
                            Some(width_mm),
                        );

                        engrave_layer(&raster, &layer.params, origin)
                            .map_err(|e| format!("engrave '{}': {}", path, e))?
                    }
                    LayerSource::Svg { path } => {
                        if layer.params.engrave_mode == EngraveMode::Lines {
                            let target_px = (layer.params.pixels_per_mm * width_mm)
                                .round()
                                .clamp(512.0, 4096.0) as u32;

                            let img = svg_trace::rasterize_svg_to_image(
                                std::path::Path::new(path),
                                target_px,
                            )
                            .map_err(|e| format!("svg '{}': {}", path, e))?;

                            let paths = svg_trace::trace_image_to_paths(&img, 128);

                            let scale_x = width_mm / target_px as f64;
                            let scale_y = (width_mm * (img.height() as f64 / img.width() as f64))
                                / img.height() as f64;

                            svg_trace::traced_paths_to_gcode(
                                &paths,
                                &layer.params,
                                origin,
                                scale_x,
                                scale_y,
                            )
                        } else {
                            let target_px = (layer.params.pixels_per_mm * width_mm)
                                .round()
                                .clamp(64.0, 4096.0) as u32;

                            // The raster carries the layer's aspect ratio, so the engraving
                            // lands exactly where the canvas shows the layer.
                            let (box_w, box_h) = svg_raster_box(width_mm, height_mm, target_px);
                            let image =
                                rasterize_svg_into_box(std::path::Path::new(path), box_w, box_h)
                                    .map_err(|e| format!("svg '{}': {}", path, e))?;

                            let raster = rasterize(
                                &image,
                                RasterizeOptions {
                                    pixels_per_mm: layer.params.pixels_per_mm,
                                    threshold: layer.params.threshold,
                                    invert: layer.params.invert,
                                },
                                Some(width_mm),
                            );

                            engrave_layer(&raster, &layer.params, origin)
                                .map_err(|e| format!("engrave '{}': {}", path, e))?
                        }
                    }
                    LayerSource::Gcode { path } => build_gcode_layer(path, layer)?,
                    LayerSource::Gerber { path } => build_gerber_layer(path, layer)?,
                    LayerSource::Vector => export_vector_layer(layer, &layer.params, origin)
                        .map_err(|e| format!("vector '{}': {}", layer.name, e))?,
                };

                for cmd in layer_cmds {
                    lines.push(cmd.raw);
                }

                lines.push("M5".to_string());
            }
        }

        lines.push("G0 X0 Y0 ; return home".to_string());
        lines.push("M5 ; laser off".to_string());

        Ok(lines)
    }

    pub(crate) fn export_job_gcode(&self) -> Result<String, String> {
        let lines = self.build_job_gcode()?;
        Ok(lines.join("\n"))
    }
}

fn build_gcode_layer(path: &str, layer: &Layer) -> Result<Vec<GcodeCommand>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("gcode '{}': {}", path, e))?;

    let raw = parse_gcode_raw(&content);

    if raw.is_empty() {
        return Ok(Vec::new());
    }

    let mut segs = raw;

    if layer.mirror_x || layer.mirror_y {
        if let Some(center) = layer.mirror_center {
            for s in &mut segs {
                let (mut ax, mut ay, mut bx, mut by) = (s.0, s.1, s.2, s.3);
                if layer.mirror_x {
                    ax = 2.0 * center[0] - ax;
                    bx = 2.0 * center[0] - bx;
                }
                if layer.mirror_y {
                    ay = 2.0 * center[1] - ay;
                    by = 2.0 * center[1] - by;
                }
                s.0 = ax;
                s.1 = ay;
                s.2 = bx;
                s.3 = by;
            }
        }
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for (x0, y0, x1, y1) in &segs {
        min_x = min_x.min(*x0).min(*x1);
        min_y = min_y.min(*y0).min(*y1);
    }

    let ox = layer.offset[0];
    let oy = layer.offset[1];

    let s_max = 1000.0;
    let s_on = ((layer.params.power_percent.clamp(0.0, 100.0) / 100.0) * s_max).round();
    let laser_on = if layer.params.dynamic_power {
        format!("M4 S{:.0}", s_on)
    } else {
        format!("M3 S{:.0}", s_on)
    };

    let mut cmds = Vec::new();
    cmds.push(make_cmd("G21".to_string()));
    cmds.push(make_cmd("G90".to_string()));
    cmds.push(make_cmd(laser_on));
    cmds.push(make_cmd(format!("G1 F{:.0}", layer.params.feed_rate)));

    for pass in 0..layer.params.passes.max(1) {
        if pass > 0 {
            cmds.push(make_cmd("; --- next pass ---".to_string()));
        }
        for (x0, y0, x1, y1) in &segs {
            cmds.push(make_cmd(format!(
                "G0 X{:.4} Y{:.4}",
                x0 - min_x + ox,
                y0 - min_y + oy
            )));
            cmds.push(make_cmd(format!(
                "G1 X{:.4} Y{:.4}",
                x1 - min_x + ox,
                y1 - min_y + oy
            )));
        }
    }

    cmds.push(make_cmd("M5".to_string()));

    Ok(cmds)
}

/// Turns a Gerber file into laser moves.
///
/// Segments that share an endpoint are chained into polylines first, so the laser is
/// switched on once per continuous trace instead of once per segment.
fn build_gerber_layer(path: &str, layer: &Layer) -> Result<Vec<GcodeCommand>, String> {
    let segments = crate::core::image::parse_gerber_segments(std::path::Path::new(path))
        .map_err(|e| format!("gerber '{}': {}", path, e))?;

    if segments.is_empty() {
        return Ok(Vec::new());
    }

    let polylines = chain_segments(&segments);

    let ox = layer.offset[0];
    let oy = layer.offset[1];

    let s_max = 1000.0;
    let s_on = ((layer.params.power_percent.clamp(0.0, 100.0) / 100.0) * s_max).round();
    let laser_on = if layer.params.dynamic_power {
        format!("M4 S{:.0}", s_on)
    } else {
        format!("M3 S{:.0}", s_on)
    };

    let mut cmds = Vec::new();
    cmds.push(make_cmd("G21".to_string()));
    cmds.push(make_cmd("G90".to_string()));
    cmds.push(make_cmd(format!("G1 F{:.0}", layer.params.feed_rate)));

    for pass in 0..layer.params.passes.max(1) {
        if pass > 0 {
            cmds.push(make_cmd("; --- next pass ---".to_string()));
        }

        for polyline in &polylines {
            let first = polyline[0];

            cmds.push(make_cmd("M5".to_string()));
            cmds.push(make_cmd(format!(
                "G0 X{:.4} Y{:.4}",
                first.0 + ox,
                first.1 + oy
            )));
            cmds.push(make_cmd(laser_on.clone()));

            for point in polyline.iter().skip(1) {
                cmds.push(make_cmd(format!(
                    "G1 X{:.4} Y{:.4}",
                    point.0 + ox,
                    point.1 + oy
                )));
            }

            cmds.push(make_cmd("M5".to_string()));
        }
    }

    cmds.push(make_cmd("M5".to_string()));

    Ok(cmds)
}

/// Joins segments whose end point is the next segment's start point.
fn chain_segments(segments: &[(f64, f64, f64, f64)]) -> Vec<Vec<(f64, f64)>> {
    const EPS: f64 = 1e-9;

    let mut polylines: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut current: Vec<(f64, f64)> = Vec::new();

    for &(x0, y0, x1, y1) in segments {
        let start = (x0, y0);
        let end = (x1, y1);

        let continues = current
            .last()
            .is_some_and(|last| (last.0 - start.0).abs() <= EPS && (last.1 - start.1).abs() <= EPS);

        if continues {
            current.push(end);
        } else {
            if current.len() > 1 {
                polylines.push(std::mem::take(&mut current));
            }
            current.clear();
            current.push(start);
            current.push(end);
        }
    }

    if current.len() > 1 {
        polylines.push(current);
    }

    polylines
}

fn parse_gcode_raw(content: &str) -> Vec<(f64, f64, f64, f64)> {
    let mut raw = Vec::new();
    let mut cur = (0.0f64, 0.0f64);
    let mut mode_rapid = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        let mut nx = None;
        let mut ny = None;
        let mut seen_g = None;

        for tok in line.split_whitespace() {
            if let Some(rest) = tok.strip_prefix('G') {
                if let Ok(g) = rest.parse::<u32>() {
                    seen_g = Some(g);
                }
            } else if let Some(rest) = tok.strip_prefix('X') {
                nx = rest.parse::<f64>().ok();
            } else if let Some(rest) = tok.strip_prefix('Y') {
                ny = rest.parse::<f64>().ok();
            }
        }

        if let Some(g) = seen_g {
            if g == 0 {
                mode_rapid = true;
            } else if g == 1 || g == 2 || g == 3 {
                mode_rapid = false;
            }
        }

        let px = nx.unwrap_or(cur.0);
        let py = ny.unwrap_or(cur.1);

        if !mode_rapid && (px != cur.0 || py != cur.1) {
            raw.push((cur.0, cur.1, px, py));
        }

        cur = (px, py);
    }

    raw
}

fn make_cmd(raw: String) -> GcodeCommand {
    GcodeCommand {
        raw,
        line_number: None,
        motion: None,
        x: None,
        y: None,
        z: None,
        i: None,
        j: None,
        feed: None,
        spindle: None,
        comment: None,
    }
}
