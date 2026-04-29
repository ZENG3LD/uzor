use super::types::EdgeSide;

/// A single named edge panel slot (toolbar strip or sidebar column).
///
/// Slots on the same side are stacked in `order` ascending: lower order = closer
/// to the viewport edge.
#[derive(Debug, Clone)]
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
