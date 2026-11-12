use std::path::Path;

use crate::core::image::{
    RasterizeOptions, engrave_segments, load_image, rasterize, rasterize_svg, svg_trace,
};
use crate::core::layer::{EngraveMode, Layer, LayerSource};

use super::state::SegmentCacheEntry;

pub fn layer_cache_key(layer: &Layer) -> Option<String> {
    match &layer.source {
        LayerSource::Image { path } => Some(format!(
            "img::{}::{}::{}::{}::{}::{}::{}",
            path,
            layer.params.pixels_per_mm,
            layer.params.threshold,
            layer.params.invert,
            layer.params.power_percent,
            layer.params.feed_rate,
            layer.params.passes,
        )),
        LayerSource::Svg { path } => Some(format!(
            "svg::{}::{:?}::{}::{}::{}::{}::{}::{}::{}",
            path,
            layer.params.engrave_mode,
            layer.params.pixels_per_mm,
            layer.params.threshold,
            layer.params.invert,
            layer.params.power_percent,
            layer.params.feed_rate,
            layer.params.passes,
            layer.bounds.map(|b| format!("{:?}", b)).unwrap_or_default(),
        )),
        LayerSource::Gcode { path } => {
            let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            Some(format!("gcode::{}::{}", path, len))
        }
        LayerSource::Vector => None,
    }
}

pub fn get_or_build<'a>(
    cache: &'a mut std::collections::HashMap<String, SegmentCacheEntry>,
    layer: &'a Layer,
) -> Option<&'a SegmentCacheEntry> {
    let key = layer_cache_key(layer)?;

    if !cache.contains_key(&key) {
        let segments = build_segments(layer);
        let bounds = segments_bounds(&segments);
        cache.insert(
            key.clone(),
            SegmentCacheEntry {
                key: key.clone(),
                segments,
                bounds,
            },
        );
    }

    cache.get(&key)
}

fn segments_bounds(segments: &[(f64, f64, f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    if segments.is_empty() {
        return None;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x0, y0, x1, y1) in segments {
        min_x = min_x.min(*x0).min(*x1);
        min_y = min_y.min(*y0).min(*y1);
        max_x = max_x.max(*x0).max(*x1);
        max_y = max_y.max(*y0).max(*y1);
    }
    if !min_x.is_finite() {
        None
    } else {
        Some((min_x, min_y, max_x, max_y))
    }
}

fn build_segments(layer: &Layer) -> Vec<(f64, f64, f64, f64)> {
    let origin = layer_origin(layer);
    let width_mm = layer
        .bounds()
        .map(|b| (b.2 - b.0).max(1.0))
        .unwrap_or(100.0);

    match &layer.source {
        LayerSource::Image { path } => {
            let Ok(loaded) = load_image(Path::new(path)) else {
                return Vec::new();
            };
            let raster = rasterize(
                &loaded.dynamic,
                RasterizeOptions {
                    pixels_per_mm: layer.params.pixels_per_mm,
                    threshold: layer.params.threshold,
                    invert: layer.params.invert,
                },
                Some(width_mm),
            );
            engrave_segments(&raster, &layer.params, origin)
                .unwrap_or_default()
                .into_iter()
                .map(|s| (s.x0, s.y0, s.x1, s.y1))
                .collect()
        }
        LayerSource::Svg { path } => {
            if layer.params.engrave_mode == EngraveMode::Lines {
                let target_px = (layer.params.pixels_per_mm * width_mm)
                    .round()
                    .clamp(512.0, 4096.0) as u32;

                let Ok(img) = svg_trace::rasterize_svg_to_image(Path::new(path), target_px) else {
                    return Vec::new();
                };

                let paths = svg_trace::trace_image_to_paths(&img, 128);
                let scale_x = width_mm / target_px as f64;
                let scale_y =
                    (width_mm * (img.height() as f64 / img.width() as f64)) / img.height() as f64;

                let mut out = Vec::new();
                for p in paths {
                    if p.points.len() < 2 {
                        continue;
                    }
                    for w in p.points.windows(2) {
                        let x0 = origin.0 + w[0].0 * scale_x;
                        let y0 = origin.1 + w[0].1 * scale_y;
                        let x1 = origin.0 + w[1].0 * scale_x;
                        let y1 = origin.1 + w[1].1 * scale_y;
                        out.push((x0, y0, x1, y1));
                    }
                }
                out
            } else {
                let target_px = (layer.params.pixels_per_mm * width_mm)
                    .round()
                    .clamp(64.0, 4096.0) as u32;
                let Ok(svg) = rasterize_svg(Path::new(path), target_px) else {
                    return Vec::new();
                };
                let raster = rasterize(
                    &svg.image,
                    RasterizeOptions {
                        pixels_per_mm: layer.params.pixels_per_mm,
                        threshold: layer.params.threshold,
                        invert: layer.params.invert,
                    },
                    Some(width_mm),
                );
                engrave_segments(&raster, &layer.params, origin)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| (s.x0, s.y0, s.x1, s.y1))
                    .collect()
            }
        }
        LayerSource::Gcode { path } => parse_gcode(Path::new(path)),
        LayerSource::Vector => Vec::new(),
    }
}

fn parse_gcode(path: &Path) -> Vec<(f64, f64, f64, f64)> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut cur = (0.0f64, 0.0f64);
    let mut mode_rapid = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        let mut nx = None;
        let mut ny = None;
        let mut seen_g = None;

        for tok in line.split_whitespace() {
            if let Some(rest) = tok.strip_prefix('G') {
                if let Ok(g) = rest.parse::<u32>() {
                    seen_g = Some(g);
                }
            } else if let Some(rest) = tok.strip_prefix('X') {
                nx = rest.parse::<f64>().ok();
            } else if let Some(rest) = tok.strip_prefix('Y') {
                ny = rest.parse::<f64>().ok();
            }
        }

        if let Some(g) = seen_g {
            if g == 0 {
                mode_rapid = true;
            } else if g == 1 || g == 2 || g == 3 {
                mode_rapid = false;
            }
        }

        let px = nx.unwrap_or(cur.0);
        let py = ny.unwrap_or(cur.1);

        if !mode_rapid && (px != cur.0 || py != cur.1) {
            out.push((cur.0, cur.1, px, py));
        }

        cur = (px, py);
    }

    out
}

fn layer_origin(layer: &Layer) -> (f64, f64) {
    if layer.is_vector() {
        (layer.offset[0], layer.offset[1])
    } else {
        layer.bounds().map(|b| (b.0, b.1)).unwrap_or((0.0, 0.0))
    }
}
