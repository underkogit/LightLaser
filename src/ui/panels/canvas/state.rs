use egui::{Rect, Vec2};

use crate::core::vector::CurvePoint;
use crate::ui::selection::HandleKind;

pub const MIN_ZOOM: f32 = 0.05;
pub const MAX_ZOOM: f32 = 100.0;

#[derive(Debug, Default)]
pub enum DragMode {
    #[default]
    None,
    Pan,
    RubberBand {
        start_world: CurvePoint,
        current_world: CurvePoint,
    },
    MoveLayer {
        start_world: CurvePoint,
        start_offset: [f64; 2],
    },
    MoveNode {
        layer: usize,
        shape: usize,
        node: usize,
    },
    MoveHandle {
        layer: usize,
        shape: usize,
        node: usize,
        handle: HandleKind,
    },
}

#[derive(Debug)]
pub struct CanvasState {
    pub zoom: f32,
    pub offset: Vec2,
    pub rect: Rect,
    pub pending_center: bool,
    pub hovered_layer: Option<usize>,
    pub drag: DragMode,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: Vec2::ZERO,
            rect: Rect::NOTHING,
            pending_center: false,
            hovered_layer: None,
            drag: DragMode::None,
        }
    }
}