mod draw;
mod input;
mod state;

pub use state::CanvasState;
pub(crate) use input::duplicate_selected;

use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_canvas(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());

        let rect = response.rect;
        self.canvas.rect = rect;

        if self.canvas.pending_center {
            self.center_view();
        }

        let transform = crate::ui::CanvasTransform::new(rect, self.canvas.zoom, self.canvas.offset);

        draw::render_background(&painter, &transform, self);
        draw::render_layers(&painter, &transform, self);

        if let Some(status) = &self.grbl_status {
            if let Some((x, y, _z)) = status.mpos {
                let pos = transform.to_screen(x, y);
                painter.circle_filled(pos, 4.0, crate::ui::colors::LASER_POSITION);
                painter.circle_stroke(
                    pos,
                    8.0,
                    egui::Stroke::new(1.0, crate::ui::colors::LASER_POSITION),
                );
            }
        }

        input::handle_interaction(self, ui, &response, &transform);

        draw::render_overlays(&painter, &transform, self);

        egui::Area::new(egui::Id::new("canvas_controls"))
            .fixed_pos(egui::pos2(rect.max.x - 110.0, rect.max.y - 40.0))
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("−").clicked() {
                        self.zoom_out();
                    }
                    if ui.button("+").clicked() {
                        self.zoom_in();
                    }
                    if ui.button("⊡").clicked() {
                        self.center_view();
                    }
                });
            });
    }

    pub(crate) fn zoom_in(&mut self) {
        self.zoom_on_position(1.15, egui::Vec2::ZERO);
    }

    pub(crate) fn zoom_out(&mut self) {
        self.zoom_on_position(0.85, egui::Vec2::ZERO);
    }

    pub(crate) fn zoom_on_position(&mut self, factor: f32, position: egui::Vec2) {
        let old_zoom = self.canvas.zoom;
        let new_zoom = (old_zoom * factor).clamp(state::MIN_ZOOM, state::MAX_ZOOM);
        self.canvas.zoom = new_zoom;
        self.canvas.offset = position + (self.canvas.offset - position) * (new_zoom / old_zoom);
    }

    pub(crate) fn center_view(&mut self) {
        if self.canvas.rect.width() <= 0.0 || self.canvas.rect.height() <= 0.0 {
            self.canvas.pending_center = true;
            return;
        }

        let bb = self.get_scene_bounding_box();
        if bb.is_infinite() {
            self.canvas.zoom = 1.0;
            self.canvas.offset = egui::Vec2::ZERO;
        } else {
            let (min_x, min_y, max_x, max_y) = bb.to_tuples();
            let w = (max_x - min_x) as f32;
            let h = (max_y - min_y) as f32;

            if w > 0.0 && h > 0.0 {
                let sx = self.canvas.rect.width() * 0.9 / w;
                let sy = self.canvas.rect.height() * 0.9 / h;
                self.canvas.zoom = sx.min(sy).clamp(state::MIN_ZOOM, state::MAX_ZOOM);
            } else {
                self.canvas.zoom = 1.0;
            }

            let cx = ((min_x + max_x) / 2.0) as f32;
            let cy = ((min_y + max_y) / 2.0) as f32;
            self.canvas.offset.x = -cx * self.canvas.zoom;
            self.canvas.offset.y = cy * self.canvas.zoom;
        }

        self.canvas.pending_center = false;
    }

    pub(crate) fn get_scene_bounding_box(&self) -> crate::core::bounding_box::BoundingBox {
        let mut bb = crate::core::bounding_box::BoundingBox::new();

        for layer in &self.project.layers {
            if let Some((min_x, min_y, max_x, max_y)) = layer.bounds() {
                bb.update_from_rect(min_x, min_y, max_x, max_y);
            }
        }

        if self.app_config.show_work_area {
            let (w, h) = self.project.machine.work_area_mm;
            bb.update_from_rect(0.0, 0.0, w, h);
        }

        bb
    }
}