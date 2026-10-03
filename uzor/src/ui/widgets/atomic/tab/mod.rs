//! Tab widget — single tab (composite with optional close-button child).
//!
//! - `theme`  — `TabTheme` trait.
//! - `tokens` — `TabTokens`: the token-contract implementation of
//!              `TabTheme`, one pre-rendered value per owned method, built
//!              from `SemanticRoles` + optional `component.tab.*` overrides
//!              (H1 token contract design §3). `TokenTheme`
//!              (`crate::tokens::theme`) implements `TabTheme` by reading
//!              these fields (and, for pure-delegator methods, by falling
//!              through to the trait's own default body — see `tokens.rs`'s
//!              module doc).

pub mod types;
pub mod state;
pub mod theme;
pub mod tokens;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

// Core types
pub use types::{TabConfig, TabKind, TabResponse};
pub use state::TabState;
pub use theme::TabTheme;

// Style — generic trait + all variant presets
pub use style::{
    ChromeTabStyle, DefaultTabStyle, ModalHorizontalTabStyle, ModalSidebarTabStyle,
    TagsTabsSidebarTabStyle, TabStyle};

// Settings bundle
pub use settings::TabSettings;

// Render — generic + all variant-specific draw functions
pub use render::{
    draw_chrome_tab, draw_modal_horizontal_tab, draw_modal_sidebar_tab,
    draw_tags_tabs_sidebar_tab, draw_tab, draw_tab_variant, TabResult, TabView};

// Input registration helpers
pub use input::{
    register_chrome_tab, register_horizontal_tab, register_sidebar_tab, register_tab,
    register_tab_on_layer,
    register_input_coordinator_tab};
