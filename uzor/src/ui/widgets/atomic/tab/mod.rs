//! Tab widget — single tab (composite with optional close-button child).

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

// Core types
pub use types::{TabConfig, TabKind, TabResponse};
pub use state::TabState;
pub use theme::{DefaultTabTheme, TabTheme};

// Style — generic trait + all variant presets
pub use style::{
    ChromeTabStyle, DefaultTabStyle, ModalHorizontalTabStyle, ModalSidebarTabStyle,
    TagsTabsSidebarTabStyle, TabStyle,
};

// Settings bundle
pub use settings::TabSettings;

// Render — generic + all variant-specific draw functions
pub use render::{
    draw_chrome_tab, draw_modal_horizontal_tab, draw_modal_sidebar_tab,
    draw_tags_tabs_sidebar_tab, draw_tab, draw_tab_variant, TabResult, TabView,
};

// Input registration helpers
pub use input::{
    register_chrome_tab, register_horizontal_tab, register_sidebar_tab, register_tab,
    register_tab_on_layer,
};
