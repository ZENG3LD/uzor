//! Generic N-ary docking tree for panel management.
//!
//! This module provides a generic tree structure for managing hierarchical
//! panel layouts with tabs, splits, and grids. It's agnostic to the actual
//! panel content type (defined by the `DockPanel` trait).

use super::{LeafId, BranchId, PanelRect, WindowLayout, SplitKind, DropZone, DockPanel};

/// Leaf node — actual panel container with tabs
#[derive(Clone, Debug)]
pub struct Leaf<P: DockPanel> {
    pub id: LeafId,
    pub panels: Vec<P>,
    pub active_tab: usize,
    pub hidden: bool,
    /// Domain-agnostic color group token (None = ungrouped).
    pub color_tag: Option<u8>,
}

impl<P: DockPanel> Leaf<P> {
    pub fn new(id: LeafId, panel: P) -> Self {
        Self {
            id,
            panels: vec![panel],
            active_tab: 0,
            hidden: false,
            color_tag: None,
        }
    }

    /// Create an empty leaf (no panels).  Used by compound split when
    /// freshly-allocated sibling slots must not alias the original leaf's
    /// panel identities — caller is expected to populate `panels` afterwards.
    pub fn new_empty(id: LeafId) -> Self {
        Self {
            id,
            panels: Vec::new(),
            active_tab: 0,
            hidden: false,
            color_tag: None,
        }
    }

    /// Get the color group tag for this leaf.
    pub fn color_tag(&self) -> Option<u8> {
        self.color_tag
    }

    /// Set the color group tag for this leaf.
    pub fn set_color_tag(&mut self, tag: Option<u8>) {
        self.color_tag = tag;
    }

    pub fn active_panel(&self) -> Option<&P> {
        self.panels.get(self.active_tab)
    }

    pub fn active_panel_mut(&mut self) -> Option<&mut P> {
        self.panels.get_mut(self.active_tab)
    }

    pub fn tab_count(&self) -> usize {
        self.panels.len()
    }
}

/// Branch node — container with children and layout
#[derive(Clone, Debug)]
pub struct Branch<P: DockPanel> {
    pub id: BranchId,
    pub children: Vec<PanelNode<P>>,
    pub layout: WindowLayout,
    pub custom_rects: Vec<PanelRect>,
    pub proportions: Vec<f64>,
    pub cross_ratio: Option<(f64, f64)>,
    /// When true, this branch survives `collapse_single_children_branch`
    /// (the cleanup pass that normally inlines branches whose `children`
    /// has shrunk to one item).  Consumers carry sidecar metadata keyed
    /// by `BranchId` (e.g. group semantics, accept policy, container
    /// kind); a `BranchId` that vanishes mid-frame orphans that
    /// metadata.  Setting `preserve_if_empty = true` keeps the branch
    /// node alive across remove/move operations.
    ///
    /// Default: `false` — backwards-compatible with existing callers
    /// that expect aggressive single-child collapse.
    pub preserve_if_empty: bool,
}

/// A node in the recursive panel tree
#[derive(Clone, Debug)]
pub enum PanelNode<P: DockPanel> {
    Leaf(Leaf<P>),
    Branch(Branch<P>),
}

impl<P: DockPanel> PanelNode<P> {
    pub fn leaf_id(&self) -> Option<LeafId> {
        match self { PanelNode::Leaf(l) => Some(l.id), _ => None }
    }

    pub fn branch_id(&self) -> Option<BranchId> {
        match self { PanelNode::Branch(b) => Some(b.id), _ => None }
    }

    /// Raw numeric ID for internal use
    pub fn raw_id(&self) -> u64 {
        match self { PanelNode::Leaf(l) => l.id.0, PanelNode::Branch(b) => b.id.0 }
    }

    pub fn is_hidden(&self) -> bool {
        match self {
            PanelNode::Leaf(l) => l.hidden,
            // A branch is considered hidden when ALL its descendants are hidden.
            // This ensures compute_child_rects correctly collapses fully-hidden
            // sub-trees (e.g. one half of a 2x2 grid) so the visible branch
            // receives the entire parent rect rather than just its proportional share.
            PanelNode::Branch(b) => b.children.iter().all(|c| c.is_hidden()),
        }
    }
}

/// Generic docking tree for hierarchical panel layouts
#[derive(Clone, Debug)]
pub struct DockingTree<P: DockPanel> {
    root: Branch<P>,
    active_leaf: Option<LeafId>,
    next_id: u64,
}

