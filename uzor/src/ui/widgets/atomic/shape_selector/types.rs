//! ShapeSelector widget types — view and render-kind enum.

/// Per-frame data for the `Shape` render kind.
///
/// A small square toggle button that contains a rendered shape icon.
/// The caller draws the shape itself via a closure passed to the render function.
pub struct ShapeSelectorView<'a> {
    /// `true` when this shape is the currently selected one.
    pub selected: bool,
    /// `true` when the pointer is hovering over this button.
    pub hovered: bool,
    /// Optional label rendered below the button.
    pub label: Option<&'a str>,
}

/// Per-frame data for the `ThemePreset` render kind.
///
/// An appearance-tab button that shows a color swatch preview on the left
/// and a theme name on the right.
pub struct ThemePresetView<'a> {
    /// Theme name label drawn to the right of the swatch.
    pub label: &'a str,
    /// Preview color displayed as an 18×18 square swatch.
    /// CSS color string (e.g. `"#1e222d"` or `"rgba(30,34,45,1)"`).
    pub preview_color: &'a str,
    /// Muted border color drawn around the swatch square.
    /// Matches mlc `muted_color` (e.g. `"rgba(0,0,0,0.3)"`).
    pub swatch_border_color: &'a str,
    /// `true` when this is the currently active theme preset.
    pub selected: bool,
    /// `true` when the pointer is hovering over this button.
    pub hovered: bool,
}

/// Per-frame data for the `UIStyle` render kind.
///
/// A text-only selector button for UI style choices (e.g. "Dark", "Light",
/// "High contrast").
pub struct UIStyleView<'a> {
    /// Label text displayed inside the button.
    pub label: &'a str,
    /// `true` when this is the currently active UI style.
    pub selected: bool,
    /// `true` when the pointer is hovering over this button.
    pub hovered: bool,
}

/// Selects the visual variant used by `draw_shape_selector`.
///
/// `Custom` is an escape hatch for app-supplied renderers.
pub enum ShapeSelectorRenderKind<'a> {
    /// Shape-icon selector button with caller-supplied draw_shape closure.
    /// Square toggle button; shape rendered in an inset inner rect.
    /// Ports section 34 from button-full.md (indicator_settings signals tab shape row).
    Shape,
    /// Appearance-tab preset selector: color swatch preview + theme name text.
    /// Ports section 39 from button-full.md (chart_settings appearance tab).
    ThemePreset,
    /// Text-only UI style selector button.
    /// Ports section 40 from button-full.md (chart_settings appearance tab).
    UIStyle,
    /// Caller-supplied renderer. Bypasses all built-in draw logic.
    Custom(Box<dyn Fn(&mut dyn crate::render::RenderContext, crate::types::Rect, crate::types::WidgetState, &super::settings::ShapeSelectorSettings) + 'a>),
}
