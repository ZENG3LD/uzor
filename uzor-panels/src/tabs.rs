//! Tab system for panel containers
//!
//! This module provides tab-related types for managing tabbed panel containers:
//! - **TabInfo**: Individual tab metadata (panel_id, title, closable, rect)
//! - **TabHit**: Hit test result (Tab or Close button)
//! - **TabBar**: Tab bar controller with layout and hit testing
//! - **TabDragState**: Tab drag state
//! - **TabDragController**: Tab drag controller with start/update/complete/cancel
//! - **TabBarInfo**: Container-level tab bar info for rendering
//! - **TabItem**: Individual tab item for rendering
//! - **TabReorderState**: Tab reorder drag state

use crate::id::LeafId;
use crate::rect::PanelRect;
use crate::drop_zone::DropZone;

// =============================================================================
// Tab Bar Types (for TabBar controller)
// =============================================================================

/// Information about a single tab
#[derive(Clone, Debug)]
pub struct TabInfo {
    /// Panel ID this tab represents
    pub panel_id: LeafId,
    /// Tab title
    pub title: String,
    /// Whether this tab can be closed
    pub closable: bool,
    /// Cached position after layout
    pub rect: PanelRect,
}

impl TabInfo {
    /// Create new tab info
    pub fn new(panel_id: LeafId, title: String, closable: bool) -> Self {
        Self {
            panel_id,
            title,
            closable,
            rect: PanelRect::zero(),
        }
    }
}

/// Tab bar (shows all tabs in a container)
pub struct TabBar {
    tabs: Vec<TabInfo>,
    active_idx: Option<usize>,
    height: f32,
    scroll_offset: f32,
}

impl TabBar {
    // Layout constants
    const TAB_PADDING: f32 = 12.0;
    const TAB_MIN_WIDTH: f32 = 80.0;
    const TAB_MAX_WIDTH: f32 = 200.0;
    const TAB_SPACING: f32 = 2.0;

    /// Create new tab bar with specified height
    pub fn new(height: f32) -> Self {
        Self {
            tabs: Vec::new(),
            active_idx: None,
            height,
            scroll_offset: 0.0,
        }
    }

    /// Set tabs from a list of (panel_id, title, closable) tuples
    ///
    /// This is a simplified version that doesn't need tree reference.
    /// The caller is responsible for extracting titles from panels.
    pub fn set_tabs(&mut self, tabs: Vec<(LeafId, String, bool)>) {
        self.tabs = tabs
            .into_iter()
            .map(|(panel_id, title, closable)| TabInfo::new(panel_id, title, closable))
            .collect();
    }

    /// Set active tab by panel ID
    pub fn set_active(&mut self, panel_id: Option<LeafId>) {
        self.active_idx = panel_id.and_then(|id| {
            self.tabs.iter().position(|tab| tab.panel_id == id)
        });
    }

    /// Layout tabs - compute rectangle positions
    ///
    /// Uses simple text width estimation: `title.len() * 8.0`
    /// (Rendering will measure properly later)
    pub fn layout(&mut self, area: PanelRect) {
        let mut x = area.x - self.scroll_offset;
        let y = area.y;

        for tab in &mut self.tabs {
            // Estimate text width (8px per character)
            let text_width = tab.title.len() as f32 * 8.0;
            let width = (text_width + Self::TAB_PADDING * 2.0)
                .clamp(Self::TAB_MIN_WIDTH, Self::TAB_MAX_WIDTH);

            tab.rect = PanelRect::new(x, y, width, self.height);

            x += width + Self::TAB_SPACING;
        }
    }

    /// Hit test - determine what was clicked
    pub fn hit_test(&self, x: f32, y: f32) -> Option<TabHit> {
        for (idx, tab) in self.tabs.iter().enumerate() {
            if tab.rect.contains(x, y) {
                // Check if close button clicked
                if tab.closable {
                    let close_size = 16.0;
                    let close_x = tab.rect.x + tab.rect.width - close_size - 8.0;
                    let close_y = tab.rect.y + (tab.rect.height - close_size) / 2.0;
                    let close_rect = PanelRect::new(close_x, close_y, close_size, close_size);

                    if close_rect.contains(x, y) {
                        return Some(TabHit::Close(idx));
                    }
                }
                return Some(TabHit::Tab(idx));
            }
        }
        None
    }

    /// Get tab by index
    pub fn get_tab(&self, idx: usize) -> Option<&TabInfo> {
        self.tabs.get(idx)
    }

    /// Get active tab index
    pub fn active_idx(&self) -> Option<usize> {
        self.active_idx
    }