impl<P: DockPanel> DockingTree<P> {
    /// Create an empty docking tree
    pub fn new() -> Self {
        Self {
            root: Branch {
                id: BranchId(0),
                children: Vec::new(),
                layout: WindowLayout::Single,
                custom_rects: Vec::new(),
                proportions: Vec::new(),
                cross_ratio: None,
                preserve_if_empty: false,
            },
            active_leaf: None,
            next_id: 1,
        }
    }

    /// Create with a single leaf containing one panel
    pub fn with_single_leaf(panel: P) -> Self {
        let mut tree = Self::new();
        tree.add_leaf(panel);
        tree
    }

    /// Create from restored serialized structure (for deserialize module)
    pub(crate) fn from_restored_structure(
        root: Branch<P>,
        active_leaf: Option<LeafId>,
        next_id: u64,
    ) -> Self {
        let mut tree = Self {
            root,
            active_leaf,
            next_id,
        };

        // Validate active leaf still exists
        if let Some(active_id) = tree.active_leaf {
            if tree.leaf(active_id).is_none() {
                tree.active_leaf = tree.first_leaf_id();
            }
        }

        tree
    }

    /// Get reference to the root branch
    pub fn root(&self) -> &Branch<P> {
        &self.root
    }

    fn next_leaf_id(&mut self) -> LeafId {
        let id = LeafId(self.next_id);
        self.next_id += 1;
        id
    }

    fn next_branch_id(&mut self) -> BranchId {
        let id = BranchId(self.next_id);
        self.next_id += 1;
        id
    }

    // --- Leaf Management ---

    /// Add a new leaf to the root with a single panel
    pub fn add_leaf(&mut self, panel: P) -> LeafId {
        let id = self.next_leaf_id();
        let leaf = PanelNode::Leaf(Leaf::new(id, panel));

        self.root.children.push(leaf);
        self.root.custom_rects.clear();
        self.root.layout = Self::infer_layout(self.root.children.len());

        if self.active_leaf.is_none() {
            self.active_leaf = Some(id);
        }

        id
    }

    /// Add a new leaf with pre-existing panels (for re-inserting)
    pub fn add_leaf_with_panels(&mut self, panels: Vec<P>, active_tab: usize) -> LeafId {
        let id = self.next_leaf_id();
        let leaf = PanelNode::Leaf(Leaf {
            id,
            panels,
            active_tab,
            hidden: false,
            color_tag: None,
        });
        self.root.children.push(leaf);
        self.root.custom_rects.clear();
        self.root.layout = Self::infer_layout(self.root.children.len());
        if self.active_leaf.is_none() {
            self.active_leaf = Some(id);
        }
        id
    }

    /// Add a new leaf as a sibling of `sibling_id` within the same parent branch
    pub fn add_leaf_near(&mut self, panel: P, sibling_id: LeafId) -> LeafId {
        let id = self.next_leaf_id();

        // Find the parent branch of the sibling
        let parent_id = self.find_parent_of_leaf(sibling_id).map(|p| p.id);

        if let Some(pid) = parent_id {
            if let Some(branch) = self.find_branch_mut(pid) {
                let leaf = PanelNode::Leaf(Leaf::new(id, panel));
                // Insert after the sibling
                if let Some(pos) = branch.children.iter().position(|c| c.leaf_id() == Some(sibling_id)) {
                    branch.children.insert(pos + 1, leaf);
                } else {
                    branch.children.push(leaf);
                }
                branch.custom_rects.clear();
                branch.proportions.clear();
                branch.layout = Self::infer_layout(branch.children.len());

                if self.active_leaf.is_none() {
                    self.active_leaf = Some(id);
                }
                return id;
            }
        }

        // Fallback: add to root
        self.add_leaf(panel)
    }

    pub fn remove_leaf(&mut self, id: LeafId) {
        // Remove from tree
        Self::remove_node_recursive(&mut self.root, id);

        // Collapse single-child branches
        Self::collapse_single_children_branch(&mut self.root);

        // Recursively fix layouts for branches whose child count may have changed
        Self::fix_branch_layouts(&mut self.root);

        if self.active_leaf == Some(id) {
            self.active_leaf = self.first_leaf_id();
        }
    }

