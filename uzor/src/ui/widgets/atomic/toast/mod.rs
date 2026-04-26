//! Toast widget — Info / Success / Warning / Error.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{ToastSeverity, ToastType};
pub use state::ToastState;
pub use theme::{DefaultToastTheme, ToastTheme};
pub use style::{DefaultToastStyle, ToastStyle};
pub use settings::ToastSettings;
pub use render::{draw_toast, ToastView};
pub use input::register;
