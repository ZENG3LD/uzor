//! `FocusSet` — hover/selection state, generalized from
//! `uzor-graph::interaction::focus::FocusSet`
//! (`uzor-graph/src/interaction/focus.rs`): same "one dim/highlight set
//! instead of three parallel, drifting implementations" idea (engine
//! design doc §4.3), keyed by a plain `u64` instead of graph's
//! `NodeIndex`/`EdgeIndex` — a figure casts its own row/category/point
//! index to `u64`, since uzor-figures has no node-link identity type of its
//! own (that stays in `uzor-graph`; re-pointing `uzor-graph::FocusSet`
//! onto this one is a later, separate arc — not done here).
//!
//! `generation` bumps on every actual state change so a caller can cheaply
//! skip redraw when nothing moved (the same alpha/dirty convention
//! `uzor-graph`'s engine already follows).

use std::collections::HashSet;

/// Hover (single, transient) + selection (multi, persistent) state for a
/// figure's own row/category/point ids.
#[derive(Debug, Clone, Default)]
pub struct FocusSet {
    pub hovered: Option<u64>,
    pub selected: HashSet<u64>,
    pub generation: u64,
}

impl FocusSet {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set (or clear) the hovered id. Returns `true` if it actually
    /// changed (and bumped `generation`).
    pub fn set_hovered(&mut self, id: Option<u64>) -> bool {
        if self.hovered == id {
            return false;
        }
        self.hovered = id;
        self.generation += 1;
        true
    }

    /// Add `id` to the selection. Returns `true` if it was newly added.
    pub fn select(&mut self, id: u64) -> bool {
        if !self.selected.insert(id) {
            return false;
        }
        self.generation += 1;
        true
    }

    /// Remove `id` from the selection. Returns `true` if it was present.
    pub fn deselect(&mut self, id: u64) -> bool {
        if !self.selected.remove(&id) {
            return false;
        }
        self.generation += 1;
        true
    }

    /// Select `id` if not selected, deselect it otherwise.
    pub fn toggle_selected(&mut self, id: u64) -> bool {
        if self.selected.contains(&id) {
            self.deselect(id)
        } else {
            self.select(id)
        }
    }

    /// Clear the whole selection. Returns `true` if it was non-empty.
    pub fn clear_selection(&mut self) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        self.selected.clear();
        self.generation += 1;
        true
    }

    pub fn is_hovered(&self, id: u64) -> bool {
        self.hovered == Some(id)
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    /// `true` if anything is hovered or selected.
    pub fn is_active(&self) -> bool {
        self.hovered.is_some() || !self.selected.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_hovered_bumps_generation_only_on_actual_change() {
        let mut focus = FocusSet::empty();
        assert!(focus.set_hovered(Some(3)));
        assert_eq!(focus.generation, 1);
        assert!(!focus.set_hovered(Some(3))); // same value again — no bump
        assert_eq!(focus.generation, 1);
        assert!(focus.set_hovered(None));
        assert_eq!(focus.generation, 2);
    }

    #[test]
    fn select_deselect_toggle_selected() {
        let mut focus = FocusSet::empty();
        assert!(focus.select(1));
        assert!(!focus.select(1)); // already selected
        assert!(focus.is_selected(1));

        assert!(focus.toggle_selected(1)); // -> deselected
        assert!(!focus.is_selected(1));
        assert!(focus.toggle_selected(1)); // -> selected again
        assert!(focus.is_selected(1));
    }

    #[test]
    fn clear_selection_reports_whether_anything_changed() {
        let mut focus = FocusSet::empty();
        assert!(!focus.clear_selection()); // already empty
        focus.select(1);
        focus.select(2);
        let gen_before = focus.generation;
        assert!(focus.clear_selection());
        assert!(focus.selected.is_empty());
        assert!(focus.generation > gen_before);
    }

    #[test]
    fn is_active_reflects_hover_or_selection() {
        let mut focus = FocusSet::empty();
        assert!(!focus.is_active());
        focus.set_hovered(Some(5));
        assert!(focus.is_active());
        focus.set_hovered(None);
        assert!(!focus.is_active());
        focus.select(5);
        assert!(focus.is_active());
    }
}
