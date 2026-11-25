use std::path::Path;

use crate::core::gcode::parse_gcode_file_segments;
use crate::core::image::{
    RasterizeOptions, engrave_segments, load_image, parse_gerber_segments, rasterize,
    rasterize_svg_into_box, svg_raster_box, svg_trace,
};
use crate::core::layer::{EngraveMode, Layer, LayerParams, LayerSource};
use crate::core::mirror;
use crate::core::vector::VectorPath;

pub type Segment = (f64, f64, f64, f64);

/// Where the segments of a layer start. Raster and traced layers are laid out from the
/// bottom left corner of their box, while vector nodes already carry their absolute position.
pub fn placement(layer: &Layer) -> (f64, f64) {
    if layer.is_vector() {
        return (0.0, 0.0);
    }
    layer.bounds().map(|b| (b.0, b.1)).unwrap_or((0.0, 0.0))
}

/// Layer geometry in its own box, starting at the origin and already mirrored.
///
/// Every consumer (canvas, preview, job) goes through this function, so a mirror flag is
/// applied exactly once and in the same way everywhere.
pub fn layer_segments(layer: &Layer) -> Result<Vec<Segment>, String> {
    match &layer.source {
        LayerSource::Image { path } => image_segments(path, layer),
        LayerSource::Svg { path } => svg_segments(path, layer),
        LayerSource::Gcode { path } => {
            let raw = parse_gcode_file_segments(Path::new(path));
            let flipped = mirror::flip_segments(raw, layer.mirror_x, layer.mirror_y);
            Ok(shift_to_origin(flipped))
        }
        LayerSource::Gerber { path } => {
            let segments = parse_gerber_segments(Path::new(path))
                .map_err(|e| format!("gerber '{}': {}", path, e))?;
            let flipped = mirror::flip_segments(segments, layer.mirror_x, layer.mirror_y);
            Ok(shift_to_origin(flipped))
        }
        LayerSource::Vector => Ok(vector_segments(layer)),
    }
}

fn shift_to_origin(mut segments: Vec<Segment>) -> Vec<Segment> {
    let Some((min_x, min_y, _, _)) = mirror::bounds(&segments) else {
        return segments;
    };

    for segment in &mut segments {
        segment.0 -= min_x;
        segment.1 -= min_y;
        segment.2 -= min_x;
        segment.3 -= min_y;
    }

    segments
}

fn size_mm(layer: &Layer) -> (f64, f64) {
    layer
        .bounds()
        .map(|b| ((b.2 - b.0).max(1.0), (b.3 - b.1).max(1.0)))
        .unwrap_or((100.0, 100.0))
}

fn raster_options(layer: &Layer) -> RasterizeOptions {
    RasterizeOptions {
        pixels_per_mm: layer.params.pixels_per_mm,
        threshold: layer.params.threshold,
        invert: layer.params.invert,
    }
}

fn single_pass_params(layer: &Layer) -> LayerParams {
    let mut params = layer.params.clone();
    params.passes = 1;
    params
}

fn raster_segments(raster: &crate::core::image::RasterImage, layer: &Layer) -> Vec<Segment> {
    engrave_segments(raster, &single_pass_params(layer), (0.0, 0.0))
        .unwrap_or_default()
        .into_iter()
        .map(|s| (s.x0, s.y0, s.x1, s.y1))
        .collect()
}

fn image_segments(path: &str, layer: &Layer) -> Result<Vec<Segment>, String> {
    let loaded = load_image(Path::new(path)).map_err(|e| format!("image '{}': {}", path, e))?;
    let image = mirror::flip_image(loaded.dynamic, layer.mirror_x, layer.mirror_y);
    let (width_mm, _) = size_mm(layer);
    let raster = rasterize(&image, raster_options(layer), Some(width_mm));
    Ok(raster_segments(&raster, layer))
}

fn svg_segments(path: &str, layer: &Layer) -> Result<Vec<Segment>, String> {
    let (width_mm, height_mm) = size_mm(layer);

    if layer.params.engrave_mode == EngraveMode::Lines {
        let target_px = (layer.params.pixels_per_mm * width_mm)
            .round()
            .clamp(512.0, 4096.0) as u32;

        let image = svg_trace::rasterize_svg_to_image(Path::new(path), target_px)
            .map_err(|e| format!("svg '{}': {}", path, e))?;
        let image = mirror::flip_image(image, layer.mirror_x, layer.mirror_y);

        let paths = svg_trace::trace_image_to_paths(&image, 128);
        let scale_x = width_mm / target_px as f64;
        let scale_y = width_mm / image.width().max(1) as f64;

        let mut out = Vec::new();
        for path in paths {
            if path.points.len() < 2 {
                continue;
            }
            for pair in path.points.windows(2) {
                out.push((
                    pair[0].0 * scale_x,
                    pair[0].1 * scale_y,
                    pair[1].0 * scale_x,
                    pair[1].1 * scale_y,
                ));
            }
            if path.closed
                && let (Some(first), Some(last)) = (path.points.first(), path.points.last())
                && ((first.0 - last.0).abs() > 1e-6 || (first.1 - last.1).abs() > 1e-6)
            {
                out.push((
                    last.0 * scale_x,
                    last.1 * scale_y,
                    first.0 * scale_x,
                    first.1 * scale_y,
                ));
            }
        }
        return Ok(out);
    }

    let target_px = (layer.params.pixels_per_mm * width_mm)
        .round()
        .clamp(64.0, 4096.0) as u32;
    let (box_w, box_h) = svg_raster_box(width_mm, height_mm, target_px);
    let image = rasterize_svg_into_box(Path::new(path), box_w, box_h)
        .map_err(|e| format!("svg '{}': {}", path, e))?;
    let image = mirror::flip_image(image, layer.mirror_x, layer.mirror_y);

    let raster = rasterize(&image, raster_options(layer), Some(width_mm));
    Ok(raster_segments(&raster, layer))
}

