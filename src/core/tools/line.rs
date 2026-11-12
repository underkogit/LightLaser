use egui::{Color32, Pos2};

use crate::core::tools::tool::{
    Tool, ToolCommit, ToolContext, ToolEvent, ToolKind, ToolPreview, ToolResponse,
};
use crate::core::vector::{CurvePoint, VectorPath};

pub struct LineTool {
    start: Option<CurvePoint>,
    current: CurvePoint,
}

impl Default for LineTool {
    fn default() -> Self {
        Self {
            start: None,
            current: CurvePoint::default(),
        }
    }
}

impl LineTool {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for LineTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Line
    }

    fn reset(&mut self) {
        self.start = None;
    }

    fn is_active(&self) -> bool {
        self.start.is_some()
    }

    fn hint(&self) -> &'static str {
        "click to place start, click again to finish, Shift = snap axis"
    }

    fn handle(&mut self, event: ToolEvent, ctx: &ToolContext) -> ToolResponse {
        match event {
            ToolEvent::PrimaryDown => {
                let mut p = ctx.snapped;
                if let Some(s) = self.start {
                    if ctx.modifiers.shift {
                        let dx = (p.x - s.x).abs();
                        let dy = (p.y - s.y).abs();
                        if dx > dy {
                            p.y = s.y;
                        } else {
                            p.x = s.x;
                        }
                    }
                    let mut path = VectorPath::new();
                    path.push_corner(s.x, s.y);
                    path.push_corner(p.x, p.y);
                    path.closed = false;
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
                        if dx > dy {
                            p.y = s.y;
                        } else {
                            p.x = s.x;
                        }
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
            let a = to_screen(s);
            let b = to_screen(self.current);
            preview.polylines.push((vec![a, b], color));
            preview.points.push((a, color));
            preview.points.push((b, color));
        }
        preview
    }
}