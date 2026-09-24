//! Popup colour palette trait; token-contract default implementation lives
//! in `crate::tokens::theme::TokenTheme`, backed by `super::tokens`.

/// Colour tokens for the popup composite.
///
/// Implement this trait on your app theme struct to plug in custom colours.
pub trait PopupTheme {
    // --- Frame ---

    /// Popup background fill.  Default: `#1e222d`.
    fn bg(&self) -> &str;

    /// Frame border (1 px stroke).  Default: `#363a45`.
    fn border(&self) -> &str;

    /// Shadow rect fill.  Default: `rgba(0,0,0,0.4)`.
    fn shadow(&self) -> &str;

    // --- Items (ItemList / IndicatorStrip) ---

    /// Normal item background.  Default: transparent (same as frame bg).
    fn item_bg_normal(&self) -> &str;

    /// Hovered item background.  Default: `#2a2e39`.
    fn item_bg_hover(&self) -> &str;

    /// Selected / active item background.  Default: `rgba(41,98,255,0.15)`.
    fn item_bg_selected(&self) -> &str;

    /// Normal item text colour.  Default: `#d1d4dc`.
    fn item_text(&self) -> &str;

    /// Hovered item text colour.  Default: `#ffffff`.
    fn item_text_hover(&self) -> &str;

    /// Disabled item text colour.  Default: `#6a6d78`.
    fn item_text_disabled(&self) -> &str;

    /// Danger item text colour.  Default: `#f23645`.
    fn item_text_danger(&self) -> &str;

    /// Danger item background on hover.  Default: `rgba(242,54,69,0.15)`.
    fn item_bg_danger_hover(&self) -> &str;

    /// Section header text colour.  Default: `#ffffff`.
    fn header_text(&self) -> &str;

    /// Separator line colour.  Default: `#363a45`.
    fn separator(&self) -> &str;

    // --- Hex input (ColorPickerHsv) ---

    /// Hex input field background.  Default: `#2a2e39`.
    fn hex_input_bg(&self) -> &str;

    /// Hex input text colour.  Default: `#d1d4dc`.
    fn hex_input_text(&self) -> &str;

    /// Hex input border (focused).  Default: `#2962ff`.
    fn hex_input_border_focus(&self) -> &str;

    // --- HSV / color picker ---

    /// Hue bar gradient — provided as CSS; renderers may ignore this and draw
    /// a native rainbow gradient.  Default: `"rainbow"`.
    fn hsv_indicator(&self) -> &str;

    /// Accent colour for sliders, active borders, Add button.  Default: `#2962ff`.
    fn accent(&self) -> &str;

    // --- Backdrop ---

    /// Backdrop dim fill used for `BackdropKind::Dim`.  Default: `rgba(0,0,0,0.45)`.
    fn backdrop_dim(&self) -> &str;

    // --- Generic colour-picker grid (`input::register_popup_grid`) ---

    /// Hover halo painted behind a hovered cell in a generic swatch/colour
    /// grid.  Default alias: `text.on_accent`.
    fn grid_hover_halo(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Token-contract implementation
// ---------------------------------------------------------------------------
//
// `DefaultPopupTheme` (a literal-colour prototype impl) was deleted in H1
// Brief 8b — `crate::tokens::theme::TokenTheme` is now the one `PopupTheme`
// implementation this crate ships, backed by
// `crate::ui::widgets::composite::popup::tokens::PopupTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