    /// After removing a node and collapsing single-child branches, ensure all branches
    /// have correct layout/proportions for their current child count.
    fn fix_branch_layouts(branch: &mut Branch<P>) {
        // If proportions don't match child count, clear them
        if branch.proportions.len() != branch.children.len() {
            branch.proportions.clear();
        }
        // If custom_rects don't match child count, clear them
        if branch.custom_rects.len() != branch.children.len() {
            branch.custom_rects.clear();
        }
        // Only fix layout if it's truly incompatible with child count
        let layout_compatible = match branch.layout {
            WindowLayout::Single => branch.children.len() <= 1,
            WindowLayout::SplitHorizontal | WindowLayout::SplitVertical => branch.children.len() == 2,
            WindowLayout::Grid2x2 => branch.children.len() == 4,
            WindowLayout::ThreeColumns | WindowLayout::ThreeRows |
            WindowLayout::OneLeftTwoRight | WindowLayout::OneTopTwoBottom |
            WindowLayout::TwoLeftOneRight | WindowLayout::TwoTopOneBottom => branch.children.len() == 3,
            WindowLayout::Custom => true, // Custom always valid
        };
        if !layout_compatible {
            branch.layout = Self::infer_layout(branch.children.len());
        }

        // Recurse into children
        for child in &mut branch.children {
            if let PanelNode::Branch(ref mut b) = child {
                Self::fix_branch_layouts(b);
            }
        }
    }

    fn remove_node_recursive(root: &mut Branch<P>, id: LeafId) -> bool {
        // Check direct children
        if let Some(pos) = root.children.iter().position(|c| c.leaf_id() == Some(id)) {
            let old_layout = root.layout;
            let old_count = root.children.len();

            root.children.remove(pos);
            root.custom_rects.clear();
            root.proportions.clear();

            // Smart layout transition based on spatial analysis
            let new_count = root.children.len();

            // Grid2x2 → 3 slots: pick L-shaped layout based on which position was removed
            if old_count == 4 && new_count == 3 && old_layout == WindowLayout::Grid2x2 {
                // Grid2x2 positions: 0=TL, 1=TR, 2=BL, 3=BR
                root.layout = match pos {
                    0 | 1 => WindowLayout::OneTopTwoBottom,
                    2 | 3 => WindowLayout::TwoTopOneBottom,
                    _ => WindowLayout::ThreeColumns,
                };
                root.cross_ratio = None;
            } else if old_count == 3 && new_count == 2 {
                // 3 → 2: preserve orientation from old layout
                root.layout = match old_layout {
                    WindowLayout::SplitVertical | WindowLayout::ThreeRows |
                    WindowLayout::OneTopTwoBottom | WindowLayout::TwoTopOneBottom => WindowLayout::SplitVertical,
                    _ => WindowLayout::SplitHorizontal,
                };
                root.cross_ratio = None;
            } else {
                // Generic fallback: use simple count-based inference
                root.layout = Self::infer_layout(new_count);
                root.cross_ratio = None;
            }

            return true;
        }
        // Recurse into branches
        for child in &mut root.children {
            if let PanelNode::Branch(ref mut branch) = child {
                if Self::remove_node_recursive(branch, id) {
                    return true;
                }
            }
        }
        false
    }

    // --- Tab Management ---

    pub fn add_tab(&mut self, leaf_id: LeafId, panel: P) {
        if let Some(leaf) = self.find_leaf_mut(leaf_id) {
            leaf.panels.push(panel);
            leaf.active_tab = leaf.panels.len() - 1;
        }
    }

