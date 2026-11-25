use egui::{Color32, Painter, Pos2, Rect, Stroke};

use crate::core::image::{rasterize_svg_into_box, svg_raster_box};
use crate::core::layer::{Layer, LayerSource, placement};
use crate::core::vector::VectorPath;
use crate::ui::selection::HandleKind;
use crate::ui::{
    CanvasTransform, LaserApp, colors, line_raster,
    widget::canvas::{draw_axes, draw_grid, draw_work_area},
};

use super::cache;
use super::state::{DragMode, SegmentCacheEntry};

pub fn render_background(painter: &Painter, transform: &CanvasTransform, app: &LaserApp) {
    let rect = transform.rect;
    painter.rect_filled(rect, 0.0, Color32::from_rgb(13, 13, 18));

    if app.app_config.show_grid {
        draw_grid(painter, transform, app.app_config.grid_step_mm);
    }
    if app.app_config.show_axes {
        draw_axes(painter, transform);
    }
    if app.app_config.show_work_area {
        draw_work_area(painter, transform, &app.project.machine);
        draw_dimensions(painter, transform, app.project.machine.work_area_mm);
    }
}

/// Draws the width and height of the work area around it.
fn draw_dimensions(painter: &Painter, transform: &CanvasTransform, (w, h): (f64, f64)) {
    draw_rect_dimensions(painter, transform, (0.0, 0.0, w, h));
}

