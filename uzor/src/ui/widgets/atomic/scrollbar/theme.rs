//! Scrollbar theme contract

pub trait ScrollbarTheme {
    fn thumb_color(&self) -> u32;
    fn thumb_hover_color(&self) -> u32;
    fn thumb_drag_color(&self) -> u32;
    fn track_color(&self) -> u32;
    fn track_hover_color(&self) -> u32;
    fn width(&self) -> f64;
    fn min_thumb_height(&self) -> f64;
    fn border_radius(&self) -> f64;
}
