use crate::core::gcode::GcodeCommand;
use crate::core::image::{
    RasterizeOptions, engrave_layer, load_image, rasterize, rasterize_svg_into_box, svg_raster_box,
    svg_trace,
};
use crate::core::layer::{EngraveMode, Layer, LayerSource, Project};
use crate::core::vector_gcode::export_vector_layer;

#[derive(Debug, Clone)]
pub struct LayerReport {
    pub index: usize,
    pub name: String,
    pub kind: &'static str,
    pub laser_power: f64,
    pub power_percent: f64,
    pub feed_rate: f64,
    pub passes: u32,
    pub moves: usize,
    pub cut_length: f64,
}

impl LayerReport {
    pub fn is_silent(&self) -> bool {
        self.moves == 0 || self.cut_length <= 1e-6
    }
}

pub fn build_job(
    project: &Project,
    spindle_max: f64,
    laser_mode: Option<bool>,
) -> Result<(Vec<String>, Vec<LayerReport>), String> {
    let mut lines: Vec<String> = Vec::new();
    let mut reports: Vec<LayerReport> = Vec::new();

    lines.push("G21 ; mm".to_string());
    lines.push("G90 ; absolute".to_string());
    lines.push("G17 ; XY plane".to_string());

    for pass in 0..project.machine.passes.max(1) {
        if pass > 0 {
            lines.push(format!("; --- job pass {} ---", pass + 1));
        }

        for (i, layer) in project.layers.iter().enumerate() {
            if !project.layer_enabled(layer) {
                continue;
            }

            lines.push(format!("; --- layer #{}: {} ---", i + 1, layer.name));

            let layer_cmds = layer_commands(layer, spindle_max)?;

            if pass == 0 {
                reports.push(report_for_layer(i, layer, &layer_cmds));
            }

            for cmd in layer_cmds {
                lines.push(cmd.raw);
            }

            lines.push("M5".to_string());
        }
    }

    lines.push("G0 X0 Y0 ; return home".to_string());
    lines.push("M5 ; laser off".to_string());

    if laser_mode == Some(false) {
        for line in &mut lines {
            if line.starts_with("M4") {
                *line = format!("M3{}", &line[2..]);
            }
        }
    }

    Ok((lines, reports))
}

/// Where a layer's coordinates start. Vector shapes already carry their moved position, so
/// only raster and segment layers are placed by their bounds.
fn layer_origin(layer: &Layer) -> (f64, f64) {
    if layer.is_vector() {
        (0.0, 0.0)
    } else {
        layer.bounds().map(|b| (b.0, b.1)).unwrap_or((0.0, 0.0))
    }
}

fn layer_commands(layer: &Layer, spindle_max: f64) -> Result<Vec<GcodeCommand>, String> {
    let origin = layer_origin(layer);
    let width_mm = layer
        .bounds()
        .map(|b| (b.2 - b.0).max(1.0))
        .unwrap_or(100.0);
    let height_mm = layer
        .bounds()
        .map(|b| (b.3 - b.1).max(1.0))
        .unwrap_or(100.0);

    match &layer.source {
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

            engrave_layer(&raster, &layer.params, origin, spindle_max)
                .map_err(|e| format!("engrave '{}': {}", path, e))
        }
        LayerSource::Svg { path } => {
            if layer.params.engrave_mode == EngraveMode::Lines {
                let target_px = (layer.params.pixels_per_mm * width_mm)
                    .round()
                    .clamp(512.0, 4096.0) as u32;

                let img =
                    svg_trace::rasterize_svg_to_image(std::path::Path::new(path), target_px)
                        .map_err(|e| format!("svg '{}': {}", path, e))?;

                let paths = svg_trace::trace_image_to_paths(&img, 128);

                let scale_x = width_mm / target_px as f64;
                let scale_y =
                    (width_mm * (img.height() as f64 / img.width() as f64)) / img.height() as f64;

                Ok(svg_trace::traced_paths_to_gcode(
                    &paths,
                    &layer.params,
                    origin,
                    scale_x,
                    scale_y,
                    spindle_max,
                ))
            } else {
                let target_px = (layer.params.pixels_per_mm * width_mm)
                    .round()
                    .clamp(64.0, 4096.0) as u32;

                let (box_w, box_h) = svg_raster_box(width_mm, height_mm, target_px);
                let image = rasterize_svg_into_box(std::path::Path::new(path), box_w, box_h)
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

                engrave_layer(&raster, &layer.params, origin, spindle_max)
                    .map_err(|e| format!("engrave '{}': {}", path, e))
            }
        }
        LayerSource::Gcode { path } => {
            build_gcode_layer(path, layer, spindle_max).map_err(|e| e.to_string())
        }
        LayerSource::Gerber { path } => {
            build_gerber_layer(path, layer, spindle_max).map_err(|e| e.to_string())
        }
        LayerSource::Vector => export_vector_layer(layer, &layer.params, origin, spindle_max)
            .map_err(|e| format!("vector '{}': {}", layer.name, e)),
    }
}

