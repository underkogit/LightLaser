use std::path::Path;

use rust_i18n::t;

use crate::core::layer::{Layer, LayerSource};
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn new_project(&mut self) {
        self.push_history();
        self.project = crate::core::layer::Project::new();
        self.current_project_path = None;
        self.ui_state.selection.clear();
        self.canvas.texture_cache.clear();
        log::info!("New project");
    }

    pub(crate) fn open_project_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LightLazer project", &["toml"])
            .pick_file()
        else {
            return;
        };
        if let Err(e) = self.load_project_from(&path) {
            log::error!("{}", t!("ui.error.load_project", e = e));
            return;
        }
        self.current_project_path = Some(path.clone());
        self.canvas.texture_cache.clear();
        self.center_view();
        log::info!("{}", t!("ui.info.loaded_project", path = path.display()));
    }

    pub(crate) fn save_project_interactive(&mut self) {
        if let Some(path) = self.current_project_path.clone() {
            if let Err(e) = self.save_project_to(&path) {
                log::error!("{}", t!("ui.error.save_project", e = e));
            } else {
                log::info!("{}", t!("ui.info.project_saved", path = path.display()));
            }
            return;
        }
        self.save_project_as_interactive();
    }

    pub(crate) fn save_project_as_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LightLazer project", &["toml"])
            .set_file_name("project.toml")
            .save_file()
        else {
            return;
        };
        if let Err(e) = self.save_project_to(&path) {
            log::error!("{}", t!("ui.error.save_project", e = e));
            return;
        }
        self.current_project_path = Some(path.clone());
        log::info!("{}", t!("ui.info.project_saved", path = path.display()));
    }

    pub(crate) fn load_project_from(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut project: crate::core::layer::Project =
            toml::from_str(&content).map_err(|e| e.to_string())?;

        if project.version != crate::core::layer::PROJECT_VERSION {
            log::warn!(
                "{}",
                t!(
                    "ui.warn.project_version",
                    file = project.version,
                    app = crate::core::layer::PROJECT_VERSION
                )
            );
        }

        for layer in &mut project.layers {
            layer.recompute_bounds();
        }

        self.project = project;
        self.ui_state.selection.clear();
        Ok(())
    }

    pub(crate) fn save_project_to(&self, path: &Path) -> Result<(), String> {
        let content = toml::to_string_pretty(&self.project).map_err(|e| e.to_string())?;
        std::fs::write(path, content).map_err(|e| e.to_string())
    }

    pub(crate) fn add_image_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
            .pick_file()
        else {
            return;
        };
        self.add_image_from_path(&path);
        self.center_view();
    }

    pub(crate) fn add_svg_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .pick_file()
        else {
            return;
        };
        self.add_svg_from_path(&path);
        self.center_view();
    }

    pub(crate) fn add_svg_pcb_interactive(&mut self) {
        self.add_svg_interactive();
    }

    pub(crate) fn add_gcode_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("G-code", &["nc", "gcode", "txt"])
            .pick_file()
        else {
            return;
        };
        self.add_gcode_from_path(&path);
        self.center_view();
    }

    pub(crate) fn add_image_from_path(&mut self, path: &Path) {
        match crate::core::image::load_image(path) {
            Ok(img) => {
                let aspect = img.aspect_ratio().max(1e-6);
                let width_mm = 100.0;
                let height_mm = width_mm / aspect;
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image")
                    .to_string();
                let mut layer = Layer::new(
                    name.clone(),
                    crate::ui::colors::LAYER_DEFAULT,
                    LayerSource::Image {
                        path: path.to_string_lossy().to_string(),
                    },
                );
                layer.set_bounds(0.0, 0.0, width_mm, height_mm);
                self.push_history();
                self.project.add_layer(layer);
                log::info!("{}", t!("ui.info.layer_added", name = name));
            }
            Err(e) => log::error!("{}", t!("ui.error.add_layer", e = e.to_string())),
        }
    }

    pub(crate) fn add_svg_from_path(&mut self, path: &Path) {
        match crate::core::image::rasterize_svg(path, 1024) {
            Ok(svg) => {
                let width_mm = svg.width_mm.max(1.0);
                let height_mm = svg.height_mm.max(1.0);
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("svg")
                    .to_string();
                let mut layer = Layer::new(
                    name.clone(),
                    crate::ui::colors::LAYER_DEFAULT,
                    LayerSource::Svg {
                        path: path.to_string_lossy().to_string(),
                    },
                );
                layer.set_bounds(0.0, 0.0, width_mm, height_mm);
                self.push_history();
                self.project.add_layer(layer);
                log::info!("{}", t!("ui.info.layer_added", name = name));
            }
            Err(e) => log::error!("{}", t!("ui.error.add_layer", e = e)),
        }
    }

    pub(crate) fn add_gcode_from_path(&mut self, path: &Path) {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("gcode")
            .to_string();
        let mut layer = Layer::new(
            name.clone(),
            crate::ui::colors::LAYER_DEFAULT,
            LayerSource::Gcode {
                path: path.to_string_lossy().to_string(),
            },
        );
        layer.set_bounds(0.0, 0.0, 100.0, 100.0);
        self.push_history();
        self.project.add_layer(layer);
        log::info!("{}", t!("ui.info.layer_added", name = name));
    }
}