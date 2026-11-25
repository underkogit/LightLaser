use crate::core::gcode::GcodeCommand;
use crate::core::layer::{Layer, LayerSource, Project, Segment, layer_segments, placement};
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

fn layer_commands(layer: &Layer, spindle_max: f64) -> Result<Vec<GcodeCommand>, String> {
    let origin = placement(layer);

    if layer.is_vector() {
        return export_vector_layer(layer, &layer.params, origin, spindle_max)
            .map_err(|e| format!("vector '{}': {}", layer.name, e));
    }

    let segments = layer_segments(layer)?;

    if matches!(layer.source, LayerSource::Gerber { .. }) {
        return Ok(gerber_commands(&segments, layer, origin, spindle_max));
    }

    Ok(segment_commands(&segments, layer, origin, spindle_max))
}

fn laser_on_command(layer: &Layer, spindle_max: f64) -> String {
    let power = layer.params.spindle_value(spindle_max);
    if layer.params.dynamic_power {
        format!("M4 S{:.0}", power)
    } else {
        format!("M3 S{:.0}", power)
    }
}

fn segment_commands(
    segments: &[Segment],
    layer: &Layer,
    origin: (f64, f64),
    spindle_max: f64,
) -> Vec<GcodeCommand> {
    let mut cmds = Vec::new();
    cmds.push(make_cmd("G21".to_string()));
    cmds.push(make_cmd("G90".to_string()));
    cmds.push(make_cmd(laser_on_command(layer, spindle_max)));
    cmds.push(make_cmd(format!("G1 F{:.0}", layer.params.feed_rate)));

    for pass in 0..layer.params.passes.max(1) {
        if pass > 0 {
            cmds.push(make_cmd("; --- next pass ---".to_string()));
        }

        for (x0, y0, x1, y1) in segments {
            cmds.push(make_cmd(format!(
                "G0 X{:.4} Y{:.4}",
                x0 + origin.0,
                y0 + origin.1
            )));
            cmds.push(make_cmd(format!(
                "G1 X{:.4} Y{:.4}",
                x1 + origin.0,
                y1 + origin.1
            )));
        }
    }

    cmds.push(make_cmd("M5".to_string()));
    cmds
}

fn gerber_commands(
    segments: &[Segment],
    layer: &Layer,
    origin: (f64, f64),
    spindle_max: f64,
) -> Vec<GcodeCommand> {
    let polylines = chain_segments(segments);
    let laser_on = laser_on_command(layer, spindle_max);

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
                first.0 + origin.0,
                first.1 + origin.1
            )));
            cmds.push(make_cmd(laser_on.clone()));

            for point in polyline.iter().skip(1) {
                cmds.push(make_cmd(format!(
                    "G1 X{:.4} Y{:.4}",
                    point.0 + origin.0,
                    point.1 + origin.1
                )));
            }

            cmds.push(make_cmd("M5".to_string()));
        }
    }

    cmds.push(make_cmd("M5".to_string()));
    cmds
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

    #[test]
    fn power_scales_with_the_controller_full_scale() {
        let dir = temp_dir("power-scale");

        let path = write_png(&dir, "solid.png", 32, 32, |_, _| 0);

        let mut project = Project::new();
        let mut layer = image_layer(&path, [0.0, 0.0, 8.0, 8.0], 4.0);
        layer.params.power_percent = 25.0;
        project.add_layer(layer);

        let (lines, _) = build_job(&project, 4000.0, None).unwrap();

        assert!(lines.iter().any(|line| line == "M4 S1000"));
    }

    #[test]
    fn image_layer_is_engraved_at_its_bounds() {
        let dir = temp_dir("image-bounds");

        let path = write_png(&dir, "solid.png", 8, 8, |_, _| 0);

        let mut project = Project::new();
        project.add_layer(image_layer(&path, [12.0, 7.0, 20.0, 15.0], 4.0));

        let (lines, _) = build_job(&project, 1000.0, None).unwrap();

        assert!(
            lines.iter().any(|line| line == "G0 X12.0000 Y7.0000"),
            "{:?}",
            lines
        );
    }

    fn write_svg(dir: &Path, name: &str) -> String {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10mm" height="10mm" viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10" fill="white"/><rect x="0" y="0" width="1" height="10" fill="black"/><rect x="8" y="0" width="2" height="10" fill="black"/></svg>"##;
        let path = dir.join(name);
        std::fs::write(&path, svg).unwrap();
        path.to_string_lossy().to_string()
    }

    #[test]
    fn mirrored_svg_is_engraved_mirrored() {
        let dir = temp_dir("svg-mirror");

        let path = write_svg(&dir, "part.svg");

        let mut project = Project::new();
        let mut layer = Layer::new("svg".to_string(), LayerSource::Svg {
            path: path.clone(),
        });
        layer.set_bounds(0.0, 0.0, 10.0, 10.0);
        layer.params.pixels_per_mm = 8.0;
        project.add_layer(layer);
        let id = project.layers[0].id;

        let (plain, _) = build_job(&project, 1000.0, None).unwrap();
        let plain_x = move_x_values(&plain);
        assert!(has_value(&plain_x, 8.0), "{:?}", plain_x);
        assert!(!has_value(&plain_x, 2.0), "{:?}", plain_x);

        assert!(project.mirror_layers(&[id], true));
        let (mirrored, _) = build_job(&project, 1000.0, None).unwrap();
        let mirrored_x = move_x_values(&mirrored);

        assert!(has_value(&mirrored_x, 2.0), "{:?}", mirrored_x);
        assert!(!has_value(&mirrored_x, 8.0), "{:?}", mirrored_x);
    }

    fn has_value(values: &[f64], expected: f64) -> bool {
        values.iter().any(|value| (value - expected).abs() < 0.3)
    }

    fn move_x_values(lines: &[String]) -> Vec<f64> {
        lines
            .iter()
            .filter(|line| line.starts_with("G0 X") || line.starts_with("G1 X"))
            .filter_map(|line| {
                line.split_whitespace()
                    .find_map(|token| token.strip_prefix('X'))
                    .and_then(|value| value.parse::<f64>().ok())
            })
            .collect()
    }

    #[test]
    fn mirrored_gcode_is_engraved_mirrored() {
        let dir = temp_dir("gcode-mirror");

        let path = dir.join("part.nc");
        std::fs::write(&path, "G21\nG90\nG0 X0 Y0\nG1 X0 Y1\nG1 X2 Y1\n").unwrap();

        let mut project = Project::new();
        let mut layer = Layer::new("gcode".to_string(), LayerSource::Gcode {
            path: path.to_string_lossy().to_string(),
        });
        layer.set_bounds(0.0, 0.0, 2.0, 1.0);
        project.add_layer(layer);

        let (plain, _) = build_job(&project, 1000.0, None).unwrap();
        assert_eq!(cut_x_values(&plain), vec![0.0, 2.0]);

        project.layers[0].mirror_x = true;
        let (mirrored, _) = build_job(&project, 1000.0, None).unwrap();

        assert_eq!(cut_x_values(&mirrored), vec![2.0, 0.0]);
    }

    fn cut_x_values(lines: &[String]) -> Vec<f64> {
        lines
            .iter()
            .filter(|line| line.starts_with("G1 X"))
            .filter_map(|line| {
                line.split_whitespace()
                    .find_map(|token| token.strip_prefix('X'))
                    .and_then(|value| value.parse::<f64>().ok())
            })
            .collect()
    }
}
