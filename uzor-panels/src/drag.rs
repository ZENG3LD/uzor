//! Drag-and-drop state machine for panel system
//!
//! Implements the complete drag-and-drop lifecycle:
//! 1. `start_drag()` - Initiate drag on mouse down + threshold
//! 2. `update_hover()` - Update target as mouse moves
//! 3. `complete_drop()` - Finalize on mouse up
//! 4. `cancel_drag()` - Cancel on ESC or invalid drop
//!
//! # Lock System
//!
//! Uses egui_dock-inspired lock system to prevent rapid changes:
//! - **Unlocked**: Changes allowed
//! - **SoftLock**: Releasable after 200ms timeout
//! - **HardLock**: Absolute lock during critical operations
//!
//! # Example
//!
//! ```
//! use uzor_panels::{DragDropState, LeafId, PanelRect, DropZone};
//!
//! let mut state = DragDropState::new();
//!
//! // Start drag
//! let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);
//! state.start_drag(LeafId(1), (10.0, 10.0), rect);
//!
//! // Update hover target
//! let preview = PanelRect::new(100.0, 0.0, 50.0, 100.0);
//! state.update_hover(LeafId(2), DropZone::Right, preview);
//!
//! // Complete drop
//! if let Some((source, target, zone)) = state.complete_drop() {
//!     println!("Drop panel {:?} onto {:?} at {:?}", source, target, zone);
//! }
//! ```

use std::time::{Duration, Instant};
use crate::{LeafId, PanelRect, DropZone};

// =============================================================================
// Drag State Types
// =============================================================================

/// Source of drag operation
#[derive(Clone, Debug)]
pub struct DragSource {
    /// Panel being dragged
    pub panel_id: LeafId,
    /// Original rectangle
    pub source_rect: PanelRect,
    /// Mouse offset from panel origin (for preview positioning)
    pub offset: (f32, f32),
}

/// Target of hover during drag
#[derive(Clone, Debug)]
pub struct HoverTarget {
    /// Container being hovered
    pub container_id: LeafId,
    /// Drop zone within container
    pub zone: DropZone,
    /// Preview rectangle (where panel will be placed)
    pub preview_rect: PanelRect,
}

/// Lock state to prevent rapid changes during drag
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LockState {
    /// No lock - changes allowed
    Unlocked,
    /// Soft lock - releasable after timeout (200ms)
    SoftLock,
    /// Hard lock - absolute lock during critical operations
    HardLock,
}

// =============================================================================
// Drag-and-Drop State Machine
// =============================================================================

/// Main drag-and-drop state machine
///
/// Manages the complete drag-and-drop lifecycle:
/// 1. `start_drag()` - Initiate drag on mouse down + threshold
/// 2. `update_hover()` - Update target as mouse moves
/// 3. `complete_drop()` - Finalize on mouse up
/// 4. `cancel_drag()` - Cancel on ESC or invalid drop
pub struct DragDropState {
    drag_source: Option<DragSource>,
    hover_target: Option<HoverTarget>,
    lock: LockState,
    lock_time: Option<Instant>,
}

impl DragDropState {
    /// Create new drag-and-drop state
    pub fn new() -> Self {
        Self {
            drag_source: None,
            hover_target: None,
            lock: LockState::Unlocked,
            lock_time: None,
        }
    }

    /// Start drag operation
    ///
    /// # Arguments
    /// - `panel_id`: Panel being dragged
    /// - `mouse_pos`: Current mouse position (absolute)
    /// - `rect`: Panel rectangle
    pub fn start_drag(&mut self, panel_id: LeafId, mouse_pos: (f32, f32), rect: PanelRect) {
        self.drag_source = Some(DragSource {
            panel_id,
            source_rect: rect,
            offset: (mouse_pos.0 - rect.x, mouse_pos.1 - rect.y),
        });
    }

    /// Update hover target during drag
    ///
    /// Respects lock state - will not update if locked.
    pub fn update_hover(&mut self, container_id: LeafId, zone: DropZone, preview: PanelRect) {
        // Check lock
        if self.is_locked() {
            return;
        }

        self.hover_target = Some(HoverTarget {
            container_id,
            zone,
            preview_rect: preview,
        });
    }

