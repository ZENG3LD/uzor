//! Toggle widget types — view, config, and render-kind enum.

use crate::types::IconId;

/// Per-frame rendering inputs for `draw_toggle`.
pub struct ToggleView<'a> {
    /// `true` → ON state (thumb right, accent track).
    pub toggled: bool,
    /// Optional label drawn to the right of the track.
    pub label: Option<&'a str>,
    /// When `true` renders with disabled overlay wash.
    pub disabled: bool,
}

/// Static configuration for a toggle instance.
#[derive(Debug, Clone)]
pub struct ToggleConfig {
    /// Font string used to render the optional label.
    pub label_font: String,
}

impl Default for ToggleConfig {
    fn default() -> Self {
        Self {
            label_font: "13px sans-serif".to_string(),
        }
    }
}

/// Selects the visual variant used by `draw_toggle`.
///
/// Each variant maps to a specific mlc render pattern.
/// `Custom` is an escape hatch for app-supplied renderers.
pub enum ToggleRenderKind<'a> {
    /// iOS-style pill track + circular thumb (44×22).
    /// Maps to section 25 — `indicator_settings.rs` Bool param.
    Switch,
    /// Same pill shape with slightly different thumb padding (thumb_radius=9, padding=2).
    /// Maps to section 26 — signals enable/disable.
    SwitchWide,
    /// Swap between two icons (off / on). No track drawn.
    /// Maps to section 5 — Eye/EyeOff, Lock/Unlock.
    IconSwap {
        icon_off: &'a IconId,
        icon_on: &'a IconId,
    },
    /// Caller-supplied renderer. Bypasses all built-in draw logic.
    Custom(Box<dyn Fn(&mut dyn crate::render::RenderContext, crate::types::Rect, crate::types::WidgetState, &ToggleView<'_>, &super::settings::ToggleSettings)>),
}
