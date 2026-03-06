//! Hit testing system for panel interactions
//!
//! Provides types for determining what UI element is under the cursor:
//! - Panel bodies
//! - Separators (for resize)
//! - Corners (separator intersections for bidirectional resize)
//! - Tab bars
//! - Floating window headers
//! - Floating window bodies
//! - Floating window close buttons
//!
//! # Hit Test Priority
//!
//! 1. Floating window close buttons (highest priority)
//! 2. Floating window headers
//! 3. Corners (separator intersections)
//! 4. Separators
//! 5. Tab bars
//! 6. Panel bodies
//! 7. None (lowest priority)

use crate::{LeafId, TabHit, FloatingWindowId};

// =============================================================================
// Hit Result
// =============================================================================

/// Result of hit testing - what UI element is at a given point
#[derive(Clone, Debug)]
pub enum HitResult {
    /// Hit a panel body
    Panel(LeafId),
    /// Hit a separator (by index in separator list)
    Separator(usize),
    /// Hit a corner handle (by index in corner list)
    Corner(usize),
    /// Hit a tab bar
    /// Contains: (container_id, tab_hit_detail)
    TabBar(LeafId, TabHit),
    /// Hit a floating window header (for dragging)
    FloatingHeader(FloatingWindowId),
    /// Hit a floating window body (for focus)
    FloatingBody(FloatingWindowId),
    /// Hit a floating window close button
    FloatingClose(FloatingWindowId),
    /// Hit nothing
    None,
}

impl HitResult {
    /// Check if this is a panel hit
    pub fn is_panel(&self) -> bool {
        matches!(self, HitResult::Panel(_))
    }

    /// Check if this is a separator hit
    pub fn is_separator(&self) -> bool {
        matches!(self, HitResult::Separator(_))
    }

    /// Check if this is a corner hit
    pub fn is_corner(&self) -> bool {
        matches!(self, HitResult::Corner(_))
    }

    /// Check if this is a tab bar hit
    pub fn is_tab_bar(&self) -> bool {
        matches!(self, HitResult::TabBar(_, _))
    }

    /// Check if this is a floating window hit (any part)
    pub fn is_floating(&self) -> bool {
        matches!(
            self,
            HitResult::FloatingHeader(_) | HitResult::FloatingBody(_) | HitResult::FloatingClose(_)
        )
    }

    /// Extract panel ID if this is a panel hit
    pub fn panel_id(&self) -> Option<LeafId> {
        match self {
            HitResult::Panel(id) => Some(*id),
            _ => None,
        }
    }

    /// Extract floating window ID if this is any floating window hit
    pub fn floating_id(&self) -> Option<FloatingWindowId> {
        match self {
            HitResult::FloatingHeader(id) | HitResult::FloatingBody(id) | HitResult::FloatingClose(id) => Some(*id),
            _ => None,
        }
    }
}

// =============================================================================
// Corner Handle
// =============================================================================

/// A corner where two separators intersect (for bidirectional resize)
#[derive(Clone, Debug)]
pub struct CornerHandle {
    /// Index of the vertical separator in separator list
    pub v_separator_idx: usize,
    /// Index of the horizontal separator in separator list
    pub h_separator_idx: usize,
    /// X position of corner (vertical separator's position)
    pub x: f32,
    /// Y position of corner (horizontal separator's position)
    pub y: f32,
}

impl CornerHandle {
    /// Create new corner handle
    pub fn new(v_separator_idx: usize, h_separator_idx: usize, x: f32, y: f32) -> Self {
        Self {
            v_separator_idx,
            h_separator_idx,
            x,
            y,
        }
    }

    /// Hit test this corner with given radius
    ///
    /// Uses circular hit area for better UX at separator intersections.
    ///
    /// # Arguments
    /// - `px`, `py`: Point to test
    /// - `radius`: Hit test radius in pixels (typically 8-12px)
    ///
    /// # Returns
    /// True if point is within radius of corner center
    pub fn hit_test(&self, px: f32, py: f32, radius: f32) -> bool {
        let dx = px - self.x;
        let dy = py - self.y;
        (dx * dx + dy * dy).sqrt() <= radius
    }

    /// Get rectangle for this corner (for rendering or bounding box checks)
    pub fn rect(&self, size: f32) -> crate::PanelRect {
        let half = size / 2.0;
        crate::PanelRect::new(self.x - half, self.y - half, size, size)
    }
}
