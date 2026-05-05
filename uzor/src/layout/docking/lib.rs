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
    DropZone, Separator, SeparatorOrientation, SeparatorLevel,
    CornerHandle,
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

/// Generate separators between adjacent children of a branch, recursing
/// into nested branches.  Appends to `out` — caller clears it once before
/// the top-level call.
///
/// Pure — does not touch any state besides `out`.
pub fn generate_separators<P: DockPanel>(
    branch:      &Branch<P>,
    branch_rect: PanelRect,
    out:         &mut Vec<Separator>,
) {
    let child_rects = DockingTree::<P>::compute_child_rects(branch, branch_rect);

    if child_rects.len() >= 2 {
        let child_panel_rects: Vec<(u64, PanelRect)> = branch.children.iter()
            .zip(child_rects.iter())
            .filter(|(node, _)| !node.is_hidden())
            .map(|(node, wr)| (node.raw_id(), *wr))
            .collect();

        for i in 0..child_panel_rects.len() {
            for j in (i + 1)..child_panel_rects.len() {
                let (_, r1) = child_panel_rects[i];
                let (_, r2) = child_panel_rects[j];

                let h_overlap = r1.y.max(r2.y) < (r1.y + r1.height).min(r2.y + r2.height) - 1.0;
                let v_overlap = r1.x.max(r2.x) < (r1.x + r1.width).min(r2.x + r2.width) - 1.0;

                if h_overlap {
                    let left  = if r1.x < r2.x { r1 } else { r2 };
                    let right = if r1.x < r2.x { r2 } else { r1 };
                    let gap   = right.x - (left.x + left.width);
                    if gap <= 15.0 {
                        let sep_x = left.x + left.width + gap / 2.0;
                        let sep_y = r1.y.max(r2.y);
                        let sep_h = (r1.y + r1.height).min(r2.y + r2.height) - sep_y;
                        let (ca, cb) = if child_panel_rects[i].1.x < child_panel_rects[j].1.x {
                            (child_panel_rects[i].0, child_panel_rects[j].0)
                        } else {
                            (child_panel_rects[j].0, child_panel_rects[i].0)
                        };
                        out.push(Separator::new(
                            SeparatorOrientation::Vertical,
                            sep_x, sep_y, sep_h,
                            SeparatorLevel::Node { parent_id: branch.id, child_a: ca, child_b: cb },
                        ));
                    }
                } else if v_overlap {
                    let top    = if r1.y < r2.y { r1 } else { r2 };
                    let bottom = if r1.y < r2.y { r2 } else { r1 };
                    let gap    = bottom.y - (top.y + top.height);
                    if gap <= 15.0 {
                        let sep_y = top.y + top.height + gap / 2.0;
                        let sep_x = r1.x.max(r2.x);
                        let sep_w = (r1.x + r1.width).min(r2.x + r2.width) - sep_x;
                        let (ca, cb) = if child_panel_rects[i].1.y < child_panel_rects[j].1.y {
                            (child_panel_rects[i].0, child_panel_rects[j].0)
                        } else {
                            (child_panel_rects[j].0, child_panel_rects[i].0)
                        };
                        out.push(Separator::new(
                            SeparatorOrientation::Horizontal,
                            sep_y, sep_x, sep_w,
                            SeparatorLevel::Node { parent_id: branch.id, child_a: ca, child_b: cb },
                        ));
                    }
                }
            }
        }
    }

    for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
        if let PanelNode::Branch(b) = child {
            generate_separators(b, *rect, out);
        }
    }
}

/// Compute corner handles at separator intersections.  Pure — given
/// the separator list, produces `CornerHandle`s for every vertical+
/// horizontal cross-pair.
pub fn detect_corners(separators: &[Separator]) -> Vec<CornerHandle> {
    let mut out = Vec::new();
    for (vi, v_sep) in separators.iter().enumerate() {
        if v_sep.orientation != SeparatorOrientation::Vertical { continue; }
        for (hi, h_sep) in separators.iter().enumerate() {
            if h_sep.orientation != SeparatorOrientation::Horizontal { continue; }
            out.push(CornerHandle {
                v_separator_idx: vi,
                h_separator_idx: hi,
                x: v_sep.position,
                y: h_sep.position,
            });
        }
    }
    out
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
