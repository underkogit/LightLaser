use image::{DynamicImage, GrayImage, Luma};

#[derive(Debug, Clone, Copy)]
pub struct RasterizeOptions {
    pub pixels_per_mm: f64,
    pub threshold: u8,
    pub invert: bool,
}

#[derive(Debug, Clone)]
pub struct RasterImage {
    pub px_width: u32,
    pub px_height: u32,
    pub width_mm: f64,
    pub height_mm: f64,
    pub pixels: GrayImage,
    pub threshold: u8,
    pub invert: bool,
}

impl RasterImage {
    pub fn width_px(&self) -> u32 {
        self.px_width
    }

    pub fn height_px(&self) -> u32 {
        self.px_height
    }

    pub fn is_dark(&self, x: u32, y: u32) -> bool {
        if x >= self.px_width || y >= self.px_height {
            return false;
        }
        let luma = self.pixels.get_pixel(x, y).0[0];
        let effective = if self.invert { 255 - luma } else { luma };
        effective < self.threshold
    }
}

pub fn rasterize(
    image: &DynamicImage,
    options: RasterizeOptions,
    target_width_mm: Option<f64>,
) -> RasterImage {
    let ppm = options.pixels_per_mm.max(0.01);
    let aspect = if image.height() == 0 {
        1.0
    } else {
        image.width() as f64 / image.height() as f64
    };

    let width_mm = target_width_mm.unwrap_or(image.width() as f64 / ppm);
    let height_mm = width_mm / aspect.max(1e-9);

    let px_width = (width_mm * ppm).round().max(1.0) as u32;
    let px_height = (height_mm * ppm).round().max(1.0) as u32;

    let resized = image.resize_exact(px_width, px_height, image::imageops::FilterType::Lanczos3);
    let gray: GrayImage = resized.to_luma8();

    RasterImage {
        px_width,
        px_height,
        width_mm,
        height_mm,
        pixels: gray,
        threshold: options.threshold,
        invert: options.invert,
    }
}