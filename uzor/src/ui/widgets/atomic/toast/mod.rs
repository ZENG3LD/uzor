//! Toast widget — Info / Success / Warning / Error, with mlc render parity.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{ToastSeverity, ToastType};
pub use state::{ToastEntry, ToastStackState};
pub use theme::{DefaultToastTheme, ToastTheme};
pub use style::{DefaultToastStyle, ToastGeometry, ToastStyle};
pub use settings::ToastSettings;
pub use render::{alpha_for, draw_toast, draw_toast_at, draw_toast_stack};
pub use input::register;
