//! Scrollbar widget.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

// ── Types ─────────────────────────────────────────────────────────────────────
pub use types::{ScrollbarOrientation, ScrollbarType};

// ── State ─────────────────────────────────────────────────────────────────────
pub use state::ScrollState;

// ── Theme ─────────────────────────────────────────────────────────────────────
pub use theme::{DefaultScrollbarTheme, LightScrollbarTheme, ScrollbarTheme};

// ── Style ─────────────────────────────────────────────────────────────────────
pub use style::{
    CompactScrollbarStyle, DefaultScrollbarStyle, ScrollbarStyle, SignalScrollbarStyle,
    StandardScrollbarStyle,
};

// ── Settings ──────────────────────────────────────────────────────────────────
pub use settings::ScrollbarSettings;

// ── Render ────────────────────────────────────────────────────────────────────
pub use render::{
    draw_scrollbar, draw_scrollbar_compact, draw_scrollbar_signal, draw_scrollbar_standard,
    ScrollbarResult, ScrollbarView, ScrollbarVisualState,
};

// ── Input ─────────────────────────────────────────────────────────────────────
pub use input::{
    end_thumb_drag, handle_scroll_wheel, handle_track_click, register_thumb, register_track,
    start_thumb_drag, try_end_scrollbar_drag, try_handle_scrollbar_drag,
    try_handle_track_click, try_handle_wheel, try_start_scrollbar_drag, update_thumb_drag,
    ScrollableInfo,
    register_input_coordinator_scrollbar,
    register_context_manager_scrollbar,
};
