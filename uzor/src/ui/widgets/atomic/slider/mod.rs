//! Slider widget — Single / Dual.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{DualSliderHandle, SliderType};
pub use state::SliderDragState;
pub use theme::{DefaultSliderTheme, SliderTheme};
pub use style::{DefaultSliderStyle, SliderStyle};
pub use settings::SliderSettings;
pub use render::{draw_slider, SliderResult, SliderView};
pub use input::register;
