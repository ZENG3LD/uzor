use serde::{Serialize, Deserialize};

/// Rectangle primitive for panel geometry (f32 precision)
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PanelRect {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };

    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.height / 2.0
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width
            && py >= self.y && py <= self.y + self.height
    }

    /// Split horizontally: left gets `left_width`, right gets the rest
    pub fn split_horizontal(&self, left_width: f32) -> (Self, Self) {
        let left = Self::new(self.x, self.y, left_width, self.height);
        let right = Self::new(self.x + left_width, self.y, self.width - left_width, self.height);
        (left, right)
    }

    /// Split vertically: top gets `top_height`, bottom gets the rest
    pub fn split_vertical(&self, top_height: f32) -> (Self, Self) {
        let top = Self::new(self.x, self.y, self.width, top_height);
        let bottom = Self::new(self.x, self.y + top_height, self.width, self.height - top_height);
        (top, bottom)
    }

    /// Shrink by padding on all sides
    pub fn inset(&self, padding: f32) -> Self {
        Self::new(
            self.x + padding,
            self.y + padding,
            (self.width - 2.0 * padding).max(0.0),
            (self.height - 2.0 * padding).max(0.0),
        )
    }

    pub fn area(&self) -> f32 {
        self.width * self.height
    }
}

/// Convert from (f64) tuple for interop with uzor-core Rect
impl From<(f64, f64, f64, f64)> for PanelRect {
    fn from((x, y, w, h): (f64, f64, f64, f64)) -> Self {
        Self::new(x as f32, y as f32, w as f32, h as f32)
    }
}

impl From<PanelRect> for (f64, f64, f64, f64) {
    fn from(r: PanelRect) -> Self {
        (r.x as f64, r.y as f64, r.width as f64, r.height as f64)
    }
}
