//! Tooltip widget — text popup with configurable show delay and fade-in.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{TooltipConfig, TooltipPosition, TooltipResponse};
pub use state::TooltipState;
pub use theme::{DefaultTooltipTheme, TooltipTheme};
pub use style::{DefaultTooltipStyle, TooltipStyle};
pub use settings::TooltipSettings;
pub use render::{draw_tooltip, tooltip_rect_from_anchor};
pub use input::register_tooltip;
