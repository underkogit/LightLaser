use egui::{Align, Color32, Layout, RichText};

use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn open_gcode_viewer(&mut self) {
        self.canvas.gcode_window.open = true;
        self.canvas.gcode_window.pending = true;
    }

    pub(crate) fn ui_gcode_window(&mut self, ctx: &egui::Context) {
        if !self.canvas.gcode_window.open {
            return;
        }

        let mut open = self.canvas.gcode_window.open;

        egui::Window::new("G-code")
            .id(egui::Id::new("gcode_window"))
            .open(&mut open)
            .resizable(true)
            .default_size([720.0, 600.0])
            .min_size([320.0, 200.0])
            .default_pos(egui::pos2(120.0, 80.0))
            .collapsible(true)
            .show(ctx, |ui| {
                if self.canvas.gcode_window.pending {
                    rebuild_gcode(self);
                    self.canvas.gcode_window.pending = false;
                }

                let line_count = self.canvas.gcode_window.text.lines().count();

                ui.horizontal(|ui| {
                    if ui.button("Refresh").clicked() {
                        self.canvas.gcode_window.pending = true;
                    }
                    ui.checkbox(&mut self.canvas.gcode_window.wrap, "Wrap");
                    ui.label(format!("Lines: {}", line_count));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(line_count > 0, egui::Button::new("Copy all"))
                            .clicked()
                        {
                            ui.ctx().copy_text(self.canvas.gcode_window.text.clone());
                        }
                    });
                });

                ui.separator();

                if let Some(err) = self.canvas.gcode_window.error.clone() {
                    ui.colored_label(Color32::LIGHT_RED, err);
                    ui.add_space(4.0);
                } else if line_count == 0 {
                    ui.label(RichText::new("No G-code to display").weak());
                    ui.add_space(4.0);
                }

                let wrap = self.canvas.gcode_window.wrap;
                let text = std::mem::take(&mut self.canvas.gcode_window.text);

                {
                    let mut readonly: &str = text.as_str();

                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let width = if wrap {
                                ui.available_width()
                            } else {
                                f32::INFINITY
                            };

                            ui.add(
                                egui::TextEdit::multiline(&mut readonly)
                                    .code_editor()
                                    .desired_width(width)
                                    .desired_rows(20),
                            );
                        });
                }

                self.canvas.gcode_window.text = text;
            });

        self.canvas.gcode_window.open = open;
    }
}

fn rebuild_gcode(app: &mut LaserApp) {
    match app.export_job_gcode() {
        Ok(text) => {
            app.canvas.gcode_window.error = None;
            app.canvas.gcode_window.text = text;
        }
        Err(e) => {
            log::error!("{}", e);
            app.canvas.gcode_window.error = Some(e);
            app.canvas.gcode_window.text.clear();
        }
    }
}
