//! LayoutEngine: per-window chrome, edge slots, dock trees and solved rects
//! (design §3.2): rect solve, dock glue, typed hit-testing, splitters with
//! policy, chrome / bezel, layout persistence (part 1) and inner / outer
//! expand, drag-out and dwell (part 2, [`expand`], [`dragout`]).
//!
//! The one writer of window layout state. Every change goes through
//! [`LayoutEngine::apply`] (and time through [`LayoutEngine::tick`]); the
//! library [`DockState`] of each window is private and reachable only
//! read-only through the view. No widget-id strings are parsed anywhere:
//! a pointer position is classified once, by [`LayoutEngine::hit`], into a
//! typed [`LayoutHit`].
//!
//! ## Per window
//!
//! | Field | Library piece | Written by |
//! |---|---|---|
//! | chrome slot + [`ChromeModel`] | `ChromeSlot`, chrome composite hit-test | `SetChrome`, `SetChromeModel` |
//! | edge slots | `EdgePanels` | `AddEdgeSlot`, `RemoveEdgeSlot` |
//! | dock | `DockState<P>` (tree, separators, tabs, floating, drag, snap-back) | dock commands, pointer sessions |
//! | solved rects | `solve_layout` + `DockState::layout` | re-solved after every change (rects are never stale) |
//!
//! A new window has an empty dock, a hidden chrome strip (32 px when
//! shown) and no edge slots.
//!
//! ## Hit precedence (exactly one [`LayoutHit`] per point)
//!
//! [`LayoutEngine::hit`] walks one fixed list and returns the first zone
//! that contains the point; each step returns at most one value, so every
//! point of a window maps to exactly one hit:
//!
//! 1. outside the viewport → `None`;
//!    then, only while a panel drag is live in a window that may expand
//!    (see [`expand`]), the edge gutter bands → `EdgeGutter(side)`;
//! 2. the resize bezel, `LayoutPolicy::bezel_px` wide along the viewport
//!    border, only when the host has no OS resize border → `Bezel(dir)`
//!    (corners when two borders meet);
//! 3. the chrome strip → `Chrome(part)` (inert areas `Chrome(None)`). The
//!    chrome composite's own border zone is not a resize zone: the bezel is
//!    the one owner of window resize, so a point there is re-classified as
//!    the chrome part just inside it;
//! 4. an edge slot (toolbar, sidebar) → `None` (its widgets are content);
//! 5. floating windows, topmost first: resize border → `FloatingResize`,
//!    close button → `FloatingClose`, header → `FloatingHeader`, rest →
//!    `FloatingBody`;
//! 6. the dock: a real splitter crossing → `Corner`; a splitter →
//!    `Splitter { sep, orientation }`; a multi-tab leaf's bar: "+" →
//!    `TabNew`, a chip's close button → `TabClose`, a chip → `Tab`; a
//!    single-tab header: "+" → `TabNew`, else `PanelHeader`; a leaf's rect
//!    → `PanelBody` (leaves in `LeafId` order);
//! 7. `None`.
//!
//! ## Pointer sessions
//!
//! [`DockPointer::Down`] re-runs the same hit (so routing and action can
//! never disagree) and, for the primary button, starts at most one session
//! per window. A session that needs the pointer emits
//! [`LayoutEffect::Capture`] `{ hold: true }` on press and `{ hold: false }`
//! when it ends (release or cancel):
//!
//! - **Splitter / corner**: the separator follows the pointer (the offset
//!   between pointer and line at press is kept); every move calls
//!   `DockState::drag_separator` with the window's [`SplitterPolicy`]
//!   (`Cascade` default, `RejectSnapBack`, `Clamp`). Under `RejectSnapBack`
//!   the first refused move queues the library snap-back and freezes the
//!   drag until release (the whole drag is rejected, one snap-back per
//!   drag); [`LayoutEngine::tick`] advances the spring until it settles.
//! - **Tab chip**: the press activates the tab (`TabActivated` when it was
//!   not active). Past a 4 px move inside the bar the tab reorders; leaving
//!   the bar (or moving off it before) tears it off: the library tab drag
//!   targets headers / bars / bodies / window edges and the release drops it
//!   (`PanelMoved`; Center joins the target's stack and makes it active; a
//!   miss leaves the stack unchanged).
//! - **Panel header**: the press activates the leaf; past 4 px the whole
//!   leaf drags; the release drops it (`PanelMoved`) or, with no target,
//!   floats it in-window (`PanelTornOff`).
//! - A locked panel ([`Pin::locks_tear_off`](uzor::layout::docking::Pin))
//!   never starts a tear-off or leaf drag: the library refuses and the
//!   session stays inert until release.
//! - **Floating header / resize border**: moves / resizes the floating
//!   window inside the dock area.
//! - **Clicks** (tab close, "+", floating close, chrome buttons and tabs):
//!   act on release only when the release hits the same part
//!   (`PanelClosed` for closable panels, `NewPanelRequested`,
//!   [`LayoutEffect::Chrome`]).
//! - **Chrome drag zone / bezel**: the press emits `WindowCommand::DragWindow`
//!   / `DragResizeWindow(dir)` for the OS; no session.
//!
//! ## Intents and revisions
//!
//! Every op compares an observation of the touched window (tree, ratios,
//! rects, tabs, floating windows, drag / snap-back state, chrome, edges,
//! policy) before and after: the engine revision bumps exactly once when it
//! changed and never otherwise. The dock revision (`DockView::dock_rev`)
//! bumps exactly when the blob-relevant part (tree structure, ratios,
//! active tabs / leaf, floating windows) changed; such a window is queued
//! and [`LayoutEngine::tick`] emits one `DockIntent::LayoutChanged` per
//! queued window (coalesced per tick). `RequestBlob` / `Restore` answer at
//! once (`LayoutBlob` / `LayoutBlobFailed`, `LayoutRestored` /
//! `LayoutRestoreFailed`); a failed restore changes nothing.
//!
//! ## Layout blob
//!
//! A postcard envelope: a `u16` version, then the dock structure (the
//! library `LayoutSnapshot` fields, mirrored without the attributes a
//! non-self-describing format cannot honour) and the floating windows.
//! Restore validates the structure (unique ids, a tree, bounded size and
//! depth) before the library rebuilds it, decodes every panel with the
//! app's [`PanelDecoder`] (`Spec::decode_panel`) and swaps the window's dock
//! only when everything succeeded. Malformed bytes, an unknown version or a
//! panel the app cannot decode are typed [`LayoutCodecError`]s, never
//! panics.
//!
//! ## Seams
//!
//! The kernel (F7) conducts the effects (capture, window commands, chrome
//! actions, invalidation, expand targets, micro-window spawn), feeds
//! `OuterRect` / `ExpandValue` / `AdoptPanel`, and decides routing with
//! [`LayoutHit::claims_press`].
//!
//! [`SplitterPolicy`]: crate::types::command::SplitterPolicy
//! [`LayoutCodecError`]: crate::types::layout_blob::LayoutCodecError