/// Draws the width and height of a world-space rectangle just outside it.
///
/// Shared by the work area and the current selection, so both read the same way.
fn draw_rect_dimensions(
    painter: &Painter,
    transform: &CanvasTransform,
    (min_x, min_y, max_x, max_y): (f64, f64, f64, f64),
) {
    let w = max_x - min_x;
    let h = max_y - min_y;
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let text_color = Color32::from_rgb(180, 180, 200);
    let line_color = Color32::from_rgb(120, 120, 150);
    let stroke = Stroke::new(1.0, line_color);
    let font = egui::FontId::proportional(12.0);

    let p_bot_left = transform.to_screen(min_x, min_y);
    let p_bot_right = transform.to_screen(max_x, min_y);
    let p_top_left = transform.to_screen(min_x, max_y);

    let offset_y = 15.0;
    let tick_size = 5.0;

    let y_line = p_bot_left.y + offset_y;
    painter.line_segment(
        [
            egui::pos2(p_bot_left.x, y_line),
            egui::pos2(p_bot_right.x, y_line),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(p_bot_left.x, y_line - tick_size),
            egui::pos2(p_bot_left.x, y_line + tick_size),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(p_bot_right.x, y_line - tick_size),
            egui::pos2(p_bot_right.x, y_line + tick_size),
        ],
        stroke,
    );

    let w_text = format!("{:.1} mm", w);
    painter.text(
        egui::pos2(p_bot_left.x + (p_bot_right.x - p_bot_left.x) / 2.0, y_line),
        egui::Align2::CENTER_CENTER,
        w_text,
        font.clone(),
        text_color,
    );

    let offset_x = 15.0;
    let x_line = p_bot_left.x - offset_x;
    painter.line_segment(
        [
            egui::pos2(x_line, p_bot_left.y),
            egui::pos2(x_line, p_top_left.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(x_line - tick_size, p_bot_left.y),
            egui::pos2(x_line + tick_size, p_bot_left.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(x_line - tick_size, p_top_left.y),
            egui::pos2(x_line + tick_size, p_top_left.y),
        ],
        stroke,
    );

    let h_text = format!("{:.1} mm", h);
    painter.text(
        egui::pos2(x_line, p_bot_left.y + (p_top_left.y - p_bot_left.y) / 2.0),
        egui::Align2::CENTER_CENTER,
        h_text,
        font,
        text_color,
    );
}

/// World-space bounding box of whatever is currently selected.
///
/// Returns `None` when nothing is selected, which is what keeps the dimensions off the
/// canvas until an object is picked.
fn selection_bounds(app: &LaserApp) -> Option<(f64, f64, f64, f64)> {
    let sel = &app.ui_state.selection;

    // A selected shape is measured on its own rather than through its whole layer.
    if let (Some(li), Some(si)) = (sel.layer, sel.shape) {
        let layer = app.project.layers.get(li)?;
        let shape = layer.shapes.get(si)?;

        let mut path = VectorPath::new();
        path.nodes = shape.nodes.clone();
        path.stroke = shape.stroke.clone();
        path.closed = shape.closed;
        return path.bounds(0.1);
    }

    let mut ids = sel.layers.clone();
    if let Some(id) = sel.layer {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut any = false;

    for id in ids {
        if let Some(layer) = app.project.layers.get(id) {
            if let Some((a, b, c, d)) = layer.bounds() {
                min_x = min_x.min(a);
                min_y = min_y.min(b);
                max_x = max_x.max(c);
                max_y = max_y.max(d);
                any = true;
            }
        }
    }

    if any && min_x.is_finite() && min_y.is_finite() {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

/// Label drawn on top of a layer: its name plus the kind of source it came from.
fn source_label(layer: &Layer) -> String {
    format!("Source: {} ({})", layer.name, layer.source.display_label())
}

pub fn render_layers(
    ui: &mut egui::Ui,
    painter: &Painter,
    transform: &CanvasTransform,
    app: &mut LaserApp,
) {
    let selection = app.ui_state.selection.clone();

    for (i, layer) in app.project.layers.iter().enumerate() {
        if !app.project.layer_visible(layer) {
            continue;
        }
        let layer_selected = selection.is_layer_selected(i);
        draw_layer(
            ui,
            painter,
            transform,
            layer,
            layer_selected,
            selection.shape,
            selection.node,
            selection.handle,
            &mut app.canvas.texture_cache,
            &mut app.canvas.segment_cache,
        );
    }
}

pub fn render_overlays(painter: &Painter, transform: &CanvasTransform, app: &LaserApp) {
    let active_tool = app.tools.current != crate::core::tools::ToolKind::Select;

    if active_tool {
        let preview = app
            .tools
            .active_tool()
            .preview(&|p: crate::core::vector::CurvePoint| transform.to_screen(p.x, p.y));

        for (line, color) in &preview.polylines {
            if line.len() >= 2 {
                painter.add(egui::Shape::line(line.clone(), Stroke::new(1.5, *color)));
            }
        }
        for (p, color) in &preview.points {
            painter.circle_filled(*p, 3.0, *color);
        }
        for (p, r, color) in &preview.circles {
            painter.circle_stroke(*p, *r, Stroke::new(1.0, *color));
        }

        let hint = app.tools.active_tool().hint();
        let hint_pos = egui::pos2(transform.rect.min.x + 10.0, transform.rect.max.y - 28.0);
        egui::Area::new(egui::Id::new("tool_hint"))
            .fixed_pos(hint_pos)
            .order(egui::Order::Foreground)
            .show(painter.ctx(), |ui| {
                ui.colored_label(Color32::from_rgb(220, 220, 220), hint);
            });
    }

    if let DragMode::RubberBand {
        start_world,
        current_world,
    } = app.canvas.drag
    {
        let p0 = transform.to_screen(start_world.x, start_world.y);
        let p1 = transform.to_screen(current_world.x, current_world.y);
        let r = Rect::from_two_pos(p0, p1);
        painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(80, 160, 255, 30));
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(120, 200, 255)),
            egui::StrokeKind::Inside,
        );
    }

    // Dimensions of the selection, drawn exactly like the work area's.
    if let Some(bounds) = selection_bounds(app) {
        draw_rect_dimensions(painter, transform, bounds);
    }

    draw_snap_indicator(painter, transform, app);
}

/// Marks the grid intersection nearest to the pointer.
///
/// Shown whenever the canvas is hovered, whether or not the grid itself is drawn, so the
/// snap position is always visible. Holding Shift moves objects freely instead of snapping
/// them, so the marker is hidden then: it would otherwise promise a snap that cannot happen.
fn draw_snap_indicator(painter: &Painter, transform: &CanvasTransform, app: &LaserApp) {
    if painter.ctx().input(|i| i.modifiers.shift) {
        return;
    }

    let step = app.app_config.grid_step_mm;
    if step <= 0.0 {
        return;
    }

    let Some(pointer) = painter.ctx().pointer_hover_pos() else {
        return;
    };
    if !transform.rect.contains(pointer) {
        return;
    }

    let (world_x, world_y) = transform.to_world(pointer);
    let snapped = transform.to_screen(
        (world_x / step).round() * step,
        (world_y / step).round() * step,
    );

    painter.circle_stroke(snapped, 6.0, Stroke::new(1.0, colors::SNAP_POINT));
    painter.circle_filled(snapped, 2.0, colors::SNAP_POINT);
}

#[inline]
fn segment_visible(a: Pos2, b: Pos2, view: Rect, pad: f32) -> bool {
    let min_x = a.x.min(b.x) - pad;
    let max_x = a.x.max(b.x) + pad;
    let min_y = a.y.min(b.y) - pad;
    let max_y = a.y.max(b.y) + pad;
    max_x >= view.min.x && min_x <= view.max.x && max_y >= view.min.y && min_y <= view.max.y
}

/// Texture coordinates that flip the bitmap to match the layer's mirror flags.
///
/// Image and SVG layers are drawn as a texture, so the mirror is applied here instead of
/// rebuilding the texture.
fn texture_uv(mirror_x: bool, mirror_y: bool) -> Rect {
    let (u_min, u_max) = if mirror_x { (1.0, 0.0) } else { (0.0, 1.0) };
    let (v_min, v_max) = if mirror_y { (1.0, 0.0) } else { (0.0, 1.0) };

    Rect::from_min_max(Pos2::new(u_min, v_min), Pos2::new(u_max, v_max))
}

/// Draws a segment based layer (G-code).
///
/// Thousands of separate line shapes per frame are far too expensive, so the layer is
/// rasterized once into a texture which is then drawn as a single quad. The texture is
/// only rebuilt when the zoom crosses a power of two, which keeps panning/zooming cheap.
/// When the cached texture would be too coarse for the current zoom, the segments are
/// drawn directly with view culling plus a sub-pixel simplification pass.
fn draw_segment_layer(
    ui: &mut egui::Ui,
    painter: &Painter,
    transform: &CanvasTransform,
    layer: &Layer,
    entry: &SegmentCacheEntry,
    color: Color32,
    texture_cache: &mut std::collections::HashMap<String, egui::TextureHandle>,
) {
    let Some((min_x, min_y, max_x, max_y)) = entry.bbox else {
        return;
    };

    let (ox, oy) = placement(layer);

    let p0 = transform.to_screen(min_x + ox, max_y + oy);
    let p1 = transform.to_screen(max_x + ox, min_y + oy);
    let rect = Rect::from_two_pos(p0, p1);

    if rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }

    let ppp = ui.ctx().pixels_per_point();
    let needed = transform.scale as f64 * ppp as f64;
    let max_mm = (max_x - min_x).max(max_y - min_y).max(1e-3);
    let level = line_raster::texture_level(needed, max_mm);
    let screen_px_per_texel = (needed / level).max(0.25);

    if screen_px_per_texel <= 2.0 {
        if let Some(key) = cache::layer_cache_key(layer) {
            let texture = texture_cache
                .entry(format!("gsegs::{}::{}", key, level))
                .or_insert_with(|| {
                    ui.ctx().load_texture(
                        format!("layer_gsegs_{}_{}", layer.name, level),
                        rasterize_segments(entry, level),
                        egui::TextureOptions::LINEAR,
                    )
                });

            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                color,
            );
            return;
        }
    }

    draw_segments_lod(painter, transform, &entry.segments, ox, oy, color);
}

