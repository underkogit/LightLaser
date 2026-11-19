use egui::{Rect, Vec2};
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
        start_states: Vec<LayerDragState>,
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

#[derive(Debug, Clone)]
pub struct LayerDragState {
    pub id: usize,
    pub start_offset: [f64; 2],
    pub start_bounds: Option<[f64; 4]>,
    pub start_shape_nodes: Vec<Vec<crate::core::vector::PathNode>>,
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
    /// Incremented whenever `lines`/`bounds` change, so caches can detect stale data.
    pub revision: u64,
    pub raster: PreviewRaster,
}

/// Cached rasterization of the preview geometry.
///
/// The segments are rasterized into world-space textures covering the currently visible
/// area, so panning/zooming only needs to map a quad again. A stale texture is stretched
/// while the user is still moving the view and re-rendered once the view settles, which
/// keeps interaction smooth even for jobs with hundreds of thousands of segments.
pub struct PreviewRaster {
    /// View key the textures were rendered for (0 = nothing rendered yet).
    pub key: u64,
    /// View key requested in the previous frame (used to detect a settled view).
    pub pending_key: u64,
    /// World rectangle covered by the textures.
    pub world_rect: (f64, f64, f64, f64),
    /// Pixels per mm of the textures.
    pub level: f64,
    /// All segments in the "not processed" color.
    pub base: Option<egui::TextureHandle>,
    /// Work-in-progress pixmap of the processed (bright) segments.
    pub done_pixmap: Option<tiny_skia::Pixmap>,
    /// Processed segments as a texture.
    pub done: Option<egui::TextureHandle>,
    /// Number of segments baked into `done_pixmap`.
    pub drawn: usize,
    /// When the textures were last rasterized, used to throttle updates while the view moves.
    pub last_render: Option<std::time::Instant>,
    /// How long the last rasterization took; cheap ones may be redone every frame.
    pub last_cost: std::time::Duration,
    /// When the processed texture was last updated, used to throttle progress dragging.
    pub last_progress: Option<std::time::Instant>,
}

impl Default for PreviewRaster {
    fn default() -> Self {
        Self {
            key: 0,
            pending_key: 0,
            world_rect: (0.0, 0.0, 1.0, 1.0),
            level: 1.0,
            base: None,
            done_pixmap: None,
            done: None,
            drawn: 0,
            last_render: None,
            last_cost: std::time::Duration::ZERO,
            last_progress: None,
        }
    }
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
            revision: 1,
            raster: PreviewRaster::default(),
        }
    }
}

pub struct GcodeWindow {
    pub open: bool,
    pub text: String,
    pub error: Option<String>,
    pub pending: bool,
    pub wrap: bool,
}

impl Default for GcodeWindow {
    fn default() -> Self {
        Self {
            open: false,
            text: String::new(),
            error: None,
            pending: false,
            wrap: true,
        }
    }
}

pub struct SegmentCacheEntry {
    pub segments: Vec<(f64, f64, f64, f64)>,
    /// Bounding box of `segments` (local coordinates, layer offset not included).
    pub bbox: Option<(f64, f64, f64, f64)>,
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
    pub gcode_window: GcodeWindow,
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
            .field("gcode_open", &self.gcode_window.open)
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
            gcode_window: GcodeWindow::default(),
        }
    }
}
