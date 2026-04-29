//! Popup geometry parameters trait and default implementation.
//!
//! Style holds layout numbers only — no colours (those live in `PopupTheme`).
//!
//! Default values ported from the mlc audit (`popup-deep.md` §7).

// ---------------------------------------------------------------------------
// BackgroundFill
// ---------------------------------------------------------------------------

/// Selects how the popup frame background is filled.
///
/// Default is `Solid` (flat colour from `theme.bg()`).
/// `ItemList` uses `Solid` (dropdowns must be opaque).
/// `ColorPickerGrid` / `ColorPickerHsv` use `Glass` (frosted-glass blur).
/// `IndicatorStrip` uses `Solid` with alpha via the alpha-fill path.
#[derive(Debug, Clone)]
pub enum BackgroundFill {
    /// Solid colour — uses `theme.bg()`.
    Solid,

    /// Frosted-glass blur effect — renders a GPU blur of what's behind the
    /// popup plus `theme.bg()` at reduced alpha.
    ///
    /// Falls back to `Solid` on backends without blur support.
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
// PopupStyle
// ---------------------------------------------------------------------------

/// Geometry parameters for the popup composite.
pub trait PopupStyle {
    /// Frame corner radius.  Default: `4.0`.
    fn radius(&self) -> f64;

    /// Frame border width in pixels.  Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Inner content padding in pixels.  Default: `8.0`.
    fn padding(&self) -> f64;

    /// Shadow rect offset (x, y).  Default: `(2.0, 4.0)`.
    fn shadow_offset(&self) -> (f64, f64);

    // --- ColorPickerGrid (L1) ---

    /// Swatch size for the L1 color grid.  Default: `18.0`.
    fn swatch_size(&self) -> f64;

    /// Gap between swatches in the L1 grid.  Default: `2.0`.
    fn grid_gap(&self) -> f64;

    /// Swatch corner radius for the L1 grid.  Default: `2.0`.
    fn swatch_radius(&self) -> f64;

    /// Number of columns in the L1 palette grid.  Default: `10`.
    fn grid_columns(&self) -> usize;

    /// Height of the opacity row.  Default: `24.0`.
    fn opacity_row_height(&self) -> f64;

    // --- ColorPickerHsv (L2) ---

    /// SV square size.  Default: `180.0`.
    fn hsv_square_size(&self) -> f64;

    /// Hue bar width.  Default: `20.0`.
    fn hue_bar_width(&self) -> f64;

    /// Hex input row height.  Default: `32.0`.
    fn hex_row_height(&self) -> f64;

    /// Action button height (Back / Add).  Default: `28.0`.
    fn action_button_height(&self) -> f64;

    /// Gap between SV square and hue bar.  Default: `8.0`.
    fn hsv_inner_gap(&self) -> f64;

    // --- SwatchGrid (SyncColorGrid) ---

    /// Swatch size for the compact SwatchGrid.  Default: `20.0`.
    fn swatch_grid_size(&self) -> f64;

    /// Gap between swatches in the SwatchGrid.  Default: `3.0`.
    fn swatch_grid_gap(&self) -> f64;

    /// Number of columns in the SwatchGrid.  Default: `4`.
    fn swatch_grid_columns(&self) -> usize;

    /// Remove row height in the SwatchGrid.  Default: `22.0`.
    fn remove_row_height(&self) -> f64;

    // --- ItemList (Dropdown) ---

    /// Item row height.  Default: `32.0`.
    fn item_height(&self) -> f64;

    /// Separator row height.  Default: `9.0`.
    fn separator_height(&self) -> f64;

    /// Header row height.  Default: `28.0`.
    fn header_height(&self) -> f64;

    /// Minimum width for item list popup.  Default: `180.0`.
    fn min_width(&self) -> f64;

    // --- IndicatorStrip ---

    /// Row height for indicator strip rows.  Default: `20.0`.
    fn strip_row_height(&self) -> f64;

    /// Gap between indicator strip rows.  Default: `2.0`.
    fn strip_row_gap(&self) -> f64;

    /// Icon size in the indicator strip.  Default: `14.0`.
    fn strip_icon_size(&self) -> f64;

    // --- Background ---

    /// Background fill strategy.  Default: `BackgroundFill::Solid`.
    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }
}

// ---------------------------------------------------------------------------
// Default
// ---------------------------------------------------------------------------

/// Default style preset — matches mlc popup geometry from `popup-deep.md` §7.
#[derive(Default)]
pub struct DefaultPopupStyle;

impl PopupStyle for DefaultPopupStyle {
    fn radius(&self)         -> f64   { 4.0  }
    fn border_width(&self)   -> f64   { 1.0  }
    fn padding(&self)        -> f64   { 8.0  }
    fn shadow_offset(&self)  -> (f64, f64) { (2.0, 4.0) }

    // ColorPickerGrid (L1)
    fn swatch_size(&self)       -> f64   { 18.0 }
    fn grid_gap(&self)          -> f64   { 2.0  }
    fn swatch_radius(&self)     -> f64   { 2.0  }
    fn grid_columns(&self)      -> usize { 10   }
    fn opacity_row_height(&self)-> f64   { 24.0 }

    // ColorPickerHsv (L2)
    fn hsv_square_size(&self)       -> f64 { 180.0 }
    fn hue_bar_width(&self)         -> f64 { 20.0  }
    fn hex_row_height(&self)        -> f64 { 32.0  }
    fn action_button_height(&self)  -> f64 { 28.0  }
    fn hsv_inner_gap(&self)         -> f64 { 8.0   }

    // SwatchGrid
    fn swatch_grid_size(&self)    -> f64   { 20.0 }
    fn swatch_grid_gap(&self)     -> f64   { 3.0  }
    fn swatch_grid_columns(&self) -> usize { 4    }
    fn remove_row_height(&self)   -> f64   { 22.0 }

    // ItemList
    fn item_height(&self)     -> f64 { 32.0  }
    fn separator_height(&self)-> f64 { 9.0   }
    fn header_height(&self)   -> f64 { 28.0  }
    fn min_width(&self)       -> f64 { 180.0 }

    // IndicatorStrip
    fn strip_row_height(&self) -> f64 { 20.0 }
    fn strip_row_gap(&self)    -> f64 { 2.0  }
    fn strip_icon_size(&self)  -> f64 { 14.0 }
}
