use std::path::Path;

use rust_i18n::t;

use crate::core::{
    image::{extract_palette, load_image},
    layer::{Layer, LayerSource, PROJECT_VERSION, Project, VectorShape},
};
use crate::ui::LaserApp;

const DEFAULT_LAYER_WIDTH_MM: f64 = 100.0;

impl LaserApp {
    pub(crate) fn new_project(&mut self) {
        self.project = Project::new();
        self.current_project_path = None;
        self.ui_state.selection.clear();
        self.canvas = Default::default();
        log::info!("New project created");
    }

    pub(crate) fn open_project_interactive(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("LightLazer Project", &["toml"])
            .pick_file()
        {
            match self.load_project_from(&path) {
                Ok(()) => {
                    self.current_project_path = Some(path.clone());
                    self.center_view();
                    log::info!("{}", t!("ui.info.loaded_project", path = path.display()));
                }
                Err(e) => log::error!("{}", t!("ui.error.load_project", e = e)),
            }
        }
    }

    pub(crate) fn save_project_interactive(&mut self) {
        if let Some(path) = self.current_project_path.clone() {
            if let Err(e) = self.save_project_to(&path) {
                log::error!("{}", t!("ui.error.save_project", e = e));
            } else {
                log::info!("{}", t!("ui.info.project_saved", path = path.display()));
            }
        } else {
            self.save_project_as_interactive();
        }
    }

    pub(crate) fn save_project_as_interactive(&mut self) {
        let default_name = if self.current_project_path.is_none() {
            let date = chrono::Local::now().format("%Y-%m-%d");
            format!("{date}_lightlazer.toml")
        } else {
            "project.toml".to_string()
        };

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("LightLazer Project", &["toml"])
            .set_file_name(&default_name)
            .save_file()
        {
            match self.save_project_to(&path) {
                Ok(()) => {
                    self.current_project_path = Some(path.clone());
                    log::info!("{}", t!("ui.info.project_saved", path = path.display()));
                }
                Err(e) => log::error!("{}", t!("ui.error.save_project", e = e)),
            }
        }
    }

    fn save_project_to(&self, path: &Path) -> Result<(), String> {
        let content =
            toml::to_string_pretty(&self.project).map_err(|e| format!("serialize: {}", e))?;
        std::fs::write(path, content).map_err(|e| format!("write: {}", e))
    }

    pub(crate) fn load_project_from(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path).map_err(|e| format!("read: {}", e))?;
        let project: Project = toml::from_str(&content).map_err(|e| format!("parse: {}", e))?;

        if project.version != PROJECT_VERSION {
            log::warn!(
                "{}",
                t!(
                    "ui.warn.project_version",
                    file = project.version,
                    app = PROJECT_VERSION
                )
            );
        }

