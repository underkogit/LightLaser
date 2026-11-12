use crate::core::vector::{CurvePoint, VectorPath};

#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub layers: Vec<usize>,
    pub layer: Option<usize>,
    pub shape: Option<usize>,
    pub node: Option<usize>,
    pub handle: Option<HandleKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleKind {
    In,
    Out,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty() && self.layer.is_none()
    }

    pub fn clear(&mut self) {
        self.layers.clear();
        self.layer = None;
        self.shape = None;
        self.node = None;
        self.handle = None;
    }

    pub fn clear_nodes(&mut self) {
        self.node = None;
        self.handle = None;
    }

    pub fn is_layer_selected(&self, idx: usize) -> bool {
        self.layers.contains(&idx) || self.layer == Some(idx)
    }

    pub fn toggle_layer(&mut self, idx: usize) {
        if let Some(pos) = self.layers.iter().position(|&x| x == idx) {
            self.layers.remove(pos);
        } else {
            self.layers.push(idx);
        }
        self.layer = self.layers.last().copied();
        self.shape = None;
        self.node = None;
        self.handle = None;
    }

    pub fn set_single_layer(&mut self, idx: usize) {
        self.layers = vec![idx];
        self.layer = Some(idx);
        self.shape = None;
        self.node = None;
        self.handle = None;
    }
}

pub fn dist2(a: CurvePoint, b: CurvePoint) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

pub fn hit_node(path: &VectorPath, p: CurvePoint, tol: f64) -> Option<usize> {
    let t2 = tol * tol;
    for (i, n) in path.nodes.iter().enumerate() {
        if dist2(n.point, p) <= t2 {
            return Some(i);
        }
    }
    None
}

pub fn hit_handle(
    path: &VectorPath,
    node_idx: usize,
    p: CurvePoint,
    tol: f64,
) -> Option<HandleKind> {
    let t2 = tol * tol;
    let node = path.nodes.get(node_idx)?;
    if dist2(node.handle_out, p) <= t2 {
        return Some(HandleKind::Out);
    }
    if dist2(node.handle_in, p) <= t2 {
        return Some(HandleKind::In);
    }
    None
}

pub fn hit_path(path: &VectorPath, p: CurvePoint, tol: f64) -> bool {
    let pts = path.flatten(tol.max(0.05) * 0.5);
    if pts.len() < 2 {
        return false;
    }
    let t2 = tol * tol;
    for w in pts.windows(2) {
        if point_seg_dist2(p, w[0], w[1]) <= t2 {
            return true;
        }
    }
    false
}

fn point_seg_dist2(p: CurvePoint, a: CurvePoint, b: CurvePoint) -> f64 {
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let apx = p.x - a.x;
    let apy = p.y - a.y;
    let len2 = abx * abx + aby * aby;
    let t = if len2 <= 1e-12 {
        0.0
    } else {
        ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0)
    };
    let cx = a.x + abx * t;
    let cy = a.y + aby * t;
    let dx = p.x - cx;
    let dy = p.y - cy;
    dx * dx + dy * dy
}
