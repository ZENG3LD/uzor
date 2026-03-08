//! Alignment utilities for UZOR layouts

use crate::types::rect::Rect;

/// Center a child rect within a parent rect
pub fn center_rect(parent: Rect, child_width: f64, child_height: f64) -> Rect {
    let x = parent.x + (parent.width - child_width) / 2.0;
    let y = parent.y + (parent.height - child_height) / 2.0;
    Rect::new(x, y, child_width, child_height)
}

/// Align rect to the right of parent
pub fn align_right(parent: Rect, child_width: f64, child_height: f64, margin: f64) -> Rect {
    let x = parent.x + parent.width - child_width - margin;
    let y = parent.y + margin;
    Rect::new(x, y, child_width, child_height)
}

/// Align rect to the left of parent
pub fn align_left(parent: Rect, child_width: f64, child_height: f64, margin: f64) -> Rect {
    let x = parent.x + margin;
    let y = parent.y + margin;
    Rect::new(x, y, child_width, child_height)
}

/// Align rect to the top of parent
pub fn align_top(parent: Rect, child_width: f64, child_height: f64, margin: f64) -> Rect {
    let x = parent.x + (parent.width - child_width) / 2.0;
    let y = parent.y + margin;
    Rect::new(x, y, child_width, child_height)
}

/// Align rect to the bottom of parent
pub fn align_bottom(parent: Rect, child_width: f64, child_height: f64, margin: f64) -> Rect {
    let x = parent.x + (parent.width - child_width) / 2.0;
    let y = parent.y + parent.height - child_height - margin;
    Rect::new(x, y, child_width, child_height)
}
