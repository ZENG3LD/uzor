//! Sidebar geometry parameters trait, presets, and `BackgroundFill`.
//!
//! Values ported from the mlc sidebar audit (`sidebar-deep.md` §9).

// ---------------------------------------------------------------------------
// Border / Divider styling
// ---------------------------------------------------------------------------

/// Per-edge border configuration for a sidebar frame.
///
/// Each side independently picks visibility, width, and an opacity
/// multiplier applied on top of the theme's border colour. `None` means
/// "no border on this side". The default — `BorderConfig::inner()` —
/// reproduces the legacy behaviour: a 1-px line on the inner edge only.
#[derive(Clone, Debug)]
pub struct BorderConfig {
    pub top:    Option<BorderStroke>,
    pub right:  Option<BorderStroke>,
    pub bottom: Option<BorderStroke>,
    pub left:   Option<BorderStroke>,
}

/// One side of a `BorderConfig`.
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

impl BorderConfig {
    /// All sides off.
    pub fn none() -> Self {
        Self { top: None, right: None, bottom: None, left: None }
    }

    /// All four sides at the default 1-px / fully opaque stroke.
    pub fn all() -> Self {
        let s = Some(BorderStroke::default());
        Self { top: s, right: s, bottom: s, left: s }
    }
}

/// Configuration for one of the composite's internal divider lines
/// (header bottom, footer top, item separator, …).
#[derive(Clone, Copy, Debug)]
pub struct DividerConfig {
    /// Whether the divider is drawn at all.
    pub visible: bool,
    /// Line thickness in pixels.
    pub width:   f64,
    /// Opacity multiplier `[0.0, 1.0]` applied to the theme divider colour.
    pub opacity: f64,
    /// Length of the line as a fraction of the available extent `[0.0, 1.0]`.
    /// `1.0` = full edge-to-edge; `0.5` = centred 50% line; etc.
    pub length_frac: f64,
}

impl Default for DividerConfig {
    fn default() -> Self {
        Self { visible: true, width: 1.0, opacity: 1.0, length_frac: 1.0 }
    }
}

// ---------------------------------------------------------------------------
// BackgroundFill
// ---------------------------------------------------------------------------

/// How the sidebar frame background is filled.
///
/// Mirrors `modal/style.rs::BackgroundFill` but is independent so sidebar and
/// modal styles can evolve separately.
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
// SidebarStyle trait
// ---------------------------------------------------------------------------

/// Geometry parameters for the sidebar composite.
///
/// Implement this trait to customise sizes without touching colours.
pub trait SidebarStyle {
    /// Sidebar header height in pixels.  Default: `40.0`.
    fn header_height(&self) -> f64;

    /// Tab strip height for `WithTypeSelector`.  Default: `32.0`.
    fn tab_strip_height(&self) -> f64;

    /// Inner body padding in pixels.  Default: `12.0`.
    fn padding(&self) -> f64;

    /// Width of the interactive resize zone centered on the border line.  Default: `8.0`.
    fn resize_zone_width(&self) -> f64;

    /// Border line thickness in pixels.  Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Minimum sidebar width in pixels.  Default: `280.0`.
    fn min_width(&self) -> f64;

    /// Maximum sidebar width in pixels.  Default: `4000.0`.
    fn max_width(&self) -> f64;

    /// Default sidebar width in pixels.  Default: `340.0`.
    fn default_width(&self) -> f64;

    /// Scrollbar column width in pixels.  Default: `8.0`.
    fn scrollbar_width(&self) -> f64;

