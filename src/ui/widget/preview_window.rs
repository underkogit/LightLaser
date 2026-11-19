use std::time::{Duration, Instant};

use egui::{Color32, Pos2, Rect, Stroke, Vec2};

use crate::core::gcode::{normalize_gcode, parse_gcode_file_segments, transform_for_layer};
use crate::core::image::{
    RasterizeOptions, engrave_segments, load_image, rasterize, rasterize_svg, svg_trace,
};
use crate::core::layer::{EngraveMode, Layer, LayerSource};
use crate::core::vector::VectorPath;
use crate::ui::line_raster;
use crate::ui::panels::canvas::state::PreviewWindow;
use crate::ui::LaserApp;

/// Rasterizations that took less than this are cheap enough to redo on every frame.
const CHEAP_RENDER: Duration = Duration::from_millis(4);

/// Minimum interval between two expensive rasterizations while the view is still moving.
/// Stretching the stale texture in between keeps panning and zooming responsive.
const INTERACTIVE_RENDER_INTERVAL: Duration = Duration::from_millis(100);

/// Minimum interval between two updates of the processed texture while the progress
/// slider is dragged.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(33);

/// Extra margin around the visible area that is included in the rasterized texture, as a
/// fraction of the visible size. Lets short pans reuse the cached texture.
const WORLD_PADDING: f64 = 0.2;

/// Color of segments that are not processed yet.
const PENDING_COLOR: [u8; 4] = [255, 140, 40, 30];

/// Color of already processed segments.
const DONE_COLOR: [u8; 4] = [255, 140, 40, 255];

impl LaserApp {
    pub(crate) fn open_preview(&mut self) {
        self.canvas.preview_window.open = true;
        self.canvas.preview_window.pending = true;
    }

    pub(crate) fn ui_preview_window(&mut self, ctx: &egui::Context) {
        if !self.canvas.preview_window.open {
            return;
        }

        let mut open = self.canvas.preview_window.open;

        egui::Window::new("Preview")
            .id(egui::Id::new("preview_window"))
            .open(&mut open)
            .resizable(true)
            .default_size([800.0, 600.0])
            .min_size([400.0, 300.0])
            .default_pos(egui::pos2(60.0, 60.0))
            .collapsible(true)
            .show(ctx, |ui| {
                let panel_height = 64.0;
                let canvas_height = (ui.available_height() - panel_height).max(50.0);

                let (response, painter) = ui.allocate_painter(
                    egui::vec2(ui.available_width(), canvas_height),
                    egui::Sense::click_and_drag(),
                );
                let rect = response.rect;

                if self.canvas.preview_window.pending {
                    rebuild_preview(self);
                    self.canvas.preview_window.pending = false;
                }

                if self.canvas.preview_window.pending_fit {
                    fit_preview(&mut self.canvas.preview_window, rect);
                    self.canvas.preview_window.pending_fit = false;
                }

                painter.rect_filled(rect, 0.0, Color32::from_rgb(13, 13, 18));

                if ui.input(|i| i.pointer.middle_down()) && response.dragged() {
                    self.canvas.preview_window.offset += response.drag_delta();
                }

                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 && response.hovered() {
                    let factor = 1.0 + scroll * 0.001;
                    let center = response.hover_pos().unwrap_or(rect.center()) - rect.center();
                    let old = self.canvas.preview_window.zoom;
                    let new = (old * factor).clamp(0.05, 100.0);
                    self.canvas.preview_window.offset =
                        center + (self.canvas.preview_window.offset - center) * (new / old);
                    self.canvas.preview_window.zoom = new;
                }

                let ppp = ui.ctx().pixels_per_point();
                update_preview_raster(ui.ctx(), &mut self.canvas.preview_window, rect, ppp);

                draw_preview_grid_lines(
                    &painter,
                    rect,
                    &self.canvas.preview_window,
                    self.app_config.grid_step_mm,
                );
                draw_preview_axes(&painter, rect, &self.canvas.preview_window);
                draw_preview_bounds(&painter, rect, &self.canvas.preview_window);
                draw_preview_raster(&painter, rect, &self.canvas.preview_window);

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.canvas.preview_window.pending = true;
                    }
                    if ui.button("Fit").clicked() {
                        self.canvas.preview_window.pending_fit = true;
                    }
                    let count = self.canvas.preview_window.lines.len();
                    ui.label(format!("Segments: {}", count));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let total = self.canvas.preview_window.lines.len();
                        if total > 0 {
                            let current_count =
                                (total as f32 * self.canvas.preview_window.progress) as usize;
                            ui.label(format!("{}/{}", current_count, total));
                        }
                    });
                });

                ui.add_space(4.0);

                let total = self.canvas.preview_window.lines.len();
                let mut progress = self.canvas.preview_window.progress;

                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 16.0),
                    egui::Sense::click_and_drag(),
                );

                if response.dragged() || response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let local_x = (pos.x - rect.min.x).clamp(0.0, rect.width());
                        progress = local_x / rect.width();
                        self.canvas.preview_window.progress = progress;
                    }
                }

                let painter = ui.painter();
                let track_rect = Rect::from_min_max(
                    egui::pos2(rect.min.x, rect.center().y - 3.0),
                    egui::pos2(rect.max.x, rect.center().y + 3.0),
                );

                painter.rect_filled(track_rect, 3.0, Color32::from_rgb(40, 40, 50));

                if total > 0 {
                    let fill_rect = Rect::from_min_max(
                        track_rect.min,
                        egui::pos2(
                            track_rect.min.x + track_rect.width() * progress,
                            track_rect.max.y,
                        ),
                    );
                    painter.rect_filled(fill_rect, 3.0, Color32::from_rgb(255, 140, 40));

                    let handle_pos = egui::pos2(
                        track_rect.min.x + track_rect.width() * progress,
                        track_rect.center().y,
                    );
                    painter.circle_filled(handle_pos, 8.0, Color32::from_rgb(255, 140, 40));
                    painter.circle_filled(handle_pos, 6.0, Color32::WHITE);
                }
            });

        self.canvas.preview_window.open = open;
    }
}

