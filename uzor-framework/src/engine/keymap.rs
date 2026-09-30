//! KeymapEngine: key bindings per scope and their precedence (design §3.5).
//!
//! The one writer of the binding tables. Bindings are long-lived
//! declarations edited only by app commands (`AppCommand::Keymap`, conducted
//! as [`KeymapOp::Cmd`]); there is no per-frame clear-and-rebind. The table
//! is the library's pure [`KeymapRegistry`], keyed at overlay scope by the
//! app's own overlay identity `O` (so no string or widget id names an
//! overlay).
//!
//! ## Resolution (routing key step 3, design §4.3)
//!
//! [`KeymapEngine::resolve`] is a read. Precedence: the focused widget's
//! bindings, then the top overlay's, then global; within one scope the
//! binding registered last wins; chords match exactly. While a modal is open
//! a global binding resolves only if it was bound with `through_modal`
//! (a later global binding without it shadows an earlier one with it, the
//! same "last wins" rule). Focused-widget and overlay bindings are not
//! filtered: the focus scope already keeps focus inside the modal, and the
//! top overlay is the modal or something opened above it.
//!
//! The kernel resolves only keys the InputEngine passed on
//! (`InputEffect::KeyPassed`): a focused text field's editing chords are
//! consumed in step 1, before this table is consulted, so they beat any
//! binding on the same chord.
//!
//! ## Overlay scope lifetime
//!
//! An `Overlay(o)` binding is reachable exactly while `o` is the top
//! overlay (with a keymap scope) of the window the key arrived in: the
//! kernel passes that overlay as `top_overlay`. Closing the overlay puts its
//! bindings out of reach in the same tick without any conducted op, and
//! re-opening it brings them back (bindings are declarations of the app's
//! overlay type, not of one open instance).
//!
//! ## Revision
//!
//! Bumped when a binding is added or removed; an unbind or clear that
//! removes nothing leaves it unchanged.

use std::hash::Hash;

use smallvec::SmallVec;
use uzor::input::core::keymap::{KeymapLevel, KeymapRegistry};
use uzor::input::KeyboardShortcut;
use uzor::WidgetId;

use crate::types::command::{Binding, KeymapCmd, KeymapScope};
use crate::types::ids::Revision;
use crate::types::ops::{KeymapEffect, KeymapOp};

/// Effects of one KeymapEngine op (always empty, see [`KeymapEffect`]).
pub type KeymapEffects = SmallVec<[KeymapEffect; 2]>;

/// One stored binding: the action plus its modal opt-out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry<A> {
    action: A,
    through_modal: bool,
}

/// The KeymapEngine: see the module docs. `O` is the app's overlay
/// identity, `A` its action type.
#[derive(Clone, Debug)]
pub struct KeymapEngine<O, A: Clone> {
    table: KeymapRegistry<Entry<A>, O>,
    rev: Revision,
}

impl<O, A: Clone> Default for KeymapEngine<O, A> {
    fn default() -> Self {
        Self {
            table: KeymapRegistry::default(),
            rev: Revision::ZERO,
        }
    }
}

impl<O: Copy + Eq + Hash, A: Copy + Eq> KeymapEngine<O, A> {
    /// An engine with no bindings.
    pub fn new() -> Self {
        Self::default()
    }

    /// The only mutation door. Bumps the revision iff a binding was added or
    /// removed.
    pub fn apply(&mut self, op: KeymapOp<O, A>) -> KeymapEffects {
        let changed = match op {
            KeymapOp::Cmd(cmd) => self.cmd(cmd),
        };
        if changed {
            self.rev.bump();
        }
        KeymapEffects::new()
    }

    /// The action bound to `chord`, given the focused widget of the key's
    /// window, its top overlay (with a keymap scope) and whether a modal is
    /// open there. See the module docs for precedence.
    pub fn resolve(
        &self,
        chord: &KeyboardShortcut,
        focused: Option<&WidgetId>,
        top_overlay: Option<&O>,
        modal_open: bool,
    ) -> Option<A> {
        match self.table.resolve_with_level(chord, focused, top_overlay)? {
            (KeymapLevel::Global, e) if modal_open && !e.through_modal => None,
            (_, e) => Some(e.action),
        }
    }