mod blob;
mod chrome;
mod dock;
pub mod dragout;
pub mod expand;
mod rects;
mod splitter;

use std::collections::{BTreeMap, BTreeSet};

use smallvec::SmallVec;
use uzor::layout::docking::{DockPanel, FloatingWindowId, LeafId, SnapBackAnimation};
use uzor::layout::{ChromeSlot, EdgePanels, EdgeSide, LayoutSolved, LayoutTree};
use uzor::render::InvalidateBits;
use uzor::widgets::composite::chrome::ChromeState;
use uzor::{Rect, ResizeDirection};

use crate::types::bus::HostCaps;
use crate::types::command::{
    ChromeModel, DockTarget, DragOutChrome, LayoutCmd, LayoutHit, LayoutPolicy,
};
use crate::types::ids::{Revision, Seconds, WindowId};
use crate::types::intent::DockIntent;
use crate::types::layout_blob::WindowGeometrySnapshot;
use crate::types::ops::{DockPointer, LayoutEffect, LayoutOp};
use crate::types::snapshot::{ChromeView, DockView, EdgeSlotView};
use crate::types::spec::{PanelHome, Spec};
use crate::types::window::{Point, WindowCommand};

pub use blob::BLOB_VERSION;
pub use dragout::{DragOutView, DWELL_S, MOVE_FRESH_S, STALE_S};
pub use expand::{ExpandState, ExpandTarget};
pub use uzor::layout::DockState;

/// Effects of one LayoutEngine op or tick.
pub type LayoutEffects = SmallVec<[LayoutEffect; 2]>;

/// The app's panel factory for layout restore (`Spec::decode_panel`).
pub type PanelDecoder<P> = fn(PanelHome, &str) -> Option<P>;

/// Pointer travel (logical px) before a tab / header press becomes a drag.
pub const DRAG_START_PX: f64 = 4.0;

/// The kind of pointer session a window's layout holds (for compose: which
/// splitter is active, whether a drag ghost is shown).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SessionKind {
    /// A splitter drag (`frozen` after a rejected move under
    /// `RejectSnapBack`).
    Splitter {
        /// Separator index.
        sep: usize,
        /// The drag was refused and waits for release.
        frozen: bool,
    },
    /// A corner drag (two separators).
    Corner {
        /// Vertical separator index.
        vertical: usize,
        /// Horizontal separator index.
        horizontal: usize,
        /// The drag was refused and waits for release.
        frozen: bool,
    },
    /// A tab chip is pressed (not moved yet).
    TabPressed {
        /// The leaf.
        leaf: LeafId,
        /// Tab index.
        tab: usize,
    },
    /// A tab is being reordered inside its bar.
    TabReorder {
        /// The leaf.
        leaf: LeafId,
    },
    /// A panel header is pressed (not moved yet).
    HeaderPressed(LeafId),
    /// A tab or a whole leaf is being dragged over the dock (drop targets
    /// live in the dock's `panel_drag_state`).
    PanelDrag {
        /// The source leaf.
        leaf: LeafId,
    },
    /// A tear-off or leaf drag was refused (locked panel); inert until
    /// release.
    Refused,
    /// A floating window is being moved.
    FloatingMove(FloatingWindowId),
    /// A floating window is being resized.
    FloatingResize(FloatingWindowId),
    /// A click-type part is pressed; it acts on release over the same part.
    Click(LayoutHit),
    /// A drag-out micro-window without chrome follows its panel header.
    MicroMove,
}

/// One live pointer session of a window.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Session {
    Splitter(splitter::SplitDrag),
    Tab {
        leaf: LeafId,
        tab: usize,
        origin: Point,
        stage: dock::TabStage,
    },
    Header {
        leaf: LeafId,
        origin: Point,
        stage: dock::HeaderStage,
    },
    FloatingMove {
        id: FloatingWindowId,
        grab: (f64, f64),
    },
    FloatingResize {
        id: FloatingWindowId,
        dir: ResizeDirection,
        origin: Point,
        start: Rect,
    },
    Click(LayoutHit),
    /// Moving a chrome-less micro-window by its panel header; `grab` is the
    /// press point (window-local), which the cursor keeps while the window
    /// follows it.
    MicroMove {
        grab: Point,
    },
}

