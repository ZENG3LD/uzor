//! App-owned typed keymap: chords bound to the app's own action type at
//! three scopes, resolved by precedence.
//!
//! uzor never names an app action. The app owns a `KeymapRegistry<A>` of
//! its own enum, fills it each frame the same immediate-mode way
//! `ClickDispatcher` is filled (clear, then bind while building the UI),
//! and on each key press asks `resolve` with the focused widget and the
//! active overlay the coordinator reports.
//!
//! The overlay key type `K` defaults to [`WidgetId`] (the overlay's owner
//! composite); an app-level engine may key overlay scopes by its own typed
//! overlay identity instead. Bindings can also be kept as long-lived
//! declarations and edited in place with the `unbind_*` / `clear_*`
//! methods instead of the per-frame clear-and-rebind.

use std::collections::HashMap;
use std::hash::Hash;

use crate::input::keyboard::shortcuts::KeyboardShortcut;
use crate::types::WidgetId;

/// One chord bound to an action.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyBinding<A> {
    pub chord: KeyboardShortcut,
    pub action: A,
}

/// The scope a resolved binding came from, most specific first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeymapLevel {
    /// Bound on the focused widget.
    Widget,
    /// Bound on the active overlay.
    Overlay,
    /// Bound for the whole app.
    Global,
}

/// Chords bound at global, per-overlay and per-widget scope.
///
/// Precedence in [`Self::resolve`]: the focused widget's bindings, then the
/// active overlay's, then global — the most specific context wins. Within
/// one scope the binding registered last wins. `K` is the overlay key
/// (default: the overlay owner's [`WidgetId`]).
#[derive(Clone, Debug)]
pub struct KeymapRegistry<A: Clone, K = WidgetId> {
    global: Vec<KeyBinding<A>>,
    per_overlay: HashMap<K, Vec<KeyBinding<A>>>,
    per_widget: HashMap<WidgetId, Vec<KeyBinding<A>>>,
}

impl<A: Clone, K> Default for KeymapRegistry<A, K> {
    fn default() -> Self {
        Self { global: Vec::new(), per_overlay: HashMap::new(), per_widget: HashMap::new() }
    }
}

impl<A: Clone> KeymapRegistry<A> {
    /// An empty registry keyed by overlay owner [`WidgetId`]s.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<A: Clone, K: Eq + Hash> KeymapRegistry<A, K> {

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
    pub fn bind_overlay(&mut self, owner: impl Into<K>, chord: KeyboardShortcut, action: A) {
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
        active_overlay: Option<&K>,
    ) -> Option<A> {
        self.resolve_with_level(chord, focused, active_overlay).map(|(_, action)| action)
    }

    /// [`Self::resolve`], also reporting which scope the action came from
    /// (e.g. so a caller can refuse global bindings while a modal is open).
    pub fn resolve_with_level(
        &self,
        chord: &KeyboardShortcut,
        focused: Option<&WidgetId>,
        active_overlay: Option<&K>,
    ) -> Option<(KeymapLevel, A)> {
        let last_match = |bindings: &[KeyBinding<A>]| {
            bindings.iter().rev().find(|b| &b.chord == chord).map(|b| b.action.clone())
        };
        focused
            .and_then(|id| self.per_widget.get(id))
            .and_then(|b| last_match(b))
            .map(|a| (KeymapLevel::Widget, a))
            .or_else(|| {
                active_overlay
                    .and_then(|id| self.per_overlay.get(id))
                    .and_then(|b| last_match(b))
                    .map(|a| (KeymapLevel::Overlay, a))
            })
            .or_else(|| last_match(&self.global).map(|a| (KeymapLevel::Global, a)))
    }

    /// Remove every global binding of `chord`; `true` if any was removed.
    pub fn unbind_global(&mut self, chord: &KeyboardShortcut) -> bool {
        remove_chord(&mut self.global, chord)
    }

    /// Remove every binding of `chord` in the overlay scope `owner`; `true`
    /// if any was removed. An emptied scope is dropped.
    pub fn unbind_overlay(&mut self, owner: &K, chord: &KeyboardShortcut) -> bool {
        remove_scoped_chord(&mut self.per_overlay, owner, chord)
    }

