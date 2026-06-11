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

        // 3. L-shaped (3-child, multi-axis) layouts with cross_ratio.
        //
        // Each L-shape has exactly one vertical separator and one horizontal
        // separator.  cross_ratio.x drives the vertical divider (column split)
        // and cross_ratio.y drives the horizontal divider (row split).
        //
        // Children order matches calculate_rects_with_gap / presets.rs:
        //   OneLeftTwoRight  : [left_full, right_top, right_bottom]
        //   TwoLeftOneRight  : [left_top, left_bottom, right_full]
        //   OneTopTwoBottom  : [top_full, bottom_left, bottom_right]
        //   TwoTopOneBottom  : [top_left, top_right, bottom_full]
        if n == 3 {
            if let Some((xr, yr)) = branch.cross_ratio {
                let xr = xr as f32;
                let yr = yr as f32;
                let w  = parent_rect.width;
                let h  = parent_rect.height;
                let ox = parent_rect.x;
                let oy = parent_rect.y;

                match branch.layout {
                    WindowLayout::OneLeftTwoRight => {
                        // xr = left column fraction; yr = right column row split
                        let left_w  = ((w - gap) * xr).max(50.0);
                        let right_w = ((w - gap) * (1.0 - xr)).max(50.0);
                        let top_h   = ((h - gap) * yr).max(50.0);
                        let bot_h   = ((h - gap) * (1.0 - yr)).max(50.0);
                        return vec![
                            PanelRect::new(ox,                  oy,           left_w,  h),
                            PanelRect::new(ox + left_w + gap,   oy,           right_w, top_h),
                            PanelRect::new(ox + left_w + gap,   oy + top_h + gap, right_w, bot_h),
                        ];
                    }
                    WindowLayout::TwoLeftOneRight => {
                        // xr = left column fraction; yr = left column row split
                        let left_w  = ((w - gap) * xr).max(50.0);
                        let right_w = ((w - gap) * (1.0 - xr)).max(50.0);
                        let top_h   = ((h - gap) * yr).max(50.0);
                        let bot_h   = ((h - gap) * (1.0 - yr)).max(50.0);
                        return vec![
                            PanelRect::new(ox,            oy,               left_w,  top_h),
                            PanelRect::new(ox,            oy + top_h + gap, left_w,  bot_h),
                            PanelRect::new(ox + left_w + gap, oy,           right_w, h),
                        ];
                    }
                    WindowLayout::OneTopTwoBottom => {
                        // yr = top row fraction; xr = bottom row column split
                        let top_h   = ((h - gap) * yr).max(50.0);
                        let bot_h   = ((h - gap) * (1.0 - yr)).max(50.0);
                        let left_w  = ((w - gap) * xr).max(50.0);
                        let right_w = ((w - gap) * (1.0 - xr)).max(50.0);
                        return vec![
                            PanelRect::new(ox,            oy,               w,       top_h),
                            PanelRect::new(ox,            oy + top_h + gap, left_w,  bot_h),
                            PanelRect::new(ox + left_w + gap, oy + top_h + gap, right_w, bot_h),
                        ];
                    }
                    WindowLayout::TwoTopOneBottom => {
                        // yr = top row fraction; xr = top row column split
                        let top_h   = ((h - gap) * yr).max(50.0);
                        let bot_h   = ((h - gap) * (1.0 - yr)).max(50.0);
                        let left_w  = ((w - gap) * xr).max(50.0);
                        let right_w = ((w - gap) * (1.0 - xr)).max(50.0);
                        return vec![
                            PanelRect::new(ox,                oy,           left_w,  top_h),
                            PanelRect::new(ox + left_w + gap, oy,           right_w, top_h),
                            PanelRect::new(ox,                oy + top_h + gap, w,   bot_h),
                        ];
                    }
                    _ => {}
                }
            }
        }

        // 4. Proportions — only for true single-axis layouts.
        // Multi-axis layouts (Grid2x2, OneLeftTwoRight, TwoLeftOneRight,
        // OneTopTwoBottom, TwoTopOneBottom) must fall through to their
        // shape-aware path (cross_ratio / calculate_rects) even when a
        // stale proportions vec happens to match child count.  Without
        // this guard a Grid2x2 with proportions.len()==4 collapses into
        // 4 horizontal strips on the first separator drag.
        let is_single_axis_layout = matches!(
            branch.layout,
            WindowLayout::SplitHorizontal
                | WindowLayout::SplitVertical
                | WindowLayout::ThreeColumns
                | WindowLayout::ThreeRows
        );
        if is_single_axis_layout && branch.proportions.len() == n {
            let is_vertical = matches!(branch.layout, WindowLayout::SplitVertical | WindowLayout::ThreeRows);
            let total_gap = if n > 1 { (n - 1) as f32 * gap } else { 0.0 };

            // Normalise proportions on read.  `drag_separator` preserves
            // the sum but doesn't force it to 1.0, and callers can write
            // arbitrary weights via `set_branch_proportions`.  Without
            // this we'd get either a gap (sum < 1.0 — the white-bg leak
            // visible on resize) or overlap (sum > 1.0).
            let sum: f64 = branch.proportions.iter().sum();
            let inv_sum: f32 = if sum > f64::EPSILON { (1.0 / sum) as f32 } else { 1.0 / n as f32 };

            if is_vertical {
                let available = (parent_rect.height - total_gap).max(0.0);
                let mut result = Vec::with_capacity(n);
                let mut y = parent_rect.y;
                for i in 0..n {
                    let h = available * (branch.proportions[i] as f32 * inv_sum);
                    result.push(PanelRect::new(parent_rect.x, y, parent_rect.width, h));
                    y += h + gap;
                }
                return result;
            } else {
                let available = (parent_rect.width - total_gap).max(0.0);
                let mut result = Vec::with_capacity(n);
                let mut x = parent_rect.x;
                for i in 0..n {
                    let w = available * (branch.proportions[i] as f32 * inv_sum);
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::docking::{
        DockPanel, DockingTree, Branch, PanelNode, Leaf, BranchId, LeafId,
        WindowLayout, SplitKind,
    };

    /// Minimal panel type for tests.
    #[derive(Clone)]
    struct P;
    impl DockPanel for P {
        fn title(&self)   -> &str { "p" }
        fn type_id(&self) -> &'static str { "p" }
        fn min_size(&self) -> (f32, f32) { (0.0, 0.0) }
    }

    /// Build a 4-child Branch in Grid2x2 layout with stale proportions.
    ///
    /// This is the state that previously triggered the 4x1 collapse: a Grid2x2
    /// branch whose `proportions` vec happened to be length 4 (written by an
    /// earlier separator drag before Part A guard was added).
    fn grid2x2_branch_with_proportions() -> Branch<P> {
        let make_leaf = |id: u64| PanelNode::Leaf(Leaf::new(LeafId(id), P));
        Branch {
            id: BranchId(0),
            children: vec![make_leaf(1), make_leaf(2), make_leaf(3), make_leaf(4)],
            layout: WindowLayout::Grid2x2,
            custom_rects: Vec::new(),
            // Stale proportions from an old (pre-fix) separator drag — must be ignored.
            proportions: vec![0.3, 0.3, 0.2, 0.2],
            cross_ratio: None,
            preserve_if_empty: false,
        }
    }

    // -------------------------------------------------------------------------
    // Part A — proportions guard
    // -------------------------------------------------------------------------

    /// Grid2x2 with stale proportions must NOT collapse to 4 horizontal strips.
    ///
    /// Before the fix, `compute_child_rects` entered the proportions fast-path
    /// (because `proportions.len() == 4 == n`) and rendered 4 same-height rows,
    /// destroying the 2×2 shape.  After the fix the proportions path is skipped
    /// for multi-axis layouts, so we fall through to `calculate_rects` which
    /// returns proper quadrants.
    #[test]
    fn grid2x2_with_proportions_does_not_flatten() {
        let branch = grid2x2_branch_with_proportions();
        let parent = PanelRect::new(0.0, 0.0, 1000.0, 800.0);
        let rects = DockingTree::<P>::compute_child_rects(&branch, parent);

        assert_eq!(rects.len(), 4, "must have 4 children");

        // Quadrant shape check: TL and BL must share x=0 (left column),
        // TR and BR must share a positive x (right column).
        let tl = rects[0];
        let tr = rects[1];
        let bl = rects[2];
        let br = rects[3];

        // Left column: x == 0
        assert!(tl.x < 1.0,  "TL must start at left edge, got x={}", tl.x);
        assert!(bl.x < 1.0,  "BL must start at left edge, got x={}", bl.x);

        // Right column: x > 0
        assert!(tr.x > 1.0,  "TR must be in right column, got x={}", tr.x);
        assert!(br.x > 1.0,  "BR must be in right column, got x={}", br.x);

        // Top row: y == 0
        assert!(tl.y < 1.0,  "TL must start at top, got y={}", tl.y);
        assert!(tr.y < 1.0,  "TR must start at top, got y={}", tr.y);

        // Bottom row: y > 0
        assert!(bl.y > 1.0,  "BL must be in bottom row, got y={}", bl.y);
        assert!(br.y > 1.0,  "BR must be in bottom row, got y={}", br.y);
    }

    /// Grid2x2 with an explicit cross_ratio must lay out correct quadrant sizes.
    #[test]
    fn grid2x2_cross_ratio_quadrant_sizes() {
        let make_leaf = |id: u64| PanelNode::Leaf(Leaf::new(LeafId(id), P));
        let branch = Branch {
            id: BranchId(0),
            children: vec![make_leaf(1), make_leaf(2), make_leaf(3), make_leaf(4)],
            layout: WindowLayout::Grid2x2,
            custom_rects: Vec::new(),
            proportions: Vec::new(),
            // 40% left / 60% right,  30% top / 70% bottom
            cross_ratio: Some((0.4, 0.3)),
            preserve_if_empty: false,
        };

        let parent = PanelRect::new(0.0, 0.0, 1000.0, 800.0);
        let rects = DockingTree::<P>::compute_child_rects(&branch, parent);

        let tl = rects[0];
        let tr = rects[1];
        let bl = rects[2];

        // Left column width ≈ 400
        assert!((tl.width - 400.0).abs() < 1.0,
            "TL width should be ~400, got {}", tl.width);
        // Right column width ≈ 600
        assert!((tr.width - 600.0).abs() < 1.0,
            "TR width should be ~600, got {}", tr.width);
        // Top row height ≈ 240
        assert!((tl.height - 240.0).abs() < 1.0,
            "TL height should be ~240, got {}", tl.height);
        // Bottom row height ≈ 560
        assert!((bl.height - 560.0).abs() < 1.0,
            "BL height should be ~560, got {}", bl.height);
    }

    // -------------------------------------------------------------------------
    // Part A — single-axis layouts still respect proportions
    // -------------------------------------------------------------------------

    /// SplitHorizontal with explicit proportions must use them (not fall through).
    #[test]
    fn split_horizontal_respects_proportions() {
        let make_leaf = |id: u64| PanelNode::Leaf(Leaf::new(LeafId(id), P));
        let branch = Branch {
            id: BranchId(0),
            children: vec![make_leaf(1), make_leaf(2)],
            layout: WindowLayout::SplitHorizontal,
            custom_rects: Vec::new(),
            proportions: vec![0.75, 0.25],
            cross_ratio: None,
            preserve_if_empty: false,
        };

        let parent = PanelRect::new(0.0, 0.0, 1000.0, 600.0);
        let rects = DockingTree::<P>::compute_child_rects(&branch, parent);

        assert_eq!(rects.len(), 2);
        // First child should be ~75% of 1000px = 750px wide.
        assert!((rects[0].width - 750.0).abs() < 2.0,
            "first child width should be ~750, got {}", rects[0].width);
        assert!((rects[1].width - 250.0).abs() < 2.0,
            "second child width should be ~250, got {}", rects[1].width);
    }

    /// ThreeRows with explicit proportions must stack vertically per proportions.
    #[test]
    fn three_rows_respects_proportions() {
        let make_leaf = |id: u64| PanelNode::Leaf(Leaf::new(LeafId(id), P));
        let branch = Branch {
            id: BranchId(0),
            children: vec![make_leaf(1), make_leaf(2), make_leaf(3)],
            layout: WindowLayout::ThreeRows,
            custom_rects: Vec::new(),
            proportions: vec![0.5, 0.25, 0.25],
            cross_ratio: None,
            preserve_if_empty: false,
        };

        let parent = PanelRect::new(0.0, 0.0, 800.0, 600.0);
        let rects = DockingTree::<P>::compute_child_rects(&branch, parent);

        assert_eq!(rects.len(), 3);
        // Heights: 300, 150, 150
        assert!((rects[0].height - 300.0).abs() < 2.0,
            "row 0 height should be ~300, got {}", rects[0].height);
        assert!((rects[1].height - 150.0).abs() < 2.0,
            "row 1 height should be ~150, got {}", rects[1].height);
        assert!((rects[2].height - 150.0).abs() < 2.0,
            "row 2 height should be ~150, got {}", rects[2].height);
    }

    // -------------------------------------------------------------------------
    // Part B — Grid2x2 drag actually moves cross_ratio via DockingTree
    // -------------------------------------------------------------------------

    /// Helper: after `add_leaf` + `split_leaf_with_children(Grid2x2)`, the root
    /// has 1 child which is the Grid2x2 branch.  Returns a reference to it.
    fn find_grid2x2_branch(tree: &DockingTree<P>) -> &Branch<P> {
        // root → children[0] = Branch(Grid2x2)
        match &tree.root().children[0] {
            PanelNode::Branch(b) => b,
            PanelNode::Leaf(_) => panic!("expected Grid2x2 branch as root child"),
        }
    }

    /// Dragging the vertical (left/right) separator of a Grid2x2 branch must
    /// update cross_ratio.x and NOT write proportions.
    ///
    /// We directly call `set_branch_cross_ratio` to mirror the Part B path,
    /// then verify via `compute_child_rects`.
    #[test]
    fn grid2x2_set_cross_ratio_changes_quadrant_widths() {
        let mut tree: DockingTree<P> = DockingTree::new();
        let first_id = tree.add_leaf(P);
        // After split, root has 1 child = Grid2x2 branch with 4 leaves
        let leaf_ids = tree.split_leaf_with_children(first_id, SplitKind::Grid2x2, 1000.0, 800.0);
        assert_eq!(leaf_ids.len(), 4);

        // The Grid2x2 branch is root's first (only) child.
        let grid_branch_id = find_grid2x2_branch(&tree).id;

        // Set cross_ratio: 60% left / 40% right, 50/50 top/bottom
        tree.set_branch_cross_ratio(grid_branch_id, 0.6, 0.5);

        let grid_branch = find_grid2x2_branch(&tree);
        let parent = PanelRect::new(0.0, 0.0, 1000.0, 800.0);
        let rects = DockingTree::<P>::compute_child_rects(grid_branch, parent);

        assert_eq!(rects.len(), 4);
        let tl = rects[0];
        let tr = rects[1];

        // Left column ~600, right column ~400
        assert!((tl.width - 600.0).abs() < 1.0,
            "TL width after cross_ratio should be ~600, got {}", tl.width);
        assert!((tr.width - 400.0).abs() < 1.0,
            "TR width after cross_ratio should be ~400, got {}", tr.width);

        // proportions must NOT have been written on the grid branch
        assert!(find_grid2x2_branch(&tree).proportions.is_empty(),
            "proportions must stay empty on Grid2x2 cross_ratio drag");
    }

    /// Moving the horizontal separator (Horizontal orientation) must update
    /// cross_ratio.y — top/bottom row heights change, column widths stay equal.
    #[test]
    fn grid2x2_cross_ratio_y_changes_row_heights() {
        let mut tree: DockingTree<P> = DockingTree::new();
        let first_id = tree.add_leaf(P);
        let _ = tree.split_leaf_with_children(first_id, SplitKind::Grid2x2, 1000.0, 800.0);

        let grid_branch_id = find_grid2x2_branch(&tree).id;
        // 50/50 left/right, 30/70 top/bottom
        tree.set_branch_cross_ratio(grid_branch_id, 0.5, 0.3);

        let grid_branch = find_grid2x2_branch(&tree);
        let parent = PanelRect::new(0.0, 0.0, 1000.0, 800.0);
        let rects = DockingTree::<P>::compute_child_rects(grid_branch, parent);

        assert_eq!(rects.len(), 4);
        let tl = rects[0];
        let bl = rects[2];

        // Top row height ~240 (30% of 800)
        assert!((tl.height - 240.0).abs() < 1.0,
            "TL height should be ~240, got {}", tl.height);
        // Bottom row height ~560 (70% of 800)
        assert!((bl.height - 560.0).abs() < 1.0,
            "BL height should be ~560, got {}", bl.height);
    }

    // -------------------------------------------------------------------------
    // L-shape cross_ratio rect math
    // -------------------------------------------------------------------------

    /// OneLeftTwoRight with explicit cross_ratio must place 3 children correctly:
    ///   child[0] = full-height left column (width = w * xr)
    ///   child[1] = right-top  (height = h * yr)
    ///   child[2] = right-bot  (height = h * (1-yr))
    #[test]
    fn one_left_two_right_cross_ratio_rect_math() {
        let make_leaf = |id: u64| PanelNode::Leaf(Leaf::new(LeafId(id), P));
        let branch = Branch {
            id: BranchId(0),
            children: vec![make_leaf(1), make_leaf(2), make_leaf(3)],
            layout: WindowLayout::OneLeftTwoRight,
            custom_rects: Vec::new(),
            proportions: Vec::new(),
            // 70% left column, 40% top in right column
            cross_ratio: Some((0.7, 0.4)),
            preserve_if_empty: false,
        };

        let parent = PanelRect::new(0.0, 0.0, 1000.0, 800.0);
        let rects = DockingTree::<P>::compute_child_rects(&branch, parent);

        assert_eq!(rects.len(), 3, "must have 3 children");

        let left  = rects[0];
        let rt    = rects[1];  // right-top
        let rb    = rects[2];  // right-bottom

        // Left column: starts at x=0, full height
        assert!(left.x < 1.0,  "left starts at x=0, got {}", left.x);
        assert!(left.y < 1.0,  "left starts at y=0, got {}", left.y);
        assert!((left.height - 800.0).abs() < 1.0,
            "left height must equal parent height, got {}", left.height);

        // Left column width ~ (1000 - gap) * 0.7
        let gap = PANEL_GAP;
        let exp_left_w = (1000.0 - gap) * 0.7;
        assert!((left.width - exp_left_w).abs() < 1.0,
            "left width should be ~{exp_left_w}, got {}", left.width);

        // Right column: starts at x = left.width + gap
        let exp_right_x = left.width + gap;
        assert!((rt.x - exp_right_x).abs() < 1.0,
            "right-top x should be ~{exp_right_x}, got {}", rt.x);
        assert!((rb.x - exp_right_x).abs() < 1.0,
            "right-bot x should be ~{exp_right_x}, got {}", rb.x);

        // Right column heights: top ~ (800-gap)*0.4, bot ~ (800-gap)*0.6
        let exp_top_h = (800.0 - gap) * 0.4;
        let exp_bot_h = (800.0 - gap) * 0.6;
        assert!((rt.height - exp_top_h).abs() < 1.0,
            "right-top height should be ~{exp_top_h}, got {}", rt.height);
        assert!((rb.height - exp_bot_h).abs() < 1.0,
            "right-bot height should be ~{exp_bot_h}, got {}", rb.height);

        // proportions must stay empty (we set cross_ratio directly)
        assert!(branch.proportions.is_empty(),
            "proportions must remain empty when cross_ratio is set");
    }

    /// TwoTopOneBottom with explicit cross_ratio must place 3 children correctly:
    ///   child[0] = top-left  (width = w * xr, height = h * yr)
    ///   child[1] = top-right (width = w * (1-xr), height = h * yr)
    ///   child[2] = bottom full-width (height = h * (1-yr))
    #[test]
    fn two_top_one_bottom_cross_ratio_rect_math() {
        let make_leaf = |id: u64| PanelNode::Leaf(Leaf::new(LeafId(id), P));
        let branch = Branch {
            id: BranchId(0),
            children: vec![make_leaf(1), make_leaf(2), make_leaf(3)],
            layout: WindowLayout::TwoTopOneBottom,
            custom_rects: Vec::new(),
            proportions: Vec::new(),
            // 60% left in top row, 30% top row height
            cross_ratio: Some((0.6, 0.3)),
            preserve_if_empty: false,
        };

        let parent = PanelRect::new(0.0, 0.0, 1000.0, 800.0);
        let rects = DockingTree::<P>::compute_child_rects(&branch, parent);

        assert_eq!(rects.len(), 3, "must have 3 children");

        let tl  = rects[0];  // top-left
        let tr  = rects[1];  // top-right
        let bot = rects[2];  // bottom full

        let gap = PANEL_GAP;

        // Top row height ~ (800-gap)*0.3
        let exp_top_h = (800.0 - gap) * 0.3;
        assert!((tl.height - exp_top_h).abs() < 1.0,
            "top-left height should be ~{exp_top_h}, got {}", tl.height);
        assert!((tr.height - exp_top_h).abs() < 1.0,
            "top-right height should be ~{exp_top_h}, got {}", tr.height);

        // Bottom: full width, starts below top row
        let exp_bot_y = exp_top_h + gap;
        assert!((bot.y - exp_bot_y).abs() < 1.0,
            "bottom y should be ~{exp_bot_y}, got {}", bot.y);
        assert!((bot.width - 1000.0).abs() < 1.0,
            "bottom width must equal parent width, got {}", bot.width);

        // Top-left width ~ (1000-gap)*0.6
        let exp_tl_w = (1000.0 - gap) * 0.6;
        assert!((tl.width - exp_tl_w).abs() < 1.0,
            "top-left width should be ~{exp_tl_w}, got {}", tl.width);

        // Top-right: starts at tl.width + gap
        let exp_tr_x = tl.width + gap;
        assert!((tr.x - exp_tr_x).abs() < 1.0,
            "top-right x should be ~{exp_tr_x}, got {}", tr.x);
    }
}