    /// Background fill strategy.  Default: `BackgroundFill::Solid`.
    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }

    /// Whether to render a 1 px divider line under the header.  Default: `true`.
    ///
    /// **Deprecated**: prefer `header_divider()`. Kept for backwards
    /// compatibility — when this returns `false` the new `header_divider`
    /// is force-hidden.
    fn show_header_divider(&self) -> bool {
        true
    }

    /// Per-side border configuration. Default: a 1-px stroke on the inner
    /// edge only — i.e. `Right` for `Left` sidebars, `Left` for `Right`/
    /// `WithTypeSelector`, `Bottom` for `Top` sidebars, `Top` for `Bottom`.
    /// Returning `BorderConfig::none()` paints no frame border at all.
    /// The composite resolves "inner edge" against the active render kind.
    fn borders(&self) -> BorderConfig {
        BorderConfig {
            top:    None,
            right:  None,
            bottom: None,
            left:   None,
        } // sentinel — composite falls back to legacy single-edge mode
    }

    /// Configuration for the divider line drawn below the header.
    /// Default: visible, 1 px, fully opaque, full width.
    fn header_divider(&self) -> DividerConfig {
        DividerConfig::default()
    }

    /// Configuration for an optional divider above the footer (if the
    /// caller draws a footer band). Default: hidden.
    fn footer_divider(&self) -> DividerConfig {
        DividerConfig { visible: false, ..DividerConfig::default() }
    }
}

// ---------------------------------------------------------------------------
// Default (Right / Left)
// ---------------------------------------------------------------------------

/// Default style preset — matches mlc Right/Left sidebar geometry.
#[derive(Default)]
pub struct DefaultSidebarStyle;

impl SidebarStyle for DefaultSidebarStyle {
    fn header_height(&self)    -> f64 { 40.0  }
    fn tab_strip_height(&self) -> f64 { 32.0  }
    fn padding(&self)          -> f64 { 12.0  }
    fn resize_zone_width(&self) -> f64 { 8.0  }
    fn border_width(&self)     -> f64 { 1.0   }
    fn min_width(&self)        -> f64 { 280.0 }
    fn max_width(&self)        -> f64 { 4000.0 }
    fn default_width(&self)    -> f64 { 340.0 }
    fn scrollbar_width(&self)  -> f64 { 8.0   }
}

// ---------------------------------------------------------------------------
// Named presets
// ---------------------------------------------------------------------------

/// Style preset for `SidebarRenderKind::Right`.
///
/// Identical to `DefaultSidebarStyle` (right sidebar is the canonical form).
pub type RightSidebarStyle = DefaultSidebarStyle;

/// Style preset for `SidebarRenderKind::Left`.
///
/// Mirrors `Right` — same dimensions.
pub type LeftSidebarStyle = DefaultSidebarStyle;

/// Style preset for `SidebarRenderKind::WithTypeSelector`.
///
/// Adds a 32 px tab strip; all other dimensions match the default.
#[derive(Default)]
pub struct WithTypeSelectorStyle;

impl SidebarStyle for WithTypeSelectorStyle {
    fn header_height(&self)    -> f64 { 40.0  }
    fn tab_strip_height(&self) -> f64 { 32.0  }
    fn padding(&self)          -> f64 { 12.0  }
    fn resize_zone_width(&self) -> f64 { 8.0  }
    fn border_width(&self)     -> f64 { 1.0   }
    fn min_width(&self)        -> f64 { 280.0 }
    fn max_width(&self)        -> f64 { 4000.0 }
    fn default_width(&self)    -> f64 { 340.0 }
    fn scrollbar_width(&self)  -> f64 { 8.0   }
}

/// Style preset for `SidebarRenderKind::Embedded`.
///
/// No resize edge (ignored by layout since Embedded never registers a resize zone).
/// Slightly narrower default width to fit inside modals.
#[derive(Default)]
pub struct EmbeddedSidebarStyle;

impl SidebarStyle for EmbeddedSidebarStyle {
    fn header_height(&self)    -> f64 { 40.0  }
    fn tab_strip_height(&self) -> f64 { 32.0  }
    fn padding(&self)          -> f64 { 12.0  }
    fn resize_zone_width(&self) -> f64 { 0.0  }
    fn border_width(&self)     -> f64 { 0.0   }
    fn min_width(&self)        -> f64 { 280.0 }
    fn max_width(&self)        -> f64 { 4000.0 }
    fn default_width(&self)    -> f64 { 280.0 }
    fn scrollbar_width(&self)  -> f64 { 8.0   }
}
