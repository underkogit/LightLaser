use crate::core::layer::{Layer, LayerSource, layer_segments};
use crate::core::mirror;

use super::state::SegmentCacheEntry;

pub fn layer_cache_key(layer: &Layer) -> Option<String> {
    let mirror = format!("{}::{}", layer.mirror_x, layer.mirror_y);
    let size = layer
        .bounds()
        .map(|(min_x, min_y, max_x, max_y)| {
            format!("{:.4}x{:.4}", (max_x - min_x).max(0.0), (max_y - min_y).max(0.0))
        })
        .unwrap_or_default();

    match &layer.source {
        LayerSource::Image { path } => Some(format!(
            "img::{}::{}::{}::{}::{}::{}",
            path,
            layer.params.pixels_per_mm,
            layer.params.threshold,
            layer.params.invert,
            size,
            mirror,
        )),
        LayerSource::Svg { path } => Some(format!(
            "svg::{}::{:?}::{}::{}::{}::{}::{}",
            path,
            layer.params.engrave_mode,
            layer.params.pixels_per_mm,
            layer.params.threshold,
            layer.params.invert,
            size,
            mirror,
        )),
        LayerSource::Gcode { path } => Some(format!("gcode::{}::{}", path, mirror)),
        LayerSource::Gerber { path } => Some(format!("gerber::{}::{}", path, mirror)),
        LayerSource::Vector => None,
    }
}

pub fn get_or_build<'a>(
    cache: &'a mut std::collections::HashMap<String, SegmentCacheEntry>,
    layer: &'a Layer,
) -> Option<&'a SegmentCacheEntry> {
    let key = layer_cache_key(layer)?;

    if !cache.contains_key(&key) {
        let segments = layer_segments(layer).unwrap_or_default();
        let bbox = mirror::bounds(&segments);
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
