//! Shared paint helpers for the convenience functions.

use uzor::app_context::StateRegistry;
use uzor::input::InputCoordinator;
use uzor::tokens::{TokenTheme, Tokens};
use uzor::LayerId;
use uzor::WidgetId;
use uzor::WidgetState;

use crate::handle::{VisualView, Widgets};
use crate::types::spec::Spec;

use super::host::ContentCx;

/// Hover / press / disabled as the library's [`WidgetState`].
///
/// Pressed wins over hovered. Disabled (the view flag) wins over both.
/// Focus is not a [`WidgetState`] variant in the library; the focus ring
/// is the caller's.
pub(crate) fn interaction_state<S: Spec>(
    view: &VisualView<S>,
    id: &WidgetId,
    disabled: bool,
) -> WidgetState {
    if disabled {
        WidgetState::Disabled
    } else if view.is_pressed(id) {
        WidgetState::Pressed
    } else if view.is_hovered(id) {
        WidgetState::Hovered
    } else {
        WidgetState::Normal
    }
}

/// A [`TokenTheme`] over a copy of `tokens` (the theme holds a shared set).
pub(crate) fn token_theme(tokens: &Tokens) -> TokenTheme {
    TokenTheme::from_tokens(tokens)
}

/// The theme of the context the hook is painting into.
pub(crate) fn theme_of<'a, C: ContentCx<'a>>(cx: &mut C) -> TokenTheme {
    cx.with_paint(|_, _, _, tokens| token_theme(tokens))
}

/// Disjoint borrows of a [`Widgets`] face.
pub(crate) fn parts<'w, 'a>(
    widgets: &'w mut Widgets<'a>,
) -> (&'w mut InputCoordinator, &'w mut StateRegistry, &'w LayerId) {
    (&mut widgets.coord, &mut widgets.states, &widgets.layer)
}