impl Session {
    fn kind(&self) -> SessionKind {
        match *self {
            Session::Splitter(d) => d.kind(),
            Session::Tab {
                leaf, tab, stage, ..
            } => match stage {
                dock::TabStage::Pressed => SessionKind::TabPressed { leaf, tab },
                dock::TabStage::Reorder => SessionKind::TabReorder { leaf },
                dock::TabStage::Tear => SessionKind::PanelDrag { leaf },
                dock::TabStage::Refused => SessionKind::Refused,
            },
            Session::Header { leaf, stage, .. } => match stage {
                dock::HeaderStage::Pressed => SessionKind::HeaderPressed(leaf),
                dock::HeaderStage::Dragging => SessionKind::PanelDrag { leaf },
                dock::HeaderStage::Refused => SessionKind::Refused,
            },
            Session::FloatingMove { id, .. } => SessionKind::FloatingMove(id),
            Session::FloatingResize { id, .. } => SessionKind::FloatingResize(id),
            Session::Click(hit) => SessionKind::Click(hit),
            Session::MicroMove { .. } => SessionKind::MicroMove,
        }
    }
}

/// Layout state of one window.
struct WindowLayout<P: DockPanel> {
    caps: HostCaps,
    viewport: Rect,
    chrome_slot: ChromeSlot,
    chrome: ChromeModel,
    chrome_state: ChromeState,
    edges: EdgePanels,
    tree: LayoutTree,
    solved: LayoutSolved,
    dock: DockState<P>,
    session: Option<Session>,
    dock_rev: Revision,
    wire: blob::WireDock,
    /// Outer rect (physical screen px), from host echoes.
    outer: Option<Rect>,
    /// Device pixel ratio from the last outer-rect echo.
    scale: f64,
    /// The latched expand, if any.
    expand: Option<expand::Expand>,
    /// `Some` for a drag-out micro-window (with the chrome it wears).
    micro: Option<DragOutChrome>,
}

impl<P: DockPanel> WindowLayout<P> {
    fn new(caps: HostCaps, viewport: Rect, policy: &LayoutPolicy) -> Self {
        let mut dock = DockState::new();
        dock::configure(&mut dock, policy);
        let wire = blob::WireDock::capture(&dock);
        let mut w = Self {
            caps,
            viewport,
            chrome_slot: ChromeSlot {
                height: 32.0,
                visible: false,
            },
            chrome: ChromeModel::default(),
            chrome_state: ChromeState::new(),
            edges: EdgePanels::new(),
            tree: LayoutTree::new(),
            solved: LayoutSolved::default(),
            dock,
            session: None,
            dock_rev: Revision::ZERO,
            wire,
            outer: None,
            scale: 1.0,
            expand: None,
            micro: None,
        };
        rects::solve(&mut w);
        w
    }
}

/// Everything observable about one window, compared before / after each op
/// to bump the revision exactly on change.
#[derive(PartialEq)]
struct Observed {
    caps: HostCaps,
    viewport: Rect,
    chrome: chrome::ChromeObs,
    edges: Vec<EdgeSlotView>,
    dock: dock::DockObs,
    session: Option<SessionKind>,
    outer: Option<Rect>,
    scale: f64,
    expand: Option<expand::Expand>,
    micro: Option<DragOutChrome>,
}

impl Observed {
    fn of<P: DockPanel>(w: &WindowLayout<P>) -> Self {
        Self {
            caps: w.caps,
            viewport: w.viewport,
            chrome: chrome::ChromeObs::of(w),
            edges: rects::edge_views(w),
            dock: dock::DockObs::of(&w.dock),
            session: w.session.map(|s| s.kind()),
            outer: w.outer,
            scale: w.scale,
            expand: w.expand,
            micro: w.micro,
        }
    }
}

/// Engine-wide settings an op on one window may read.
struct Ctx<P> {
    policy: LayoutPolicy,
    decode: PanelDecoder<P>,
    /// A drag-out panel is held waiting for its micro-window: no second
    /// drag-out may start.
    dragout_busy: bool,
}

impl<P> Clone for Ctx<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for Ctx<P> {}

/// The LayoutEngine. `P` is the app's panel type ([`Spec::Panel`]).
pub struct LayoutEngine<P: DockPanel> {
    windows: BTreeMap<WindowId, WindowLayout<P>>,
    dragout: Option<dragout::DragOutSession<P>>,
    policy: LayoutPolicy,
    decode: PanelDecoder<P>,
    changed: BTreeSet<WindowId>,
    clock: Option<Seconds>,
    /// Something observable changed during the current op / tick; the
    /// revision bumps once when it ends.
    touched: bool,
    rev: Revision,
}

impl<P: DockPanel> LayoutEngine<P> {
    /// An engine with no windows, the default [`LayoutPolicy`] and the
    /// app's panel factory for layout restore.
    pub fn new(decode: PanelDecoder<P>) -> Self {
        Self::with_policy(decode, LayoutPolicy::default())
    }

    /// An engine with no windows and the given policy.
    pub fn with_policy(decode: PanelDecoder<P>, policy: LayoutPolicy) -> Self {
        Self {
            windows: BTreeMap::new(),
            dragout: None,
            policy,
            decode,
            changed: BTreeSet::new(),
            clock: None,
            touched: false,
            rev: Revision::ZERO,
        }
    }

