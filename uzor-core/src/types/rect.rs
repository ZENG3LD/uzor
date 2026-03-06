//! Rectangle type for widget layout

use serde::{Deserialize, Serialize};

/// Rectangle for widget layout
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Compatibility alias
pub type WidgetRect = Rect;

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }

    pub fn min_x(&self) -> f64 { self.x }
    pub fn min_y(&self) -> f64 { self.y }
    pub fn max_x(&self) -> f64 { self.x + self.width }
    pub fn max_y(&self) -> f64 { self.y + self.height }

    pub fn width(&self) -> f64 { self.width }
    pub fn height(&self) -> f64 { self.height }

    pub fn intersect(&self, other: Rect) -> Rect {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        
        let w = (right - x).max(0.0);
        let h = (bottom - y).max(0.0);
        
        Rect::new(x, y, w, h)
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn center_x(&self) -> f64 {
        self.x + self.width / 2.0
    }

    pub fn center_y(&self) -> f64 {
        self.y + self.height / 2.0
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Shrink by padding
    pub fn inset(&self, padding: f64) -> Self {
        Self {
            x: self.x + padding,
            y: self.y + padding,
            width: (self.width - padding * 2.0).max(0.0),
            height: (self.height - padding * 2.0).max(0.0),
        }
    }

    /// Split horizontally, return (left, right)
    pub fn split_horizontal(&self, left_width: f64) -> (Self, Self) {
        let left = Self {
            x: self.x,
            y: self.y,
            width: left_width.min(self.width),
            height: self.height,
        };
        let right = Self {
            x: self.x + left.width,
            y: self.y,
            width: (self.width - left.width).max(0.0),
            height: self.height,
        };
        (left, right)
    }

    /// Split vertically, return (top, bottom)
    pub fn split_vertical(&self, top_height: f64) -> (Self, Self) {
        let top = Self {
            x: self.x,
            y: self.y,
            width: self.width,
            height: top_height.min(self.height),
        };
        let bottom = Self {
            x: self.x,
            y: self.y + top.height,
            width: self.width,
            height: (self.height - top.height).max(0.0),
        };
        (top, bottom)
    }
}