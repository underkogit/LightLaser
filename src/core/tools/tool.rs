use egui::{Color32, Pos2};

use crate::core::vector::CurvePoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Select,
    Line,
    Curve,
    Rect,
    Circle,
}

impl ToolKind {
    pub fn icon(&self) -> &'static str {
        match self {
            ToolKind::Select => egui_phosphor::regular::CURSOR,
            ToolKind::Line => egui_phosphor::regular::LINE_SEGMENT,
            ToolKind::Curve => egui_phosphor::regular::BEZIER_CURVE,
            ToolKind::Rect => egui_phosphor::regular::RECTANGLE,
            ToolKind::Circle => egui_phosphor::regular::CIRCLE,
        }
    }

    pub fn tooltip(&self) -> &'static str {
        match self {
            ToolKind::Select => "Select / move",
            ToolKind::Line => "Draw line",
            ToolKind::Curve => "Draw curve",
            ToolKind::Rect => "Draw rectangle",
            ToolKind::Circle => "Draw circle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEvent {
    PrimaryDown,
    PrimaryUp,
    SecondaryDown,
    SecondaryUp,
    Move,
    Escape,
    Enter,
    Finish,
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub world: CurvePoint,
    pub snapped: CurvePoint,
    pub modifiers: ToolModifiers,
    pub tolerance_mm: f64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ToolModifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone)]
pub enum ToolResponse {
    None,
    Consumed,
    Commit(Vec<ToolCommit>),
    Cancel,
}

#[derive(Debug, Clone)]
pub enum ToolCommit {
    Path(VectorPath),
}

use crate::core::vector::VectorPath;

#[derive(Debug, Clone, Default)]
pub struct ToolPreview {
    pub polylines: Vec<(Vec<Pos2>, Color32)>,
    pub points: Vec<(Pos2, Color32)>,
    pub circles: Vec<(Pos2, f32, Color32)>,
}

pub trait Tool {
    fn kind(&self) -> ToolKind;
    fn reset(&mut self);
    fn handle(&mut self, event: ToolEvent, ctx: &ToolContext) -> ToolResponse;
    fn preview(&self, to_screen: &dyn Fn(CurvePoint) -> Pos2) -> ToolPreview;
    fn is_active(&self) -> bool;
    fn hint(&self) -> &'static str;
}