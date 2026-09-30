//! Per-window `DockState` glue: dock commands, dock / floating hit-testing,
//! tab and header drag sessions, and the published dock view. The docking
//! mechanics themselves (tree surgery, drop targeting, pins, floating
//! windows) are the library's; this file only drives them.

use std::collections::BTreeSet;

use uzor::layout::docking::{
    BranchId, DockPanel, DragPayload, DropZone, FloatingWindow, FloatingWindowId, LeafId,
    PanelRect, SeparatorOrientation,
};
use uzor::{Rect, ResizeDirection};

use crate::types::command::{DockTarget, LayoutHit, LayoutPolicy, SplitDir};
use crate::types::ids::{Revision, WindowId};
use crate::types::intent::DockIntent;
use crate::types::ops::LayoutEffect;
use crate::types::snapshot::{DockView, FloatingView, LeafView, PanelRef, SeparatorView};
use crate::types::window::Point;

use super::dragout::{self, Outbound, Payload};
use super::{
    blob, chrome, rects, splitter, Ctx, DockState, LayoutEffects, Session, WindowLayout,
    DRAG_START_PX,
};

/// Hit radius of a splitter crossing, logical px.
const CORNER_RADIUS: f32 = 6.0;
/// Size of a floating window's close button (library geometry).
const FLOAT_CLOSE: f64 = 20.0;
/// Smallest size a floating window can be resized to, logical px.
const FLOAT_MIN: (f64, f64) = (120.0, 80.0);

/// Where a tab-chip session is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TabStage {
    /// Pressed, not moved past the drag threshold.
    Pressed,
    /// Reordering inside its bar.
    Reorder,
    /// Torn off: the library tab drag is live.
    Tear,
    /// The tear-off was refused (locked panel); inert until release.
    Refused,
}

/// Where a panel-header session is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HeaderStage {
    /// Pressed, not moved past the drag threshold.
    Pressed,
    /// The library leaf drag is live.
    Dragging,
    /// The drag was refused (a locked panel in the leaf).
    Refused,
}

/// Apply the engine policy to one dock.
pub(super) fn configure<P: DockPanel>(dock: &mut DockState<P>, policy: &LayoutPolicy) {
    dock.set_splitter_policy(policy.splitter);
    let reserve = if policy.tab_new_button {
        dock.tab_bar_height()
    } else {
        0.0
    };
    dock.set_tab_strip_reserve(reserve);
}

// ---------------------------------------------------------------------------
// Observation
// ---------------------------------------------------------------------------

/// A leaf as observed: id, rect, active tab, hidden, panel type ids and
/// titles.
type LeafObs = (
    LeafId,
    Option<PanelRect>,
    usize,
    bool,
    Vec<&'static str>,
    Vec<String>,
);

/// A tab bar as observed: leaf, bar rect, per chip (rect, close rect,
/// active).
type BarObs = (LeafId, PanelRect, Vec<(PanelRect, PanelRect, bool)>);

/// A live library panel drag as observed: dragged leaf, payload, pointer,
/// target leaf, zone, window-edge flag.
type DragObs = (
    LeafId,
    DragPayload,
    f32,
    f32,
    Option<LeafId>,
    Option<DropZone>,
    bool,
);

