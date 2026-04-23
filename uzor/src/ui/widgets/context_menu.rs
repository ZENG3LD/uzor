//! Context menu system for uzor
//!
//! Provides right-click context menus with keyboard shortcuts, icons,
//! separators, and enabled/disabled states.
//!
//! # Example
//!
//! ```ignore
//! use uzor::input::context_menu::{ContextMenuState, ContextMenuItem};
//! use uzor::input::shortcuts::KeyboardShortcut;
//! use uzor::input::events::KeyCode;
//!
//! let mut menu = ContextMenuState::new();
//!
//! // Create menu items
//! let items = vec![
//!     ContextMenuItem::new("copy", "Copy")
//!         .with_shortcut(KeyboardShortcut::command(KeyCode::C)),
//!     ContextMenuItem::new("paste", "Paste")
//!         .with_shortcut(KeyboardShortcut::command(KeyCode::V)),
//!     ContextMenuItem::separator(),
//!     ContextMenuItem::new("delete", "Delete").disabled(),
//! ];
//!
//! // Open menu at cursor position
//! menu.open((100.0, 200.0), items);
//!
//! // Handle click on item
//! if let Some(item_id) = menu.handle_click(0) {
//!     println!("Clicked: {}", item_id);
//! }
//! ```

use crate::input::shortcuts::KeyboardShortcut;
use crate::input::widget_state::WidgetId;

// =============================================================================
// ContextMenuItem
// =============================================================================

/// A single item in a context menu
///
/// Represents a menu item with optional shortcuts, icons, and visual separators.
/// Items can be enabled or disabled to control user interaction.
#[derive(Clone, Debug)]
pub struct ContextMenuItem {
    /// Unique identifier for the item
    pub id: String,
    /// Display label
    pub label: String,
    /// Optional keyboard shortcut hint
    pub shortcut: Option<KeyboardShortcut>,
    /// Whether the item is enabled
    pub enabled: bool,
    /// Whether to show a separator after this item
    pub separator_after: bool,
    /// Optional icon identifier
    pub icon: Option<String>,
}

impl ContextMenuItem {
    /// Create a new context menu item
    ///
    /// # Example
    ///
    /// ```ignore
    /// let item = ContextMenuItem::new("copy", "Copy");
    /// ```
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            enabled: true,
            separator_after: false,
            icon: None,
        }
    }

    /// Add a keyboard shortcut to this item
    ///
    /// The shortcut will be displayed as a hint next to the item label.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let item = ContextMenuItem::new("copy", "Copy")
    ///     .with_shortcut(KeyboardShortcut::command(KeyCode::C));
    /// ```
    pub fn with_shortcut(mut self, shortcut: KeyboardShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// Mark this item as disabled
    ///
    /// Disabled items will be displayed but cannot be clicked.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let item = ContextMenuItem::new("paste", "Paste").disabled();
    /// ```
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add a separator after this item
    ///
    /// A visual separator line will be drawn after this item in the menu.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let item = ContextMenuItem::new("copy", "Copy").with_separator();
    /// ```
    pub fn with_separator(mut self) -> Self {
        self.separator_after = true;
        self
    }

    /// Add an icon to this item
    ///
    /// The icon identifier can be used by the renderer to display an icon
    /// next to the menu item.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let item = ContextMenuItem::new("open", "Open")
    ///     .with_icon("folder-open");
    /// ```
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Create a separator-only item
    ///
    /// This creates a non-interactive item that only displays a separator.
    /// The item will have an empty label and be disabled.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let items = vec![
    ///     ContextMenuItem::new("copy", "Copy"),
    ///     ContextMenuItem::separator(),
    ///     ContextMenuItem::new("paste", "Paste"),
    /// ];
    /// ```
    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            shortcut: None,
            enabled: false,
            separator_after: true,
            icon: None,
        }
    }

    /// Check if this is a separator-only item
    pub fn is_separator(&self) -> bool {
        self.id.is_empty() && self.label.is_empty()
    }
}

// =============================================================================
// ContextMenuRequest
// =============================================================================

/// Request to show a context menu
///
/// Contains the menu items and position information. Optionally tracks
/// the source widget that triggered the menu.
#[derive(Clone, Debug)]
pub struct ContextMenuRequest {
    /// Position to show menu (usually at cursor)
    pub position: (f64, f64),
    /// Menu items
    pub items: Vec<ContextMenuItem>,
    /// Widget that triggered the menu (optional)
    pub source_widget: Option<WidgetId>,
}

