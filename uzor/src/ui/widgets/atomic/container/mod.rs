//! Container widget — Plain / Bordered / Card / Clip / Section / Panel.

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod types;

pub use input::{
    register, register_clickable,
    register_input_coordinator_container,
    register_context_manager_container,
};
pub use render::{
    begin_clipping_container, draw_bordered_container, draw_card_container, draw_container,
    draw_panel_container, draw_plain_container, draw_section_container, end_clipping_container,
    ContainerView,
};
pub use settings::ContainerSettings;
pub use state::ContainerState;
pub use style::{
    BorderedContainerStyle, CardContainerStyle, ClippingContainerStyle, ContainerStyle,
    DefaultContainerStyle, PanelContainerStyle, PlainContainerStyle, SectionContainerStyle,
};
pub use theme::{ContainerTheme, DefaultContainerTheme};
pub use types::{ContainerType, PanelRole};
