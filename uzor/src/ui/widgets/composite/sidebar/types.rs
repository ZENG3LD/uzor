//! Sidebar type definitions — per-frame view data and render kind enum.
//!
//! Ported from the mlc deep audit in `sidebar-deep.md`.
//! Five structurally-distinct templates cover all sidebar variants.

use crate::render::RenderContext;
use crate::types::{IconId, Rect};

use super::settings::SidebarSettings;

// ---------------------------------------------------------------------------
// SidebarHeader
// ---------------------------------------------------------------------------

/// Header slot data: icon + title + optional action buttons.
///
/// Rendered as a 40 px strip at the top of the sidebar.
pub struct SidebarHeader<'a> {
    /// Optional icon drawn at the left edge of the header.
    pub icon: Option<&'a IconId>,
    /// Title text, left-aligned after the icon.
    pub title: &'a str,
    /// Action buttons rendered right-to-left before the close/collapse button.
    pub actions: &'a [HeaderAction<'a>],
}

// ---------------------------------------------------------------------------
// HeaderAction
// ---------------------------------------------------------------------------

/// One icon-only action button in the sidebar header.
pub struct HeaderAction<'a> {
    /// Stable string id returned on click.
    pub id: &'a str,
    /// Icon identifier.
    pub icon: &'a IconId,
    /// Optional tooltip shown on hover.
    pub tooltip: Option<&'a str>,
}

// ---------------------------------------------------------------------------
// SidebarTab  (WithTypeSelector)
// ---------------------------------------------------------------------------

/// One tab entry in the `WithTypeSelector` tab strip.
pub struct SidebarTab<'a> {
    /// Stable string id used to identify which tab is active.
    pub id: &'a str,
    /// Display label.
    pub label: &'a str,
    /// Optional icon drawn left of the label.
    pub icon: Option<&'a IconId>,
}

// ---------------------------------------------------------------------------
// SidebarView
// ---------------------------------------------------------------------------

/// Per-frame data handed to `register_*_sidebar`.
pub struct SidebarView<'a> {
    /// Header slot — icon, title, actions.
    pub header: SidebarHeader<'a>,

    /// Tab entries for `WithTypeSelector`.  Empty for other kinds.
    pub tabs: &'a [SidebarTab<'a>],

    /// Currently active tab id for `WithTypeSelector`.  `None` = no tab active.
    pub active_tab: Option<&'a str>,

    /// Whether to render a scrollbar on the body edge.
    pub show_scrollbar: bool,

    /// Total content height in pixels (for scrollbar ratio).
    pub content_height: f64,
}

// ---------------------------------------------------------------------------
// SidebarRenderKind
// ---------------------------------------------------------------------------

/// Selects which layout pipeline the composite runs.
///
/// | Kind             | border | resize-edge | tab-strip | scrollbar | collapse |
/// |------------------|--------|-------------|-----------|-----------|----------|
/// | `Right`          | left   | left edge   | ✗         | right     | ✓        |
/// | `Left`           | right  | right edge  | ✗         | right     | ✓        |
/// | `WithTypeSelector` | left | left edge   | top strip | right     | ✓        |
/// | `Embedded`       | ✗      | ✗           | ✗         | right     | ✗        |
/// | `Custom`         | —      | —           | —         | —         | —        |
pub enum SidebarRenderKind {
    /// Collapsible right-side panel.  Border and resize edge on the left.
    Right,
    /// Collapsible left-side panel.  Mirror of `Right` — resize edge on the right.
    Left,
    /// Sidebar with a top tab strip that switches panel type.
    WithTypeSelector,
    /// Minimalist embedded sidebar (inside modals).  No resize, no collapse.
    Embedded,
    /// Escape hatch — caller drives every draw call.
    Custom(
        Box<dyn Fn(&mut dyn RenderContext, Rect, &SidebarView<'_>, &SidebarSettings)>,
    ),
}