impl ContextMenuRequest {
    /// Create a new context menu request
    pub fn new(position: (f64, f64), items: Vec<ContextMenuItem>) -> Self {
        Self {
            position,
            items,
            source_widget: None,
        }
    }

    /// Create a context menu request with a source widget
    pub fn with_source(
        position: (f64, f64),
        items: Vec<ContextMenuItem>,
        source: WidgetId,
    ) -> Self {
        Self {
            position,
            items,
            source_widget: Some(source),
        }
    }
}

// =============================================================================
// ContextMenuState
// =============================================================================

/// Manages context menu state
///
/// Tracks the currently active menu, hover state, and handles click detection.
/// Only one context menu can be active at a time.
#[derive(Clone, Debug, Default)]
pub struct ContextMenuState {
    /// Active menu request
    active: Option<ContextMenuRequest>,
    /// Currently hovered item index
    hovered_item: Option<usize>,
    /// Menu rect for hit testing (set after layout)
    /// (x, y, width, height)
    menu_rect: Option<(f64, f64, f64, f64)>,
}

impl ContextMenuState {
    /// Create a new context menu state
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut menu = ContextMenuState::new();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a context menu at the specified position
    ///
    /// If a menu is already open, it will be replaced with the new menu.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let items = vec![
    ///     ContextMenuItem::new("copy", "Copy"),
    ///     ContextMenuItem::new("paste", "Paste"),
    /// ];
    /// menu.open((100.0, 200.0), items);
    /// ```
    pub fn open(&mut self, position: (f64, f64), items: Vec<ContextMenuItem>) {
        self.active = Some(ContextMenuRequest::new(position, items));
        self.hovered_item = None;
        self.menu_rect = None;
    }

    /// Open a context menu with a source widget
    ///
    /// This allows tracking which widget triggered the menu, which can be
    /// useful for context-specific actions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let widget_id = WidgetId::new("my_widget");
    /// menu.open_for_widget((100.0, 200.0), items, widget_id);
    /// ```
    pub fn open_for_widget(
        &mut self,
        position: (f64, f64),
        items: Vec<ContextMenuItem>,
        widget: WidgetId,
    ) {
        self.active = Some(ContextMenuRequest::with_source(position, items, widget));
        self.hovered_item = None;
        self.menu_rect = None;
    }

    /// Close the context menu
    ///
    /// Clears the active menu and all associated state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// menu.close();
    /// ```
    pub fn close(&mut self) {
        self.active = None;
        self.hovered_item = None;
        self.menu_rect = None;
    }

    /// Check if a context menu is currently open
    ///
    /// # Example
    ///
    /// ```ignore
    /// if menu.is_open() {
    ///     // Render menu
    /// }
    /// ```
    pub fn is_open(&self) -> bool {
        self.active.is_some()
    }

    /// Get the active menu request
    ///
    /// Returns None if no menu is currently open.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(request) = menu.get_active() {
    ///     for item in &request.items {
    ///         println!("{}", item.label);
    ///     }
    /// }
    /// ```
    pub fn get_active(&self) -> Option<&ContextMenuRequest> {
        self.active.as_ref()
    }

    /// Set the menu rectangle for hit testing
    ///
    /// This should be called after layout to enable proper click detection.
    /// The rectangle is in screen coordinates (x, y, width, height).
    ///
    /// # Example
    ///
    /// ```ignore
    /// menu.set_menu_rect((100.0, 200.0, 150.0, 100.0));
    /// ```
    pub fn set_menu_rect(&mut self, rect: (f64, f64, f64, f64)) {
        self.menu_rect = Some(rect);
    }

