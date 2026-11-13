use std::path::Path;

use image::DynamicImage;
use resvg::usvg;
use tiny_skia::Pixmap;

pub struct RasterizedSvg {
    pub image: DynamicImage,
    pub width_mm: f64,
    pub height_mm: f64,
}

pub fn rasterize_svg(path: &Path, target_px_width: u32) -> Result<RasterizedSvg, String> {
    let svg_data = std::fs::read(path).map_err(|e| format!("svg read: {}", e))?;
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg_data, &opt).map_err(|e| format!("svg parse: {}", e))?;

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
    let dynamic = DynamicImage::ImageRgba8(rgba);

    let width_mm = svg_w * 25.4 / 96.0;
    let height_mm = svg_h * 25.4 / 96.0;

    Ok(RasterizedSvg {
        image: dynamic,
        width_mm,
        height_mm,
    })
}
