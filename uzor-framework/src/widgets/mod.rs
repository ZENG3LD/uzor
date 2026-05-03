//! Tier-organised widget registration shortcuts.
//!
//! Three short namespaces re-exporting widget registration functions per
//! access level.  The verb in the function name signals what it does, the
//! module signals which manager owns the registration.
//!
//! - [`coord`] — L1, raw `InputCoordinator` entry. `register_X(coord, ...)`. No drawing.
//! - [`ctx`] — L2, `ContextManager` paint+register. `draw_X(ctx, render, ...)`.
//! - [`lm`] — L3, `LayoutManager` declarative API. `build_X(layout, render, ...)`.
//!
//! L3 framework apps should only need `lm::*`.  L1/L2 are for blackbox handler
//! bodies that paint their own subtree.
//!
//! # Where these live
//!
//! Re-exports live here in `uzor-framework` (not in core `uzor`) because they
//! are the recommended app-facing surface — `uzor` core stays minimal and
//! provides the long names (`register_layout_manager_*` etc.) for internal /
//! legacy callers that pin the older API.

pub mod coord;
pub mod ctx;
pub mod lm;
