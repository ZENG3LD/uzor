//! Sizing utilities

use crate::types::rect::Rect;

/// Calculate rect with aspect ratio
pub fn aspect_ratio(width: f64, ratio: f64) -> (f64, f64) {
    (width, width / ratio)
}

/// Fit rect within bounds while maintaining aspect ratio
pub fn fit_in_bounds(content_width: f64, content_height: f64, max_width: f64, max_height: f64) -> (f64, f64) {
    let width_ratio = max_width / content_width;
    let height_ratio = max_height / content_height;
    let scale = width_ratio.min(height_ratio);

    (content_width * scale, content_height * scale)
}

/// Calculate modal rect centered in screen
pub fn modal_rect(screen_width: f64, screen_height: f64, modal_width: f64, modal_height: f64) -> Rect {
    let x = (screen_width - modal_width) / 2.0;
    let y = (screen_height - modal_height) / 2.0;
    Rect::new(x, y, modal_width, modal_height)
}