fn rasterize_segments(entry: &SegmentCacheEntry, level: f64) -> egui::ColorImage {
    let Some((min_x, min_y, max_x, max_y)) = entry.bbox else {
        return egui::ColorImage::filled([1, 1], Color32::TRANSPARENT);
    };

    let width_px = ((max_x - min_x) * level).ceil().max(1.0) as u32;
    let height_px = ((max_y - min_y) * level).ceil().max(1.0) as u32;

    let Some(mut pixmap) = line_raster::make_pixmap(width_px, height_px) else {
        return egui::ColorImage::filled([1, 1], Color32::TRANSPARENT);
    };

    line_raster::stroke_segments(
        &mut pixmap,
        entry.segments.iter().map(|(x0, y0, x1, y1)| {
            (
                ((x0 - min_x) * level) as f32,
                ((max_y - y0) * level) as f32,
                ((x1 - min_x) * level) as f32,
                ((max_y - y1) * level) as f32,
            )
        }),
        [255, 255, 255, 255],
    );

    line_raster::to_color_image(&pixmap)
}

/// Fallback for zoom levels where a texture would be too blurry.
///
/// Segments shorter than `TOLERANCE_PX` are merged into a single straight line, which
/// removes the sub-pixel work of dense raster engraving while keeping the picture intact.
fn draw_segments_lod(
    painter: &Painter,
    transform: &CanvasTransform,
    segments: &[(f64, f64, f64, f64)],
    ox: f64,
    oy: f64,
    color: Color32,
) {
    const TOLERANCE_PX: f32 = 0.6;

    let stroke = Stroke::new(1.0, color);
    let view = transform.rect;
    let pad = 2.0f32;

    let mut prev_end: Option<Pos2> = None;
    let mut last_emitted: Option<Pos2> = None;

    for (x0, y0, x1, y1) in segments {
        let a = transform.to_screen(x0 + ox, y0 + oy);
        let b = transform.to_screen(x1 + ox, y1 + oy);

        if !segment_visible(a, b, view, pad) {
            prev_end = Some(b);
            last_emitted = None;
            continue;
        }

        match last_emitted {
            Some(from) if prev_end.is_some_and(|p| (p - a).length() <= 0.01) => {
                if (b - from).length() >= TOLERANCE_PX {
                    painter.line_segment([from, b], stroke);
                    last_emitted = Some(b);
                }
            }
            _ => {
                painter.line_segment([a, b], stroke);
                last_emitted = Some(b);
            }
        }

        prev_end = Some(b);
    }
}


