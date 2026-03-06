//! Separator - Interactive dividers between panels with hit testing
//!
//! This module provides the visual and interactive elements for panel separation:
//! - **Separator**: Draggable separators between panels with hit testing
//! - **SeparatorController**: Drag state management and constraint enforcement
//!
//! # Architecture
//!
//! Separators are the interactive dividers between panels in linear containers.
//! They support:
//! - Visual feedback (thickness changes on hover/drag)
//! - Hit testing with wider interaction area (8px hit width vs 2px visual)
//! - Constraint enforcement (minimum panel sizes)
//! - Snap-back indication when constraints violated (returns None)
//!
//! # Usage
//!
//! ```rust,ignore
//! use uzor_panels::{Separator, SeparatorOrientation, SeparatorController};
//!
//! // Create separator between two panels (child 0 and child 1)
//! let separator = Separator::new(
//!     SeparatorOrientation::Vertical,
//!     300.0,
//!     0.0,
//!     600.0,
//!     SeparatorLevel::Node {
//!         parent_id: BranchId(0),
//!         child_a: 1,
//!         child_b: 2
//!     }
//! );
//!
//! // Check if mouse is over separator
//! if separator.hit_test(mouse_x, mouse_y) {
//!     // Start drag
//!     controller.start_drag(0, container_id, 300.0, vec![1.0, 1.0]);
//! }
//!
//! // Update drag and check constraints
//! if let Some(new_shares) = controller.update_drag(delta, &children, &min_sizes, total_size) {
//!     // Apply new shares - constraints satisfied
//! } else {
//!     // Constraint violated - animate snap-back
//! }
//! ```

use crate::id::{NodeId, BranchId};

// =============================================================================
// Separator Geometry
// =============================================================================

/// Separator between children of a branch node at any depth in the panel tree
#[derive(Clone, Debug)]
pub enum SeparatorLevel {
    /// Separator between two children of the same branch node
    Node {
        parent_id: BranchId,   // the branch node containing both siblings
        child_a: u64,          // left/top child node (raw ID - could be leaf or branch)
        child_b: u64,          // right/bottom child node (raw ID - could be leaf or branch)
    },
}

/// Separator orientation (vertical = |, horizontal = —)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SeparatorOrientation {
    /// Vertical separator (between horizontal panels: | )
    Vertical,
    /// Horizontal separator (between vertical panels: — )
    Horizontal,
}

/// Separator visual/interaction state
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SeparatorState {
    /// Idle state (default thickness)
    Idle,
    /// Hover state (thicker, highlighted)
    Hover,
    /// Dragging state (thicker, highlighted)
    Dragging,
}

/// Separator between panels in linear container
///
/// Separators are draggable dividers that allow resizing adjacent panels.
/// They have:
/// - Visual thickness (2px idle, 4px hover/drag)
/// - Hit width (8px for easier grabbing)
/// - Position along axis
/// - Start position on perpendicular axis
/// - Length perpendicular to axis
pub struct Separator {
    /// Orientation (vertical or horizontal)
    pub orientation: SeparatorOrientation,
    /// Position along axis (pixels from container start)
    pub position: f32,
    /// Start position on perpendicular axis (y for vertical, x for horizontal)
    pub start: f32,
    /// Length perpendicular to axis (pixels)
    pub length: f32,
    /// Visual thickness (changes with state)
    #[allow(dead_code)]
    thickness: f32,
    /// Interaction width (always 8px for easier hit testing)
    hit_width: f32,
    /// Current state
    pub state: SeparatorState,
    /// What level this separator operates at (node level)
    pub level: SeparatorLevel,
}

impl Separator {
    /// Create new separator
    ///
    /// # Arguments
    /// - `orientation`: Vertical or horizontal
    /// - `position`: Position along axis (pixels)
    /// - `start`: Start position on perpendicular axis (y for vertical, x for horizontal)
    /// - `length`: Length perpendicular to axis (pixels)
    /// - `level`: What level this separator operates at (node)
    pub fn new(orientation: SeparatorOrientation, position: f32, start: f32, length: f32, level: SeparatorLevel) -> Self {
        Self {
            orientation,
            position,
            start,
            length,
            thickness: 2.0,
            hit_width: 8.0,
            state: SeparatorState::Idle,
            level,
        }
    }

