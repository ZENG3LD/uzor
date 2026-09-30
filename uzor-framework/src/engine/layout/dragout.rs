//! Drag-out: a panel leaves its window for a micro-window and docks back
//! into a sibling by dwelling over it (design §3.2.4).
//!
//! ## Start
//!
//! A torn tab or a dragged leaf released **outside the viewport** of a
//! window that allows it — `LayoutPolicy::drag_out` is `Enabled`, the host
//! has `multi_window`, the window's outer rect is known, it is not itself a
//! micro-window, and no earlier drag-out panel still waits for its window —
//! leaves the source dock. The release point is projected to screen space
//! (outer position + client point × scale). Over a sibling main window the
//! panel docks there at once (leaf under the cursor, else the dock root;
//! `PanelMoved`). Otherwise the engine keeps the panel in its one
//! drag-out session (see [`DragOutView`]) and emits [`LayoutEffect::SpawnWindow`] with a spec
//! sized to the panel (at least 320 x 240, at most 92 % of the source
//! viewport, plus the chrome strip for `DragOutChrome::UzorChrome`) and
//! placed so the cursor sits on the new window's grab point. Anything else
//! — `Disabled`, a single-window host (web), a drop inside the viewport —
//! keeps the part 1 behaviour (in-window `FloatingWindow`, `PanelTornOff`).
//!
//! ## Micro-window
//!
//! The kernel creates the window and answers with
//! [`LayoutOp::AdoptPanel`] (`PanelDraggedOut`); once the host has created
//! it (`LayoutOp::Open`) the held panel becomes its only leaf. It wears the
//! policy's chrome: `Os` decorations, the uzor chrome strip (shown), or
//! none — then the panel header moves the OS window
//! ([`SessionKind::MicroMove`], `SetOuterRect` per move). Nothing tears off
//! a micro-window and it never latches an expand; it is never a drop
//! target.
//!
//! ## Dwell
//!
//! Every [`LayoutEngine::tick`] projects the cursor — the micro-window's
//! outer position plus its grab point (the last press in it, the spawn grab
//! point before) — and looks for a main window whose outer rect contains it
//! (window id order). A dwell over that window **starts** only if the
//! micro-window moved within [`MOVE_FRESH_S`] (an `OuterRect` echo with a new
//! position, or a header move); once started it **completes** after
//! [`DWELL_S`] even if the user holds perfectly still, unless the window has
//! not moved for [`STALE_S`] (a parked window never merges by itself). On
//! completion the panel docks into the leaf under the cursor (or the dock
//! root), `PanelMoved` names it, and the micro-window is hidden, then closed
//! (`SetVisible(false)`, `Close`). Leaving every sibling clears the dwell.
//! Time enters only as `tick(now)` and the ops' `now`.
//!
//! Ported from the previous kernel: `engine/window/drag_out.rs` —
//! `tick_dwell` (:30-179: cursor projection, fresh-move gate `MOVE_FRESH_S`
//! :94, stale reset :118-126, dwell merge :129-165 with hide-then-close),
//! `spawn` (:188-280: sizing, chrome, micro flags), `handle_pointer_move`
//! (:285-337: chrome-less header moves the OS window),
//! `handle_pointer_left` (:354-423: land on a sibling, never into a
//! micro-window, no spawn from a micro-window); `drag_out_policy.rs`
//! (`DragOutChrome::Tessera` is `UzorChrome`; the single-variant
//! `DragOutProbe::Cursor` is dropped, :39-62); the session / dwell shapes of
//! `engine/window/mod.rs:95-110`. Rewritten: typed session state instead of
//! `ui_state` blob keys (`"drag"/"overlap_hover"`, `"moved_at_s"`); the
//! cross-window hit is a pure function over the outer rects the engine
//! holds; the merge docks into a leaf instead of floating; the dwell
//! constant (`DRAG_OVERLAP_DWELL_S`, 1 s) is [`DWELL_S`].
//!
//! [`LayoutEffect::SpawnWindow`]: crate::types::ops::LayoutEffect::SpawnWindow
//! [`LayoutOp::AdoptPanel`]: crate::types::ops::LayoutOp::AdoptPanel
//! [`SessionKind::MicroMove`]: super::SessionKind::MicroMove
//! [`LayoutEngine::tick`]: super::LayoutEngine::tick