    /// Remove every binding of `chord` on `widget`; `true` if any was
    /// removed. An emptied scope is dropped.
    pub fn unbind_widget(&mut self, widget: &WidgetId, chord: &KeyboardShortcut) -> bool {
        remove_scoped_chord(&mut self.per_widget, widget, chord)
    }

    /// Drop every global binding; `true` if there was one.
    pub fn clear_global(&mut self) -> bool {
        let had = !self.global.is_empty();
        self.global.clear();
        had
    }

    /// Drop the overlay scope `owner`; `true` if it had bindings.
    pub fn clear_overlay(&mut self, owner: &K) -> bool {
        self.per_overlay.remove(owner).is_some()
    }

    /// Drop the widget scope `widget`; `true` if it had bindings.
    pub fn clear_widget(&mut self, widget: &WidgetId) -> bool {
        self.per_widget.remove(widget).is_some()
    }

    /// Number of bindings across every scope.
    pub fn len(&self) -> usize {
        self.global.len()
            + self.per_overlay.values().map(Vec::len).sum::<usize>()
            + self.per_widget.values().map(Vec::len).sum::<usize>()
    }

    /// Whether no chord is bound at any scope.
    pub fn is_empty(&self) -> bool {
        self.global.is_empty() && self.per_overlay.is_empty() && self.per_widget.is_empty()
    }
}

/// Remove every binding of `chord` from `bindings`; `true` if any was.
fn remove_chord<A>(bindings: &mut Vec<KeyBinding<A>>, chord: &KeyboardShortcut) -> bool {
    let before = bindings.len();
    bindings.retain(|b| &b.chord != chord);
    bindings.len() != before
}

/// [`remove_chord`] inside one keyed scope, dropping the scope once empty.
fn remove_scoped_chord<A, K: Eq + Hash>(
    scopes: &mut HashMap<K, Vec<KeyBinding<A>>>,
    key: &K,
    chord: &KeyboardShortcut,
) -> bool {
    let Some(bindings) = scopes.get_mut(key) else { return false };
    let removed = remove_chord(bindings, chord);
    if bindings.is_empty() {
        scopes.remove(key);
    }
    removed
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

    #[test]
    fn typed_overlay_keys_levels_and_in_place_edits() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        enum Overlay {
            Settings,
            Palette,
        }
        let mut r: KeymapRegistry<Action, Overlay> = KeymapRegistry::default();
        let field = WidgetId::new("field");
        r.bind_global(ctrl_s(), Action::Save);
        r.bind_overlay(Overlay::Settings, ctrl_s(), Action::CloseModal);
        r.bind_widget(field.clone(), ctrl_s(), Action::SubmitField);
        assert_eq!(r.len(), 3);
        assert_eq!(
            r.resolve_with_level(&ctrl_s(), Some(&field), Some(&Overlay::Settings)),
            Some((KeymapLevel::Widget, Action::SubmitField))
        );
        assert_eq!(
            r.resolve_with_level(&ctrl_s(), None, Some(&Overlay::Settings)),
            Some((KeymapLevel::Overlay, Action::CloseModal))
        );
        assert_eq!(
            r.resolve_with_level(&ctrl_s(), None, Some(&Overlay::Palette)),
            Some((KeymapLevel::Global, Action::Save))
        );

        // In-place edits report whether anything changed.
        assert!(r.unbind_widget(&field, &ctrl_s()));
        assert!(!r.unbind_widget(&field, &ctrl_s()));
        assert!(!r.unbind_overlay(&Overlay::Palette, &ctrl_s()));
        assert!(r.clear_overlay(&Overlay::Settings));
        assert!(!r.clear_overlay(&Overlay::Settings));
        assert!(!r.clear_widget(&field));
        assert_eq!(r.resolve(&ctrl_s(), Some(&field), Some(&Overlay::Settings)), Some(Action::Save));
        assert!(r.unbind_global(&ctrl_s()));
        assert!(!r.clear_global());
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }
}
