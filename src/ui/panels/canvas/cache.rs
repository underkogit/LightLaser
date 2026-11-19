use std::path::Path;

use crate::core::gcode::{normalize_gcode, parse_gcode_file_segments, transform_for_layer};
use crate::core::image::{
    RasterizeOptions, engrave_segments, load_image, rasterize, rasterize_svg, svg_trace,
};
use crate::core::layer::{EngraveMode, Layer, LayerSource};

use super::state::SegmentCacheEntry;

pub fn layer_cache_key(layer: &Layer) -> Option<String> {
    match &layer.source {
        LayerSource::Image { path } => Some(format!(
            "img::{}::{}::{}::{}::{}::{}::{}::{}::{}",
            path,
            layer.params.pixels_per_mm,
            layer.params.threshold,
            layer.params.invert,
            layer.params.power_percent,
            layer.params.feed_rate,
            layer.params.passes,
            layer.mirror_x,
            layer.mirror_y,
        )),
        LayerSource::Svg { path } => Some(format!(
            "svg::{}::{:?}::{}::{}::{}::{}::{}::{}::{}::{}::{}",
            path,
            layer.params.engrave_mode,
            layer.params.pixels_per_mm,
            layer.params.threshold,
            layer.params.invert,
            layer.params.power_percent,
            layer.params.feed_rate,
            layer.params.passes,
            layer.bounds.map(|b| format!("{:?}", b)).unwrap_or_default(),
            layer.mirror_x,
            layer.mirror_y,
        )),
        LayerSource::Gcode { path } => Some(format!(
            "gcode::{}::{}::{}::{}",
            path,
            layer.mirror_x,
            layer.mirror_y,
            layer
                .mirror_center
                .map(|c| format!("{:?}", c))
                .unwrap_or_default(),
        )),
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
        let bbox = segments_bbox(&segments);
        cache.insert(
            key.clone(),
            SegmentCacheEntry {
                segments,
                bbox,
            },
        );
    }

    cache.get(&key)
}

fn segments_bbox(segments: &[(f64, f64, f64, f64)]) -> Option<(f64, f64, f64, f64)> {
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

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
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
        LayerSource::Gcode { path } => {
            let raw = parse_gcode_file_segments(Path::new(path));
            let transformed = transform_for_layer(raw, layer);
            normalize_gcode(transformed)
        }
        LayerSource::Vector => Vec::new(),
    }
}

fn layer_origin(layer: &Layer) -> (f64, f64) {
    if layer.is_vector() {
        (layer.offset[0], layer.offset[1])
    } else {
        layer.bounds().map(|b| (b.0, b.1)).unwrap_or((0.0, 0.0))
    }
}
