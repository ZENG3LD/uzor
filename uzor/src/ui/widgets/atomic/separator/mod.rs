//! Separator / resize handle widget.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{SeparatorOrientation, SeparatorType};
pub use state::SeparatorDragState;
pub use theme::{DefaultSeparatorTheme, SeparatorTheme};
pub use style::{DefaultSeparatorStyle, SeparatorStyle};
pub use settings::SeparatorSettings;
pub use render::{draw_separator, SeparatorView};
pub use input::register;
