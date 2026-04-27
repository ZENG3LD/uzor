//! Widget kind classification for InputCoordinator hierarchy.

/// Classifies widgets as composite parents or atomic leaves.
///
/// Composite parents contain children whose rects lie inside the parent's rect.
/// Atomic leaves are individual interactive elements with no sub-widgets.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    // --- Composite (parent — has children): ---
    /// Full-screen or large overlay that blocks interaction with content behind it.
    Modal,
    /// Dropdown or context menu (list of items).
    Menu,
    /// Small transient floating panel.
    Popup,
    /// Inline dropdown list attached to a trigger widget.
    Dropdown,
    /// Horizontal or vertical bar of grouped controls.
    Toolbar,
    /// Collapsible side panel.
    Sidebar,
    /// Right-click or long-press context menu.
    ContextMenu,
    /// Generic container / card.
    Panel,
    /// Canvas-like panel whose internal input dispatch is managed externally.
    /// The coordinator does NOT recurse into its children.
    /// `is_over_ui()` returns `false` when the cursor is over a BlackboxPanel.
    BlackboxPanel,
    /// Window decoration container: titlebar with tabs + min/max/close buttons.
    Chrome,
    /// Single tab in a tab strip (composite — can have a close-button child).
    Tab,

    // --- Atomic (leaf — no children): ---
    /// Clickable button.
    Button,
    /// Boolean checkbox (checked / unchecked).
    Checkbox,
    /// Toolbar time display with hover-only behavior.
    Clock,
    /// X-glyph dismiss button for modals and panels.
    CloseButton,
    /// Color swatch square that opens a color picker (atomic leaf).
    ColorSwatch,
    /// Trigger button for a dropdown menu (atomic leaf — trigger only, not the menu).
    DropdownTrigger,
    /// Selectable item inside a menu or list (non-interactive).
    Item,
    /// Single radio option (part of a group).
    Radio,
    /// Draggable thumb of a scrollbar.
    ScrollbarHandle,
    /// Non-draggable track of a scrollbar (click-to-jump area).
    ScrollbarTrack,
    /// Directional chevron for toolbar overflow navigation.
    ScrollChevron,
    /// Non-interactive visual divider.
    Separator,
    /// Selector button for shape/theme/UI-style choices (atomic leaf).
    ShapeSelector,
    /// Draggable slider thumb.
    Slider,
    /// iOS-style on/off toggle switch.
    Toggle,
    /// Text-with-background popup overlay. No internal interaction; hover tracked
    /// by the coordinator for fade-out logic.
    Tooltip,
    /// Escape hatch — any widget that doesn't fit the above categories.
    Custom,
}

impl WidgetKind {
    /// `true` for kinds that act as parent containers.
    pub const fn is_composite(self) -> bool {
        use WidgetKind::*;
        matches!(
            self,
            Modal | Menu | Popup | Dropdown | Toolbar | Sidebar
                | ContextMenu | Panel | BlackboxPanel | Chrome | Tab
        )
    }

    /// `true` for leaf kinds that cannot have registered children.
    pub const fn is_atomic(self) -> bool {
        !self.is_composite()
    }

    /// `true` only for `BlackboxPanel`.
    pub const fn is_blackbox(self) -> bool {
        matches!(self, WidgetKind::BlackboxPanel)
    }

    /// Whether widgets of this kind can have children registered under them.
    /// All composites except `BlackboxPanel` allow children.
    pub const fn allows_children(self) -> bool {
        self.is_composite() && !self.is_blackbox()
    }

    /// Whether widgets of this kind block events from reaching lower z layers
    /// when they are hit (modal / menu / context-menu behaviour).
    pub const fn blocks_lower_layers(self) -> bool {
        use WidgetKind::*;
        matches!(self, Modal | Menu | ContextMenu | Popup)
    }
}