fn vector_segments(layer: &Layer) -> Vec<Segment> {
    let mut out = Vec::new();

    for shape in &layer.shapes {
        let mut path = VectorPath::new();
        path.nodes = shape.nodes.clone();
        path.stroke = shape.stroke.clone();
        path.closed = shape.closed;

        let points = path.flatten(0.1);
        if points.len() < 2 {
            continue;
        }

        for pair in points.windows(2) {
            out.push((pair[0].x, pair[0].y, pair[1].x, pair[1].y));
        }

        if path.closed
            && let (Some(last), Some(first)) = (points.last(), points.first())
        {
            out.push((last.x, last.y, first.x, first.y));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layer::LayerSource;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lightlaser-geometry-{}", name));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_svg(dir: &Path, name: &str) -> String {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10mm" height="10mm" viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10" fill="white"/><rect x="0" y="0" width="1" height="10" fill="black"/><rect x="8" y="0" width="2" height="10" fill="black"/></svg>"##;
        let path = dir.join(name);
        std::fs::write(&path, svg).unwrap();
        path.to_string_lossy().to_string()
    }

    fn svg_layer(path: &str) -> Layer {
        let mut layer = Layer::new(
            "svg".to_string(),
            LayerSource::Svg {
                path: path.to_string(),
            },
        );
        layer.set_bounds(0.0, 0.0, 10.0, 10.0);
        layer.params.pixels_per_mm = 8.0;
        layer
    }

    fn write_gcode(dir: &Path, name: &str) -> String {
        let gcode = "G21\nG90\nG0 X0 Y0\nG1 X3 Y0\nG0 X1 Y1\nG1 X3 Y1\n";
        let path = dir.join(name);
        std::fs::write(&path, gcode).unwrap();
        path.to_string_lossy().to_string()
    }

    fn gcode_layer(path: &str) -> Layer {
        let mut layer = Layer::new(
            "gcode".to_string(),
            LayerSource::Gcode {
                path: path.to_string(),
            },
        );
        layer.set_bounds(0.0, 0.0, 3.0, 1.0);
        layer
    }

    fn bounds_of(segments: &[Segment]) -> Segment {
        mirror::bounds(segments).unwrap()
    }

    fn close(a: Segment, b: Segment) -> bool {
        (a.0 - b.0).abs() < 1e-6
            && (a.1 - b.1).abs() < 1e-6
            && (a.2 - b.2).abs() < 1e-6
            && (a.3 - b.3).abs() < 1e-6
    }

    #[test]
    fn mirrored_svg_keeps_the_same_box() {
        let dir = temp_dir("svg");
        let layer = svg_layer(&write_svg(&dir, "shape.svg"));

        let plain = bounds_of(&layer_segments(&layer).unwrap());

        let mut mirrored = layer.clone();
        mirrored.mirror_x = true;
        let flipped = bounds_of(&layer_segments(&mirrored).unwrap());

        assert!(close(plain, flipped));
    }

    #[test]
    fn mirrored_svg_moves_the_drawing_to_the_other_side() {
        let dir = temp_dir("svg-side");
        let layer = svg_layer(&write_svg(&dir, "shape.svg"));

        let plain_x = x_endpoints(&layer_segments(&layer).unwrap());
        assert!(
            plain_x.iter().any(|x| (*x - 8.0).abs() < 0.3),
            "{:?}",
            plain_x
        );
        assert!(
            !plain_x.iter().any(|x| (*x - 2.0).abs() < 0.3),
            "{:?}",
            plain_x
        );

        let mut mirrored = layer.clone();
        mirrored.mirror_x = true;
        let flipped_x = x_endpoints(&layer_segments(&mirrored).unwrap());

        assert!(
            flipped_x.iter().any(|x| (*x - 2.0).abs() < 0.3),
            "{:?}",
            flipped_x
        );
        assert!(
            !flipped_x.iter().any(|x| (*x - 8.0).abs() < 0.3),
            "{:?}",
            flipped_x
        );
    }

    fn x_endpoints(segments: &[Segment]) -> Vec<f64> {
        let mut out = Vec::new();
        for (x0, _, x1, _) in segments {
            out.push(*x0);
            out.push(*x1);
        }
        out
    }

    #[test]
    fn mirrored_svg_in_lines_mode_is_flipped_too() {
        let dir = temp_dir("svg-lines");
        let mut layer = svg_layer(&write_svg(&dir, "lines.svg"));
        layer.params.engrave_mode = EngraveMode::Lines;

        let plain = bounds_of(&layer_segments(&layer).unwrap());

        layer.mirror_x = true;
        let flipped = bounds_of(&layer_segments(&layer).unwrap());

        assert!(close(plain, flipped));
    }

    #[test]
    fn mirroring_svg_twice_restores_the_geometry() {
        let dir = temp_dir("svg-twice");
        let layer = svg_layer(&write_svg(&dir, "twice.svg"));

        let plain = layer_segments(&layer).unwrap();

        let mut mirrored = layer.clone();
        mirrored.mirror_x = true;
        let once = layer_segments(&mirrored).unwrap();
        mirrored.mirror_x = false;
        let twice = layer_segments(&mirrored).unwrap();

        for (a, b) in plain.iter().zip(twice.iter()) {
            assert!(close(*a, *b));
        }
        assert_eq!(plain.len(), twice.len());
        assert_ne!(x_endpoints(&once), x_endpoints(&plain));
    }

    #[test]
    fn mirrored_gcode_flips_inside_its_own_box() {
        let dir = temp_dir("gcode");
        let layer = gcode_layer(&write_gcode(&dir, "part.nc"));

        let plain = layer_segments(&layer).unwrap();
        let plain_x = x_endpoints(&plain);
        assert!(
            plain_x.iter().any(|x| (*x - 1.0).abs() < 1e-6),
            "{:?}",
            plain_x
        );
        assert!(
            !plain_x.iter().any(|x| (*x - 2.0).abs() < 1e-6),
            "{:?}",
            plain_x
        );

        let mut mirrored = layer.clone();
        mirrored.mirror_x = true;
        let flipped = layer_segments(&mirrored).unwrap();
        let flipped_x = x_endpoints(&flipped);

        assert!(close(bounds_of(&plain), bounds_of(&flipped)));
        assert!(
            flipped_x.iter().any(|x| (*x - 2.0).abs() < 1e-6),
            "{:?}",
            flipped_x
        );
        assert!(
            !flipped_x.iter().any(|x| (*x - 1.0).abs() < 1e-6),
            "{:?}",
            flipped_x
        );
    }

    #[test]
    fn moving_a_layer_does_not_change_its_geometry() {
        let dir = temp_dir("move");
        let mut layer = gcode_layer(&write_gcode(&dir, "move.nc"));
        let before = layer_segments(&layer).unwrap();

        layer.translate(25.0, 7.0);
        let after = layer_segments(&layer).unwrap();

        for (a, b) in before.iter().zip(after.iter()) {
            assert!(close(*a, *b));
        }
    }

    #[test]
    fn placement_follows_the_bottom_left_corner() {
        let dir = temp_dir("placement");
        let mut layer = gcode_layer(&write_gcode(&dir, "place.nc"));

        assert_eq!(placement(&layer), (0.0, 0.0));

        layer.translate(12.5, 3.0);

        assert_eq!(placement(&layer), (12.5, 3.0));
    }

    #[test]
    fn gerber_geometry_is_anchored_at_its_own_origin() {
        let dir = temp_dir("gerber");
        let path = dir.join("part.gbr");
        std::fs::write(
            &path,
            "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.100000*%\nG01*\nD10*\nX1000000Y1000000D02*\nX2000000Y1000000D01*\nM02*\n",
        )
        .unwrap();

        let mut layer = Layer::new(
            "gerber".to_string(),
            LayerSource::Gerber {
                path: path.to_string_lossy().to_string(),
            },
        );
        layer.set_bounds(10.0, 10.0, 20.0, 10.0);

        let plain = layer_segments(&layer).unwrap();
        let plain_bounds = bounds_of(&plain);

        assert!(!plain.is_empty());
        assert!(plain_bounds.0.abs() < 1e-6, "{:?}", plain_bounds);
        assert!(plain_bounds.1.abs() < 1e-6, "{:?}", plain_bounds);
        assert!(plain_bounds.2 > 0.5 && plain_bounds.3 < 0.5, "{:?}", plain_bounds);

        let mut mirrored = layer.clone();
        mirrored.mirror_x = true;
        let flipped = layer_segments(&mirrored).unwrap();

        assert!(close(bounds_of(&flipped), plain_bounds), "{:?}", bounds_of(&flipped));
    }
}
