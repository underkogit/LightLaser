use egui::{Color32, Painter, Pos2, Rect, Stroke};

use crate::core::layer::Layer;
use crate::core::vector::VectorPath;
use crate::ui::selection::HandleKind;
use crate::ui::{
    CanvasTransform, LaserApp, colors,
    widget::canvas::{draw_axes, draw_grid, draw_work_area},
};

use super::state::DragMode;

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

fn draw_dimensions(painter: &Painter, transform: &CanvasTransform, (w, h): (f64, f64)) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let text_color = Color32::from_rgb(180, 180, 200);
    let line_color = Color32::from_rgb(120, 120, 150);
    let stroke = Stroke::new(1.0, line_color);
    let font = egui::FontId::proportional(12.0);

    let p_bot_left = transform.to_screen(0.0, 0.0);
    let p_bot_right = transform.to_screen(w, 0.0);
    let p_top_left = transform.to_screen(0.0, h);

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

pub fn render_layers(painter: &Painter, transform: &CanvasTransform, app: &LaserApp) {
    let selection = &app.ui_state.selection;

    for (i, layer) in app.project.layers.iter().enumerate() {
        if !layer.is_visible {
            continue;
        }
        let layer_selected = selection.layer == Some(i);
        draw_layer(
            painter,
            transform,
            layer,
            layer_selected,
            selection.shape,
            selection.node,
            selection.handle,
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
}

fn draw_layer(
    painter: &Painter,
    transform: &CanvasTransform,
    layer: &Layer,
    layer_selected: bool,
    selected_shape: Option<usize>,
    selected_node: Option<usize>,
    selected_handle: Option<HandleKind>,
) {
    if layer.is_vector() {
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
                Color32::from_rgb(layer.color[0], layer.color[1], layer.color[2])
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
                        painter.line_segment(
                            [p, hi],
                            Stroke::new(1.0, Color32::from_rgb(120, 200, 255)),
                        );
                        painter.line_segment(
                            [p, ho],
                            Stroke::new(1.0, Color32::from_rgb(120, 200, 255)),
                        );
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
                    &layer.name,
                    egui::FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
        }
        return;
    }

    let (min_x, min_y, max_x, max_y) = match layer.bounds() {
        Some(b) => b,
        None => return,
    };

    let p0 = transform.to_screen(min_x, max_y);
    let p1 = transform.to_screen(max_x, min_y);
    let rect = Rect::from_two_pos(p0, p1);

    let alpha = if layer_selected { 180 } else { 120 };
    let color =
        Color32::from_rgba_unmultiplied(layer.color[0], layer.color[1], layer.color[2], alpha);
    painter.rect_filled(rect, 2.0, color);

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
            &layer.name,
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
}