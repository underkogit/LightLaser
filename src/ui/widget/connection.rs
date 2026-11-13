use egui::{Align, Layout};
use rust_i18n::t;

use crate::comm::StreamerCommand;
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_connection_dialog(&mut self, ctx: &egui::Context) {
        if !self.connection_dialog_open {
            return;
        }

        let mut open = self.connection_dialog_open;

        let modal = egui::Modal::new(egui::Id::new("connection_dialog"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .inner_margin(egui::Margin::symmetric(16, 16)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                ui.heading(t!("ui.label.connection.title").to_string());
                ui.separator();
                ui.add_space(8.0);

                let ports: Vec<String> = serialport::available_ports()
                    .map(|list| list.into_iter().map(|p| p.port_name).collect())
                    .unwrap_or_default();

                ui.horizontal(|ui| {
                    ui.label(t!("ui.label.connection.port").to_string());
                    egui::ComboBox::from_id_salt("port_combo")
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
                    egui::ComboBox::from_id_salt("baudrate_combo")
                        .selected_text(self.project.machine.baudrate.to_string())
                        .width(180.0)
                        .show_ui(ui, |ui| {
                            for b in [9600u32, 19200, 38400, 57600, 115200] {
                                ui.selectable_value(
                                    &mut self.project.machine.baudrate,
                                    b,
                                    b.to_string(),
                                );
                            }
                        });
                });

                ui.add_space(16.0);
                ui.separator();

                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    let can_connect = !self.project.machine.port.is_empty();
                    if ui
                        .add_enabled(
                            can_connect,
                            egui::Button::new(t!("ui.button.connect").to_string()),
                        )
                        .clicked()
                    {
                        let port = self.project.machine.port.clone();
                        let baudrate = self.project.machine.baudrate;

                        self.streamer.send(StreamerCommand::Connect {
                            port: port.clone(),
                            baudrate,
                        });
                        log::info!("{}", t!("comm.info.connecting", port = port));
                        open = false;
                    }
                    if ui
                        .add(egui::Button::new(t!("ui.button.cancel").to_string()))
                        .clicked()
                    {
                        open = false;
                    }
                });
            });

        if modal.should_close() {
            open = false;
        }
        self.connection_dialog_open = open;
    }
}
