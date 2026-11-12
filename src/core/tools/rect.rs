use egui::{Color32, Pos2};

use crate::core::tools::tool::{
    Tool, ToolCommit, ToolContext, ToolEvent, ToolKind, ToolPreview, ToolResponse,
};
use crate::core::vector::{CurvePoint, VectorPath};

pub struct RectTool {
    start: Option<CurvePoint>,
    current: CurvePoint,
}

impl Default for RectTool {
    fn default() -> Self {
        Self {
            start: None,
            current: CurvePoint::default(),
        }
    }
}

impl RectTool {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for RectTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Rect
    }

    fn reset(&mut self) {
        self.start = None;
    }

    fn is_active(&self) -> bool {
        self.start.is_some()
    }

    fn hint(&self) -> &'static str {
        "click to place first corner, click again to finish, Shift = square, Esc to cancel"
    }

    fn handle(&mut self, event: ToolEvent, ctx: &ToolContext) -> ToolResponse {
        match event {
            ToolEvent::PrimaryDown => {
                let mut p = ctx.snapped;
                if let Some(s) = self.start {
                    if ctx.modifiers.shift {
                        let dx = (p.x - s.x).abs();
                        let dy = (p.y - s.y).abs();
                        let side = dx.max(dy);
                        p.x = s.x + side.copysign(p.x - s.x);
                        p.y = s.y + side.copysign(p.y - s.y);
                    }
                    let mut path = VectorPath::new();
                    path.push_corner(s.x, s.y);
                    path.push_corner(p.x, s.y);
                    path.push_corner(p.x, p.y);
                    path.push_corner(s.x, p.y);
                    path.closed = true;
                    self.start = None;
                    ToolResponse::Commit(vec![ToolCommit::Path(path)])
                } else {
                    self.start = Some(p);
                    self.current = p;
                    ToolResponse::Consumed
                }
            }
            ToolEvent::Move => {
                let mut p = ctx.snapped;
                if let Some(s) = self.start {
                    if ctx.modifiers.shift {
                        let dx = (p.x - s.x).abs();
                        let dy = (p.y - s.y).abs();
                        let side = dx.max(dy);
                        p.x = s.x + side.copysign(p.x - s.x);
                        p.y = s.y + side.copysign(p.y - s.y);
                    }
                }
                self.current = p;
                ToolResponse::Consumed
            }
            ToolEvent::Escape => {
                self.start = None;
                ToolResponse::Cancel
            }
            _ => ToolResponse::None,
        }
    }

    fn preview(&self, to_screen: &dyn Fn(CurvePoint) -> Pos2) -> ToolPreview {
        let mut preview = ToolPreview::default();
        let color = Color32::from_rgb(255, 200, 60);
        if let Some(s) = self.start {
            let p = self.current;
            let a = to_screen(CurvePoint::new(s.x, s.y));
            let b = to_screen(CurvePoint::new(p.x, s.y));
            let c = to_screen(CurvePoint::new(p.x, p.y));
            let d = to_screen(CurvePoint::new(s.x, p.y));
            preview.polylines.push((vec![a, b, c, d, a], color));
            preview.points.push((a, color));
            preview.points.push((c, color));
        }
        preview
    }
}