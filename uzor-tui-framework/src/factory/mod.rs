//! TUI widget factory - render uzor-core widgets to terminal cells

pub mod defaults;
pub mod button;
pub mod slider;
pub mod toast;
pub mod container;
pub mod popup;
pub mod panel;
pub mod text_input;
pub mod dropdown;

// Re-export key types
pub use defaults::{TuiColors, TuiIcons, TuiButtonDefaults, icon_to_char};

// Re-export render functions
pub use button::render_default as render_button;
pub use slider::render_default as render_slider;
pub use toast::render_default as render_toast;
pub use container::render_default as render_container;
pub use popup::render_default as render_popup;
pub use panel::render_default as render_panel;
pub use text_input::render_default as render_text_input;
pub use dropdown::render_default as render_dropdown;