    /// Get the current menu rectangle
    pub fn get_menu_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.menu_rect
    }

    /// Set the currently hovered item
    ///
    /// Pass None to clear hover state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// menu.set_hovered(Some(2)); // Hover third item
    /// menu.set_hovered(None);     // Clear hover
    /// ```
    pub fn set_hovered(&mut self, index: Option<usize>) {
        self.hovered_item = index;
    }

    /// Get the currently hovered item index
    ///
    /// Returns None if no item is hovered.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(index) = menu.get_hovered() {
    ///     println!("Hovering item {}", index);
    /// }
    /// ```
    pub fn get_hovered(&self) -> Option<usize> {
        self.hovered_item
    }

    /// Handle a click on a menu item
    ///
    /// Returns the item ID if the item is enabled and was clicked successfully.
    /// Returns None if the item is disabled or out of bounds.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(item_id) = menu.handle_click(0) {
    ///     match item_id.as_str() {
    ///         "copy" => copy_to_clipboard(),
    ///         "paste" => paste_from_clipboard(),
    ///         _ => {}
    ///     }
    ///     menu.close();
    /// }
    /// ```
    pub fn handle_click(&mut self, index: usize) -> Option<String> {
        if let Some(ref request) = self.active {
            if let Some(item) = request.items.get(index) {
                if item.enabled && !item.is_separator() {
                    return Some(item.id.clone());
                }
            }
        }
        None
    }

    /// Get the number of items in the active menu
    ///
    /// Returns 0 if no menu is open.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let count = menu.item_count();
    /// ```
    pub fn item_count(&self) -> usize {
        self.active
            .as_ref()
            .map(|req| req.items.len())
            .unwrap_or(0)
    }

    /// Get a menu item by index
    ///
    /// Returns None if the index is out of bounds or no menu is open.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(item) = menu.get_item(0) {
    ///     println!("First item: {}", item.label);
    /// }
    /// ```
    pub fn get_item(&self, index: usize) -> Option<&ContextMenuItem> {
        self.active
            .as_ref()
            .and_then(|req| req.items.get(index))
    }
}

// =============================================================================
// ContextMenuResult
// =============================================================================

/// Result of context menu input handling
///
/// Contains information about what happened during input handling,
/// allowing the application to respond appropriately.
#[derive(Clone, Debug, Default)]
pub struct ContextMenuResult {
    /// Whether menu should close
    pub should_close: bool,
    /// Item that was clicked (id)
    pub clicked_item: Option<String>,
    /// Currently hovered item index
    pub hovered_index: Option<usize>,
}

impl ContextMenuResult {
    /// Create a new empty result
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a result indicating the menu should close
    pub fn close() -> Self {
        Self {
            should_close: true,
            ..Default::default()
        }
    }

    /// Create a result with a clicked item
    pub fn clicked(item_id: String) -> Self {
        Self {
            should_close: true,
            clicked_item: Some(item_id),
            hovered_index: None,
        }
    }

    /// Create a result with hover state
    pub fn hovered(index: usize) -> Self {
        Self {
            should_close: false,
            clicked_item: None,
            hovered_index: Some(index),
        }
    }
}

// =============================================================================
// Input Handling
// =============================================================================

