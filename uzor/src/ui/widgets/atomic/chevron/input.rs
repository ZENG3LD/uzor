//! Chevron input registration helpers — three composition layers, mirroring
//! the rest of the atomic widgets.

use crate::app_context::ContextManager;
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::layout::{LayoutManager, LayoutNodeId, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};

use super::render::draw_chevron;
use super::settings::ChevronSettings;
use super::types::{ChevronView, HitAreaPolicy};

/// L1 — register the chevron's hit rect with the coordinator only. No drawing.
///
/// Hit area is derived from `view.hit_area`:
/// - `Visual` ⇒ register `rect`.
/// - `Inflated { padding }` ⇒ register the inflated rect.
/// - `None` ⇒ skip registration entirely (the host owns the click).
pub fn register_input_coordinator_chevron(
    coord: &mut InputCoordinator,
    id:    impl Into<WidgetId>,
    rect:  Rect,
    view:  &ChevronView,
    layer: &LayerId,
) {
    let hit = match view.hit_area {
        HitAreaPolicy::None => return,
        HitAreaPolicy::Visual => rect,
        HitAreaPolicy::Inflated { padding } => Rect::new(
            rect.x - padding,
            rect.y - padding,
            rect.width  + padding * 2.0,
            rect.height + padding * 2.0,
        ),
    };
    coord.register_atomic(id, WidgetKind::ScrollChevron, hit, Sense::CLICK | Sense::HOVER, layer);
}

/// L2 — register + draw via a `ContextManager`.
pub fn register_context_manager_chevron(
    ctx:      &mut ContextManager,
    render:   &mut dyn RenderContext,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    view:     &ChevronView,
    settings: &ChevronSettings,
    layer:    &LayerId,
) {
    register_input_coordinator_chevron(&mut ctx.input, id, rect, view, layer);
    draw_chevron(render, rect, view, settings);
}

/// L3 — register + draw via a `LayoutManager`. Inserts a widget node into
/// the layout tree so introspection / debug overlays can find the chevron.
pub fn register_layout_manager_chevron<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    id:       impl Into<WidgetId>,
    rect:     Rect,
    view:     &ChevronView,
    settings: &ChevronSettings,
) {
    let id: WidgetId = id.into();
    let layer = layout.compute_layer_for(parent);
    if !matches!(view.hit_area, HitAreaPolicy::None) {
        layout.tree_mut().add_widget(parent, WidgetNode {
            id: id.clone(),
            kind: WidgetKind::ScrollChevron,
            rect,
            sense: Sense::CLICK | Sense::HOVER,
        });
    }
    register_context_manager_chevron(layout.ctx_mut(), render, id, rect, view, settings, &layer);
}
