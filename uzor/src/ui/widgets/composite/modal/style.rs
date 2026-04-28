//! Modal geometry parameters trait and default implementation.
//!
//! Style holds layout numbers only — no colours (those live in `ModalTheme`).
//!
//! Default values ported from the mlc audit (`modal-deep.md` §7) and the
//! concrete modal implementations in
//! `mylittlechart/crates/chart/src/layout/modals/`.

/// Geometry parameters for the modal composite.
///
/// Implement this trait to customise sizes without touching colours.
pub trait ModalStyle {
    /// Frame corner radius.
    ///
    /// mlc: `0.0` for all settings modals (sharp corners), `8.0` for wizard.
    fn radius(&self) -> f64;

    /// Frame border width in pixels.  Default: `1.0`.
    fn border_width(&self) -> f64;

    /// Header zone height in pixels.  Default: `44.0`.
    ///
    /// mlc range: 36–44 px.  Largest value (IndicatorSettings, PresetNameInput) used here.
    fn header_height(&self) -> f64;

    /// Footer zone height in pixels.  Default: `52.0`.
    ///
    /// mlc range: 48–52 px.  Largest value (PresetNameInput) used here.
    fn footer_height(&self) -> f64;

    /// Sidebar width for `SideTabs`.  Default: `48.0`.
    ///
    /// mlc: 48 px (IndicatorSettings icon sidebar).
    fn sidebar_width(&self) -> f64;

    /// Inner body padding in pixels.  Default: `16.0`.
    fn padding(&self) -> f64;

    /// Horizontal tab strip height for `TopTabs`.  Default: `32.0`.
    ///
    /// mlc: `TAB_BAR_H = 32`.
    fn tab_height(&self) -> f64;

    /// Close-button bounding box size.  Default: `24.0`.
    ///
    /// mlc: 16–24 px icon zone.
    fn close_btn_size(&self) -> f64;

    /// Shadow rect offset (x and y).  Default: `3.0`.
    ///
    /// mlc: shadow drawn at `+3 px` right and down.
    fn shadow_offset(&self) -> f64;

    /// Shadow blur approximation (informational — not used by all renderers).
    /// Default: `6.0`.
    fn shadow_blur(&self) -> f64;

    /// Wizard bottom-nav zone height (page dots + Back/Next buttons).
    /// Default: `52.0`.
    fn wizard_nav_height(&self) -> f64;
}

// ---------------------------------------------------------------------------
// Default
// ---------------------------------------------------------------------------

/// Default style preset — matches mlc IndicatorSettings / PresetNameInput geometry.
#[derive(Default)]
pub struct DefaultModalStyle;

impl ModalStyle for DefaultModalStyle {
    fn radius(&self)          -> f64 { 0.0  }
    fn border_width(&self)    -> f64 { 1.0  }
    fn header_height(&self)   -> f64 { 44.0 }
    fn footer_height(&self)   -> f64 { 52.0 }
    fn sidebar_width(&self)   -> f64 { 48.0 }
    fn padding(&self)         -> f64 { 16.0 }
    fn tab_height(&self)      -> f64 { 32.0 }
    fn close_btn_size(&self)  -> f64 { 24.0 }
    fn shadow_offset(&self)   -> f64 { 3.0  }
    fn shadow_blur(&self)     -> f64 { 6.0  }
    fn wizard_nav_height(&self) -> f64 { 52.0 }
}