fn report_for_layer(index: usize, layer: &Layer, cmds: &[GcodeCommand]) -> LayerReport {
    let mut laser_power: f64 = 0.0;
    let mut moves = 0;
    let mut cut_length = 0.0;
    let mut pos = (0.0f64, 0.0f64);

    for cmd in cmds {
        let raw = cmd.raw.trim();
        if let Some(rest) = raw.strip_prefix("M3").or_else(|| raw.strip_prefix("M4")) {
            if let Some(value) = s_value(rest) {
                laser_power = laser_power.max(value);
            }
            continue;
        }
        if raw.starts_with("M5") {
            continue;
        }

        let motion = raw.split_whitespace().next().unwrap_or_default();
        if motion != "G0" && motion != "G1" {
            continue;
        }

        let Some(target) = xy_of(raw) else {
            continue;
        };

        if motion == "G1" {
            moves += 1;
            cut_length += ((target.0 - pos.0).powi(2) + (target.1 - pos.1).powi(2)).sqrt();
        }

        pos = target;
    }

    LayerReport {
        index,
        name: layer.name.clone(),
        kind: layer.source.kind_label(),
        laser_power,
        power_percent: layer.params.power_percent,
        feed_rate: layer.params.feed_rate,
        passes: layer.params.passes.max(1),
        moves,
        cut_length,
    }
}

fn s_value(rest: &str) -> Option<f64> {
    rest.split_whitespace()
        .find_map(|token| token.strip_prefix('S'))
        .and_then(|value| value.parse::<f64>().ok())
}

fn xy_of(raw: &str) -> Option<(f64, f64)> {
    let mut x = None;
    let mut y = None;
    for token in raw.split_whitespace() {
        if let Some(value) = token.strip_prefix('X') {
            x = value.parse::<f64>().ok();
        } else if let Some(value) = token.strip_prefix('Y') {
            y = value.parse::<f64>().ok();
        }
    }
    Some((x?, y?))
}

