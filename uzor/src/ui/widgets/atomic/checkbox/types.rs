//! Checkbox widget types — view, config, and render-kind enum.

/// Per-frame rendering inputs for `draw_checkbox`.
pub struct CheckboxView<'a> {
    /// Whether the checkbox is in its checked/enabled state.
    pub checked: bool,
    /// Optional label drawn to the right of the box.
    pub label: Option<&'a str>,
}

/// Static configuration for a checkbox instance.
#[derive(Debug, Clone)]
pub struct CheckboxConfig {
    /// Font string used to render the optional label.
    pub label_font: String,
}

impl Default for CheckboxConfig {
    fn default() -> Self {
        Self {
            label_font: "13px sans-serif".to_string(),
        }
    }
}

/// Selects the visual variant used by `draw_checkbox`.
///
/// Each variant maps to a specific mlc render pattern.
/// `Custom` is an escape hatch for app-supplied renderers.
pub enum CheckboxRenderKind<'a> {
    /// Standard checkbox with checkmark path (sections 21-23).
    /// Used in chart_settings, indicator_settings, primitive_settings.
    Standard,
    /// Visibility checkbox — same as Standard but smaller label gap (section 22).
    Visibility,
    /// Level-visibility — Standard with radius=2.0 (section 23).
    LevelVisibility,
    /// Notification-style: stroke-only outer, filled inner rect when checked (section 24).
    Notification,
    /// Cross mark instead of checkmark (reserve).
    Cross,
    /// Circle fill instead of checkmark (reserve).
    CircleCheck,
    /// Caller-supplied renderer.
    Custom(Box<dyn Fn(&mut dyn crate::render::RenderContext, crate::types::Rect, crate::types::WidgetState, &CheckboxView<'_>, &super::settings::CheckboxSettings) + 'a>),
}