    /// Get child_a (for backward compatibility with corner drag code)
    pub fn child_a(&self) -> Option<u64> {
        match &self.level {
            SeparatorLevel::Node { child_a, .. } => Some(*child_a),
        }
    }

    /// Get child_b (for backward compatibility with corner drag code)
    pub fn child_b(&self) -> Option<u64> {
        match &self.level {
            SeparatorLevel::Node { child_b, .. } => Some(*child_b),
        }
    }

    /// Hit test - check if point is over separator
    ///
    /// Uses wider hit_width (8px) for easier interaction.
    ///
    /// # Arguments
    /// - `x`, `y`: Point to test (relative to separator's container)
    ///
    /// # Returns
    /// `true` if point is within hit area
    pub fn hit_test(&self, x: f32, y: f32) -> bool {
        match self.orientation {
            SeparatorOrientation::Vertical => {
                // Check X position (with hit_width padding)
                let min_x = self.position - self.hit_width / 2.0;
                let max_x = self.position + self.hit_width / 2.0;
                x >= min_x && x <= max_x && y >= self.start && y <= self.start + self.length
            }
            SeparatorOrientation::Horizontal => {
                // Check Y position
                let min_y = self.position - self.hit_width / 2.0;
                let max_y = self.position + self.hit_width / 2.0;
                y >= min_y && y <= max_y && x >= self.start && x <= self.start + self.length
            }
        }
    }

    /// Get visual thickness based on current state
    ///
    /// - Idle: 2px
    /// - Hover: 4px
    /// - Dragging: 4px
    pub fn thickness_for_state(&self) -> f32 {
        match self.state {
            SeparatorState::Idle => 2.0,
            SeparatorState::Hover | SeparatorState::Dragging => 4.0,
        }
    }
}

// =============================================================================
// Separator Controller
// =============================================================================

/// Drag state for separator resizing
#[derive(Clone, Debug)]
pub struct SeparatorDragState {
    /// Index of separator being dragged
    separator_idx: usize,
    /// Container owning the separator
    #[allow(dead_code)]
    container_id: NodeId,
    /// Starting position of separator
    #[allow(dead_code)]
    start_pos: f32,
    /// Original shares of all children
    start_shares: Vec<f32>,
}

/// Separator drag controller
///
/// Manages separator dragging with constraint enforcement:
/// 1. Start drag: capture initial state
/// 2. Update drag: calculate new shares, check constraints
/// 3. End drag: finalize or cancel
///
/// When constraints are violated (panel below min_size), update_drag returns None.
/// The caller should then trigger a snap-back animation.
pub struct SeparatorController {
    /// Current drag state (if dragging)
    dragging: Option<SeparatorDragState>,
}

impl SeparatorController {
    /// Create new separator controller
    pub fn new() -> Self {
        Self { dragging: None }
    }

    /// Start drag operation
    ///
    /// # Arguments
    /// - `separator_idx`: Index of separator (between children[idx] and children[idx+1])
    /// - `container_id`: Container owning the separator
    /// - `pos`: Starting position of separator
    /// - `shares`: Current shares of all children
    pub fn start_drag(
        &mut self,
        separator_idx: usize,
        container_id: NodeId,
        pos: f32,
        shares: Vec<f32>,
    ) {
        self.dragging = Some(SeparatorDragState {
            separator_idx,
            container_id,
            start_pos: pos,
            start_shares: shares,
        });
    }

