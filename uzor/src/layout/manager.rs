use crate::docking::panels::{PanelDockingManager, PanelRect, DockPanel};
use crate::core::types::Rect;
use crate::app_context::{ContextManager, layout::types::LayoutNode};
use super::chrome_slot::ChromeSlot;
use super::edge_panels::EdgePanels;
use super::overlay_stack::{OverlayStack, OverlayEntry};
use super::tree::LayoutTree;
use super::z_layers::ZLayerTable;
use super::types::{OverlayKind, LayoutSolved};
use super::solve::solve_layout;

/// Convert a `Rect` (f64) to a `PanelRect` (f32) for the dock layout pass.
fn panel_rect_from_rect(r: Rect) -> PanelRect {
    PanelRect::new(r.x as f32, r.y as f32, r.width as f32, r.height as f32)
}

/// Convert a `PanelRect` (f32) to a `Rect` (f64) for rect-query results.
fn panel_rect_to_rect(pr: PanelRect) -> Rect {
    Rect::new(pr.x as f64, pr.y as f64, pr.width as f64, pr.height as f64)
}

/// Top-level macro layout owner for uzor.
///
/// `LayoutManager` is the single source of truth for all widget rects in the
/// window:
/// - **Chrome** — system titlebar/menubar strip.
/// - **Edges** — top/bottom toolbars and left/right sidebars outside the dock.
/// - **Dock** — panel docking tree (delegated to `PanelDockingManager`).
/// - **Floating** — floating panel windows (also in `PanelDockingManager`).
/// - **Overlays** — z-ordered stack: dropdown, popup, modal, context_menu, etc.
///
/// ## System vs. user access
///
/// - System slots (chrome, edges, overlays) are managed by uzor internals.
///   App developers configure but do not add/remove them at runtime.
/// - User slots (dock panels, floating panels) are accessed via `panels_mut()`,
///   which returns `&mut PanelDockingManager<P>`.
///
/// ## Frame usage
///
/// ```rust,ignore
/// layout_manager.solve(viewport);   // produces all rects
/// let dock = layout_manager.rect_for_dock_area().unwrap();
/// // register panels, chrome, edges with InputCoordinator using solved rects …
/// ```
pub struct LayoutManager<P: DockPanel> {
    chrome: ChromeSlot,
    edges: EdgePanels,
    panels: PanelDockingManager<P>,
    overlays: OverlayStack,
    z_layers: ZLayerTable,
    tree: LayoutTree,
    last_solved: Option<LayoutSolved>,
    last_window: Option<Rect>,
    /// Retained-mode context manager — owned here so Level-3 composite
    /// helpers can access it directly via `layout.ctx_mut()`.
    ctx: ContextManager,
}

impl<P: DockPanel> LayoutManager<P> {
    /// Create a new `LayoutManager` with default chrome (32 px, visible) and
    /// empty edges, panels, and overlays.
    pub fn new() -> Self {
        Self {
            chrome: ChromeSlot::default(),
            edges: EdgePanels::new(),
            panels: PanelDockingManager::new(),
            overlays: OverlayStack::new(),
            z_layers: ZLayerTable::default(),
            tree: LayoutTree::new(),
            last_solved: None,
            last_window: None,
            ctx: ContextManager::new(LayoutNode::new("__layout_root__")),
        }
    }

    // ------------------------------------------------------------------
    // System slots — uzor-internal
    // ------------------------------------------------------------------

    /// Read-only access to the chrome slot configuration.
    pub fn chrome(&self) -> &ChromeSlot {
        &self.chrome
    }

    /// Mutable access to the chrome slot configuration.
    pub fn chrome_mut(&mut self) -> &mut ChromeSlot {
        &mut self.chrome
    }

    /// Read-only access to the edge panel registry.
    pub fn edges(&self) -> &EdgePanels {
        &self.edges
    }

    /// Mutable access to the edge panel registry.
    pub fn edges_mut(&mut self) -> &mut EdgePanels {
        &mut self.edges
    }

    /// Read-only access to the overlay stack.
    pub fn overlays(&self) -> &OverlayStack {
        &self.overlays
    }

    /// Mutable access to the overlay stack.
    pub fn overlays_mut(&mut self) -> &mut OverlayStack {
        &mut self.overlays
    }

    /// Read-only access to the z-layer table.
    pub fn z_layers(&self) -> &ZLayerTable {
        &self.z_layers
    }

    /// Mutable access to the z-layer table.
    pub fn z_layers_mut(&mut self) -> &mut ZLayerTable {
        &mut self.z_layers
    }

    /// Read-only access to the embedded `ContextManager`.
    pub fn ctx(&self) -> &ContextManager {
        &self.ctx
    }

    /// Mutable access to the embedded `ContextManager`.
    ///
    /// Level-3 registration helpers call this internally to forward to
    /// `register_context_manager_*` without requiring the caller to hold a
    /// separate `ContextManager` reference.
    pub fn ctx_mut(&mut self) -> &mut ContextManager {
        &mut self.ctx
    }

