//! Panel type definitions - semantic panel variants
//!
//! This module defines the panel taxonomy through enum types.
//! NO colors, NO rendering logic - only semantic classification.

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Main panel type enum covering all panel variants in the terminal
///
/// Panels are large container widgets that organize UI into sections.
///
/// # Variants
///
/// - **Toolbar**: Top/Bottom/Left/Right toolbars with buttons
/// - **Sidebar**: Left/Right/Bottom sidebars with controls
/// - **Modal**: Full-screen overlays with content (Search/Settings/Simple/Primitive)
/// - **Hideable**: Collapsible floating panels with toggle button (e.g., indicator menu)
#[derive(Debug, Clone, PartialEq)]
pub enum PanelType {
    /// Toolbar with positional variant and dimensions
    Toolbar {
        variant: ToolbarVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Sidebar with positional variant and dimensions
    Sidebar {
        variant: SidebarVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Modal with type variant and dimensions
    Modal {
        variant: ModalVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Hideable collapsible floating panel
    Hideable {
        /// Is currently hidden (collapsed)
        is_hidden: bool,
        /// Position (x, y) where panel appears
        position: (f64, f64),
        /// Panel width
        width: f64,
        /// Panel height (when expanded)
        height: f64,
    },
}

/// Toolbar positional variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarVariant {
    /// Top toolbar (main toolbar)
    Top,
    /// Bottom toolbar (status bar)
    Bottom,
    /// Left toolbar (vertical)
    Left,
    /// Right toolbar (vertical)
    Right,
}

/// Sidebar positional variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarVariant {
    /// Left sidebar (orders, positions)
    Left,
    /// Right sidebar (chart settings)
    Right,
    /// Bottom sidebar (alerts, watchlist)
    Bottom,
}

/// Modal type variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalVariant {
    /// Search modal with search input and results
    Search,
    /// Settings modal with tabs and forms
    Settings,
    /// Simple modal with single content area
    Simple,
    /// Primitive properties modal (drawing tools)
    Primitive,
}

impl WidgetCapabilities for PanelType {
    fn sense(&self) -> Sense {
        match self {
            PanelType::Hideable { .. } => Sense::CLICK.with_drag(),
            _ => Sense::CLICK,
        }
    }
}

impl PanelType {
    /// Create a toolbar with dimensions
    pub fn toolbar(variant: ToolbarVariant, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Toolbar {
            variant,
            position: (x, y),
            width,
            height,
        }
    }

    /// Create a sidebar with dimensions
    pub fn sidebar(variant: SidebarVariant, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Sidebar {
            variant,
            position: (x, y),
            width,
            height,
        }
    }

    /// Create a modal with dimensions
    pub fn modal(variant: ModalVariant, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Modal {
            variant,
            position: (x, y),
            width,
            height,
        }
    }

    /// Create a hideable panel
    pub fn hideable(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Hideable {
            is_hidden: false,
            position: (x, y),
            width,
            height,
        }
    }

    /// Check if panel is a toolbar
    pub fn is_toolbar(&self) -> bool {
        matches!(self, Self::Toolbar { .. })
    }

    /// Check if panel is a sidebar
    pub fn is_sidebar(&self) -> bool {
        matches!(self, Self::Sidebar { .. })
    }

    /// Check if panel is a modal
    pub fn is_modal(&self) -> bool {
        matches!(self, Self::Modal { .. })
    }

    /// Check if panel is hideable
    pub fn is_hideable(&self) -> bool {
        matches!(self, Self::Hideable { .. })
    }

    /// Get position (x, y)
    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Toolbar { position, .. } => *position,
            Self::Sidebar { position, .. } => *position,
            Self::Modal { position, .. } => *position,
            Self::Hideable { position, .. } => *position,
        }
    }

    /// Get width
    pub fn width(&self) -> f64 {
        match self {
            Self::Toolbar { width, .. } => *width,
            Self::Sidebar { width, .. } => *width,
            Self::Modal { width, .. } => *width,
            Self::Hideable { width, .. } => *width,
        }
    }

    /// Get height
    pub fn height(&self) -> f64 {
        match self {
            Self::Toolbar { height, .. } => *height,
            Self::Sidebar { height, .. } => *height,
            Self::Modal { height, .. } => *height,
            Self::Hideable { height, .. } => *height,
        }
    }
}
