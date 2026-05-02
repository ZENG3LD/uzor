use super::types::EdgeSide;

/// How an edge slot relates to the dock area.
///
/// - **Compress** (default): the slot eats `thickness` pixels off the dock
///   area. Center content (panels, etc.) shrinks to fit. Use this for
///   primary navigation and sidebars that should never overlap content.
/// - **Overlay**: the slot reserves visual space at the edge but the dock
///   area keeps its full size. Render order makes the slot appear on top
///   of the dock content. Use this for transient panels (notifications,
///   floating toolboxes, drawers that animate in over content).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EdgePlacement {
    /// Subtract `thickness` from the dock area. Default for traditional
    /// app-shell layout.
    #[default]
    Compress,
    /// Don't touch the dock area; the slot draws over it.
    Overlay,
}

/// A single named edge panel slot (toolbar strip or sidebar column).
///
/// Slots on the same side are stacked in `order` ascending: lower order = closer
/// to the viewport edge.
///
/// Use struct-update syntax to opt into the new `placement` field without
/// touching every existing call site:
/// ```ignore
/// EdgeSlot { id: "right-drawer".into(), side: EdgeSide::Right, thickness: 240.0,
///            visible: true, placement: EdgePlacement::Overlay, ..Default::default() }
/// ```
#[derive(Debug, Clone, Default)]
pub struct EdgeSlot {
    /// Stable identifier (used for rect queries and input registration).
    pub id: String,
    /// Which edge this slot belongs to.
    pub side: EdgeSide,
    /// Preferred thickness in logical pixels: height for Top/Bottom, width for Left/Right.
    pub thickness: f32,
    /// Whether this slot is visible. Hidden slots contribute zero thickness.
    pub visible: bool,
    /// Stack order within the edge; lower = outer (closer to viewport border).
    pub order: u32,
    /// Whether the slot compresses the dock area or overlays on top of it.
    /// Default: `Compress`.
    pub placement: EdgePlacement,
}

/// Registry of all edge panel slots for all four sides.
///
/// Managed internally by `LayoutManager`; app developers add/remove slots via
/// `layout_manager.edges_mut()`.
#[derive(Debug, Clone, Default)]
pub struct EdgePanels {
    slots: Vec<EdgeSlot>,
}

impl EdgePanels {
    /// Create an empty edge panel registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add (or replace) a slot. If a slot with the same `id` already exists it
    /// is removed first so the new one takes its place.
    pub fn add(&mut self, slot: EdgeSlot) {
        self.remove(&slot.id);
        self.slots.push(slot);
    }

    /// Remove the slot with the given id (no-op if absent).
    pub fn remove(&mut self, id: &str) {
        self.slots.retain(|s| s.id != id);
    }

    /// Forget every slot. The app re-adds whatever is needed each frame —
    /// slots that aren't re-added simply disappear and the layout solver
    /// gives the freed space back to the dock area.
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// Look up a slot by id.
    pub fn get(&self, id: &str) -> Option<&EdgeSlot> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// Look up a slot mutably by id.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut EdgeSlot> {
        self.slots.iter_mut().find(|s| s.id == id)
    }

    /// Iterate over all registered slots regardless of side or visibility.
    pub fn iter(&self) -> impl Iterator<Item = &EdgeSlot> {
        self.slots.iter()
    }

    /// Iterate over visible slots for a given side, in `order` ascending.
    pub fn slots_for(&self, side: EdgeSide) -> impl Iterator<Item = &EdgeSlot> {
        // Collect + sort by order; returning a vec iterator is fine — edge counts are tiny.
        let mut v: Vec<&EdgeSlot> = self.slots
            .iter()
            .filter(move |s| s.side == side && s.visible)
            .collect();
        v.sort_by_key(|s| s.order);
        v.into_iter()
    }

    /// Sum of `thickness` for all visible slots on a given side.
    pub fn total_thickness(&self, side: EdgeSide) -> f32 {
        self.slots_for(side).map(|s| s.thickness).sum()
    }
}
