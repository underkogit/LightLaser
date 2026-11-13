use egui::{Color32, Pos2, Rect, Stroke, Vec2};

use crate::core::gcode::{normalize_gcode, parse_gcode_file_segments, transform_for_layer};
use crate::core::image::{
    RasterizeOptions, engrave_segments, load_image, rasterize, rasterize_svg, svg_trace,
};
use crate::core::layer::{EngraveMode, Layer, LayerSource};
use crate::core::vector::VectorPath;
use crate::ui::LaserApp;

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

                draw_preview_grid_lines(
                    &painter,
                    rect,
                    &self.canvas.preview_window,
                    self.app_config.grid_step_mm,
                );
                draw_preview_axes(&painter, rect, &self.canvas.preview_window);
                draw_preview_bounds(&painter, rect, &self.canvas.preview_window);
                draw_preview_lines(&painter, rect, &self.canvas.preview_window);

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

fn draw_preview_lines(
    painter: &egui::Painter,
    rect: Rect,
    pw: &crate::ui::panels::canvas::state::PreviewWindow,
) {
    let total = pw.lines.len();
    let current_count = (total as f32 * pw.progress) as usize;

    for (i, &(x0, y0, x1, y1)) in pw.lines.iter().enumerate() {
        let a = to_screen(pw, rect, x0, y0);
        let b = to_screen(pw, rect, x1, y1);

        if i < current_count {
            painter.line_segment([a, b], Stroke::new(1.0, Color32::from_rgb(255, 140, 40)));
        } else {
            painter.line_segment(
                [a, b],
                Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 140, 40, 30)),
            );
        }
    }
}