    /// An engine whose restore uses `S::decode_panel`.
    pub fn for_spec<S: Spec<Panel = P>>() -> Self {
        Self::new(S::decode_panel)
    }

    /// The only mutation door.
    pub fn apply(&mut self, op: LayoutOp<P>) -> LayoutEffects {
        let before = self.dragout_obs();
        let mut fx = LayoutEffects::new();
        match op {
            LayoutOp::Open {
                win,
                caps,
                viewport,
            } => {
                if !self.windows.contains_key(&win) {
                    self.windows
                        .insert(win, WindowLayout::new(caps, viewport, &self.policy));
                    self.touched = true;
                    fx.push(invalidate(win, InvalidateBits::ALL));
                } else {
                    self.edit(win, &mut fx, |w, _, _| {
                        w.caps = caps;
                        w.viewport = viewport;
                    });
                }
                self.dragout_place(win, &mut fx);
            }
            LayoutOp::Close(win) => {
                if self.windows.remove(&win).is_some() {
                    self.changed.remove(&win);
                    self.touched = true;
                }
                self.dragout_window_closed(win);
            }
            LayoutOp::Solve { win, viewport } => {
                self.edit(win, &mut fx, |w, _, _| w.viewport = viewport);
            }
            LayoutOp::Pointer { win, now, event } => {
                self.clock = Some(match self.clock {
                    Some(c) if c.get() > now.get() => c,
                    _ => now,
                });
                self.dragout_pointer(win, event);
                let outer_before = self.windows.get(&win).and_then(|w| w.outer);
                let mut out = None;
                self.edit(win, &mut fx, |w, ctx, fx| {
                    pointer(win, w, ctx, event, fx, &mut out)
                });
                let outer_after = self.windows.get(&win).and_then(|w| w.outer);
                if outer_after != outer_before {
                    self.dragout_moved(win, now);
                }
                if let Some(out) = out {
                    self.dragout_start(win, now, out, &mut fx);
                }
            }
            LayoutOp::Cmd(cmd) => self.cmd(cmd, &mut fx),
            LayoutOp::OuterRect {
                win,
                now,
                rect,
                scale,
            } => {
                let before = self.windows.get(&win).and_then(|w| w.outer);
                let scale = if scale.is_finite() && scale > 0.0 {
                    scale
                } else {
                    1.0
                };
                self.edit(win, &mut fx, |w, _, _| {
                    w.outer = Some(rect);
                    w.scale = scale;
                });
                let moved = match before {
                    Some(b) => b.x != rect.x || b.y != rect.y,
                    None => false,
                };
                if moved {
                    self.dragout_moved(win, now);
                }
            }
            LayoutOp::ExpandValue { win, kind, t } => {
                self.edit(win, &mut fx, |w, _, fx| expand::value(win, w, kind, t, fx));
            }
            LayoutOp::AdoptPanel { win } => self.dragout_adopt(win, &mut fx),
        }
        self.settle_rev(before);
        fx
    }

    /// End of an op / tick: one revision bump when anything observable
    /// changed (a window or the drag-out session).
    fn settle_rev(&mut self, dragout_before: Option<DragOutView>) {
        if std::mem::take(&mut self.touched) || self.dragout_obs() != dragout_before {
            self.rev.bump();
        }
    }

    /// Advance time: snap-back springs move (one revision bump and one
    /// `GEOMETRY` invalidation per window whose springs moved), the
    /// drag-out dwell runs (see [`dragout`]), then one
    /// `DockIntent::LayoutChanged` per window whose dock changed since the
    /// last tick.
    pub fn tick(&mut self, now: Seconds) -> LayoutEffects {
        let dragout_before = self.dragout_obs();
        let dt = match self.clock {
            Some(c) => now.since(c).get(),
            None => 0.0,
        };
        match self.clock {
            Some(c) if c.get() >= now.get() => {}
            _ => self.clock = Some(now),
        }
        let mut fx = LayoutEffects::new();
        let mut moved = false;
        for (&win, w) in self.windows.iter_mut() {
            if w.dock.snap_animations().is_empty() {
                continue;
            }
            let before = splitter::snap_obs(&w.dock);
            w.dock.update_snap_animations(dt as f32);
            if splitter::snap_obs(&w.dock) != before {
                moved = true;
                fx.push(invalidate(win, InvalidateBits::GEOMETRY));
            }
        }
        if moved {
            self.touched = true;
        }
        self.dragout_tick(now, &mut fx);
        for win in std::mem::take(&mut self.changed) {
            if let Some(w) = self.windows.get(&win) {
                fx.push(LayoutEffect::Intent(DockIntent::LayoutChanged {
                    win,
                    dock_rev: w.dock_rev,
                }));
            }
        }
        self.settle_rev(dragout_before);
        fx
    }

    /// What `p` (window-local logical pixels) hits in `win`; exactly one
    /// value per point (see the module docs for the precedence). A pure
    /// query: nothing changes.
    pub fn hit(&self, win: WindowId, p: Point) -> LayoutHit {
        match self.windows.get(&win) {
            Some(w) => hit_window(w, &self.policy, p),
            None => LayoutHit::None,
        }
    }

    /// Bumped exactly when observable layout state changes.
    pub fn revision(&self) -> Revision {
        self.rev
    }

