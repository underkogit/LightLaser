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

#[derive(Debug, Clone, Copy)]
pub struct EngraveSegment {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

pub fn engrave_segments(
    raster: &RasterImage,
    params: &LayerParams,
    origin: (f64, f64),
) -> Result<Vec<EngraveSegment>, EngraveError> {
    if raster.px_width == 0 || raster.px_height == 0 {
        return Err(EngraveError::EmptyRaster);
    }
    if params.feed_rate <= 0.0 {
        return Err(EngraveError::InvalidParams("feed_rate must be > 0".to_string()));
    }

    let step_x = (raster.width_mm / raster.px_width as f64).max(1e-6);
    let step_y = (raster.height_mm / raster.px_height as f64).max(1e-6);

    let origin_x = origin.0;
    let origin_y = origin.1;

    let mut segs: Vec<EngraveSegment> = Vec::new();

    for _pass in 0..params.passes.max(1) {
        for row in 0..raster.px_height {
            let y = origin_y + (row as f64) * step_y;
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
                        let end_col = last_col.unwrap_or(start_col);
                        let (lo, hi) = if start_col <= end_col {
                            (start_col, end_col)
                        } else {
                            (end_col, start_col)
                        };
                        segs.push(EngraveSegment {
                            x0: origin_x + (lo as f64) * step_x,
                            y0: y,
                            x1: origin_x + (hi as f64) * step_x,
                            y1: y,
                        });
                        run_start = None;
                        last_col = None;
                    }
                    (false, None) => {}
                }
            }

            if let (Some(start_col), Some(end_col)) = (run_start, last_col) {
                let (lo, hi) = if start_col <= end_col {
                    (start_col, end_col)
                } else {
                    (end_col, start_col)
                };
                segs.push(EngraveSegment {
                    x0: origin_x + (lo as f64) * step_x,
                    y0: y,
                    x1: origin_x + (hi as f64) * step_x,
                    y1: y,
                });
            }
        }
    }

    Ok(segs)
}

pub fn engrave_layer(
    raster: &RasterImage,
    params: &LayerParams,
    origin: (f64, f64),
) -> Result<Vec<GcodeCommand>, EngraveError> {
    let segs = engrave_segments(raster, params, origin)?;

    let feed = params.feed_rate;
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

    for pass in 1..params.passes.max(1) {
        cmds.push(make_cmd(format!("; --- pass {} ---", pass + 1)));
    }

    for s in segs {
        cmds.push(make_cmd(format!("G0 X{:.4} Y{:.4}", s.x0, s.y0)));
        cmds.push(make_cmd(format!("G1 X{:.4} Y{:.4}", s.x1, s.y1)));
    }

    cmds.push(make_cmd("M5 ; laser off".to_string()));
    cmds.push(make_cmd("G0 X0 Y0 ; return home".to_string()));

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