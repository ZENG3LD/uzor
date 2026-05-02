//! Overflow modes — what a container does when its content doesn't fit.
//!
//! This is the cross-cutting policy every container-shaped widget must
//! honour: panels, sidebars, modal bodies, etc. The mode is set per-widget
//! via its `Settings` (or `View`) and consulted by the widget's render path.

/// What a container does when its declared content extent exceeds the rect
/// it has been given.
///
/// All four behaviours operate on the **same** underlying inputs:
/// `content_size` (what the caller wants to draw) and `viewport_size` (what
/// the layout solver gave us). The container picks one of these strategies
/// depending on the application's UX preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OverflowMode {
    /// Hard clip at the rect boundary. Content past the edge simply isn't
    /// drawn. No scrollbar, no chevron, no shrink. Default — matches the
    /// existing behaviour for widgets that pre-date this enum.
    #[default]
    Clip,

    /// Compress: scale content proportionally to fit the rect.
    ///
    /// Implemented by widgets whose layout is fluid (panel grids,
    /// auto-flow toolbars, etc.). For widgets with rigid content
    /// (e.g. plain text rows) compression is meaningless and the widget
    /// should fall back to `Clip`.
    Compress,

    /// Show paging chevrons (arrows) on the overflowing edges so the user
    /// can step through hidden content. Used by toolbars, tab strips,
    /// breadcrumbs — anywhere paging by item rather than by pixel makes
    /// sense.
    Chevrons,

    /// Show a scrollbar on the overflowing axis (or both, if both overflow).
    /// Used by sidebars, panel bodies, long modal bodies — anywhere
    /// continuous pixel-scroll is the natural interaction.
    Scrollbar,
}

impl OverflowMode {
    /// Should the container draw a scrollbar when content overflows?
    #[inline]
    pub fn shows_scrollbar(self) -> bool {
        matches!(self, OverflowMode::Scrollbar)
    }

    /// Should the container draw paging chevrons when content overflows?
    #[inline]
    pub fn shows_chevrons(self) -> bool {
        matches!(self, OverflowMode::Chevrons)
    }

    /// Should the container scale content to fit?
    #[inline]
    pub fn compresses(self) -> bool {
        matches!(self, OverflowMode::Compress)
    }
}
