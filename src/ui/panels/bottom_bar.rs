use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use egui::{Align, Layout};
use rust_i18n::t;

use crate::logger::LogEntry;
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_bottom_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("bottom_bar")
            .resizable(true)
            .default_size(180.0)
            .min_size(100.0)
            .show(ui, |ui| {
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.set_min_height(ui.available_height());

                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(format!("LightLazer v{}", env!("CARGO_PKG_VERSION")));
                        });
                    });
                    ui.separator();

                    self.ui_log(ui);
                });
            });
    }

    fn ui_log(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t!("ui.label.logs").to_string()).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(t!("ui.button.clear_logs")).clicked() {
                    if let Ok(mut logs) = self.log_buffer.lock() {
                        logs.clear();
                    }
                }
            });
        });
        ui.add_space(4.0);

        let available = ui.available_height().max(40.0);

        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_height(available - 12.0)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());

                        let snapshot: Vec<LogEntry> = match self.log_buffer.lock() {
                            Ok(logs) => logs.iter().cloned().collect(),
                            Err(_) => Vec::new(),
                        };

                        if snapshot.is_empty() {
                            ui.weak("(empty)");
                        } else {
                            for (i, entry) in snapshot.iter().enumerate() {
                                let color = match entry.level {
                                    log::Level::Error => ui.visuals().error_fg_color,
                                    log::Level::Warn => ui.visuals().warn_fg_color,
                                    log::Level::Info => ui.visuals().text_color(),
                                    _ => ui.visuals().weak_text_color(),
                                };
                                ui.push_id(i, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&entry.message)
                                                .color(color)
                                                .monospace()
                                                .size(12.0),
                                        )
                                        .wrap_mode(egui::TextWrapMode::Wrap),
                                    );
                                });
                            }
                        }
                    });
            });

        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));
    }
}
