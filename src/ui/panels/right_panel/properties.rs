use rust_i18n::t;

use crate::core::layer::{EngraveMode, LayerParams, LayerSource};
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn delete_selection_public(&mut self) {
        let sel = self.ui_state.selection.clone();
        if let (Some(li), Some(si)) = (sel.layer, sel.shape) {
            self.push_history();
            if let Some(layer) = self.project.layers.get_mut(li) {
                if si < layer.shapes.len() {
                    layer.shapes.remove(si);
                    layer.recompute_bounds();
                }
            }
            self.ui_state.selection.shape = None;
            self.ui_state.selection.node = None;
            self.ui_state.selection.handle = None;
        } else if let Some(li) = sel.layer {
            self.push_history();
            self.project.remove_layer(li);
            self.ui_state.selection.clear();
        }
    }

    pub(crate) fn ui_layer_params(&mut self, ui: &mut egui::Ui, idx: usize) {
        let (name, source_label, params, color) = {
            let l = &self.project.layers[idx];
            (
                l.name.clone(),
                l.source.kind_label(),
                l.params.clone(),
                l.color,
            )
        };

        let mut new_name = name;
        let mut new_params: LayerParams = params;
        let mut color32 = egui::Color32::from_rgb(color[0], color[1], color[2]);

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.heading(format!("{} #{}", t!("ui.label.layer"), idx + 1));
            ui.label(format!("{}: {}", t!("ui.label.source"), source_label));
        });
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.layer_name").to_string());
            ui.text_edit_singleline(&mut new_name)
                .on_hover_text(t!("ui.tip.layer_name").to_string());
        });

        ui.horizontal(|ui| {
            ui.label("Color");
            if ui
                .color_edit_button_srgba(&mut color32)
                .on_hover_text(t!("ui.tip.color").to_string())
                .changed()
            {
                self.project.layers[idx].color = [color32.r(), color32.g(), color32.b()];
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(egui::RichText::new(t!("ui.label.params").to_string()).strong());
        ui.add_space(4.0);

        ui.add(
            egui::Slider::new(&mut new_params.power_percent, 0.0..=100.0)
                .text(format!("{} (%)", t!("ui.label.power")))
                .suffix("%"),
        )
        .on_hover_text(t!("ui.tip.power").to_string());

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.feed_rate").to_string());
            ui.add(
                egui::DragValue::new(&mut new_params.feed_rate)
                    .speed(10.0)
                    .range(1.0..=20000.0)
                    .suffix(" mm/min"),
            )
            .on_hover_text(t!("ui.tip.feed_rate").to_string());
        });

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.passes").to_string());
            ui.add(
                egui::DragValue::new(&mut new_params.passes)
                    .speed(0.1)
                    .range(1..=100),
            )
            .on_hover_text(t!("ui.tip.passes").to_string());
        });

        ui.checkbox(&mut new_params.dynamic_power, t!("ui.label.dynamic_power"))
            .on_hover_text(t!("ui.tip.dynamic_power").to_string());

        let is_svg = matches!(self.project.layers[idx].source, LayerSource::Svg { .. });
        if is_svg {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Mode:");
                ui.selectable_value(&mut new_params.engrave_mode, EngraveMode::Raster, "Raster")
                    .on_hover_text(t!("ui.tip.engrave_raster").to_string());
                ui.selectable_value(&mut new_params.engrave_mode, EngraveMode::Lines, "Lines")
                    .on_hover_text(t!("ui.tip.engrave_lines").to_string());
            });
        }

        let is_raster = matches!(
            self.project.layers[idx].source,
            LayerSource::Image { .. } | LayerSource::Svg { .. }
        );
        if is_raster && (is_svg && new_params.engrave_mode == EngraveMode::Raster || !is_svg) {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new(t!("ui.label.image_params").to_string()).strong());
            ui.add_space(4.0);

            ui.add(egui::Slider::new(&mut new_params.pixels_per_mm, 1.0..=50.0).text("px/mm"))
                .on_hover_text(t!("ui.tip.pixels_per_mm").to_string());

            ui.add(
                egui::Slider::new(&mut new_params.threshold, 0..=255)
                    .text(t!("ui.label.threshold").to_string()),
            )
            .on_hover_text(t!("ui.tip.threshold").to_string());

            ui.checkbox(&mut new_params.dither, t!("ui.label.dither"))
                .on_hover_text(t!("ui.tip.dither").to_string());
            ui.checkbox(&mut new_params.invert, t!("ui.label.invert"))
                .on_hover_text(t!("ui.tip.invert").to_string());
        }

        let layer = &mut self.project.layers[idx];
        if layer.name != new_name {
            layer.name = new_name;
        }
        if layer.params.power_percent != new_params.power_percent
            || layer.params.feed_rate != new_params.feed_rate
            || layer.params.passes != new_params.passes
            || layer.params.dynamic_power != new_params.dynamic_power
            || layer.params.pixels_per_mm != new_params.pixels_per_mm
            || layer.params.threshold != new_params.threshold
            || layer.params.dither != new_params.dither
            || layer.params.invert != new_params.invert
            || layer.params.engrave_mode != new_params.engrave_mode
        {
            layer.params = new_params;
        }

        if let Some(si) = self.ui_state.selection.shape {
            if si < self.project.layers[idx].shapes.len() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Shape").strong());
                ui.add_space(4.0);

                let mut width = self.project.layers[idx].shapes[si].stroke.width_mm;
                if ui
                    .add(egui::Slider::new(&mut width, 0.05..=5.0).text("Line width (mm)"))
                    .on_hover_text(t!("ui.tip.line_width").to_string())
                    .changed()
                {
                    self.project.layers[idx].shapes[si].stroke.width_mm = width;
                }

                let mut closed = self.project.layers[idx].shapes[si].closed;
                if ui
                    .checkbox(&mut closed, "Closed path")
                    .on_hover_text(t!("ui.tip.closed_path").to_string())
                    .changed()
                {
                    self.project.layers[idx].shapes[si].closed = closed;
                }
            }
        }
    }
}