    /// Read-only projection for publish, compose and other engines' ticks.
    pub fn view(&self) -> LayoutEngineView<'_, P> {
        LayoutEngineView { engine: self }
    }

    // ---------------------------------------------------------------------

    /// Run `f` on one window and account for what it changed: re-solve,
    /// mark the op as changed (one revision bump when it ends) if anything
    /// observable moved, bump the dock revision and queue `LayoutChanged`
    /// if the blob-relevant part moved.
    fn edit<F>(&mut self, win: WindowId, fx: &mut LayoutEffects, f: F)
    where
        F: FnOnce(&mut WindowLayout<P>, Ctx<P>, &mut LayoutEffects),
    {
        if self.edit_quiet(win, fx, f) {
            self.touched = true;
        }
    }

    /// [`Self::edit`] without the revision bump; `true` when something
    /// observable changed.
    fn edit_quiet<F>(&mut self, win: WindowId, fx: &mut LayoutEffects, f: F) -> bool
    where
        F: FnOnce(&mut WindowLayout<P>, Ctx<P>, &mut LayoutEffects),
    {
        let ctx = Ctx {
            policy: self.policy,
            decode: self.decode,
            dragout_busy: self.dragout.as_ref().is_some_and(|s| s.holds_panel()),
        };
        let Some(w) = self.windows.get_mut(&win) else {
            return false;
        };
        let before = Observed::of(w);
        f(w, ctx, fx);
        rects::solve(w);
        let after = Observed::of(w);
        let moved = after != before;
        if moved {
            let bits = if after.dock.structure_differs(&before.dock) {
                InvalidateBits::STRUCTURE.union(InvalidateBits::GEOMETRY)
            } else {
                InvalidateBits::GEOMETRY
            };
            fx.push(invalidate(win, bits));
        }
        let wire = blob::WireDock::capture(&w.dock);
        if wire != w.wire {
            w.wire = wire;
            w.dock_rev.bump();
            self.changed.insert(win);
        }
        moved
    }

    fn cmd(&mut self, cmd: LayoutCmd<P>, fx: &mut LayoutEffects) {
        match cmd {
            LayoutCmd::SetPolicy(policy) => {
                if policy == self.policy {
                    return;
                }
                // One bump for the whole op (the policy itself changed).
                self.policy = policy;
                self.touched = true;
                let wins: Vec<WindowId> = self.windows.keys().copied().collect();
                for win in wins {
                    self.edit_quiet(win, fx, |w, ctx, _| {
                        dock::configure(&mut w.dock, &ctx.policy)
                    });
                }
            }
            LayoutCmd::OpenPanel {
                panel,
                at: DockTarget::Window(target),
                ..
            } => {
                self.edit(target, fx, |w, _, _| {
                    dock::open_panel(w, panel, DockTarget::Root)
                });
            }
            LayoutCmd::RequestBlob { win, ticket } => {
                if let Some(w) = self.windows.get(&win) {
                    let wire = blob::WireDock::capture(&w.dock);
                    fx.push(LayoutEffect::Intent(match blob::encode(&wire, None) {
                        Ok(blob) => DockIntent::LayoutBlob { win, ticket, blob },
                        Err(error) => DockIntent::LayoutBlobFailed { win, ticket, error },
                    }));
                }
            }
            LayoutCmd::Restore { win, blob } => {
                if !self.windows.contains_key(&win) {
                    return;
                }
                let mut outcome = None;
                self.edit(win, fx, |w, ctx, fx| {
                    let held = w.session.is_some();
                    let result = blob::restore(w, &blob, &ctx);
                    if let Ok(geometry) = &result {
                        if held {
                            fx.push(LayoutEffect::Capture { win, hold: false });
                        }
                        restore_geometry(win, *geometry, fx);
                    }
                    outcome = Some(result);
                });
                fx.push(LayoutEffect::Intent(match outcome {
                    Some(Err(error)) => DockIntent::LayoutRestoreFailed { win, error },
                    _ => DockIntent::LayoutRestored { win },
                }));
            }
            other => {
                let Some(win) = cmd_window(&other) else {
                    return;
                };
                self.edit(win, fx, |w, _, _| window_cmd(w, other));
            }
        }
    }
}

/// Window geometry stored in a restored blob becomes host commands.
fn restore_geometry(
    win: WindowId,
    geometry: Option<WindowGeometrySnapshot>,
    fx: &mut LayoutEffects,
) {
    let Some(g) = geometry else {
        return;
    };
    if g.maximized {
        fx.push(LayoutEffect::Window {
            win,
            cmd: WindowCommand::SetMaximized(true),
        });
    } else if let Some(position) = g.position {
        fx.push(LayoutEffect::Window {
            win,
            cmd: WindowCommand::SetOuterRect {
                position,
                size: g.outer_size,
            },
        });
    }
}

/// The window a per-window layout command targets.
fn cmd_window<P>(cmd: &LayoutCmd<P>) -> Option<WindowId> {
    match cmd {
        LayoutCmd::SetChrome { win, .. }
        | LayoutCmd::SetChromeModel { win, .. }
        | LayoutCmd::AddEdgeSlot { win, .. }
        | LayoutCmd::RemoveEdgeSlot { win, .. }
        | LayoutCmd::OpenPanel { win, .. }
        | LayoutCmd::ClosePanel { win, .. }
        | LayoutCmd::ActivateTab { win, .. }
        | LayoutCmd::Split { win, .. }
        | LayoutCmd::Float { win, .. }
        | LayoutCmd::SetPreset { win, .. }
        | LayoutCmd::SetRatios { win, .. }
        | LayoutCmd::SetGrid { win, .. }
        | LayoutCmd::RequestBlob { win, .. }
        | LayoutCmd::Restore { win, .. } => Some(*win),
        LayoutCmd::SetPolicy(_) => None,
    }
}

