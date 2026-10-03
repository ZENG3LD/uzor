//! Rect solve (design §3.2.1): the library macro solver for chrome →
//! edge slots → dock area, then the dock's own layout for leaves,
//! separators, tab bars and corners. Pure calls into `uzor`; the results
//! are the only source of rects for hit-testing and compose.

use uzor::layout::docking::{DockPanel, PanelRect};
use uzor::layout::{solve_layout, EdgeSide, LayoutSolved};
use uzor::Rect;

use crate::types::snapshot::EdgeSlotView;
use crate::types::window::Point;

use super::WindowLayout;

/// Re-solve one window from its viewport, chrome slot, edge slots and dock
/// tree. Idempotent.
pub(super) fn solve<P: DockPanel>(w: &mut WindowLayout<P>) {
    let content = super::expand::content_rect(w);
    w.solved = solve_layout(content, &w.chrome_slot, &w.edges, &mut w.tree);
    w.dock.layout(to_panel(w.solved.dock_area));
}

/// Half-open containment: the right and bottom borders belong to the
/// neighbour, so adjacent rects never both contain a point.
pub(super) fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.x && p.x < r.x + r.width && p.y >= r.y && p.y < r.y + r.height
}

/// Half-open containment for library `f32` rects.
pub(super) fn contains_panel(r: &PanelRect, p: Point) -> bool {
    contains(from_panel(*r), p)
}

/// Is `p` inside a visible edge slot?
pub(super) fn in_edge_slot(solved: &LayoutSolved, p: Point) -> bool {
    let e = &solved.edges;
    e.top
        .iter()
        .chain(&e.bottom)
        .chain(&e.left)
        .chain(&e.right)
        .any(|r| contains(*r, p))
}

/// Published edge-slot views, in the order the slots were added (hidden
/// slots with an empty rect).
pub(super) fn edge_views<P: DockPanel>(w: &WindowLayout<P>) -> Vec<EdgeSlotView> {
    let e = &w.solved.edges;
    let mut out = Vec::new();
    for slot in w.edges.iter() {
        let rect = if slot.visible {
            let rects = match slot.side {
                EdgeSide::Top => &e.top,
                EdgeSide::Bottom => &e.bottom,
                EdgeSide::Left => &e.left,
                EdgeSide::Right => &e.right,
            };
            // `solved.edges.<side>` follows `slots_for(side)` order.
            w.edges
                .slots_for(slot.side)
                .position(|s| s.id == slot.id)
                .and_then(|i| rects.get(i).copied())
                .unwrap_or_default()
        } else {
            Rect::default()
        };
        out.push(EdgeSlotView {
            id: slot.id.clone(),
            side: slot.side,
            visible: slot.visible,
            rect,
        });
    }
    out
}

/// Framework rect (`f64`) to library dock rect (`f32`).
pub(super) fn to_panel(r: Rect) -> PanelRect {
    PanelRect::new(r.x as f32, r.y as f32, r.width as f32, r.height as f32)
}

/// Library dock rect (`f32`) to framework rect (`f64`).
pub(super) fn from_panel(r: PanelRect) -> Rect {
    Rect::new(r.x as f64, r.y as f64, r.width as f64, r.height as f64)
}
