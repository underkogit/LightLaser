use tiny_skia::Pixmap;

pub const MAX_TEXTURE_DIM: f64 = 2048.0;

const MIN_LEVEL: f64 = 1.0 / 64.0;
const MAX_LEVEL: f64 = 2048.0;

const DOT_LENGTH: f32 = 0.75;

pub fn make_pixmap(width: u32, height: u32) -> Option<Pixmap> {
    Pixmap::new(width.max(1), height.max(1))
}

pub fn to_color_image(pixmap: &Pixmap) -> egui::ColorImage {
    egui::ColorImage::from_rgba_premultiplied(
        [pixmap.width() as usize, pixmap.height() as usize],
        pixmap.data(),
    )
}

pub fn texture_level(needed_px_per_mm: f64, max_dim_mm: f64) -> f64 {
    let needed = needed_px_per_mm.max(MIN_LEVEL);
    let cap = (MAX_TEXTURE_DIM / max_dim_mm.max(1.0)).max(MIN_LEVEL);

    (needed.min(cap).log2().floor()).clamp(MIN_LEVEL.log2(), MAX_LEVEL.log2()).exp2()
}

pub fn stroke_segments<I>(pixmap: &mut Pixmap, segments: I, rgba: [u8; 4])
where
    I: IntoIterator<Item=(f32, f32, f32, f32)>,
{
    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;
    let data = pixmap.data_mut();

    for (x0, y0, x1, y1) in segments {
        draw_line(data, width, height, x0, y0, x1, y1, rgba);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_line(
    px: &mut [u8],
    width: i32,
    height: i32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    rgba: [u8; 4],
) {
    if !(x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite()) {
        return;
    }

    let w = width as f32;
    let h = height as f32;

    if (x0 < -1.0 && x1 < -1.0)
        || (x0 > w + 1.0 && x1 > w + 1.0)
        || (y0 < -1.0 && y1 < -1.0)
        || (y0 > h + 1.0 && y1 > h + 1.0)
    {
        return;
    }

    let dx = x1 - x0;
    let dy = y1 - y0;
    let length = dx.abs().max(dy.abs());

    if length <= DOT_LENGTH {
        splat(px, width, height, x0 + dx * 0.5, y0 + dy * 0.5, rgba);
        return;
    }

    let steps = length.ceil() as i32;
    let step_x = dx / steps as f32;
    let step_y = dy / steps as f32;

    let mut x = x0;
    let mut y = y0;

    for _ in 0..=steps {
        splat(px, width, height, x, y, rgba);
        x += step_x;
        y += step_y;
    }
}

fn splat(px: &mut [u8], width: i32, height: i32, x: f32, y: f32, rgba: [u8; 4]) {
    let fx = x.floor();
    let fy = y.floor();
    let tx = x - fx;
    let ty = y - fy;

    let x0 = fx as i32;
    let y0 = fy as i32;

    let top = (1.0 - ty) * 255.0;
    let bottom = ty * 255.0;

    blend(px, width, height, x0, y0, ((1.0 - tx) * top) as u8, rgba);
    blend(px, width, height, x0 + 1, y0, (tx * top) as u8, rgba);
    blend(px, width, height, x0, y0 + 1, ((1.0 - tx) * bottom) as u8, rgba);
    blend(px, width, height, x0 + 1, y0 + 1, (tx * bottom) as u8, rgba);
}

#[inline]
fn blend(px: &mut [u8], width: i32, height: i32, x: i32, y: i32, coverage: u8, rgba: [u8; 4]) {
    if coverage == 0 || x < 0 || y < 0 || x >= width || y >= height {
        return;
    }

    let alpha = (rgba[3] as u32 * coverage as u32 + 127) / 255;
    if alpha == 0 {
        return;
    }

    let idx = ((y * width + x) * 4) as usize;

    if alpha >= 255 {
        px[idx] = rgba[0];
        px[idx + 1] = rgba[1];
        px[idx + 2] = rgba[2];
        px[idx + 3] = 255;
        return;
    }

    let inv = 255 - alpha;
    let src_r = (rgba[0] as u32 * alpha + 127) / 255;
    let src_g = (rgba[1] as u32 * alpha + 127) / 255;
    let src_b = (rgba[2] as u32 * alpha + 127) / 255;

    px[idx] = (src_r + (px[idx] as u32 * inv + 127) / 255).min(255) as u8;
    px[idx + 1] = (src_g + (px[idx + 1] as u32 * inv + 127) / 255).min(255) as u8;
    px[idx + 2] = (src_b + (px[idx + 2] as u32 * inv + 127) / 255).min(255) as u8;
    px[idx + 3] = (alpha + (px[idx + 3] as u32 * inv + 127) / 255).min(255) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn dense_segments(count: usize) -> Vec<(f32, f32, f32, f32)> {
        (0..count)
            .map(|i| {
                let x = (i % 2000) as f32 * 0.5;
                let y = (i / 2000) as f32 * 0.5;
                (x, y, x + 0.4, y)
            })
            .collect()
    }

    fn bench(label: &str, segments: &[(f32, f32, f32, f32)], rgba: [u8; 4]) {
        let mut pixmap = make_pixmap(1600, 1000).expect("pixmap");

        let start = Instant::now();
        stroke_segments(&mut pixmap, segments.iter().copied(), rgba);
        let elapsed = start.elapsed();

        let ink = pixmap.pixels().iter().filter(|p| p.alpha() > 0).count();
        println!("{label}: stroke={elapsed:?} ink={ink}");
    }

    #[test]
    #[ignore]
    fn rasterize_large_job() {
        let dense = dense_segments(1_000_000);
        bench("dense 1M bright", &dense, [255, 140, 40, 255]);
        bench("dense 1M faint ", &dense, [255, 140, 40, 30]);

        let diagonals: Vec<(f32, f32, f32, f32)> = (0..500)
            .map(|i| {
                let t = i as f32;
                (t, 10.0 + t * 1.7, t + 400.0, 60.0 + t * 1.7)
            })
            .collect();
        bench("diagonals 500 ", &diagonals, [255, 140, 40, 255]);

        let rows: Vec<(f32, f32, f32, f32)> = (0..2000)
            .map(|i| {
                let y = i as f32 * 0.5;
                (0.0, y, 1599.0, y)
            })
            .collect();
        bench("long rows 2000", &rows, [255, 140, 40, 255]);
    }
}
