use egui::Align;
use rust_i18n::t;

use crate::comm::{GrblState, StreamerState};
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top_bar")
            .resizable(false)
            .min_size(40.0)
            .max_size(40.0)
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    self.ui_top_menu(ui);

                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        self.ui_top_status(ui);
                        ui.add_space(8.0);
                        self.ui_top_connection(ui);
                    });
                });
            });
    }

    fn ui_top_menu(&mut self, ui: &mut egui::Ui) {
        let file_label = t!("ui.menu.file").to_string();
        let view_label = t!("ui.menu.view").to_string();
        let help_label = t!("ui.menu.help").to_string();

        ui.menu_button(file_label, |ui| {
            if ui.button(t!("ui.menu.new_project")).clicked() {
                self.new_project();
                ui.close();
            }
            if ui.button(t!("ui.menu.open_project")).clicked() {
                self.open_project_interactive();
                ui.close();
            }
            if ui.button(t!("ui.menu.save_project")).clicked() {
                self.save_project_interactive();
                ui.close();
            }
            if ui.button(t!("ui.menu.save_project_as")).clicked() {
                self.save_project_as_interactive();
                ui.close();
            }
            ui.separator();
            if ui.button(t!("ui.menu.add_image")).clicked() {
                self.add_image_interactive();
                ui.close();
            }
            if ui.button(t!("ui.menu.add_svg")).clicked() {
                self.add_svg_interactive();
                ui.close();
            }
            if ui.button(t!("ui.menu.add_gcode")).clicked() {
                self.add_gcode_interactive();
                ui.close();
            }
            ui.separator();
            if ui.button(t!("ui.menu.export_gcode")).clicked() {
                self.export_dialog_open = true;
                ui.close();
            }
            ui.separator();
            if ui.button(t!("ui.menu.exit")).clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });

        ui.menu_button(t!("ui.menu.edit").to_string(), |ui| {
            if ui
                .add_enabled(self.history.can_undo(), egui::Button::new(format!(
                    "{} {}",
                    egui_phosphor::regular::ARROW_U_UP_LEFT,
                    t!("ui.menu.undo")
                )))
                .clicked()
            {
                self.undo();
                ui.close();
            }
            if ui
                .add_enabled(self.history.can_redo(), egui::Button::new(format!(
                    "{} {}",
                    egui_phosphor::regular::ARROW_U_UP_RIGHT,
                    t!("ui.menu.redo")
                )))
                .clicked()
            {
                self.redo();
                ui.close();
            }
        });

        ui.menu_button(view_label, |ui| {
            if ui
                .button(format!(
                    "{} {}",
                    egui_phosphor::regular::MAGNIFYING_GLASS_PLUS,
                    t!("ui.menu.zoom_in")
                ))
                .clicked()
            {
                self.zoom_in();
                ui.close();
            }
            if ui
                .button(format!(
                    "{} {}",
                    egui_phosphor::regular::MAGNIFYING_GLASS_MINUS,
                    t!("ui.menu.zoom_out")
                ))
                .clicked()
            {
                self.zoom_out();
                ui.close();
            }
            if ui
                .button(format!(
                    "{} {}",
                    egui_phosphor::regular::ARROWS_OUT,
                    t!("ui.menu.center_view")
                ))
                .clicked()
            {
                self.center_view();
                ui.close();
            }
            ui.separator();
            ui.checkbox(&mut self.app_config.show_grid, t!("ui.menu.show_grid"));
            ui.checkbox(&mut self.app_config.show_axes, t!("ui.menu.show_axes"));
            ui.checkbox(
                &mut self.app_config.show_work_area,
                t!("ui.menu.show_work_area"),
            );
            ui.separator();
            if ui
                .checkbox(&mut self.tools.snap_enabled, t!("ui.menu.snap_to_grid"))
                .changed()
            {
                self.tools.snap_mm = self.app_config.grid_step_mm;
            }
            ui.add_enabled_ui(self.tools.snap_enabled, |ui| {
                let mut step = self.tools.snap_mm;
                if ui
                    .add(
                        egui::DragValue::new(&mut step)
                            .speed(0.1)
                            .range(0.01..=100.0)
                            .suffix(" mm"),
                    )
                    .changed()
                {
                    self.tools.snap_mm = step;
                }
            });
        });

        ui.menu_button(help_label, |ui| {
            ui.label(format!("LightLazer v{}", env!("CARGO_PKG_VERSION")));
        });
    }

    fn ui_top_connection(&mut self, ui: &mut egui::Ui) {
        let connected = self.grbl_status.is_some();

        let (label, color) = if connected {
            (t!("ui.label.connected").to_string(), egui::Color32::GREEN)
        } else {
            (t!("ui.label.disconnected").to_string(), egui::Color32::RED)
        };

        ui.colored_label(color, format!("● {}", label));

        let btn = if connected {
            egui::Button::new(t!("ui.button.disconnect"))
        } else {
            egui::Button::new(t!("ui.button.connect"))
        };

        if ui.add(btn).clicked() {
            if connected {
                self.streamer.send(crate::comm::StreamerCommand::Disconnect);
            } else {
                self.connection_dialog_open = true;
            }
        }
    }

    fn ui_top_status(&mut self, ui: &mut egui::Ui) {
        let (state_label, mpos) = match &self.grbl_status {
            Some(s) => (s.state.label(), s.mpos),
            None => (GrblState::Unknown.label(), None),
        };

        let state_color = match self.grbl_status.as_ref().map(|s| s.state) {
            Some(GrblState::Alarm) => egui::Color32::RED,
            Some(GrblState::Run) | Some(GrblState::Jog) => egui::Color32::YELLOW,
            Some(GrblState::Idle) => egui::Color32::GREEN,
            _ => egui::Color32::GRAY,
        };

        ui.colored_label(state_color, state_label);
        ui.label("|");
        if let Some((x, y, z)) = mpos {
            ui.label(format!("X:{:.2} Y:{:.2} Z:{:.2}", x, y, z));
        } else {
            ui.label("X:— Y:— Z:—");
        }
    }
}