/// Apply a per-window command (chrome, edges, dock structure).
fn window_cmd<P: DockPanel>(w: &mut WindowLayout<P>, cmd: LayoutCmd<P>) {
    match cmd {
        LayoutCmd::SetChrome {
            visible, height, ..
        } => {
            w.chrome_slot.visible = visible;
            w.chrome_slot.height = if height.is_finite() {
                height.max(0.0)
            } else {
                0.0
            };
        }
        LayoutCmd::SetChromeModel { model, .. } => w.chrome = model,
        LayoutCmd::AddEdgeSlot { slot, .. } => w.edges.add(slot),
        LayoutCmd::RemoveEdgeSlot { id, .. } => w.edges.remove(&id),
        LayoutCmd::OpenPanel { panel, at, .. } => dock::open_panel(w, panel, at),
        LayoutCmd::ClosePanel { leaf, index, .. } => {
            w.dock.tree_mut().remove_tab(leaf, index);
        }
        LayoutCmd::ActivateTab { leaf, index, .. } => {
            dock::activate_tab(&mut w.dock, leaf, index);
        }
        LayoutCmd::Split {
            leaf, dir, panel, ..
        } => dock::split(&mut w.dock, leaf, dir, panel),
        LayoutCmd::Float {
            leaf, index, rect, ..
        } => dock::float_tab(w, leaf, index, rect),
        LayoutCmd::SetPreset { preset, .. } => w.dock.tree_mut().set_layout(preset),
        LayoutCmd::SetRatios { branch, ratios, .. } => {
            dock::set_ratios(&mut w.dock, branch, ratios)
        }
        LayoutCmd::SetGrid {
            branch, rows, cols, ..
        } => {
            w.dock.tree_mut().set_branch_grid(branch, rows, cols);
        }
        // Engine-level commands are handled by `LayoutEngine::cmd`.
        LayoutCmd::SetPolicy(_) | LayoutCmd::RequestBlob { .. } | LayoutCmd::Restore { .. } => {}
    }
}

/// One pointer event for one window. A release that drags a panel out of
/// the viewport leaves the panel in `out` for the engine-level drag-out.
fn pointer<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    ctx: Ctx<P>,
    event: DockPointer,
    fx: &mut LayoutEffects,
    out: &mut Option<dragout::Outbound<P>>,
) {
    match event {
        DockPointer::Down { pos, button } => {
            if button != uzor::input::MouseButton::Left {
                return;
            }
            if w.session.is_some() {
                // A press while a session is live (a lost release): end the
                // old one first, as a cancel.
                end_session(win, w, fx, None, ctx, out);
            }
            let hit = hit_window(w, &ctx.policy, pos);
            press(win, w, hit, pos, fx);
        }
        DockPointer::Move(pos) => {
            let Some(session) = w.session else {
                return;
            };
            let next = match session {
                Session::Splitter(d) => Session::Splitter(splitter::drag(w, d, pos, &ctx.policy)),
                Session::Tab { .. } | Session::Header { .. } => {
                    let next = dock::drag_move(w, session, pos);
                    if dock::is_live_drag(&next) {
                        expand::track(win, w, &ctx.policy, pos, fx);
                    }
                    next
                }
                Session::FloatingMove { .. } | Session::FloatingResize { .. } => {
                    dock::floating_move(w, session, pos);
                    session
                }
                Session::Click(_) => session,
                Session::MicroMove { grab } => {
                    dragout::micro_move(win, w, grab, pos, fx);
                    session
                }
            };
            w.session = Some(next);
        }
        DockPointer::Up(pos) => {
            if w.session.is_some() {
                end_session(win, w, fx, Some(pos), ctx, out);
            }
        }
        DockPointer::Cancel => {
            if w.session.is_some() {
                end_session(win, w, fx, None, ctx, out);
            }
        }
    }
}

