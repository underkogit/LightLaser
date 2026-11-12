use serde::{Deserialize, Serialize};

use crate::core::vector::curve::{Bezier, CurvePoint};
use crate::core::vector::stroke::StrokeStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Corner,
    Smooth,
}

impl Default for NodeKind {
    fn default() -> Self {
        NodeKind::Corner
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PathNode {
    #[serde(default)]
    pub point: CurvePoint,

    #[serde(default)]
    pub handle_in: CurvePoint,

    #[serde(default)]
    pub handle_out: CurvePoint,

    #[serde(default)]
    pub kind: NodeKind,
}

impl PathNode {
    pub fn corner(x: f64, y: f64) -> Self {
        Self {
            point: CurvePoint::new(x, y),
            handle_in: CurvePoint::new(x, y),
            handle_out: CurvePoint::new(x, y),
            kind: NodeKind::Corner,
        }
    }

    pub fn smooth(x: f64, y: f64, hx: f64, hy: f64) -> Self {
        Self {
            point: CurvePoint::new(x, y),
            handle_in: CurvePoint::new(hx, hy),
            handle_out: CurvePoint::new(2.0 * x - hx, 2.0 * y - hy),
            kind: NodeKind::Smooth,
        }
    }

    pub fn has_handles(&self) -> bool {
        (self.handle_in.x - self.point.x).abs() > 1e-9
            || (self.handle_in.y - self.point.y).abs() > 1e-9
            || (self.handle_out.x - self.point.x).abs() > 1e-9
            || (self.handle_out.y - self.point.y).abs() > 1e-9
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.point.x += dx;
        self.point.y += dy;
        self.handle_in.x += dx;
        self.handle_in.y += dy;
        self.handle_out.x += dx;
        self.handle_out.y += dy;
    }

    pub fn set_point(&mut self, x: f64, y: f64) {
        let dx = x - self.point.x;
        let dy = y - self.point.y;
        self.translate(dx, dy);
    }

    pub fn set_handle_out(&mut self, x: f64, y: f64) {
        self.handle_out.x = x;
        self.handle_out.y = y;
        if self.kind == NodeKind::Smooth {
            self.handle_in.x = 2.0 * self.point.x - x;
            self.handle_in.y = 2.0 * self.point.y - y;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorPath {
    #[serde(default)]
    pub nodes: Vec<PathNode>,

    #[serde(default)]
    pub stroke: StrokeStyle,

    #[serde(default)]
    pub closed: bool,
}

impl VectorPath {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            stroke: StrokeStyle::default(),
            closed: false,
        }
    }

    pub fn push_corner(&mut self, x: f64, y: f64) {
        self.nodes.push(PathNode::corner(x, y));
    }

    pub fn push_smooth(&mut self, x: f64, y: f64, hx: f64, hy: f64) {
        self.nodes.push(PathNode::smooth(x, y, hx, hy));
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn to_bezier(&self) -> Bezier {
        let mut bez = Bezier::new();
        if self.nodes.len() < 2 {
            return bez;
        }

        let n = self.nodes.len();
        let last = if self.closed { n } else { n - 1 };

        for i in 0..last {
            let a = &self.nodes[i];
            let b = &self.nodes[(i + 1) % n];
            bez.add_cubic(a.point, a.handle_out, b.handle_in, b.point);
        }

        bez
    }

    pub fn flatten(&self, tolerance: f64) -> Vec<CurvePoint> {
        self.to_bezier().flatten(tolerance)
    }

    pub fn bounds(&self, tolerance: f64) -> Option<(f64, f64, f64, f64)> {
        if self.nodes.is_empty() {
            return None;
        }

        let pts = self.flatten(tolerance);
        if pts.is_empty() {
            return None;
        }

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for p in &pts {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }

        let pad = self.stroke.width_mm * 0.5;
        Some((min_x - pad, min_y - pad, max_x + pad, max_y + pad))
    }
}