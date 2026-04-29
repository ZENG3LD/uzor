//! Dropdown geometry parameters trait and default implementation.
//!
//! Style holds layout numbers only — no colours (those live in `DropdownTheme`).
//!
//! Default values ported from the mlc audit (`dropdown-deep.md` §6–§8).

// ---------------------------------------------------------------------------
// BackgroundFill
// ---------------------------------------------------------------------------

/// Selects how the dropdown panel background is filled.
///
/// Default is `Solid` — dropdowns are always opaque (no blur).
#[derive(Debug, Clone)]
pub enum BackgroundFill {
    /// Solid colour — uses `theme.bg()`.  Default for all dropdown kinds.
    Solid,

    /// Frosted-glass blur.  Included for completeness; dropdowns should use `Solid`.
    Glass {
        /// Blur kernel radius in pixels.
        blur_radius: f64,
    },

    /// Tiled texture fill.  Falls back to `Solid` until asset system is wired.
    Texture {
        /// Asset identifier used to resolve the texture.
        asset_id: &'static str,
    },
}

// ---------------------------------------------------------------------------
// DropdownStyle
// ---------------------------------------------------------------------------

/// Geometry parameters for the Dropdown composite.
///
/// All method defaults match mlc values from the deep audit.
pub trait DropdownStyle {
    // --- Panel geometry ---

    /// Frame corner radius.  Default: `4.0`.
    fn radius(&self) -> f64;

    /// Frame border width in pixels.  Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Outer padding (all four sides inside panel).  Default: `4.0`.
    fn padding(&self) -> f64;

    // --- Item rows ---

    /// Height per Item / Submenu row.  Default: `32.0`.
    fn item_height(&self) -> f64;

    /// Height per Header row.  Default: `28.0`.
    fn header_height(&self) -> f64;

    /// Height per Separator row (visual center gets the 1 px line).  Default: `9.0`.
    fn separator_height(&self) -> f64;

    /// Horizontal text/icon padding inside each item row.  Default: `12.0`.
    fn item_padding_x(&self) -> f64;

    // --- Typography ---

    /// Item label font size in pixels.  Default: `13.0`.
    fn font_size(&self) -> f64;

    /// Subtitle / shortcut font size in pixels.  Default: `12.0`.
    fn font_size_subtitle(&self) -> f64;

    // --- Icons ---

    /// Icon bounding box (square).  Default: `24.0`.
    fn icon_size(&self) -> f64;

    /// Gap between icon and label text.  Default: `6.0`.
    fn icon_text_gap(&self) -> f64;

    // --- Shadow ---

    /// Shadow rect offset `(x, y)`.  Default: `(2.0, 4.0)`.
    fn shadow_offset(&self) -> (f64, f64);

    // --- Toggle switch geometry ---

    /// Toggle pill track width.  Default: `36.0`.
    fn toggle_track_w(&self) -> f64;

    /// Toggle pill track height.  Default: `18.0`.
    fn toggle_track_h(&self) -> f64;

    /// Toggle thumb diameter.  Default: `14.0`.
    fn toggle_thumb_d(&self) -> f64;

    // --- Accent bar ---

    /// Left-edge accent bar width in pixels.  Default: `2.0`.
    fn accent_bar_w(&self) -> f64;

    /// Top + bottom inset for accent bar.  Default: `4.0`.
    fn accent_bar_inset_y(&self) -> f64;

    // --- Item hover ---

    /// Corner radius of the item hover background fill.  Default: `2.0`.
    fn item_hover_radius(&self) -> f64;

    // --- Submenu panel ---

    /// Gap between parent panel right edge and sibling submenu left edge.  Default: `2.0`.
    fn submenu_gap(&self) -> f64;

    // --- Scroll clipping ---

    /// Maximum number of items before the panel clips and scroll activates.
    /// `0` = no clip (unlimited height).  Default: `0`.
    fn max_visible_items(&self) -> usize;

    // --- Panel width ---

    /// Minimum panel width in pixels.  Default: `180.0`.
    fn min_width(&self) -> f64;

    // --- Grid template extras ---

    /// Square cell side for Grid / Grouped templates.  Default: `32.0`.
    fn cell_size(&self) -> f64;

    /// Gap between cells.  Default: `2.0`.
    fn cell_gap(&self) -> f64;

    // --- Grouped template extras ---

    /// Width of the left row-label column in the Grouped template.  Default: `16.0`.
    fn row_label_width(&self) -> f64;

    /// Stroke-only checkbox square size for Grouped list section.  Default: `14.0`.
    fn checkbox_size(&self) -> f64;

    // --- Background ---

    /// Background fill strategy.  Default: `BackgroundFill::Solid`.
    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }
}

// ---------------------------------------------------------------------------
// DefaultDropdownStyle
// ---------------------------------------------------------------------------

/// Default style preset — matches mlc dropdown geometry from the deep audit.
#[derive(Default)]
pub struct DefaultDropdownStyle;

impl DropdownStyle for DefaultDropdownStyle {
    // Panel geometry
    fn radius(&self)       -> f64 { 4.0 }
    fn border_width(&self) -> f64 { 1.0 }
    fn padding(&self)      -> f64 { 4.0 }

    // Item rows
    fn item_height(&self)      -> f64 { 32.0 }
    fn header_height(&self)    -> f64 { 28.0 }
    fn separator_height(&self) -> f64 { 9.0  }
    fn item_padding_x(&self)   -> f64 { 12.0 }

    // Typography
    fn font_size(&self)          -> f64 { 13.0 }
    fn font_size_subtitle(&self) -> f64 { 12.0 }

    // Icons
    fn icon_size(&self)     -> f64 { 24.0 }
    fn icon_text_gap(&self) -> f64 { 6.0  }

    // Shadow
    fn shadow_offset(&self) -> (f64, f64) { (2.0, 4.0) }

    // Toggle
    fn toggle_track_w(&self) -> f64 { 36.0 }
    fn toggle_track_h(&self) -> f64 { 18.0 }
    fn toggle_thumb_d(&self) -> f64 { 14.0 }

    // Accent bar
    fn accent_bar_w(&self)       -> f64 { 2.0 }
    fn accent_bar_inset_y(&self) -> f64 { 4.0 }

    // Item hover
    fn item_hover_radius(&self) -> f64 { 2.0 }

    // Submenu
    fn submenu_gap(&self) -> f64 { 2.0 }

    // Scroll
    fn max_visible_items(&self) -> usize { 0 }

    // Panel width
    fn min_width(&self) -> f64 { 180.0 }

    // Grid extras
    fn cell_size(&self) -> f64 { 32.0 }
    fn cell_gap(&self)  -> f64 { 2.0  }

    // Grouped extras
    fn row_label_width(&self) -> f64 { 16.0 }
    fn checkbox_size(&self)   -> f64 { 14.0 }
}
