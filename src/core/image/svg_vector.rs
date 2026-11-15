use std::path::Path;

use resvg::usvg;

use crate::core::vector::CurvePoint;

pub struct SvgPath {
    pub points: Vec<CurvePoint>,
    pub closed: bool,
}

fn convert_path(path: &usvg::Path) -> Option<SvgPath> {
    let mut points = Vec::new();
    let mut closed = false;

    for seg in path.data().segments() {
        match seg {
            tiny_skia_path::PathSegment::MoveTo(p) => {
                points.push(CurvePoint::new(p.x as f64, p.y as f64));
                closed = false;
            }
            tiny_skia_path::PathSegment::LineTo(p) => {
                points.push(CurvePoint::new(p.x as f64, p.y as f64));
            }
            tiny_skia_path::PathSegment::QuadTo(p1, p2) => {
                points.push(CurvePoint::new(p1.x as f64, p1.y as f64));
                points.push(CurvePoint::new(p2.x as f64, p2.y as f64));
            }
            tiny_skia_path::PathSegment::CubicTo(p1, p2, p3) => {
                points.push(CurvePoint::new(p1.x as f64, p1.y as f64));
                points.push(CurvePoint::new(p2.x as f64, p2.y as f64));
                points.push(CurvePoint::new(p3.x as f64, p3.y as f64));
            }
            tiny_skia_path::PathSegment::Close => {
                closed = true;
            }
        }
    }

    if points.len() < 2 {
        return None;
    }

    Some(SvgPath { points, closed })
}

fn is_white(paint: usvg::Paint, opacity: f32) -> bool {
    if opacity <= 0.01 {
        return false;
    }
    if let usvg::Paint::Color(c) = paint {
        c.red > 240 && c.green > 240 && c.blue > 240
    } else {
        false
    }
}

fn walk_group(group: &usvg::Group, out: &mut Vec<SvgPath>) {
    for node in group.children() {
        match node {
            usvg::Node::Path(p) => {
                let fill = p.fill();
                let stroke = p.stroke();

                let has_white_fill = match fill {
                    Some(f) => is_white(f.paint().clone(), f.opacity().get()),
                    None => false,
                };

                let has_any_stroke = stroke.is_some();

                if has_any_stroke || !has_white_fill {
                    if let Some(path) = convert_path(p) {
                        out.push(path);
                    }
                }
            }
            usvg::Node::Group(g) => walk_group(g, out),
            _ => {}
        }
    }
}

pub fn extract_svg_paths(path: &Path) -> Result<Vec<SvgPath>, String> {
    let data = std::fs::read(path).map_err(|e| format!("svg read: {}", e))?;
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&data, &opt).map_err(|e| format!("svg parse: {}", e))?;

    let mut out = Vec::new();
    walk_group(tree.root(), &mut out);

    if out.is_empty() {
        return Err("no paths found in svg".to_string());
    }

    Ok(out)
}

pub fn svg_paths_to_gcode(
    paths: &[SvgPath],
    params: &crate::core::layer::LayerParams,
    origin: (f64, f64),
    scale: f64,
) -> Vec<crate::core::gcode::GcodeCommand> {
    use crate::core::gcode::GcodeCommand;

    let s_max = 1000.0;
    let s_on = ((params.power_percent.clamp(0.0, 100.0) / 100.0) * s_max).round();

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
            cmds.push(mk("M5".to_string()));
            cmds.push(mk(format!(
                "G0 X{:.4} Y{:.4}",
                origin.0 + first.x * scale,
                origin.1 + first.y * scale
            )));

            let laser_on_cmd = if params.dynamic_power {
                format!("M4 S{:.0}", s_on)
            } else {
                format!("M3 S{:.0}", s_on)
            };
            cmds.push(mk(laser_on_cmd));

            for pt in p.points.iter().skip(1) {
                cmds.push(mk(format!(
                    "G1 X{:.4} Y{:.4}",
                    origin.0 + pt.x * scale,
                    origin.1 + pt.y * scale
                )));
            }

            if p.closed {
                cmds.push(mk(format!(
                    "G1 X{:.4} Y{:.4}",
                    origin.0 + first.x * scale,
                    origin.1 + first.y * scale
                )));
            }
            cmds.push(mk("M5".to_string()));
        }
    }

    cmds.push(mk("M5".to_string()));

    cmds
}
