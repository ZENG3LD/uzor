//! Toolbar geometry parameters trait, presets, and BackgroundFill.
//!
//! Style holds layout numbers only — no colours (those live in `ToolbarTheme`).
//!
//! Default values ported from the mlc audit (`toolbar-deep.md` §6–§8).

// ---------------------------------------------------------------------------
// BackgroundFill
// ---------------------------------------------------------------------------

/// Selects how the toolbar background is filled.
///
/// Default is `Solid` — toolbars are always opaque unless explicitly overridden.
#[derive(Debug, Clone)]
pub enum BackgroundFill {
    /// Solid colour — uses `theme.bg()`.  Default for all toolbar kinds.
    Solid,

    /// Frosted-glass blur.  Falls back to `Solid` on backends without blur.
    Glass {
        /// Blur kernel radius in pixels.
        blur_radius: f64,
    },

    /// Tiled texture fill.  Falls back to `Solid` until asset system is wired.
    Texture {
        /// Asset identifier used to resolve the texture.
        asset_id: &'static str,
    },

    /// Completely transparent — no background drawn.
    /// Used by `Inline` toolbars that render inside a panel.
    Transparent,
}

// ---------------------------------------------------------------------------
// ToolbarStyle
// ---------------------------------------------------------------------------

/// Geometry parameters for the Toolbar composite.
///
/// All method defaults match mlc values from the deep audit.
pub trait ToolbarStyle {
    // --- Bar dimensions ---

    /// Toolbar bar height in pixels (Horizontal / ChromeStrip / Inline).
    /// Default: `32.0`.
    fn height(&self) -> f64;

    /// Toolbar bar width in pixels (Vertical).  Default: `40.0`.
    fn width(&self) -> f64;

    // --- Item geometry ---

    /// Square item size in pixels (button bounding box).  Default: `28.0`.
    fn item_size(&self) -> f64;

    /// Icon bounding box inside the item (square).  Default: `16.0`.
    fn icon_size(&self) -> f64;

    /// Gap between adjacent items in the same section.  Default: `2.0`.
    fn item_spacing(&self) -> f64;

    /// Gap between two sections (start↔center, center↔end).  Default: `4.0`.
    fn section_gap(&self) -> f64;

    /// Horizontal padding at the outer edges of the toolbar.  Default: `4.0`.
    fn padding(&self) -> f64;

    // --- Item corner radius ---

    /// Corner radius of item hover / active backgrounds.  Default: `2.0`.
    fn item_radius(&self) -> f64;

    // --- Separator ---

    /// Separator line thickness in pixels.  Default: `1.0`.
    fn separator_thickness(&self) -> f64;

    /// Padding on each side of the separator along the main axis.  Default: `4.0`.
    fn separator_padding(&self) -> f64;

    // --- Scroll chevron ---

    /// Size of the overflow-scroll chevron button (square).  Default: `20.0`.
    fn scroll_chevron_size(&self) -> f64;

    // --- Split button ---

    /// Width of the chevron sub-zone in split buttons.  Default: `12.0`.
    fn split_chevron_width(&self) -> f64;

    // --- Color swatch ---

    /// Color swatch square size inside the button.  Default: `16.0`.
    fn color_swatch_size(&self) -> f64;

    /// Color swatch border width.  Default: `1.0`.
    fn color_swatch_border_width(&self) -> f64;

    // --- Typography ---

    /// Font size for text buttons and dropdown labels.  Default: `12.0`.
    fn font_size(&self) -> f64;

    /// Font size for clock and label items.  Default: `11.0`.
    fn font_size_small(&self) -> f64;

    // --- Background ---