        self.project = project;
        self.ui_state.selection.clear();
        Ok(())
    }

    pub(crate) fn add_image_interactive(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
            .pick_files()
        else {
            return;
        };

        for path in paths {
            self.add_image_from_path(&path);
        }

        self.center_view();
    }

    pub(crate) fn add_image_from_path(&mut self, path: &Path) {
        match self.create_image_layers(path) {
            Ok(layers) => {
                for layer in layers {
                    let name = layer.name.clone();
                    self.project.add_layer(layer);
                    log::info!("{}", t!("ui.info.layer_added", name = name));
                }
            }
            Err(e) => log::error!("{}", t!("ui.error.add_layer", e = e)),
        }
    }

    fn create_image_layers(&self, path: &Path) -> Result<Vec<Layer>, String> {
        let loaded = load_image(path).map_err(|e| e.to_string())?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Image")
            .to_string();

        let palette = extract_palette(&loaded.dynamic);
        let aspect = loaded.aspect_ratio();
        let width_mm = DEFAULT_LAYER_WIDTH_MM;
        let height_mm = width_mm / aspect.max(1e-9);
        let ppm = loaded.width as f64 / width_mm;

        let mut layers: Vec<Layer> = Vec::new();

        if palette.is_empty() {
            let mut layer = Layer::new(
                stem.clone(),
                [220, 220, 220],
                LayerSource::Image {
                    path: path.to_string_lossy().to_string(),
                },
            );
            layer.set_bounds(0.0, 0.0, width_mm, height_mm);
            layer.params.pixels_per_mm = ppm.max(1.0);
            layers.push(layer);
            return Ok(layers);
        }

        let mut layer = Layer::new(
            stem,
            palette[0].rgb,
            LayerSource::Image {
                path: path.to_string_lossy().to_string(),
            },
        );
        layer.set_bounds(0.0, 0.0, width_mm, height_mm);
        layer.params.pixels_per_mm = ppm.max(1.0);
        layers.push(layer);

        Ok(layers)
    }

    pub(crate) fn add_svg_interactive(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .pick_files()
        else {
            return;
        };

        for path in paths {
            self.add_svg_from_path(&path);
        }

        self.center_view();
    }

    pub(crate) fn add_svg_from_path(&mut self, path: &Path) {
        match self.create_svg_layer(path) {
            Ok(layer) => {
                let name = layer.name.clone();
                self.project.add_layer(layer);
                log::info!("{}", t!("ui.info.layer_added", name = name));
            }
            Err(e) => log::error!("{}", t!("ui.error.add_layer", e = e)),
        }
    }

    fn create_svg_layer(&self, path: &Path) -> Result<Layer, String> {
        use resvg::usvg;
        use tiny_skia::Pixmap;

        let svg_data = std::fs::read(path).map_err(|e| format!("svg read: {}", e))?;
        let opt = usvg::Options::default();
        let tree =
            usvg::Tree::from_data(&svg_data, &opt).map_err(|e| format!("svg parse: {}", e))?;

        let size = tree.size();
        let width_mm = size.width() as f64 * 25.4 / 96.0;
        let height_mm = size.height() as f64 * 25.4 / 96.0;

        let mut pixmap = Pixmap::new(size.width() as u32, size.height() as u32)
            .ok_or("failed to create pixmap")?;
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        let image =
            image::RgbaImage::from_raw(pixmap.width(), pixmap.height(), pixmap.data().to_vec())
                .ok_or("failed to convert pixmap to image")?;

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Svg")
            .to_string();

        let mut layer = Layer::new(
            stem,
            [200, 200, 220],
            LayerSource::Svg {
                path: path.to_string_lossy().to_string(),
            },
        );

        layer.set_bounds(0.0, 0.0, width_mm, height_mm);
        layer.params.pixels_per_mm = (pixmap.width() as f64 / width_mm).max(1.0);

        let _ = image;

        Ok(layer)
    }

    pub(crate) fn add_gcode_interactive(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter("G-code", &["nc", "gcode", "gbr", "txt"])
            .pick_files()
        else {
            return;
        };

        for path in paths {
            self.add_gcode_from_path(&path);
        }

        self.center_view();
    }

    pub(crate) fn add_gcode_from_path(&mut self, path: &Path) {
        match self.create_gcode_layer(path) {
            Ok(layer) => {
                let name = layer.name.clone();
                self.project.add_layer(layer);
                log::info!("{}", t!("ui.info.layer_added", name = name));
            }
            Err(e) => log::error!("{}", t!("ui.error.add_layer", e = e)),
        }
    }

    fn create_gcode_layer(&self, path: &Path) -> Result<Layer, String> {
        let cmds =
            crate::core::gcode::parse_file(path).map_err(|e| format!("gcode parse: {}", e))?;

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut cur_x = 0.0;
        let mut cur_y = 0.0;

        for c in &cmds {
            if let Some(x) = c.x {
                cur_x = x;
            }
            if let Some(y) = c.y {
                cur_y = y;
            }
            if c.x.is_some() || c.y.is_some() {
                min_x = min_x.min(cur_x);
                min_y = min_y.min(cur_y);
                max_x = max_x.max(cur_x);
                max_y = max_y.max(cur_y);
            }
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Gcode")
            .to_string();

        let mut layer = Layer::new(
            stem,
            [180, 180, 200],
            LayerSource::Gcode {
                path: path.to_string_lossy().to_string(),
            },
        );

        if min_x.is_finite() && min_y.is_finite() {
            layer.set_bounds(min_x, min_y, max_x, max_y);
        } else {
            layer.set_bounds(0.0, 0.0, DEFAULT_LAYER_WIDTH_MM, DEFAULT_LAYER_WIDTH_MM);
        }

        Ok(layer)
    }

    #[allow(dead_code)]
    fn _unused(_s: VectorShape) {}
}