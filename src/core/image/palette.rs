use std::collections::HashMap;

use image::DynamicImage;

#[derive(Debug, Clone)]
pub struct PaletteColor {
    pub rgb: [u8; 3],
    pub coverage: f64,
    pub count: u32,
}

pub fn extract_palette(image: &DynamicImage) -> Vec<PaletteColor> {
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    let total = (w as u64 * h as u64).max(1) as f64;

    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();

    for pixel in rgba.pixels() {
        if pixel[3] == 0 {
            continue;
        }
        let rgb = [pixel[0], pixel[1], pixel[2]];
        *counts.entry(rgb).or_insert(0) += 1;
    }

    let mut colors: Vec<PaletteColor> = counts
        .into_iter()
        .map(|(rgb, count)| PaletteColor {
            rgb,
            coverage: count as f64 / total,
            count,
        })
        .collect();

    colors.sort_by(|a, b| b.count.cmp(&a.count));
    colors
}
