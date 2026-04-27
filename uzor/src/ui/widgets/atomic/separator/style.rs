//! Separator geometry — styles and per-variant presets.

// =============================================================================
// Per-variant size constants (mlc hardcoded numbers, §9)
// =============================================================================

/// Sub-pane separator: hit tolerance ±6 px around the 1 px line center.
pub const SUB_PANE_HIT_TOLERANCE: f64 = 6.0;
/// Sub-pane separator: minimum pane height enforced during drag (px).
pub const SUB_PANE_MIN_SIZE_DRAG: f64 = 80.0;
/// Sub-pane separator: minimum pane height used by the layout engine (px).
pub const SUB_PANE_MIN_SIZE_LAYOUT: f64 = 30.0;

/// Split-panel separator: visual thickness at idle (px).
pub const SPLIT_PANEL_THICKNESS_IDLE: f64 = 2.0;
/// Split-panel separator: visual thickness while hovered or dragging (px).
pub const SPLIT_PANEL_THICKNESS_HOVER_DRAG: f64 = 4.0;
/// Split-panel separator: hit zone width centered on the visual line (px).
pub const SPLIT_PANEL_HIT_ZONE: f64 = 8.0;

/// Sidebar separator: visual thickness (px).
pub const SIDEBAR_VISUAL_THICKNESS: f64 = 1.0;
/// Sidebar separator: hit zone width centered on the visual line (px).
pub const SIDEBAR_HIT_ZONE: f64 = 8.0;
/// Sidebar: minimum width enforced during drag (px).
pub const SIDEBAR_MIN_WIDTH: f64 = 280.0;

// =============================================================================
// Core trait
// =============================================================================

pub trait SeparatorStyle {
    /// Line thickness in pixels (visual).
    fn thickness(&self) -> f64;
    /// Margin (perpendicular padding from container edge).
    fn margin(&self) -> f64;
    /// Resize-handle hit area thickness (wider than visible line).
    fn handle_hit_thickness(&self) -> f64;
}

// =============================================================================
// Default (generic divider / resize handle)
// =============================================================================

/// Default style: 1 px line, ±6 px hit zone, no margin.
pub struct DefaultSeparatorStyle;

impl Default for DefaultSeparatorStyle {
    fn default() -> Self {
        Self
    }
}

impl SeparatorStyle for DefaultSeparatorStyle {
    fn thickness(&self) -> f64 {
        1.0
    }
    fn margin(&self) -> f64 {
        0.0
    }
    fn handle_hit_thickness(&self) -> f64 {
        6.0
    }
}

// =============================================================================
// Sub-pane separator style
// =============================================================================

/// Sub-pane separator (between main chart and indicator panes).
///
/// Visual: 1 px filled line spanning full panel width.
/// Hit zone: ±6 px around line center (total 12 px).
/// Min sizes: 80 px drag / 30 px layout.
pub struct SubPaneSeparatorStyle;

impl Default for SubPaneSeparatorStyle {
    fn default() -> Self {
        Self
    }
}

impl SeparatorStyle for SubPaneSeparatorStyle {
    fn thickness(&self) -> f64 {
        1.0
    }
    fn margin(&self) -> f64 {
        0.0
    }
    fn handle_hit_thickness(&self) -> f64 {
        SUB_PANE_HIT_TOLERANCE * 2.0
    }
}

impl SubPaneSeparatorStyle {
    /// Minimum pane height enforced during drag.
    pub fn min_size_drag(&self) -> f64 {
        SUB_PANE_MIN_SIZE_DRAG
    }
    /// Minimum pane height used by layout engine.
    pub fn min_size_layout(&self) -> f64 {
        SUB_PANE_MIN_SIZE_LAYOUT
    }
}

// =============================================================================
// Split-panel separator style
// =============================================================================

/// Split-panel separator (between chart sub-windows in ChartPanelGrid).
///
/// Visual: 2 px idle / 4 px hover-drag.
/// Hit zone: 8 px centered on position.
/// Color changes on hover/drag (see `SeparatorTheme::pane_handle_*`).
pub struct SplitPanelSeparatorStyle {
    /// Whether the separator is currently hovered or being dragged.
    pub active: bool,
}

impl Default for SplitPanelSeparatorStyle {
    fn default() -> Self {
        Self { active: false }
    }
}

impl SeparatorStyle for SplitPanelSeparatorStyle {
    fn thickness(&self) -> f64 {
        if self.active {
            SPLIT_PANEL_THICKNESS_HOVER_DRAG
        } else {
            SPLIT_PANEL_THICKNESS_IDLE
        }
    }
    fn margin(&self) -> f64 {
        0.0
    }
    fn handle_hit_thickness(&self) -> f64 {
        SPLIT_PANEL_HIT_ZONE
    }
}

// =============================================================================
// Sidebar separator style
// =============================================================================

/// Sidebar separator (between chart area and right sidebar).
///
/// Visual: 1 px stroke at left edge of sidebar.
/// Hit zone: 8 px centered on the visual line.
/// No visual change on hover/drag — cursor changes only.
/// Min sidebar width: 280 px.
pub struct SidebarSeparatorStyle;

impl Default for SidebarSeparatorStyle {
    fn default() -> Self {
        Self
    }
}

impl SeparatorStyle for SidebarSeparatorStyle {
    fn thickness(&self) -> f64 {
        SIDEBAR_VISUAL_THICKNESS
    }
    fn margin(&self) -> f64 {
        0.0
    }
    fn handle_hit_thickness(&self) -> f64 {
        SIDEBAR_HIT_ZONE
    }
}

impl SidebarSeparatorStyle {
    /// Minimum sidebar width enforced during drag.
    pub fn min_size_drag(&self) -> f64 {
        SIDEBAR_MIN_WIDTH
    }
}

// =============================================================================
// Modal section divider style
// =============================================================================

/// Modal section divider (header/footer separator lines inside modals).
///
/// Visual: 1 px stroke (not fill). Non-interactive.
pub struct ModalSectionDividerStyle;

impl Default for ModalSectionDividerStyle {
    fn default() -> Self {
        Self
    }
}

impl SeparatorStyle for ModalSectionDividerStyle {
    fn thickness(&self) -> f64 {
        1.0
    }
    fn margin(&self) -> f64 {
        0.0
    }
    fn handle_hit_thickness(&self) -> f64 {
        0.0
    }
}
