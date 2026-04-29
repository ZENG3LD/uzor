//! Panel type definitions — per-frame view data and render kind enum.
//!
//! Five structurally-distinct templates plus a Custom escape hatch cover
//! all panel variants observed in mlc (DOM, PositionManager, TradeLog, OrderEntry).
//!
//! Legacy taxonomy enums (`PanelType`, `ToolbarVariant`, `SidebarVariant`,
//! `ModalVariant`) are retained for backwards compatibility with existing callers.

use crate::input::{InputCoordinator, Sense};
use crate::render::RenderContext;
use crate::types::Rect;
use crate::ui::widgets::WidgetCapabilities;

use super::settings::PanelSettings;

// ---------------------------------------------------------------------------
// Legacy taxonomy (kept for backwards compatibility with crate public API)
// ---------------------------------------------------------------------------

/// Main panel type enum covering all panel variants in the terminal.
///
/// Panels are large container widgets that organise UI into sections.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelType {
    /// Toolbar with positional variant and dimensions.
    Toolbar {
        variant: ToolbarVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    /// Sidebar with positional variant and dimensions.
    Sidebar {
        variant: SidebarVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    /// Modal with type variant and dimensions.
    Modal {
        variant: ModalVariant,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
    /// Hideable collapsible floating panel.
    Hideable {
        is_hidden: bool,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}

/// Toolbar positional variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarVariant {
    Top,
    Bottom,
    Left,
    Right,
}

/// Sidebar positional variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarVariant {
    Left,
    Right,
    Bottom,
}

/// Modal type variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalVariant {
    Search,
    Settings,
    Simple,
    Primitive,
}

impl WidgetCapabilities for PanelType {
    fn sense(&self) -> Sense {
        Sense::CLICK
    }
}

impl PanelType {
    /// Create a toolbar with dimensions.
    pub fn toolbar(variant: ToolbarVariant, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Toolbar { variant, position: (x, y), width, height }
    }

    /// Create a sidebar with dimensions.
    pub fn sidebar(variant: SidebarVariant, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Sidebar { variant, position: (x, y), width, height }
    }

    /// Create a modal with dimensions.
    pub fn modal(variant: ModalVariant, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Modal { variant, position: (x, y), width, height }
    }

    /// Create a hideable panel.
    pub fn hideable(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Hideable { is_hidden: false, position: (x, y), width, height }
    }

    /// Returns `(x, y)` position.
    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Toolbar { position, .. } => *position,
            Self::Sidebar { position, .. } => *position,
            Self::Modal   { position, .. } => *position,
            Self::Hideable { position, .. } => *position,
        }
    }

    /// Returns width.
    pub fn width(&self) -> f64 {
        match self {
            Self::Toolbar { width, .. } => *width,
            Self::Sidebar { width, .. } => *width,
            Self::Modal   { width, .. } => *width,
            Self::Hideable { width, .. } => *width,
        }
    }

    /// Returns height.
    pub fn height(&self) -> f64 {
        match self {
            Self::Toolbar { height, .. } => *height,
            Self::Sidebar { height, .. } => *height,
            Self::Modal   { height, .. } => *height,
            Self::Hideable { height, .. } => *height,
        }
    }
}

// ---------------------------------------------------------------------------
// PanelHeader
// ---------------------------------------------------------------------------

/// Header strip data: title text and optional action buttons.
///
/// Rendered as a fixed-height strip at the top of the panel.
pub struct PanelHeader<'a> {
    /// Title text, left-aligned in the header strip.
    pub title: &'a str,
    /// Action buttons rendered right-to-left in the header strip.
    pub actions: &'a [HeaderAction<'a>],
}

// ---------------------------------------------------------------------------
// HeaderAction
// ---------------------------------------------------------------------------

/// One icon-only action button in the panel header.
pub struct HeaderAction<'a> {
    /// Stable string id returned on click.
    pub id: &'a str,
    /// Icon identifier (placeholder — actual SVG render by caller).
    pub icon: &'a str,
    /// Optional tooltip shown on hover.
    pub tooltip: Option<&'a str>,
    /// Whether the pointer is currently hovering over this action.
    pub hovered: bool,
}

// ---------------------------------------------------------------------------
// ColumnDef
// ---------------------------------------------------------------------------

/// Column definition for the column-header row.
pub struct ColumnDef<'a> {
    /// Stable column identifier.
    pub id: &'a str,
    /// Short display label (typically ALL-CAPS).
    pub label: &'a str,
    /// Fractional width in the range `0.0..=1.0`.
    /// The composite multiplies this by the total available width.
    pub width: f64,
    /// Whether clicking this column header toggles sort order.
    pub sortable: bool,
}

// ---------------------------------------------------------------------------
// PanelView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_*_panel`.
pub struct PanelView<'a> {
    /// Header slot — title and optional action buttons.
    ///
    /// `None` for `Plain` kind (no header strip rendered).
    pub header: Option<PanelHeader<'a>>,

    /// Column definitions for the column-header row.
    ///
    /// Empty slice for kinds without a column-header zone.
    pub columns: &'a [ColumnDef<'a>],

    /// Body closure — called with the computed body rect after all fixed zones
    /// (header, column-header) have been drawn.
    ///
    /// Per-frame `Box` allocation is acceptable (single alloc per frame).
    pub body: Box<dyn FnMut(&mut dyn RenderContext, Rect, &mut InputCoordinator) + 'a>,

    /// Footer closure — called with the computed footer rect.
    ///
    /// `None` for kinds without a footer zone.
    pub footer: Option<Box<dyn FnMut(&mut dyn RenderContext, Rect, &mut InputCoordinator) + 'a>>,

    /// Whether to render a scrollbar on the body right edge.
    pub show_scrollbar: bool,

    /// Total content height in pixels (for scrollbar thumb ratio).
    pub content_height: f64,
}

// ---------------------------------------------------------------------------
// PanelRenderKind
// ---------------------------------------------------------------------------

/// Selects which layout pipeline the composite runs.
///
/// | Kind                      | header | col-header | body | footer | scrollbar |
/// |---------------------------|--------|------------|------|--------|-----------|
/// | `Plain`                   | ✗      | ✗          | ✓    | ✗      | opt       |
/// | `WithHeader`              | ✓      | ✗          | ✓    | ✗      | opt       |
/// | `WithHeaderColumns`       | ✓      | ✓          | ✓    | ✗      | opt       |
/// | `WithFooter`              | ✓      | ✗          | ✓    | ✓      | opt       |
/// | `WithHeaderColumnsFooter` | ✓      | ✓          | ✓    | ✓      | opt       |
/// | `Custom`                  | —      | —          | —    | —      | —         |
pub enum PanelRenderKind {
    /// Background fill + body closure only.
    Plain,
    /// Header strip + body closure.
    WithHeader,
    /// Header strip + sticky column-header row + scrollable body closure.
    WithHeaderColumns,
    /// Header strip + body closure + footer closure.
    WithFooter,
    /// Full layout: header + column-header + scrollable body + footer.
    WithHeaderColumnsFooter,
    /// Escape hatch — caller drives every draw call via a single closure.
    Custom(Box<dyn Fn(&mut dyn RenderContext, Rect, &PanelView<'_>, &PanelSettings)>),
}
