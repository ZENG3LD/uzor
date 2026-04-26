//! Desktop factory rendering functions for uzor widgets
//!
//! This module provides default rendering implementations for all 9 widget types
//! using a minimal RenderContext API. Applications implement the RenderContext trait
//! for their specific renderer (vello, skia, canvas, etc.).

pub mod button;
pub mod container;
pub mod popup;
pub mod panel;
pub mod text_input;
pub mod dropdown;
pub mod slider;
pub mod toast;

// Re-export main rendering functions
pub use button::render_default as render_button;
pub use container::render_default as render_container;
pub use popup::render_default as render_popup;
pub use panel::render_default as render_panel;
pub use text_input::render_default as render_text_input;
pub use dropdown::render_default as render_dropdown;
pub use slider::render_default as render_slider;
pub use toast::render_default as render_toast;

// =============================================================================
// Re-export RenderContext from uzor-render
// =============================================================================

pub use uzor::render::{RenderContext, TextAlign, TextBaseline};

// =============================================================================
// Helper Functions
// =============================================================================

/// Convert RGBA color array to hex color string
pub fn rgba_to_hex(rgba: [u8; 4]) -> String {
    if rgba[3] == 255 {
        // Fully opaque - use RGB hex
        format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2])
    } else {
        // Has transparency - use RGBA hex
        format!("#{:02X}{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2], rgba[3])
    }
}