    /// Background fill strategy.  Default: `BackgroundFill::Solid`.
    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

/// Default style for `Horizontal` toolbars (full-width strip, 32 px tall).
#[derive(Default)]
pub struct HorizontalToolbarStyle;

impl ToolbarStyle for HorizontalToolbarStyle {
    fn height(&self)                -> f64 { 32.0 }
    fn width(&self)                 -> f64 { 40.0 }
    fn item_size(&self)             -> f64 { 28.0 }
    fn icon_size(&self)             -> f64 { 16.0 }
    fn item_spacing(&self)          -> f64 { 2.0  }
    fn section_gap(&self)           -> f64 { 4.0  }
    fn padding(&self)               -> f64 { 4.0  }
    fn item_radius(&self)           -> f64 { 2.0  }
    fn separator_thickness(&self)   -> f64 { 1.0  }
    fn separator_padding(&self)     -> f64 { 4.0  }
    fn scroll_chevron_size(&self)   -> f64 { 20.0 }
    fn split_chevron_width(&self)   -> f64 { 12.0 }
    fn color_swatch_size(&self)     -> f64 { 16.0 }
    fn color_swatch_border_width(&self) -> f64 { 1.0 }
    fn font_size(&self)             -> f64 { 12.0 }
    fn font_size_small(&self)       -> f64 { 11.0 }
}

/// Default style for `Vertical` toolbars (sidebar column, 40 px wide).
#[derive(Default)]
pub struct VerticalToolbarStyle;

impl ToolbarStyle for VerticalToolbarStyle {
    fn height(&self)                -> f64 { 32.0 }
    fn width(&self)                 -> f64 { 40.0 }
    fn item_size(&self)             -> f64 { 32.0 }
    fn icon_size(&self)             -> f64 { 18.0 }
    fn item_spacing(&self)          -> f64 { 2.0  }
    fn section_gap(&self)           -> f64 { 4.0  }
    fn padding(&self)               -> f64 { 4.0  }
    fn item_radius(&self)           -> f64 { 4.0  }
    fn separator_thickness(&self)   -> f64 { 1.0  }
    fn separator_padding(&self)     -> f64 { 8.0  }
    fn scroll_chevron_size(&self)   -> f64 { 20.0 }
    fn split_chevron_width(&self)   -> f64 { 12.0 }
    fn color_swatch_size(&self)     -> f64 { 18.0 }
    fn color_swatch_border_width(&self) -> f64 { 1.0 }
    fn font_size(&self)             -> f64 { 12.0 }
    fn font_size_small(&self)       -> f64 { 11.0 }
}

/// Default style for `ChromeStrip` (window titlebar, 32 px tall).
#[derive(Default)]
pub struct ChromeStripStyle;

impl ToolbarStyle for ChromeStripStyle {
    fn height(&self)                -> f64 { 32.0 }
    fn width(&self)                 -> f64 { 40.0 }
    fn item_size(&self)             -> f64 { 28.0 }
    fn icon_size(&self)             -> f64 { 16.0 }
    fn item_spacing(&self)          -> f64 { 2.0  }
    fn section_gap(&self)           -> f64 { 8.0  }
    fn padding(&self)               -> f64 { 4.0  }
    fn item_radius(&self)           -> f64 { 2.0  }
    fn separator_thickness(&self)   -> f64 { 1.0  }
    fn separator_padding(&self)     -> f64 { 4.0  }
    fn scroll_chevron_size(&self)   -> f64 { 20.0 }
    fn split_chevron_width(&self)   -> f64 { 12.0 }
    fn color_swatch_size(&self)     -> f64 { 16.0 }
    fn color_swatch_border_width(&self) -> f64 { 1.0 }
    fn font_size(&self)             -> f64 { 12.0 }
    fn font_size_small(&self)       -> f64 { 11.0 }
    fn background_fill(&self) -> BackgroundFill { BackgroundFill::Transparent }
}

/// Default style for `Inline` toolbars (embedded inside a panel, 24 px tall).
#[derive(Default)]
pub struct InlineToolbarStyle;

impl ToolbarStyle for InlineToolbarStyle {
    fn height(&self)                -> f64 { 24.0 }
    fn width(&self)                 -> f64 { 32.0 }
    fn item_size(&self)             -> f64 { 20.0 }
    fn icon_size(&self)             -> f64 { 14.0 }
    fn item_spacing(&self)          -> f64 { 1.0  }
    fn section_gap(&self)           -> f64 { 2.0  }
    fn padding(&self)               -> f64 { 2.0  }
    fn item_radius(&self)           -> f64 { 2.0  }
    fn separator_thickness(&self)   -> f64 { 1.0  }
    fn separator_padding(&self)     -> f64 { 2.0  }
    fn scroll_chevron_size(&self)   -> f64 { 16.0 }
    fn split_chevron_width(&self)   -> f64 { 10.0 }
    fn color_swatch_size(&self)     -> f64 { 14.0 }
    fn color_swatch_border_width(&self) -> f64 { 1.0 }
    fn font_size(&self)             -> f64 { 11.0 }
    fn font_size_small(&self)       -> f64 { 10.0 }
    fn background_fill(&self) -> BackgroundFill { BackgroundFill::Transparent }
}

/// Alias used by `ToolbarSettings::default()`.
pub type DefaultToolbarStyle = HorizontalToolbarStyle;
