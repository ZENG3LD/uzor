//! The atomic half of the `lm` set: text, checkbox, toggle, separator,
//! chevron, tooltip. Each call registers with the hook's coordinator and
//! draws with the library function.

use uzor::render::RenderContext;
use uzor::ui::widgets::atomic::checkbox::{
    draw_checkbox, register_input_coordinator_checkbox, CheckboxRenderKind, CheckboxSettings,
    CheckboxState, CheckboxView,
};
use uzor::ui::widgets::atomic::chevron::{
    draw_chevron, register_input_coordinator_chevron, ChevronSettings, ChevronView,
};
use uzor::ui::widgets::atomic::separator::{
    draw_separator, register_input_coordinator_separator, SeparatorDragState, SeparatorKind,
    SeparatorSettings, SeparatorView,
};
use uzor::ui::widgets::atomic::text::{
    draw_text, register_input_coordinator_text, TextSettings, TextView,
};
use uzor::ui::widgets::atomic::toggle::{
    draw_toggle, register_input_coordinator_toggle, ToggleRenderKind, ToggleSettings, ToggleState,
    ToggleView,
};
use uzor::ui::widgets::atomic::tooltip::{
    draw_tooltip, register_input_coordinator_tooltip, TooltipConfig, TooltipSettings, TooltipState,
};
use uzor::Rect;
use uzor::WidgetId;

use crate::handle::{VisualView, Widgets};
use crate::types::spec::Spec;

use super::host::ContentCx;
use super::paint::{interaction_state, parts};

/// Register and draw a text label. Hover colour is `view.hovered` (the
/// library field), not a separate widget state.
pub fn text<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    view: &TextView<'_>,
    settings: &TextSettings,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        paint_text(render, widgets, id, rect, view, settings);
    });
}

pub(crate) fn paint_text(
    render: &mut dyn RenderContext,
    widgets: &mut Widgets<'_>,
    id: WidgetId,
    rect: Rect,
    view: &TextView<'_>,
    settings: &TextSettings,
) {
    let (coord, _states, layer) = parts(widgets);
    register_input_coordinator_text(coord, id, rect, layer);
    draw_text(render, rect, view, settings);
}

/// Register and draw a checkbox. `font` is the label font shorthand.
pub fn checkbox<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    view: &CheckboxView<'_>,
    settings: &CheckboxSettings,
    kind: &CheckboxRenderKind<'_>,
    font: &str,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, visual, _tokens| {
        paint_checkbox(
            render, widgets, visual, id, rect, view, settings, kind, font,
        );
    });
}

pub(crate) fn paint_checkbox<S: Spec>(
    render: &mut dyn RenderContext,
    widgets: &mut Widgets<'_>,
    visual: &VisualView<S>,
    id: WidgetId,
    rect: Rect,
    view: &CheckboxView<'_>,
    settings: &CheckboxSettings,
    kind: &CheckboxRenderKind<'_>,
    font: &str,
) {
    let ws = interaction_state(visual, &id, false);
    let (coord, states, layer) = parts(widgets);
    let state = states.get_or_insert_with(id.clone(), CheckboxState::default);
    register_input_coordinator_checkbox(coord, id, rect, layer, state);
    draw_checkbox(render, rect, ws, view, settings, kind, font);
}

/// Register and draw a toggle. Disabled comes from `view.disabled`.
pub fn toggle<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    view: &ToggleView<'_>,
    settings: &ToggleSettings,
    kind: &ToggleRenderKind<'_>,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, visual, _tokens| {
        paint_toggle(render, widgets, visual, id, rect, view, settings, kind);
    });
}

pub(crate) fn paint_toggle<S: Spec>(
    render: &mut dyn RenderContext,
    widgets: &mut Widgets<'_>,
    visual: &VisualView<S>,
    id: WidgetId,
    rect: Rect,
    view: &ToggleView<'_>,
    settings: &ToggleSettings,
    kind: &ToggleRenderKind<'_>,
) {
    let ws = interaction_state(visual, &id, view.disabled);
    let (coord, states, layer) = parts(widgets);
    let state = states.get_or_insert_with(id.clone(), ToggleState::default);
    register_input_coordinator_toggle(coord, id, rect, layer, state);
    draw_toggle(render, rect, ws, view, settings, kind, |_, _, _, _| {});
}

/// Register and draw a separator. `kind` picks the hit sense (divider or
/// resize handle); `view` picks the paint.
pub fn separator<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    kind: SeparatorKind,
    view: &SeparatorView,
    settings: &SeparatorSettings,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        paint_separator(render, widgets, id, rect, kind, view, settings);
    });
}

pub(crate) fn paint_separator(
    render: &mut dyn RenderContext,
    widgets: &mut Widgets<'_>,
    id: WidgetId,
    rect: Rect,
    kind: SeparatorKind,
    view: &SeparatorView,
    settings: &SeparatorSettings,
) {
    let (coord, states, layer) = parts(widgets);
    let state = states.get_or_insert_with(id.clone(), SeparatorDragState::default);
    register_input_coordinator_separator(coord, id, rect, kind, layer, state);
    draw_separator(render, rect, view, settings);
}

/// Register and draw a chevron. The hit rect follows `view.hit_area`.
pub fn chevron<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    view: &ChevronView,
    settings: &ChevronSettings,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        paint_chevron(render, widgets, id, rect, view, settings);
    });
}

pub(crate) fn paint_chevron(
    render: &mut dyn RenderContext,
    widgets: &mut Widgets<'_>,
    id: WidgetId,
    rect: Rect,
    view: &ChevronView,
    settings: &ChevronSettings,
) {
    let (coord, _states, layer) = parts(widgets);
    register_input_coordinator_chevron(coord, id, rect, view, layer);
    draw_chevron(render, rect, view, settings);
}

/// Register and draw a tooltip at `alpha` (0.0..=1.0, caller-animated).
pub fn tooltip<'a, C: ContentCx<'a>>(
    cx: &mut C,
    id: impl Into<WidgetId>,
    rect: Rect,
    config: &TooltipConfig,
    alpha: f64,
    settings: &TooltipSettings,
) {
    let id = id.into();
    cx.with_paint(|render, widgets, _visual, _tokens| {
        paint_tooltip(render, widgets, id, rect, config, alpha, settings);
    });
}

pub(crate) fn paint_tooltip(
    render: &mut dyn RenderContext,
    widgets: &mut Widgets<'_>,
    id: WidgetId,
    rect: Rect,
    config: &TooltipConfig,
    alpha: f64,
    settings: &TooltipSettings,
) {
    let (coord, states, layer) = parts(widgets);
    let state = states.get_or_insert_with(id.clone(), TooltipState::default);
    register_input_coordinator_tooltip(coord, id, rect, layer, state);
    draw_tooltip(render, rect, config, alpha, settings);
}
