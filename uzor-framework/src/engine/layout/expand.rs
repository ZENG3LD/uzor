//! Inner / outer window expand (design §3.2.3).
//!
//! While a panel is dragged (a torn tab or a leaf drag), the viewport's edge
//! bands are expand gutters ([`LayoutHit::EdgeGutter`]). Entering one
//! latches an [`ExpandTarget`] — the side, the window's outer rect at that
//! moment (`origin`, physical screen px) and the full extent
//! (`thickness_px`, physical) — and emits
//! [`LayoutEffect::ExpandTarget`] `{ side: Some(side) }`; the kernel points
//! the AnimationEngine's `AnimKey::Expand { win, kind }` at `1`. Leaving the
//! gutter (or ending the drag anywhere else) emits `side: None` (animate
//! back to `0`). Every animated value comes back as
//! [`LayoutOp::ExpandValue`], and the engine turns it into one
//! `WindowCommand::SetOuterRect` (plus `RequestRedraw`) whenever the rect
//! for that `t` differs from the one last sent:
//!
//! | kind / side | outer rect at `t` (`e = round(thickness_px * t)`) | content inset |
//! |---|---|---|
//! | any, Right | `origin` + `e` width | none |
//! | any, Bottom | `origin` + `e` height | none |
//! | Outer, Left | x `- e`, width `+ e` (right edge fixed) | `e` on x |
//! | Outer, Top | y `- e`, height `+ e` (bottom edge fixed) | `e` on y |
//! | Inner, Left | width `+ e`, position fixed | `e` on x |
//! | Inner, Top | height `+ e`, position fixed | `e` on y |
//!
//! Every rect is computed from the latched `origin`, never from the current
//! window position, so a Left / Top outer expand keeps its far edge exactly
//! fixed at every intermediate `t`, and a released expand that reaches
//! `t = 0` sends exactly `origin` back and clears the target
//! ([`LayoutEffect::ExpandDone`]). While a target is latched the content is
//! laid out in the viewport of latch time shifted by the inset (the
//! "pinned" viewport), so the grown strip stays empty for the drop preview.
//! A release inside the latched gutter commits: the window gets the full
//! extent, the dragged panel becomes a new root-level leaf on that side
//! sized to the strip (`PanelMoved`), and the target clears.
//!
//! Right / Bottom gutters latch [`ExpandKind::Outer`]; Left / Top latch
//! `LayoutPolicy::left_top_expand` (`Inner` by default, as before). No
//! gutter exists — so nothing ever latches — without `HostCaps::multi_window`
//! (web), without a known outer rect, or in a drag-out micro-window.
//!
//! Ported from the previous kernel: gutter bands and latch order from
//! `engine/dock/drop.rs:776-883` (`resolve_and_store_drop_hover`), the
//! per-value apply from `engine/dock/anim.rs:90-253` (`tick_edge_preview`)
//! and `:259-399` (`tick_inner_expand`), the gutter drops from
//! `engine/dock/drop.rs:557-715`. Rewritten: every string-keyed blob read /
//! write (`get_ui_state(nid, "expand", ..)`, anim.rs:121-131) is a typed
//! field; the animator itself lives in the AnimationEngine; the outer rect is
//! computed from the latched origin instead of `current position + offset`
//! (anim.rs:237-242 added the offset to the live position every frame); a
//! second side cannot latch while a retract is still running (the previous
//! kernel re-pinned the grown viewport on a side switch, losing the original
//! rect).
//!
//! [`LayoutHit::EdgeGutter`]: crate::types::command::LayoutHit::EdgeGutter
//! [`LayoutEffect::ExpandTarget`]: crate::types::ops::LayoutEffect::ExpandTarget
//! [`LayoutEffect::ExpandDone`]: crate::types::ops::LayoutEffect::ExpandDone
//! [`LayoutOp::ExpandValue`]: crate::types::ops::LayoutOp::ExpandValue

use uzor::layout::docking::{DockPanel, DropZone};
use uzor::layout::EdgeSide;
use uzor::Rect;

use crate::types::anim::ExpandKind;
use crate::types::command::LayoutPolicy;
use crate::types::ids::WindowId;
use crate::types::intent::DockIntent;
use crate::types::ops::LayoutEffect;
use crate::types::window::{Point, SizePx, WindowCommand};

