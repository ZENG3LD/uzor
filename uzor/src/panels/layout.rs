//! Layout computation for the docking tree.
//!
//! Computes final rectangles for all leaves and branches in the tree
//! based on layout presets, proportions, custom rects, and cross ratios.

use super::{DockingTree, DockPanel, Branch, PanelNode, LeafId, BranchId, PanelRect, WindowLayout};
use super::presets::PANEL_GAP;

impl<P: DockPanel> DockingTree<P> {
    /// Compute layout rectangles for all visible leaves
    pub fn layout_rects(&self, width: f32, height: f32) -> Vec<(LeafId, PanelRect)> {
        let root_rect = PanelRect::new(0.0, 0.0, width, height);
        let mut result = Vec::new();
        self.layout_node(&PanelNode::Branch(self.root().clone()), root_rect, &mut result);

        // Sanitize
        const MIN_SIZE: f32 = 1.0;
        result.into_iter().map(|(id, rect)| {
            (id, PanelRect::new(
                rect.x.max(0.0),
                rect.y.max(0.0),
                rect.width.max(MIN_SIZE),
                rect.height.max(MIN_SIZE),
            ))
        }).collect()
    }

    /// Compute bounding rectangles for all branches in the tree.
    /// Returns (BranchId, PanelRect, depth) for each branch.
    /// depth=0 is root, depth=1 is root's children, etc.
    pub fn branch_rects(&self, width: f32, height: f32) -> Vec<(BranchId, PanelRect, usize)> {
        let root_rect = PanelRect::new(0.0, 0.0, width, height);
        let mut result = Vec::new();
        self.branch_rects_node(&PanelNode::Branch(self.root().clone()), root_rect, 0, &mut result);
        result
    }