/// Everything observable about a dock, for exact revision bumps.
#[derive(PartialEq)]
pub(super) struct DockObs {
    leaves: Vec<LeafObs>,
    headers: Vec<(LeafId, PanelRect)>,
    bars: Vec<BarObs>,
    seps: Vec<(SeparatorOrientation, f32, f32, f32)>,
    floating: Vec<(FloatingWindowId, PanelRect, usize, Vec<&'static str>)>,
    active_leaf: Option<LeafId>,
    drag: Option<DragObs>,
    reorder: Option<(LeafId, usize, usize)>,
    floating_drag: Option<FloatingWindowId>,
    snaps: Vec<(usize, f32)>,
}

impl DockObs {
    pub(super) fn of<P: DockPanel>(dock: &DockState<P>) -> Self {
        let mut leaves: Vec<LeafObs> = dock
            .tree()
            .leaves()
            .iter()
            .map(|l| {
                (
                    l.id,
                    dock.panel_rects().get(&l.id).copied(),
                    l.active_tab,
                    l.hidden,
                    l.panels.iter().map(|p| p.type_id()).collect(),
                    l.panels.iter().map(|p| p.title().to_string()).collect(),
                )
            })
            .collect();
        leaves.sort_by_key(|l| l.0 .0);
        let mut headers: Vec<(LeafId, PanelRect)> =
            dock.panel_headers().iter().map(|(k, v)| (*k, *v)).collect();
        headers.sort_by_key(|h| h.0 .0);
        let mut bars: Vec<_> = dock
            .tab_bars()
            .iter()
            .map(|b| {
                (
                    b.container_id,
                    b.rect,
                    b.tabs
                        .iter()
                        .map(|t| (t.rect, t.close_rect, t.is_active))
                        .collect(),
                )
            })
            .collect();
        bars.sort_by_key(|b: &BarObs| b.0 .0);
        Self {
            leaves,
            headers,
            bars,
            seps: dock
                .separators()
                .iter()
                .map(|s| (s.orientation, s.position, s.start, s.length))
                .collect(),
            floating: dock
                .floating_windows()
                .iter()
                .map(|f| {
                    (
                        f.id,
                        f.rect(),
                        f.active_tab,
                        f.panels.iter().map(|p| p.type_id()).collect(),
                    )
                })
                .collect(),
            active_leaf: dock.tree().active_leaf_id(),
            drag: dock.panel_drag_state().map(|d| {
                (
                    d.dragged_leaf_id,
                    d.payload,
                    d.current_x,
                    d.current_y,
                    d.target_leaf_id,
                    d.drop_zone,
                    d.is_window_edge,
                )
            }),
            reorder: dock
                .tab_reorder_state()
                .map(|r| (r.container_id, r.original_index, r.insert_index)),
            floating_drag: dock.floating_drag_state().map(|f| f.window_id),
            snaps: splitter::snap_obs(dock),
        }
    }

