use serde::{Deserialize, Serialize};

use crate::core::vector::{PathNode, StrokeStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LayerUnit {
    #[default]
    Millimeter,
    Inch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EngraveMode {
    #[default]
    Raster,
    Lines,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayerSource {
    Image { path: String },
    Svg { path: String },
    Gcode { path: String },
    Gerber { path: String },
    Vector,
}

impl LayerSource {
    pub fn kind_label(&self) -> &'static str {
        match self {
            LayerSource::Image { .. } => "image",
            LayerSource::Svg { .. } => "svg",
            LayerSource::Gcode { .. } => "gcode",
            LayerSource::Gerber { .. } => "gerber",
            LayerSource::Vector => "vector",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerParams {
    #[serde(default = "default_power")]
    pub power_percent: f64,

    #[serde(default = "default_feed")]
    pub feed_rate: f64,

    #[serde(default = "default_passes")]
    pub passes: u32,

    #[serde(default = "default_true")]
    pub dynamic_power: bool,

    #[serde(default = "default_px_mm")]
    pub pixels_per_mm: f64,

    #[serde(default = "default_threshold")]
    pub threshold: u8,

    #[serde(default)]
    pub dither: bool,

    #[serde(default)]
    pub invert: bool,

    #[serde(default)]
    pub engrave_mode: EngraveMode,
}

impl Default for LayerParams {
    fn default() -> Self {
        Self {
            power_percent: default_power(),
            feed_rate: default_feed(),
            passes: default_passes(),
            dynamic_power: true,
            pixels_per_mm: default_px_mm(),
            threshold: default_threshold(),
            dither: false,
            invert: false,
            engrave_mode: EngraveMode::Raster,
        }
    }
}

fn default_power() -> f64 {
    50.0
}

fn default_feed() -> f64 {
    1000.0
}

fn default_passes() -> u32 {
    1
}

fn default_px_mm() -> f64 {
    10.0
}

fn default_threshold() -> u8 {
    128
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorShape {
    #[serde(default)]
    pub nodes: Vec<PathNode>,

    #[serde(default)]
    pub stroke: StrokeStyle,

    #[serde(default)]
    pub closed: bool,
}

impl Default for VectorShape {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            stroke: StrokeStyle::default(),
            closed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub color: [u8; 3],
    pub source: LayerSource,

    #[serde(default = "default_true")]
    pub is_visible: bool,

    #[serde(default = "default_true")]
    pub is_enabled: bool,

    #[serde(default)]
    pub params: LayerParams,

    #[serde(default)]
    pub bounds: Option<[f64; 4]>,

    #[serde(default)]
    pub shapes: Vec<VectorShape>,

    #[serde(default)]
    pub offset: [f64; 2],
}

impl Layer {
    pub fn new(name: String, color: [u8; 3], source: LayerSource) -> Self {
        Self {
            name,
            color,
            source,
            is_visible: true,
            is_enabled: true,
            params: LayerParams::default(),
            bounds: None,
            shapes: Vec::new(),
            offset: [0.0, 0.0],
        }
    }

    pub fn set_bounds(&mut self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) {
        self.bounds = Some([min_x, min_y, max_x, max_y]);
    }

    pub fn bounds(&self) -> Option<(f64, f64, f64, f64)> {
        self.bounds.map(|b| (b[0], b[1], b[2], b[3]))
    }

    pub fn is_vector(&self) -> bool {
        matches!(self.source, LayerSource::Vector)
    }

    pub fn is_raster(&self) -> bool {
        matches!(
            self.source,
            LayerSource::Image { .. } | LayerSource::Svg { .. }
        )
    }

    pub fn recompute_bounds(&mut self) {
        if self.shapes.is_empty() {
            if self.is_vector() {
                self.bounds = None;
            }
            return;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut any = false;

        for shape in &self.shapes {
            let mut path = crate::core::vector::VectorPath::new();
            path.nodes = shape.nodes.clone();
            path.stroke = shape.stroke.clone();
            path.closed = shape.closed;

            if let Some((a, b, c, d)) = path.bounds(0.1) {
                min_x = min_x.min(a);
                min_y = min_y.min(b);
                max_x = max_x.max(c);
                max_y = max_y.max(d);
                any = true;
            }
        }

        if any {
            self.set_bounds(min_x, min_y, max_x, max_y);
        }
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.offset[0] += dx;
        self.offset[1] += dy;

        for shape in &mut self.shapes {
            for node in &mut shape.nodes {
                node.point.x += dx;
                node.point.y += dy;
                node.handle_in.x += dx;
                node.handle_in.y += dy;
                node.handle_out.x += dx;
                node.handle_out.y += dy;
            }
        }

        if let Some(b) = self.bounds.as_mut() {
            b[0] += dx;
            b[1] += dy;
            b[2] += dx;
            b[3] += dy;
        }
    }
}
