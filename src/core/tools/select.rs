use crate::core::tools::tool::{
    Tool, ToolContext, ToolEvent, ToolKind, ToolPreview, ToolResponse,
};

#[derive(Default)]
pub struct SelectTool;

impl SelectTool {
    pub fn new() -> Self {
        Self
    }
}

impl Tool for SelectTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Select
    }

    fn reset(&mut self) {}

    fn is_active(&self) -> bool {
        false
    }

    fn hint(&self) -> &'static str {
        "select and move objects"
    }

    fn handle(&mut self, _event: ToolEvent, _ctx: &ToolContext) -> ToolResponse {
        ToolResponse::None
    }

    fn preview(&self, _to_screen: &dyn Fn(crate::core::vector::CurvePoint) -> egui::Pos2) -> ToolPreview {
        ToolPreview::default()
    }
}