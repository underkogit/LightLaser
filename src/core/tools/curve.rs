use egui::{Color32, Pos2};

use crate::core::tools::tool::{
    Tool, ToolCommit, ToolContext, ToolEvent, ToolKind, ToolPreview, ToolResponse,
};
use crate::core::vector::{CurvePoint, PathNode, VectorPath};

pub struct CurveTool {
    nodes: Vec<PathNode>,
    current: CurvePoint,
    dragging_handle: bool,
}

impl Default for CurveTool {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            current: CurvePoint::default(),
            dragging_handle: false,
        }
    }
}

impl CurveTool {
    pub fn new() -> Self {
        Self::default()
    }

    fn commit(&mut self) -> ToolResponse {
        if self.nodes.is_empty() {
            return ToolResponse::Cancel;
        }

        let mut path = VectorPath::new();
        path.nodes = std::mem::take(&mut self.nodes);
        path.closed = false;
        self.dragging_handle = false;
        ToolResponse::Commit(vec![ToolCommit::Path(path)])
    }

    fn apply_smooth(&self, node: &mut PathNode) {
        let len_in_x = node.handle_in.x - node.point.x;
        let len_in_y = node.handle_in.y - node.point.y;
        if len_in_x.abs() < 1e-9 && len_in_y.abs() < 1e-9 {
            return;
        }
        node.handle_out.x = node.point.x - len_in_x;
        node.handle_out.y = node.point.y - len_in_y;
    }
}

impl Tool for CurveTool {
    fn kind(&self) -> ToolKind {
        ToolKind::Curve
    }

    fn reset(&mut self) {
        self.nodes.clear();
        self.dragging_handle = false;
    }

    fn is_active(&self) -> bool {
        !self.nodes.is_empty()
    }

    fn hint(&self) -> &'static str {
        "click to add smooth nodes, drag to adjust handle, Enter/double-click to finish, Esc to cancel"
    }

    fn handle(&mut self, event: ToolEvent, ctx: &ToolContext) -> ToolResponse {
        match event {
            ToolEvent::PrimaryDown => {
                let p = ctx.snapped;
                if let Some(last) = self.nodes.last_mut() {
                    let mut node = PathNode::corner(p.x, p.y);
                    node.kind = crate::core::vector::NodeKind::Smooth;
                    let dx = (node.point.x - last.point.x) * 0.333;
                    let dy = (node.point.y - last.point.y) * 0.333;
                    node.handle_in = CurvePoint::new(node.point.x - dx, node.point.y - dy);
                    node.handle_out = CurvePoint::new(node.point.x + dx, node.point.y + dy);
                    self.nodes.push(node);
                } else {
                    let mut node = PathNode::corner(p.x, p.y);
                    node.kind = crate::core::vector::NodeKind::Smooth;
                    self.nodes.push(node);
                }
                self.dragging_handle = true;
                self.current = p;
                ToolResponse::Consumed
            }
            ToolEvent::PrimaryUp => {
                self.dragging_handle = false;
                ToolResponse::Consumed
            }
            ToolEvent::Move => {
                self.current = ctx.snapped;
                if self.dragging_handle {
                    if let Some(last) = self.nodes.last_mut() {
                        last.handle_out = ctx.snapped;
                        last.handle_in = CurvePoint::new(
                            2.0 * last.point.x - ctx.snapped.x,
                            2.0 * last.point.y - ctx.snapped.y,
                        );
                    }
                }
                ToolResponse::Consumed
            }
            ToolEvent::Enter | ToolEvent::Finish => self.commit(),
            ToolEvent::Escape => {
                self.nodes.clear();
                self.dragging_handle = false;
                ToolResponse::Cancel
            }
            ToolEvent::SecondaryDown => self.commit(),
            _ => ToolResponse::None,
        }
    }

    fn preview(&self, to_screen: &dyn Fn(CurvePoint) -> Pos2) -> ToolPreview {
        let mut preview = ToolPreview::default();
        let color = Color32::from_rgb(80, 200, 255);
        let handle_color = Color32::from_rgb(255, 160, 60);

        let mut path = VectorPath::new();
        path.nodes = self.nodes.clone();
        let pts = path.flatten(0.2);

        if pts.len() >= 2 {
            let screen: Vec<Pos2> = pts.iter().map(|p| to_screen(*p)).collect();
            preview.polylines.push((screen, color));
        }

        if let Some(last) = self.nodes.last() {
            let a = to_screen(last.point);
            let b = to_screen(self.current);
            preview.polylines.push((vec![a, b], Color32::from_rgba_unmultiplied(80, 200, 255, 80)));
        }

        for node in &self.nodes {
            preview.points.push((to_screen(node.point), color));
            if node.has_handles() {
                let hp = to_screen(node.handle_out);
                preview.points.push((hp, handle_color));
                preview.circles.push((hp, 3.0, handle_color));
            }
        }

        preview
    }
}