    fn branch_rects_node(&self, node: &PanelNode<P>, available_rect: PanelRect, depth: usize, result: &mut Vec<(BranchId, PanelRect, usize)>) {
        match node {
            PanelNode::Leaf(_) => {}  // leaves don't have branch rects
            PanelNode::Branch(branch) => {
                // Record this branch's rect (skip root at depth 0 — it covers entire area)
                if depth > 0 {
                    result.push((branch.id, available_rect, depth));
                }
                let child_rects = Self::compute_child_rects(branch, available_rect);
                for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
                    self.branch_rects_node(child, *rect, depth + 1, result);
                }
            }
        }
    }

    fn layout_node(&self, node: &PanelNode<P>, available_rect: PanelRect, result: &mut Vec<(LeafId, PanelRect)>) {
        match node {
            PanelNode::Leaf(leaf) => {
                if !leaf.hidden {
                    result.push((leaf.id, available_rect));
                }
            }
            PanelNode::Branch(branch) => {
                let child_rects = Self::compute_child_rects(branch, available_rect);
                for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
                    self.layout_node(child, *rect, result);
                }
            }
        }
    }

    /// Compute child rects for a branch node
    pub fn compute_child_rects(branch: &Branch<P>, parent_rect: PanelRect) -> Vec<PanelRect> {
        let n = branch.children.len();
        if n == 0 { return Vec::new(); }

        // Check for hidden children
        let has_hidden = branch.children.iter().any(|c| c.is_hidden());

        // DEBUG expand
        if has_hidden {
            let vis: Vec<(usize, bool, bool)> = branch.children.iter().enumerate()
                .map(|(i, c)| (i, c.is_hidden(), matches!(c, super::PanelNode::Leaf(_))))
                .collect();
            eprintln!("[LAYOUT] branch={} layout={:?} n={} has_hidden={} vis={:?} parent_rect=({:.0},{:.0},{:.0},{:.0}) proportions={:?} custom_rects_len={}",
                branch.id.0, branch.layout, n, has_hidden, vis,
                parent_rect.x, parent_rect.y, parent_rect.width, parent_rect.height,
                branch.proportions, branch.custom_rects.len());
        }

        if has_hidden {
            // Compute layout for visible children only, preserving relative proportions
            // and orientation from the original branch layout.
            let visible_indices: Vec<usize> = branch.children.iter().enumerate()
                .filter(|(_, c)| !c.is_hidden())
                .map(|(i, _)| i)
                .collect();
            let vis_n = visible_indices.len();
            if vis_n == 0 {
                return vec![PanelRect::zero(); n];
            }

            // Single visible child always fills the full parent rect.
            if vis_n == 1 {
                let mut result = vec![PanelRect::zero(); n];
                result[visible_indices[0]] = parent_rect;
                return result;
            }

            // Multiple visible children: preserve orientation and rescale proportions.
            let gap = PANEL_GAP;
            let is_vertical = matches!(
                branch.layout,
                WindowLayout::SplitVertical | WindowLayout::ThreeRows
            );

            // Gather the weight for each visible child from branch.proportions (if
            // available) so that their relative sizes survive the hide operation.
            // Fall back to equal weights when proportions are absent or mismatched.
            let weights: Vec<f32> = if branch.proportions.len() == n {
                visible_indices.iter().map(|&ci| branch.proportions[ci] as f32).collect()
            } else {
                vec![1.0_f32; vis_n]
            };
            let weight_sum = weights.iter().sum::<f32>().max(f32::EPSILON);

            let total_gap = (vis_n - 1) as f32 * gap;
            let mut result = vec![PanelRect::zero(); n];

            if is_vertical {
                let available = (parent_rect.height - total_gap).max(0.0);
                let mut y = parent_rect.y;
                for (wi, &ci) in visible_indices.iter().enumerate() {
                    let h = available * weights[wi] / weight_sum;
                    result[ci] = PanelRect::new(parent_rect.x, y, parent_rect.width, h);
                    y += h + gap;
                }
            } else {
                let available = (parent_rect.width - total_gap).max(0.0);
                let mut x = parent_rect.x;
                for (wi, &ci) in visible_indices.iter().enumerate() {
                    let w = available * weights[wi] / weight_sum;
                    result[ci] = PanelRect::new(x, parent_rect.y, w, parent_rect.height);
                    x += w + gap;
                }
            }
            return result;
        }

        // === Original code below (all children visible) ===

        // 1. If custom_rects exist and match child count, rescale to parent_rect
        if !branch.custom_rects.is_empty() && branch.custom_rects.len() == n {
            // Compute bounding box of stored custom_rects
            let mut old_min_x = f32::MAX;
            let mut old_min_y = f32::MAX;
            let mut old_max_x = f32::MIN;
            let mut old_max_y = f32::MIN;
            for r in &branch.custom_rects {
                old_min_x = old_min_x.min(r.x);
                old_min_y = old_min_y.min(r.y);
                old_max_x = old_max_x.max(r.x + r.width);
                old_max_y = old_max_y.max(r.y + r.height);
            }
            let old_w = (old_max_x - old_min_x).max(1.0);
            let old_h = (old_max_y - old_min_y).max(1.0);

            let tol = 2.0;
            let same_bounds = (parent_rect.x - old_min_x).abs() < tol
                && (parent_rect.y - old_min_y).abs() < tol
                && (parent_rect.width - old_w).abs() < tol
                && (parent_rect.height - old_h).abs() < tol;

            if same_bounds {
                return branch.custom_rects.clone();
            }

            let sx = parent_rect.width / old_w;
            let sy = parent_rect.height / old_h;
            return branch.custom_rects.iter().map(|r| {
                PanelRect::new(
                    parent_rect.x + (r.x - old_min_x) * sx,
                    parent_rect.y + (r.y - old_min_y) * sy,
                    r.width * sx,
                    r.height * sy,
                )
            }).collect();
        }

        let gap = PANEL_GAP;

        // 2. Grid2x2 with cross_ratio
        if matches!(branch.layout, WindowLayout::Grid2x2) && n == 4 {
            if let Some((xr, yr)) = branch.cross_ratio {
                let w = parent_rect.width;
                let h = parent_rect.height;
                let left_w = ((w - gap) * xr as f32).max(50.0);
                let right_w = ((w - gap) * (1.0 - xr as f32)).max(50.0);
                let top_h = ((h - gap) * yr as f32).max(50.0);
                let bottom_h = ((h - gap) * (1.0 - yr as f32)).max(50.0);
                let ox = parent_rect.x;
                let oy = parent_rect.y;
                return vec![
                    PanelRect::new(ox, oy, left_w, top_h),
                    PanelRect::new(ox + left_w + gap, oy, right_w, top_h),
                    PanelRect::new(ox, oy + top_h + gap, left_w, bottom_h),
                    PanelRect::new(ox + left_w + gap, oy + top_h + gap, right_w, bottom_h),
                ];
            }
        }

        // 3. Proportions
        if branch.proportions.len() == n {
            let is_vertical = matches!(branch.layout, WindowLayout::SplitVertical | WindowLayout::ThreeRows);
            let total_gap = if n > 1 { (n - 1) as f32 * gap } else { 0.0 };

            if is_vertical {
                let available = (parent_rect.height - total_gap).max(0.0);
                let mut result = Vec::with_capacity(n);
                let mut y = parent_rect.y;
                for i in 0..n {
                    let h = available * branch.proportions[i] as f32;
                    result.push(PanelRect::new(parent_rect.x, y, parent_rect.width, h));
                    y += h + gap;
                }
                return result;
            } else {
                let available = (parent_rect.width - total_gap).max(0.0);
                let mut result = Vec::with_capacity(n);
                let mut x = parent_rect.x;
                for i in 0..n {
                    let w = available * branch.proportions[i] as f32;
                    result.push(PanelRect::new(x, parent_rect.y, w, parent_rect.height));
                    x += w + gap;
                }
                return result;
            }
        }

        // 4. Default: use layout.calculate_rects()
        let local_rects = branch.layout.calculate_rects(parent_rect.width, parent_rect.height, n);
        local_rects.into_iter()
            .map(|mut r| {
                r.x += parent_rect.x;
                r.y += parent_rect.y;
                r
            })
            .collect()
    }

    /// Compute the rect of a specific leaf given the full layout dimensions
    pub fn rect_for_leaf(&self, leaf_id: LeafId, width: f32, height: f32) -> Option<PanelRect> {
        let root_rect = PanelRect::new(0.0, 0.0, width, height);
        Self::rect_for_leaf_recursive(self.root(), leaf_id, root_rect)
    }

    fn rect_for_leaf_recursive(branch: &Branch<P>, target_id: LeafId, parent_rect: PanelRect) -> Option<PanelRect> {
        let child_rects = Self::compute_child_rects(branch, parent_rect);
        for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
            if child.leaf_id() == Some(target_id) {
                return Some(*rect);
            }
            if let PanelNode::Branch(b) = child {
                if let Some(found) = Self::rect_for_leaf_recursive(b, target_id, *rect) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Compute the rect of a branch given the full layout dimensions
    pub fn rect_for_branch(&self, branch_id: BranchId, width: f32, height: f32) -> Option<PanelRect> {
        let root_rect = PanelRect::new(0.0, 0.0, width, height);
        if self.root().id == branch_id {
            return Some(root_rect);
        }
        Self::rect_for_branch_recursive(self.root(), branch_id, root_rect)
    }

    fn rect_for_branch_recursive(branch: &Branch<P>, target_id: BranchId, parent_rect: PanelRect) -> Option<PanelRect> {
        let child_rects = Self::compute_child_rects(branch, parent_rect);
        for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
            if let PanelNode::Branch(b) = child {
                if b.id == target_id {
                    return Some(*rect);
                }
                if let Some(found) = Self::rect_for_branch_recursive(b, target_id, *rect) {
                    return Some(found);
                }
            }
        }
        None
    }
}
