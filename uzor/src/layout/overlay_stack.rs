use crate::core::types::Rect;
use super::types::{OverlayKind, SlotId};
use super::z_layers::ZLayerTable;

/// A single open overlay registered with the layout system.
#[derive(Debug, Clone)]
pub struct OverlayEntry {
    /// Stable identifier (matches the id used when pushing the overlay).
    pub id: SlotId,
    /// Overlay kind — controls z-ordering.
    pub kind: OverlayKind,
    /// Screen-space rect of this overlay.
    pub rect: Rect,
    /// Optional anchor rect (trigger widget or cursor area) for re-positioning
    /// when the viewport changes.
    pub anchor: Option<Rect>,
}

/// Z-ordered stack of currently-open overlay entries.
///
/// The stack stores entries in insertion order; use `sort_by_z` before
/// handing the slice to a renderer so that higher-z overlays are drawn last.
#[derive(Debug, Clone, Default)]
pub struct OverlayStack {
    entries: Vec<OverlayEntry>,
}

impl OverlayStack {
    /// Create an empty overlay stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an overlay entry, replacing any existing entry with the same id.
    pub fn push(&mut self, entry: OverlayEntry) {
        self.entries.retain(|e| e.id != entry.id);
        self.entries.push(entry);
    }

    /// Remove all overlay entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Iterate over all entries in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &OverlayEntry> {
        self.entries.iter()
    }

    /// Look up an entry by id.
    pub fn get(&self, id: &str) -> Option<&OverlayEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Sort entries ascending by z (lowest z rendered first — highest drawn on top).
    pub fn sort_by_z(&mut self, table: &ZLayerTable) {
        self.entries.sort_by_key(|e| table.z_for(e.kind));
    }

    /// Return the entries slice (use after `sort_by_z` for draw order).
    pub fn entries(&self) -> &[OverlayEntry] {
        &self.entries
    }

    /// Clamp `rect` so it stays inside `viewport`, preserving size where possible.
    ///
    /// If the rect is larger than the viewport on an axis it is truncated.
    pub fn clamp_to_viewport(rect: Rect, viewport: Rect) -> Rect {
        let w = rect.width.min(viewport.width);
        let h = rect.height.min(viewport.height);

        let x = rect.x
            .max(viewport.x)
            .min(viewport.x + viewport.width - w);
        let y = rect.y
            .max(viewport.y)
            .min(viewport.y + viewport.height - h);

        Rect::new(x, y, w, h)
    }
}
