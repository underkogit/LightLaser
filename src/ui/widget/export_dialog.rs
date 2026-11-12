use egui::{Align, Layout};
use rust_i18n::t;

use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_export_dialog(&mut self, ctx: &egui::Context) {
        if !self.export_dialog_open {
            return;
        }

        let mut open = self.export_dialog_open;

        let modal = egui::Modal::new(egui::Id::new("export_dialog"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .inner_margin(egui::Margin::symmetric(16, 16)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                ui.heading(t!("ui.label.export.title").to_string());
                ui.separator();
                ui.add_space(8.0);

                ui.label(t!("ui.label.export.description").to_string());
                ui.add_space(8.0);

                let enabled_count = self.project.layers.iter().filter(|l| l.is_enabled).count();
                ui.label(format!(
                    "{}: {}",
                    t!("ui.label.export.layers_count"),
                    enabled_count
                ));

                ui.add_space(16.0);
                ui.separator();

                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if ui
                        .add(elegance::Button::new(t!("ui.button.export").to_string()))
                        .clicked()
                    {
                        self.run_export();
                        open = false;
                    }
                    if ui
                        .add(elegance::Button::new(t!("ui.button.cancel").to_string()))
                        .clicked()
                    {
                        open = false;
                    }
                });
            });

        if modal.should_close() {
            open = false;
        }
        self.export_dialog_open = open;
    }

    fn run_export(&mut self) {
        let gcode = match self.export_job_gcode() {
            Ok(g) => g,
            Err(e) => {
                log::error!("{}", t!("ui.error.build_job", e = e));
                return;
            }
        };

        if gcode.trim().is_empty() {
            log::warn!("{}", t!("ui.warn.empty_job"));
            return;
        }

        let default_name = self
            .current_project_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("lightlazer");

        let Some(path) = rfd::FileDialog::new()
            .add_filter("G-code", &["nc", "gcode"])
            .set_file_name(format!("{default_name}.nc"))
            .save_file()
        else {
            return;
        };

        match std::fs::write(&path, gcode) {
            Ok(()) => log::info!("{}", t!("ui.info.exported", path = path.display())),
            Err(e) => log::error!("{}", t!("ui.error.export", e = e.to_string())),
        }
    }
}