    /// Clear hover target (mouse left container)
    pub fn clear_hover(&mut self) {
        self.hover_target = None;
    }

    /// Complete drop operation on mouse up
    ///
    /// Returns (source_panel_id, target_container_id, drop_zone) if successful.
    /// Sets soft lock to prevent rapid re-drops.
    pub fn complete_drop(&mut self) -> Option<(LeafId, LeafId, DropZone)> {
        if self.is_locked() {
            return None;
        }

        let source = self.drag_source.take()?;
        let target = self.hover_target.take()?;

        // Set soft lock (prevent rapid re-drops)
        self.lock = LockState::SoftLock;
        self.lock_time = Some(Instant::now());

        Some((source.panel_id, target.container_id, target.zone))
    }

    /// Cancel drag operation (ESC or invalid drop)
    pub fn cancel_drag(&mut self) {
        self.drag_source = None;
        self.hover_target = None;
        self.lock = LockState::Unlocked;
        self.lock_time = None;
    }

    /// Check if drag is active
    pub fn is_dragging(&self) -> bool {
        self.drag_source.is_some()
    }

    /// Check if locked (any lock state except Unlocked)
    pub fn is_locked(&self) -> bool {
        match self.lock {
            LockState::Unlocked => false,
            LockState::HardLock => true,
            LockState::SoftLock => {
                // Check timeout (200ms)
                if let Some(lock_time) = self.lock_time {
                    lock_time.elapsed() < Duration::from_millis(200)
                } else {
                    false
                }
            }
        }
    }

    /// Update lock state (call every frame)
    ///
    /// Releases soft lock after timeout expires.
    pub fn update(&mut self) {
        if self.lock == LockState::SoftLock {
            if let Some(lock_time) = self.lock_time {
                if lock_time.elapsed() >= Duration::from_millis(200) {
                    self.lock = LockState::Unlocked;
                    self.lock_time = None;
                }
            }
        }
    }

    /// Get current drag source (if dragging)
    pub fn drag_source(&self) -> Option<&DragSource> {
        self.drag_source.as_ref()
    }

    /// Get current hover target (if hovering)
    pub fn hover_target(&self) -> Option<&HoverTarget> {
        self.hover_target.as_ref()
    }

    /// Set hard lock (for critical operations)
    pub fn set_hard_lock(&mut self) {
        self.lock = LockState::HardLock;
        self.lock_time = Some(Instant::now());
    }

    /// Release lock (force unlock)
    pub fn unlock(&mut self) {
        self.lock = LockState::Unlocked;
        self.lock_time = None;
    }
}

impl Default for DragDropState {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Panel Drag State (application-level)
// =============================================================================

/// Drag state for panel being dragged by header
///
/// This is a higher-level state used by the application layer to track
/// panel drag operations. It's separate from DragDropState which handles
/// the core state machine.
#[derive(Clone, Debug)]
pub struct PanelDragState {
    /// Panel being dragged
    pub dragged_leaf_id: LeafId,
    /// Current mouse position (panel-local coords)
    pub current_x: f32,
    pub current_y: f32,
    /// Target panel under cursor (if any)
    pub target_leaf_id: Option<LeafId>,
    /// Current drop zone
    pub drop_zone: Option<DropZone>,
    /// True if this is a window-level edge drop (vs leaf-level)
    pub is_window_edge: bool,
}

impl PanelDragState {
    /// Create new panel drag state
    pub fn new(dragged_leaf_id: LeafId, x: f32, y: f32) -> Self {
        Self {
            dragged_leaf_id,
            current_x: x,
            current_y: y,
            target_leaf_id: None,
            drop_zone: None,
            is_window_edge: false,
        }
    }

    /// Update mouse position
    pub fn update_position(&mut self, x: f32, y: f32) {
        self.current_x = x;
        self.current_y = y;
    }

    /// Update target and drop zone
    pub fn update_target(&mut self, target: LeafId, zone: DropZone, is_window_edge: bool) {
        self.target_leaf_id = Some(target);
        self.drop_zone = Some(zone);
        self.is_window_edge = is_window_edge;
    }

