use std::path::Path;

use rust_i18n::t;

use crate::core::image::{SvgImportProfile, svg_size_mm};
use crate::core::layer::{Layer, LayerSource};
use crate::ui::LaserApp;

impl LaserApp {
    pub(crate) fn new_project(&mut self) {
        self.push_history();
        self.project = crate::core::layer::Project::new();
        self.current_project_path = None;
        self.ui_state.selection.clear();
        self.canvas.texture_cache.clear();
        self.canvas.segment_cache.clear();
        log::info!("New project");
    }

    pub(crate) fn open_project_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("LightLaser project", &["toml"])
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
        self.canvas.segment_cache.clear();
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
            .add_filter("LightLaser project", &["toml"])
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
        let profile = self.app_config.svg_import_profile;
        self.add_svg_interactive_with_profile(profile);
    }

    /// Asks for an SVG file and imports it with the given profile.
    ///
    /// The import menu uses this for explicit profiles such as the PCB one; drag and drop uses
    /// the profile configured in the settings window.
    pub(crate) fn add_svg_interactive_with_profile(&mut self, profile: SvgImportProfile) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .pick_file()
        else {
            return;
        };
        self.add_svg_with_profile(&path, profile);
        self.center_view();
    }

    pub(crate) fn add_svg_from_path(&mut self, path: &Path) {
        let profile = self.app_config.svg_import_profile;
        self.add_svg_with_profile(path, profile);
    }

    /// Adds an SVG layer whose bounds are the size the file itself declares, in millimetres.
    pub(crate) fn add_svg_with_profile(&mut self, path: &Path, profile: SvgImportProfile) {
        // The declared width and height are what the drawing is meant to measure on the
        // workbed; the renderer's own size is only a fallback.
        let (width_mm, height_mm) = match svg_size_mm(path, profile) {
            Ok(size) => size,
            Err(e) => {
                log::error!("{}", t!("ui.error.add_layer", e = e));
                return;
            }
        };

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
        log::info!(
            "{}",
            t!(
                "ui.info.svg_added",
                name = name,
                width = format!("{:.4}", width_mm),
                height = format!("{:.4}", height_mm)
            )
        );
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

    pub(crate) fn add_gerber_interactive(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Gerber",
                &[
                    "gbr", "ger", "gerber", "gtl", "gbl", "gts", "gbs", "gto", "gbo", "gko", "gm1",
                ],
            )
            .pick_file()
        else {
            return;
        };
        self.add_gerber_from_path(&path);
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

    pub(crate) fn add_gcode_from_path(&mut self, path: &Path) {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("gcode")
            .to_string();

        let (min_x, min_y, max_x, max_y) = gcode_bounds(path).unwrap_or((0.0, 0.0, 100.0, 100.0));

        let mut layer = Layer::new(
            name.clone(),
            crate::ui::colors::LAYER_DEFAULT,
            LayerSource::Gcode {
                path: path.to_string_lossy().to_string(),
            },
        );
        layer.set_bounds(min_x, min_y, max_x, max_y);
        self.push_history();
        self.project.add_layer(layer);
        log::info!("{}", t!("ui.info.layer_added", name = name));
    }

    pub(crate) fn add_gerber_from_path(&mut self, path: &Path) {
        let segments = match crate::core::image::parse_gerber_segments(path) {
            Ok(segments) => segments,
            Err(e) => {
                log::error!("{}", t!("ui.error.add_layer", e = e));
                return;
            }
        };

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("gerber")
            .to_string();

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for (x0, y0, x1, y1) in &segments {
            min_x = min_x.min(*x0).min(*x1);
            min_y = min_y.min(*y0).min(*y1);
            max_x = max_x.max(*x0).max(*x1);
            max_y = max_y.max(*y0).max(*y1);
        }

        let mut layer = Layer::new(
            name.clone(),
            crate::ui::colors::LAYER_DEFAULT,
            LayerSource::Gerber {
                path: path.to_string_lossy().to_string(),
            },
        );

        if min_x.is_finite() && min_y.is_finite() {
            layer.set_bounds(min_x, min_y, max_x, max_y);
        }

        self.push_history();
        self.project.add_layer(layer);
        log::info!("{}", t!("ui.info.layer_added", name = name));
    }
}

fn gcode_bounds(path: &Path) -> Option<(f64, f64, f64, f64)> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut cur = (0.0f64, 0.0f64);
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut any = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        let mut nx = None;
        let mut ny = None;

        for tok in line.split_whitespace() {
            if let Some(rest) = tok.strip_prefix('X') {
                nx = rest.parse::<f64>().ok();
            } else if let Some(rest) = tok.strip_prefix('Y') {
                ny = rest.parse::<f64>().ok();
            }
        }

        let px = nx.unwrap_or(cur.0);
        let py = ny.unwrap_or(cur.1);

        min_x = min_x.min(px);
        min_y = min_y.min(py);
        max_x = max_x.max(px);
        max_y = max_y.max(py);
        any = true;

        cur = (px, py);
    }

    if !any || !min_x.is_finite() || !min_y.is_finite() {
        return None;
    }

    Some((0.0, 0.0, max_x - min_x, max_y - min_y))
}
