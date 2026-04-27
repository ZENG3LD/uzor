use super::state::TooltipState;
use super::style::{ChromeTooltipStyle, CrosshairTooltipStyle, DefaultTooltipStyle, TooltipStyle};
use super::theme::{ChromeTooltipTheme, CrosshairTooltipTheme, DefaultTooltipTheme, TooltipTheme};

/// Bundles theme + style for a tooltip.
///
/// Use `TooltipSettings::default()` for the generic variant, or one of the
/// named constructors to get an mlc-matched preset.
pub struct TooltipSettings {
    pub theme: Box<dyn TooltipTheme>,
    pub style: Box<dyn TooltipStyle>,
}

impl Default for TooltipSettings {
    fn default() -> Self {
        Self {
            theme: Box::<DefaultTooltipTheme>::default(),
            style: Box::new(DefaultTooltipStyle),
        }
    }
}

impl TooltipSettings {
    /// Settings for the mlc Chrome button tooltip (single-line, shadow, auto-flip).
    pub fn chrome() -> Self {
        Self {
            theme: Box::new(ChromeTooltipTheme),
            style: Box::new(ChromeTooltipStyle),
        }
    }

    /// Settings for the mlc Toolbar button tooltip (same renderer as Chrome).
    pub fn toolbar() -> Self {
        // Same visual as Chrome.
        Self::chrome()
    }

    /// Settings for the mlc OHLC crosshair tooltip (multi-line, border, clamp).
    pub fn crosshair() -> Self {
        Self {
            theme: Box::new(CrosshairTooltipTheme),
            style: Box::new(CrosshairTooltipStyle),
        }
    }
}

/// Pre-configured (settings + state) bundle for a specific mlc tooltip kind.
///
/// Keeps state and visual settings colocated so callers only hold one value.
pub struct TooltipPreset {
    pub settings: TooltipSettings,
    pub state: TooltipState,
}

impl TooltipPreset {
    /// mlc Chrome button tooltip — 500 ms delay, 150 ms fade.
    pub fn for_chrome() -> Self {
        Self { settings: TooltipSettings::chrome(), state: TooltipState::for_chrome() }
    }

    /// mlc Toolbar button tooltip — 700 ms delay, 150 ms fade.
    pub fn for_toolbar() -> Self {
        Self { settings: TooltipSettings::toolbar(), state: TooltipState::for_toolbar() }
    }

    /// mlc OHLC crosshair tooltip — no delay, no fade.
    pub fn for_crosshair() -> Self {
        Self { settings: TooltipSettings::crosshair(), state: TooltipState::for_crosshair() }
    }
}