    /// Clear target (mouse left valid drop area)
    pub fn clear_target(&mut self) {
        self.target_leaf_id = None;
        self.drop_zone = None;
        self.is_window_edge = false;
    }

    /// Check if there's a valid drop target
    pub fn has_target(&self) -> bool {
        self.target_leaf_id.is_some() && self.drop_zone.is_some()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drag_drop_lifecycle() {
        let mut state = DragDropState::new();

        assert!(!state.is_dragging());

        // Start drag
        let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);
        state.start_drag(LeafId(1), (10.0, 10.0), rect);
        assert!(state.is_dragging());

        // Update hover
        let preview = PanelRect::new(100.0, 0.0, 50.0, 100.0);
        state.update_hover(LeafId(2), DropZone::Right, preview);
        assert!(state.hover_target().is_some());

        // Complete drop
        let result = state.complete_drop();
        assert!(result.is_some());
        assert!(!state.is_dragging());
        assert!(state.is_locked()); // Soft lock after drop
    }

    #[test]
    fn test_lock_prevents_hover_update() {
        let mut state = DragDropState::new();

        // Start drag and complete to enter soft lock
        let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);
        state.start_drag(LeafId(1), (10.0, 10.0), rect);
        state.update_hover(LeafId(2), DropZone::Center, rect);
        state.complete_drop();

        // Try to start new drag while locked
        assert!(state.is_locked());
        state.start_drag(LeafId(3), (20.0, 20.0), rect);
        state.update_hover(LeafId(4), DropZone::Left, rect);

        // Hover should not update due to lock
        // But drag source should exist (start_drag doesn't check lock)
        assert!(state.drag_source().is_some());
    }

    #[test]
    fn test_cancel_drag() {
        let mut state = DragDropState::new();
        let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);

        state.start_drag(LeafId(1), (10.0, 10.0), rect);
        assert!(state.is_dragging());

        state.cancel_drag();
        assert!(!state.is_dragging());
        assert!(!state.is_locked());
    }

    #[test]
    fn test_panel_drag_state() {
        let mut state = PanelDragState::new(LeafId(1), 10.0, 20.0);

        assert_eq!(state.dragged_leaf_id, LeafId(1));
        assert_eq!(state.current_x, 10.0);
        assert_eq!(state.current_y, 20.0);
        assert!(!state.has_target());

        // Update position
        state.update_position(30.0, 40.0);
        assert_eq!(state.current_x, 30.0);
        assert_eq!(state.current_y, 40.0);

        // Update target
        state.update_target(LeafId(2), DropZone::Center, false);
        assert!(state.has_target());
        assert_eq!(state.target_leaf_id, Some(LeafId(2)));
        assert_eq!(state.drop_zone, Some(DropZone::Center));
        assert!(!state.is_window_edge);

        // Clear target
        state.clear_target();
        assert!(!state.has_target());
        assert_eq!(state.target_leaf_id, None);
        assert_eq!(state.drop_zone, None);
    }

    #[test]
    fn test_hard_lock() {
        let mut state = DragDropState::new();

        state.set_hard_lock();
        assert!(state.is_locked());
        assert_eq!(state.lock, LockState::HardLock);

        // Hard lock doesn't release on update
        state.update();
        assert!(state.is_locked());

        // Must explicitly unlock
        state.unlock();
        assert!(!state.is_locked());
    }

    #[test]
    fn test_soft_lock_timeout() {
        let mut state = DragDropState::new();
        let rect = PanelRect::new(0.0, 0.0, 100.0, 100.0);

        // Complete drop to enter soft lock
        state.start_drag(LeafId(1), (10.0, 10.0), rect);
        state.update_hover(LeafId(2), DropZone::Center, rect);
        state.complete_drop();

        assert!(state.is_locked());
        assert_eq!(state.lock, LockState::SoftLock);

        // Soft lock should still be active immediately
        state.update();
        assert!(state.is_locked());

        // Note: Can't test timeout in unit test without sleeping
        // In real usage, after 200ms the lock would release
    }
}
