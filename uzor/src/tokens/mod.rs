//! The one token contract: semantic colour/geometry roles, colour math, and
//! (from Brief 2 onward) the DTCG loader that resolves a JSON token file
//! into a widget-consumable [`crate::tokens::semantic::SemanticRoles`] +
//! [`crate::tokens::geometry::GeometryRoles`] set. See
//! `docs/uzor/plans/h1-token-contract-design-2026-09-24.md` for the full
//! design (semantic role list, module layout, DTCG file shape, delivery to
//! widgets).
//!
//! H1 Brief 1 lands the colour math and the fixed role types only —
//! `dtcg`/`loader`/`set`/`builtin`/`theme` (raw parsing, alias resolution,
//! `Tokens`/`TokenSet`, the 4 built-in JSON sets, `TokenTheme`) land in
//! later briefs (§6).

pub mod color;
pub mod geometry;
pub mod semantic;

pub use color::{ColorParseError, ColorValue, Rgba};
pub use geometry::GeometryRoles;
pub use semantic::{Role, SemanticRoles};