use uzor::layout::docking::{DockPanel, LeafId};
use uzor::Rect;

use crate::types::command::{DragOutChrome, DragOutPolicy};
use crate::types::ids::{Seconds, WindowId};
use crate::types::intent::DockIntent;
use crate::types::ops::{DockPointer, LayoutEffect};
use crate::types::window::{Point, SizePx, WindowCommand, WindowSpec};

use super::{dock, rects, Ctx, LayoutEffects, LayoutEngine, WindowLayout};

/// A dwell starts only if the micro-window moved within this many seconds.
pub const MOVE_FRESH_S: f64 = 0.75;
/// How long the cursor must stay over a sibling for the panel to dock back.
pub const DWELL_S: f64 = 1.0;
/// A running dwell is dropped once the micro-window has not moved for this
/// long (it would otherwise merge a parked window after the user let go).
pub const STALE_S: f64 = 1.25;

/// Smallest micro-window inner size, logical px.
const MICRO_MIN: (f64, f64) = (320.0, 240.0);
/// Largest micro-window inner size as a fraction of the source viewport.
const MICRO_CAP: f64 = 0.92;
/// Minimum inner size the OS may shrink a micro-window to, physical px.
const MICRO_MIN_INNER: SizePx = SizePx::new(240, 160);
/// Height of the uzor chrome strip a `UzorChrome` micro-window shows.
const MICRO_CHROME_H: f64 = 32.0;
/// Horizontal grab point cap, logical px from the left.
const GRAB_X: f64 = 60.0;

/// The panels a drag carries: one torn tab, or every panel of a leaf.
#[derive(Clone, Debug)]
pub(super) struct Payload<P> {
    panels: Vec<P>,
    active: usize,
    /// Logical size of the leaf it left.
    size: (f64, f64),
}

impl<P: DockPanel> Payload<P> {
    pub(super) fn new(panels: Vec<P>, active: usize, size: (f64, f64)) -> Self {
        Self {
            panels,
            active,
            size,
        }
    }

    pub(super) fn into_parts(self) -> (Vec<P>, usize) {
        (self.panels, self.active)
    }

    fn title(&self) -> String {
        self.panels
            .get(self.active)
            .or_else(|| self.panels.first())
            .map(|p| p.title().to_string())
            .unwrap_or_default()
    }
}

/// A panel released outside its window, handed from the window's pointer
/// op to the engine.
pub(super) struct Outbound<P> {
    leaf: LeafId,
    payload: Payload<P>,
    pos: Point,
}

impl<P> Outbound<P> {
    pub(super) fn new(leaf: LeafId, payload: Payload<P>, pos: Point) -> Self {
        Self { leaf, payload, pos }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Dwell {
    started: Seconds,
    target: WindowId,
}

/// The one drag-out of the engine (previous kernel `DragOutSession` +
/// `DwellProbe`, `engine/window/mod.rs:95-110`).
pub(super) struct DragOutSession<P> {
    src: WindowId,
    from: LeafId,
    chrome: DragOutChrome,
    started: Seconds,
    /// The panel until the micro-window is open.
    held: Option<Payload<P>>,
    micro: Option<WindowId>,
    /// Cursor position inside the micro-window, logical px.
    grab: Point,
    last_moved: Option<Seconds>,
    dwell: Option<Dwell>,
}

impl<P> DragOutSession<P> {
    /// A panel waits for its micro-window.
    pub(super) fn holds_panel(&self) -> bool {
        self.held.is_some()
    }

    /// When a running dwell completes.
    pub(super) fn deadline(&self) -> Option<Seconds> {
        self.dwell.map(|d| d.started.after(Seconds(DWELL_S)))
    }

    fn placed_in(&self) -> Option<WindowId> {
        match (self.micro, &self.held) {
            (Some(m), None) => Some(m),
            _ => None,
        }
    }
}

/// Published state of the drag-out session.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragOutView {
    src: WindowId,
    from: LeafId,
    chrome: DragOutChrome,
    started: Seconds,
    micro: Option<WindowId>,
    placed: bool,
    dwell: Option<(WindowId, Seconds)>,
}

impl DragOutView {
    /// The window the panel came from.
    pub fn src(&self) -> WindowId {
        self.src
    }