use super::{dock, rects, LayoutEffects, Session, WindowLayout};

/// What an expand latched: the side, the outer rect it grows from and how
/// far it grows at `t = 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpandTarget {
    side: EdgeSide,
    origin: Rect,
    thickness_px: f64,
}

impl ExpandTarget {
    /// The gutter side.
    pub fn side(&self) -> EdgeSide {
        self.side
    }

    /// The window's outer rect when the expand latched, physical screen px.
    pub fn origin(&self) -> Rect {
        self.origin
    }

    /// The full extent at `t = 1`, physical px.
    pub fn thickness_px(&self) -> f64 {
        self.thickness_px
    }

    /// The extent at `t` (clamped to `0..=1`), whole physical px.
    pub fn extent(&self, t: f64) -> f64 {
        (self.thickness_px * clamp01(t)).round()
    }

    /// The outer rect a `kind` expand has at `t` (module table).
    pub fn outer_at(&self, kind: ExpandKind, t: f64) -> Rect {
        let e = self.extent(t);
        let o = self.origin;
        match (kind, self.side) {
            (_, EdgeSide::Right) => Rect::new(o.x, o.y, o.width + e, o.height),
            (_, EdgeSide::Bottom) => Rect::new(o.x, o.y, o.width, o.height + e),
            (ExpandKind::Outer, EdgeSide::Left) => Rect::new(o.x - e, o.y, o.width + e, o.height),
            (ExpandKind::Outer, EdgeSide::Top) => Rect::new(o.x, o.y - e, o.width, o.height + e),
            (ExpandKind::Inner, EdgeSide::Left) => Rect::new(o.x, o.y, o.width + e, o.height),
            (ExpandKind::Inner, EdgeSide::Top) => Rect::new(o.x, o.y, o.width, o.height + e),
        }
    }
}

/// Published state of a latched expand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpandState {
    kind: ExpandKind,
    target: ExpandTarget,
    t: f64,
    hot: bool,
    inset: Point,
}

impl ExpandState {
    /// Outer or inner.
    pub fn kind(&self) -> ExpandKind {
        self.kind
    }

    /// What was latched.
    pub fn target(&self) -> ExpandTarget {
        self.target
    }

    /// The last animated value applied.
    pub fn t(&self) -> f64 {
        self.t
    }

    /// The pointer is in the gutter (the animator runs toward `1`).
    pub fn hot(&self) -> bool {
        self.hot
    }

    /// How far the content is shifted, logical px.
    pub fn inset(&self) -> Point {
        self.inset
    }
}

/// A latched expand of one window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Expand {
    kind: ExpandKind,
    target: ExpandTarget,
    /// The logical viewport at latch time.
    pinned: Rect,
    /// Device pixel ratio at latch time.
    scale: f64,
    t: f64,
    hot: bool,
    /// The outer rect last sent to the host.
    applied: Rect,
}

impl Expand {
    pub(super) fn state(&self) -> ExpandState {
        ExpandState {
            kind: self.kind,
            target: self.target,
            t: self.t,
            hot: self.hot,
            inset: self.inset(),
        }
    }

    /// Content shift, logical px (anim.rs:222-226).
    fn inset(&self) -> Point {
        let e = self.target.extent(self.t) / self.scale;
        match self.target.side {
            EdgeSide::Left => Point::new(e, 0.0),
            EdgeSide::Top => Point::new(0.0, e),
            EdgeSide::Right | EdgeSide::Bottom => Point::new(0.0, 0.0),
        }
    }
}

