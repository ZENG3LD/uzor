//! Slider widget — Single / Dual / LineWidth.
//!
//! - `theme`  — `SliderTheme` trait.
//! - `tokens` — `SliderTokens`: the token-contract implementation of
//!              `SliderTheme`, one pre-rendered value per owned method,
//!              built from `SemanticRoles` + optional `component.slider.*`
//!              overrides (H1 token contract design §3). `TokenTheme`
//!              (`crate::tokens::theme`) implements `SliderTheme` by
//!              reading these fields.

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{
    DualSliderHandle, SliderType,
    SliderConfig, SliderEditingInfo, SliderTrackInfo,
    SingleSliderView, DualSliderView, LineWidthSliderView,
    SingleSliderResult, DualSliderResult, LineWidthSliderResult,
};
pub use state::SliderDragState;
pub use theme::SliderTheme;
pub use style::{DefaultSliderStyle, SliderStyle};
pub use settings::SliderSettings;
pub use render::{
    draw_slider, SliderResult, SliderView,
    draw_single_slider, draw_dual_slider, draw_line_width_slider,
};
pub use input::{
    register,
    // Math helpers
    pixel_to_value, value_to_pixel, clamp_step,
    // Drag lifecycle
    start_slider_drag, update_slider_drag_float, end_slider_drag,
    // Other input events
    handle_slider_scroll, handle_slider_click,
    handle_slider_text_input,
    handle_slider_arrow_key, ArrowDirection,
    // Level 1 / Level 2
    register_input_coordinator_slider,
    register_context_manager_slider,
    register_layout_manager_slider,
};
