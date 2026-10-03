//! Button: one call registers the hit rect and paints it.

use uzor::render::RenderContext;
use uzor::ui::widgets::atomic::button::{
    draw_button, register_input_coordinator_button, ButtonSettings, ButtonState, ButtonView,
    DefaultButtonStyle,
};
use uzor::Rect;
use uzor::WidgetId;

use crate::handle::{VisualView, Widgets};
use crate::types::spec::Spec;

use super::host::ContentCx;
use super::paint::{interaction_state, parts, token_theme};

/// Register and draw a button.
///
/// This is the one-call convenience the deleted level-2
/// `register_context_manager_button` used to be, over [`Widgets`] instead of
/// `ContextManager` (design §6.4). Hover and press come from `cx`'s
/// [`VisualView`]; `view.disabled` forces the disabled colours. A click is
/// not consumed here — the app reads [`VisualView::clicked`].
pub fn button<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    view: &ButtonView<'_>,
    settings: &ButtonSettings,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, visual, _tokens| {
        paint_button(render, widgets, visual, id, rect, view, settings);
    });
}

pub(crate) fn paint_button<S: Spec>(
    render: &mut dyn RenderContext,
    widgets: &mut Widgets<'_>,
    visual: &VisualView<S>,
    id: WidgetId,
    rect: Rect,
    view: &ButtonView<'_>,
    settings: &ButtonSettings,
) {
    let ws = interaction_state(visual, &id, view.disabled);
    let (coord, states, layer) = parts(widgets);
    let state = states.get_or_insert_with(id.clone(), ButtonState::default);
    register_input_coordinator_button(coord, id, rect, layer, state);
    draw_button(render, rect, ws, view, settings, |_, _, _, _| {});
}

/// Settings whose theme is `tokens` and whose geometry is the library default.
pub(crate) fn button_settings(tokens: &uzor::tokens::Tokens) -> ButtonSettings {
    ButtonSettings {
        theme: Box::new(token_theme(tokens)),
        style: Box::new(DefaultButtonStyle),
    }
}
