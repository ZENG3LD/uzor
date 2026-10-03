//! Atomic Text widget — read-only label with proper align/baseline/clip
//! handling. Saves/restores RenderContext text state internally so callers
//! don't have to manage `TextAlign`/`TextBaseline` manually.

pub mod input;
pub mod render;
pub mod settings;
pub mod state;
pub mod style;
pub mod theme;
pub mod tokens;
pub mod types;

pub use input::{
    register_input_coordinator_text};
pub use render::{draw_text, draw_text_with_selection};
pub use settings::TextSettings;
pub use state::TextState;
pub use style::{DefaultTextStyle, TextStyle};
pub use theme::TextTheme;
pub use types::{TextOverflow, TextRenderKind, TextView};
