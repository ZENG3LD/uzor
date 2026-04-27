//! Tooltip geometry.

pub trait TooltipStyle {
    fn radius(&self)           -> f64;
    fn padding_x(&self)        -> f64;
    fn padding_y(&self)        -> f64;
    fn font_size(&self)        -> f64;
    fn border_width(&self)     -> f64;
    fn anchor_gap(&self)       -> f64;
    fn fade_duration_ms(&self) -> f64;
    /// Whether to draw a 1-px drop shadow (Chrome/Toolbar variant only).
    fn has_shadow(&self)       -> bool { false }
    /// Whether to flip the box when near screen edges (vs. clamp-only).
    fn auto_flip(&self)        -> bool { false }
    /// Whether the tooltip may contain multiple lines.
    fn multi_line(&self)       -> bool { false }
    /// Minimum content width for multi-line layout (crosshair variant).
    fn min_content_width(&self) -> f64 { 0.0 }
    /// Line-height multiplier for multi-line layout.
    fn line_height_factor(&self) -> f64 { 1.4 }
}

pub struct DefaultTooltipStyle;

impl Default for DefaultTooltipStyle {
    fn default() -> Self {
        Self
    }
}

impl TooltipStyle for DefaultTooltipStyle {
    fn radius(&self)           -> f64 { 4.0 }
    fn padding_x(&self)        -> f64 { 8.0 }
    fn padding_y(&self)        -> f64 { 4.0 }
    fn font_size(&self)        -> f64 { 12.0 }
    fn border_width(&self)     -> f64 { 1.0 }
    fn anchor_gap(&self)       -> f64 { 6.0 }
    fn fade_duration_ms(&self) -> f64 { 150.0 }
}

/// Style matching mlc `chrome.rs:render_tooltip_themed`.
///
/// - 12 px font, 6 px padding, 24 px fixed height, 4 px radius
/// - 1 px drop-shadow at offset (1, 1), colour `#00000060`
/// - 20 px anchor gap (below cursor)
/// - Auto-flip at screen edges
/// - Fade-in 150 ms after 500 ms delay (configured in `TooltipState::for_chrome`)
pub struct ChromeTooltipStyle;

impl Default for ChromeTooltipStyle {
    fn default() -> Self { Self }
}

impl TooltipStyle for ChromeTooltipStyle {
    fn radius(&self)            -> f64 { 4.0 }
    /// Horizontal padding: 6 px each side.
    fn padding_x(&self)         -> f64 { 6.0 }
    /// Vertical padding: produces 24 px total height (font 12 + 2×6).
    fn padding_y(&self)         -> f64 { 6.0 }
    fn font_size(&self)         -> f64 { 12.0 }
    /// Chrome variant has no border stroke — shadow only.
    fn border_width(&self)      -> f64 { 0.0 }
    /// 20 px below cursor, 0 px horizontal shift.
    fn anchor_gap(&self)        -> f64 { 20.0 }
    fn fade_duration_ms(&self)  -> f64 { 150.0 }
    fn has_shadow(&self)        -> bool { true }
    fn auto_flip(&self)         -> bool { true }
    fn multi_line(&self)        -> bool { false }
}

/// Style matching mlc `overlays.rs:draw_tooltip` (OHLC crosshair).
///
/// - 11 px font, 8 px padding, 1.4× line-height, 4 px radius, 1 px border
/// - No shadow, no fade, no flip (clamp only)
/// - Minimum content width 80 px
pub struct CrosshairTooltipStyle;

impl Default for CrosshairTooltipStyle {
    fn default() -> Self { Self }
}

impl TooltipStyle for CrosshairTooltipStyle {
    fn radius(&self)             -> f64 { 4.0 }
    fn padding_x(&self)          -> f64 { 8.0 }
    fn padding_y(&self)          -> f64 { 8.0 }
    fn font_size(&self)          -> f64 { 11.0 }
    fn border_width(&self)       -> f64 { 1.0 }
    /// Offset from crosshair position (both axes), matches mlc default `offset_x/y = 10`.
    fn anchor_gap(&self)         -> f64 { 10.0 }
    /// Crosshair tooltip has no fade — always full opacity.
    fn fade_duration_ms(&self)   -> f64 { 0.0 }
    fn has_shadow(&self)         -> bool { false }
    fn auto_flip(&self)          -> bool { false }
    fn multi_line(&self)         -> bool { true }
    fn min_content_width(&self)  -> f64 { 80.0 }
    fn line_height_factor(&self) -> f64 { 1.4 }
}
