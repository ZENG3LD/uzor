//! Separator / resize handle widget.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

// --- types ---
pub use types::{SeparatorOrientation, SeparatorType};

// --- state ---
pub use state::{SeparatorDragState, SeparatorHoverState};

// --- theme ---
pub use theme::{DefaultSeparatorTheme, SeparatorTheme};

// --- style ---
pub use style::{
    DefaultSeparatorStyle, ModalSectionDividerStyle, SeparatorStyle, SidebarSeparatorStyle,
    SplitPanelSeparatorStyle, SubPaneSeparatorStyle,
    // per-variant constants
    SIDEBAR_HIT_ZONE, SIDEBAR_MIN_WIDTH, SIDEBAR_VISUAL_THICKNESS,
    SPLIT_PANEL_HIT_ZONE, SPLIT_PANEL_THICKNESS_HOVER_DRAG, SPLIT_PANEL_THICKNESS_IDLE,
    SUB_PANE_HIT_TOLERANCE, SUB_PANE_MIN_SIZE_DRAG, SUB_PANE_MIN_SIZE_LAYOUT,
};

// --- settings ---
pub use settings::SeparatorSettings;

// --- render ---
pub use render::{
    draw_modal_section_divider, draw_pane_resize_handle, draw_separator, draw_separator_line,
    draw_sidebar_handle, draw_split_panel_handle, SeparatorView,
};

// --- input ---
pub use input::{
    end_separator_drag, register, register_separator, start_separator_drag,
    update_separator_drag, SeparatorKind,
};
