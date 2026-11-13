use std::path::Path;

use crate::core::layer::Layer;

pub fn parse_gcode_segments(content: &str) -> Vec<(f64, f64, f64, f64)> {
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

pub fn parse_gcode_file_segments(path: &Path) -> Vec<(f64, f64, f64, f64)> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_gcode_segments(&content)
}

pub fn transform_gcode_mirror(
    segs: Vec<(f64, f64, f64, f64)>,
    mirror_x: bool,
    mirror_y: bool,
    center: Option<[f64; 2]>,
) -> Vec<(f64, f64, f64, f64)> {
    if segs.is_empty() {
        return segs;
    }

    let Some(center) = center else {
        return segs;
    };

    if !mirror_x && !mirror_y {
        return segs;
    }

    segs.into_iter()
        .map(|(x0, y0, x1, y1)| {
            let mut ax = x0;
            let mut ay = y0;
            let mut bx = x1;
            let mut by = y1;
            if mirror_x {
                ax = 2.0 * center[0] - ax;
                bx = 2.0 * center[0] - bx;
            }
            if mirror_y {
                ay = 2.0 * center[1] - ay;
                by = 2.0 * center[1] - by;
            }
            (ax, ay, bx, by)
        })
        .collect()
}

pub fn transform_for_layer(
    segs: Vec<(f64, f64, f64, f64)>,
    layer: &Layer,
) -> Vec<(f64, f64, f64, f64)> {
    transform_gcode_mirror(segs, layer.mirror_x, layer.mirror_y, layer.mirror_center)
}

pub fn normalize_gcode(mut segs: Vec<(f64, f64, f64, f64)>) -> Vec<(f64, f64, f64, f64)> {
    if segs.is_empty() {
        return segs;
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for (x0, y0, x1, y1) in &segs {
        min_x = min_x.min(*x0).min(*x1);
        min_y = min_y.min(*y0).min(*y1);
    }

    for s in &mut segs {
        s.0 -= min_x;
        s.1 -= min_y;
        s.2 -= min_x;
        s.3 -= min_y;
    }

    segs
}