fn clamp01(t: f64) -> f64 {
    if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// The rect the window's content is solved in (module docs).
pub(super) fn content_rect<P: DockPanel>(w: &WindowLayout<P>) -> Rect {
    match &w.expand {
        Some(x) => {
            let i = x.inset();
            Rect::new(
                x.pinned.x + i.x,
                x.pinned.y + i.y,
                x.pinned.width,
                x.pinned.height,
            )
        }
        None => w.viewport,
    }
}

/// The host can resize this window for an expand.
fn allowed<P: DockPanel>(w: &WindowLayout<P>) -> bool {
    w.caps.multi_window && w.micro.is_none() && w.outer.is_some()
}

/// The window's session is a live panel drag.
pub(super) fn drag_live<P: DockPanel>(w: &WindowLayout<P>) -> bool {
    w.session.as_ref().is_some_and(dock::is_live_drag)
}

/// Which expand a gutter side latches.
fn kind_for(side: EdgeSide, policy: &LayoutPolicy) -> ExpandKind {
    match side {
        EdgeSide::Left | EdgeSide::Top => policy.left_top_expand,
        EdgeSide::Right | EdgeSide::Bottom => ExpandKind::Outer,
    }
}

/// The gutter under `p` while a panel drag is `live` (drop.rs:776-829).
/// A latched side keeps the whole grown strip beyond the content; otherwise
/// the bands are tested Top, Bottom, Left, Right. Left / Top bands are at
/// least as wide as the chrome and edge slots on that side, Right / Bottom
/// bands are `edge_expand_px`.
pub(super) fn gutter<P: DockPanel>(
    w: &WindowLayout<P>,
    policy: &LayoutPolicy,
    p: Point,
    live: bool,
) -> Option<EdgeSide> {
    if !live || !allowed(w) || !rects::contains(w.viewport, p) {
        return None;
    }
    let c = content_rect(w);
    if let Some(x) = &w.expand {
        let in_strip = match x.target.side {
            EdgeSide::Right => p.x >= c.x + c.width,
            EdgeSide::Left => p.x < c.x,
            EdgeSide::Bottom => p.y >= c.y + c.height,
            EdgeSide::Top => p.y < c.y,
        };
        if in_strip {
            return Some(x.target.side);
        }
    }
    let g = policy.edge_expand_px;
    if !g.is_finite() || g <= 0.0 {
        return None;
    }
    let d = w.solved.dock_area;
    let left_band = (d.x - c.x).max(g);
    let top_band = (d.y - c.y).max(g);
    if p.y < c.y + top_band {
        Some(EdgeSide::Top)
    } else if (c.y + c.height) - p.y < g {
        Some(EdgeSide::Bottom)
    } else if p.x < c.x + left_band {
        Some(EdgeSide::Left)
    } else if (c.x + c.width) - p.x < g {
        Some(EdgeSide::Right)
    } else {
        None
    }
}

/// A move (or the release point) of a live panel drag: latch, re-heat or
/// cool the window's expand (anim.rs:135-154, "latch follows hover").
pub(super) fn track<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    policy: &LayoutPolicy,
    p: Point,
    fx: &mut LayoutEffects,
) {
    let side = gutter(w, policy, p, true);
    match (&mut w.expand, side) {
        (None, Some(side)) => {
            let Some(origin) = w.outer else {
                return;
            };
            let thickness_px = (policy.expand_thickness_px * w.scale).round();
            if !thickness_px.is_finite() || thickness_px <= 0.0 {
                return;
            }
            let kind = kind_for(side, policy);
            w.expand = Some(Expand {
                kind,
                target: ExpandTarget {
                    side,
                    origin,
                    thickness_px,
                },
                pinned: w.viewport,
                scale: w.scale,
                t: 0.0,
                hot: true,
                applied: origin,
            });
            fx.push(LayoutEffect::ExpandTarget {
                win,
                kind,
                side: Some(side),
            });
        }
        (Some(x), side) => {
            // Only the latched side re-heats; another side waits until the
            // retract is over.
            let want = side == Some(x.target.side);
            if want != x.hot {
                x.hot = want;
                fx.push(LayoutEffect::ExpandTarget {
                    win,
                    kind: x.kind,
                    side: side.filter(|_| want),
                });
            }
        }
        (None, None) => {}
    }
}

/// The drag ended without a gutter drop: retract.
pub(super) fn unhot<P: DockPanel>(win: WindowId, w: &mut WindowLayout<P>, fx: &mut LayoutEffects) {
    if let Some(x) = &mut w.expand {
        if x.hot {
            x.hot = false;
            fx.push(LayoutEffect::ExpandTarget {
                win,
                kind: x.kind,
                side: None,
            });
        }
    }
}

