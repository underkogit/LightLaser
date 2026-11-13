#[derive(Debug, Default)]
pub struct StatusBar {
    mouse_x: f64,
    mouse_y: f64,
}

impl StatusBar {
    pub fn set_mouse_world(&mut self, x: f64, y: f64) {
        self.mouse_x = x;
        self.mouse_y = y;
    }

    pub fn mouse_x(&self) -> f64 {
        self.mouse_x
    }

    pub fn mouse_y(&self) -> f64 {
        self.mouse_y
    }
}
