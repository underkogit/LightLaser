use egui::{Rect, TextureHandle, Vec2};
use std::collections::HashMap;

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

pub struct PreviewWindow {
    pub open: bool,
    pub lines: Vec<(f64, f64, f64, f64)>,
    pub bounds: Option<(f64, f64, f64, f64)>,
    pub zoom: f32,
    pub offset: Vec2,
    pub pending: bool,
    pub pending_fit: bool,
    pub progress: f32,
}

impl Default for PreviewWindow {
    fn default() -> Self {
        Self {
            open: false,
            lines: Vec::new(),
            bounds: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
            pending: false,
            pending_fit: false,
            progress: 1.0,
        }
    }
}

pub struct SegmentCacheEntry {
    pub key: String,
    pub segments: Vec<(f64, f64, f64, f64)>,
    pub bounds: Option<(f64, f64, f64, f64)>,
}

pub struct CanvasState {
    pub zoom: f32,
    pub offset: Vec2,
    pub rect: Rect,
    pub pending_center: bool,
    pub hovered_layer: Option<usize>,
    pub drag: DragMode,
    pub texture_cache: HashMap<String, egui::TextureHandle>,
    pub segment_cache: HashMap<String, SegmentCacheEntry>,
    pub preview_window: PreviewWindow,
}

impl std::fmt::Debug for CanvasState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CanvasState")
            .field("zoom", &self.zoom)
            .field("offset", &self.offset)
            .field("rect", &self.rect)
            .field("pending_center", &self.pending_center)
            .field("hovered_layer", &self.hovered_layer)
            .field("drag", &self.drag)
            .field("preview_open", &self.preview_window.open)
            .finish()
    }
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
            texture_cache: HashMap::new(),
            segment_cache: HashMap::new(),
            preview_window: PreviewWindow::default(),
        }
    }
}