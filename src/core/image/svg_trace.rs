use std::path::Path;

use image::DynamicImage;
use resvg::usvg;
use tiny_skia::Pixmap;

pub struct TracedPath {
    pub points: Vec<(f64, f64)>,
    pub closed: bool,
}

pub fn rasterize_svg_to_image(path: &Path, target_px_width: u32) -> Result<DynamicImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("svg read: {}", e))?;
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("svg parse: {}", e))?;

    let size = tree.size();
    let svg_w = size.width() as f64;
    let svg_h = size.height() as f64;
    if svg_w <= 0.0 || svg_h <= 0.0 {
        return Err("svg has zero size".to_string());
    }

    let aspect = svg_w / svg_h;
    let px_w = target_px_width.max(1);
    let px_h = ((px_w as f64) / aspect).round().max(1.0) as u32;

    let mut pixmap =
        Pixmap::new(px_w, px_h).ok_or_else(|| "failed to create pixmap".to_string())?;

    let scale_x = px_w as f32 / svg_w as f32;
    let scale_y = px_h as f32 / svg_h as f32;
    let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba = image::RgbaImage::from_raw(px_w, px_h, pixmap.data().to_vec())
        .ok_or_else(|| "failed to convert pixmap to image".to_string())?;

    Ok(DynamicImage::ImageRgba8(rgba))
}

fn thinning_zhang_suen(img: &mut [u8], w: usize, h: usize) {
    let mut changed = true;
    while changed {
        changed = false;

        let mut to_remove = Vec::new();
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = y * w + x;
                if img[idx] == 0 {
                    continue;
                }

                let p2 = img[(y - 1) * w + x];
                let p3 = img[(y - 1) * w + (x + 1)];
                let p4 = img[y * w + (x + 1)];
                let p5 = img[(y + 1) * w + (x + 1)];
                let p6 = img[(y + 1) * w + x];
                let p7 = img[(y + 1) * w + (x - 1)];
                let p8 = img[y * w + (x - 1)];
                let p9 = img[(y - 1) * w + (x - 1)];

                let neighbors = [p2, p3, p4, p5, p6, p7, p8, p9];
                let mut transitions = 0;
                for i in 0..8 {
                    if neighbors[i] == 0 && neighbors[(i + 1) % 8] == 1 {
                        transitions += 1;
                    }
                }

                let sum: usize = neighbors.iter().map(|&v| v as usize).sum();

                if transitions == 1 && sum >= 2 && sum <= 6 {
                    if p2 * p4 * p6 == 0 && p4 * p6 * p8 == 0 {
                        to_remove.push(idx);
                    }
                }
            }
        }

        for idx in to_remove {
            img[idx] = 0;
            changed = true;
        }

        let mut to_remove = Vec::new();
        for y in 1..h - 1 {
            for x in 1..w - 1 {
                let idx = y * w + x;
                if img[idx] == 0 {
                    continue;
                }

                let p2 = img[(y - 1) * w + x];
                let p3 = img[(y - 1) * w + (x + 1)];
                let p4 = img[y * w + (x + 1)];
                let p5 = img[(y + 1) * w + (x + 1)];
                let p6 = img[(y + 1) * w + x];
                let p7 = img[(y + 1) * w + (x - 1)];
                let p8 = img[y * w + (x - 1)];
                let p9 = img[(y - 1) * w + (x - 1)];

                let neighbors = [p2, p3, p4, p5, p6, p7, p8, p9];
                let mut transitions = 0;
                for i in 0..8 {
                    if neighbors[i] == 0 && neighbors[(i + 1) % 8] == 1 {
                        transitions += 1;
                    }
                }

                let sum: usize = neighbors.iter().map(|&v| v as usize).sum();

                if transitions == 1 && sum >= 2 && sum <= 6 {
                    if p2 * p4 * p8 == 0 && p2 * p6 * p8 == 0 {
                        to_remove.push(idx);
                    }
                }
            }
        }

        for idx in to_remove {
            img[idx] = 0;
            changed = true;
        }
    }
}

fn point_line_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-9 {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2;
    let t = t.clamp(0.0, 1.0);
    let px = a.0 + t * dx;
    let py = a.1 + t * dy;
    ((p.0 - px).powi(2) + (p.1 - py).powi(2)).sqrt()
}

