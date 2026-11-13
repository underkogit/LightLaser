use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokeStyle {
    #[serde(default = "default_width")]
    pub width_mm: f64,

    #[serde(default)]
    pub line_cap: LineCap,

    #[serde(default)]
    pub line_join: LineJoin,

    #[serde(default = "default_true")]
    pub closed: bool,
}

impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            width_mm: default_width(),
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
            closed: true,
        }
    }
}

fn default_width() -> f64 {
    0.5
}

fn default_true() -> bool {
    true
}