    /// Get all tabs
    pub fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }

    /// Get tab bar height
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Get scroll offset
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Set scroll offset
    pub fn set_scroll_offset(&mut self, offset: f32) {
        self.scroll_offset = offset.max(0.0);
    }
}

/// Result of tab hit test
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum TabHit {
    /// Clicked on tab (for selection/drag)
    Tab(usize),
    /// Clicked on close button
    Close(usize),
}

/// State for tab drag operation
#[derive(Clone, Debug)]
pub struct TabDragState {
    /// Index of tab being dragged
    pub tab_idx: usize,
    /// Source container ID
    pub source_container: LeafId,
    /// Mouse offset from tab origin
    pub offset: (f32, f32),
    /// Preview rectangle (follows mouse)
    pub preview_rect: PanelRect,
}

/// Tab drag controller (handles tab reordering and splitting)
pub struct TabDragController {
    dragging: Option<TabDragState>,
}

impl TabDragController {
    /// Create new tab drag controller
    pub fn new() -> Self {
        Self { dragging: None }
    }

    /// Start drag operation
    ///
    /// # Arguments
    /// - `tab_idx`: Index of tab being dragged
    /// - `container_id`: Source container ID
    /// - `mouse_pos`: Current mouse position (absolute)
    /// - `tab_rect`: Tab rectangle
    pub fn start_drag(
        &mut self,
        tab_idx: usize,
        container_id: LeafId,
        mouse_pos: (f32, f32),
        tab_rect: PanelRect,
    ) {
        self.dragging = Some(TabDragState {
            tab_idx,
            source_container: container_id,
            offset: (mouse_pos.0 - tab_rect.x, mouse_pos.1 - tab_rect.y),
            preview_rect: tab_rect,
        });
    }

    /// Update drag position (on mouse move)
    pub fn update_drag(&mut self, mouse_pos: (f32, f32)) {
        if let Some(ref mut state) = self.dragging {
            state.preview_rect.x = mouse_pos.0 - state.offset.0;
            state.preview_rect.y = mouse_pos.1 - state.offset.1;
        }
    }

    /// Complete drag operation (on mouse up)
    ///
    /// Returns (source_container, tab_idx, target_container, drop_zone)
    pub fn complete_drag(
        &mut self,
        target_container: LeafId,
        zone: DropZone,
    ) -> Option<(LeafId, usize, LeafId, DropZone)> {
        let state = self.dragging.take()?;
        Some((
            state.source_container,
            state.tab_idx,
            target_container,
            zone,
        ))
    }

    /// Cancel drag operation
    pub fn cancel(&mut self) {
        self.dragging = None;
    }

    /// Check if drag is active
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    /// Get current drag state (if dragging)
    pub fn drag_state(&self) -> Option<&TabDragState> {
        self.dragging.as_ref()
    }
}

impl Default for TabDragController {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tab Bar Rendering Types (from panel_manager)
// =============================================================================

/// Tab bar info for rendering
#[derive(Clone, Debug)]
pub struct TabBarInfo {
    /// Container ID that owns these tabs
    pub container_id: LeafId,
    /// Position and size of the tab bar
    pub rect: PanelRect,
    /// Individual tab items
    pub tabs: Vec<TabItem>,
}

/// Individual tab item for rendering
#[derive(Clone, Debug)]
pub struct TabItem {
    /// Panel ID this tab represents
    pub panel_id: LeafId,
    /// Tab title
    pub title: String,
    /// Tab rect (position within tab bar)
    pub rect: PanelRect,
    /// Whether this tab is active
    pub is_active: bool,
    /// Close button rect (right side of tab)
    pub close_rect: PanelRect,
}

/// Tab reorder state (for dragging tabs within a container)
#[derive(Clone, Debug)]
pub struct TabReorderState {
    /// Container owning the tab being reordered
    pub container_id: LeafId,
    /// ID of tab being dragged
    pub dragged_tab_id: LeafId,
    /// Original index before drag
    pub original_index: usize,
    /// Current X position during drag
    pub current_x: f32,
    /// Computed insert index based on drag position
    pub insert_index: usize,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_bar_layout() {
        let mut tab_bar = TabBar::new(32.0);

        // Add some tabs
        tab_bar.set_tabs(vec![
            (LeafId(1), "Chart".to_string(), true),
            (LeafId(2), "Table".to_string(), true),
            (LeafId(3), "Long Title Here".to_string(), false),
        ]);

        // Layout in area
        let area = PanelRect::new(0.0, 0.0, 800.0, 32.0);
        tab_bar.layout(area);

        // Check that tabs have positions
        let tabs = tab_bar.tabs();
        assert_eq!(tabs.len(), 3);

        // First tab should start at x=0
        assert_eq!(tabs[0].rect.x, 0.0);
        assert_eq!(tabs[0].rect.y, 0.0);
        assert_eq!(tabs[0].rect.height, 32.0);

        // Width should be between min and max
        assert!(tabs[0].rect.width >= TabBar::TAB_MIN_WIDTH);
        assert!(tabs[0].rect.width <= TabBar::TAB_MAX_WIDTH);

        // Second tab should be offset from first
        assert!(tabs[1].rect.x > tabs[0].rect.x);
    }

