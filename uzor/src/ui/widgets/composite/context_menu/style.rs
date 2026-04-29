//! ContextMenu geometry parameters traits, presets, and background fill enum.
//!
//! Two presets:
//! - `DefaultContextMenuStyle` — 32 px items, blur background (~180 px wide).
//! - `MinimalContextMenuStyle` — 28 px items, solid background (~160 px wide).

// ---------------------------------------------------------------------------
// BackgroundFill
// ---------------------------------------------------------------------------

/// Selects how the context menu panel background is filled.
#[derive(Debug, Clone)]
pub enum BackgroundFill {
    /// Solid colour — uses `theme.bg()`.  Used by `Minimal`.
    Solid,

    /// Frosted-glass blur behind the panel.  Used by `Default`.
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
// ContextMenuStyle trait
// ---------------------------------------------------------------------------

/// Geometry parameters for the ContextMenu composite.
///
/// All method defaults match the `Default` preset (chart drawing tools style).
pub trait ContextMenuStyle {
    /// Frame corner radius.  Default: `4.0`.
    fn radius(&self) -> f64;

    /// Frame border width in pixels.  Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Outer padding on all four sides inside panel.  Default: `4.0`.
    fn padding(&self) -> f64;

    /// Height per item row.  Default (Default): `32.0`.  Minimal: `28.0`.
    fn item_height(&self) -> f64;

    /// Height of the separator row (1 px line drawn at vertical centre).  Default: `9.0`.
    fn separator_height(&self) -> f64;

    /// Horizontal padding inside each item row.  Default: `12.0`.
    fn item_padding_x(&self) -> f64;

    /// Icon bounding box (square).  Default: `16.0`.
    fn icon_size(&self) -> f64;

    /// Gap between icon right edge and label text.  Default: `8.0`.
    fn icon_text_gap(&self) -> f64;

    /// Minimum panel width in pixels.  Default (Default): `180.0`.  Minimal: `160.0`.
    fn min_width(&self) -> f64;

    /// Maximum panel width in pixels (`0.0` = unconstrained).  Default: `0.0`.
    fn max_width(&self) -> f64;

    /// Shadow rect offset `(x, y)`.  Default: `(3.0, 3.0)`.
    fn shadow_offset(&self) -> (f64, f64);

    /// Shadow alpha multiplier (0.0–1.0).  Used for documentation; actual alpha
    /// is baked into `theme.shadow()` colour string.  Default: `0.3`.
    fn shadow_alpha(&self) -> f64;

    /// Corner radius of the item hover background fill.  Default: `2.0`.
    fn item_hover_radius(&self) -> f64;

    /// Item label font size in pixels.  Default (Default): `13.0`.  Minimal: `12.0`.
    fn font_size(&self) -> f64;

    /// Background fill strategy.
    /// `Default` preset uses `Glass`, `Minimal` preset uses `Solid`.
    fn background_fill(&self) -> BackgroundFill;
}

// ---------------------------------------------------------------------------
// DefaultContextMenuStyle  (32 px / blur)
// ---------------------------------------------------------------------------

/// Preset for the chart drawing-tools context menu.
///
/// Features: icon column, separators, frosted-glass background, 32 px items.
#[derive(Default)]
pub struct DefaultContextMenuStyle;

impl ContextMenuStyle for DefaultContextMenuStyle {
    fn radius(&self)         -> f64 { 4.0 }
    fn border_width(&self)   -> f64 { 1.0 }
    fn padding(&self)        -> f64 { 4.0 }
    fn item_height(&self)    -> f64 { 32.0 }
    fn separator_height(&self) -> f64 { 9.0 }
    fn item_padding_x(&self) -> f64 { 12.0 }
    fn icon_size(&self)      -> f64 { 16.0 }
    fn icon_text_gap(&self)  -> f64 { 8.0 }
    fn min_width(&self)      -> f64 { 180.0 }
    fn max_width(&self)      -> f64 { 0.0 }
    fn shadow_offset(&self)  -> (f64, f64) { (3.0, 3.0) }
    fn shadow_alpha(&self)   -> f64 { 0.3 }
    fn item_hover_radius(&self) -> f64 { 2.0 }
    fn font_size(&self)      -> f64 { 13.0 }

    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Glass { blur_radius: 12.0 }
    }
}

// ---------------------------------------------------------------------------
// MinimalContextMenuStyle  (28 px / no blur)
// ---------------------------------------------------------------------------

/// Preset for the chrome-style minimal context menu.
///
/// Features: no icon column, no separators, solid background, 28 px items.
#[derive(Default)]
pub struct MinimalContextMenuStyle;

impl ContextMenuStyle for MinimalContextMenuStyle {
    fn radius(&self)         -> f64 { 4.0 }
    fn border_width(&self)   -> f64 { 1.0 }
    fn padding(&self)        -> f64 { 4.0 }
    fn item_height(&self)    -> f64 { 28.0 }
    fn separator_height(&self) -> f64 { 9.0 }
    fn item_padding_x(&self) -> f64 { 12.0 }
    fn icon_size(&self)      -> f64 { 16.0 }
    fn icon_text_gap(&self)  -> f64 { 8.0 }
    fn min_width(&self)      -> f64 { 160.0 }
    fn max_width(&self)      -> f64 { 0.0 }
    fn shadow_offset(&self)  -> (f64, f64) { (3.0, 3.0) }
    fn shadow_alpha(&self)   -> f64 { 0.3 }
    fn item_hover_radius(&self) -> f64 { 2.0 }
    fn font_size(&self)      -> f64 { 12.0 }

    fn background_fill(&self) -> BackgroundFill {
        BackgroundFill::Solid
    }
}