    /// The leaf it left (may be gone).
    pub fn from_leaf(&self) -> LeafId {
        self.from
    }

    /// The micro-window's chrome.
    pub fn chrome(&self) -> DragOutChrome {
        self.chrome
    }

    /// When the drag-out started.
    pub fn started(&self) -> Seconds {
        self.started
    }

    /// The micro-window, once allocated.
    pub fn micro(&self) -> Option<WindowId> {
        self.micro
    }

    /// The panel is in the micro-window's dock (it is open).
    pub fn placed(&self) -> bool {
        self.placed
    }

    /// The sibling the cursor dwells over, and since when.
    pub fn dwell(&self) -> Option<(WindowId, Seconds)> {
        self.dwell
    }
}

/// Whether a release outside this window's viewport starts a drag-out.
pub(super) fn allowed<P: DockPanel>(w: &WindowLayout<P>, ctx: &Ctx<P>) -> bool {
    matches!(ctx.policy.drag_out, DragOutPolicy::Enabled { .. })
        && w.caps.multi_window
        && w.micro.is_none()
        && w.outer.is_some()
        && !ctx.dragout_busy
}

/// A window-local logical point in physical screen px.
fn project<P: DockPanel>(w: &WindowLayout<P>, client: Point) -> Option<Point> {
    w.outer
        .map(|o| Point::new(o.x + client.x * w.scale, o.y + client.y * w.scale))
}

/// A move of a chrome-less micro-window's header session: the OS window
/// follows so the cursor stays on the grab point (drag_out.rs:317-335). The
/// outer rect is updated at once so the next move builds on it.
pub(super) fn micro_move<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    grab: Point,
    p: Point,
    fx: &mut LayoutEffects,
) {
    let Some(o) = w.outer else {
        return;
    };
    let (dx, dy) = ((p.x - grab.x) * w.scale, (p.y - grab.y) * w.scale);
    if dx.abs() < 0.5 && dy.abs() < 0.5 {
        return;
    }
    let moved = Rect::new((o.x + dx).round(), (o.y + dy).round(), o.width, o.height);
    w.outer = Some(moved);
    fx.push(LayoutEffect::Window {
        win,
        cmd: WindowCommand::SetOuterRect {
            position: (moved.x as i32, moved.y as i32),
            size: SizePx::new(o.width.max(1.0) as u32, o.height.max(1.0) as u32),
        },
    });
}

impl<P: DockPanel> LayoutEngine<P> {
    /// The observable part of the session (revisioned, published).
    pub(super) fn dragout_obs(&self) -> Option<DragOutView> {
        self.dragout.as_ref().map(|s| DragOutView {
            src: s.src,
            from: s.from,
            chrome: s.chrome,
            started: s.started,
            micro: s.micro,
            placed: s.placed_in().is_some(),
            dwell: s.dwell.map(|d| (d.target, d.started)),
        })
    }

    /// A press in the micro-window is the grab point the cursor keeps while
    /// the OS (or the header session) moves the window.
    pub(super) fn dragout_pointer(&mut self, win: WindowId, event: DockPointer) {
        if let (Some(s), DockPointer::Down { pos, .. }) = (&mut self.dragout, event) {
            if s.micro == Some(win) {
                s.grab = pos;
            }
        }
    }

    /// The window's outer position changed at `now`.
    pub(super) fn dragout_moved(&mut self, win: WindowId, now: Seconds) {
        if let Some(s) = &mut self.dragout {
            if s.placed_in() == Some(win) {
                s.last_moved = Some(now);
            }
        }
    }

    /// The first window whose outer rect contains `screen`, skipping
    /// `exclude` and every micro-window (never a drop target).
    fn window_at(&self, screen: Point, exclude: WindowId) -> Option<WindowId> {
        self.windows
            .iter()
            .filter(|(id, w)| **id != exclude && w.micro.is_none())
            .find(|(_, w)| w.outer.is_some_and(|o| rects::contains(o, screen)))
            .map(|(id, _)| *id)
    }