    #[test]
    fn test_tab_bar_hit_test_tab() {
        let mut tab_bar = TabBar::new(32.0);
        tab_bar.set_tabs(vec![
            (LeafId(1), "Chart".to_string(), true),
            (LeafId(2), "Table".to_string(), true),
        ]);

        let area = PanelRect::new(0.0, 0.0, 800.0, 32.0);
        tab_bar.layout(area);

        // Hit test in first tab (center of tab, away from close button)
        let first_tab_rect = tab_bar.tabs()[0].rect;
        let hit = tab_bar.hit_test(first_tab_rect.x + 20.0, first_tab_rect.y + 16.0);
        assert_eq!(hit, Some(TabHit::Tab(0)));

        // Hit test outside tabs
        let hit = tab_bar.hit_test(1000.0, 16.0);
        assert_eq!(hit, None);
    }

    #[test]
    fn test_tab_bar_hit_test_close() {
        let mut tab_bar = TabBar::new(32.0);
        tab_bar.set_tabs(vec![
            (LeafId(1), "Chart".to_string(), true),
        ]);

        let area = PanelRect::new(0.0, 0.0, 800.0, 32.0);
        tab_bar.layout(area);

        // Hit test on close button (right side of tab)
        let tab_rect = tab_bar.tabs()[0].rect;
        let close_x = tab_rect.x + tab_rect.width - 16.0;
        let close_y = tab_rect.y + tab_rect.height / 2.0;

        let hit = tab_bar.hit_test(close_x, close_y);
        assert_eq!(hit, Some(TabHit::Close(0)));
    }

    #[test]
    fn test_tab_drag_lifecycle() {
        let mut controller = TabDragController::new();

        assert!(!controller.is_dragging());

        // Start drag
        let tab_rect = PanelRect::new(10.0, 0.0, 100.0, 32.0);
        controller.start_drag(0, LeafId(1), (50.0, 16.0), tab_rect);
        assert!(controller.is_dragging());

        // Check state
        let state = controller.drag_state().unwrap();
        assert_eq!(state.tab_idx, 0);
        assert_eq!(state.source_container, LeafId(1));

        // Update drag position
        controller.update_drag((100.0, 50.0));
        let state = controller.drag_state().unwrap();
        // Preview rect should follow mouse (with offset applied)
        assert_eq!(state.preview_rect.x, 100.0 - state.offset.0);
        assert_eq!(state.preview_rect.y, 50.0 - state.offset.1);

        // Complete drag
        let result = controller.complete_drag(LeafId(2), DropZone::Center);
        assert!(result.is_some());
        let (source, idx, target, zone) = result.unwrap();
        assert_eq!(source, LeafId(1));
        assert_eq!(idx, 0);
        assert_eq!(target, LeafId(2));
        assert_eq!(zone, DropZone::Center);

        assert!(!controller.is_dragging());
    }

    #[test]
    fn test_tab_drag_cancel() {
        let mut controller = TabDragController::new();

        let tab_rect = PanelRect::new(10.0, 0.0, 100.0, 32.0);
        controller.start_drag(0, LeafId(1), (50.0, 16.0), tab_rect);
        assert!(controller.is_dragging());

        controller.cancel();
        assert!(!controller.is_dragging());
    }

    #[test]
    fn test_tab_bar_set_active() {
        let mut tab_bar = TabBar::new(32.0);
        tab_bar.set_tabs(vec![
            (LeafId(1), "Chart".to_string(), true),
            (LeafId(2), "Table".to_string(), true),
            (LeafId(3), "Timeline".to_string(), true),
        ]);

        // Set active to second tab
        tab_bar.set_active(Some(LeafId(2)));
        assert_eq!(tab_bar.active_idx(), Some(1));

        // Set active to non-existent panel
        tab_bar.set_active(Some(LeafId(99)));
        assert_eq!(tab_bar.active_idx(), None);

        // Clear active
        tab_bar.set_active(None);
        assert_eq!(tab_bar.active_idx(), None);
    }
}
