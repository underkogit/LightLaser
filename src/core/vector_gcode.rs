use std::fmt;

use crate::core::gcode::GcodeCommand;
use crate::core::layer::{Layer, LayerParams};

#[derive(Debug)]
pub enum VectorExportError {
    EmptyLayer,
    InvalidParams(String),
}

impl fmt::Display for VectorExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VectorExportError::EmptyLayer => write!(f, "vector layer is empty"),
            VectorExportError::InvalidParams(m) => write!(f, "invalid params: {}", m),
        }
    }
}

impl std::error::Error for VectorExportError {}

pub fn export_vector_layer(
    layer: &Layer,
    params: &LayerParams,
    origin: (f64, f64),
    spindle_max: f64,
) -> Result<Vec<GcodeCommand>, VectorExportError> {
    if layer.shapes.is_empty() {
        return Err(VectorExportError::EmptyLayer);
    }

    if params.feed_rate <= 0.0 {
        return Err(VectorExportError::InvalidParams(
            "feed_rate must be > 0".to_string(),
        ));
    }

    let s_on = params.spindle_value(spindle_max);

    let mut cmds: Vec<GcodeCommand> = Vec::new();

    cmds.push(make_cmd("G21".to_string()));
    cmds.push(make_cmd("G90".to_string()));
    cmds.push(make_cmd(format!("G1 F{:.0}", params.feed_rate)));

    for pass in 0..params.passes.max(1) {
        if pass > 0 {
            cmds.push(make_cmd("; --- next pass ---".to_string()));
        }

        for shape in &layer.shapes {
            let mut path = crate::core::vector::VectorPath::new();
            path.nodes = shape.nodes.clone();
            path.stroke = shape.stroke.clone();
            path.closed = shape.closed;

            let pts = path.flatten(0.05);
            if pts.len() < 2 {
                continue;
            }

            let first = pts[0];

            cmds.push(make_cmd("M5".to_string()));
            cmds.push(make_cmd(format!(
                "G0 X{:.4} Y{:.4}",
                origin.0 + first.x,
                origin.1 + first.y
            )));

            let laser_on_cmd = if params.dynamic_power {
                format!("M4 S{:.0}", s_on)
            } else {
                format!("M3 S{:.0}", s_on)
            };
            cmds.push(make_cmd(laser_on_cmd));

            let mut last = first;
            for p in pts.iter().skip(1) {
                if (p.x - last.x).abs() < 1e-9 && (p.y - last.y).abs() < 1e-9 {
                    continue;
                }
                cmds.push(make_cmd(format!(
                    "G1 X{:.4} Y{:.4}",
                    origin.0 + p.x,
                    origin.1 + p.y
                )));
                last = *p;
            }

            // A closed path already ends where it started, so the closing move would be a
            // zero-length instruction.
            if path.closed && ((last.x - first.x).abs() >= 1e-9 || (last.y - first.y).abs() >= 1e-9)
            {
                cmds.push(make_cmd(format!(
                    "G1 X{:.4} Y{:.4}",
                    origin.0 + first.x,
                    origin.1 + first.y
                )));
            }

            cmds.push(make_cmd("M5".to_string()));
        }
    }

    cmds.push(make_cmd("M5".to_string()));

    Ok(cmds)
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