    /// Dock `payload` into `dst` at the screen point (leaf under it, else
    /// the dock root). The leaf that holds it.
    fn dock_at_screen(
        &mut self,
        dst: WindowId,
        screen: Point,
        payload: Payload<P>,
        fx: &mut LayoutEffects,
    ) -> Option<LeafId> {
        let mut out = None;
        self.edit(dst, fx, |w, _, _| {
            let local = match w.outer {
                Some(o) => Point::new((screen.x - o.x) / w.scale, (screen.y - o.y) / w.scale),
                None => Point::new(f64::NAN, f64::NAN),
            };
            let leaf = dock::leaf_at(&w.dock, local);
            out = dock::dock_payload(&mut w.dock, leaf, payload);
        });
        out
    }

    /// A panel was released outside `src` (module docs, "Start").
    pub(super) fn dragout_start(
        &mut self,
        src: WindowId,
        now: Seconds,
        out: Outbound<P>,
        fx: &mut LayoutEffects,
    ) {
        let Outbound { leaf, payload, pos } = out;
        let chrome = match self.policy.drag_out {
            DragOutPolicy::Enabled { chrome } => Some(chrome),
            DragOutPolicy::Disabled => None,
        };
        let screen = self.windows.get(&src).and_then(|w| project(w, pos));
        let (Some(chrome), Some(screen)) = (chrome, screen) else {
            // Unreachable (checked before the payload left): give it back.
            self.dock_at_screen(src, Point::new(f64::NAN, f64::NAN), payload, fx);
            return;
        };
        if let Some(dst) = self.window_at(screen, src) {
            if let Some(to) = self.dock_at_screen(dst, screen, payload, fx) {
                fx.push(LayoutEffect::Intent(DockIntent::PanelMoved {
                    from: (src, leaf),
                    to: (dst, to),
                    zone: uzor::layout::docking::DropZone::Center,
                }));
            }
            return;
        }
        let Some(sw) = self.windows.get(&src) else {
            return;
        };
        let (spec, grab) = micro_spec(sw, src, leaf, &payload, chrome, screen);
        self.dragout = Some(DragOutSession {
            src,
            from: leaf,
            chrome,
            started: now,
            held: Some(payload),
            micro: None,
            grab,
            last_moved: None,
            dwell: None,
        });
        fx.push(LayoutEffect::SpawnWindow { src, spec });
    }

    /// `LayoutOp::AdoptPanel`: `win` is the micro-window the spawn created.
    pub(super) fn dragout_adopt(&mut self, win: WindowId, fx: &mut LayoutEffects) {
        let Some(s) = &mut self.dragout else {
            return;
        };
        if s.micro.is_some() || s.held.is_none() {
            return;
        }
        s.micro = Some(win);
        fx.push(LayoutEffect::Intent(DockIntent::PanelDraggedOut {
            from: (s.src, s.from),
            to: win,
        }));
        self.dragout_place(win, fx);
    }

    /// Hand the held panel to the micro-window once it is open.
    pub(super) fn dragout_place(&mut self, win: WindowId, fx: &mut LayoutEffects) {
        if !self.windows.contains_key(&win) {
            return;
        }
        let Some(s) = &mut self.dragout else {
            return;
        };
        if s.micro != Some(win) {
            return;
        }
        let Some(payload) = s.held.take() else {
            return;
        };
        let chrome = s.chrome;
        self.edit(win, fx, |w, _, _| {
            w.micro = Some(chrome);
            if chrome == DragOutChrome::UzorChrome {
                w.chrome_slot.visible = true;
                w.chrome_slot.height = MICRO_CHROME_H as f32;
            }
            // The micro-window never expands.
            w.expand = None;
            dock::dock_payload(&mut w.dock, None, payload);
        });
    }

    /// `LayoutOp::Close`: closing the micro-window ends the session.
    pub(super) fn dragout_window_closed(&mut self, win: WindowId) {
        if self.dragout.as_ref().is_some_and(|s| s.micro == Some(win)) {
            self.dragout = None;
        }
    }

