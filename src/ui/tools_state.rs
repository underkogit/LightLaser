use crate::core::tools::{
    CircleTool, CurveTool, LineTool, RectTool, SelectTool, Tool, ToolContext, ToolEvent, ToolKind,
    ToolResponse,
};
use crate::core::vector::CurvePoint;

pub struct ToolsState {
    pub current: ToolKind,
    pub select: SelectTool,
    pub line: LineTool,
    pub curve: CurveTool,
    pub rect: RectTool,
    pub circle: CircleTool,
    pub snap_mm: f64,
    pub snap_enabled: bool,
}

impl Default for ToolsState {
    fn default() -> Self {
        Self {
            current: ToolKind::Select,
            select: SelectTool::new(),
            line: LineTool::new(),
            curve: CurveTool::new(),
            rect: RectTool::new(),
            circle: CircleTool::new(),
            snap_mm: 1.0,
            snap_enabled: false,
        }
    }
}

impl ToolsState {
    pub fn set(&mut self, kind: ToolKind) {
        if self.current == kind {
            return;
        }
        self.active_tool_mut().reset();
        self.current = kind;
    }

    pub fn active_tool(&self) -> &dyn Tool {
        match self.current {
            ToolKind::Select => &self.select,
            ToolKind::Line => &self.line,
            ToolKind::Curve => &self.curve,
            ToolKind::Rect => &self.rect,
            ToolKind::Circle => &self.circle,
        }
    }

    pub fn active_tool_mut(&mut self) -> &mut dyn Tool {
        match self.current {
            ToolKind::Select => &mut self.select,
            ToolKind::Line => &mut self.line,
            ToolKind::Curve => &mut self.curve,
            ToolKind::Rect => &mut self.rect,
            ToolKind::Circle => &mut self.circle,
        }
    }

    pub fn snap(&self, p: CurvePoint) -> CurvePoint {
        if !self.snap_enabled || self.snap_mm <= 0.0 {
            return p;
        }
        let step = self.snap_mm;
        CurvePoint::new((p.x / step).round() * step, (p.y / step).round() * step)
    }

    pub fn dispatch(&mut self, event: ToolEvent, ctx: &ToolContext) -> ToolResponse {
        self.active_tool_mut().handle(event, ctx)
    }
}