    /// Bumped on every observable state change.
    pub fn revision(&self) -> Revision {
        self.rev
    }

    /// Read-only projection.
    pub fn view(&self) -> KeymapEngineView<'_, O, A> {
        KeymapEngineView { e: self }
    }

    fn cmd(&mut self, cmd: KeymapCmd<O, A>) -> bool {
        match cmd {
            KeymapCmd::Bind { scope, binding } => {
                let Binding {
                    chord,
                    action,
                    through_modal,
                } = binding;
                let entry = Entry {
                    action,
                    through_modal,
                };
                match scope {
                    KeymapScope::Global => self.table.bind_global(chord, entry),
                    KeymapScope::Overlay(o) => self.table.bind_overlay(o, chord, entry),
                    KeymapScope::Focused(id) => self.table.bind_widget(id, chord, entry),
                }
                true
            }
            KeymapCmd::Unbind { scope, chord } => match scope {
                KeymapScope::Global => self.table.unbind_global(&chord),
                KeymapScope::Overlay(o) => self.table.unbind_overlay(&o, &chord),
                KeymapScope::Focused(id) => self.table.unbind_widget(&id, &chord),
            },
            KeymapCmd::Clear(scope) => match scope {
                KeymapScope::Global => self.table.clear_global(),
                KeymapScope::Overlay(o) => self.table.clear_overlay(&o),
                KeymapScope::Focused(id) => self.table.clear_widget(&id),
            },
        }
    }
}

/// Read-only projection of the [`KeymapEngine`].
#[derive(Debug)]
pub struct KeymapEngineView<'a, O, A: Clone> {
    e: &'a KeymapEngine<O, A>,
}

impl<O, A: Clone> Clone for KeymapEngineView<'_, O, A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<O, A: Clone> Copy for KeymapEngineView<'_, O, A> {}

