//! ColorSwatch theme trait — color contract for swatch rendering.

/// Theme trait for color swatch colors.
pub trait ColorSwatchTheme {
    /// Idle border drawn around a color swatch square.
    /// mlc: toolbar_theme.separator
    fn color_swatch_border(&self) -> &str;

    /// Outline drawn around the swatch when hovered or picker is open.
    /// mlc indicator_settings: toolbar_theme.item_bg_hover (expand rect fill color).
    fn color_swatch_hover_outline(&self) -> &str;

    /// Accent border color when the color picker is open (selected).
    /// mlc indicator_settings: toolbar_theme.accent
    fn color_swatch_selected_border(&self) -> &str;

    /// Light tile color for the transparency checkerboard background.
    /// mlc appearance tab: "#ffffff"
    fn transparency_checker_a(&self) -> &str;

    /// Dark tile color for the transparency checkerboard background.
    /// mlc appearance tab: "#cccccc"
    fn transparency_checker_b(&self) -> &str;

    /// Border color for the fill-toggle when fill is enabled (active state).
    /// mlc primitive_settings: toolbar_theme.item_bg_active
    fn fill_toggle_active_border(&self) -> &str;

    /// Diagonal strikethrough color for the fill-toggle when fill is disabled.
    /// mlc primitive_settings: toolbar_theme.separator
    fn fill_toggle_off_pattern_color(&self) -> &str;

    /// Background color used as the base for fill-toggle.
    /// mlc: toolbar_theme.background
    fn fill_toggle_background(&self) -> &str;

    /// Semi-transparent dark overlay drawn over a fill-toggle in the
    /// disabled state. H1 Brief 5 addition — previously an inline literal
    /// (`"rgba(0,0,0,0.35)"`) in `render.rs`, not reachable through this
    /// trait; same disabled-overlay role `radio`/`toggle` alias onto
    /// `backdrop.dim`'s black at a lower alpha.
    fn fill_toggle_disabled_overlay(&self) -> &str;
}

// =============================================================================
// Token-contract implementation
// =============================================================================
//
// `DefaultColorSwatchTheme` (a literal-colour prototype impl) was deleted in
// H1 Brief 5 — `crate::tokens::theme::TokenTheme` is now the one
// `ColorSwatchTheme` implementation ships, backed by
// `crate::ui::widgets::atomic::color_swatch::tokens::ColorSwatchTokens` (see
// `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` §3).
