//! Sidebar input helpers.
//!
//! Re-exports `register_input_coordinator_sidebar` and provides lightweight
//! helpers for common input operations (resize, scroll, collapse).

pub use super::render::register_input_coordinator_sidebar;

use super::render::register_context_manager_sidebar;

use super::settings::SidebarSettings;
use super::state::{SidebarState, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH};
use super::types::{SidebarRenderKind, SidebarView};
use crate::docking::panels::DockPanel;
use crate::input::core::coordinator::LayerId;
use crate::input::{Sense, WidgetKind};
use crate::layout::{ChevronStepDirection, CompositeKind, CompositeRegistration, DispatchEvent, LayoutManager, LayoutNodeId, SidebarHandle, SidebarNode, WidgetNode};
use crate::render::RenderContext;
use crate::types::{Rect, WidgetId};
use crate::ui::widgets::atomic::text::render::draw_text;
use crate::ui::widgets::atomic::text::settings::TextSettings;
use crate::ui::widgets::atomic::text::types::{TextOverflow, TextView};
use crate::render::{TextAlign, TextBaseline};

/// Cursor position and view metadata for events that need spatial context
/// (resize start, scrollbar drag start, track click).
pub struct ConsumeEventCtx {
    /// Current pointer position in screen coordinates.
    pub cursor: (f64, f64),
    /// Resolved frame rect of the sidebar this frame.
    pub frame_rect: Rect,
    /// Viewport size used for resize cap computation.
    pub viewport: (f64, f64),
}

