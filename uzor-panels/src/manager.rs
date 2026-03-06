//! DockingManager — Generic panel orchestration layer
//!
//! This module provides the orchestration layer for the docking panel system.
//! It bridges the panel tree data structures with layout computation, separator
//! generation, drag-and-drop, and floating windows.
//!
//! # Architecture
//!
//! DockingManager wraps:
//! - **DockingTree<P>**: Generic N-ary panel tree
//! - **Separators**: Generated from tree branches
//! - **Floating Windows**: Panels extracted from tree
//! - **Drag State**: Panel/tab/floating window drag operations
//!
//! # Type Parameter
//!
//! - `P: DockPanel` — The panel type stored in leaves (generic over domain-specific panels)
//!
//! # Usage
//!
//! ```rust,ignore
//! use uzor_panels::{DockingManager, DockPanel, LeafId, PanelRect};
//!
//! #[derive(Clone)]
//! struct MyPanel { title: String }
//!
//! impl DockPanel for MyPanel {
//!     fn title(&self) -> &str { &self.title }
//!     fn type_id(&self) -> &'static str { "my_panel" }
//! }
//!
//! let mut manager = DockingManager::<MyPanel>::new();
//! manager.add_leaf(MyPanel { title: "Chart".to_string() });
//! manager.layout(PanelRect::new(0.0, 0.0, 1920.0, 1080.0));
//!
//! // Hit test
//! match manager.hit_test(100.0, 200.0) {
//!     HitResult::Panel(id) => println!("Hit panel {:?}", id),
//!     HitResult::Separator(idx) => println!("Hit separator {}", idx),
//!     _ => {}
//! }
//! ```

use crate::{
    DockPanel, DockingTree, Leaf, Branch, PanelNode, LeafId, PanelRect,
    Separator, SeparatorOrientation, SeparatorState, SeparatorLevel,
    SnapBackAnimation, TabBarInfo, TabItem, TabReorderState,
    FloatingWindow, FloatingWindowId, FloatingDragState,
    HitResult, CornerHandle, DropZone, PanelDragState,
};
use std::collections::HashMap;

// =============================================================================
// DockingManager
// =============================================================================

/// Generic panel manager for docking system
///
/// Orchestrates:
/// - Panel tree layout (DockingTree)
/// - Separator generation and interaction
/// - Floating windows
/// - Drag-and-drop operations
/// - Tab management
///
/// Type parameter `P` is the panel type (must implement `DockPanel` trait).
pub struct DockingManager<P: DockPanel> {
    /// Docking tree (N-ary panel tree with tabs, splits, grids)
    tree: DockingTree<P>,
    /// Computed separators (after layout)
    separators: Vec<Separator>,
    /// Computed panel rects (after layout) — keyed by LeafId
    panel_rects: HashMap<LeafId, PanelRect>,
    /// Computed panel header rects (after layout) — for drag detection
    panel_headers: HashMap<LeafId, PanelRect>,
    /// Tab bars (for Tabs containers with multiple panels)
    tab_bars: Vec<TabBarInfo>,
    /// Corner handles (separator intersections for bidirectional resize)
    corners: Vec<CornerHandle>,
    /// Layout area (full manager dimensions) — for window-edge drop detection
    layout_area: PanelRect,
    /// Window edge indicator rects (for window-level drop zones)
    /// [top, bottom, left, right]
    window_edge_rects: Option<[PanelRect; 4]>,
    /// Panel drag state (for header drag-and-drop)
    panel_drag: Option<PanelDragState>,
    /// Tab reorder state (for dragging tabs within a container)
    tab_reorder: Option<TabReorderState>,
    /// Snap-back animations for separators (when constraints violated)
    snap_animations: Vec<SnapBackAnimation>,
    /// Floating windows (extracted from tree, hovering above layout)
    floating_windows: Vec<FloatingWindow<P>>,
    /// Drag state for floating window repositioning
    floating_drag: Option<FloatingDragState>,
    /// Next floating window ID counter
    next_floating_id: u64,
    /// Hovered header (for transparent overlay headers)
    hovered_header: Option<LeafId>,
    /// Active panel (focused leaf)
    active_leaf: Option<LeafId>,
    /// Panel header height (default 24px)
    header_height: f32,
}

