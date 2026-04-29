//! Sidebar geometry parameters trait, presets, and `BackgroundFill`.
//!
//! Values ported from the mlc sidebar audit (`sidebar-deep.md` §9).

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
