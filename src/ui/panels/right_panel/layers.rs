use egui::{Align, Layout};
use rust_i18n::t;

use crate::ui::selection::Selection;
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_layer_list(&mut self, ui: &mut egui::Ui) {
        let mut to_remove: Option<usize> = None;
        let mut to_move_up: Option<usize> = None;
        let mut to_move_down: Option<usize> = None;
        let mut new_selection: Option<usize> = self.ui_state.selection.layer;

        let selected = self.ui_state.selection.layer;

        for (i, layer) in self.project.layers.iter_mut().enumerate() {
            let is_selected = selected == Some(i);

            let frame = egui::Frame::group(ui.style())
                .fill(if is_selected {
                    ui.style().visuals.selection.bg_fill
                } else {
                    ui.style().visuals.faint_bg_color
                })
                .inner_margin(egui::Margin::symmetric(6, 4));

            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(
                        rect,
                        2.0,
                        egui::Color32::from_rgb(layer.color[0], layer.color[1], layer.color[2]),
                    );

                    let resp = ui.add(
                        egui::Label::new(egui::RichText::new(&layer.name).strong())
                            .sense(egui::Sense::click()),
                    );
                    if resp.clicked() {
                        new_selection = Some(i);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(format!(
                                "{}",
                                egui_phosphor::regular::TRASH
                            )))
                            .clicked()
                        {
                            to_remove = Some(i);
                        }

                        if ui
                            .add(
                                egui::Button::new(format!("{}", egui_phosphor::regular::ARROW_UP))
                                    .small(),
                            )
                            .clicked()
                        {
                            to_move_up = Some(i);
                        }
                        if ui
                            .add(
                                egui::Button::new(format!(
                                    "{}",
                                    egui_phosphor::regular::ARROW_DOWN
                                ))
                                    .small(),
                            )
                            .clicked()
                        {
                            to_move_down = Some(i);
                        }

                        if ui
                            .checkbox(&mut layer.is_visible, "")
                            .on_hover_text(t!("ui.tooltip.layer_visible"))
                            .changed()
                        {}

                        if ui
                            .checkbox(&mut layer.is_enabled, "")
                            .on_hover_text(t!("ui.tooltip.layer_enabled"))
                            .changed()
                        {}
                    });
                });
            });
            ui.add_space(4.0);
        }

        if let Some(i) = to_remove {
            self.project.remove_layer(i);
            if self.ui_state.selection.layer == Some(i) {
                self.ui_state.selection.clear();
            } else if let Some(sel) = self.ui_state.selection.layer {
                if sel > i {
                    self.ui_state.selection.layer = Some(sel - 1);
                }
            }
        }
        if let Some(i) = to_move_up {
            self.project.move_layer_up(i);
            if new_selection == Some(i) {
                new_selection = Some(i - 1);
            }
        }
        if let Some(i) = to_move_down {
            self.project.move_layer_down(i);
            if new_selection == Some(i) {
                new_selection = Some(i + 1);
            }
        }

        if new_selection != self.ui_state.selection.layer {
            self.ui_state.selection = Selection {
                layer: new_selection,
                shape: None,
                node: None,
                handle: None,
            };
        }
    }
}