/// Start what a primary press on `hit` implies.
fn press<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    hit: LayoutHit,
    pos: Point,
    fx: &mut LayoutEffects,
) {
    let session = match hit {
        LayoutHit::Bezel(dir) => {
            fx.push(LayoutEffect::Window {
                win,
                cmd: WindowCommand::DragResizeWindow(dir),
            });
            None
        }
        LayoutHit::Chrome(part) => chrome::press(win, part, fx),
        LayoutHit::Splitter { sep, .. } => {
            splitter::start_line(&w.dock, sep, pos).map(Session::Splitter)
        }
        LayoutHit::Corner {
            vertical,
            horizontal,
        } => splitter::start_corner(&w.dock, vertical, horizontal, pos).map(Session::Splitter),
        LayoutHit::PanelHeader(_) if w.micro == Some(DragOutChrome::None) => {
            // A chrome-less micro-window: its panel header moves the OS
            // window (previous kernel `drag_out.rs:285-337`).
            Some(Session::MicroMove { grab: pos })
        }
        LayoutHit::Tab { leaf, tab } => {
            if dock::activate_tab(&mut w.dock, leaf, tab) {
                fx.push(LayoutEffect::Intent(DockIntent::TabActivated {
                    win,
                    leaf,
                    index: tab,
                }));
            }
            // Nothing tears off a micro-window (previous kernel
            // `drag_out.rs:299`: infinite tear threshold there).
            let stage = if w.micro.is_some() {
                dock::TabStage::Refused
            } else {
                dock::TabStage::Pressed
            };
            Some(Session::Tab {
                leaf,
                tab,
                origin: pos,
                stage,
            })
        }
        LayoutHit::PanelHeader(leaf) => {
            w.dock.set_active_leaf(leaf);
            let stage = if w.micro.is_some() {
                dock::HeaderStage::Refused
            } else {
                dock::HeaderStage::Pressed
            };
            Some(Session::Header {
                leaf,
                origin: pos,
                stage,
            })
        }
        LayoutHit::FloatingHeader(id) => dock::floating_press(&mut w.dock, id, pos),
        LayoutHit::FloatingResize { id, dir } => dock::floating_resize_press(&w.dock, id, dir, pos),
        LayoutHit::TabClose { .. } | LayoutHit::TabNew { .. } | LayoutHit::FloatingClose(_) => {
            Some(Session::Click(hit))
        }
        LayoutHit::PanelBody(_)
        | LayoutHit::FloatingBody(_)
        | LayoutHit::EdgeGutter(_)
        | LayoutHit::None => None,
    };
    if let Some(s) = session {
        w.session = Some(s);
        fx.push(LayoutEffect::Capture { win, hold: true });
    }
}

/// End the window's session: `release = Some(pos)` completes it (drop,
/// click), `None` cancels it. Always releases the capture.
fn end_session<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    fx: &mut LayoutEffects,
    release: Option<Point>,
    ctx: Ctx<P>,
    out: &mut Option<dragout::Outbound<P>>,
) {
    let Some(session) = w.session.take() else {
        return;
    };
    match (session, release) {
        (Session::Splitter(_), _) => {}
        (Session::Tab { .. } | Session::Header { .. }, Some(pos)) => {
            if dock::is_live_drag(&session) {
                // The release point decides, not the last move.
                expand::track(win, w, &ctx.policy, pos, fx);
            }
            if !expand::commit_drop(win, w, session, fx) {
                dock::drag_release(win, w, session, pos, ctx, fx, out);
            }
            expand::unhot(win, w, fx);
        }
        (Session::Tab { .. } | Session::Header { .. }, None) => {
            dock::drag_cancel(w, session);
            expand::unhot(win, w, fx);
        }
        (Session::FloatingMove { .. }, _) => {
            w.dock.end_floating_drag();
        }
        (Session::FloatingResize { .. }, _) | (Session::MicroMove { .. }, _) => {}
        (Session::Click(hit), Some(pos)) => {
            if hit_window(w, &ctx.policy, pos) == hit {
                click(win, w, hit, fx);
            }
        }
        (Session::Click(_), None) => {}
    }
    fx.push(LayoutEffect::Capture { win, hold: false });
}

/// A completed click on a click-type part.
fn click<P: DockPanel>(
    win: WindowId,
    w: &mut WindowLayout<P>,
    hit: LayoutHit,
    fx: &mut LayoutEffects,
) {
    match hit {
        LayoutHit::TabClose { leaf, tab } => {
            if let Some(type_id) = dock::close_tab(&mut w.dock, leaf, tab) {
                fx.push(LayoutEffect::Intent(DockIntent::PanelClosed {
                    win,
                    leaf,
                    index: tab,
                    type_id,
                }));
            }
        }
        LayoutHit::TabNew { leaf } => {
            fx.push(LayoutEffect::Intent(DockIntent::NewPanelRequested {
                win,
                leaf,
            }));
        }
        LayoutHit::FloatingClose(id) => w.dock.close_floating(id),
        LayoutHit::Chrome(part) => chrome::click(win, part, fx),
        _ => {}
    }
}

/// The hit walk (module docs, "Hit precedence").
fn hit_window<P: DockPanel>(w: &WindowLayout<P>, policy: &LayoutPolicy, p: Point) -> LayoutHit {
    if !rects::contains(w.viewport, p) {
        return LayoutHit::None;
    }
    if let Some(side) = expand::gutter(w, policy, p, expand::drag_live(w)) {
        return LayoutHit::EdgeGutter(side);
    }
    if !w.caps.os_resize_bezel {
        if let Some(dir) = chrome::bezel(w.viewport, policy.bezel_px, p) {
            return LayoutHit::Bezel(dir);
        }
    }
    if let Some(hit) = chrome::hit(w, p) {
        return hit;
    }
    if rects::in_edge_slot(&w.solved, p) {
        return LayoutHit::None;
    }
    if let Some(hit) = dock::floating_hit(&w.dock, policy, p) {
        return hit;
    }
    dock::dock_hit(&w.dock, policy, p)
}

fn invalidate(win: WindowId, bits: InvalidateBits) -> LayoutEffect {
    LayoutEffect::Invalidate { win, bits }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

/// Read-only projection of the LayoutEngine.
pub struct LayoutEngineView<'a, P: DockPanel> {
    engine: &'a LayoutEngine<P>,
}

impl<P: DockPanel> Clone for LayoutEngineView<'_, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: DockPanel> Copy for LayoutEngineView<'_, P> {}

