mod cache;
mod draw;
mod input;
pub(crate) mod state;

pub use state::CanvasState;

use crate::core::layer::LayerSource;
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
        draw::render_layers(ui, &painter, &transform, self);

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
            .fixed_pos(egui::pos2(rect.max.x - 120.0, rect.max.y - 40.0))
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

        let painter_clone = painter.clone();
        let mouse_x = self.status_bar.mouse_x();
        let mouse_y = self.status_bar.mouse_y();
        let color = if mouse_x.is_finite() && mouse_y.is_finite() {
            egui::Color32::LIGHT_GRAY
        } else {
            egui::Color32::DARK_GRAY
        };
        let text = format!("X: {:.3} mm, Y: {:.3} mm", mouse_x, mouse_y);

        painter_clone.text(
            egui::pos2(rect.min.x + 10.0, rect.max.y - 20.0),
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::monospace(14.0),
            color,
        );

        let mut has_selection = !self.ui_state.selection.layers.is_empty();
        if self.ui_state.selection.layer.is_some() {
            has_selection = true;
        }

        response.context_menu(|ui| {
            if ui.button("Preview").clicked() {
                self.open_preview();
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(has_selection, egui::Button::new("Copy"))
                .clicked()
            {
                self.copy_selection();
                ui.close();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .clicked()
            {
                self.delete_selection_public();
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(has_selection, egui::Button::new("Mirror X"))
                .clicked()
            {
                self.mirror_selection_x();
                ui.close();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Mirror Y"))
                .clicked()
            {
                self.mirror_selection_y();
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(has_selection, egui::Button::new("Round X to 0.1"))
                .clicked()
            {
                self.align_selection(0.1);
                ui.close();
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Round X to 1.0"))
                .clicked()
            {
                self.align_selection(1.0);
                ui.close();
            }
        });
    }

    pub(crate) fn align_selection(&mut self, step: f64) {
        let sel = self.ui_state.selection.clone();
        let mut ids = sel.layers.clone();
        if let Some(id) = sel.layer {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.is_empty() {
            return;
        }
        self.push_history();
        for id in ids {
            if let Some(layer) = self.project.layers.get_mut(id) {
                layer.offset[0] = (layer.offset[0] / step).round() * step;
                layer.offset[1] = (layer.offset[1] / step).round() * step;
                if let Some(b) = layer.bounds.as_mut() {
                    b[0] = (b[0] / step).round() * step;
                    b[1] = (b[1] / step).round() * step;
                    b[2] = (b[2] / step).round() * step;
                    b[3] = (b[3] / step).round() * step;
                }
                for shape in &mut layer.shapes {
                    for node in &mut shape.nodes {
                        node.point.x = (node.point.x / step).round() * step;
                        node.point.y = (node.point.y / step).round() * step;
                    }
                }
            }
        }
    }

    pub(crate) fn mirror_selection_x(&mut self) {
        self.mirror_selection(true);
    }

    pub(crate) fn mirror_selection_y(&mut self) {
        self.mirror_selection(false);
    }

    fn mirror_selection(&mut self, mirror_x: bool) {
        let sel = self.ui_state.selection.clone();
        let mut ids = sel.layers.clone();
        if let Some(id) = sel.layer {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.is_empty() {
            return;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut any = false;

        for &id in &ids {
            if let Some(layer) = self.project.layers.get(id) {
                if let Some((a, b, c, d)) = layer.bounds() {
                    min_x = min_x.min(a);
                    min_y = min_y.min(b);
                    max_x = max_x.max(c);
                    max_y = max_y.max(d);
                    any = true;
                }
            }
        }

        if !any {
            return;
        }

        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;

        self.push_history();

        for &id in &ids {
            if let Some(layer) = self.project.layers.get_mut(id) {
                let is_gcode = matches!(layer.source, LayerSource::Gcode { .. });

                if is_gcode {
                    if let Some((min_x_l, min_y_l, max_x_l, max_y_l)) = layer.bounds() {
                        let local_cx = (min_x_l + max_x_l) / 2.0;
                        let local_cy = (min_y_l + max_y_l) / 2.0;
                        let shift_x = cx - local_cx;
                        let shift_y = cy - local_cy;

                        layer.offset[0] += if mirror_x { 2.0 * shift_x } else { 0.0 };
                        layer.offset[1] += if !mirror_x { 2.0 * shift_y } else { 0.0 };

                        let center = layer.mirror_center.unwrap_or([0.0, 0.0]);
                        let new_center = if mirror_x {
                            [layer.offset[0] + local_cx, center[1]]
                        } else {
                            [center[0], layer.offset[1] + local_cy]
                        };
                        let _ = new_center;

                        layer.mirror_center = Some([local_cx, local_cy]);

                        if mirror_x {
                            layer.mirror_x = !layer.mirror_x;
                        } else {
                            layer.mirror_y = !layer.mirror_y;
                        }

                        if let Some(b) = layer.bounds.as_mut() {
                            let a = *b;
                            if mirror_x {
                                b[0] = 2.0 * cx - a[2];
                                b[2] = 2.0 * cx - a[0];
                            } else {
                                b[1] = 2.0 * cy - a[3];
                                b[3] = 2.0 * cy - a[1];
                            }
                        }
                    }
                    continue;
                }

                if mirror_x {
                    for shape in &mut layer.shapes {
                        for n in &mut shape.nodes {
                            n.point.x = 2.0 * cx - n.point.x;
                            n.handle_in.x = 2.0 * cx - n.handle_in.x;
                            n.handle_out.x = 2.0 * cx - n.handle_out.x;
                        }
                        shape.nodes.reverse();
                    }
                    layer.offset[0] = 2.0 * cx - layer.offset[0];
                    if let Some(b) = layer.bounds.as_mut() {
                        let a = *b;
                        b[0] = 2.0 * cx - a[2];
                        b[2] = 2.0 * cx - a[0];
                    }
                } else {
                    for shape in &mut layer.shapes {
                        for n in &mut shape.nodes {
                            n.point.y = 2.0 * cy - n.point.y;
                            n.handle_in.y = 2.0 * cy - n.handle_in.y;
                            n.handle_out.y = 2.0 * cy - n.handle_out.y;
                        }
                        shape.nodes.reverse();
                    }
                    layer.offset[1] = 2.0 * cy - layer.offset[1];
                    if let Some(b) = layer.bounds.as_mut() {
                        let a = *b;
                        b[1] = 2.0 * cy - a[3];
                        b[3] = 2.0 * cy - a[1];
                    }
                }
            }
        }

        self.canvas.segment_cache.clear();
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
