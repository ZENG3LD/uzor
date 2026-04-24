//! Radio group theme contract

pub trait RadioTheme {
    fn circle_radius(&self) -> f64;
    fn circle_border_color(&self) -> u32;
    fn circle_fill_color(&self) -> u32;
    fn circle_selected_color(&self) -> u32;
    fn label_color(&self) -> u32;
    fn description_color(&self) -> u32;
    fn hover_background(&self) -> u32;
    fn row_height(&self) -> f64;
    fn gap(&self) -> f64;
}