    /// The dwell step of [`LayoutEngine::tick`] (drag_out.rs:61-169).
    pub(super) fn dragout_tick(&mut self, now: Seconds, fx: &mut LayoutEffects) {
        let Some(s) = &self.dragout else {
            return;
        };
        let Some(m) = s.placed_in() else {
            return;
        };
        let screen = self.windows.get(&m).and_then(|w| project(w, s.grab));
        let target = screen.and_then(|p| self.window_at(p, m));
        let since_move = s.last_moved.map(|t| now.get() - t.get());
        let fresh = since_move.is_some_and(|d| d <= MOVE_FRESH_S);
        let stale = since_move.is_none_or(|d| d > STALE_S);
        let next = match (target, s.dwell) {
            (None, _) => None,
            // First overlap during an active move: start the timer.
            (Some(target), None) if fresh => Some(Dwell {
                started: now,
                target,
            }),
            // Overlapping but parked: no timer.
            (Some(_), None) => None,
            // The window stopped long ago: a parked window never merges.
            (Some(_), Some(_)) if stale => None,
            // Moved on to another sibling: restart there.
            (Some(target), Some(d)) if d.target != target => fresh.then_some(Dwell {
                started: now,
                target,
            }),
            (Some(target), Some(d)) if now.get() - d.started.get() >= DWELL_S => {
                if let Some(screen) = screen {
                    self.dragout_complete(m, target, screen, fx);
                }
                return;
            }
            (Some(_), Some(d)) => Some(d),
        };
        if let Some(s) = &mut self.dragout {
            s.dwell = next;
        }
    }

    /// The dwell completed: dock the micro-window's panels into `dst` and
    /// hide, then close, the micro-window.
    fn dragout_complete(
        &mut self,
        micro: WindowId,
        dst: WindowId,
        screen: Point,
        fx: &mut LayoutEffects,
    ) {
        let mut taken = None;
        self.edit(micro, fx, |w, _, _| taken = dock::take_all(&mut w.dock));
        if let Some((from, payload)) = taken {
            if let Some(to) = self.dock_at_screen(dst, screen, payload, fx) {
                fx.push(LayoutEffect::Intent(DockIntent::PanelMoved {
                    from: (micro, from),
                    to: (dst, to),
                    zone: uzor::layout::docking::DropZone::Center,
                }));
            }
        }
        // Hide first: the OS may still own the window in a modal move loop
        // and defer the close (drag_out.rs:150-164).
        fx.push(LayoutEffect::Window {
            win: micro,
            cmd: WindowCommand::SetVisible(false),
        });
        fx.push(LayoutEffect::Window {
            win: micro,
            cmd: WindowCommand::Close,
        });
        self.dragout = None;
    }
}

/// The micro-window spec and its grab point (drag_out.rs:208-269).
fn micro_spec<P: DockPanel>(
    sw: &WindowLayout<P>,
    src: WindowId,
    leaf: LeafId,
    payload: &Payload<P>,
    chrome: DragOutChrome,
    screen: Point,
) -> (WindowSpec, Point) {
    let pad_top = match chrome {
        DragOutChrome::UzorChrome => MICRO_CHROME_H,
        DragOutChrome::Os | DragOutChrome::None => 0.0,
    };
    let vp = sw.viewport;
    let (pw, ph) = payload.size;
    let w = pw.min(vp.width * MICRO_CAP).max(MICRO_MIN.0);
    let h = (ph + pad_top).min(vp.height * MICRO_CAP).max(MICRO_MIN.1);
    let header = sw.dock.tab_bar_height() as f64;
    let grab = Point::new(
        (w / 2.0).min(GRAB_X),
        if pad_top > 0.0 {
            pad_top / 2.0
        } else {
            header / 2.0
        },
    );
    let scale = sw.scale;
    let inner = SizePx::new((w * scale).round() as u32, (h * scale).round() as u32);
    let mut spec = WindowSpec::new(
        format!("drag-out-{}-{}", src.0, leaf.0),
        payload.title(),
        inner,
    );
    spec.min_inner_size = Some(MICRO_MIN_INNER);
    spec.decorations = chrome == DragOutChrome::Os;
    spec.position = Some((
        (screen.x - grab.x * scale).round() as i32,
        (screen.y - grab.y * scale).round() as i32,
    ));
    (spec, grab)
}