    /// Read-only access to the macro layout tree (solved node rects).
    pub fn tree(&self) -> &LayoutTree {
        &self.tree
    }

    // ------------------------------------------------------------------
    // User-facing dock + floating panels
    // ------------------------------------------------------------------

    /// Read-only access to the panel docking manager.
    pub fn panels(&self) -> &PanelDockingManager<P> {
        &self.panels
    }

    /// Mutable access to the panel docking manager.
    ///
    /// App developers use this to add/remove panels, perform drag operations,
    /// and query panel rects.
    pub fn panels_mut(&mut self) -> &mut PanelDockingManager<P> {
        &mut self.panels
    }

    // ------------------------------------------------------------------
    // Per-frame solve
    // ------------------------------------------------------------------

    /// Recompute all macro-level rects given the current window size.
    ///
    /// Must be called each frame or on resize. Drives the dock layout pass
    /// internally. Returns a reference to the freshly computed `LayoutSolved`.
    pub fn solve(&mut self, window: Rect) -> &LayoutSolved {
        let solved = solve_layout(window, &self.chrome, &self.edges, &mut self.tree);

        // Drive the dock layout pass with the computed dock area.
        let dock_pr = panel_rect_from_rect(solved.dock_area);
        self.panels.layout(dock_pr);

        self.last_solved = Some(solved);
        self.last_window = Some(window);

        self.last_solved.as_ref()
            .expect("last_solved is Some — we just assigned it")
    }

    /// The result of the most recent `solve` call, or `None` if never solved.
    pub fn last_solved(&self) -> Option<&LayoutSolved> {
        self.last_solved.as_ref()
    }

    /// The window rect passed to the most recent `solve` call.
    pub fn last_window(&self) -> Option<Rect> {
        self.last_window
    }

    // ------------------------------------------------------------------
    // Rect accessors
    // ------------------------------------------------------------------

    /// Rect of the chrome strip, or `None` if not yet solved or chrome hidden.
    pub fn rect_for_chrome(&self) -> Option<Rect> {
        self.last_solved.as_ref().and_then(|s| s.chrome)
    }

    /// Dock content area, or `None` if not yet solved.
    pub fn rect_for_dock_area(&self) -> Option<Rect> {
        self.last_solved.as_ref().map(|s| s.dock_area)
    }

    /// Floating panel area (same as dock area, z-above dock), or `None` if not yet solved.
    pub fn rect_for_floating_area(&self) -> Option<Rect> {
        self.last_solved.as_ref().map(|s| s.floating_area)
    }

    /// Rect for a named overlay entry, or `None` if not present.
    pub fn rect_for_overlay(&self, id: &str) -> Option<Rect> {
        self.overlays.get(id).map(|e| e.rect)
    }

    /// Rect for a named edge slot, or `None` if not present or not yet solved.
    ///
    /// Looks up the slot in `edges` (matching by `id`), determines its position
    /// within the solved edge rects array, and returns the corresponding `Rect`.
    pub fn rect_for_edge_slot(&self, id: &str) -> Option<Rect> {
        let solved = self.last_solved.as_ref()?;
        let slot = self.edges.get(id)?;

        // Find the index of this slot among visible slots on the same side
        // (order matches the per-side Vec in `solved.edges`).
        use super::types::EdgeSide;
        let visible: Vec<_> = self.edges.slots_for(slot.side).collect();
        let idx = visible.iter().position(|s| s.id == id)?;

        let rects = match slot.side {
            EdgeSide::Top    => &solved.edges.top,
            EdgeSide::Bottom => &solved.edges.bottom,
            EdgeSide::Left   => &solved.edges.left,
            EdgeSide::Right  => &solved.edges.right,
        };
        rects.get(idx).copied()
    }

    /// Resolve a slot id to a rect by checking each layer in order:
    ///
    /// 1. `"chrome"` → chrome strip rect.
    /// 2. Edge slot id → edge slot rect via `rect_for_edge_slot`.
    /// 3. Overlay id → overlay rect via `rect_for_overlay`.
    /// 4. Dock leaf id (string form of `LeafId` display, e.g. `"Leaf(42)"`) →
    ///    dock panel rect via `panels.rect_for_leaf_str`.
    /// 5. Otherwise `None`.
    pub fn rect_for(&self, slot_id: &str) -> Option<Rect> {
        if slot_id == "chrome" {
            return self.rect_for_chrome();
        }
        if let Some(r) = self.rect_for_edge_slot(slot_id) {
            return Some(r);
        }
        if let Some(r) = self.rect_for_overlay(slot_id) {
            return Some(r);
        }
        if let Some(pr) = self.panels.rect_for_leaf_str(slot_id) {
            return Some(panel_rect_to_rect(pr));
        }
        None
    }

    // ------------------------------------------------------------------
    // Overlay helpers
    // ------------------------------------------------------------------

    /// Push an overlay entry onto the stack, replacing any existing entry with the same id.
    pub fn push_overlay(&mut self, entry: OverlayEntry) {
        self.overlays.push(entry);
    }

    /// Remove all overlay entries from the stack.
    pub fn clear_overlays(&mut self) {
        self.overlays.clear();
    }