impl<P: DockPanel> DockingManager<P> {
    /// Create new docking manager with empty tree
    pub fn new() -> Self {
        Self {
            tree: DockingTree::new(),
            separators: Vec::new(),
            panel_rects: HashMap::new(),
            panel_headers: HashMap::new(),
            tab_bars: Vec::new(),
            corners: Vec::new(),
            layout_area: PanelRect::ZERO,
            window_edge_rects: None,
            panel_drag: None,
            tab_reorder: None,
            snap_animations: Vec::new(),
            floating_windows: Vec::new(),
            floating_drag: None,
            next_floating_id: 1,
            hovered_header: None,
            active_leaf: None,
            header_height: 24.0,
        }
    }

    /// Create manager from an existing `DockingTree`.
    ///
    /// All derived state (separators, rects, drag state, etc.) is reset to
    /// empty. Call [`layout`](Self::layout) after construction to recompute
    /// geometry.
    ///
    /// This is the primary entry-point for restoring a layout from a
    /// [`LayoutSnapshot`]:
    ///
    /// ```rust,ignore
    /// let tree = snapshot.restore_tree(|type_id| create_panel(type_id))?;
    /// let manager = DockingManager::from_tree(tree);
    /// ```
    pub fn from_tree(tree: DockingTree<P>) -> Self {
        let active_leaf = tree.active_leaf_id();
        Self {
            tree,
            separators: Vec::new(),
            panel_rects: HashMap::new(),
            panel_headers: HashMap::new(),
            tab_bars: Vec::new(),
            corners: Vec::new(),
            layout_area: PanelRect::ZERO,
            window_edge_rects: None,
            panel_drag: None,
            tab_reorder: None,
            snap_animations: Vec::new(),
            floating_windows: Vec::new(),
            floating_drag: None,
            next_floating_id: 1,
            hovered_header: None,
            active_leaf,
            header_height: 24.0,
        }
    }

    /// Create manager with single panel
    pub fn with_panel(panel: P) -> Self {
        let tree = DockingTree::with_single_leaf(panel);
        let active_leaf = tree.active_leaf_id();
        Self {
            tree,
            separators: Vec::new(),
            panel_rects: HashMap::new(),
            panel_headers: HashMap::new(),
            tab_bars: Vec::new(),
            corners: Vec::new(),
            layout_area: PanelRect::ZERO,
            window_edge_rects: None,
            panel_drag: None,
            tab_reorder: None,
            snap_animations: Vec::new(),
            floating_windows: Vec::new(),
            floating_drag: None,
            next_floating_id: 1,
            hovered_header: None,
            active_leaf,
            header_height: 24.0,
        }
    }

    // =============================================================================
    // Tree Access
    // =============================================================================

    /// Get reference to docking tree
    pub fn tree(&self) -> &DockingTree<P> {
        &self.tree
    }

    /// Get mutable reference to docking tree
    pub fn tree_mut(&mut self) -> &mut DockingTree<P> {
        &mut self.tree
    }

    // =============================================================================
    // Layout
    // =============================================================================

    /// Compute layout for all panels
    ///
    /// This walks the tree recursively and computes PanelRect for each panel.
    /// Results are stored in `panel_rects`, `panel_headers`, `separators`, and `tab_bars`.
    pub fn layout(&mut self, area: PanelRect) {
        self.layout_area = area;

        self.panel_rects.clear();
        self.panel_headers.clear();
        self.separators.clear();
        self.tab_bars.clear();

        // Layout tree (compute rects for all leaves)
        let rects = self.compute_leaf_rects(area);

        for (leaf_id, rect) in rects {
            let leaf = match self.tree.leaf(leaf_id) {
                Some(l) => l,
                None => continue,
            };

            // Store panel rect
            self.panel_rects.insert(leaf_id, rect);

            // Panel header (for single-tab panels only)
            if rect.width >= 1.0 && rect.height >= 1.0 && leaf.tab_count() <= 1 {
                self.panel_headers.insert(leaf_id, PanelRect::new(
                    rect.x, rect.y, rect.width, self.header_height,
                ));
            }

            // Tab bar (for multi-tab panels)
            if leaf.tab_count() > 1 && rect.width >= 1.0 && rect.height >= 1.0 {
                let tab_bar = self.create_tab_bar(leaf_id, &leaf, rect);
                self.tab_bars.push(tab_bar);
            }
        }

        // Generate separators recursively (clone root to avoid borrow conflict)
        let root = self.tree.root().clone();
        self.generate_separators_recursive(&root, area);

        // Detect corners at separator intersections
        self.detect_corners();
    }

