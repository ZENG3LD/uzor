//! Sticky chevron — registration and painting helpers.

use crate::input::{InputCoordinator, Sense, WidgetKind};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId, WidgetState, CompositeId};
use crate::ui::widgets::atomic::chevron::{
    render::draw_chevron,
    settings::ChevronSettings,
    types::{ChevronUseCase, ChevronView, HitAreaPolicy, PlacementPolicy, VisibilityPolicy},
};

use super::types::{place_sticky_chevron, StickyChevronSpec, StickyVisibility};

/// Register the chevron as a child of `host_id` in the coordinator if
/// the visibility policy is satisfied. Returns the chevron's [`WidgetId`].
///
/// `slot` — disambiguates multiple chevrons on the same host. The child
/// widget id becomes `{host_id}:chev:{slot}`. Use `"_"` for a single
/// chevron per host (produces the legacy `{host_id}:chev:_` id) or a
/// cardinal label (`"n"`, `"s"`, `"e"`, `"w"`) for 4-direction demos.
///
/// `host_state` is the host's current widget state (so we can decide
/// whether the chevron should be present this frame for `OnHostHover`).
pub fn register_sticky_chevron(
    coord:      &mut InputCoordinator,
    host_id:    &CompositeId,
    host_rect:  Rect,
    spec:       &StickyChevronSpec,
    host_state: WidgetState,
    slot:       &str,
) -> Option<WidgetId> {
    let chev_id = WidgetId::new(format!("{}:chev:{}", host_id.0.0, slot));
    let visible = match spec.visibility {
        StickyVisibility::Always     => true,
        StickyVisibility::OnHostHover => host_state.is_hovered() || host_state.is_pressed(),
        // Register so the chevron can become hovered; draw_sticky_chevron
        // handles the final paint-or-skip decision.
        StickyVisibility::OnSelfHover => true,
    };
    if !visible {
        return None;
    }
    let rect = place_sticky_chevron(host_rect, spec);
    let sense = if spec.interactive {
        Sense::CLICK | Sense::HOVER
    } else {
        Sense::NONE
    };
    coord.register_child(
        host_id,
        chev_id.clone(),
        WidgetKind::Button,
        rect,
        sense,
    );
    Some(chev_id)
}

/// Paint the chevron on top of the host. Caller passes the chevron's
/// own state (hovered / pressed / normal) which it reads from the coord.
pub fn draw_sticky_chevron(
    ctx:        &mut dyn RenderContext,
    host_rect:  Rect,
    spec:       &StickyChevronSpec,
    chev_state: WidgetState,
    host_state: WidgetState,
) {
    let visible = match spec.visibility {
        StickyVisibility::Always => true,
        StickyVisibility::OnHostHover => {
            host_state.is_hovered()
                || host_state.is_pressed()
                || chev_state.is_hovered()
                || chev_state.is_pressed()
        }
        StickyVisibility::OnSelfHover => chev_state.is_hovered() || chev_state.is_pressed(),
    };
    if !visible {
        return;
    }
    let rect = place_sticky_chevron(host_rect, spec);
    let view = ChevronView {
        direction:   spec.direction,
        use_case:    ChevronUseCase::DropdownTrigger,
        visibility:  VisibilityPolicy::Always,
        placement:   PlacementPolicy::Standalone,
        hit_area:    HitAreaPolicy::Visual,
        visual_kind: spec.visual,
        hovered:     spec.hover_visual && chev_state.is_hovered(),
        pressed:     spec.hover_visual && chev_state.is_pressed(),
        ..Default::default()
    };
    draw_chevron(ctx, rect, &view, &ChevronSettings::default());
}
