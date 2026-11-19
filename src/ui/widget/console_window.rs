use rust_i18n::t;

use crate::logger::LogEntry;
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn ui_console_window(&mut self, ctx: &egui::Context) {
        if !self.console_open {
            return;
        }

        let mut open = self.console_open;

        egui::Window::new(t!("ui.console.title").to_string())
            .id(egui::Id::new("console_window"))
            .open(&mut open)
            .resizable(true)
            .default_size([720.0, 340.0])
            .min_size([320.0, 160.0])
            .default_pos(egui::pos2(80.0, 120.0))
            .collapsible(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t!("ui.console.title").to_string()).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t!("ui.button.clear_logs").to_string()).clicked() {
                            if let Ok(mut logs) = self.log_buffer.lock() {
                                logs.clear();
                            }
                        }
                    });
                });

                ui.separator();

                let font_size = self.app_config.console_font_size.clamp(6.0, 32.0);

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
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
                                                .size(font_size),
                                        )
                                        .wrap_mode(egui::TextWrapMode::Wrap),
                                    );
                                });
                            }
                        }
                    });
            });

        self.console_open = open;

        // Keep the log fresh while the window is up.
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}