    /// Compute leaf rects from tree layout
    fn compute_leaf_rects(&self, area: PanelRect) -> HashMap<LeafId, PanelRect> {
        let mut rects = HashMap::new();
        self.collect_leaf_rects_from_branch(self.tree.root(), area, &mut rects);
        rects
    }

    /// Recursively collect leaf rects from branch
    fn collect_leaf_rects_from_branch(
        &self,
        branch: &Branch<P>,
        branch_rect: PanelRect,
        out: &mut HashMap<LeafId, PanelRect>,
    ) {
        let child_rects = self.compute_child_rects(branch, branch_rect);

        for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
            match child {
                PanelNode::Leaf(leaf) => {
                    if !leaf.hidden {
                        out.insert(leaf.id, *rect);
                    }
                }
                PanelNode::Branch(b) => {
                    self.collect_leaf_rects_from_branch(b, *rect, out);
                }
            }
        }
    }

    /// Compute child rects for a branch (layout algorithm)
    fn compute_child_rects(&self, branch: &Branch<P>, area: PanelRect) -> Vec<PanelRect> {
        DockingTree::<P>::compute_child_rects(branch, area)
    }

    /// Create tab bar for multi-tab leaf
    fn create_tab_bar(&self, leaf_id: LeafId, leaf: &Leaf<P>, rect: PanelRect) -> TabBarInfo {
        let tab_bar_height = self.header_height;
        let mut tab_items = Vec::new();
        let mut tab_x_offset = 0.0_f32;

        for (i, panel) in leaf.panels.iter().enumerate() {
            let title = panel.title();
            let estimated_text_w = title.len() as f32 * 7.0; // ~7px per char
            let tab_w: f32 = (8.0 + estimated_text_w + 24.0 + 8.0).max(80.0).min(200.0);

            let remaining = rect.width - tab_x_offset;
            let tab_w = tab_w.min(remaining).max(0.0);

            let tab_rect = PanelRect::new(
                rect.x + tab_x_offset,
                rect.y,
                tab_w,
                tab_bar_height,
            );

            let close_size = 14.0;
            let close_rect = PanelRect::new(
                tab_rect.x + tab_rect.width - close_size - 4.0,
                tab_rect.y + (tab_bar_height - close_size) / 2.0,
                close_size,
                close_size,
            );

            tab_items.push(TabItem {
                panel_id: LeafId(leaf_id.0 * 100 + i as u64), // unique id per tab
                title: title.to_string(),
                rect: tab_rect,
                is_active: i == leaf.active_tab,
                close_rect,
            });

            tab_x_offset += tab_w;
        }

        let tab_bar_rect = PanelRect::new(rect.x, rect.y, tab_x_offset, tab_bar_height);

        TabBarInfo {
            container_id: leaf_id,
            rect: tab_bar_rect,
            tabs: tab_items,
        }
    }

    /// Generate separators recursively through the tree
    fn generate_separators_recursive(&mut self, branch: &Branch<P>, branch_rect: PanelRect) {
        let child_rects = self.compute_child_rects(branch, branch_rect);

        if child_rects.len() < 2 {
            // Single or no children — no separators at this level
            // But still recurse into child branches
            for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
                if let PanelNode::Branch(b) = child {
                    self.generate_separators_recursive(b, *rect);
                }
            }
            return;
        }

        // Convert to rects for adjacency detection (include both leaves and branches)
        let child_panel_rects: Vec<(u64, PanelRect)> = branch.children.iter()
            .zip(child_rects.iter())
            .filter(|(node, _)| !node.is_hidden())
            .map(|(node, wr)| (node.raw_id(), *wr))
            .collect();

        // Generate separators between adjacent children
        for i in 0..child_panel_rects.len() {
            for j in (i + 1)..child_panel_rects.len() {
                let (_, r1) = child_panel_rects[i];
                let (_, r2) = child_panel_rects[j];

                let h_overlap = r1.y.max(r2.y) < (r1.y + r1.height).min(r2.y + r2.height) - 1.0;
                let v_overlap = r1.x.max(r2.x) < (r1.x + r1.width).min(r2.x + r2.width) - 1.0;

                if h_overlap {
                    // Horizontal overlap — vertical separator
                    let left = if r1.x < r2.x { r1 } else { r2 };
                    let right = if r1.x < r2.x { r2 } else { r1 };
                    let gap = right.x - (left.x + left.width);

                    if gap <= 15.0 {
                        let sep_x = left.x + left.width + gap / 2.0;
                        let sep_y = r1.y.max(r2.y);
                        let sep_h = (r1.y + r1.height).min(r2.y + r2.height) - sep_y;
                        let (ca, cb) = if child_panel_rects[i].1.x < child_panel_rects[j].1.x {
                            (child_panel_rects[i].0, child_panel_rects[j].0)
                        } else {
                            (child_panel_rects[j].0, child_panel_rects[i].0)
                        };

                        self.separators.push(Separator::new(
                            SeparatorOrientation::Vertical,
                            sep_x, sep_y, sep_h,
                            SeparatorLevel::Node { parent_id: branch.id, child_a: ca, child_b: cb },
                        ));
                    }
                } else if v_overlap {
                    // Vertical overlap — horizontal separator
                    let top = if r1.y < r2.y { r1 } else { r2 };
                    let bottom = if r1.y < r2.y { r2 } else { r1 };
                    let gap = bottom.y - (top.y + top.height);

                    if gap <= 15.0 {
                        let sep_y = top.y + top.height + gap / 2.0;
                        let sep_x = r1.x.max(r2.x);
                        let sep_w = (r1.x + r1.width).min(r2.x + r2.width) - sep_x;
                        let (ca, cb) = if child_panel_rects[i].1.y < child_panel_rects[j].1.y {
                            (child_panel_rects[i].0, child_panel_rects[j].0)
                        } else {
                            (child_panel_rects[j].0, child_panel_rects[i].0)
                        };

                        self.separators.push(Separator::new(
                            SeparatorOrientation::Horizontal,
                            sep_y, sep_x, sep_w,
                            SeparatorLevel::Node { parent_id: branch.id, child_a: ca, child_b: cb },
                        ));
                    }
                }
            }
        }

        // Recurse into child branches
        for (child, rect) in branch.children.iter().zip(child_rects.iter()) {
            if let PanelNode::Branch(b) = child {
                self.generate_separators_recursive(b, *rect);
            }
        }
    }

    /// Detect corners at separator intersections
    fn detect_corners(&mut self) {
        self.corners.clear();
        for (vi, v_sep) in self.separators.iter().enumerate() {
            if v_sep.orientation != SeparatorOrientation::Vertical {
                continue;
            }
            for (hi, h_sep) in self.separators.iter().enumerate() {
                if h_sep.orientation != SeparatorOrientation::Horizontal {
                    continue;
                }
                self.corners.push(CornerHandle {
                    v_separator_idx: vi,
                    h_separator_idx: hi,
                    x: v_sep.position,
                    y: h_sep.position,
                });
            }
        }
    }

    // =============================================================================
    // Hit Testing
    // =============================================================================

    /// Hit test at given point
    ///
    /// Returns what's at this point (priority: corners > separators > panels > none)
    pub fn hit_test(&self, x: f32, y: f32) -> HitResult {
        // Check corners first (highest priority)
        let corner_hit_radius = 10.0_f32;
        for (i, corner) in self.corners.iter().enumerate() {
            if corner.hit_test(x, y, corner_hit_radius) {
                return HitResult::Corner(i);
            }
        }

        // Check separators
        for (idx, sep) in self.separators.iter().enumerate() {
            if sep.hit_test(x, y) {
                return HitResult::Separator(idx);
            }
        }

        // Check panels
        for (&id, rect) in &self.panel_rects {
            if rect.contains(x, y) {
                return HitResult::Panel(id);
            }
        }

        HitResult::None
    }

    // =============================================================================
    // Separator Hover
    // =============================================================================

    /// Update separator hover state based on mouse position
    /// Returns true if any separator is hovered (for cursor change)
    pub fn update_separator_hover(&mut self, x: f32, y: f32) -> bool {
        let mut any_hovered = false;
        for sep in &mut self.separators {
            if sep.hit_test(x, y) {
                sep.state = SeparatorState::Hover;
                any_hovered = true;
            } else {
                sep.state = SeparatorState::Idle;
            }
        }
        any_hovered
    }

    /// Get the orientation of the hovered separator (for cursor style)
    pub fn hovered_separator_orientation(&self) -> Option<SeparatorOrientation> {
        self.separators.iter()
            .find(|s| s.state == SeparatorState::Hover)
            .map(|s| s.orientation)
    }

    // =============================================================================
    // Panel Drag-and-Drop
    // =============================================================================

    /// Start panel drag (called when mouse down on panel header)
    pub fn start_panel_drag(&mut self, leaf_id: LeafId, x: f32, y: f32) {
        self.panel_drag = Some(PanelDragState {
            dragged_leaf_id: leaf_id,
            current_x: x,
            current_y: y,
            target_leaf_id: None,
            drop_zone: None,
            is_window_edge: false,
        });
    }

    /// Update panel drag (called on mouse move during drag)
    pub fn update_panel_drag(&mut self, x: f32, y: f32) {
        if let Some(ref mut drag) = self.panel_drag {
            drag.current_x = x;
            drag.current_y = y;
            drag.is_window_edge = false;

            let mut target = None;
            let mut zone = None;

            // Check headers first — dropping on a header always creates tabs
            for (&id, &header_rect) in &self.panel_headers {
                if id == drag.dragged_leaf_id {
                    continue; // Skip the panel being dragged
                }
                if header_rect.contains(x, y) {
                    target = Some(id);
                    zone = Some(DropZone::Center);
                    break;
                }
            }

            // Check tab bars
            if target.is_none() {
                for bar in &self.tab_bars {
                    if bar.rect.contains(x, y) {
                        if bar.container_id != drag.dragged_leaf_id {
                            target = Some(bar.container_id);
                            zone = Some(DropZone::Center);
                            break;
                        }
                    }
                }
            }

            // Check window-level edges (before panel body detection)
            if target.is_none() {
                if let Some(edge_rects) = &self.window_edge_rects {
                    let zones = [DropZone::Up, DropZone::Down, DropZone::Left, DropZone::Right];
                    for (i, rect) in edge_rects.iter().enumerate() {
                        if rect.contains(x, y) {
                            let fallback_target = self.panel_rects.keys()
                                .find(|&&id| id != drag.dragged_leaf_id)
                                .copied();
                            if let Some(ft) = fallback_target {
                                target = Some(ft);
                                zone = Some(zones[i]);
                                drag.is_window_edge = true;
                                break;
                            }
                        }
                    }
                }
            }

            // Fall back to panel body detection with drop zone algorithm
            if target.is_none() {
                for (&id, &rect) in &self.panel_rects {
                    if id == drag.dragged_leaf_id {
                        continue;
                    }
                    if rect.contains(x, y) {
                        target = Some(id);
                        let local_x = x - rect.x;
                        let local_y = y - rect.y;
                        zone = Some(Self::detect_drop_zone(local_x, local_y, rect.width, rect.height));
                        break;
                    }
                }
            }

            drag.target_leaf_id = target;
            drag.drop_zone = zone;
        }
    }

    /// End panel drag - perform the drop action, or float the leaf if no target
    pub fn end_panel_drag(&mut self, area_width: f32, area_height: f32) -> Option<FloatingWindowId> {
        let drag = match self.panel_drag.take() {
            Some(d) => d,
            None => return None,
        };

        let target_id = match drag.target_leaf_id {
            Some(id) => id,
            None => {
                // No valid target → float the leaf
                return self.float_leaf(
                    drag.dragged_leaf_id,
                    drag.current_x - 150.0,
                    drag.current_y - 150.0,
                    area_width,
                    area_height,
                );
            }
        };

        let zone = match drag.drop_zone {
            Some(z) => z,
            None => return None,
        };

        // Perform the tree restructuring based on drop zone
        self.apply_panel_drop(drag.dragged_leaf_id, target_id, zone, drag.is_window_edge);
        None
    }

    /// Cancel panel drag without dropping
    pub fn cancel_panel_drag(&mut self) {
        self.panel_drag = None;
    }

    /// Get current panel drag state (for rendering)
    pub fn panel_drag_state(&self) -> Option<&PanelDragState> {
        self.panel_drag.as_ref()
    }

    /// Detect drop zone using improved algorithm with smaller center zone
    fn detect_drop_zone(x: f32, y: f32, width: f32, height: f32) -> DropZone {
        let center_margin = 0.20;
        let cx = width * center_margin;
        let cy = height * center_margin;

        // Center zone — only if clearly in the middle
        if x > cx && x < width - cx && y > cy && y < height - cy {
            return DropZone::Center;
        }

        // Determine direction based on which edge is closest
        let dist_left = x;
        let dist_right = width - x;
        let dist_top = y;
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

    /// Apply panel drop - restructure the tree based on drop zone
    fn apply_panel_drop(&mut self, dragged_id: LeafId, target_id: LeafId, zone: DropZone, is_window_edge: bool) {
        match zone {
            DropZone::Center => {
                // Move all panels from dragged leaf as tabs into target leaf
                let panels = match self.tree.leaf(dragged_id) {
                    Some(leaf) => leaf.panels.clone(),
                    None => return,
                };

                for panel in panels {
                    self.tree.add_tab(target_id, panel);
                }

                self.tree.remove_leaf(dragged_id);
            }
            DropZone::Left | DropZone::Right | DropZone::Up | DropZone::Down => {
                if is_window_edge {
                    self.tree.move_leaf_to_root_split(dragged_id, zone);
                } else {
                    self.tree.move_leaf_to_branch(dragged_id, target_id, zone);
                }
            }
        }
    }

    // =============================================================================
    // Tab Management
    // =============================================================================

    /// Switch active tab in a leaf
    pub fn set_active_tab(&mut self, container_id: LeafId, tab_id: LeafId) {
        if let Some(leaf) = self.tree.leaf_mut(container_id) {
            let tab_idx = (tab_id.0 % 100) as usize;
            if tab_idx < leaf.panels.len() {
                leaf.active_tab = tab_idx;
            }
        }
        self.tree.set_active_leaf(container_id);
    }

    /// Close a tab (remove panel from leaf)
    pub fn close_tab(&mut self, container_id: LeafId, tab_id: LeafId) {
        let tab_idx = (tab_id.0 % 100) as usize;
        self.tree.remove_tab(container_id, tab_idx);
    }

    /// Start tab reorder drag
    pub fn start_tab_reorder(&mut self, container_id: LeafId, tab_id: LeafId, x: f32) {
        if let Some(leaf) = self.tree.leaf(container_id) {
            let tab_idx = (tab_id.0 % 100) as usize;
            if tab_idx < leaf.panels.len() {
                self.tab_reorder = Some(TabReorderState {
                    container_id,
                    dragged_tab_id: tab_id,
                    original_index: tab_idx,
                    current_x: x,
                    insert_index: tab_idx,
                });
            }
        }
    }

    /// Update tab reorder drag
    pub fn update_tab_reorder(&mut self, x: f32) {
        if let Some(ref mut state) = self.tab_reorder {
            state.current_x = x;

            // Calculate insert index based on mouse position
            if let Some(bar) = self.tab_bars.iter().find(|b| b.container_id == state.container_id) {
                let mut insert_idx = 0;
                for (idx, tab) in bar.tabs.iter().enumerate() {
                    if x > tab.rect.x + tab.rect.width / 2.0 {
                        insert_idx = idx + 1;
                    }
                }
                state.insert_index = insert_idx.min(bar.tabs.len());
            }
        }
    }

    /// End tab reorder drag - reorder tabs in leaf
    pub fn end_tab_reorder(&mut self) {
        if let Some(state) = self.tab_reorder.take() {
            if state.original_index != state.insert_index {
                if let Some(leaf) = self.tree.leaf_mut(state.container_id) {
                    if state.original_index < leaf.panels.len() {
                        let dragged = leaf.panels.remove(state.original_index);
                        let final_idx = if state.insert_index > state.original_index {
                            (state.insert_index - 1).min(leaf.panels.len())
                        } else {
                            state.insert_index.min(leaf.panels.len())
                        };
                        leaf.panels.insert(final_idx, dragged);
                    }
                }
            }
        }
    }

    // =============================================================================
    // Floating Windows
    // =============================================================================

    /// Extract leaf from tree into floating window at given position
    pub fn float_leaf(&mut self, leaf_id: LeafId, x: f32, y: f32, area_width: f32, area_height: f32) -> Option<FloatingWindowId> {
        if self.tree.visible_leaf_count() <= 1 {
            return None;
        }

        let leaf = self.tree.leaf(leaf_id)?.clone();
        self.tree.remove_leaf(leaf_id);

        let id = FloatingWindowId(self.next_floating_id);
        self.next_floating_id += 1;

        let fw = FloatingWindow {
            id,
            panels: leaf.panels,
            active_tab: leaf.active_tab,
            x: x.clamp(0.0, (area_width - 300.0).max(0.0)),
            y: y.clamp(0.0, (area_height - 300.0).max(0.0)),
            width: 300.0,
            height: 300.0,
        };

        self.floating_windows.push(fw);
        Some(id)
    }

    /// Dock floating window back into tree at target leaf with drop zone
    pub fn dock_floating(&mut self, fw_id: FloatingWindowId, target_id: LeafId, zone: DropZone, is_window_edge: bool) {
        let idx = match self.floating_windows.iter().position(|fw| fw.id == fw_id) {
            Some(i) => i,
            None => return,
        };
        let fw = self.floating_windows.remove(idx);

        if fw.panels.is_empty() {
            return;
        }

        // Re-insert into tree: add first panel, then rest as tabs
        let new_leaf_id = self.tree.add_leaf(fw.panels[0].clone());

        for panel in fw.panels.iter().skip(1) {
            self.tree.add_tab(new_leaf_id, panel.clone());
        }

        // Restore active tab
        if let Some(leaf) = self.tree.leaf_mut(new_leaf_id) {
            leaf.active_tab = fw.active_tab.min(fw.panels.len().saturating_sub(1));
        }

        // Apply drop
        self.apply_panel_drop(new_leaf_id, target_id, zone, is_window_edge);
    }

    /// Close floating window (removes it)
    pub fn close_floating(&mut self, fw_id: FloatingWindowId) {
        if let Some(idx) = self.floating_windows.iter().position(|fw| fw.id == fw_id) {
            self.floating_windows.remove(idx);
        }
    }

    /// Start dragging a floating window (reposition)
    pub fn start_floating_drag(&mut self, fw_id: FloatingWindowId, cursor_x: f32, cursor_y: f32) {
        if let Some(fw) = self.floating_windows.iter().find(|fw| fw.id == fw_id) {
            self.floating_drag = Some(FloatingDragState {
                window_id: fw_id,
                offset_x: cursor_x - fw.x,
                offset_y: cursor_y - fw.y,
                dock_target: None,
            });
        }
    }

    /// Update floating window position during drag
    pub fn update_floating_drag(&mut self, cursor_x: f32, cursor_y: f32, area_width: f32, area_height: f32) {
        if let Some(ref drag) = self.floating_drag {
            let fw_id = drag.window_id;
            let ox = drag.offset_x;
            let oy = drag.offset_y;
            if let Some(fw) = self.floating_windows.iter_mut().find(|fw| fw.id == fw_id) {
                fw.x = (cursor_x - ox).clamp(0.0, (area_width - fw.width).max(0.0));
                fw.y = (cursor_y - oy).clamp(0.0, (area_height - fw.height).max(0.0));
            }
        }
    }

    /// Update dock target detection during floating window drag
    pub fn update_floating_dock_target(&mut self, x: f32, y: f32) {
        if self.floating_drag.is_none() {
            return;
        }

        let mut target = None;
        let mut zone = None;
        let mut is_window_edge = false;

        // Check headers first
        for (&id, &header_rect) in &self.panel_headers {
            if header_rect.contains(x, y) {
                target = Some(id);
                zone = Some(DropZone::Center);
                break;
            }
        }

        // Check tab bars
        if target.is_none() {
            for bar in &self.tab_bars {
                if bar.rect.contains(x, y) {
                    target = Some(bar.container_id);
                    zone = Some(DropZone::Center);
                    break;
                }
            }
        }

        // Check window edges
        if target.is_none() {
            if let Some(edge_rects) = &self.window_edge_rects {
                let zones = [DropZone::Up, DropZone::Down, DropZone::Left, DropZone::Right];
                for (i, rect) in edge_rects.iter().enumerate() {
                    if rect.contains(x, y) {
                        let fallback_target = self.panel_rects.keys().copied().next();
                        if let Some(ft) = fallback_target {
                            target = Some(ft);
                            zone = Some(zones[i]);
                            is_window_edge = true;
                            break;
                        }
                    }
                }
            }
        }

        // Fall back to panel body
        if target.is_none() {
            for (&id, &rect) in &self.panel_rects {
                if rect.contains(x, y) {
                    target = Some(id);
                    let local_x = x - rect.x;
                    let local_y = y - rect.y;
                    zone = Some(Self::detect_drop_zone(local_x, local_y, rect.width, rect.height));
                    break;
                }
            }
        }

        if let Some(ref mut drag) = self.floating_drag {
            drag.dock_target = match (target, zone) {
                (Some(t), Some(z)) => Some((t, z, is_window_edge)),
                _ => None,
            };
        }
    }

    /// End floating window drag
    pub fn end_floating_drag(&mut self) -> Option<(FloatingWindowId, LeafId, DropZone, bool)> {
        let drag = self.floating_drag.take()?;
        if let Some((target_id, zone, is_edge)) = drag.dock_target {
            Some((drag.window_id, target_id, zone, is_edge))
        } else {
            None
        }
    }

    /// Hit test: is cursor over a floating window header?
    pub fn hit_test_floating_header(&self, x: f32, y: f32) -> Option<FloatingWindowId> {
        for fw in self.floating_windows.iter().rev() {
            let header_h = self.header_height;
            if x >= fw.x && x <= fw.x + fw.width && y >= fw.y && y <= fw.y + header_h {
                return Some(fw.id);
            }
        }
        None
    }

    /// Hit test: is cursor over a floating window body?
    pub fn hit_test_floating_body(&self, x: f32, y: f32) -> Option<FloatingWindowId> {
        for fw in self.floating_windows.iter().rev() {
            if fw.contains(x, y) {
                return Some(fw.id);
            }
        }
        None
    }

    /// Hit test: is cursor over a floating window close button?
    pub fn hit_test_floating_close(&self, x: f32, y: f32) -> Option<FloatingWindowId> {
        for fw in self.floating_windows.iter().rev() {
            let close_size = 20.0_f32;
            let close_x = fw.x + fw.width - close_size - 4.0;
            let close_y = fw.y + 2.0;
            if x >= close_x && x <= close_x + close_size && y >= close_y && y <= close_y + close_size {
                return Some(fw.id);
            }
        }
        None
    }

    // =============================================================================
    // Snap-back Animations
    // =============================================================================

    /// Update snap-back animations
    pub fn update_snap_animations(&mut self, dt: f32) {
        for anim in &mut self.snap_animations {
            anim.update(dt);
        }
        self.snap_animations.retain(|a| !a.done);
    }

    // =============================================================================
    // Window Edge Rects
    // =============================================================================

    /// Compute window edge indicator rects (for window-level drop zones)
    pub fn compute_window_edge_rects(&mut self) {
        let area = self.layout_area;
        let size = 28.0_f32;
        let half = size / 2.0;
        let cx = area.x + area.width / 2.0;
        let cy = area.y + area.height / 2.0;
        let inset = 4.0_f32;

        self.window_edge_rects = Some([
            // Top
            PanelRect::new(cx - half, area.y + inset, size, size),
            // Bottom
            PanelRect::new(cx - half, area.y + area.height - size - inset, size, size),
            // Left
            PanelRect::new(area.x + inset, cy - half, size, size),
            // Right
            PanelRect::new(area.x + area.width - size - inset, cy - half, size, size),
        ]);
    }

    // =============================================================================
    // Accessors
    // =============================================================================

    pub fn separators(&self) -> &[Separator] {
        &self.separators
    }

    pub fn panel_rects(&self) -> &HashMap<LeafId, PanelRect> {
        &self.panel_rects
    }

    pub fn panel_headers(&self) -> &HashMap<LeafId, PanelRect> {
        &self.panel_headers
    }

    pub fn tab_bars(&self) -> &[TabBarInfo] {
        &self.tab_bars
    }

    pub fn corners(&self) -> &[CornerHandle] {
        &self.corners
    }

    pub fn floating_windows(&self) -> &[FloatingWindow<P>] {
        &self.floating_windows
    }

    pub fn snap_animations(&self) -> &[SnapBackAnimation] {
        &self.snap_animations
    }

    pub fn active_leaf(&self) -> Option<LeafId> {
        self.active_leaf
    }

    pub fn set_active_leaf(&mut self, id: LeafId) {
        self.tree.set_active_leaf(id);
        self.active_leaf = Some(id);
    }

    pub fn layout_area(&self) -> PanelRect {
        self.layout_area
    }

    pub fn window_edge_rects(&self) -> Option<&[PanelRect; 4]> {
        self.window_edge_rects.as_ref()
    }

    pub fn hovered_header(&self) -> Option<LeafId> {
        self.hovered_header
    }

    pub fn set_hovered_header(&mut self, id: Option<LeafId>) {
        self.hovered_header = id;
    }

    pub fn tab_reorder_state(&self) -> Option<&TabReorderState> {
        self.tab_reorder.as_ref()
    }

    pub fn floating_drag_state(&self) -> Option<&FloatingDragState> {
        self.floating_drag.as_ref()
    }
}

impl<P: DockPanel> Default for DockingManager<P> {
    fn default() -> Self {
        Self::new()
    }
}
