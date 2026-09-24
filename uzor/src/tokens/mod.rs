//! The one token contract: semantic colour/geometry roles, colour math, and
//! (from Brief 2 onward) the DTCG loader that resolves a JSON token file
//! into a widget-consumable [`crate::tokens::semantic::SemanticRoles`] +
//! [`crate::tokens::geometry::GeometryRoles`] set. See
//! `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` for the full
//! design (semantic role list, module layout, DTCG file shape, delivery to
//! widgets).
//!
//! H1 Brief 1 landed the colour math and the fixed role types
//! (`color`/`geometry`/`semantic`). Brief 2 added the DTCG loader: `dtcg`
//! (raw parse types, path walking, [`TokenLoadError`]), `loader`
//! (`load_token_set`, alias DFS with cycle detection), and `set`
//! (`TokenSet`/`Tokens`, the `app.*` and `component.*` surfaces). Brief 3
//! added `builtin`: the 4 generic built-in JSON sets and [`Tokens::builtin`].
//! Brief 4 (this) adds `theme` ([`theme::TokenTheme`], the one type that
//! implements every converted widget's colour-theme trait) and
//! `component_macro` (the `component_tokens!` scaffold each widget's own
//! `tokens.rs` uses — `ui::widgets::atomic::button::tokens` is the worked
//! example).

mod component_macro;
pub mod builtin;
pub mod color;
pub mod dtcg;
pub mod geometry;
pub mod loader;
pub mod semantic;
pub mod set;
pub mod theme;

pub(crate) use component_macro::component_tokens;
pub use builtin::BuiltinSet;
pub use color::{ColorParseError, ColorValue, Rgba};
pub use dtcg::TokenLoadError;
pub use geometry::GeometryRoles;
pub use loader::load_token_set;
pub use semantic::{Role, SemanticRoles};
pub use set::{AppTokens, ColorSpec, ComponentOverrides, Modifier, TokenSet, TokenValue, Tokens};
pub use theme::TokenTheme;

/// The `component.*` keys a widget accepts, if it has registered any. Until
/// a widget calls this with its own key list, every `component.<widget>.*`
/// path in a token file is rejected as [`TokenLoadError::UnknownPath`]. The
/// set of accepted component paths grows only as widgets are converted;
/// this function's body is the single place that growth happens (one match
/// arm added per converted widget).
pub(crate) fn component_keys(widget: &str) -> Option<&'static [&'static str]> {
    match widget {
        "button" => Some(crate::ui::widgets::atomic::button::tokens::BUTTON_KEYS),
        "checkbox" => Some(crate::ui::widgets::atomic::checkbox::tokens::CHECKBOX_KEYS),
        "chevron" => Some(crate::ui::widgets::atomic::chevron::tokens::CHEVRON_KEYS),
        "clock" => Some(crate::ui::widgets::atomic::clock::tokens::CLOCK_KEYS),
        _ => None,
    }
}
