//! `lm::dock_area` — L4 helper around the `LayoutManager` docking engine.
//!
//! The docking machinery (split tree, leaf rects, separators, floating
//! windows, drop zones) lives entirely on `LayoutManager` via
//! [`panels_mut()`](crate::layout::LayoutManager::panels_mut).  This builder
//! wires the standard pieces of a multi-leaf dock UI in one call:
//!
//! 1. Calls [`LayoutManager::register_dock_separators`] so each separator
//!    has a `dock-sep-N` hit-zone (drag handled by the L4 manager).
//! 2. Paints separator strips in the active style.
//! 3. Iterates `panels().panel_rects()` and yields `(LeafId, &P, Rect)` to
//!    a caller-supplied closure that fills each leaf with whatever
//!    composite it wants (`lm::panel`, `lm::blackbox`, custom paint).
//!
//! Floating-window / tab-drag / edge-expand are layered on top of this
//! same hit-zone infrastructure in subsequent helpers.

use crate::core::types::Rect;
use crate::layout::docking::{DockPanel, LeafId};
use crate::input::core::coordinator::LayerId;
use crate::layout::LayoutManager;
use crate::render::RenderContext;

/// Per-leaf summary handed to the caller's body closure.
pub struct DockLeafInfo<'a, P: DockPanel> {
    pub leaf_id: LeafId,
    pub rect:    Rect,
    /// Active panel of this leaf (the first / only panel for plain layouts;
    /// for tabbed leaves callers can read the rest via `layout.panels()`
    /// directly).
    pub panel:   &'a P,
}

/// Chainable builder for the dock area.
pub struct DockArea {
    layer:           LayerId,
    paint_separators: bool,
}

/// Entry point — start a `DockArea` builder.
pub fn dock_area() -> DockArea {
    DockArea {
        layer:           LayerId::main(),
        paint_separators: true,
    }
}

impl DockArea {
    /// Override the input-coordinator layer used for separator hit-zones.
    pub fn layer(mut self, layer: LayerId) -> Self { self.layer = layer; self }

    /// Toggle separator painting (default `true`).  Hit-zones are still
    /// registered so drag continues to work even with painting disabled.
    pub fn paint_separators(mut self, on: bool) -> Self { self.paint_separators = on; self }

    /// Terminal call — register separators, paint them, and iterate leaf
    /// rects.  The body closure is invoked once per leaf in registration
    /// order; it is responsible for painting / registering whatever
    /// composite belongs in that leaf.
    pub fn build<P, F>(
        self,
        layout: &mut LayoutManager<P>,
        render: &mut dyn RenderContext,
        mut body:   F,
    )
    where
        P: DockPanel,
        F: FnMut(&mut LayoutManager<P>, &mut dyn RenderContext, DockLeafInfo<'_, P>),
    {
        // 1. Register separator hit-zones (drag start fires
        //    DispatchEvent::DockSeparatorDragStarted; the L4 manager owns
        //    the per-frame mouse-move drag math).
        layout.register_dock_separators(&self.layer);

        // 2. Paint separator strips.
        if self.paint_separators {
            use crate::layout::docking::SeparatorOrientation as SO;
            let bg = layout.styles().color_or_owned("border_strong", "rgba(255,255,255,0.18)");
            let separators: Vec<_> = layout.panels().separators().iter().map(|s| {
                let thickness = s.thickness_for_state() as f64;
                match s.orientation {
                    SO::Vertical => Rect::new(
                        s.position as f64 - thickness / 2.0,
                        s.start    as f64,
                        thickness,
                        s.length   as f64,
                    ),
                    SO::Horizontal => Rect::new(
                        s.start    as f64,
                        s.position as f64 - thickness / 2.0,
                        s.length   as f64,
                        thickness,
                    ),
                }
            }).collect();
            render.set_fill_color(bg.as_str());
            for r in &separators {
                render.fill_rect(r.x, r.y, r.width, r.height);
            }
        }

        // 3. Iterate leaf rects and hand each to the caller.  We collect
        //    first so the closure can borrow `layout` mutably without
        //    conflicting with the panels iterator.
        let leaves: Vec<(LeafId, Rect, P)> = layout.panels()
            .panel_rects()
            .iter()
            .filter_map(|(leaf_id, panel_rect)| {
                let leaf = layout.panels().tree().leaf(*leaf_id)?;
                let panel = leaf.panels.first()?.clone();
                Some((
                    *leaf_id,
                    Rect::new(
                        panel_rect.x      as f64,
                        panel_rect.y      as f64,
                        panel_rect.width  as f64,
                        panel_rect.height as f64,
                    ),
                    panel,
                ))
            })
            .collect();

        for (leaf_id, rect, panel) in &leaves {
            body(layout, render, DockLeafInfo {
                leaf_id: *leaf_id,
                rect:    *rect,
                panel,
            });
        }
    }
}
