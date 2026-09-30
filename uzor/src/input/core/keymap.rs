//! App-owned typed keymap: chords bound to the app's own action type at
//! three scopes, resolved by precedence.
//!
//! uzor never names an app action. The app owns a `KeymapRegistry<A>` of
//! its own enum, fills it each frame the same immediate-mode way
//! `ClickDispatcher` is filled (clear, then bind while building the UI),
//! and on each key press asks `resolve` with the focused widget and the
//! active overlay the coordinator reports.

use std::collections::HashMap;

use crate::input::keyboard::shortcuts::KeyboardShortcut;
use crate::types::WidgetId;

/// One chord bound to an action.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyBinding<A> {
    pub chord: KeyboardShortcut,
    pub action: A,
}

/// Chords bound at global, per-overlay and per-widget scope.
///
/// Precedence in [`Self::resolve`]: the focused widget's bindings, then the
/// active overlay's, then global — the most specific context wins. Within
/// one scope the binding registered last wins.
#[derive(Clone, Debug)]
pub struct KeymapRegistry<A: Clone> {
    global: Vec<KeyBinding<A>>,
    per_overlay: HashMap<WidgetId, Vec<KeyBinding<A>>>,
    per_widget: HashMap<WidgetId, Vec<KeyBinding<A>>>,
}

impl<A: Clone> Default for KeymapRegistry<A> {
    fn default() -> Self {
        Self { global: Vec::new(), per_overlay: HashMap::new(), per_widget: HashMap::new() }
    }
}

impl<A: Clone> KeymapRegistry<A> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop every binding; call once per frame before re-binding.
    pub fn clear(&mut self) {
        self.global.clear();
        self.per_overlay.clear();
        self.per_widget.clear();
    }

    /// Bind `chord` for the whole app.
    pub fn bind_global(&mut self, chord: KeyboardShortcut, action: A) {
        self.global.push(KeyBinding { chord, action });
    }

    /// Bind `chord` while the overlay owned by `owner` is the active one.
    pub fn bind_overlay(&mut self, owner: impl Into<WidgetId>, chord: KeyboardShortcut, action: A) {
        self.per_overlay.entry(owner.into()).or_default().push(KeyBinding { chord, action });
    }

    /// Bind `chord` while `widget` has keyboard focus.
    pub fn bind_widget(&mut self, widget: impl Into<WidgetId>, chord: KeyboardShortcut, action: A) {
        self.per_widget.entry(widget.into()).or_default().push(KeyBinding { chord, action });
    }

    /// Action for `chord` given the focused widget and the active overlay,
    /// or `None` when nothing is bound. Chords match exactly (key and
    /// modifiers).
    pub fn resolve(
        &self,
        chord: &KeyboardShortcut,
        focused: Option<&WidgetId>,
        active_overlay: Option<&WidgetId>,
    ) -> Option<A> {
        let last_match = |bindings: &[KeyBinding<A>]| {
            bindings.iter().rev().find(|b| &b.chord == chord).map(|b| b.action.clone())
        };
        focused
            .and_then(|id| self.per_widget.get(id))
            .and_then(|b| last_match(b))
            .or_else(|| active_overlay.and_then(|id| self.per_overlay.get(id)).and_then(|b| last_match(b)))
            .or_else(|| last_match(&self.global))
    }

    /// Whether no chord is bound at any scope.
    pub fn is_empty(&self) -> bool {
        self.global.is_empty() && self.per_overlay.is_empty() && self.per_widget.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::keyboard::events::KeyCode;

    #[derive(Clone, Debug, PartialEq)]
    enum Action {
        Save,
        CloseModal,
        SubmitField,
        SaveAs,
    }

    fn ctrl_s() -> KeyboardShortcut {
        KeyboardShortcut::command(KeyCode::S)
    }

    fn registry() -> KeymapRegistry<Action> {
        let mut r = KeymapRegistry::new();
        r.bind_global(ctrl_s(), Action::Save);
        r.bind_overlay("modal", ctrl_s(), Action::CloseModal);
        r.bind_widget("modal/field", ctrl_s(), Action::SubmitField);
        r
    }

    // H2 §7 scenario 5 — keymap precedence.
    #[test]
    fn focused_widget_beats_overlay_beats_global() {
        let r = registry();
        let modal = WidgetId::new("modal");
        let field = WidgetId::new("modal/field");
        assert_eq!(r.resolve(&ctrl_s(), Some(&field), Some(&modal)), Some(Action::SubmitField));
        assert_eq!(r.resolve(&ctrl_s(), None, Some(&modal)), Some(Action::CloseModal));
        assert_eq!(r.resolve(&ctrl_s(), None, None), Some(Action::Save));
    }

    #[test]
    fn a_scope_without_the_chord_falls_through() {
        let r = registry();
        let other = WidgetId::new("elsewhere");
        let modal = WidgetId::new("modal");
        assert_eq!(r.resolve(&ctrl_s(), Some(&other), Some(&modal)), Some(Action::CloseModal));
        assert_eq!(r.resolve(&ctrl_s(), Some(&other), Some(&other)), Some(Action::Save));
    }

    #[test]
    fn last_binding_in_a_scope_wins_and_chords_match_exactly() {
        let mut r = KeymapRegistry::new();
        r.bind_global(ctrl_s(), Action::Save);
        r.bind_global(ctrl_s(), Action::SaveAs);
        assert_eq!(r.resolve(&ctrl_s(), None, None), Some(Action::SaveAs));
        assert_eq!(r.resolve(&KeyboardShortcut::key(KeyCode::S), None, None), None);
    }

    #[test]
    fn clear_drops_every_scope() {
        let mut r = registry();
        assert!(!r.is_empty());
        r.clear();
        assert!(r.is_empty());
        assert_eq!(r.resolve(&ctrl_s(), None, None), None);
    }
}