impl<'a, P: DockPanel> LayoutEngineView<'a, P> {
    /// Windows the engine tracks, in id order.
    pub fn ids(&self) -> impl Iterator<Item = WindowId> + 'a {
        self.engine.windows.keys().copied()
    }

    /// One window's layout.
    pub fn window(&self, win: WindowId) -> Option<WindowLayoutView<'a, P>> {
        self.engine
            .windows
            .get(&win)
            .map(|w| WindowLayoutView { w })
    }

    /// The layout policy.
    pub fn policy(&self) -> LayoutPolicy {
        self.engine.policy
    }

    /// Same as [`LayoutEngine::hit`].
    pub fn hit(&self, win: WindowId, p: Point) -> LayoutHit {
        self.engine.hit(win, p)
    }

    /// The engine revision.
    pub fn revision(&self) -> Revision {
        self.engine.rev
    }

    /// The live drag-out session, if any.
    pub fn drag_out(&self) -> Option<DragOutView> {
        self.engine.dragout_obs()
    }

    /// When the engine needs its next [`LayoutEngine::tick`] without any
    /// input: the instant a running dwell completes. Snap-back springs are
    /// reported by [`WindowLayoutView::snap_backs`] (they want every frame).
    pub fn next_deadline(&self) -> Option<Seconds> {
        self.engine.dragout.as_ref().and_then(|s| s.deadline())
    }
}

/// Read-only view of one window's layout.
pub struct WindowLayoutView<'a, P: DockPanel> {
    w: &'a WindowLayout<P>,
}

impl<P: DockPanel> Clone for WindowLayoutView<'_, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: DockPanel> Copy for WindowLayoutView<'_, P> {}

impl<'a, P: DockPanel> WindowLayoutView<'a, P> {
    /// The logical viewport.
    pub fn viewport(&self) -> Rect {
        self.w.viewport
    }

    /// Host capabilities of the window.
    pub fn caps(&self) -> HostCaps {
        self.w.caps
    }

    /// The solved macro layout (chrome, edge slots, dock area); the only
    /// source of these rects for compose and hit-testing.
    pub fn solved(&self) -> &'a LayoutSolved {
        &self.w.solved
    }

    /// The dock area (viewport minus chrome and compressing edge slots).
    pub fn dock_area(&self) -> Rect {
        self.w.solved.dock_area
    }

    /// The chrome slot (visibility, height).
    pub fn chrome_slot(&self) -> &'a ChromeSlot {
        &self.w.chrome_slot
    }

    /// What the chrome strip contains.
    pub fn chrome_model(&self) -> &'a ChromeModel {
        &self.w.chrome
    }

    /// The chrome composite's draw state (hover, tab widths); the compose
    /// phase reads it to paint the same strip the hit-test classifies.
    pub fn chrome_state(&self) -> &'a ChromeState {
        &self.w.chrome_state
    }

    /// The dock (tree, solved leaf / header / tab bar / separator rects,
    /// floating windows, drag and snap-back state), read-only.
    pub fn dock(&self) -> &'a DockState<P> {
        &self.w.dock
    }

    /// The dock revision (bumps when a stored layout blob goes stale).
    pub fn dock_rev(&self) -> Revision {
        self.w.dock_rev
    }

    /// The live pointer session, if any.
    pub fn session(&self) -> Option<SessionKind> {
        self.w.session.map(|s| s.kind())
    }

    /// The outer rect (physical screen px) from the last host echo.
    pub fn outer_rect(&self) -> Option<Rect> {
        self.w.outer
    }

    /// The device pixel ratio from the last outer-rect echo (`1.0` before).
    pub fn scale(&self) -> f64 {
        self.w.scale
    }

    /// The latched expand, if any (drives the gutter preview in compose).
    pub fn expand(&self) -> Option<ExpandState> {
        self.w.expand.map(|x| x.state())
    }

    /// The rect the window's content is laid out in: the viewport, or while
    /// an expand is latched the viewport at latch time shifted by the
    /// expand inset (so the content keeps its place while the window grows).
    pub fn content_rect(&self) -> Rect {
        expand::content_rect(self.w)
    }

    /// `Some(chrome)` when this window is a drag-out micro-window.
    pub fn micro(&self) -> Option<DragOutChrome> {
        self.w.micro
    }

    /// Running separator snap-back springs.
    pub fn snap_backs(&self) -> &'a [SnapBackAnimation] {
        self.w.dock.snap_animations()
    }

    /// The "+" button rect of a leaf, when the policy shows it.
    pub fn tab_new_rect(&self, leaf: LeafId, policy: &LayoutPolicy) -> Option<Rect> {
        dock::tab_new_rect(&self.w.dock, policy, leaf)
    }

    /// The published chrome view.
    pub fn chrome_view(&self) -> ChromeView {
        ChromeView {
            visible: self.w.solved.chrome.is_some(),
            rect: self.w.solved.chrome.unwrap_or_default(),
        }
    }

    /// The published edge-slot views, in slot insertion order.
    pub fn edge_views(&self) -> Vec<EdgeSlotView> {
        rects::edge_views(self.w)
    }

    /// The published dock view (leaves in `LeafId` order).
    pub fn dock_view(&self) -> DockView {
        dock::dock_view(&self.w.dock, self.w.dock_rev)
    }

    /// Which edge an [`EdgeSide`] slot list belongs to (helper for compose).
    pub fn edge_rects(&self, side: EdgeSide) -> &'a [Rect] {
        let e = &self.w.solved.edges;
        match side {
            EdgeSide::Top => &e.top,
            EdgeSide::Bottom => &e.bottom,
            EdgeSide::Left => &e.left,
            EdgeSide::Right => &e.right,
        }
    }
}

#[cfg(test)]
mod tests;