/// Consume a `DispatchEvent` if it belongs to this sidebar. Returns:
/// - `None` — the event was consumed (composite mutated its state).
/// - `Some(event)` — the event is not for this sidebar; pass it through.
///
/// `host_id` is the sidebar composite's WidgetId (e.g. `"sidebar-widget"`).
/// Only events whose carried id starts with `{host_id}:` (or equals `host_id`
/// for resize) are consumed.
pub fn consume_event(
    event: DispatchEvent,
    state: &mut SidebarState,
    host_id: &WidgetId,
    ctx: ConsumeEventCtx,
) -> Option<DispatchEvent> {
    match event {
        DispatchEvent::ChevronStepRequested { ref chevron_id, direction } => {
            let is_own = chevron_id.0 == format!("{}:chevron_up", host_id.0)
                || chevron_id.0 == format!("{}:chevron_down", host_id.0);
            if is_own {
                let step = 40.0_f64;
                let signed = match direction {
                    ChevronStepDirection::Up | ChevronStepDirection::Left => -step,
                    _ => step,
                };
                let scroll = state.get_or_insert_scroll("default");
                scroll.offset = (scroll.offset + signed).max(0.0);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ResizeHandleDragStarted { host_id: ref hid, edge } => {
            if hid == host_id {
                let min_size = MIN_SIDEBAR_WIDTH;
                let cap_size = (ctx.viewport.0.max(ctx.viewport.1)).max(MAX_SIDEBAR_WIDTH);
                state.start_resize(edge, ctx.frame_rect, ctx.cursor, min_size, cap_size);
                None
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarTrackClicked { ref track_id } => {
            if track_id.0 == format!("{}:scrollbar_track", host_id.0) {
                // TODO: body_y / body_h / content_h / viewport_h not available
                // on SidebarState — pass through until dimensions are wired.
                Some(event)
            } else {
                Some(event)
            }
        }
        DispatchEvent::ScrollbarThumbDragStarted { ref thumb_id } => {
            if thumb_id.0 == format!("{}:scrollbar_handle", host_id.0) {
                state.get_or_insert_scroll("default").start_drag(ctx.cursor.1);
                None
            } else {
                Some(event)
            }
        }
        _ => Some(event),
    }
}

/// Inspect sidebar state after `consume_event` returned `None` (consumed) to
/// determine what drag was started.
///
/// `which`       — app-supplied tag for the sidebar (e.g. `"main"`, `"right"`).
/// `sidebar_rect`— the sidebar frame rect this frame (used for scrollbar track geometry).
/// `est_content_h` — estimated content height in pixels (used for scrollbar math).
pub fn drag_outcome_sidebar(
    state:       &SidebarState,
    which:       &'static str,
    sidebar_rect: crate::types::Rect,
    est_content_h: f64,
) -> Option<crate::layout::DragOutcome> {
    if state.resize_drag.is_some() {
        return Some(crate::layout::DragOutcome::SidebarResize { which });
    }
    if let Some(scroll) = state.scroll_per_panel.get("default") {
        if scroll.is_dragging {
            let track_rect   = SidebarState::scrollbar_track_rect(sidebar_rect);
            let viewport_h   = track_rect.height;
            return Some(crate::layout::DragOutcome::SidebarScrollbar {
                track_rect,
                content_h:  est_content_h,
                viewport_h,
            });
        }
    }
    None
}

/// Register + draw a sidebar in one call using a [`LayoutManager`].
///
/// Resolves the rect from the edge slot identified by `slot_id`, then
/// forwards to [`register_context_manager_sidebar`].  Returns `None` if the
/// slot is not present in the edge panels.
pub fn register_layout_manager_sidebar<P: DockPanel>(
    layout:   &mut LayoutManager<P>,
    render:   &mut dyn RenderContext,
    parent:   LayoutNodeId,
    slot_id:  &str,
    handle:   &SidebarHandle,
    view:     &mut SidebarView<'_>,
    settings: &SidebarSettings,
    kind:     &SidebarRenderKind,
) -> Option<SidebarNode> {
    let id: WidgetId = handle.id.clone();
    let rect = layout.rect_for_edge_slot(slot_id)?;

    // Take state out of the map (or create default), work with it, then
    // re-insert — avoids borrow conflicts with the rest of `layout`.
    let mut state = layout.sidebars.remove(&id).unwrap_or_default();

    let layer = layout.compute_layer_for(parent);

    // Initialise size from viewport % on first registration. Top/Bottom use
    // viewport height, Left/Right/WithTypeSelector use viewport width. Once
    // sized, subsequent calls are no-ops so user resize stays sticky.
    if let Some(win) = layout.last_window() {
        let is_horizontal_kind = !matches!(kind, super::types::SidebarRenderKind::Top | super::types::SidebarRenderKind::Bottom);
        state.ensure_sized(win.width, win.height, is_horizontal_kind);
    }

    let node_id = layout.tree_mut().add_widget(parent, WidgetNode { id: id.clone(), kind: WidgetKind::Sidebar, rect, sense: Sense::CLICK });

    // Resize handle dispatcher pattern. Composite already registers a child
    // `:resize` Sense::DRAG zone on the appropriate edge; install the pattern
    // so an L1 hit translates to ResizeHandleDragStarted with the right edge,
    // and z-order filtering automatically suppresses hits under open overlays.
    {
        use crate::layout::{EventBuilder, ResizeEdge};
        let edge = match kind {
            super::types::SidebarRenderKind::Left
            | super::types::SidebarRenderKind::WithTypeSelector => ResizeEdge::E,
            super::types::SidebarRenderKind::Right              => ResizeEdge::W,
            super::types::SidebarRenderKind::Top                => ResizeEdge::S,
            super::types::SidebarRenderKind::Bottom             => ResizeEdge::N,
            super::types::SidebarRenderKind::Embedded           => ResizeEdge::E,
            super::types::SidebarRenderKind::Custom(_)          => ResizeEdge::E,
        };
        layout.dispatcher_mut().on_exact(
            format!("{}:resize", id.0),
            EventBuilder::ResizeHandle { host_id: id.clone(), edge },
        );
    }

    // Register dispatcher patterns so the inner scrollbar (when shown) gets
    // semantic events. Sidebar composite registers child rects as
    // "{id}:scrollbar_handle" (DRAG) and "{id}:scrollbar_track" (CLICK).
    if view.effective_show_scrollbar() {
        use crate::layout::EventBuilder;
        layout.dispatcher_mut().on_exact(
            format!("{}:scrollbar_track", id.0),
            EventBuilder::ScrollbarTrack { track_id: WidgetId(format!("{}:scrollbar_track", id.0)) },
        );
        layout.dispatcher_mut().on_exact(
            format!("{}:scrollbar_handle", id.0),
            EventBuilder::ScrollbarThumb { thumb_id: WidgetId(format!("{}:scrollbar_handle", id.0)) },
        );
    }

    // Chevrons mode — register paging step events on the two overlay strips.
    if matches!(view.overflow, crate::types::OverflowMode::Chevrons) {
        use crate::layout::{ChevronStepDirection, EventBuilder};
        let chev_up_id = WidgetId(format!("{}:chevron_up", id.0));
        let chev_down_id = WidgetId(format!("{}:chevron_down", id.0));
        layout.dispatcher_mut().on_exact(
            format!("{}:chevron_up", id.0),
            EventBuilder::ChevronStep { chevron_id: chev_up_id, direction: ChevronStepDirection::Up },
        );
        layout.dispatcher_mut().on_exact(
            format!("{}:chevron_down", id.0),
            EventBuilder::ChevronStep { chevron_id: chev_down_id, direction: ChevronStepDirection::Down },
        );
    }

    register_context_manager_sidebar(
        layout.ctx_mut(), render, id.clone(), rect, &mut state, view, settings, kind, &layer,
    );

    // Register this composite in the per-frame registry so consume_event can route it.
    layout.push_composite_registration(CompositeRegistration {
        kind:       CompositeKind::Sidebar,
        slot_id:    slot_id.to_string(),
        widget_id:  id.clone(),
        frame_rect: rect,
    });

    // Return state to the map.
    layout.sidebars.insert(id, state);

    Some(SidebarNode(node_id))
}

// ---------------------------------------------------------------------------
// Resize
// ---------------------------------------------------------------------------

/// Clamp a new size and apply it to `state.width` using the global pixel
/// limits `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`.
///
/// Use this when the sidebar lives on a vertical edge (Left/Right) — the
/// default min/max are sized for typical sidebar widths.
pub fn handle_sidebar_resize(state: &mut SidebarState, new_width: f64) {
    state.width = new_width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
}

/// Like [`handle_sidebar_resize`] but with explicit min/max bounds.
///
/// Top / Bottom sidebars want different limits than Left / Right because the
/// dimension being resized is height, not width. Caller passes whatever range
/// is appropriate (e.g. 60..viewport_height/2).
pub fn handle_sidebar_resize_clamped(state: &mut SidebarState, new_size: f64, min: f64, max: f64) {
    state.width = new_size.clamp(min, max);
}

// ---------------------------------------------------------------------------
// Scroll
// ---------------------------------------------------------------------------

/// Apply a scroll wheel delta to the per-panel scroll state.
///
/// `panel_id` — matches the key used in `state.scroll_per_panel`.
/// `delta`    — pixels; positive scrolls down.
/// `content_height` / `viewport_height` — needed to clamp the offset.
pub fn handle_sidebar_scroll(
    state:          &mut SidebarState,
    panel_id:       &str,
    delta:          f64,
    content_height: f64,
    viewport_height: f64,
) {
    let scroll = state.get_or_insert_scroll(panel_id);
    let max_scroll = (content_height - viewport_height).max(0.0);
    scroll.offset = (scroll.offset + delta).clamp(0.0, max_scroll);
}

// ---------------------------------------------------------------------------
// Collapse
// ---------------------------------------------------------------------------

/// Toggle the sidebar between collapsed and expanded.
pub fn handle_sidebar_collapse_toggle(state: &mut SidebarState) {
    state.toggle_collapse();
}

// ---------------------------------------------------------------------------
// SidebarBodyBuilder
// ---------------------------------------------------------------------------

/// A row entry for [`SidebarBodyBuilder::add_radio_group`].
pub struct SidebarRadioItem<'a> {
    /// Stable widget id for this radio button.
    pub id: &'a str,
    /// Display label.
    pub label: &'a str,
    /// Whether this item is currently selected.
    pub selected: bool,
}

/// A row entry for [`SidebarBodyBuilder::add_panel_list`].
pub struct SidebarPanelEntry<'a> {
    /// Widget id for the panel row's close button (e.g. `"dock-leaf-close-0"`).
    pub close_id: &'a str,
    /// Display title.
    pub title: &'a str,
    /// Whether this panel is the active leaf.
    pub active: bool,
}

/// Stateful builder for rendering and registering sidebar body content.
///
/// Create via [`SidebarBodyBuilder::new`], call item methods to add rows,
/// then call [`SidebarBodyBuilder::finish`] to end the body clip region.
///
/// The builder tracks a y-cursor internally and applies standard horizontal
/// padding (`8 px` on each side).
pub struct SidebarBodyBuilder<'a, P: DockPanel> {
    render: &'a mut dyn RenderContext,
    layout: &'a mut LayoutManager<P>,
    layer:  LayerId,
    bx:     f64,
    bw:     f64,
    y:      f64,
}

impl<'a, P: DockPanel> SidebarBodyBuilder<'a, P> {
    /// Create a new builder.
    ///
    /// `body_rect`      — screen-space rect of the sidebar body (header
    ///                    excluded).  Typically from
    ///                    `begin_body(…).content_origin_y` + `body_rect.x/width`.
    /// `content_origin_y` — first y of scrollable content (scroll already
    ///                    applied; from `SidebarBodyViewport::content_origin_y`).
    /// `layer`          — render layer to register atomics on (typically
    ///                    `LayerId::main()`).
    pub fn new(
        render:          &'a mut dyn RenderContext,
        layout:          &'a mut LayoutManager<P>,
        body_rect:        Rect,
        content_origin_y: f64,
        layer:            LayerId,
    ) -> Self {
        let bx = body_rect.x + 8.0;
        let bw = body_rect.width - 16.0;
        Self { render, layout, layer, bx, bw, y: content_origin_y + 8.0 }
    }

    /// Draw a section header label (e.g. `"NEW PANEL"`, `"PANELS"`).
    ///
    /// Advances `y` by 22 px.
    pub fn add_section_header(&mut self, text: &str) {
        draw_text(
            self.render,
            Rect::new(self.bx, self.y, self.bw, 22.0),
            &TextView { text, align: TextAlign::Left, baseline: TextBaseline::Middle,
                color: Some("rgba(255,255,255,0.4)"), font: None, overflow: TextOverflow::Clip, hovered: false },
            &TextSettings::default(),
        );
        self.y += 22.0;
    }

    /// Draw a muted sub-label (e.g. `"Type:"`, `"Split:"`).
    ///
    /// Advances `y` by 20 px.
    pub fn add_sub_label(&mut self, text: &str) {
        draw_text(
            self.render,
            Rect::new(self.bx, self.y, self.bw, 20.0),
            &TextView { text, align: TextAlign::Left, baseline: TextBaseline::Middle,
                color: Some("rgba(255,255,255,0.55)"), font: None, overflow: TextOverflow::Clip, hovered: false },
            &TextSettings::default(),
        );
        self.y += 20.0;
    }

    /// Draw a vertical spacer.
    ///
    /// Advances `y` by `height` px.
    pub fn add_spacer(&mut self, height: f64) {
        self.y += height;
    }

    /// Draw a horizontal divider line and advance y by 10 px.
    pub fn add_divider(&mut self) {
        self.render.set_fill_color("rgba(255,255,255,0.08)");
        self.render.fill_rect(self.bx, self.y, self.bw, 1.0);
        self.y += 10.0;
    }

    /// Draw a list of radio-button rows and register each as a `Button` atomic.
    ///
    /// Each row is 22 px tall.  The selected item shows a filled blue dot;
    /// unselected items show a dim dot.  Clicking any row will be reported
    /// as a click on `item.id`.
    pub fn add_radio_group(&mut self, items: &[SidebarRadioItem<'_>]) {
        let bx = self.bx;
        let bw = self.bw;
        for item in items {
            let rx = bx + 6.0;
            let ry = self.y;
            // Radio dot
            if item.selected {
                self.render.set_fill_color("#2962ff");
            } else {
                self.render.set_fill_color("rgba(255,255,255,0.18)");
            }
            self.render.fill_rounded_rect(rx, ry + 3.0, 10.0, 10.0, 5.0);
            // Label
            draw_text(
                self.render,
                Rect::new(rx + 16.0, ry, bw - 22.0, 20.0),
                &TextView {
                    text: item.label,
                    align: TextAlign::Left,
                    baseline: TextBaseline::Middle,
                    color: Some(if item.selected { "#ffffff" } else { "#a0a0b0" }),
                    font: None, overflow: TextOverflow::Clip, hovered: false,
                },
                &TextSettings::default(),
            );
            // Register hit rect (full row width for easy clicking).
            let layer = self.layer.clone();
            self.layout.ctx_mut().input.register_atomic(
                WidgetId(item.id.to_owned()),
                WidgetKind::Button,
                Rect::new(bx, ry, bw, 20.0),
                Sense::CLICK | Sense::HOVER,
                &layer,
            );
            self.y += 22.0;
        }
    }

    /// Draw a filled action button with centered label and register it.
    ///
    /// The button is 28 px tall.  Advances `y` by 36 px (button + gap).
    ///
    /// `id`    — stable widget id.
    /// `label` — button text.
    pub fn add_action_button(&mut self, id: &str, label: &str) {
        let bx = self.bx;
        let bw = self.bw;
        let y = self.y;
        self.render.set_fill_color("#2962ff");
        self.render.fill_rounded_rect(bx, y, bw, 28.0, 4.0);
        draw_text(
            self.render,
            Rect::new(bx, y, bw, 28.0),
            &TextView { text: label, align: TextAlign::Center, baseline: TextBaseline::Middle,
                color: Some("#ffffff"), font: None, overflow: TextOverflow::Clip, hovered: false },
            &TextSettings::default(),
        );
        let layer = self.layer.clone();
        self.layout.ctx_mut().input.register_atomic(
            WidgetId(id.to_owned()),
            WidgetKind::Button,
            Rect::new(bx, y, bw, 28.0),
            Sense::CLICK | Sense::HOVER,
            &layer,
        );
        self.y += 36.0;
    }

    /// Draw a list of dock-panel rows with close buttons.
    ///
    /// Each row is 30 px tall.  Active panels are highlighted in blue.
    /// For each entry, two atomics are registered:
    /// - Row click (whole row minus close zone) — id `"{entry.close_id}-row"`.
    ///   Actually, only the close button is registered; row-activation is
    ///   handled by the caller via dispatch on `entry.close_id`.
    ///
    /// `close_label` — character drawn in the close zone (default `"×"`).
    pub fn add_panel_list(
        &mut self,
        entries:     &[SidebarPanelEntry<'_>],
        close_label: &str,
    ) {
        let bx = self.bx;
        let bw = self.bw;
        for entry in entries {
            let y = self.y;
            // Row background
            self.render.set_fill_color(if entry.active {
                "rgba(41,98,255,0.18)"
            } else {
                "rgba(255,255,255,0.05)"
            });
            self.render.fill_rounded_rect(bx, y, bw, 26.0, 3.0);
            // Title
            draw_text(
                self.render,
                Rect::new(bx + 10.0, y, bw - 36.0, 26.0),
                &TextView {
                    text: entry.title,
                    align: TextAlign::Left,
                    baseline: TextBaseline::Middle,
                    color: Some(if entry.active { "#4d90fe" } else { "#d1d4dc" }),
                    font: None, overflow: TextOverflow::Clip, hovered: false,
                },
                &TextSettings::default(),
            );
            // Close button
            let close_x = bx + bw - 22.0;
            draw_text(
                self.render,
                Rect::new(close_x, y + 5.0, 16.0, 16.0),
                &TextView { text: close_label, align: TextAlign::Center,
                    baseline: TextBaseline::Middle, color: Some("rgba(255,80,80,0.5)"),
                    font: None, overflow: TextOverflow::Clip, hovered: false },
                &TextSettings::default(),
            );
            let layer = self.layer.clone();
            self.layout.ctx_mut().input.register_atomic(
                WidgetId(entry.close_id.to_owned()),
                WidgetKind::Button,
                Rect::new(close_x, y + 5.0, 16.0, 16.0),
                Sense::CLICK | Sense::HOVER,
                &layer,
            );
            self.y += 30.0;
        }
    }

    /// End the body clip region.  Must be called after all `add_*` methods.
    pub fn finish(self) {
        self.render.restore();
    }

    /// Current y-cursor position (useful for custom content between builder calls).
    pub fn current_y(&self) -> f64 {
        self.y
    }
}