fn simplify_rdp(points: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let first = points[0];
    let last = points[points.len() - 1];

    let mut max_dist = 0.0;
    let mut max_idx = 0;
    for i in 1..points.len() - 1 {
        let dist = point_line_dist(points[i], first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        let mut left = simplify_rdp(&points[..=max_idx], epsilon);
        let right = simplify_rdp(&points[max_idx..], epsilon);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![first, last]
    }
}

pub fn trace_image_to_paths(img: &DynamicImage, threshold: u8) -> Vec<TracedPath> {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    let mut binary_img = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let luma = gray.get_pixel(x, y).0[0];
            if luma < threshold {
                binary_img[(y * w + x) as usize] = 1;
            }
        }
    }

    thinning_zhang_suen(&mut binary_img, w as usize, h as usize);

    let mut points = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if binary_img[(y * w + x) as usize] == 1 {
                points.push((x as i32, y as i32));
            }
        }
    }

    let mut paths = Vec::new();
    let mut visited = std::collections::HashSet::new();

    let neighbors_dirs = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];

    for &start in &points {
        if visited.contains(&start) {
            continue;
        }

        let mut path = Vec::new();
        let mut curr = start;

        loop {
            visited.insert(curr);
            path.push((curr.0 as f64, curr.1 as f64));

            let mut next_pt = None;
            for &(dx, dy) in &neighbors_dirs {
                let nx = curr.0 + dx;
                let ny = curr.1 + dy;
                if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                    let n_idx = (ny as u32 * w + nx as u32) as usize;
                    if binary_img[n_idx] == 1 && !visited.contains(&(nx, ny)) {
                        next_pt = Some((nx, ny));
                        break;
                    }
                }
            }

            if let Some(np) = next_pt {
                curr = np;
            } else {
                break;
            }
        }

        if path.len() > 1 {
            let simplified = simplify_rdp(&path, 0.5);
            if simplified.len() > 1 {
                paths.push(TracedPath {
                    points: simplified,
                    closed: false,
                });
            }
        }
    }

    paths
}

pub fn traced_paths_to_gcode(
    paths: &[TracedPath],
    params: &crate::core::layer::LayerParams,
    origin: (f64, f64),
    scale_x: f64,
    scale_y: f64,
) -> Vec<crate::core::gcode::GcodeCommand> {
    use crate::core::gcode::GcodeCommand;

    let s_max = 1000.0;
    let s_on = ((params.power_percent.clamp(0.0, 100.0) / 100.0) * s_max).round();

    let laser_on = if params.dynamic_power {
        format!("M4 S{:.0}", s_on)
    } else {
        format!("M3 S{:.0}", s_on)
    };

    let mut cmds = Vec::new();
    let mk = |raw: String| GcodeCommand {
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
    };

    cmds.push(mk("G21".to_string()));
    cmds.push(mk("G90".to_string()));
    cmds.push(mk(laser_on));
    cmds.push(mk(format!("G1 F{:.0}", params.feed_rate)));

    for pass in 0..params.passes.max(1) {
        if pass > 0 {
            cmds.push(mk("; --- next pass ---".to_string()));
        }

        for p in paths {
            if p.points.len() < 2 {
                continue;
            }

            let first = p.points[0];
            let fx = origin.0 + first.0 * scale_x;
            let fy = origin.1 + first.1 * scale_y;

            cmds.push(mk(format!("M5")));
            cmds.push(mk(format!("G0 X{:.4} Y{:.4}", fx, fy)));

            let laser_on_cmd = if params.dynamic_power {
                format!("M4 S{:.0}", s_on)
            } else {
                format!("M3 S{:.0}", s_on)
            };
            cmds.push(mk(laser_on_cmd));

            for pt in p.points.iter().skip(1) {
                let px = origin.0 + pt.0 * scale_x;
                let py = origin.1 + pt.1 * scale_y;
                cmds.push(mk(format!("G1 X{:.4} Y{:.4}", px, py)));
            }

            cmds.push(mk("M5".to_string()));
        }
    }

    cmds.push(mk("M5".to_string()));

    cmds
}