/// Handle input for context menu
///
/// This is a helper function that processes mouse input for context menus.
/// It handles hover detection, clicks, and outside clicks.
///
/// # Arguments
///
/// * `state` - The context menu state to update
/// * `item_rects` - List of (index, rect) pairs for each menu item
/// * `cursor_pos` - Current cursor position (None if not available)
/// * `clicked` - Whether a mouse click occurred this frame
/// * `clicked_outside` - Whether a click occurred outside the menu
///
/// # Returns
///
/// A ContextMenuResult indicating what happened
///
/// # Example
///
/// ```ignore
/// let item_rects = vec![
///     (0, (100.0, 200.0, 150.0, 20.0)),
///     (1, (100.0, 220.0, 150.0, 20.0)),
/// ];
///
/// let result = handle_context_menu_input(
///     &mut menu,
///     &item_rects,
///     Some((125.0, 210.0)),
///     true,
///     false,
/// );
///
/// if result.should_close {
///     menu.close();
/// }
/// ```
#[allow(clippy::type_complexity)]
pub fn handle_context_menu_input(
    state: &mut ContextMenuState,
    item_rects: &[(usize, (f64, f64, f64, f64))],
    cursor_pos: Option<(f64, f64)>,
    clicked: bool,
    clicked_outside: bool,
) -> ContextMenuResult {
    // If clicked outside, close menu
    if clicked_outside {
        return ContextMenuResult::close();
    }

    // Check cursor position against items
    if let Some((cx, cy)) = cursor_pos {
        let mut hovered_index = None;

        for &(index, (x, y, w, h)) in item_rects {
            if cx >= x && cx < x + w && cy >= y && cy < y + h {
                hovered_index = Some(index);
                break;
            }
        }

        // Update hover state
        state.set_hovered(hovered_index);

        // Handle click
        if clicked {
            if let Some(index) = hovered_index {
                if let Some(item_id) = state.handle_click(index) {
                    return ContextMenuResult::clicked(item_id);
                }
            }
        }

        // Return hover state
        if let Some(index) = hovered_index {
            return ContextMenuResult::hovered(index);
        }
    }

    ContextMenuResult::new()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::events::KeyCode;

    #[test]
    fn test_menu_item_creation() {
        let item = ContextMenuItem::new("copy", "Copy");
        assert_eq!(item.id, "copy");
        assert_eq!(item.label, "Copy");
        assert!(item.enabled);
        assert!(!item.separator_after);
        assert!(item.shortcut.is_none());
        assert!(item.icon.is_none());
    }

    #[test]
    fn test_menu_item_builder() {
        let item = ContextMenuItem::new("copy", "Copy")
            .with_shortcut(KeyboardShortcut::command(KeyCode::C))
            .with_icon("clipboard")
            .with_separator();

        assert!(item.shortcut.is_some());
        assert_eq!(item.icon, Some("clipboard".to_string()));
        assert!(item.separator_after);
        assert!(item.enabled);
    }

    #[test]
    fn test_menu_item_disabled() {
        let item = ContextMenuItem::new("paste", "Paste").disabled();
        assert!(!item.enabled);
    }

    #[test]
    fn test_separator_item() {
        let sep = ContextMenuItem::separator();
        assert!(sep.is_separator());
        assert!(!sep.enabled);
        assert!(sep.separator_after);
        assert_eq!(sep.id, "");
        assert_eq!(sep.label, "");
    }

    #[test]
    fn test_menu_state_open_close() {
        let mut menu = ContextMenuState::new();
        assert!(!menu.is_open());

        let items = vec![
            ContextMenuItem::new("copy", "Copy"),
            ContextMenuItem::new("paste", "Paste"),
        ];

        menu.open((100.0, 200.0), items);
        assert!(menu.is_open());
        assert_eq!(menu.item_count(), 2);

        menu.close();
        assert!(!menu.is_open());
        assert_eq!(menu.item_count(), 0);
    }

    #[test]
    fn test_menu_state_with_widget() {
        let mut menu = ContextMenuState::new();
        let widget_id = WidgetId::new("my_widget");

        let items = vec![ContextMenuItem::new("action", "Action")];

        menu.open_for_widget((100.0, 200.0), items, widget_id.clone());

        assert!(menu.is_open());
        let request = menu.get_active().unwrap();
        assert_eq!(request.source_widget, Some(widget_id));
    }

    #[test]
    fn test_hover_tracking() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("item1", "Item 1"),
            ContextMenuItem::new("item2", "Item 2"),
        ];

        menu.open((100.0, 200.0), items);

        assert_eq!(menu.get_hovered(), None);

        menu.set_hovered(Some(0));
        assert_eq!(menu.get_hovered(), Some(0));

        menu.set_hovered(Some(1));
        assert_eq!(menu.get_hovered(), Some(1));

        menu.set_hovered(None);
        assert_eq!(menu.get_hovered(), None);
    }

    #[test]
    fn test_click_handling_enabled() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("copy", "Copy"),
            ContextMenuItem::new("paste", "Paste"),
        ];

        menu.open((100.0, 200.0), items);

        let result = menu.handle_click(0);
        assert_eq!(result, Some("copy".to_string()));

        let result = menu.handle_click(1);
        assert_eq!(result, Some("paste".to_string()));
    }

    #[test]
    fn test_click_handling_disabled() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("copy", "Copy"),
            ContextMenuItem::new("paste", "Paste").disabled(),
        ];

        menu.open((100.0, 200.0), items);

        // Enabled item should work
        let result = menu.handle_click(0);
        assert_eq!(result, Some("copy".to_string()));

        // Disabled item should not work
        let result = menu.handle_click(1);
        assert_eq!(result, None);
    }

    #[test]
    fn test_click_handling_separator() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("copy", "Copy"),
            ContextMenuItem::separator(),
            ContextMenuItem::new("paste", "Paste"),
        ];

        menu.open((100.0, 200.0), items);

        // Separator should not be clickable
        let result = menu.handle_click(1);
        assert_eq!(result, None);

        // Regular items should work
        let result = menu.handle_click(0);
        assert_eq!(result, Some("copy".to_string()));

        let result = menu.handle_click(2);
        assert_eq!(result, Some("paste".to_string()));
    }

    #[test]
    fn test_click_out_of_bounds() {
        let mut menu = ContextMenuState::new();
        let items = vec![ContextMenuItem::new("copy", "Copy")];

        menu.open((100.0, 200.0), items);

        let result = menu.handle_click(5);
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_item() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("copy", "Copy"),
            ContextMenuItem::new("paste", "Paste"),
        ];

        menu.open((100.0, 200.0), items);

        let item = menu.get_item(0).unwrap();
        assert_eq!(item.id, "copy");

        let item = menu.get_item(1).unwrap();
        assert_eq!(item.id, "paste");

        assert!(menu.get_item(2).is_none());
    }

    #[test]
    fn test_menu_rect() {
        let mut menu = ContextMenuState::new();
        assert_eq!(menu.get_menu_rect(), None);

        menu.set_menu_rect((100.0, 200.0, 150.0, 80.0));
        assert_eq!(menu.get_menu_rect(), Some((100.0, 200.0, 150.0, 80.0)));
    }

    #[test]
    fn test_handle_input_hover() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("item1", "Item 1"),
            ContextMenuItem::new("item2", "Item 2"),
        ];
        menu.open((100.0, 200.0), items);

        let item_rects = vec![
            (0, (100.0, 200.0, 150.0, 20.0)),
            (1, (100.0, 220.0, 150.0, 20.0)),
        ];

        // Hover over first item
        let result = handle_context_menu_input(
            &mut menu,
            &item_rects,
            Some((125.0, 210.0)),
            false,
            false,
        );

        assert!(!result.should_close);
        assert_eq!(result.hovered_index, Some(0));
        assert_eq!(menu.get_hovered(), Some(0));

        // Hover over second item
        let result = handle_context_menu_input(
            &mut menu,
            &item_rects,
            Some((125.0, 230.0)),
            false,
            false,
        );

        assert_eq!(result.hovered_index, Some(1));
        assert_eq!(menu.get_hovered(), Some(1));
    }

    #[test]
    fn test_handle_input_click() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("copy", "Copy"),
            ContextMenuItem::new("paste", "Paste"),
        ];
        menu.open((100.0, 200.0), items);

        let item_rects = vec![
            (0, (100.0, 200.0, 150.0, 20.0)),
            (1, (100.0, 220.0, 150.0, 20.0)),
        ];

        // Click first item
        let result = handle_context_menu_input(
            &mut menu,
            &item_rects,
            Some((125.0, 210.0)),
            true,
            false,
        );

        assert!(result.should_close);
        assert_eq!(result.clicked_item, Some("copy".to_string()));
    }

    #[test]
    fn test_handle_input_click_outside() {
        let mut menu = ContextMenuState::new();
        let items = vec![ContextMenuItem::new("copy", "Copy")];
        menu.open((100.0, 200.0), items);

        let item_rects = vec![(0, (100.0, 200.0, 150.0, 20.0))];

        // Click outside
        let result = handle_context_menu_input(
            &mut menu,
            &item_rects,
            Some((50.0, 50.0)),
            true,
            true,
        );

        assert!(result.should_close);
        assert_eq!(result.clicked_item, None);
    }

    #[test]
    fn test_handle_input_disabled_item() {
        let mut menu = ContextMenuState::new();
        let items = vec![
            ContextMenuItem::new("copy", "Copy"),
            ContextMenuItem::new("paste", "Paste").disabled(),
        ];
        menu.open((100.0, 200.0), items);

        let item_rects = vec![
            (0, (100.0, 200.0, 150.0, 20.0)),
            (1, (100.0, 220.0, 150.0, 20.0)),
        ];

        // Click disabled item
        let result = handle_context_menu_input(
            &mut menu,
            &item_rects,
            Some((125.0, 230.0)),
            true,
            false,
        );

        // Should not trigger click
        assert!(!result.should_close);
        assert_eq!(result.clicked_item, None);
    }

    #[test]
    fn test_context_menu_result() {
        let result = ContextMenuResult::new();
        assert!(!result.should_close);
        assert_eq!(result.clicked_item, None);
        assert_eq!(result.hovered_index, None);

        let result = ContextMenuResult::close();
        assert!(result.should_close);

        let result = ContextMenuResult::clicked("copy".to_string());
        assert!(result.should_close);
        assert_eq!(result.clicked_item, Some("copy".to_string()));

        let result = ContextMenuResult::hovered(2);
        assert!(!result.should_close);
        assert_eq!(result.hovered_index, Some(2));
    }
}
