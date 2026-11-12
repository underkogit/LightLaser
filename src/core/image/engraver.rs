use std::fmt;

use crate::core::gcode::GcodeCommand;
use crate::core::image::raster::RasterImage;
use crate::core::layer::LayerParams;

#[derive(Debug)]
pub enum EngraveError {
    EmptyRaster,
    InvalidParams(String),
}

impl fmt::Display for EngraveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngraveError::EmptyRaster => write!(f, "Raster is empty"),
            EngraveError::InvalidParams(msg) => write!(f, "Invalid engrave params: {}", msg),
        }
    }
}

impl std::error::Error for EngraveError {}

pub fn engrave_layer(
    raster: &RasterImage,
    params: &LayerParams,
    origin: (f64, f64),
) -> Result<Vec<GcodeCommand>, EngraveError> {
    if raster.px_width == 0 || raster.px_height == 0 {
        return Err(EngraveError::EmptyRaster);
    }

    let feed = params.feed_rate;
    if feed <= 0.0 {
        return Err(EngraveError::InvalidParams(
            "feed_rate must be > 0".to_string(),
        ));
    }

    let ppm = params.pixels_per_mm.max(0.01);
    let px_mm = 1.0 / ppm;

    let s_max = 1000.0;
    let s_on = ((params.power_percent.clamp(0.0, 100.0) / 100.0) * s_max).round();

    let laser_on_cmd = if params.dynamic_power {
        format!("M4 S{:.0}", s_on)
    } else {
        format!("M3 S{:.0}", s_on)
    };

    let mut cmds: Vec<GcodeCommand> = Vec::new();

    cmds.push(make_cmd("G21 ; mm".to_string()));
    cmds.push(make_cmd("G90 ; absolute".to_string()));
    cmds.push(make_cmd(laser_on_cmd));
    cmds.push(make_cmd(format!("G1 F{:.0}", feed)));

    let origin_x = origin.0;
    let origin_y = origin.1;

    for pass in 0..params.passes.max(1) {
        if pass > 0 {
            cmds.push(make_cmd("; --- next pass ---".to_string()));
        }

        for row in 0..raster.px_height {
            let y = origin_y + (row as f64) * px_mm;
            let reverse = row % 2 == 1;

            let cols: Box<dyn Iterator<Item = u32>> = if reverse {
                Box::new((0..raster.px_width).rev())
            } else {
                Box::new(0..raster.px_width)
            };

            let mut run_start: Option<u32> = None;
            let mut last_col: Option<u32> = None;

            for col in cols {
                let dark = raster.is_dark(col, row);

                match (dark, run_start) {
                    (true, None) => {
                        run_start = Some(col);
                        last_col = Some(col);
                    }
                    (true, Some(_)) => {
                        last_col = Some(col);
                    }
                    (false, Some(start_col)) => {
                        let end_col = last_col.expect("dark run had no last_col");
                        emit_segment(&mut cmds, start_col, end_col, y, origin_x, px_mm);
                        run_start = None;
                        last_col = None;
                    }
                    (false, None) => {}
                }
            }

            if let (Some(start_col), Some(end_col)) = (run_start, last_col) {
                emit_segment(&mut cmds, start_col, end_col, y, origin_x, px_mm);
            }
        }
    }

    cmds.push(make_cmd("M5 ; laser off".to_string()));
    cmds.push(make_cmd("G0 X0 Y0 ; return home".to_string()));

    Ok(cmds)
}

fn emit_segment(
    cmds: &mut Vec<GcodeCommand>,
    start_col: u32,
    end_col: u32,
    y: f64,
    origin_x: f64,
    px_mm: f64,
) {
    let x_start = origin_x + (start_col as f64) * px_mm;
    let x_end = origin_x + (end_col as f64) * px_mm;

    cmds.push(make_cmd(format!("G0 X{:.4} Y{:.4}", x_start, y)));
    cmds.push(make_cmd(format!("G1 X{:.4} Y{:.4}", x_end, y)));
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