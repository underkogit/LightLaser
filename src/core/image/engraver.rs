use std::fmt;

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
        return Err(EngraveError::InvalidParams(
            "feed_rate must be > 0".to_string(),
        ));
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
                        segs.push(run_segment(origin_x, y, lo, hi, step_x));
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
                segs.push(run_segment(origin_x, y, lo, hi, step_x));
            }
        }
    }

    Ok(segs)
}

/// A dark run that is a single pixel wide would turn into a zero length move, and a zero
/// length move never fires the laser in laser mode. It is widened to one pixel of travel.
fn run_segment(origin_x: f64, y: f64, lo: u32, hi: u32, step_x: f64) -> EngraveSegment {
    let x0 = origin_x + (lo as f64) * step_x;
    let mut x1 = origin_x + (hi as f64) * step_x;

    if (x1 - x0).abs() < step_x * 0.5 {
        x1 = x0 + step_x;
    }

    EngraveSegment {
        x0,
        y0: y,
        x1,
        y1: y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layer::LayerParams;

    fn dark_raster() -> RasterImage {
        RasterImage {
            px_width: 4,
            px_height: 1,
            width_mm: 1.0,
            height_mm: 1.0,
            pixels: image::GrayImage::new(4, 1),
            threshold: 128,
            invert: false,
        }
    }

    #[test]
    fn every_dark_run_becomes_one_segment() {
        let params = LayerParams::default();
        let segments = engrave_segments(&dark_raster(), &params, (0.0, 0.0)).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].x0, 0.0);
        assert!(segments[0].x1 > 0.0);
    }

    #[test]
    fn passes_repeat_the_segments() {
        let mut params = LayerParams::default();
        params.passes = 3;

        let segments = engrave_segments(&dark_raster(), &params, (0.0, 0.0)).unwrap();

        assert_eq!(segments.len(), 3);
    }
}
