//! Tooltip colour palette.

pub trait TooltipTheme {
    fn bg(&self)     -> &str;
    fn border(&self) -> &str;
    fn text(&self)   -> &str;
    /// Drop-shadow colour.  Only used when `TooltipStyle::has_shadow()` returns `true`.
    /// Matches mlc hardcoded `"#00000060"`.
    fn shadow(&self) -> &str { "#00000060" }
}

#[derive(Default)]
pub struct DefaultTooltipTheme;

impl TooltipTheme for DefaultTooltipTheme {
    fn bg(&self)     -> &str { "#2a2a2a" }
    fn border(&self) -> &str { "#3a3a3a" }
    fn text(&self)   -> &str { "#ffffff" }
}

/// Theme matching mlc chrome/toolbar tooltip defaults.
///
/// In mlc these are synced from `theme.colors.button_bg` / `theme.colors.text_primary`
/// each frame.  The static defaults here match the mlc struct defaults before first sync.
/// Callers that integrate with a live theme should supply a custom `TooltipTheme` impl.
#[derive(Default)]
pub struct ChromeTooltipTheme;

impl TooltipTheme for ChromeTooltipTheme {
    fn bg(&self)     -> &str { "#323232" }
    fn border(&self) -> &str { "" }        // No border on chrome variant — shadow only.
    fn text(&self)   -> &str { "#ffffff" }
    fn shadow(&self) -> &str { "#00000060" }
}

/// Theme matching mlc OHLC crosshair tooltip.
///
/// In mlc these are assembled in `draw_chart_tooltip()` from chart theme:
/// `bg = theme.background + "ee"`, `text = theme.text`, `border = theme.scale_border`.
/// Static defaults here match the mlc `Tooltip::default()` values.
/// Callers integrating a chart theme should supply a custom impl.
#[derive(Default)]
pub struct CrosshairTooltipTheme;

impl TooltipTheme for CrosshairTooltipTheme {
    fn bg(&self)     -> &str { "#1e222dee" }
    fn border(&self) -> &str { "#2a2e39" }
    fn text(&self)   -> &str { "#b2b5be" }
}
