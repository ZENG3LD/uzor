//! Separator theme contract

pub trait SeparatorTheme {
    fn divider_color(&self) -> u32;
    fn handle_color(&self) -> u32;
    fn handle_hover_color(&self) -> u32;
    fn handle_drag_color(&self) -> u32;
    fn thickness(&self) -> f64;
    fn hit_width(&self) -> f64;
}