fn build_gcode_layer(
    path: &str,
    layer: &Layer,
    spindle_max: f64,
) -> Result<Vec<GcodeCommand>, String> {
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

    let s_on = layer.params.spindle_value(spindle_max);
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
fn build_gerber_layer(
    path: &str,
    layer: &Layer,
    spindle_max: f64,
) -> Result<Vec<GcodeCommand>, String> {
    let segments = crate::core::image::parse_gerber_segments(std::path::Path::new(path))
        .map_err(|e| format!("gerber '{}': {}", path, e))?;

    if segments.is_empty() {
        return Ok(Vec::new());
    }

    let polylines = chain_segments(&segments);

    let ox = layer.offset[0];
    let oy = layer.offset[1];

    let s_on = layer.params.spindle_value(spindle_max);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layer::LayerSource;
    use image::GrayImage;
    use std::path::{Path, PathBuf};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lightlaser-job-{}", name));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_png(
        dir: &Path,
        name: &str,
        width: u32,
        height: u32,
        luma: impl Fn(u32, u32) -> u8,
    ) -> String {
        let mut image = GrayImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                image.put_pixel(x, y, image::Luma([luma(x, y)]));
            }
        }
        let path = dir.join(name);
        image.save(&path).unwrap();
        path.to_string_lossy().to_string()
    }

    fn image_layer(path: &str, bounds: [f64; 4], pixels_per_mm: f64) -> Layer {
        let mut layer = Layer::new("image".to_string(), LayerSource::Image {
            path: path.to_string(),
        });
        layer.set_bounds(bounds[0], bounds[1], bounds[2], bounds[3]);
        layer.params.pixels_per_mm = pixels_per_mm;
        layer
    }

    #[test]
    fn every_layer_of_a_group_is_engraved() {
        let dir = temp_dir("group");

        let mut project = Project::new();
        let group = project.create_group();

        for index in 0..3 {
            let path = write_png(&dir, &format!("solid{}.png", index), 32, 32, |_, _| 0);
            let mut layer = image_layer(&path, [10.0, 10.0, 18.0, 18.0], 4.0);
            layer.name = format!("layer {}", index + 1);
            project.add_layer_to_group(layer, group);
        }

        let (lines, reports) = build_job(&project, 1000.0, None).unwrap();

        assert_eq!(reports.len(), 3);
        assert_eq!(lines.iter().filter(|line| *line == "M4 S500").count(), 3);

        for report in &reports {
            assert!(report.laser_power > 0.0, "{:?}", report);
            assert!(!report.is_silent(), "{:?}", report);
        }
    }

    #[test]
    fn single_pixel_features_still_burn() {
        let dir = temp_dir("thin");

        let path = write_png(&dir, "thin.png", 32, 32, |x, _| {
            if x % 2 == 0 {
                0
            } else {
                255
            }
        });

        let mut project = Project::new();
        project.add_layer(image_layer(&path, [0.0, 0.0, 8.0, 8.0], 4.0));

        let (_, reports) = build_job(&project, 1000.0, None).unwrap();

        assert_eq!(reports.len(), 1);
        assert!(reports[0].moves > 0);
        assert!(reports[0].cut_length > 0.0, "{:?}", reports[0]);
    }

    #[test]
    fn disabled_layer_is_left_out_of_the_job() {
        let dir = temp_dir("disabled");

        let path = write_png(&dir, "solid.png", 32, 32, |_, _| 0);

        let mut project = Project::new();
        let group = project.create_group();
        let first = project.add_layer_to_group(image_layer(&path, [0.0, 0.0, 8.0, 8.0], 4.0), group);
        let second =
            project.add_layer_to_group(image_layer(&path, [0.0, 0.0, 8.0, 8.0], 4.0), group);
        project.layers[second].is_enabled = false;

        let (_, reports) = build_job(&project, 1000.0, None).unwrap();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].index, first);
    }

    #[test]
    fn moved_vector_layer_cuts_where_the_canvas_shows_it() {
        let mut layer = Layer::new("vector".to_string(), LayerSource::Vector);

        let mut shape = crate::core::layer::VectorShape::default();
        shape
            .nodes
            .push(crate::core::vector::PathNode::corner(0.0, 0.0));
        shape
            .nodes
            .push(crate::core::vector::PathNode::corner(10.0, 0.0));
        layer.shapes.push(shape);
        layer.recompute_bounds();
        layer.translate(5.0, 5.0);

        let mut project = Project::new();
        project.add_layer(layer);

        let (lines, reports) = build_job(&project, 1000.0, Some(true)).unwrap();

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].is_silent(), "{:?}", reports[0]);

        let cuts: Vec<&String> = lines.iter().filter(|line| line.starts_with("G1 X")).collect();
        assert!(cuts.iter().any(|line| line.contains("X15.0000")), "{:?}", cuts);
        assert!(!cuts.iter().any(|line| line.contains("X20.0000")), "{:?}", cuts);
    }

    #[test]
    fn laser_mode_off_uses_constant_power() {
        let dir = temp_dir("no-laser-mode");

        let path = write_png(&dir, "solid.png", 32, 32, |_, _| 0);

        let mut project = Project::new();
        project.add_layer(image_layer(&path, [0.0, 0.0, 8.0, 8.0], 4.0));

        let (lines, _) = build_job(&project, 1000.0, Some(false)).unwrap();

        assert!(lines.iter().any(|line| line == "M3 S500"));
        assert!(!lines.iter().any(|line| line.starts_with("M4")));
    }

    #[test]
    fn laser_mode_on_keeps_dynamic_power() {
        let dir = temp_dir("laser-mode");

        let path = write_png(&dir, "solid.png", 32, 32, |_, _| 0);

        let mut project = Project::new();
        project.add_layer(image_layer(&path, [0.0, 0.0, 8.0, 8.0], 4.0));

        let (lines, _) = build_job(&project, 1000.0, Some(true)).unwrap();

        assert!(lines.iter().any(|line| line == "M4 S500"));
    }
}
