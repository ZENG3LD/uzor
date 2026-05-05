//! Panel geometry parameters trait, presets, and `BackgroundFill`.
//!
//! Values ported from mlc panel audit (`panel-deep.md` §5).

// ---------------------------------------------------------------------------
// Border + edge-handle configuration
// ---------------------------------------------------------------------------

/// Per-side stroke for a panel frame border.
#[derive(Clone, Copy, Debug)]
pub struct BorderStroke {
    /// Line thickness in pixels.
    pub width: f64,
    /// Opacity multiplier `[0.0, 1.0]` applied to the theme border colour.
    pub opacity: f64,
}

impl Default for BorderStroke {
    fn default() -> Self {
        Self { width: 1.0, opacity: 1.0 }
    }
}

/// Per-edge visibility for the panel frame border.
///
/// Each side independently picks visibility / width / opacity.  `None`
/// means "no visual on that side" (the default for `BorderConfig::none()`).
/// The edge-handle hit zone (`EdgeHandlesConfig`) is independent — a
/// panel can have a drag handle without painting a visible stroke and
/// vice versa.
#[derive(Clone, Copy, Debug, Default)]
pub struct BorderConfig {
    pub top:    Option<BorderStroke>,
    pub right:  Option<BorderStroke>,
    pub bottom: Option<BorderStroke>,
    pub left:   Option<BorderStroke>,
}

impl BorderConfig {
    /// All sides off (default).
    pub fn none() -> Self { Self::default() }

    /// All four sides at the default stroke.
    pub fn all() -> Self {
        let s = Some(BorderStroke::default());
        Self { top: s, right: s, bottom: s, left: s }
    }
}

/// Per-edge resize-handle hit zones.  `true` enables a drag zone on
/// that side — the composite registers a `Sense::DRAG` rect dispatched
/// to `EventBuilder::ResizeHandle`.  Independent from `BorderConfig` —
/// the visual stroke lives in `BorderConfig`, the hit zone here.
#[derive(Clone, Copy, Debug, Default)]
pub struct EdgeHandlesConfig {
    pub top:    bool,
    pub right:  bool,
    pub bottom: bool,
    pub left:   bool,
}

impl EdgeHandlesConfig {
    /// All four sides off (no resize hit zones).
    pub fn none() -> Self { Self::default() }

    /// All four sides have a drag hit zone.
    pub fn all() -> Self {
        Self { top: true, right: true, bottom: true, left: true }
    }
}

// ---------------------------------------------------------------------------
// BackgroundFill
// ---------------------------------------------------------------------------

/// How the panel frame background is filled.
#[derive(Debug, Clone)]
pub enum BackgroundFill {
    /// Flat colour — uses `theme.bg()`.
    Solid,

    /// GPU blur of what is behind the panel, then `theme.bg()` at reduced alpha.
    ///
    /// Falls back to `Solid` on backends without blur support.
    Glass {
        /// Blur kernel radius in pixels.
        blur_radius: f64,
    },
}

// ---------------------------------------------------------------------------
// PanelStyle trait
// ---------------------------------------------------------------------------

/// Geometry parameters for the panel composite.
///
/// Implement this trait to customise sizes without touching colours.
pub trait PanelStyle {
    /// Header strip height in pixels.  Default: `22.0`.
    fn header_height(&self) -> f64;

    /// Column-header row height in pixels.  Default: `18.0`.
    fn column_header_height(&self) -> f64;

    /// Body row height hint (informational — used by callers, not the composite).
    /// Default: `20.0`.
    fn row_height(&self) -> f64;

    /// Footer strip height in pixels.  Default: `20.0`.
    fn footer_height(&self) -> f64;

    /// Inner horizontal padding in pixels.  Default: `6.0`.
    fn padding(&self) -> f64;

    /// Border line thickness in pixels.  Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Scrollbar column width in pixels.  Default: `8.0`.
    fn scrollbar_width(&self) -> f64;

    /// Background fill strategy.  Default: `BackgroundFill::Solid`.
    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }

    /// Per-side border-stroke visibility.  Default: all four sides off
    /// (the panel paints no frame strokes — matches legacy behaviour).
    fn borders(&self) -> BorderConfig {
        BorderConfig::none()
    }

    /// Per-side resize-handle hit zones.  Default: all four sides off
    /// (the panel is non-resizable until a higher layer enables it).
    fn edge_handles(&self) -> EdgeHandlesConfig {
        EdgeHandlesConfig::none()
    }

    /// Hit-zone thickness for the resize handles in pixels.  Default
    /// `8.0` matches the sidebar resize zone for visual consistency.
    fn edge_handle_width(&self) -> f64 { 8.0 }
}

// ---------------------------------------------------------------------------
// Default style
// ---------------------------------------------------------------------------

/// Default style preset matching mlc panel geometry.
#[derive(Default)]
pub struct DefaultPanelStyle;

impl PanelStyle for DefaultPanelStyle {
    fn header_height(&self)        -> f64 { 22.0 }
    fn column_header_height(&self) -> f64 { 18.0 }
    fn row_height(&self)           -> f64 { 20.0 }
    fn footer_height(&self)        -> f64 { 20.0 }
    fn padding(&self)              -> f64 { 6.0  }
    fn border_width(&self)         -> f64 { 1.0  }
    fn scrollbar_width(&self)      -> f64 { 8.0  }
}
