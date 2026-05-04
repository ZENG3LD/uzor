//! Flex layout helpers used by the `view!` macro to convert a parent rect
//! plus a list of child specs into per-child rects.
//!
//! This is a thin pure-function wrapper around the same primitive maths the
//! core `app_context::layout` engine uses — kept here to avoid pulling the
//! retained-mode tree machinery in for trivial macro-driven layouts.
//!
//! Algorithm: 1D flex along the main axis.
//! - Each child has `basis: f64` (preferred size) and `flex: f64` (grow weight).
//! - Free space = main_axis_len - 2*pad - sum(basis) - gap*(n-1).
//! - If free > 0, distribute proportional to flex weights.
//! - If free < 0, basis values are kept (children may overflow — caller's
//!   problem; later we can shrink proportionally).

use crate::core::types::Rect;

#[derive(Copy, Clone, Debug)]
pub enum FlexDir {
    Row,
    Col,
}

#[derive(Copy, Clone, Debug)]
pub struct FlexChild {
    /// Preferred size along the main axis (px). 0 means "purely flex-driven".
    pub basis: f64,
    /// Flex grow weight. Children with flex=0 stay at `basis`.
    pub flex:  f64,
}

/// Solve a 1-D flex layout. Returns one `Rect` per child in declaration order.
pub fn flex_solve(parent: Rect, dir: FlexDir, gap: f64, pad: f64, children: &[FlexChild]) -> Vec<Rect> {
    let n = children.len();
    if n == 0 {
        return Vec::new();
    }

    let (main_origin, main_len, cross_origin, cross_len, is_row) = match dir {
        FlexDir::Row => (parent.x, parent.width,  parent.y, parent.height, true),
        FlexDir::Col => (parent.y, parent.height, parent.x, parent.width,  false),
    };

    let inner_main  = (main_len - 2.0 * pad - gap * (n.saturating_sub(1) as f64)).max(0.0);
    let inner_cross = (cross_len - 2.0 * pad).max(0.0);
    let basis_sum: f64 = children.iter().map(|c| c.basis).sum();
    let flex_sum:  f64 = children.iter().map(|c| c.flex).sum();
    let free = (inner_main - basis_sum).max(0.0);

    let mut rects = Vec::with_capacity(n);
    let mut cursor = main_origin + pad;
    for (i, c) in children.iter().enumerate() {
        let extra = if flex_sum > 0.0 { free * (c.flex / flex_sum) } else { 0.0 };
        let size = c.basis + extra;
        let r = if is_row {
            Rect { x: cursor, y: cross_origin + pad, width: size, height: inner_cross }
        } else {
            Rect { x: cross_origin + pad, y: cursor, width: inner_cross, height: size }
        };
        rects.push(r);
        cursor += size;
        if i + 1 < n { cursor += gap; }
    }
    rects
}
