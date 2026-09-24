//! The one token contract: semantic colour/geometry roles, colour math, and
//! (from Brief 2 onward) the DTCG loader that resolves a JSON token file
//! into a widget-consumable [`crate::tokens::semantic::SemanticRoles`] +
//! [`crate::tokens::geometry::GeometryRoles`] set. See
//! `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` for the full
//! design (semantic role list, module layout, DTCG file shape, delivery to
//! widgets).
//!
//! H1 Brief 1 landed the colour math and the fixed role types
//! (`color`/`geometry`/`semantic`). Brief 2 (this) adds the DTCG loader:
//! `dtcg` (raw parse types, path walking, [`TokenLoadError`]) landed first;
//! `loader` (`load_token_set`, alias DFS with cycle detection) and `set`
//! (`TokenSet`/`Tokens`, the `app.*` and `component.*` surfaces) land here.
//! `builtin`/`theme` (the 4 built-in JSON sets, `TokenTheme`) land in later
//! briefs (§6).

pub mod color;
pub mod dtcg;
pub mod geometry;
pub mod loader;
pub mod semantic;
pub mod set;

pub use color::{ColorParseError, ColorValue, Rgba};
pub use dtcg::TokenLoadError;
pub use geometry::GeometryRoles;
pub use loader::load_token_set;
pub use semantic::{Role, SemanticRoles};
pub use set::{AppTokens, ColorSpec, ComponentOverrides, Modifier, TokenSet, TokenValue, Tokens};

/// The `component.*` keys a widget accepts, if it has registered any. No
/// widget has been converted to the token contract yet (that starts with
/// `button` in H1 Brief 4) — until a widget calls this with its own key
/// list, every `component.<widget>.*` path in a token file is rejected as
/// [`TokenLoadError::UnknownPath`]. The set of accepted component paths
/// grows only as widgets are converted; this function's body is the single
/// place that growth happens (one match arm added per converted widget).
pub(crate) fn component_keys(_widget: &str) -> Option<&'static [&'static str]> {
    None
}