    /// Return overlay entries sorted ascending by z (lowest z first — topmost drawn last).
    ///
    /// Sorts the internal stack in-place before returning the slice.
    pub fn overlays_in_draw_order(&mut self) -> &[OverlayEntry] {
        self.overlays.sort_by_z(&self.z_layers);
        self.overlays.entries()
    }

    /// Return the z value for the given overlay kind from the current `ZLayerTable`.
    pub fn z_for(&self, kind: OverlayKind) -> i32 {
        self.z_layers.z_for(kind)
    }
}

impl<P: DockPanel> Default for LayoutManager<P> {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{EdgeSlot, EdgeSide};

    // Minimal DockPanel impl for generic tests.
    #[derive(Clone, Debug)]
    struct DummyPanel;

    impl DockPanel for DummyPanel {
        fn title(&self) -> &str { "dummy" }
        fn type_id(&self) -> &'static str { "dummy" }
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, w, h)
    }

    // ------------------------------------------------------------------
    // solve tests
    // ------------------------------------------------------------------

    #[test]
    fn solve_chrome_only() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        // Default chrome is 32 px visible.
        let solved = lm.solve(rect(0.0, 0.0, 1920.0, 1080.0));

        let chrome = solved.chrome.expect("chrome should be Some");
        assert_eq!(chrome.y, 0.0);
        assert_eq!(chrome.height, 32.0);

        let dock = solved.dock_area;
        assert_eq!(dock.y, 32.0);
        assert_eq!(dock.height, 1048.0);
        assert_eq!(dock.width, 1920.0);
    }

    #[test]
    fn solve_with_edges() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        // Chrome 32 px (default).
        lm.edges_mut().add(EdgeSlot {
            id: "toolbar".to_string(),
            side: EdgeSide::Top,
            thickness: 40.0,
            visible: true,
            order: 0,
        });
        lm.edges_mut().add(EdgeSlot {
            id: "sidebar".to_string(),
            side: EdgeSide::Left,
            thickness: 200.0,
            visible: true,
            order: 0,
        });

        let solved = lm.solve(rect(0.0, 0.0, 1920.0, 1080.0));

        // chrome 32 + toolbar 40 = 72 consumed from top
        // sidebar 200 consumed from left
        let dock = solved.dock_area;
        assert_eq!(dock.x, 200.0, "dock starts after sidebar");
        assert_eq!(dock.y, 72.0,  "dock starts after chrome+toolbar");
        assert_eq!(dock.width,  1720.0);
        assert_eq!(dock.height, 1008.0);
    }

    #[test]
    fn solve_chrome_hidden() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        lm.chrome_mut().visible = false;

        let solved = lm.solve(rect(0.0, 0.0, 800.0, 600.0));
        assert!(solved.chrome.is_none(), "hidden chrome yields None");
        assert_eq!(solved.dock_area.y, 0.0);
        assert_eq!(solved.dock_area.height, 600.0);
    }

    // ------------------------------------------------------------------
    // z-table tests
    // ------------------------------------------------------------------

    #[test]
    fn z_table_default_ordering() {
        let lm = LayoutManager::<DummyPanel>::new();
        // modal(4) < context_menu(5) < tooltip(7)
        assert!(lm.z_for(OverlayKind::Modal) < lm.z_for(OverlayKind::ContextMenu));
        assert!(lm.z_for(OverlayKind::ContextMenu) < lm.z_for(OverlayKind::Tooltip));
    }

    #[test]
    fn z_table_override() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        lm.z_layers_mut().set(OverlayKind::Modal, 10);
        assert_eq!(lm.z_for(OverlayKind::Modal), 10);
    }

    // ------------------------------------------------------------------
    // Overlay tests
    // ------------------------------------------------------------------

    #[test]
    fn overlay_sort_by_z() {
        use super::super::overlay_stack::OverlayEntry;

        let mut lm = LayoutManager::<DummyPanel>::new();
        // push in reverse z order: tooltip(7), dropdown(2), modal(4)
        lm.push_overlay(OverlayEntry { id: "tip".to_string(), kind: OverlayKind::Tooltip,  rect: rect(0.0,0.0,1.0,1.0), anchor: None });
        lm.push_overlay(OverlayEntry { id: "dd".to_string(),  kind: OverlayKind::Dropdown, rect: rect(0.0,0.0,1.0,1.0), anchor: None });
        lm.push_overlay(OverlayEntry { id: "m".to_string(),   kind: OverlayKind::Modal,    rect: rect(0.0,0.0,1.0,1.0), anchor: None });

        let ordered = lm.overlays_in_draw_order();
        // ascending z: dropdown(2), modal(4), tooltip(7)
        assert_eq!(ordered[0].id, "dd");
        assert_eq!(ordered[1].id, "m");
        assert_eq!(ordered[2].id, "tip");
    }

    // ------------------------------------------------------------------
    // Panels accessor test
    // ------------------------------------------------------------------

    #[test]
    fn panels_accessor_compiles() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        let _panels: &mut PanelDockingManager<DummyPanel> = lm.panels_mut();
    }
}
