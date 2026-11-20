use std::path::Path;

use image::DynamicImage;
use resvg::usvg;
use tiny_skia::Pixmap;

/// Parses an SVG file into a render tree.
fn load_tree(path: &Path) -> Result<usvg::Tree, String> {
    let svg_data = std::fs::read(path).map_err(|e| format!("svg read: {}", e))?;
    let opt = usvg::Options::default();
    usvg::Tree::from_data(&svg_data, &opt).map_err(|e| format!("svg parse: {}", e))
}

/// Size the renderer resolved for the file, in user units (CSS pixels at 96 dpi).
///
/// This is the size the drawing was built for, not the size it should occupy on the workbed:
/// use [`crate::core::image::svg_size_mm`] for that, because the authored millimetres are
/// only in the file's own attributes.
pub fn svg_user_size_px(path: &Path) -> Result<(f64, f64), String> {
    let tree = load_tree(path)?;
    let size = tree.size();
    let width = size.width() as f64;
    let height = size.height() as f64;

    if width > 0.0 && height > 0.0 {
        Ok((width, height))
    } else {
        Err("svg has zero size".to_string())
    }
}

/// Largest edge a raster box may have, which keeps very long drawings from allocating a huge
/// pixmap.
const MAX_BOX_EDGE: f64 = 8192.0;

/// Pixel box used to rasterize an SVG that belongs to a layer of the given size.
///
/// The box carries the layer's aspect ratio, so the texture can be drawn straight into the
/// layer's bounds without being stretched. Both edges shrink together when the box would be
/// too large: capping one edge alone would change the aspect ratio and stretch the drawing.
pub fn svg_raster_box(width_mm: f64, height_mm: f64, target_px_width: u32) -> (u32, u32) {
    let aspect = height_mm.max(1e-3) / width_mm.max(1e-3);
    let width_px = target_px_width.max(1) as f64;
    let height_px = (width_px * aspect).max(1.0);

    let scale = (MAX_BOX_EDGE / width_px)
        .min(MAX_BOX_EDGE / height_px)
        .min(1.0);

    (
        (width_px * scale).round().max(1.0) as u32,
        (height_px * scale).round().max(1.0) as u32,
    )
}

/// Rasterizes an SVG into an exact pixel box, scaled uniformly and centred.
///
/// One scale is used for both axes, so the drawing keeps its own aspect ratio inside whatever
/// box it is given: a file whose aspect differs from the layer's is letterboxed rather than
/// stretched.
pub fn rasterize_svg_into_box(
    path: &Path,
    target_px_width: u32,
    target_px_height: u32,
) -> Result<DynamicImage, String> {
    let tree = load_tree(path)?;

    let size = tree.size();
    let svg_width = size.width() as f64;
    let svg_height = size.height() as f64;
    if svg_width <= 0.0 || svg_height <= 0.0 {
        return Err("svg has zero size".to_string());
    }

    let px_width = target_px_width.max(1);
    let px_height = target_px_height.max(1);
    let mut pixmap =
        Pixmap::new(px_width, px_height).ok_or_else(|| "failed to create pixmap".to_string())?;

    let scale = (px_width as f64 / svg_width)
        .min(px_height as f64 / svg_height) as f32;
    let transform = tiny_skia::Transform {
        sx: scale,
        kx: 0.0,
        ky: 0.0,
        sy: scale,
        tx: ((px_width as f64 - svg_width * scale as f64) / 2.0) as f32,
        ty: ((px_height as f64 - svg_height * scale as f64) / 2.0) as f32,
    };

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba = image::RgbaImage::from_raw(px_width, px_height, pixmap.data().to_vec())
        .ok_or_else(|| "failed to convert pixmap to image".to_string())?;

    Ok(DynamicImage::ImageRgba8(rgba))
}
