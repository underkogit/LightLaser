use crate::core::config::MachineConfig;
use crate::ui::colors;
use crate::ui::CanvasTransform;

pub fn draw_grid(painter: &egui::Painter, transform: &CanvasTransform, step_mm: f64) {
    if step_mm <= 0.0 || transform.scale <= 0.0 {
        return;
    }

    let step_px = step_mm as f32 * transform.scale;
    if step_px < 4.0 {
        return;
    }

    let rect = transform.rect;
    let (wmin_x, wmin_y) = transform.to_world(egui::pos2(rect.min.x, rect.max.y));
    let (wmax_x, wmax_y) = transform.to_world(egui::pos2(rect.max.x, rect.min.y));

    let x_start = (wmin_x / step_mm).floor() as i64;
    let x_end = (wmax_x / step_mm).ceil() as i64;
    let y_start = (wmin_y / step_mm).floor() as i64;
    let y_end = (wmax_y / step_mm).ceil() as i64;

    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 50));

    for i in x_start..=x_end {
        let x = i as f64 * step_mm;
        let top = transform.to_screen(x, wmax_y);
        let bot = transform.to_screen(x, wmin_y);
        painter.line_segment([top, bot], stroke);
    }

    for j in y_start..=y_end {
        let y = j as f64 * step_mm;
        let left = transform.to_screen(wmin_x, y);
        let right = transform.to_screen(wmax_x, y);
        painter.line_segment([left, right], stroke);
    }
}

pub fn draw_axes(painter: &egui::Painter, transform: &CanvasTransform) {
    let rect = transform.rect;
    let (wmin_x, wmin_y) = transform.to_world(egui::pos2(rect.min.x, rect.max.y));
    let (wmax_x, wmax_y) = transform.to_world(egui::pos2(rect.max.x, rect.min.y));

    if wmin_y <= 0.0 && wmax_y >= 0.0 {
        let a = transform.to_screen(wmin_x, 0.0);
        let b = transform.to_screen(wmax_x, 0.0);
        painter.line_segment([a, b], egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 60, 60)));
    }

    if wmin_x <= 0.0 && wmax_x >= 0.0 {
        let a = transform.to_screen(0.0, wmin_y);
        let b = transform.to_screen(0.0, wmax_y);
        painter.line_segment([a, b], egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 180, 60)));
    }
}

pub fn draw_work_area(painter: &egui::Painter, transform: &CanvasTransform, machine: &MachineConfig) {
    let (w, h) = machine.work_area_mm;
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let p0 = transform.to_screen(0.0, h);
    let p1 = transform.to_screen(w, 0.0);
    let rect = egui::Rect::from_two_pos(p0, p1);
    painter.rect_filled(rect, 0.0, colors::WORK_AREA_FILL);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, colors::WORK_AREA),
        egui::StrokeKind::Outside,
    );
}