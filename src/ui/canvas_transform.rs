pub struct CanvasTransform {
    pub scale: f32,
    pub offset: egui::Vec2,
    pub rect: egui::Rect,
}

impl CanvasTransform {
    pub fn new(rect: egui::Rect, zoom: f32, pan: egui::Vec2) -> Self {
        Self {
            scale: zoom,
            offset: egui::Vec2 {
                x: rect.width() / 2.0 + pan.x,
                y: rect.height() / 2.0 + pan.y,
            },
            rect,
        }
    }

    pub fn to_screen(&self, x: f64, y: f64) -> egui::Pos2 {
        egui::pos2(
            self.rect.min.x + x as f32 * self.scale + self.offset.x,
            self.rect.min.y - y as f32 * self.scale + self.offset.y,
        )
    }

    pub fn to_world(&self, pos: egui::Pos2) -> (f64, f64) {
        (
            ((pos.x - self.rect.min.x - self.offset.x) / self.scale) as f64,
            (-(pos.y - self.rect.min.y - self.offset.y) / self.scale) as f64,
        )
    }
}