    /// Update drag and check constraints
    ///
    /// # Arguments
    /// - `delta`: Mouse movement delta along axis (pixels)
    /// - `children`: Child panel IDs
    /// - `min_sizes`: Minimum size for each child (pixels)
    /// - `total_size`: Total available size (pixels)
    ///
    /// # Returns
    /// - `Some(new_shares)`: New shares if constraints satisfied
    /// - `None`: Constraints violated (caller should animate snap-back)
    pub fn update_drag(
        &self,
        delta: f32,
        children: &[NodeId],
        min_sizes: &[f32],
        total_size: f32,
    ) -> Option<Vec<f32>> {
        let drag = self.dragging.as_ref()?;

        if children.len() < 2 || min_sizes.len() != children.len() {
            return None;
        }

        // Calculate new shares based on delta
        let mut new_shares = drag.start_shares.clone();
        let idx = drag.separator_idx;

        if idx >= children.len() - 1 {
            return None; // Invalid separator index
        }

        // Calculate total shares and convert delta to share delta
        let total_shares: f32 = new_shares.iter().sum();
        if total_shares <= 0.0 {
            return None;
        }

        // Convert pixel delta to share delta
        let delta_ratio = delta / total_size;
        let share_delta = delta_ratio * total_shares;

        // Adjust shares of adjacent children
        // Left/top child gets +delta, right/bottom child gets -delta
        new_shares[idx] += share_delta;
        new_shares[idx + 1] -= share_delta;

        // Enforce minimum sizes
        for (i, &share) in new_shares.iter().enumerate() {
            if share < 0.0 {
                // Negative share = constraint violated
                return None;
            }

            let pixel_size = (share / total_shares) * total_size;
            if pixel_size < min_sizes[i] {
                // Constraint violated - below minimum size
                return None;
            }
        }

        // Normalize shares (ensure they sum to original total)
        let current_sum: f32 = new_shares.iter().sum();
        if current_sum <= 0.0 {
            return None;
        }

        let scale = total_shares / current_sum;
        for share in &mut new_shares {
            *share *= scale;
        }

        Some(new_shares)
    }

    /// End drag operation
    pub fn end_drag(&mut self) {
        self.dragging = None;
    }

    /// Check if currently dragging
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    /// Get current drag state (if dragging)
    pub fn drag_state(&self) -> Option<&SeparatorDragState> {
        self.dragging.as_ref()
    }
}

impl Default for SeparatorController {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_separator_hit_test_vertical() {
        let separator = Separator::new(
            SeparatorOrientation::Vertical, 100.0, 0.0, 200.0,
            SeparatorLevel::Node { parent_id: BranchId(0), child_a: 0, child_b: 1 }
        );

        // Hit center
        assert!(separator.hit_test(100.0, 100.0));

        // Hit edges (within 8px hit width)
        assert!(separator.hit_test(96.0, 100.0)); // 4px left
        assert!(separator.hit_test(104.0, 100.0)); // 4px right

        // Miss horizontally
        assert!(!separator.hit_test(90.0, 100.0)); // Too far left
        assert!(!separator.hit_test(110.0, 100.0)); // Too far right

        // Miss vertically
        assert!(!separator.hit_test(100.0, -10.0)); // Above
        assert!(!separator.hit_test(100.0, 210.0)); // Below
    }

    #[test]
    fn test_separator_hit_test_horizontal() {
        let separator = Separator::new(
            SeparatorOrientation::Horizontal, 100.0, 0.0, 200.0,
            SeparatorLevel::Node { parent_id: BranchId(0), child_a: 0, child_b: 1 }
        );

        // Hit center
        assert!(separator.hit_test(100.0, 100.0));

        // Hit edges (within 8px hit width)
        assert!(separator.hit_test(100.0, 96.0)); // 4px above
        assert!(separator.hit_test(100.0, 104.0)); // 4px below

        // Miss vertically
        assert!(!separator.hit_test(100.0, 90.0)); // Too far above
        assert!(!separator.hit_test(100.0, 110.0)); // Too far below

        // Miss horizontally
        assert!(!separator.hit_test(-10.0, 100.0)); // Left
        assert!(!separator.hit_test(210.0, 100.0)); // Right
    }

    #[test]
    fn test_separator_thickness() {
        let mut separator = Separator::new(
            SeparatorOrientation::Vertical, 100.0, 0.0, 200.0,
            SeparatorLevel::Node { parent_id: BranchId(0), child_a: 0, child_b: 1 }
        );

        separator.state = SeparatorState::Idle;
        assert_eq!(separator.thickness_for_state(), 2.0);

        separator.state = SeparatorState::Hover;
        assert_eq!(separator.thickness_for_state(), 4.0);

        separator.state = SeparatorState::Dragging;
        assert_eq!(separator.thickness_for_state(), 4.0);
    }