    /// Leaves, their panels or floating windows came or went.
    pub(super) fn structure_differs(&self, other: &Self) -> bool {
        let shape = |o: &Self| {
            (
                o.leaves
                    .iter()
                    .map(|l| (l.0, l.4.clone()))
                    .collect::<Vec<_>>(),
                o.floating
                    .iter()
                    .map(|f| (f.0, f.3.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        shape(self) != shape(other)
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// `LayoutCmd::OpenPanel`.
pub(super) fn open_panel<P: DockPanel>(w: &mut WindowLayout<P>, panel: P, at: DockTarget) {
    let dock = &mut w.dock;
    match at {
        DockTarget::Root | DockTarget::Window(_) => {
            let first = dock.tree().leaves().first().map(|l| l.id);
            match first {
                Some(leaf) => dock.tree_mut().add_tab(leaf, panel),
                None => {
                    let leaf = dock.tree_mut().add_leaf(panel);
                    dock.set_active_leaf(leaf);
                }
            }
        }
        DockTarget::Leaf(leaf, zone) => {
            if dock.tree().leaf(leaf).is_none() {
                return;
            }
            match zone {
                DropZone::Center => dock.tree_mut().add_tab(leaf, panel),
                side => {
                    dock.tree_mut().split_leaf_zone(leaf, side, panel);
                }
            }
        }
        DockTarget::Floating(rect) => {
            let id = dock.next_floating_id();
            dock.insert_floating(float_window(id, vec![panel], 0, rect));
        }
    }
}

fn float_window<P: DockPanel>(
    id: FloatingWindowId,
    panels: Vec<P>,
    active: usize,
    r: Rect,
) -> FloatingWindow<P> {
    let r = rects::to_panel(r);
    FloatingWindow::new(id, panels, active, r.x, r.y, r.width, r.height)
}

/// Activate tab `tab` of `leaf` (and the leaf). `true` when the active tab
/// changed.
pub(super) fn activate_tab<P: DockPanel>(
    dock: &mut DockState<P>,
    leaf: LeafId,
    tab: usize,
) -> bool {
    let changed = match dock.tree_mut().leaf_mut(leaf) {
        Some(l) if tab < l.panels.len() => {
            let changed = l.active_tab != tab;
            l.active_tab = tab;
            changed
        }
        _ => return false,
    };
    dock.set_active_leaf(leaf);
    changed
}

/// Close tab `tab` of `leaf` if its panel is closable; its type id.
pub(super) fn close_tab<P: DockPanel>(
    dock: &mut DockState<P>,
    leaf: LeafId,
    tab: usize,
) -> Option<&'static str> {
    let type_id = dock
        .tree()
        .leaf(leaf)
        .and_then(|l| l.panels.get(tab))
        .filter(|p| p.closable())
        .map(|p| p.type_id())?;
    dock.tree_mut().remove_tab(leaf, tab);
    Some(type_id)
}

/// `LayoutCmd::Split`.
pub(super) fn split<P: DockPanel>(dock: &mut DockState<P>, leaf: LeafId, dir: SplitDir, panel: P) {
    let zone = match dir {
        SplitDir::Left => DropZone::Left,
        SplitDir::Right => DropZone::Right,
        SplitDir::Up => DropZone::Up,
        SplitDir::Down => DropZone::Down,
    };
    dock.tree_mut().split_leaf_zone(leaf, zone, panel);
}

/// `LayoutCmd::Float`: one tab leaves its leaf for an in-window floating
/// window. Refused when it is the last panel of the last visible leaf.
pub(super) fn float_tab<P: DockPanel>(
    w: &mut WindowLayout<P>,
    leaf: LeafId,
    index: usize,
    rect: Rect,
) {
    let dock = &mut w.dock;
    let Some(l) = dock.tree().leaf(leaf) else {
        return;
    };
    let Some(panel) = l.panels.get(index).cloned() else {
        return;
    };
    if l.panels.len() == 1 && dock.tree().visible_leaf_count() <= 1 {
        return;
    }
    dock.tree_mut().remove_tab(leaf, index);
    let id = dock.next_floating_id();
    dock.insert_floating(float_window(id, vec![panel], 0, rect));
}

/// `LayoutCmd::SetRatios`: one positive finite ratio per child, else
/// nothing changes.
pub(super) fn set_ratios<P: DockPanel>(
    dock: &mut DockState<P>,
    branch: BranchId,
    ratios: Vec<f64>,
) {
    let fits = dock
        .tree()
        .find_branch(branch)
        .is_some_and(|b| b.children.len() == ratios.len());
    if fits && !ratios.is_empty() && ratios.iter().all(|r| r.is_finite() && *r > 0.0) {
        dock.tree_mut().set_branch_proportions(branch, ratios);
    }
}

// ---------------------------------------------------------------------------
// Hit-testing
// ---------------------------------------------------------------------------

fn sorted_leaf_rects<P: DockPanel>(dock: &DockState<P>) -> Vec<(LeafId, PanelRect)> {
    let mut v: Vec<(LeafId, PanelRect)> =
        dock.panel_rects().iter().map(|(k, r)| (*k, *r)).collect();
    v.sort_by_key(|(k, _)| k.0);
    v
}

/// The "+" button rect of a leaf (right end of its tab bar / header).
pub(super) fn tab_new_rect<P: DockPanel>(
    dock: &DockState<P>,
    policy: &LayoutPolicy,
    leaf: LeafId,
) -> Option<Rect> {
    if !policy.tab_new_button {
        return None;
    }
    let r = dock.panel_rects().get(&leaf)?;
    let hh = dock.tab_bar_height();
    if r.width < 2.0 * hh || r.height < hh {
        return None;
    }
    Some(Rect::new(
        (r.x + r.width - hh) as f64,
        r.y as f64,
        hh as f64,
        hh as f64,
    ))
}

/// Floating windows, topmost first.
pub(super) fn floating_hit<P: DockPanel>(
    dock: &DockState<P>,
    policy: &LayoutPolicy,
    p: Point,
) -> Option<LayoutHit> {
    let hh = dock.tab_bar_height() as f64;
    for fw in dock.floating_windows().iter().rev() {
        let r = rects::from_panel(fw.rect());
        if !rects::contains(r, p) {
            continue;
        }
        let band = policy.bezel_px.min(r.width / 4.0).min(r.height / 4.0);
        if let Some(dir) = chrome::bezel(r, band, p) {
            return Some(LayoutHit::FloatingResize { id: fw.id, dir });
        }
        let close = Rect::new(
            r.x + r.width - FLOAT_CLOSE - 4.0,
            r.y + 2.0,
            FLOAT_CLOSE,
            FLOAT_CLOSE,
        );
        if rects::contains(close, p) {
            return Some(LayoutHit::FloatingClose(fw.id));
        }
        if p.y < r.y + hh {
            return Some(LayoutHit::FloatingHeader(fw.id));
        }
        return Some(LayoutHit::FloatingBody(fw.id));
    }
    None
}

/// The docked part under `p` (module docs, step 6).
pub(super) fn dock_hit<P: DockPanel>(
    dock: &DockState<P>,
    policy: &LayoutPolicy,
    p: Point,
) -> LayoutHit {
    let (x, y) = (p.x as f32, p.y as f32);
    let seps = dock.separators();

    // Crossings: only where both lines really pass through the point
    // (the library pairs every vertical with every horizontal line).
    for c in dock.corners() {
        let (Some(v), Some(h)) = (seps.get(c.v_separator_idx), seps.get(c.h_separator_idx)) else {
            continue;
        };
        let on_h = c.x >= h.start && c.x <= h.start + h.length;
        let on_v = c.y >= v.start && c.y <= v.start + v.length;
        if on_h && on_v && c.hit_test(x, y, CORNER_RADIUS) {
            return LayoutHit::Corner {
                vertical: c.v_separator_idx,
                horizontal: c.h_separator_idx,
            };
        }
    }
    for (sep, s) in seps.iter().enumerate() {
        if s.hit_test(x, y) {
            return LayoutHit::Splitter {
                sep,
                orientation: s.orientation,
            };
        }
    }

    let leaves = sorted_leaf_rects(dock);
    let hh = dock.tab_bar_height();

    // Multi-tab bars: the leaf's top strip.
    let mut bars: Vec<_> = dock.tab_bars().iter().collect();
    bars.sort_by_key(|b| b.container_id.0);
    for bar in bars {
        let leaf = bar.container_id;
        let Some((_, r)) = leaves.iter().find(|(id, _)| *id == leaf) else {
            continue;
        };
        let strip = PanelRect::new(r.x, r.y, r.width, hh);
        if !rects::contains_panel(&strip, p) {
            continue;
        }
        if tab_new_rect(dock, policy, leaf).is_some_and(|n| rects::contains(n, p)) {
            return LayoutHit::TabNew { leaf };
        }
        let panels = dock
            .tree()
            .leaf(leaf)
            .map(|l| l.panels.as_slice())
            .unwrap_or(&[]);
        for (tab, item) in bar.tabs.iter().enumerate() {
            let closable = panels.get(tab).is_some_and(|p| p.closable());
            if closable && rects::contains_panel(&item.close_rect, p) {
                return LayoutHit::TabClose { leaf, tab };
            }
            if rects::contains_panel(&item.rect, p) {
                return LayoutHit::Tab { leaf, tab };
            }
        }
    }

    // Single-tab headers.
    let mut headers: Vec<(LeafId, PanelRect)> =
        dock.panel_headers().iter().map(|(k, v)| (*k, *v)).collect();
    headers.sort_by_key(|(k, _)| k.0);
    for (leaf, h) in headers {
        if rects::contains_panel(&h, p) {
            if tab_new_rect(dock, policy, leaf).is_some_and(|n| rects::contains(n, p)) {
                return LayoutHit::TabNew { leaf };
            }
            return LayoutHit::PanelHeader(leaf);
        }
    }

    for (leaf, r) in leaves {
        if rects::contains_panel(&r, p) {
            return LayoutHit::PanelBody(leaf);
        }
    }
    LayoutHit::None
}

// ---------------------------------------------------------------------------
// Tab / header sessions
// ---------------------------------------------------------------------------

fn moved_past(origin: Point, p: Point) -> bool {
    let (dx, dy) = (p.x - origin.x, p.y - origin.y);
    dx * dx + dy * dy >= DRAG_START_PX * DRAG_START_PX
}

/// The top strip (tab bar / header band) of a leaf.
fn strip<P: DockPanel>(dock: &DockState<P>, leaf: LeafId) -> Option<PanelRect> {
    let r = dock.panel_rects().get(&leaf)?;
    Some(PanelRect::new(r.x, r.y, r.width, dock.tab_bar_height()))
}

/// Start the library tab drag (tear-off); refused for locked panels.
fn tear<P: DockPanel>(dock: &mut DockState<P>, leaf: LeafId, tab: usize, p: Point) -> TabStage {
    dock.compute_window_edge_rects();
    dock.start_tab_drag(leaf, tab, p.x as f32, p.y as f32);
    if dock.panel_drag_state().is_some() {
        dock.update_panel_drag(p.x as f32, p.y as f32);
        TabStage::Tear
    } else {
        TabStage::Refused
    }
}

/// Undo a live reorder: park the pointer on the chip's own centre (insert
/// index = original) and end it.
fn reorder_cancel<P: DockPanel>(dock: &mut DockState<P>, leaf: LeafId, tab: usize) {
    let centre = dock
        .tab_bars()
        .iter()
        .find(|b| b.container_id == leaf)
        .and_then(|b| b.tabs.get(tab))
        .map(|t| t.rect.x + t.rect.width / 2.0);
    if let Some(x) = centre {
        dock.update_tab_reorder(x);
    }
    dock.end_tab_reorder();
}

/// A pointer move of a tab or header session.
pub(super) fn drag_move<P: DockPanel>(
    w: &mut WindowLayout<P>,
    session: Session,
    p: Point,
) -> Session {
    let dock = &mut w.dock;
    let (x, y) = (p.x as f32, p.y as f32);
    match session {
        Session::Tab {
            leaf,
            tab,
            origin,
            stage,
        } => {
            let in_bar = strip(dock, leaf).is_some_and(|s| rects::contains_panel(&s, p));
            let stage = match stage {
                TabStage::Pressed if !moved_past(origin, p) => TabStage::Pressed,
                TabStage::Pressed if in_bar => {
                    let chip = dock
                        .tab_bars()
                        .iter()
                        .find(|b| b.container_id == leaf)
                        .and_then(|b| b.tabs.get(tab))
                        .map(|t| t.panel_id);
                    match chip {
                        Some(id) => {
                            dock.start_tab_reorder(leaf, id, x);
                            dock.update_tab_reorder(x);
                            TabStage::Reorder
                        }
                        None => TabStage::Refused,
                    }
                }
                TabStage::Pressed => tear(dock, leaf, tab, p),
                TabStage::Reorder if in_bar => {
                    dock.update_tab_reorder(x);
                    TabStage::Reorder
                }
                TabStage::Reorder => {
                    reorder_cancel(dock, leaf, tab);
                    tear(dock, leaf, tab, p)
                }
                TabStage::Tear => {
                    dock.update_panel_drag(x, y);
                    TabStage::Tear
                }
                TabStage::Refused => TabStage::Refused,
            };
            Session::Tab {
                leaf,
                tab,
                origin,
                stage,
            }
        }
        Session::Header {
            leaf,
            origin,
            stage,
        } => {
            let stage = match stage {
                HeaderStage::Pressed if !moved_past(origin, p) => HeaderStage::Pressed,
                HeaderStage::Pressed => {
                    dock.compute_window_edge_rects();
                    dock.start_panel_drag(leaf, x, y);
                    if dock.panel_drag_state().is_some() {
                        dock.update_panel_drag(x, y);
                        HeaderStage::Dragging
                    } else {
                        HeaderStage::Refused
                    }
                }
                HeaderStage::Dragging => {
                    dock.update_panel_drag(x, y);
                    HeaderStage::Dragging
                }
                HeaderStage::Refused => HeaderStage::Refused,
            };
            Session::Header {
                leaf,
                origin,
                stage,
            }
        }
        other => other,
    }
}

/// A tab or header session whose library panel drag is live (the only
/// sessions expand gutters and drag-out apply to).
pub(super) fn is_live_drag(session: &Session) -> bool {
    matches!(
        session,
        Session::Tab {
            stage: TabStage::Tear,
            ..
        } | Session::Header {
            stage: HeaderStage::Dragging,
            ..
        }
    )
}

/// Take the payload of the live library drag out of the dock: the torn tab,
/// or every panel of the dragged leaf. Ends the library drag. `None` (and
/// nothing changes) when there is no live drag, or when taking a leaf would
/// leave the dock without a visible leaf.
pub(super) fn take_payload<P: DockPanel>(dock: &mut DockState<P>) -> Option<(LeafId, Payload<P>)> {
    let state = dock.panel_drag_state().cloned()?;
    let leaf = state.dragged_leaf_id;
    let size = dock
        .panel_rects()
        .get(&leaf)
        .map(|r| (r.width as f64, r.height as f64))
        .unwrap_or((0.0, 0.0));
    let l = dock.tree().leaf(leaf)?;
    let payload = match state.payload {
        DragPayload::Tab { tab_idx } => {
            let panel = l.panels.get(tab_idx)?.clone();
            dock.cancel_panel_drag();
            dock.tree_mut().remove_tab(leaf, tab_idx);
            Payload::new(vec![panel], 0, size)
        }
        DragPayload::Leaf => {
            if dock.tree().visible_leaf_count() <= 1 || l.panels.is_empty() {
                return None;
            }
            let (panels, active) = (l.panels.clone(), l.active_tab);
            dock.cancel_panel_drag();
            dock.tree_mut().remove_leaf(leaf);
            Payload::new(panels, active, size)
        }
    };
    Some((leaf, payload))
}

/// Put `payload` into `leaf` as tabs (Center), or, without a leaf, at the
/// dock root (first leaf, or a new one in an empty dock). Returns the leaf
/// that holds it and activates the payload's active panel there.
pub(super) fn dock_payload<P: DockPanel>(
    dock: &mut DockState<P>,
    leaf: Option<LeafId>,
    payload: Payload<P>,
) -> Option<LeafId> {
    let (panels, active) = payload.into_parts();
    let target = leaf
        .filter(|l| dock.tree().leaf(*l).is_some())
        .or_else(|| dock.tree().leaves().first().map(|l| l.id));
    let leaf = match target {
        Some(leaf) => {
            let base = dock.tree().leaf(leaf).map(|l| l.panels.len()).unwrap_or(0);
            for p in panels {
                dock.tree_mut().add_tab(leaf, p);
            }
            if let Some(l) = dock.tree_mut().leaf_mut(leaf) {
                l.active_tab = (base + active).min(l.panels.len().saturating_sub(1));
            }
            leaf
        }
        None => {
            if panels.is_empty() {
                return None;
            }
            let leaf = dock.tree_mut().add_leaf_with_panels(panels, active);
            leaf
        }
    };
    dock.set_active_leaf(leaf);
    Some(leaf)
}

/// The leaf whose rect contains `p` (leaves in `LeafId` order).
pub(super) fn leaf_at<P: DockPanel>(dock: &DockState<P>, p: Point) -> Option<LeafId> {
    sorted_leaf_rects(dock)
        .into_iter()
        .find(|(_, r)| rects::contains_panel(r, p))
        .map(|(id, _)| id)
}

/// Take every docked panel out of `dock` (leaves in `LeafId` order; the
/// first leaf's active tab stays active). The first leaf's id, or `None`
/// for an empty dock.
pub(super) fn take_all<P: DockPanel>(dock: &mut DockState<P>) -> Option<(LeafId, Payload<P>)> {
    let mut leaves: Vec<(LeafId, Vec<P>, usize)> = dock
        .tree()
        .leaves()
        .iter()
        .map(|l| (l.id, l.panels.clone(), l.active_tab))
        .collect();
    leaves.sort_by_key(|l| l.0 .0);
    let (first, active) = leaves.first().map(|l| (l.0, l.2))?;
    let size = dock
        .panel_rects()
        .get(&first)
        .map(|r| (r.width as f64, r.height as f64))
        .unwrap_or((0.0, 0.0));
    let mut panels = Vec::new();
    for (id, ps, _) in leaves {
        dock.tree_mut().remove_leaf(id);
        panels.extend(ps);
    }
    if panels.is_empty() {
        return None;
    }
    Some((first, Payload::new(panels, active, size)))
}

/// The release of a tab or header session.
pub(super) fn drag_release<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    session: Session,
    pos: Point,
    ctx: Ctx<P>,
    fx: &mut LayoutEffects,
    out: &mut Option<Outbound<P>>,
) {
    match session {
        Session::Tab {
            stage: TabStage::Reorder,
            ..
        } => w.dock.end_tab_reorder(),
        Session::Tab {
            leaf,
            tab,
            stage: TabStage::Tear,
            ..
        } => finish_drag(win, w, leaf, tab, pos, ctx, fx, out),
        Session::Header {
            leaf,
            stage: HeaderStage::Dragging,
            ..
        } => {
            let index = w.dock.tree().leaf(leaf).map(|l| l.active_tab).unwrap_or(0);
            finish_drag(win, w, leaf, index, pos, ctx, fx, out);
        }
        _ => {}
    }
}

/// The cancel of a tab or header session: nothing moves.
pub(super) fn drag_cancel<P: DockPanel>(w: &mut WindowLayout<P>, session: Session) {
    match session {
        Session::Tab {
            leaf,
            tab,
            stage: TabStage::Reorder,
            ..
        } => reorder_cancel(&mut w.dock, leaf, tab),
        Session::Tab {
            stage: TabStage::Tear,
            ..
        }
        | Session::Header {
            stage: HeaderStage::Dragging,
            ..
        } => w.dock.cancel_panel_drag(),
        _ => {}
    }
}

/// Drop the live library drag. A release outside the viewport of a window
/// that allows drag-out ([`dragout::allowed`]) takes the payload out of the
/// dock into `out` (the engine docks it into the sibling window under the
/// cursor or spawns a micro-window). Otherwise: a target → the library
/// restructures the tree and `PanelMoved` names the leaf that holds the
/// panel now; no target → a leaf floats in-window (`PanelTornOff`), a torn
/// tab stays in its stack.
#[allow(clippy::too_many_arguments)]
fn finish_drag<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    src: LeafId,
    index: usize,
    pos: Point,
    ctx: Ctx<P>,
    fx: &mut LayoutEffects,
    out: &mut Option<Outbound<P>>,
) {
    let Some(state) = w.dock.panel_drag_state().cloned() else {
        return;
    };
    if !rects::contains(w.viewport, pos) && dragout::allowed(w, &ctx) {
        if let Some((leaf, payload)) = take_payload(&mut w.dock) {
            *out = Some(Outbound::new(leaf, payload, pos));
            return;
        }
    }
    let before: BTreeSet<u64> = w.dock.tree().leaves().iter().map(|l| l.id.0).collect();
    let wire_before = blob::WireDock::capture(&w.dock);
    let area = w.dock.layout_area();
    let floated = w.dock.end_panel_drag(area.width, area.height);
    match (state.target_leaf_id, state.drop_zone) {
        (Some(target), Some(zone)) => {
            if blob::WireDock::capture(&w.dock) == wire_before {
                return; // e.g. a tab dropped back into its own stack
            }
            let fresh: Vec<u64> = w
                .dock
                .tree()
                .leaves()
                .iter()
                .map(|l| l.id.0)
                .filter(|id| !before.contains(id))
                .collect();
            let to = match (fresh.as_slice(), zone) {
                ([one], _) => LeafId(*one),
                (_, DropZone::Center) => target,
                _ => src,
            };
            fx.push(LayoutEffect::Intent(DockIntent::PanelMoved {
                from: (win, src),
                to: (win, to),
                zone,
            }));
        }
        _ => {
            if let Some(id) = floated {
                clamp_floating(w, id);
                fx.push(LayoutEffect::Intent(DockIntent::PanelTornOff {
                    win,
                    leaf: src,
                    index,
                }));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Floating sessions
// ---------------------------------------------------------------------------

/// Keep a floating window inside the dock area.
fn clamp_floating<P: DockPanel>(w: &mut WindowLayout<P>, id: FloatingWindowId) {
    let Some(fw) = w.dock.floating_windows().iter().find(|f| f.id == id) else {
        return;
    };
    let r = rects::from_panel(fw.rect());
    let a = w.solved.dock_area;
    let x = r.x.clamp(a.x, (a.x + a.width - r.width).max(a.x));
    let y = r.y.clamp(a.y, (a.y + a.height - r.height).max(a.y));
    w.dock
        .set_floating_rect(id, rects::to_panel(Rect::new(x, y, r.width, r.height)));
}

/// Press on a floating header: start moving it.
pub(super) fn floating_press<P: DockPanel>(
    dock: &mut DockState<P>,
    id: FloatingWindowId,
    p: Point,
) -> Option<Session> {
    let fw = dock.floating_windows().iter().find(|f| f.id == id)?;
    let grab = (p.x - fw.x as f64, p.y - fw.y as f64);
    dock.start_floating_drag(id, p.x as f32, p.y as f32);
    Some(Session::FloatingMove { id, grab })
}

/// Press on a floating resize border.
pub(super) fn floating_resize_press<P: DockPanel>(
    dock: &DockState<P>,
    id: FloatingWindowId,
    dir: ResizeDirection,
    p: Point,
) -> Option<Session> {
    let fw = dock.floating_windows().iter().find(|f| f.id == id)?;
    Some(Session::FloatingResize {
        id,
        dir,
        origin: p,
        start: rects::from_panel(fw.rect()),
    })
}

/// A pointer move of a floating session.
pub(super) fn floating_move<P: DockPanel>(w: &mut WindowLayout<P>, session: Session, p: Point) {
    match session {
        Session::FloatingMove { id, grab } => {
            let Some(fw) = w.dock.floating_windows().iter().find(|f| f.id == id) else {
                return;
            };
            let r = rects::from_panel(fw.rect());
            let moved = Rect::new(p.x - grab.0, p.y - grab.1, r.width, r.height);
            w.dock.set_floating_rect(id, rects::to_panel(moved));
            clamp_floating(w, id);
        }
        Session::FloatingResize {
            id,
            dir,
            origin,
            start,
        } => {
            let (dx, dy) = (p.x - origin.x, p.y - origin.y);
            let (west, east, north, south) = match dir {
                ResizeDirection::West => (true, false, false, false),
                ResizeDirection::East => (false, true, false, false),
                ResizeDirection::North => (false, false, true, false),
                ResizeDirection::South => (false, false, false, true),
                ResizeDirection::NorthWest => (true, false, true, false),
                ResizeDirection::NorthEast => (false, true, true, false),
                ResizeDirection::SouthWest => (true, false, false, true),
                ResizeDirection::SouthEast => (false, true, false, true),
            };
            let (mut x, mut wd) = (start.x, start.width);
            if east {
                wd = (start.width + dx).max(FLOAT_MIN.0);
            }
            if west {
                wd = (start.width - dx).max(FLOAT_MIN.0);
                x = start.x + start.width - wd;
            }
            let (mut y, mut ht) = (start.y, start.height);
            if south {
                ht = (start.height + dy).max(FLOAT_MIN.1);
            }
            if north {
                ht = (start.height - dy).max(FLOAT_MIN.1);
                y = start.y + start.height - ht;
            }
            w.dock
                .set_floating_rect(id, rects::to_panel(Rect::new(x, y, wd, ht)));
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

fn panel_refs<P: DockPanel>(panels: &[P]) -> Vec<PanelRef> {
    panels
        .iter()
        .enumerate()
        .map(|(index, p)| PanelRef {
            index,
            type_id: p.type_id(),
            title: p.title().to_string(),
        })
        .collect()
}

/// The published dock view: leaves in `LeafId` order, separators in index
/// order (2 px line rects), floating windows bottom to top.
pub(super) fn dock_view<P: DockPanel>(dock: &DockState<P>, dock_rev: Revision) -> DockView {
    let mut leaves: Vec<LeafView> = dock
        .tree()
        .leaves()
        .iter()
        .map(|l| LeafView {
            leaf: l.id,
            rect: dock
                .panel_rects()
                .get(&l.id)
                .map(|r| rects::from_panel(*r))
                .unwrap_or_default(),
            panels: panel_refs(&l.panels),
            active_tab: l.active_tab,
            hidden: l.hidden,
        })
        .collect();
    leaves.sort_by_key(|l| l.leaf.0);
    let separators = dock
        .separators()
        .iter()
        .enumerate()
        .map(|(index, s)| {
            let (pos, start, len) = (s.position as f64, s.start as f64, s.length as f64);
            let rect = match s.orientation {
                SeparatorOrientation::Vertical => Rect::new(pos - 1.0, start, 2.0, len),
                SeparatorOrientation::Horizontal => Rect::new(start, pos - 1.0, len, 2.0),
            };
            SeparatorView {
                index,
                rect,
                orientation: s.orientation,
            }
        })
        .collect();
    let floating = dock
        .floating_windows()
        .iter()
        .map(|f| FloatingView {
            id: f.id,
            rect: rects::from_panel(f.rect()),
            panels: panel_refs(&f.panels),
            active_tab: f.active_tab,
        })
        .collect();
    DockView {
        dock_rev,
        leaves,
        separators,
        floating,
        active_leaf: dock.tree().active_leaf_id(),
    }
}