impl<O: Copy + Eq + Hash, A: Copy + Eq> KeymapEngineView<'_, O, A> {
    /// Number of bindings across every scope.
    pub fn len(&self) -> usize {
        self.e.table.len()
    }

    /// No binding at any scope.
    pub fn is_empty(&self) -> bool {
        self.e.table.is_empty()
    }

    /// Same as [`KeymapEngine::resolve`].
    pub fn resolve(
        &self,
        chord: &KeyboardShortcut,
        focused: Option<&WidgetId>,
        top_overlay: Option<&O>,
        modal_open: bool,
    ) -> Option<A> {
        self.e.resolve(chord, focused, top_overlay, modal_open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::input::KeyCode;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Ov {
        Settings,
        Palette,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Act {
        Save,
        CloseSettings,
        SubmitField,
        Quit,
        Other,
    }

    type Engine = KeymapEngine<Ov, Act>;

    fn ctrl_s() -> KeyboardShortcut {
        KeyboardShortcut::command(KeyCode::S)
    }

    fn bind(e: &mut Engine, scope: KeymapScope<Ov>, binding: Binding<Act>) {
        e.apply(KeymapOp::Cmd(KeymapCmd::Bind { scope, binding }));
    }

    fn field() -> WidgetId {
        WidgetId::from("settings/name")
    }

    /// H2 §7 scenario 5 at engine level: the same chord at Global, overlay
    /// (the open modal) and focused widget scope.
    #[test]
    fn precedence_focused_then_overlay_then_global() {
        let mut e = Engine::new();
        bind(
            &mut e,
            KeymapScope::Global,
            Binding::new(ctrl_s(), Act::Save),
        );
        bind(
            &mut e,
            KeymapScope::Overlay(Ov::Settings),
            Binding::new(ctrl_s(), Act::CloseSettings),
        );
        bind(
            &mut e,
            KeymapScope::Focused(field()),
            Binding::new(ctrl_s(), Act::SubmitField),
        );
        let f = field();
        // Widget inside the modal focused.
        assert_eq!(
            e.resolve(&ctrl_s(), Some(&f), Some(&Ov::Settings), true),
            Some(Act::SubmitField)
        );
        // Modal on top, nothing inside focused.
        assert_eq!(
            e.resolve(&ctrl_s(), None, Some(&Ov::Settings), true),
            Some(Act::CloseSettings)
        );
        // No overlay: global.
        assert_eq!(e.resolve(&ctrl_s(), None, None, false), Some(Act::Save));
        // Another overlay on top: the settings bindings are out of reach.
        assert_eq!(
            e.resolve(&ctrl_s(), None, Some(&Ov::Palette), false),
            Some(Act::Save)
        );
        // Closing the modal (no conducted op) makes global resolve again in
        // the same tick; re-opening brings the overlay binding back.
        assert_eq!(e.resolve(&ctrl_s(), None, None, false), Some(Act::Save));
        assert_eq!(
            e.resolve(&ctrl_s(), None, Some(&Ov::Settings), true),
            Some(Act::CloseSettings)
        );
        assert_eq!(e.view().len(), 3);
    }

    #[test]
    fn through_modal_opt_out() {
        let mut e = Engine::new();
        let ctrl_q = KeyboardShortcut::command(KeyCode::Q);
        bind(
            &mut e,
            KeymapScope::Global,
            Binding::new(ctrl_s(), Act::Save),
        );
        bind(
            &mut e,
            KeymapScope::Global,
            Binding {
                chord: ctrl_q.clone(),
                action: Act::Quit,
                through_modal: true,
            },
        );
        // A modal open without its own binding: only `through_modal` resolves.
        assert_eq!(e.resolve(&ctrl_s(), None, Some(&Ov::Settings), true), None);
        assert_eq!(
            e.resolve(&ctrl_q, None, Some(&Ov::Settings), true),
            Some(Act::Quit)
        );
        // No modal: both resolve.
        assert_eq!(e.resolve(&ctrl_s(), None, None, false), Some(Act::Save));
        assert_eq!(e.resolve(&ctrl_q, None, None, false), Some(Act::Quit));
        // A later global binding without the opt-out shadows it under a modal.
        bind(
            &mut e,
            KeymapScope::Global,
            Binding::new(ctrl_q.clone(), Act::Other),
        );
        assert_eq!(e.resolve(&ctrl_q, None, None, true), None);
        assert_eq!(e.resolve(&ctrl_q, None, None, false), Some(Act::Other));
    }

    #[test]
    fn last_binding_wins_and_revision_moves_only_on_change() {
        let mut e = Engine::new();
        assert_eq!(e.revision(), Revision::ZERO);
        assert!(e.view().is_empty());
        bind(
            &mut e,
            KeymapScope::Global,
            Binding::new(ctrl_s(), Act::Save),
        );
        bind(
            &mut e,
            KeymapScope::Global,
            Binding::new(ctrl_s(), Act::Other),
        );
        assert_eq!(e.revision(), Revision(2));
        assert_eq!(e.resolve(&ctrl_s(), None, None, false), Some(Act::Other));
        assert_eq!(
            e.resolve(&KeyboardShortcut::key(KeyCode::S), None, None, false),
            None
        );

        // Removing nothing is not a change.
        e.apply(KeymapOp::Cmd(KeymapCmd::Unbind {
            scope: KeymapScope::Overlay(Ov::Palette),
            chord: ctrl_s(),
        }));
        e.apply(KeymapOp::Cmd(KeymapCmd::Clear(KeymapScope::Focused(
            field(),
        ))));
        assert_eq!(e.revision(), Revision(2));

        // Unbind removes every binding of the chord in that scope.
        e.apply(KeymapOp::Cmd(KeymapCmd::Unbind {
            scope: KeymapScope::Global,
            chord: ctrl_s(),
        }));
        assert_eq!(e.revision(), Revision(3));
        assert_eq!(e.resolve(&ctrl_s(), None, None, false), None);

        // Clear drops one scope only.
        bind(
            &mut e,
            KeymapScope::Overlay(Ov::Settings),
            Binding::new(ctrl_s(), Act::CloseSettings),
        );
        bind(
            &mut e,
            KeymapScope::Global,
            Binding::new(ctrl_s(), Act::Save),
        );
        e.apply(KeymapOp::Cmd(KeymapCmd::Clear(KeymapScope::Overlay(
            Ov::Settings,
        ))));
        assert_eq!(e.revision(), Revision(6));
        assert_eq!(
            e.resolve(&ctrl_s(), None, Some(&Ov::Settings), false),
            Some(Act::Save)
        );
        assert_eq!(e.view().len(), 1);
    }
}
