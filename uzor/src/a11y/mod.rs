//! Accessibility node types — AccessKit-shaped, dependency-free.
//!
//! Always-on (no feature gate): per the master plan, every L0 widget is
//! meant to eventually emit one of these nodes in production, the same way
//! `draw_button` already does for painting. Only the *query/assert* test
//! helper (`A11yQuery`, part of `uzor::testing`) is test-only.

mod action;
mod node;
mod role;
mod tree;

pub use action::A11yAction;
pub use node::A11yNode;
pub use role::A11yRole;
pub use tree::A11yTree;