#[allow(clippy::too_many_arguments)]
fn draw_layer(
    ui: &mut egui::Ui,
    painter: &Painter,
    transform: &CanvasTransform,
    layer: &Layer,
    layer_selected: bool,
    selected_shape: Option<usize>,
    selected_node: Option<usize>,
    selected_handle: Option<HandleKind>,
    texture_cache: &mut std::collections::HashMap<String, egui::TextureHandle>,
    segment_cache: &mut std::collections::HashMap<String, SegmentCacheEntry>,
) {
    if layer.is_vector() {
        draw_vector_layer(
            painter,
            transform,
            layer,
            layer_selected,
            selected_shape,
            selected_node,
            selected_handle,
        );
        return;
    }

    let (min_x, min_y, max_x, max_y) = match layer.bounds() {
        Some(b) => b,
        None => return,
    };

    let p0 = transform.to_screen(min_x, max_y);
    let p1 = transform.to_screen(max_x, min_y);
    let rect = Rect::from_two_pos(p0, p1);

    if rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }

    let key = match &layer.source {
        LayerSource::Image { path } => Some(("img", path.clone())),
        LayerSource::Svg { path } => Some(("svg", path.clone())),
        _ => None,
    };

    if let Some((kind, path_str)) = key {
        let cache_key = format!("{}::{}", kind, path_str);

        let texture = texture_cache.entry(cache_key).or_insert_with(|| {
            let color_image = load_layer_texture(&kind, &path_str, (min_x, min_y, max_x, max_y));

            ui.ctx().load_texture(
                format!("layer_tex_{}", layer.name),
                color_image,
                egui::TextureOptions::LINEAR,
            )
        });

        painter.image(
            texture.id(),
            rect,
            texture_uv(layer.mirror_x, layer.mirror_y),
            if layer_selected {
                Color32::from_rgba_unmultiplied(255, 255, 255, 200)
            } else {
                Color32::WHITE
            },
        );
    } else {
        let color = if layer_selected {
            colors::PATH_LAYER_SELECTED
        } else {
            colors::PATH_LAYER
        };

        match cache::get_or_build(segment_cache, layer) {
            Some(entry) => draw_segment_layer(
                ui,
                painter,
                transform,
                layer,
                entry,
                color,
                texture_cache,
            ),
            None => {
                let alpha = if layer_selected { 180 } else { 120 };
                let fallback = Color32::from_rgba_unmultiplied(
                    colors::PATH_LAYER.r(),
                    colors::PATH_LAYER.g(),
                    colors::PATH_LAYER.b(),
                    alpha,
                );
                painter.rect_filled(rect, 2.0, fallback);
            }
        }
    }

    if layer_selected {
        painter.rect_stroke(
            rect,
            2.0,
            Stroke::new(1.5, colors::PATH_LAYER_SELECTED),
            egui::StrokeKind::Outside,
        );
    }

    if transform.scale > 0.3 {
        painter.text(
            egui::pos2(rect.min.x + 4.0, rect.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            source_label(layer),
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
}

fn draw_vector_layer(
    painter: &Painter,
    transform: &CanvasTransform,
    layer: &Layer,
    layer_selected: bool,
    selected_shape: Option<usize>,
    selected_node: Option<usize>,
    selected_handle: Option<HandleKind>,
) {
    for (si, shape) in layer.shapes.iter().enumerate() {
        let mut path = VectorPath::new();
        path.nodes = shape.nodes.clone();
        path.stroke = shape.stroke.clone();
        path.closed = shape.closed;

        let pts = path.flatten(0.1);
        if pts.len() < 2 {
            continue;
        }

        let screen: Vec<Pos2> = pts.iter().map(|p| transform.to_screen(p.x, p.y)).collect();

        let shape_selected = layer_selected && selected_shape == Some(si);

        let stroke_color = if shape_selected {
            colors::PATH_LAYER_SELECTED
        } else {
            colors::PATH_LAYER
        };

        let width = (shape.stroke.width_mm as f32 * transform.scale).clamp(1.0, 6.0);
        painter.add(egui::Shape::line(screen, Stroke::new(width, stroke_color)));

        if shape_selected && transform.scale > 0.2 {
            for (ni, node) in shape.nodes.iter().enumerate() {
                let p = transform.to_screen(node.point.x, node.point.y);
                let is_sel = selected_node == Some(ni);
                let color = if is_sel {
                    Color32::from_rgb(255, 200, 60)
                } else {
                    Color32::from_rgb(200, 200, 200)
                };
                painter.circle_filled(p, if is_sel { 4.5 } else { 3.0 }, color);

                if is_sel && node.has_handles() {
                    let hi = transform.to_screen(node.handle_in.x, node.handle_in.y);
                    let ho = transform.to_screen(node.handle_out.x, node.handle_out.y);
                    painter
                        .line_segment([p, hi], Stroke::new(1.0, Color32::from_rgb(120, 200, 255)));
                    painter
                        .line_segment([p, ho], Stroke::new(1.0, Color32::from_rgb(120, 200, 255)));
                    let in_col = if selected_handle == Some(HandleKind::In) {
                        Color32::from_rgb(255, 120, 120)
                    } else {
                        Color32::from_rgb(255, 160, 60)
                    };
                    let out_col = if selected_handle == Some(HandleKind::Out) {
                        Color32::from_rgb(255, 120, 120)
                    } else {
                        Color32::from_rgb(255, 160, 60)
                    };
                    painter.circle_filled(hi, 4.0, in_col);
                    painter.circle_filled(ho, 4.0, out_col);
                }
            }
        }
    }

    if let Some((min_x, min_y, max_x, max_y)) = layer.bounds() {
        let p0 = transform.to_screen(min_x, max_y);
        let p1 = transform.to_screen(max_x, min_y);
        let rect = Rect::from_two_pos(p0, p1);
        if layer_selected {
            painter.rect_stroke(
                rect,
                2.0,
                Stroke::new(1.0, colors::PATH_LAYER_SELECTED),
                egui::StrokeKind::Outside,
            );
        }
        if transform.scale > 0.3 {
            painter.text(
                egui::pos2(rect.min.x + 4.0, rect.min.y + 4.0),
                egui::Align2::LEFT_TOP,
                source_label(layer),
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }
}

fn load_layer_texture(kind: &str, path: &str, bounds: (f64, f64, f64, f64)) -> egui::ColorImage {
    let (min_x, min_y, max_x, max_y) = bounds;
    let width_mm = (max_x - min_x).max(1.0);
    let height_mm = (max_y - min_y).max(1.0);
    let aspect = height_mm / width_mm;

    let target_w = 1024u32;
    let target_h = ((target_w as f64) * aspect).round().clamp(1.0, 4096.0) as u32;

    if kind == "svg" {
        // The box carries the layer's aspect ratio, so drawing this texture into the layer's
        // bounds cannot stretch the drawing.
        let (box_w, box_h) = svg_raster_box(width_mm, height_mm, target_w);
        match rasterize_svg_into_box(std::path::Path::new(path), box_w, box_h) {
            Ok(image) => {
                let rgba = image.to_rgba8();
                let (w, h) = rgba.dimensions();
                return egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
            }
            Err(e) => {
                log::warn!("svg texture failed: {}", e);
                return egui::ColorImage::filled([1, 1], Color32::TRANSPARENT);
            }
        }
    }

    if let Ok(img) = image::open(path) {
        let resized = if img.width() > target_w || img.height() > target_h {
            img.resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        let rgba = resized.to_rgba8();
        let (w, h) = rgba.dimensions();
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw())
    } else {
        egui::ColorImage::filled([1, 1], Color32::TRANSPARENT)
    }
}
