use egui::{Align, Layout};

use crate::ui::LaserApp;

impl LaserApp {
    /// The bottom strip is now just a status line; the log lives in the console window.
    pub(crate) fn ui_bottom_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("bottom_bar")
            .resizable(false)
            .min_size(26.0)
            .max_size(26.0)
            .show(ui, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(format!("LightLaser v{}", env!("CARGO_PKG_VERSION")));
                });
            });
    }
}