    #[test]
    fn test_controller_normal_resize() {
        let mut controller = SeparatorController::new();
        let children = vec![NodeId(1), NodeId(2)];
        let min_sizes = vec![100.0, 100.0];

        // Start drag with equal shares (200px each in 400px total)
        controller.start_drag(0, NodeId(10), 200.0, vec![1.0, 1.0]);

        // Move separator right by 50px (left panel grows to 250px, right shrinks to 150px)
        let new_shares = controller.update_drag(50.0, &children, &min_sizes, 400.0);
        assert!(new_shares.is_some());

        let shares = new_shares.unwrap();
        assert_eq!(shares.len(), 2);

        // Verify shares maintain ratio
        let total: f32 = shares.iter().sum();
        assert!((total - 2.0).abs() < 0.01); // Should sum to original total (2.0)

        // Verify pixel sizes
        let left_size = (shares[0] / total) * 400.0;
        let right_size = (shares[1] / total) * 400.0;

        assert!((left_size - 250.0).abs() < 1.0);
        assert!((right_size - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_controller_constraint_violation() {
        let mut controller = SeparatorController::new();
        let children = vec![NodeId(1), NodeId(2)];
        let min_sizes = vec![100.0, 100.0];

        // Start drag with equal shares (200px each in 400px total)
        controller.start_drag(0, NodeId(10), 200.0, vec![1.0, 1.0]);

        // Try to move separator right by 150px (right would be 50px < min 100px)
        let new_shares = controller.update_drag(150.0, &children, &min_sizes, 400.0);

        // Should return None (constraint violated)
        assert!(new_shares.is_none());
    }

    #[test]
    fn test_controller_drag_left() {
        let mut controller = SeparatorController::new();
        let children = vec![NodeId(1), NodeId(2)];
        let min_sizes = vec![100.0, 100.0];

        // Start drag (200px each in 400px total)
        controller.start_drag(0, NodeId(10), 200.0, vec![1.0, 1.0]);

        // Move separator left by 50px (left shrinks to 150px, right grows to 250px)
        let new_shares = controller.update_drag(-50.0, &children, &min_sizes, 400.0);
        assert!(new_shares.is_some());

        let shares = new_shares.unwrap();
        let total: f32 = shares.iter().sum();

        let left_size = (shares[0] / total) * 400.0;
        let right_size = (shares[1] / total) * 400.0;

        assert!((left_size - 150.0).abs() < 1.0);
        assert!((right_size - 250.0).abs() < 1.0);
    }

    #[test]
    fn test_controller_three_panels() {
        let mut controller = SeparatorController::new();
        let children = vec![NodeId(1), NodeId(2), NodeId(3)];
        let min_sizes = vec![100.0, 100.0, 100.0];

        // Start drag with equal shares (200px each in 600px total)
        controller.start_drag(0, NodeId(10), 200.0, vec![1.0, 1.0, 1.0]);

        // Move first separator right by 50px
        // Left grows from 200 to 250, middle shrinks from 200 to 150, right unchanged at 200
        let new_shares = controller.update_drag(50.0, &children, &min_sizes, 600.0);
        assert!(new_shares.is_some());

        let shares = new_shares.unwrap();
        let total: f32 = shares.iter().sum();

        let sizes: Vec<f32> = shares.iter().map(|&s| (s / total) * 600.0).collect();

        assert!((sizes[0] - 250.0).abs() < 1.0); // Left panel
        assert!((sizes[1] - 150.0).abs() < 1.0); // Middle panel
        assert!((sizes[2] - 200.0).abs() < 1.0); // Right panel (unchanged)
    }

    #[test]
    fn test_controller_drag_lifecycle() {
        let mut controller = SeparatorController::new();

        assert!(!controller.is_dragging());

        controller.start_drag(0, NodeId(10), 200.0, vec![1.0, 1.0]);
        assert!(controller.is_dragging());

        controller.end_drag();
        assert!(!controller.is_dragging());
    }
}