    pub fn remove_tab(&mut self, leaf_id: LeafId, tab_idx: usize) {
        let should_remove_leaf = {
            if let Some(leaf) = self.find_leaf_mut(leaf_id) {
                if tab_idx < leaf.panels.len() {
                    leaf.panels.remove(tab_idx);
                    if leaf.panels.is_empty() {
                        true
                    } else {
                        if leaf.active_tab >= leaf.panels.len() {
                            leaf.active_tab = leaf.panels.len() - 1;
                        }
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_remove_leaf {
            self.remove_leaf(leaf_id);
        }
    }

    // --- Split Operations ---

    /// Split `leaf_id` into two leaves with the original kept in-place.
    ///
    /// `dir` selects how the leaf is bisected:
    /// - [`SplitKind::SplitRight`]   — original ends up **left**,  new neighbour **right**
    /// - [`SplitKind::SplitBottom`]  — original ends up **top**,   new neighbour **bottom**
    /// (other split kinds are not in-place — use [`Self::split_leaf_with_children`].)
    ///
    /// The original leaf id stays valid and points to the same panels.
    /// The returned id is the freshly created neighbour leaf, populated
    /// with `new_panel` as its sole panel.
    ///
    /// Returns `None` if `leaf_id` is not in the tree, or if `dir` is
    /// not one of the two binary splits.
    pub fn split_leaf(
        &mut self,
        leaf_id:   LeafId,
        dir:       SplitKind,
        new_panel: P,
    ) -> Option<LeafId> {
        // Reject non-binary splits — `split_leaf_with_children` handles those.
        if !matches!(dir, SplitKind::SplitRight | SplitKind::SplitBottom) {
            return None;
        }

        // Snapshot the original leaf so we can rebuild it under the new branch.
        let leaf_clone = self.find_leaf(leaf_id)?.clone();

        let new_id = self.next_leaf_id();
        // Original leaf preserved verbatim (id, panels, active_tab, color_tag …)
        let original_node = PanelNode::Leaf(leaf_clone);
        let new_node = PanelNode::Leaf(Leaf::new(new_id, new_panel));

        // SplitRight  → [original | new]
        // SplitBottom → [original ; new]
        let children = vec![original_node, new_node];

        let branch_id = self.next_branch_id();
        let new_branch = PanelNode::Branch(Branch {
            id: branch_id,
            children,
            layout: Self::split_kind_to_layout(dir),
            custom_rects: Vec::new(),
            proportions: Vec::new(),
            cross_ratio: None,
            preserve_if_empty: false,
        });

        // Replace the original leaf-node in its parent with the new branch
        // (the original leaf is one of the branch's children, so the leaf id
        // remains valid).
        self.replace_node_leaf(leaf_id, new_branch);

        Some(new_id)
    }

    /// Replace `leaf_id` with N freshly created leaves.
    ///
    /// The first new leaf inherits the original's panel set (preserving
    /// existing content in slot 0).  All subsequent leaves are spawned EMPTY
    /// — the caller is expected to populate them with freshly-allocated
    /// panels.
    ///
    /// Use for compound splits (Grid2x2 / OneBig3Small / 3-column / 3-row …)
    /// where there is no single "original" position — every sibling is new.
    /// The original `leaf_id` becomes invalid after this call; `new_ids` are
    /// the only valid handles.
    ///
    /// **Why empty siblings, not clones:** consumers key state by panel id
    /// (e.g. `HashMap<PanelId, _>`).  Duplicating the same `P` value into N
    /// new leaves would alias one identity across N positions — removing
    /// state for that id wipes content in every position simultaneously.
    /// Empty siblings let the caller pair each fresh leaf with a freshly
    /// allocated panel id.
    pub fn split_leaf_with_children(
        &mut self,
        leaf_id: LeafId,
        split:   SplitKind,
        _width:  f32,
        _height: f32,
    ) -> Vec<LeafId> {
        // 1. Get leaf data before removing it
        let leaf = match self.find_leaf(leaf_id) {
            Some(l) => l.clone(),
            None => return Vec::new(),
        };

        // 2. Determine sub-rect count
        let sub_rect_count = match split {
            SplitKind::SplitRight | SplitKind::SplitBottom => 2,
            SplitKind::Grid2x2 | SplitKind::OneBig3Small => 4,
            SplitKind::OneLeftTwoRight | SplitKind::TwoLeftOneRight
            | SplitKind::OneTopTwoBottom | SplitKind::TwoTopOneBottom
            | SplitKind::ThreeColumns | SplitKind::ThreeRows => 3,
        };

        // 3. Create N new leaf nodes.
        //
        // The first new leaf inherits the original's panel set (so existing
        // content lives on at that position).  The rest are spawned EMPTY so
        // callers can populate them with their own freshly-allocated panels
        // without colliding with the original's panel ids.
        //
        // (Earlier behavior cloned the original's panels into EVERY new leaf,
        // which produced N leaves all holding the same panel identity — a
        // catastrophic id-aliasing bug for consumers that key state by panel
        // id, e.g. mylittlechart's panels_store HashMap<PanelId, _>.)
        let new_ids: Vec<LeafId> = (0..sub_rect_count).map(|_| self.next_leaf_id()).collect();
        let new_leaves: Vec<PanelNode<P>> = new_ids.iter().enumerate().map(|(i, &id)| {
            let leaf_node = if i == 0 && !leaf.panels.is_empty() {
                let mut node = Leaf::new(id, leaf.panels[0].clone());
                node.panels = leaf.panels.clone();
                node.active_tab = leaf.active_tab.min(leaf.panels.len().saturating_sub(1));
                node
            } else {
                Leaf::new_empty(id)
            };
            PanelNode::Leaf(leaf_node)
        }).collect();

        // 4. Create new branch node
        let branch_id = self.next_branch_id();
        let new_branch = PanelNode::Branch(Branch {
            id: branch_id,
            children: new_leaves,
            layout: Self::split_kind_to_layout(split),
            custom_rects: Vec::new(),
            proportions: Vec::new(),
            cross_ratio: None,
            preserve_if_empty: false,
        });

        // 5. Replace old leaf with new branch in parent
        self.replace_node_leaf(leaf_id, new_branch);

        new_ids
    }

    // --- Tree Navigation ---

    fn find_leaf(&self, id: LeafId) -> Option<&Leaf<P>> {
        Self::find_leaf_in(&self.root, id)
    }

    fn find_leaf_mut(&mut self, id: LeafId) -> Option<&mut Leaf<P>> {
        Self::find_leaf_in_mut(&mut self.root, id)
    }

    fn find_leaf_in(branch: &Branch<P>, id: LeafId) -> Option<&Leaf<P>> {
        for child in &branch.children {
            match child {
                PanelNode::Leaf(l) if l.id == id => return Some(l),
                PanelNode::Branch(b) => {
                    if let Some(found) = Self::find_leaf_in(b, id) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn find_leaf_in_mut(branch: &mut Branch<P>, id: LeafId) -> Option<&mut Leaf<P>> {
        for child in &mut branch.children {
            match child {
                PanelNode::Leaf(ref mut l) if l.id == id => return Some(l),
                PanelNode::Branch(ref mut b) => {
                    if let Some(found) = Self::find_leaf_in_mut(b, id) {
                        return Some(found);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub fn find_branch(&self, id: BranchId) -> Option<&Branch<P>> {
        if self.root.id == id { return Some(&self.root); }
        Self::find_branch_in(&self.root, id)
    }

    pub fn find_branch_mut(&mut self, id: BranchId) -> Option<&mut Branch<P>> {
        if self.root.id == id { return Some(&mut self.root); }
        Self::find_branch_in_mut(&mut self.root, id)
    }

    fn find_branch_in(branch: &Branch<P>, id: BranchId) -> Option<&Branch<P>> {
        for child in &branch.children {
            if let PanelNode::Branch(b) = child {
                if b.id == id { return Some(b); }
                if let Some(found) = Self::find_branch_in(b, id) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn find_branch_in_mut(branch: &mut Branch<P>, id: BranchId) -> Option<&mut Branch<P>> {
        for child in &mut branch.children {
            if let PanelNode::Branch(b) = child {
                if b.id == id { return Some(b); }
                if let Some(found) = Self::find_branch_in_mut(b, id) {
                    return Some(found);
                }
            }
        }
        None
    }

    pub fn find_parent_of_leaf(&self, id: LeafId) -> Option<&Branch<P>> {
        if self.root.children.iter().any(|c| c.leaf_id() == Some(id)) {
            return Some(&self.root);
        }
        Self::find_parent_of_leaf_in(&self.root, id)
    }

    fn find_parent_of_leaf_in(branch: &Branch<P>, id: LeafId) -> Option<&Branch<P>> {
        for child in &branch.children {
            if let PanelNode::Branch(b) = child {
                if b.children.iter().any(|c| c.leaf_id() == Some(id)) {
                    return Some(b);
                }
                if let Some(found) = Self::find_parent_of_leaf_in(b, id) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn replace_node_leaf(&mut self, old_id: LeafId, new_node: PanelNode<P>) {
        Self::replace_leaf_in_branch(&mut self.root, old_id, new_node);
    }

    fn replace_leaf_in_branch(branch: &mut Branch<P>, old_id: LeafId, new_node: PanelNode<P>) -> bool {
        for child in &mut branch.children {
            if child.leaf_id() == Some(old_id) {
                *child = new_node;
                return true;
            }
            if let PanelNode::Branch(ref mut b) = child {
                if Self::replace_leaf_in_branch(b, old_id, new_node.clone()) {
                    return true;
                }
            }
        }
        false
    }

    fn collapse_single_children_branch(branch: &mut Branch<P>) {
        // First recurse into children
        for child in &mut branch.children {
            if let PanelNode::Branch(ref mut b) = child {
                Self::collapse_single_children_branch(b);
            }
        }
        // Then check if any branch child has exactly 1 child — replace it.
        // Branches marked `preserve_if_empty` survive even when their child
        // count shrinks to 1 (or 0).  Consumers attach sidecar metadata
        // keyed by BranchId; that BranchId vanishing on a transient
        // remove/move operation would orphan the metadata.
        let mut i = 0;
        while i < branch.children.len() {
            let should_collapse = matches!(
                &branch.children[i],
                PanelNode::Branch(b) if b.children.len() == 1 && !b.preserve_if_empty
            );
            if should_collapse {
                if let PanelNode::Branch(b) = branch.children.remove(i) {
                    let single_child = b.children.into_iter().next().unwrap();
                    branch.children.insert(i, single_child);
                }
            }
            i += 1;
        }
    }

    fn collect_all_leaves(&self) -> Vec<&Leaf<P>> {
        Self::collect_leaves_from(&self.root)
    }

    fn collect_leaves_from(branch: &Branch<P>) -> Vec<&Leaf<P>> {
        let mut result = Vec::new();
        for child in &branch.children {
            match child {
                PanelNode::Leaf(l) => result.push(l),
                PanelNode::Branch(b) => result.extend(Self::collect_leaves_from(b)),
            }
        }
        result
    }

    fn first_leaf_id(&self) -> Option<LeafId> {
        self.collect_all_leaves().first().map(|l| l.id)
    }

    // --- Accessors ---

    pub fn leaves(&self) -> Vec<&Leaf<P>> {
        self.collect_all_leaves()
    }

    pub fn leaf(&self, id: LeafId) -> Option<&Leaf<P>> {
        self.find_leaf(id)
    }

    pub fn leaf_mut(&mut self, id: LeafId) -> Option<&mut Leaf<P>> {
        self.find_leaf_mut(id)
    }

    pub fn leaf_count(&self) -> usize {
        self.collect_all_leaves().len()
    }

    pub fn visible_leaf_count(&self) -> usize {
        self.collect_all_leaves().into_iter().filter(|l| !l.hidden).count()
    }

    pub fn active_leaf_id(&self) -> Option<LeafId> {
        self.active_leaf
    }

    pub fn set_active_leaf(&mut self, id: LeafId) {
        if self.find_leaf(id).is_some() {
            self.active_leaf = Some(id);
        }
    }

    pub fn active_leaf(&self) -> Option<&Leaf<P>> {
        self.active_leaf.and_then(|id| self.leaf(id))
    }

    pub fn active_leaf_mut(&mut self) -> Option<&mut Leaf<P>> {
        let id = self.active_leaf?;
        self.leaf_mut(id)
    }

    // --- Layout ---

    pub fn layout(&self) -> WindowLayout {
        self.root.layout
    }

    pub fn set_layout(&mut self, layout: WindowLayout) {
        self.root.layout = layout;
        self.root.proportions.clear();
        self.root.cross_ratio = None;
        self.clear_custom_rects();
    }

    pub(crate) fn infer_layout(slot_count: usize) -> WindowLayout {
        match slot_count {
            0 | 1 => WindowLayout::Single,
            2 => WindowLayout::SplitHorizontal,
            3 => WindowLayout::ThreeColumns,
            4 => WindowLayout::Grid2x2,
            _ => WindowLayout::Custom,
        }
    }

    fn split_kind_to_layout(split: SplitKind) -> WindowLayout {
        match split {
            SplitKind::SplitRight  => WindowLayout::SplitHorizontal,
            SplitKind::SplitBottom => WindowLayout::SplitVertical,
            SplitKind::Grid2x2 => WindowLayout::Grid2x2,
            SplitKind::OneLeftTwoRight => WindowLayout::OneLeftTwoRight,
            SplitKind::TwoLeftOneRight => WindowLayout::TwoLeftOneRight,
            SplitKind::OneTopTwoBottom => WindowLayout::OneTopTwoBottom,
            SplitKind::TwoTopOneBottom => WindowLayout::TwoTopOneBottom,
            SplitKind::ThreeColumns => WindowLayout::ThreeColumns,
            SplitKind::ThreeRows => WindowLayout::ThreeRows,
            SplitKind::OneBig3Small => WindowLayout::Grid2x2, // Use 2x2 grid as base layout
        }
    }

    // --- Proportions ---

    pub fn set_proportions(&mut self, props: Vec<f64>) {
        if props.len() == self.root.children.len() {
            self.root.proportions = props;
        }
    }

    pub fn proportions(&self) -> Vec<f64> {
        if self.root.proportions.len() == self.root.children.len() && !self.root.proportions.is_empty() {
            self.root.proportions.clone()
        } else {
            let n = self.root.children.len();
            if n == 0 { Vec::new() } else { vec![1.0 / n as f64; n] }
        }
    }

    pub fn reset_proportions(&mut self) {
        Self::reset_branch_proportions(&mut self.root);
    }

    fn reset_branch_proportions(branch: &mut Branch<P>) {
        branch.proportions.clear();
        branch.custom_rects.clear();
        branch.cross_ratio = None;
        for child in &mut branch.children {
            if let PanelNode::Branch(b) = child {
                Self::reset_branch_proportions(b);
            }
        }
    }

    // --- Cross Ratio (Grid2x2) ---

    pub fn cross_ratio(&self) -> Option<(f64, f64)> {
        self.root.cross_ratio
    }

    pub fn set_cross_ratio(&mut self, x_ratio: f64, y_ratio: f64) {
        self.root.cross_ratio = Some((x_ratio.clamp(0.05, 0.95), y_ratio.clamp(0.05, 0.95)));
    }

    pub fn reset_cross_ratio(&mut self) {
        self.root.cross_ratio = None;
    }

    // --- Branch-specific setters ---

    pub fn set_branch_proportions(&mut self, node_id: BranchId, props: Vec<f64>) {
        if node_id == self.root.id {
            self.root.proportions = props;
            self.root.custom_rects.clear();
        } else if let Some(branch) = Self::find_branch_in_mut(&mut self.root, node_id) {
            branch.proportions = props;
            branch.custom_rects.clear();
        }
    }

    /// Mark a branch as `preserve_if_empty`.  Branches with this flag survive
    /// `collapse_single_children_branch` regardless of how few children they
    /// hold.  Useful when the consumer attaches group/container metadata
    /// keyed by `BranchId`: the metadata must outlive transient single-leaf
    /// states (e.g. user deletes all sub-panes leaving only the main chart).
    ///
    /// Returns `true` if the branch was found and the flag was applied.
    pub fn set_branch_preserve_if_empty(&mut self, node_id: BranchId, preserve: bool) -> bool {
        if node_id == self.root.id {
            self.root.preserve_if_empty = preserve;
            true
        } else if let Some(branch) = Self::find_branch_in_mut(&mut self.root, node_id) {
            branch.preserve_if_empty = preserve;
            true
        } else {
            false
        }
    }

    pub fn set_branch_cross_ratio(&mut self, node_id: BranchId, x_ratio: f64, y_ratio: f64) {
        if node_id == self.root.id {
            self.root.cross_ratio = Some((x_ratio.clamp(0.05, 0.95), y_ratio.clamp(0.05, 0.95)));
            self.root.custom_rects.clear();
        } else if let Some(branch) = Self::find_branch_in_mut(&mut self.root, node_id) {
            branch.cross_ratio = Some((x_ratio.clamp(0.05, 0.95), y_ratio.clamp(0.05, 0.95)));
            branch.custom_rects.clear();
        }
    }

    // --- Custom Rects ---

    pub fn set_custom_rects(&mut self, rects: Vec<(LeafId, PanelRect)>) {
        use std::collections::HashMap;

        // Distribute rects to their respective parent branches
        let mut branches_to_update: HashMap<BranchId, Vec<PanelRect>> = HashMap::new();

        for (leaf_id, rect) in rects {
            if let Some(parent) = self.find_parent_of_leaf(leaf_id) {
                let parent_id = parent.id;
                // Find the index of this leaf in parent's children
                if let Some(idx) = parent.children.iter().position(|c| c.leaf_id() == Some(leaf_id)) {
                    let entry = branches_to_update.entry(parent_id).or_insert_with(|| {
                        vec![PanelRect::zero(); parent.children.len()]
                    });
                    if idx < entry.len() {
                        entry[idx] = rect;
                    }
                }
            }
        }

        // Apply updates
        for (parent_id, rects) in branches_to_update {
            self.set_node_custom_rects(parent_id, rects);
        }
    }

    pub fn set_node_custom_rects(&mut self, node_id: BranchId, rects: Vec<PanelRect>) {
        if node_id == self.root.id {
            self.root.custom_rects = rects;
        } else if let Some(branch) = Self::find_branch_in_mut(&mut self.root, node_id) {
            branch.custom_rects = rects;
        }
    }

    pub fn clear_custom_rects(&mut self) {
        Self::clear_custom_rects_recursive(&mut self.root);
    }

    fn clear_custom_rects_recursive(branch: &mut Branch<P>) {
        branch.custom_rects.clear();
        for child in &mut branch.children {
            if let PanelNode::Branch(ref mut b) = child {
                Self::clear_custom_rects_recursive(b);
            }
        }
    }

    // --- Drag & Drop Operations ---

    pub fn move_leaf_to_branch(&mut self, dragged_id: LeafId, target_id: LeafId, zone: DropZone) {
        if dragged_id == target_id { return; }

        // 1. Clone dragged leaf before removal
        let dragged_leaf = match self.find_leaf(dragged_id) {
            Some(l) => l.clone(),
            None => return,
        };

        // 2. Remove dragged from tree + cleanup
        Self::remove_node_recursive(&mut self.root, dragged_id);
        Self::collapse_single_children_branch(&mut self.root);
        Self::fix_branch_layouts(&mut self.root);

        // 3. Verify target still exists after removal/collapse
        if self.find_leaf(target_id).is_none() { return; }

        // 4. Find target's parent branch after tree restructuring
        let target_parent_id = match self.find_parent_of_leaf(target_id) {
            Some(p) => p.id,
            None => return,
        };

        // 5. Insert dragged leaf as sibling of target
        let after = matches!(zone, DropZone::Right | DropZone::Down);

        if let Some(parent) = self.find_branch_mut(target_parent_id) {
            let target_pos = match parent.children.iter().position(|c| c.leaf_id() == Some(target_id)) {
                Some(p) => p,
                None => return,
            };
            let insert_pos = if after { target_pos + 1 } else { target_pos };
            parent.children.insert(insert_pos.min(parent.children.len()), PanelNode::Leaf(dragged_leaf));
            parent.custom_rects.clear();
            parent.proportions.clear();
            parent.layout = Self::infer_layout(parent.children.len());
        }
    }

    pub fn move_leaf_to_root_split(&mut self, dragged_id: LeafId, zone: DropZone) {
        // 1. Clone dragged leaf
        let dragged_leaf = match self.find_leaf(dragged_id) {
            Some(l) => l.clone(),
            None => return,
        };

        // 2. Remove dragged from tree + cleanup
        Self::remove_node_recursive(&mut self.root, dragged_id);
        Self::collapse_single_children_branch(&mut self.root);
        Self::fix_branch_layouts(&mut self.root);

        // 3. Determine layout and order from zone
        let (layout, dragged_first) = match zone {
            DropZone::Left  => (WindowLayout::SplitHorizontal, true),
            DropZone::Right => (WindowLayout::SplitHorizontal, false),
            DropZone::Up    => (WindowLayout::SplitVertical,   true),
            DropZone::Down  => (WindowLayout::SplitVertical,   false),
            DropZone::Center => return,
        };

        // 4. Take the current root and wrap it with the dragged leaf into a new root
        let old_root = std::mem::replace(&mut self.root, Branch {
            id: BranchId(0), // placeholder
            children: Vec::new(),
            layout: WindowLayout::Single,
            custom_rects: Vec::new(),
            proportions: Vec::new(),
            cross_ratio: None,
            preserve_if_empty: false,
        });

        let branch_id = self.next_branch_id();
        let old_root_node = PanelNode::Branch(old_root);
        let dragged_node = PanelNode::Leaf(dragged_leaf);

        let children = if dragged_first {
            vec![dragged_node, old_root_node]
        } else {
            vec![old_root_node, dragged_node]
        };

        self.root = Branch {
            id: branch_id,
            children,
            layout,
            custom_rects: Vec::new(),
            proportions: vec![0.5, 0.5],
            cross_ratio: None,
            preserve_if_empty: false,
        };
    }

    // --- Visibility ---

    pub fn hide_leaf(&mut self, id: LeafId) -> bool {
        if let Some(leaf) = self.find_leaf_mut(id) {
            leaf.hidden = true;
        }
        self.clear_custom_rects();
        true
    }

    pub fn show_leaf(&mut self, id: LeafId) {
        if let Some(leaf) = self.find_leaf_mut(id) {
            leaf.hidden = false;
        }
        self.clear_custom_rects();
    }
}

impl<P: DockPanel> Default for DockingTree<P> {
    fn default() -> Self {
        Self::new()
    }
}
