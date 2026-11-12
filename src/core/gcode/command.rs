#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind {
    Rapid,
    Linear,
    ArcCw,
    ArcCcw,
    Home,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GcodeCommand {
    pub raw: String,

    pub line_number: Option<u32>,

    pub motion: Option<MotionKind>,

    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub i: Option<f64>,
    pub j: Option<f64>,

    pub feed: Option<f64>,

    pub spindle: Option<f64>,

    pub comment: Option<String>,
}

impl GcodeCommand {
    pub fn is_motion(&self) -> bool {
        self.motion.is_some()
    }

    pub fn is_comment_only(&self) -> bool {
        self.raw.trim().is_empty() || self.raw.trim_start().starts_with(';')
    }
}