fn set_outer(win: WindowId, r: Rect, fx: &mut LayoutEffects) {
    let px = |v: f64| v.round().max(1.0) as u32;
    fx.push(LayoutEffect::Window {
        win,
        cmd: WindowCommand::SetOuterRect {
            position: (r.x.round() as i32, r.y.round() as i32),
            size: SizePx::new(px(r.width), px(r.height)),
        },
    });
    fx.push(LayoutEffect::Window {
        win,
        cmd: WindowCommand::RequestRedraw,
    });
}

/// `LayoutOp::ExpandValue` (anim.rs:172-251).
pub(super) fn value<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    kind: ExpandKind,
    t: f64,
    fx: &mut LayoutEffects,
) {
    let Some(x) = &mut w.expand else {
        return;
    };
    if x.kind != kind || !t.is_finite() {
        return;
    }
    x.t = clamp01(t);
    if !x.hot && x.t <= 0.0 {
        // Fully retracted: the exact original rect, then forget the target
        // (anim.rs:172-197).
        let (origin, applied, pinned) = (x.target.origin, x.applied, x.pinned);
        w.expand = None;
        w.viewport = pinned;
        if applied != origin {
            set_outer(win, origin, fx);
        }
        fx.push(LayoutEffect::ExpandDone { win, kind });
        return;
    }
    let r = x.target.outer_at(kind, x.t);
    if r != x.applied {
        x.applied = r;
        set_outer(win, r, fx);
    }
}

/// A release inside the latched gutter: grow to the full extent and dock the
/// dragged panel into the strip as a new root-level leaf
/// (drop.rs:557-715). `false` (nothing changed) when the gutter is not hot,
/// the session is not a live drag, or the payload cannot leave its leaf.
pub(super) fn commit_drop<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    session: Session,
    fx: &mut LayoutEffects,
) -> bool {
    let Some(x) = w.expand else {
        return false;
    };
    if !x.hot || !dock::is_live_drag(&session) {
        return false;
    }
    let Some((src, payload)) = dock::take_payload(&mut w.dock) else {
        return false;
    };
    w.expand = None;
    set_outer(win, x.target.outer_at(x.kind, 1.0), fx);
    fx.push(LayoutEffect::ExpandDone { win, kind: x.kind });

    let side = x.target.side;
    let (zone, horizontal, new_first) = match side {
        EdgeSide::Left => (DropZone::Left, true, true),
        EdgeSide::Right => (DropZone::Right, true, false),
        EdgeSide::Top => (DropZone::Up, false, true),
        EdgeSide::Bottom => (DropZone::Down, false, false),
    };
    let strip = x.target.thickness_px / x.scale;
    let old = if horizontal {
        x.pinned.width
    } else {
        x.pinned.height
    };
    // The viewport the host is about to echo.
    w.viewport = if horizontal {
        Rect::new(x.pinned.x, x.pinned.y, old + strip, x.pinned.height)
    } else {
        Rect::new(x.pinned.x, x.pinned.y, x.pinned.width, old + strip)
    };

    let (panels, active) = payload.into_parts();
    let mut panels = panels.into_iter();
    let Some(first) = panels.next() else {
        return true;
    };
    let had_leaves = w.dock.tree().leaf_count() > 0;
    let Some(leaf) = w.dock.tree_mut().add_leaf_root_split(first, zone) else {
        return true;
    };
    for p in panels {
        w.dock.tree_mut().add_tab(leaf, p);
    }
    if let Some(l) = w.dock.tree_mut().leaf_mut(leaf) {
        l.active_tab = active.min(l.panels.len().saturating_sub(1));
    }
    if had_leaves {
        // The new leaf takes exactly the grown strip; the old layout keeps
        // its size (drop.rs:598-603, :669-677).
        let total = (old + strip).max(1.0);
        let (r_new, r_old) = (strip / total, old / total);
        let props = if new_first {
            vec![r_new, r_old]
        } else {
            vec![r_old, r_new]
        };
        let root = w.dock.tree().root().id;
        w.dock.tree_mut().set_branch_proportions(root, props);
    }
    w.dock.set_active_leaf(leaf);
    fx.push(LayoutEffect::Intent(DockIntent::PanelMoved {
        from: (win, src),
        to: (win, leaf),
        zone,
    }));
    true
}
