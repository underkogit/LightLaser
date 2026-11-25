use std::path::Path;

use rust_i18n::t;

use crate::core::image::{SvgImportProfile, svg_size_mm};
use crate::core::layer::{Layer, LayerSource};
use crate::ui::LaserApp;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp"];
const SVG_EXTENSIONS: &[&str] = &["svg"];
const GCODE_EXTENSIONS: &[&str] = &["nc", "gcode", "txt"];
const GERBER_EXTENSIONS: &[&str] = &[
    "gbr", "ger", "gerber", "gtl", "gbl", "gts", "gbs", "gto", "gbo", "gko", "gm1",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileKind {
    Image,
    Svg,
    Gcode,
    Gerber,
}

impl FileKind {
    pub(crate) fn of(source: &LayerSource) -> Option<Self> {
        match source {
            LayerSource::Image { .. } => Some(Self::Image),
            LayerSource::Svg { .. } => Some(Self::Svg),
            LayerSource::Gcode { .. } => Some(Self::Gcode),
            LayerSource::Gerber { .. } => Some(Self::Gerber),
            LayerSource::Vector => None,
        }
    }

    fn filter_name(self) -> &'static str {
        match self {
            Self::Image => "Images",
            Self::Svg => "SVG",
            Self::Gcode => "G-code",
            Self::Gerber => "Gerber",
        }
    }

    fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Image => IMAGE_EXTENSIONS,
            Self::Svg => SVG_EXTENSIONS,
            Self::Gcode => GCODE_EXTENSIONS,
            Self::Gerber => GERBER_EXTENSIONS,
        }
    }

    fn dialog(self) -> rfd::FileDialog {
        rfd::FileDialog::new().add_filter(self.filter_name(), self.extensions())
    }

    fn default_name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Svg => "svg",
            Self::Gcode => "gcode",
            Self::Gerber => "gerber",
        }
    }

    fn source(self, path: &Path) -> LayerSource {
        let path = path.to_string_lossy().to_string();
        match self {
            Self::Image => LayerSource::Image { path },
            Self::Svg => LayerSource::Svg { path },
            Self::Gcode => LayerSource::Gcode { path },
            Self::Gerber => LayerSource::Gerber { path },
        }
    }

    fn bounds(
        self,
        path: &Path,
        profile: SvgImportProfile,
        previous: Option<[f64; 4]>,
    ) -> Result<[f64; 4], String> {
        let previous = previous.filter(|b| b[2] > b[0] && b[3] > b[1]);
        let left = previous.map(|b| b[0]).unwrap_or(0.0);
        let bottom = previous.map(|b| b[1]).unwrap_or(0.0);

        match self {
            Self::Image => {
                let image = crate::core::image::load_image(path).map_err(|e| e.to_string())?;
                let aspect = image.aspect_ratio().max(1e-6);
                let width = previous
                    .map(|b| b[2] - b[0])
                    .filter(|w| *w > 1e-6)
                    .unwrap_or(100.0);
                Ok([left, bottom, left + width, bottom + width / aspect])
            }
            Self::Svg => {
                let (width, height) = svg_size_mm(path, profile).map_err(|e| e.to_string())?;
                Ok([left, bottom, left + width, bottom + height])
            }
            Self::Gcode => {
                let (min_x, min_y, max_x, max_y) =
                    gcode_bounds(path).unwrap_or((0.0, 0.0, 100.0, 100.0));
                Ok([
                    left,
                    bottom,
                    left + (max_x - min_x),
                    bottom + (max_y - min_y),
                ])
            }
            Self::Gerber => {
                let segments =
                    crate::core::image::parse_gerber_segments(path).map_err(|e| e.to_string())?;
                let (min_x, min_y, max_x, max_y) =
                    gerber_bounds(&segments).unwrap_or((0.0, 0.0, 100.0, 100.0));
                Ok([min_x, min_y, max_x, max_y])
            }
        }
    }
}

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

        project.normalize();

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

    pub(crate) fn add_file_interactive(&mut self, kind: FileKind) {
        let Some(path) = kind.dialog().pick_file() else {
            return;
        };
        self.add_file_from_path(kind, &path);
        self.center_view();
    }

    pub(crate) fn add_file_from_path(&mut self, kind: FileKind, path: &Path) {
        let profile = self.app_config.svg_import_profile;
        self.add_file_with_profile(kind, path, profile);
    }

    pub(crate) fn add_file_with_profile(
        &mut self,
        kind: FileKind,
        path: &Path,
        profile: SvgImportProfile,
    ) {
        let Some(bounds) = self.resolve_bounds(kind, path, profile, None) else {
            return;
        };
        let name = file_stem(path, kind.default_name());
        let mut layer = Layer::new(name.clone(), kind.source(path));
        layer.set_bounds(bounds[0], bounds[1], bounds[2], bounds[3]);
        self.push_history();
        self.project.add_layer(layer);
        self.log_import(kind, &name, bounds);
    }

    pub(crate) fn add_image_interactive(&mut self) {
        self.add_file_interactive(FileKind::Image);
    }

    pub(crate) fn add_image_from_path(&mut self, path: &Path) {
        self.add_file_from_path(FileKind::Image, path);
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
        let Some(path) = FileKind::Svg.dialog().pick_file() else {
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
        self.add_file_with_profile(FileKind::Svg, path, profile);
    }

    pub(crate) fn add_gcode_interactive(&mut self) {
        self.add_file_interactive(FileKind::Gcode);
    }

    pub(crate) fn add_gcode_from_path(&mut self, path: &Path) {
        self.add_file_from_path(FileKind::Gcode, path);
    }

    pub(crate) fn add_gerber_interactive(&mut self) {
        self.add_file_interactive(FileKind::Gerber);
    }

    pub(crate) fn add_gerber_from_path(&mut self, path: &Path) {
        self.add_file_from_path(FileKind::Gerber, path);
    }

    pub(crate) fn replace_layer_interactive(&mut self, id: u64) {
        let Some(index) = self.project.layer_index(id) else {
            return;
        };
        let source = self.project.layers[index].source.clone();
        let Some(kind) = FileKind::of(&source) else {
            log::warn!("{}", t!("ui.warn.layer_not_replaceable"));
            return;
        };
        let Some(path) = kind.dialog().pick_file() else {
            return;
        };
        self.replace_layer_source(index, kind, &path);
    }

    fn replace_layer_source(&mut self, index: usize, kind: FileKind, path: &Path) {
        let previous = self
            .project
            .layers
            .get(index)
            .and_then(|layer| layer.bounds);
        let profile = self.app_config.svg_import_profile;
        let Some(bounds) = self.resolve_bounds(kind, path, profile, previous) else {
            return;
        };

        self.push_history();
        let Some(layer) = self.project.layers.get_mut(index) else {
            return;
        };
        layer.source = kind.source(path);
        layer.name = file_stem(path, kind.default_name());
        layer.set_bounds(bounds[0], bounds[1], bounds[2], bounds[3]);

        self.canvas.texture_cache.clear();
        self.canvas.segment_cache.clear();
        log::info!("{}", t!("ui.info.layer_replaced", name = layer.name));
    }

    fn resolve_bounds(
        &self,
        kind: FileKind,
        path: &Path,
        profile: SvgImportProfile,
        previous: Option<[f64; 4]>,
    ) -> Option<[f64; 4]> {
        match kind.bounds(path, profile, previous) {
            Ok(bounds) => Some(bounds),
            Err(e) => {
                log::error!("{}", t!("ui.error.add_layer", e = e));
                None
            }
        }
    }

    fn log_import(&self, kind: FileKind, name: &str, bounds: [f64; 4]) {
        if kind == FileKind::Svg {
            log::info!(
                "{}",
                t!(
                    "ui.info.svg_added",
                    name = name,
                    width = format!("{:.4}", bounds[2] - bounds[0]),
                    height = format!("{:.4}", bounds[3] - bounds[1])
                )
            );
        } else {
            log::info!("{}", t!("ui.info.layer_added", name = name));
        }
    }
}

fn file_stem(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(fallback)
        .to_string()
}

fn gerber_bounds(segments: &[(f64, f64, f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for (x0, y0, x1, y1) in segments {
        min_x = min_x.min(*x0).min(*x1);
        min_y = min_y.min(*y0).min(*y1);
        max_x = max_x.max(*x0).max(*x1);
        max_y = max_y.max(*y0).max(*y1);
    }

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
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
