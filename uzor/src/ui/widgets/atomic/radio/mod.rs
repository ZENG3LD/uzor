//! Radio widget — Group, Pair, and Dot variants.
//!
//! Self-contained:
//! - `types`    — `RadioOption`, `RadioGroupView`, `RadioPairView`, `RadioDotView`,
//!                `DotShape`, `RadioConfig`, `RadioRenderKind`.
//! - `state`    — `RadioState` (selected_idx).
//! - `theme`    — `RadioTheme` trait + `DefaultRadioTheme`.
//! - `style`    — `RadioStyle` / `DefaultRadioStyle` + `RadioPairStyle` / `DefaultRadioPairStyle`.
//! - `settings` — `RadioSettings` bundle.
//! - `render`   — `draw_radio` dispatcher.
//! - `input`    — `register_radio` helper.

pub mod types;
pub mod state;
pub mod theme;
pub mod style;
pub mod settings;
pub mod render;
pub mod input;

pub use types::{
    DotShape, RadioConfig, RadioDotView, RadioGroupView, RadioOption, RadioPairView,
    RadioRenderKind,
};
pub use state::RadioState;
pub use theme::{DefaultRadioTheme, RadioTheme};
pub use style::{DefaultRadioPairStyle, DefaultRadioStyle, RadioPairStyle, RadioStyle};
pub use settings::RadioSettings;
pub use render::draw_radio;
pub use input::register_radio;
