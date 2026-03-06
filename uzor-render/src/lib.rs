//! Platform-agnostic rendering abstraction for uzor

mod context;
mod types;
mod helpers;
mod ops;
mod svg;

pub mod icons;

pub use context::{RenderContext, RenderContextExt};
pub use types::{TextAlign, TextBaseline};
pub use helpers::{crisp, crisp_rect};
pub use ops::{RenderOp, RenderOps, execute_ops};
pub use svg::{draw_svg_icon, draw_svg_icon_rotated};
