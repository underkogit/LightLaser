use serde::{Deserialize, Serialize};

use crate::core::geometry::Point2d;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub x: f64,
    pub y: f64,
}

impl CurvePoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn to_point(self) -> Point2d {
        Point2d::new(self.x, self.y)
    }

    pub fn from_point(p: &Point2d) -> Self {
        Self { x: p.x, y: p.y }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BezierSegment {
    pub p0: CurvePoint,
    pub c1: CurvePoint,
    pub c2: CurvePoint,
    pub p1: CurvePoint,
}

impl BezierSegment {
    pub fn line(p0: CurvePoint, p1: CurvePoint) -> Self {
        Self {
            p0,
            c1: p0,
            c2: p1,
            p1,
        }
    }

    pub fn eval(&self, t: f64) -> CurvePoint {
        let t = t.clamp(0.0, 1.0);
        let mt = 1.0 - t;
        let a = mt * mt * mt;
        let b = 3.0 * mt * mt * t;
        let c = 3.0 * mt * t * t;
        let d = t * t * t;

        CurvePoint {
            x: a * self.p0.x + b * self.c1.x + c * self.c2.x + d * self.p1.x,
            y: a * self.p0.y + b * self.c1.y + c * self.c2.y + d * self.p1.y,
        }
    }

    pub fn flatten(&self, tolerance: f64, out: &mut Vec<CurvePoint>) {
        let tol = tolerance.max(0.001);
        let rough = (self.p0.x - self.p1.x).abs() + (self.p0.y - self.p1.y).abs()
            + (self.c1.x - self.p0.x).abs()
            + (self.c1.y - self.p0.y).abs()
            + (self.c2.x - self.p1.x).abs()
            + (self.c2.y - self.p1.y).abs();

        let steps = ((rough / tol).sqrt().ceil() as usize).clamp(4, 256);

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            out.push(self.eval(t));
        }
    }

    pub fn control_polygon(&self) -> [CurvePoint; 4] {
        [self.p0, self.c1, self.c2, self.p1]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Bezier {
    pub segments: Vec<BezierSegment>,
}

impl Bezier {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn from_points(points: &[CurvePoint]) -> Self {
        let mut segments = Vec::new();
        for w in points.windows(2) {
            segments.push(BezierSegment::line(w[0], w[1]));
        }
        Self { segments }
    }

    pub fn add_line(&mut self, from: CurvePoint, to: CurvePoint) {
        self.segments.push(BezierSegment::line(from, to));
    }

    pub fn add_cubic(
        &mut self,
        p0: CurvePoint,
        c1: CurvePoint,
        c2: CurvePoint,
        p1: CurvePoint,
    ) {
        self.segments.push(BezierSegment {
            p0,
            c1,
            c2,
            p1,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn flatten(&self, tolerance: f64) -> Vec<CurvePoint> {
        let mut out: Vec<CurvePoint> = Vec::new();
        for (i, seg) in self.segments.iter().enumerate() {
            if i == 0 {
                out.push(seg.p0);
            }
            seg.flatten(tolerance, &mut out);
        }
        out
    }

    pub fn bounds(&self, tolerance: f64) -> Option<(f64, f64, f64, f64)> {
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
        Some((min_x, min_y, max_x, max_y))
    }
}