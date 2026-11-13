use egui::{Color32, Pos2};

use crate::core::tools::tool::{
    Tool, ToolCommit, ToolContext, ToolEvent, ToolKind, ToolPreview, ToolResponse,
};
use crate::core::vector::{CurvePoint, PathNode, VectorPath};

const CIRCLE_SEGMENTS: usize = 64;

pub struct CircleTool {
    center: Option<CurvePoint>,
    current: CurvePoint,
}

impl Default for CircleTool {
    fn default() -> Self {
        Self {
            center: None,
            current: CurvePoint::default(),
        }
    }
}

impl CircleTool {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for CircleTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Circle
    }

    fn reset(&mut self) {
        self.center = None;
    }

    fn is_active(&self) -> bool {
        self.center.is_some()
    }

    fn hint(&self) -> &'static str {
        "click to place center, click again to set radius, Esc to cancel"
    }

    fn handle(&mut self, event: ToolEvent, ctx: &ToolContext) -> ToolResponse {
        match event {
            ToolEvent::PrimaryDown => {
                let p = ctx.snapped;
                if let Some(c) = self.center {
                    let r = ((p.x - c.x).powi(2) + (p.y - c.y).powi(2)).sqrt();
                    if r <= 1e-6 {
                        return ToolResponse::Consumed;
                    }
                    let mut path = VectorPath::new();
                    for i in 0..CIRCLE_SEGMENTS {
                        let a = i as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                        let mut node = PathNode::corner(c.x + a.cos() * r, c.y + a.sin() * r);
                        node.kind = crate::core::vector::NodeKind::Corner;
                        path.nodes.push(node);
                    }
                    path.closed = true;
                    self.center = None;
                    ToolResponse::Commit(vec![ToolCommit::Path(path)])
                } else {
                    self.center = Some(p);
                    self.current = p;
                    ToolResponse::Consumed
                }
            }
            ToolEvent::Move => {
                self.current = ctx.snapped;
                ToolResponse::Consumed
            }
            ToolEvent::Escape => {
                self.center = None;
                ToolResponse::Cancel
            }
            _ => ToolResponse::None,
        }
    }

    fn preview(&self, to_screen: &dyn Fn(CurvePoint) -> Pos2) -> ToolPreview {
        let mut preview = ToolPreview::default();
        let color = Color32::from_rgb(255, 200, 60);
        if let Some(c) = self.center {
            let r = ((self.current.x - c.x).powi(2) + (self.current.y - c.y).powi(2)).sqrt();
            let center_screen = to_screen(c);
            let mut pts: Vec<Pos2> = Vec::with_capacity(CIRCLE_SEGMENTS + 1);
            for i in 0..=CIRCLE_SEGMENTS {
                let a = i as f64 / CIRCLE_SEGMENTS as f64 * std::f64::consts::TAU;
                pts.push(to_screen(CurvePoint::new(
                    c.x + a.cos() * r,
                    c.y + a.sin() * r,
                )));
            }
            preview.polylines.push((pts, color));
            preview.points.push((center_screen, color));
            preview.points.push((to_screen(self.current), color));
        }
        preview
    }
}
