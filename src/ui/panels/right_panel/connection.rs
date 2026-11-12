use rust_i18n::t;

use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_connection_params(&mut self, ui: &mut egui::Ui) {
        let ports: Vec<String> = serialport::available_ports()
            .map(|list| list.into_iter().map(|p| p.port_name).collect())
            .unwrap_or_default();

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.connection.port").to_string());
            egui::ComboBox::from_id_salt("port_combo_panel")
                .selected_text(if self.project.machine.port.is_empty() {
                    "—".to_string()
                } else {
                    self.project.machine.port.clone()
                })
                .width(180.0)
                .show_ui(ui, |ui| {
                    for p in &ports {
                        ui.selectable_value(&mut self.project.machine.port, p.clone(), p);
                    }
                });
            if ui.button("⟳").clicked() {
                ui.ctx().request_repaint();
            }
        });

        ui.horizontal(|ui| {
            ui.label(t!("ui.label.connection.baudrate").to_string());
            egui::ComboBox::from_id_salt("baudrate_combo_panel")
                .selected_text(self.project.machine.baudrate.to_string())
                .width(180.0)
                .show_ui(ui, |ui| {
                    for b in [9600u32, 19200, 38400, 57600, 115200] {
                        ui.selectable_value(&mut self.project.machine.baudrate, b, b.to_string());
                    }
                });
        });
    }
}
