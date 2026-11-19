use egui::{Align, Layout};

use crate::ui::LaserApp;

impl LaserApp {
    /// The bottom strip carries the version on the left and the machine position on the
    /// right.
    pub(crate) fn ui_bottom_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("bottom_bar")
            .resizable(false)
            .min_size(32.0)
            .max_size(32.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("LightLaser v{}", env!("CARGO_PKG_VERSION")));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        match self.grbl_status.as_ref().and_then(|status| status.mpos) {
                            Some((x, y, z)) => {
                                ui.label(format!("X:{:.2} Y:{:.2} Z:{:.2}", x, y, z));
                            }
                            None => {
                                ui.label("X:— Y:— Z:—");
                            }
                        }
                    });
                });
            });
    }
}
