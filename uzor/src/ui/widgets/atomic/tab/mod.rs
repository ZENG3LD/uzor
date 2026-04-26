//! Tab widget — single tab (composite with optional close-button child).

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{TabConfig, TabResponse};
pub use state::TabState;
pub use theme::{DefaultTabTheme, TabTheme};
pub use style::{DefaultTabStyle, TabStyle};
pub use settings::TabSettings;
pub use render::{draw_tab, TabResult, TabView};
pub use input::{register_tab, register_tab_on_layer};