fn rebuild_preview(app: &mut LaserApp) {
    let mut lines: Vec<(f64, f64, f64, f64)> = Vec::new();

    for layer in &app.project.layers {
        if !layer.is_enabled || !layer.is_visible {
            continue;
        }
        let segments = build_layer_segments(layer);
        lines.extend(segments);
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (x0, y0, x1, y1) in &lines {
        min_x = min_x.min(*x0).min(*x1);
        min_y = min_y.min(*y0).min(*y1);
        max_x = max_x.max(*x0).max(*x1);
        max_y = max_y.max(*y0).max(*y1);
    }

    let bounds = if min_x.is_finite() && min_y.is_finite() {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    };

    app.canvas.preview_window.lines = lines;
    app.canvas.preview_window.bounds = bounds;
    app.canvas.preview_window.progress = 1.0;
    app.canvas.preview_window.revision = app.canvas.preview_window.revision.wrapping_add(1);
    app.canvas.preview_window.raster = Default::default();
}

fn layer_origin(layer: &Layer) -> (f64, f64) {
    if layer.is_vector() {
        (layer.offset[0], layer.offset[1])
    } else {
        layer.bounds().map(|b| (b.0, b.1)).unwrap_or((0.0, 0.0))
    }
}

fn build_layer_segments(layer: &Layer) -> Vec<(f64, f64, f64, f64)> {
    if layer.is_vector() {
        let mut out = Vec::new();
        for shape in &layer.shapes {
            let mut path = VectorPath::new();
            path.nodes = shape.nodes.clone();
            path.stroke = shape.stroke.clone();
            path.closed = shape.closed;
            let pts = path.flatten(0.1);
            if pts.len() < 2 {
                continue;
            }
            for w in pts.windows(2) {
                out.push((w[0].x, w[0].y, w[1].x, w[1].y));
            }
            if path.closed {
                if let (Some(last), Some(first)) = (pts.last(), pts.first()) {
                    out.push((last.x, last.y, first.x, first.y));
                }
            }
        }
        return out;
    }

    let origin = layer_origin(layer);
    let width_mm = layer
        .bounds()
        .map(|b| (b.2 - b.0).max(1.0))
        .unwrap_or(100.0);

    match &layer.source {
        LayerSource::Image { path } => {
            let Ok(loaded) = load_image(std::path::Path::new(path)) else {
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

                let Ok(img) =
                    svg_trace::rasterize_svg_to_image(std::path::Path::new(path), target_px)
                else {
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
                let Ok(svg) = rasterize_svg(std::path::Path::new(path), target_px) else {
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
            let raw = parse_gcode_file_segments(std::path::Path::new(path));
            let transformed = transform_for_layer(raw, layer);
            let segs = normalize_gcode(transformed);
            let ox = layer.offset[0];
            let oy = layer.offset[1];
            segs.into_iter()
                .map(|(x0, y0, x1, y1)| (x0 + ox, y0 + oy, x1 + ox, y1 + oy))
                .collect()
        }
        LayerSource::Gerber { path } => {
            let ox = layer.offset[0];
            let oy = layer.offset[1];
            crate::core::image::parse_gerber_segments(std::path::Path::new(path))
                .unwrap_or_default()
                .into_iter()
                .map(|(x0, y0, x1, y1)| (x0 + ox, y0 + oy, x1 + ox, y1 + oy))
                .collect()
        }
        LayerSource::Vector => Vec::new(),
    }
}

fn fit_preview(pw: &mut crate::ui::panels::canvas::state::PreviewWindow, rect: Rect) {
    let Some((min_x, min_y, max_x, max_y)) = pw.bounds else {
        pw.zoom = 1.0;
        pw.offset = Vec2::ZERO;
        return;
    };
    let w = (max_x - min_x).max(1.0) as f32;
    let h = (max_y - min_y).max(1.0) as f32;

    let avail_w = (rect.width() - 40.0).max(50.0);
    let avail_h = (rect.height() - 40.0).max(50.0);

    pw.zoom = (avail_w / w).min(avail_h / h).clamp(0.05, 100.0);

    let cx = ((min_x + max_x) / 2.0) as f32;
    let cy = ((min_y + max_y) / 2.0) as f32;
    pw.offset = Vec2::new(-cx * pw.zoom, cy * pw.zoom);
}

fn to_screen(
    pw: &crate::ui::panels::canvas::state::PreviewWindow,
    rect: Rect,
    x: f64,
    y: f64,
) -> Pos2 {
    egui::pos2(
        rect.center().x + x as f32 * pw.zoom + pw.offset.x,
        rect.center().y - y as f32 * pw.zoom + pw.offset.y,
    )
}

fn to_world(
    pw: &crate::ui::panels::canvas::state::PreviewWindow,
    rect: Rect,
    pos: Pos2,
) -> (f64, f64) {
    (
        ((pos.x - rect.center().x - pw.offset.x) / pw.zoom) as f64,
        (-(pos.y - rect.center().y - pw.offset.y) / pw.zoom) as f64,
    )
}

fn draw_preview_grid_lines(
    painter: &egui::Painter,
    rect: Rect,
    pw: &crate::ui::panels::canvas::state::PreviewWindow,
    step_mm: f64,
) {
    if step_mm <= 0.0 || pw.zoom <= 0.0 {
        return;
    }

    let step_px = step_mm as f32 * pw.zoom;
    if step_px < 6.0 {
        return;
    }

    let (wmin_x, wmin_y) = to_world(pw, rect, egui::pos2(rect.min.x, rect.max.y));
    let (wmax_x, wmax_y) = to_world(pw, rect, egui::pos2(rect.max.x, rect.min.y));

    let x_start = (wmin_x / step_mm).floor() as i64;
    let x_end = (wmax_x / step_mm).ceil() as i64;
    let y_start = (wmin_y / step_mm).floor() as i64;
    let y_end = (wmax_y / step_mm).ceil() as i64;

    let stroke = Stroke::new(1.0, Color32::from_rgb(40, 40, 50));

    for i in x_start..=x_end {
        let x = i as f64 * step_mm;
        let top = to_screen(pw, rect, x, wmax_y);
        let bot = to_screen(pw, rect, x, wmin_y);
        painter.line_segment([top, bot], stroke);
    }

    for j in y_start..=y_end {
        let y = j as f64 * step_mm;
        let left = to_screen(pw, rect, wmin_x, y);
        let right = to_screen(pw, rect, wmax_x, y);
        painter.line_segment([left, right], stroke);
    }
}

fn draw_preview_axes(
    painter: &egui::Painter,
    rect: Rect,
    pw: &crate::ui::panels::canvas::state::PreviewWindow,
) {
    let origin = to_screen(pw, rect, 0.0, 0.0);

    let x_end = to_screen(pw, rect, 100.0, 0.0);
    let y_end = to_screen(pw, rect, 0.0, 100.0);

    let x_color = Color32::from_rgb(200, 60, 60);
    let y_color = Color32::from_rgb(60, 200, 60);

    painter.line_segment([origin, x_end], Stroke::new(1.0, x_color));
    painter.line_segment([origin, y_end], Stroke::new(1.0, y_color));

    painter.circle_filled(origin, 4.0, Color32::from_rgb(255, 255, 0));
    painter.circle_stroke(
        origin,
        8.0,
        Stroke::new(1.0, Color32::from_rgb(255, 255, 0)),
    );

    painter.text(
        origin + egui::vec2(10.0, 10.0),
        egui::Align2::LEFT_TOP,
        "0,0",
        egui::FontId::monospace(12.0),
        Color32::from_rgb(255, 255, 0),
    );
}

fn draw_preview_bounds(
    painter: &egui::Painter,
    rect: Rect,
    pw: &crate::ui::panels::canvas::state::PreviewWindow,
) {
    if let Some((min_x, min_y, max_x, max_y)) = pw.bounds {
        let p0 = to_screen(pw, rect, min_x, max_y);
        let p1 = to_screen(pw, rect, max_x, min_y);
        let r = Rect::from_two_pos(p0, p1);
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, Color32::from_rgb(80, 80, 100)),
            egui::StrokeKind::Outside,
        );
    }
}

/// Keeps the cached rasterization of the preview geometry in sync with the view.
///
/// A huge amount of segments cannot be redrawn every frame, so an up-to-date texture is
/// reused (stretched to the current view) while the view is still moving, and the crisp
/// rasterization happens as soon as the view stops changing.
fn update_preview_raster(ctx: &egui::Context, pw: &mut PreviewWindow, rect: Rect, ppp: f32) {
    if pw.lines.is_empty() {
        return;
    }

    let total = pw.lines.len();
    let done = ((total as f32 * pw.progress.clamp(0.0, 1.0)) as usize).min(total);
    let key = preview_view_key(pw, rect, ppp);

    if pw.raster.key != key {
        let visible = visible_world_rect(pw, rect);
        let view_settled = pw.raster.pending_key == key;
        let uncovered = visible.is_some_and(|v| !covers(pw.raster.world_rect, v));

        // Never rasterize more often than the previous one took, so a heavy job cannot
        // block the frame it was started in over and over while the view keeps moving.
        let interval = INTERACTIVE_RENDER_INTERVAL.max(pw.raster.last_cost * 4);
        let throttled = pw
            .raster
            .last_render
            .is_some_and(|t| t.elapsed() < interval);

        let must_render = pw.raster.base.is_none()
            || view_settled
            || pw.raster.last_cost < CHEAP_RENDER
            || (uncovered && !throttled);

        if must_render {
            let started = Instant::now();
            render_preview_raster(ctx, pw, rect, ppp, done);
            pw.raster.last_cost = started.elapsed();
            pw.raster.last_render = Some(Instant::now());
            pw.raster.key = key;
            pw.raster.pending_key = key;
            return;
        }

        pw.raster.pending_key = key;

        // The view is still moving and the texture is stale or does not cover it yet;
        // make sure a frame comes along that can pick up the settled render.
        ctx.request_repaint_after(interval);
    }

    if pw.raster.drawn != done {
        update_preview_done(ctx, pw, done);
    }
}

fn preview_view_key(pw: &PreviewWindow, rect: Rect, ppp: f32) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    pw.revision.hash(&mut hasher);
    rect.width().to_bits().hash(&mut hasher);
    rect.height().to_bits().hash(&mut hasher);
    ppp.to_bits().hash(&mut hasher);
    pw.zoom.to_bits().hash(&mut hasher);
    pw.offset.x.to_bits().hash(&mut hasher);
    pw.offset.y.to_bits().hash(&mut hasher);
    hasher.finish()
}

fn covers(outer: (f64, f64, f64, f64), inner: (f64, f64, f64, f64)) -> bool {
    outer.0 <= inner.0 && outer.1 <= inner.1 && outer.2 >= inner.2 && outer.3 >= inner.3
}

/// Rasterizes the visible geometry into the base and progress textures.
fn render_preview_raster(
    ctx: &egui::Context,
    pw: &mut PreviewWindow,
    rect: Rect,
    ppp: f32,
    done: usize,
) {
    let Some((min_x, min_y, max_x, max_y)) = visible_world_rect(pw, rect) else {
        return;
    };

    let pad_x = (max_x - min_x) * WORLD_PADDING;
    let pad_y = (max_y - min_y) * WORLD_PADDING;
    let world_rect = (min_x - pad_x, min_y - pad_y, max_x + pad_x, max_y + pad_y);

    let needed = pw.zoom.max(1e-3) as f64 * ppp.max(0.1) as f64;
    let max_mm = (world_rect.2 - world_rect.0)
        .max(world_rect.3 - world_rect.1)
        .max(1e-3);
    let level = needed.min(line_raster::MAX_TEXTURE_DIM / max_mm).max(1e-3);

    let width_px = (((world_rect.2 - world_rect.0) * level).ceil().max(1.0)) as u32;
    let height_px = (((world_rect.3 - world_rect.1) * level).ceil().max(1.0)) as u32;

    let (Some(mut base), Some(mut processed)) = (
        line_raster::make_pixmap(width_px, height_px),
        line_raster::make_pixmap(width_px, height_px),
    ) else {
        return;
    };

    line_raster::stroke_segments(
        &mut base,
        pixel_segments(&pw.lines, world_rect, level),
        PENDING_COLOR,
    );

    let done_count = done.min(pw.lines.len());
    if done_count > 0 {
        line_raster::stroke_segments(
            &mut processed,
            pixel_segments(&pw.lines[..done_count], world_rect, level),
            DONE_COLOR,
        );
    }

    pw.raster.world_rect = world_rect;
    pw.raster.level = level;
    pw.raster.drawn = done_count;
    pw.raster.done_pixmap = Some(processed);

    upload_pixmap(ctx, &mut pw.raster.base, "preview_base", &base);
    if let Some(pixmap) = pw.raster.done_pixmap.as_ref() {
        upload_pixmap(ctx, &mut pw.raster.done, "preview_done", pixmap);
    }
}

/// Extends the "processed" texture up to `done` segments.
///
/// Growing the progress only strokes the new segments on top of the existing texture,
/// which keeps dragging the progress slider cheap even for very large jobs.
fn update_preview_done(ctx: &egui::Context, pw: &mut PreviewWindow, done: usize) {
    if pw.raster.done_pixmap.is_none() {
        return;
    }

    // Dragging the slider backwards means re-stroking every processed segment, which is
    // far too expensive to do on each frame of the drag.
    if pw
        .raster
        .last_progress
        .is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL)
    {
        ctx.request_repaint_after(PROGRESS_INTERVAL);
        return;
    }
    pw.raster.last_progress = Some(Instant::now());

    let world_rect = pw.raster.world_rect;
    let level = pw.raster.level;

    if done < pw.raster.drawn {
        if let Some(pixmap) = pw.raster.done_pixmap.as_mut() {
            pixmap.fill(tiny_skia::Color::TRANSPARENT);
        }
        pw.raster.drawn = 0;
    }

    let from = pw.raster.drawn.min(pw.lines.len());
    let to = done.min(pw.lines.len());

    if from < to {
        if let Some(pixmap) = pw.raster.done_pixmap.as_mut() {
            line_raster::stroke_segments(
                pixmap,
                pixel_segments(&pw.lines[from..to], world_rect, level),
                DONE_COLOR,
            );
        }

        pw.raster.drawn = to;
    }

    if let Some(pixmap) = pw.raster.done_pixmap.as_ref() {
        let image = line_raster::to_color_image(pixmap);
        upload_image(ctx, &mut pw.raster.done, "preview_done", image);
    }
}

