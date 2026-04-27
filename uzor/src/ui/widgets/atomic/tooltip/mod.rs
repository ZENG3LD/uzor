//! Tooltip widget — text popup with configurable show delay and fade-in.
//!
//! Two mlc-matched render variants plus the generic back-compat one:
//! - `draw_chrome_tooltip`    — single-line, shadow, auto-flip (Chrome/Toolbar)
//! - `draw_crosshair_tooltip` — multi-line, border, clamp-only (OHLC crosshair)
//! - `draw_tooltip`           — generic uzor tooltip (back-compat)

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{TooltipConfig, TooltipKind, TooltipPosition, TooltipResponse};
pub use state::{
    TooltipState, CHROME_SHOW_DELAY_MS, CROSSHAIR_SHOW_DELAY_MS, FADE_IN_DURATION_MS,
    TOOLBAR_SHOW_DELAY_MS,
};
pub use theme::{
    ChromeTooltipTheme, CrosshairTooltipTheme, DefaultTooltipTheme, TooltipTheme,
};
pub use style::{
    ChromeTooltipStyle, CrosshairTooltipStyle, DefaultTooltipStyle, TooltipStyle,
};
pub use settings::{TooltipPreset, TooltipSettings};
pub use render::{
    draw_chrome_tooltip, draw_crosshair_tooltip, draw_tooltip, tooltip_multiline_rect,
    tooltip_rect_from_anchor,
};
pub use input::register_tooltip;
