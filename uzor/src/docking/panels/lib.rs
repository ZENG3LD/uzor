//! Pure-function docking helpers.
//!
//! These functions take a `DockingTree` (and other inputs) and compute
//! geometry without owning any state.  Used by `uzor::layout::DockState`
//! to build per-frame caches in `solve()`.
//!
//! Step C of the absorption plan — anything that is "math given a tree"
//! lives here so the stateful container (DockState) can shrink toward
//! pure storage + interaction state.

use std::collections::HashMap;

use super::{
    DockPanel, DockingTree, Branch, PanelNode, LeafId, PanelRect,
    DropZone,
};

/// Compute the rect of every visible leaf in the tree, given the
/// available `area`.  Walks the tree top-down, distributing splits
/// according to each branch's [`Shares`](super::Shares).
///
/// Pure — does not mutate `tree` or anything else.
pub fn compute_leaf_rects<P: DockPanel>(
    tree: &DockingTree<P>,
    area: PanelRect,
) -> HashMap<LeafId, PanelRect> {
    let mut rects = HashMap::new();
    collect_leaf_rects_from_branch(tree.root(), area, &mut rects);
    rects
}

/// Recursively walk a branch, writing visible leaf rects into `out`.
pub fn collect_leaf_rects_from_branch<P: DockPanel>(
    branch:      &Branch<P>,
    branch_rect: PanelRect,
    out:         &mut HashMap<LeafId, PanelRect>,
) {
    let child_rects = DockingTree::<P>::compute_child_rects(branch, branch_rect);

    for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
        match child {
            PanelNode::Leaf(leaf) => {
                if !leaf.hidden {
                    out.insert(leaf.id, *rect);
                }
            }
            PanelNode::Branch(b) => {
                collect_leaf_rects_from_branch(b, *rect, out);
            }
        }
    }
}

/// Map a pointer position relative to a panel rect into a [`DropZone`].
///
/// Center zone wins when the pointer is clearly in the middle (>20 % from
/// each edge).  Otherwise the closest edge picks `Left/Right/Up/Down`.
///
/// Pure math — used by both panel-drop and floating-window-drop logic.
pub fn detect_drop_zone(x: f32, y: f32, width: f32, height: f32) -> DropZone {
    let center_margin = 0.20;
    let cx = width * center_margin;
    let cy = height * center_margin;

    if x > cx && x < width - cx && y > cy && y < height - cy {
        return DropZone::Center;
    }

    let dist_left   = x;
    let dist_right  = width - x;
    let dist_top    = y;
    let dist_bottom = height - y;

    let min_dist = dist_left.min(dist_right).min(dist_top).min(dist_bottom);

    if min_dist == dist_left {
        DropZone::Left
    } else if min_dist == dist_right {
        DropZone::Right
    } else if min_dist == dist_top {
        DropZone::Up
    } else {
        DropZone::Down
    }
}
