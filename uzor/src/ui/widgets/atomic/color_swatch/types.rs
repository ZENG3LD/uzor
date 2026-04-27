//! ColorSwatch widget types — view and render-kind enum.

/// Per-frame rendering inputs for `draw_color_swatch`.
pub struct ColorSwatchView<'a> {
    /// RGBA color of the swatch.  `[r, g, b, a]` each 0-255.
    pub color: [u8; 4],
    /// Whether the pointer is currently over this swatch.
    pub hovered: bool,
    /// Whether the color picker for this swatch is open (selected state).
    pub selected: bool,
    /// When `true` draws a two-tile checkerboard behind the color fill so
    /// that semi-transparent colors are visually legible.
    /// Set this for the appearance-tab variant (section 29).
    pub show_transparency: bool,
    /// Optional CSS-color override for the border.  `None` uses theme default.
    /// Pass `Some("rgba(0,0,0,0.4)")` to match the "muted_color" used in
    /// section 29 without exposing a new theme slot.
    pub border_color_override: Option<&'a str>,
}

/// Per-frame rendering inputs for `draw_fill_toggle`.
pub struct FillToggleView {
    /// `true` → fill is enabled; shows color fill + active border.
    /// `false` → fill is disabled; shows toolbar bg + diagonal strikethrough.
    pub filled: bool,
    /// RGBA fill color displayed when `filled = true`.  `[r, g, b, a]` 0-255.
    pub color: [u8; 4],
    /// When `true` applies a semi-transparent dark overlay (disabled state).
    pub disabled: bool,
}

/// Selects the visual variant used by `draw_color_swatch`.
///
/// `Custom` is an escape hatch for app-supplied renderers.
pub enum ColorSwatchRenderKind<'a> {
    /// Simple color fill, no checkerboard (sections 27, 28, 30).
    Simple,
    /// Color fill with checkerboard background for transparency preview (section 29).
    WithTransparency,
    /// Indicator-style swatch (20×20, hover-expand, sharp corners) (section 28).
    Indicator,
    /// Primitive-level swatch (16 px wide, variable height) (section 30).
    Primitive,
    /// Fill-toggle: shows color when filled, diagonal strikethrough when off (section 31).
    FillToggle,
    /// Caller-supplied renderer. Bypasses all built-in draw logic.
    Custom(Box<dyn Fn(&mut dyn crate::render::RenderContext, crate::types::Rect, crate::types::WidgetState, &ColorSwatchView<'_>, &super::settings::ColorSwatchSettings) + 'a>),
}
