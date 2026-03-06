//! Floating window types for panels extracted from the docking tree
//!
//! Floating windows hover above the main layout and can be:
//! - Repositioned via drag (header drag)
//! - Docked back into tree (via drop zones)
//! - Closed (destroys window and its panels)
//!
//! # Architecture
//!
//! Each floating window contains a simplified version of a leaf:
//! - A vector of panels (tabs)
//! - An active tab index
//! - Position and size
//!
//! When grid.rs is implemented with `Leaf<P>`, we can refactor to use that type.

use crate::{DockPanel, LeafId, PanelRect, DropZone};
use serde::{Serialize, Deserialize};

// =============================================================================
// Floating Window ID
// =============================================================================

/// Unique identifier for floating windows
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FloatingWindowId(pub u64);

impl std::fmt::Display for FloatingWindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FloatingWindow({})", self.0)
    }
}

// =============================================================================
// Floating Window
// =============================================================================

/// A panel container extracted from the docking tree that floats above the layout
#[derive(Clone, Debug)]
pub struct FloatingWindow<P: DockPanel> {
    /// Unique ID
    pub id: FloatingWindowId,
    /// Panels (tabs) in this floating window
    pub panels: Vec<P>,
    /// Index of active tab
    pub active_tab: usize,
    /// X position (panel-local coords)
    pub x: f32,
    /// Y position (panel-local coords)
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
}

impl<P: DockPanel> FloatingWindow<P> {
    /// Create new floating window
    pub fn new(
        id: FloatingWindowId,
        panels: Vec<P>,
        active_tab: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            id,
            panels,
            active_tab,
            x,
            y,
            width,
            height,
        }
    }

    /// Get rectangle for this floating window
    pub fn rect(&self) -> PanelRect {
        PanelRect::new(self.x, self.y, self.width, self.height)
    }

    /// Get title from active panel
    pub fn title(&self) -> &str {
        self.panels
            .get(self.active_tab)
            .map(|p| p.title())
            .unwrap_or("Floating Window")
    }

    /// Check if point is inside this window
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width
            && y >= self.y && y <= self.y + self.height
    }

    /// Get active panel
    pub fn active_panel(&self) -> Option<&P> {
        self.panels.get(self.active_tab)
    }

    /// Get mutable active panel
    pub fn active_panel_mut(&mut self) -> Option<&mut P> {
        self.panels.get_mut(self.active_tab)
    }

    /// Number of tabs
    pub fn tab_count(&self) -> usize {
        self.panels.len()
    }
}

// =============================================================================
// Floating Drag State
// =============================================================================

/// Drag state for repositioning a floating window
#[derive(Clone, Debug)]
pub struct FloatingDragState {
    /// Which floating window is being dragged
    pub window_id: FloatingWindowId,
    /// Cursor offset from window origin (for smooth dragging)
    pub offset_x: f32,
    pub offset_y: f32,
    /// Potential dock target during drag (None = just repositioning)
    /// Tuple: (target_leaf, drop_zone, is_window_edge)
    pub dock_target: Option<(LeafId, DropZone, bool)>,
}

impl FloatingDragState {
    /// Create new floating drag state
    pub fn new(window_id: FloatingWindowId, offset_x: f32, offset_y: f32) -> Self {
        Self {
            window_id,
            offset_x,
            offset_y,
            dock_target: None,
        }
    }

    /// Check if a dock target is set
    pub fn has_dock_target(&self) -> bool {
        self.dock_target.is_some()
    }

    /// Get dock target details
    pub fn dock_target(&self) -> Option<(LeafId, DropZone, bool)> {
        self.dock_target
    }

    /// Set dock target
    pub fn set_dock_target(&mut self, target: Option<(LeafId, DropZone, bool)>) {
        self.dock_target = target;
    }
}
