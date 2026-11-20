use rust_i18n::t;

use crate::core::image::SvgImportProfile;
use crate::ui::LaserApp;
use crate::ui::app_config::AppTheme;
use crate::ui::apply_fonts;

/// Which page of the settings window is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SettingsTab {
    #[default]
    General,
    Design,
}

/// Locales the UI ships with, as `(code, native name)`.
const LOCALES: &[(&str, &str)] = &[("en", "English"), ("ru", "Русский")];

fn locale_name(code: &str) -> String {
    LOCALES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| code.to_string())
}

impl LaserApp {
    pub(crate) fn open_settings(&mut self) {
        self.settings_open = true;
    }

    pub(crate) fn ui_settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        let mut open = self.settings_open;

        egui::Window::new(t!("ui.settings.title").to_string())
            .id(egui::Id::new("settings_window"))
            .open(&mut open)
            .resizable(true)
            .default_size([560.0, 440.0])
            .min_size([420.0, 300.0])
            .default_pos(egui::pos2(140.0, 80.0))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.settings_tab,
                        SettingsTab::General,
                        t!("ui.settings.tab_general").to_string(),
                    );
                    ui.selectable_value(
                        &mut self.settings_tab,
                        SettingsTab::Design,
                        t!("ui.settings.tab_design").to_string(),
                    );
                });

                ui.separator();

                match self.settings_tab {
                    SettingsTab::General => self.ui_settings_general(ui),
                    SettingsTab::Design => self.ui_settings_design(ui),
                }

                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(t!("ui.settings.save").to_string()).clicked() {
                        self.app_config.save();
                    }
                    ui.label(egui::RichText::new(t!("ui.settings.save_hint").to_string()).weak());
                });
            });

        // Persist as soon as the window is dismissed, so a crash cannot lose the settings.
        if self.settings_open && !open {
            self.app_config.save();
        }
        self.settings_open = open;
    }

    fn ui_settings_general(&mut self, ui: &mut egui::Ui) {
        let locale_before = self.app_config.app_locale.clone();

        ui.add_space(8.0);
        ui.label(egui::RichText::new(t!("ui.settings.general").to_string()).strong());
        ui.add_space(8.0);

        egui::Grid::new("settings_general_grid")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label(t!("ui.settings.language").to_string());
                egui::ComboBox::from_id_salt("settings_language")
                    .selected_text(locale_name(&self.app_config.app_locale))
                    .show_ui(ui, |ui| {
                        for (code, name) in LOCALES {
                            ui.selectable_value(
                                &mut self.app_config.app_locale,
                                (*code).to_string(),
                                *name,
                            );
                        }
                    });
                ui.end_row();

                ui.label(t!("ui.settings.ui_scale").to_string());
                ui.add(
                    egui::Slider::new(&mut self.app_config.ui_scale, 0.5..=2.0)
                        .step_by(0.05)
                        .fixed_decimals(2),
                )
                .on_hover_text(t!("ui.settings.ui_scale_hint").to_string());
                ui.end_row();

                ui.label(t!("ui.settings.console_font_size").to_string());
                ui.add(
                    egui::Slider::new(&mut self.app_config.console_font_size, 8.0..=28.0)
                        .step_by(1.0)
                        .fixed_decimals(0),
                );
                ui.end_row();
            });

        // Switching locale only takes effect from the next frame, so ask for one.
        if self.app_config.app_locale != locale_before {
            rust_i18n::set_locale(&self.app_config.app_locale);
            ui.ctx().request_repaint();
        }

        ui.add_space(12.0);
        if ui.button(t!("ui.settings.reset").to_string()).clicked() {
            self.app_config.ui_scale = 1.0;
            self.app_config.console_font_size = 12.0;
        }
    }

    fn ui_settings_design(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(egui::RichText::new(t!("ui.settings.design").to_string()).strong());
        ui.add_space(8.0);

        egui::Grid::new("settings_design_grid")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label(t!("ui.settings.theme").to_string());
                egui::ComboBox::from_id_salt("settings_theme")
                    .selected_text(format!("{:?}", self.app_config.app_theme))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.app_config.app_theme,
                            AppTheme::Slate,
                            "Slate",
                        );
                        ui.selectable_value(
                            &mut self.app_config.app_theme,
                            AppTheme::Charcoal,
                            "Charcoal",
                        );
                        ui.selectable_value(
                            &mut self.app_config.app_theme,
                            AppTheme::Frost,
                            "Frost",
                        );
                        ui.selectable_value(
                            &mut self.app_config.app_theme,
                            AppTheme::Paper,
                            "Paper",
                        );
                    });
                ui.end_row();

                ui.label(t!("ui.settings.grid_step").to_string());
                ui.add(
                    egui::Slider::new(&mut self.app_config.grid_step_mm, 0.5..=25.0)
                        .step_by(0.5)
                        .suffix(" mm"),
                );
                ui.end_row();

                ui.label(t!("ui.settings.svg_profile").to_string());
                egui::ComboBox::from_id_salt("settings_svg_profile")
                    .selected_text(self.app_config.svg_import_profile.label())
                    .show_ui(ui, |ui| {
                        for profile in SvgImportProfile::ALL {
                            ui.selectable_value(
                                &mut self.app_config.svg_import_profile,
                                profile,
                                profile.label(),
                            );
                        }
                    })
                    .response
                    .on_hover_text(t!("ui.settings.svg_profile_hint").to_string());
                ui.end_row();

                ui.label(t!("ui.settings.font").to_string());
                ui.horizontal(|ui| {
                    let font_name = self
                        .app_config
                        .ui_font
                        .as_deref()
                        .and_then(|path| std::path::Path::new(path).file_name())
                        .and_then(|name| name.to_str())
                        .unwrap_or("Default")
                        .to_string();

                    ui.label(font_name);

                    if ui.button(t!("ui.settings.font_choose").to_string()).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Fonts", &["ttf", "otf", "ttc"])
                            .pick_file()
                        {
                            self.app_config.ui_font = Some(path.to_string_lossy().to_string());
                            apply_fonts(ui.ctx(), self.app_config.ui_font.as_deref());
                        }
                    }

                    if ui.button(t!("ui.settings.font_reset").to_string()).clicked() {
                        self.app_config.ui_font = None;
                        apply_fonts(ui.ctx(), None);
                    }
                });
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.checkbox(
            &mut self.app_config.show_grid,
            t!("ui.menu.show_grid").to_string(),
        );
        ui.checkbox(
            &mut self.app_config.show_axes,
            t!("ui.menu.show_axes").to_string(),
        );
        ui.checkbox(
            &mut self.app_config.show_work_area,
            t!("ui.menu.show_work_area").to_string(),
        );
    }
}