/// Maps world-space segments into the pixel space of a rasterized texture.
///
/// Segments outside `world_rect` are skipped, so only what is really visible is stroked.
fn pixel_segments(
    lines: &[(f64, f64, f64, f64)],
    world_rect: (f64, f64, f64, f64),
    level: f64,
) -> impl Iterator<Item=(f32, f32, f32, f32)> + '_ {
    let (x0, y0, x1, y1) = world_rect;
    let pad = 1.0 / level;

    lines.iter().filter_map(move |&(ax, ay, bx, by)| {
        if ax.max(bx) < x0 - pad
            || ax.min(bx) > x1 + pad
            || ay.max(by) < y0 - pad
            || ay.min(by) > y1 + pad
        {
            return None;
        }

        Some((
            ((ax - x0) * level) as f32,
            ((y1 - ay) * level) as f32,
            ((bx - x0) * level) as f32,
            ((y1 - by) * level) as f32,
        ))
    })
}

fn visible_world_rect(pw: &PreviewWindow, rect: Rect) -> Option<(f64, f64, f64, f64)> {
    let (ax, ay) = to_world(pw, rect, egui::pos2(rect.min.x, rect.max.y));
    let (bx, by) = to_world(pw, rect, egui::pos2(rect.max.x, rect.min.y));

    let (min_x, max_x) = (ax.min(bx), ax.max(bx));
    let (min_y, max_y) = (ay.min(by), ay.max(by));

    if !(max_x > min_x && max_y > min_y) {
        return None;
    }

    Some((min_x, min_y, max_x, max_y))
}

fn upload_pixmap(
    ctx: &egui::Context,
    slot: &mut Option<egui::TextureHandle>,
    name: &str,
    pixmap: &tiny_skia::Pixmap,
) {
    upload_image(ctx, slot, name, line_raster::to_color_image(pixmap));
}

fn upload_image(
    ctx: &egui::Context,
    slot: &mut Option<egui::TextureHandle>,
    name: &str,
    image: egui::ColorImage,
) {
    match slot {
        Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
        None => *slot = Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR)),
    }
}

fn draw_preview_raster(painter: &egui::Painter, rect: Rect, pw: &PreviewWindow) {
    let (x0, y0, x1, y1) = pw.raster.world_rect;
    if !(x1 > x0 && y1 > y0) {
        return;
    }

    let p0 = to_screen(pw, rect, x0, y1);
    let p1 = to_screen(pw, rect, x1, y0);
    let screen_rect = Rect::from_two_pos(p0, p1);
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

    if let Some(texture) = &pw.raster.base {
        painter.image(texture.id(), screen_rect, uv, Color32::WHITE);
    }

    if let Some(texture) = &pw.raster.done {
        painter.image(texture.id(), screen_rect, uv, Color32::WHITE);
    }
}
