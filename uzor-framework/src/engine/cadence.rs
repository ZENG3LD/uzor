//! CadenceEngine: invalidation, regions, tick rates and the deadline wheel
//! (design §3.6).
//!
//! The one writer of "what must be painted, and when must the loop wake".
//! Hosts receive [`FrameRequest`]s and a [`Wake`] and never compute "due"
//! themselves.
//!
//! ## Scheduling rules
//!
//! Each window has a base [`TickRate`] and optional app-declared regions.
//! Without regions the window is one implicit region paced by its tick rate;
//! with regions each region is paced by its own `target_fps` (the window tick
//! rate then only describes the window in the snapshot). Per region, "due"
//! is the library's `RegionScheduleState::due` rule ported from `Instant` to
//! [`Seconds`]:
//!
//! | `target_fps` | due when |
//! |---|---|
//! | `0` (dirty-driven) | an invalidation is pending |
//! | `UNCAPPED_FPS` | always |
//! | `n` (capped) | never painted, or `1/n` s since its last paint (a pending invalidation waits for the slot) |
//!
//! One addition: a window-wide invalidation (resize, DPI, theme swap — region
//! `None`) or a change of the region set forces every region of the window
//! into the next frame, so a capped region never shows stale content on a
//! changed surface.
//!
//! [`CadenceEngine::frames`] turns due regions into one [`FrameRequest`] per
//! window (regions ascending by declaration order, invalidations mapped to
//! `RetainedScope::Region(id)`), and clears what it painted.
//! [`CadenceEngine::next_wake`] then says when the loop must run again:
//! `Immediate` while an animator runs or something is due now, `At(t)` for the
//! earliest capped region or armed deadline, `Idle` otherwise.
//!
//! ## Deadline wheel
//!
//! [`DeadlineWheel`] is a timer and nothing else: "wake at T, report the
//! token". One deadline per [`TimerOwner`]; re-arming an owner replaces its
//! deadline under a fresh token. Deadlines fire in time order, ties in arm
//! order. Keys are [`OrderedSeconds`] (a total-order `f64` newtype), so a NaN
//! instant is refused instead of corrupting the order.
//!
//! ## Revision
//!
//! Bumped when observable state changes: tick rates, regions, pending
//! invalidations, the set of armed deadlines, the set of windows. The
//! bookkeeping of last paint instants is not revisioned (it never appears in
//! the view).

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use smallvec::SmallVec;
use uzor::render::{InvalidateBits, RetainedScope, TickRate, UNCAPPED_FPS};
use uzor::Rect;

use crate::types::command::CadenceCmd;
use crate::types::frame::{FrameRequest, RegionPlan, RegionSpec, TimerOwner, Wake, WindowSurface};
use crate::types::ids::{OrderedSeconds, RegionId, Revision, Seconds, TimerToken, WindowId};
use crate::types::ops::{CadenceEffect, CadenceOp};
use crate::types::snapshot::CadenceView;

/// Effects of one CadenceEngine op or tick.
pub type CadenceEffects = SmallVec<[CadenceEffect; 2]>;

/// Deadlines fired by one [`DeadlineWheel::drain_due`], in firing order.
pub type Fired = SmallVec<[(TimerToken, TimerOwner); 4]>;

// ---------------------------------------------------------------------------
// DeadlineWheel
// ---------------------------------------------------------------------------

/// The window a timer owner is scoped to, if any.
fn owner_window(owner: TimerOwner) -> Option<WindowId> {
    match owner {
        TimerOwner::App(_) => None,
        TimerOwner::OverlayAutoClose { win, .. }
        | TimerOwner::Tooltip { win }
        | TimerOwner::CaretBlink { win } => Some(win),
    }
}

/// One-shot deadlines keyed by owner: arm, disarm, drain what is due.
///
/// Entries are kept in a map sorted by `(instant, arm sequence)` plus an
/// owner index, so re-arming and disarming are `O(log n)` and ties fire in
/// arm order. The arm sequence number is also the [`TimerToken`], so tokens
/// are unique within a run.
#[derive(Clone, Debug, Default)]
pub struct DeadlineWheel {
    by_time: BTreeMap<(OrderedSeconds, u64), TimerOwner>,
    by_owner: BTreeMap<TimerOwner, (OrderedSeconds, u64)>,
    next_token: u64,
}

// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
impl DeadlineWheel {
    /// An empty wheel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Arm `owner` to fire at `at`, replacing its previous deadline if any.
    /// Returns the new token, or `None` (and changes nothing) when `at` is
    /// NaN.
    pub fn arm(&mut self, owner: TimerOwner, at: Seconds) -> Option<TimerToken> {
        let key = OrderedSeconds::new(at)?;
        self.disarm(owner);
        let seq = self.next_token;
        self.next_token = self.next_token.saturating_add(1);
        self.by_time.insert((key, seq), owner);
        self.by_owner.insert(owner, (key, seq));
        Some(TimerToken(seq))
    }

    /// Disarm `owner`'s deadline; `true` if one was armed.
    pub fn disarm(&mut self, owner: TimerOwner) -> bool {
        match self.by_owner.remove(&owner) {
            Some(k) => {
                self.by_time.remove(&k);
                true
            }
            None => false,
        }
    }

    /// Disarm every deadline scoped to `win` (overlay auto-close, tooltip);
    /// returns how many were removed. App timers are not window-scoped.
    pub fn disarm_window(&mut self, win: WindowId) -> usize {
        let owners: Vec<TimerOwner> = self
            .by_owner
            .keys()
            .copied()
            .filter(|o| owner_window(*o) == Some(win))
            .collect();
        for o in &owners {
            self.disarm(*o);
        }
        owners.len()
    }

    /// Remove and return every deadline with `at <= now`, earliest first,
    /// ties in arm order. A NaN `now` fires nothing.
    pub fn drain_due(&mut self, now: Seconds) -> Fired {
        let mut out = Fired::new();
        let Some(now) = OrderedSeconds::new(now) else {
            return out;
        };
        while let Some((&(at, _), _)) = self.by_time.first_key_value() {
            if at > now {
                break;
            }
            if let Some(((_, seq), owner)) = self.by_time.pop_first() {
                self.by_owner.remove(&owner);
                out.push((TimerToken(seq), owner));
            }
        }
        out
    }

    /// The earliest armed instant (feeds `Wake::At`).
    pub fn earliest(&self) -> Option<Seconds> {
        self.by_time.keys().next().map(|(at, _)| at.seconds())
    }

    /// `owner`'s armed instant and token, if armed.
    pub fn deadline(&self, owner: TimerOwner) -> Option<(Seconds, TimerToken)> {
        self.by_owner
            .get(&owner)
            .map(|(at, seq)| (at.seconds(), TimerToken(*seq)))
    }

    /// Number of armed deadlines.
    pub fn len(&self) -> usize {
        self.by_time.len()
    }

    /// No deadline armed.
    pub fn is_empty(&self) -> bool {
        self.by_time.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Scheduling helpers (lib RegionScheduleState::{due, next_due} on Seconds)
// ---------------------------------------------------------------------------

fn frame_interval(fps: u32) -> f64 {
    1.0 / f64::from(fps)
}

fn due(fps: u32, dirty: bool, last: Option<Seconds>, now: Seconds) -> bool {
    match fps {
        0 => dirty,
        UNCAPPED_FPS => true,
        fps => match last {
            None => true,
            Some(t) => now.since(t).get() >= frame_interval(fps),
        },
    }
}

fn next_due(fps: u32, dirty: bool, last: Option<Seconds>, now: Seconds) -> Option<Seconds> {
    match fps {
        0 => dirty.then_some(now),
        UNCAPPED_FPS => Some(now),
        fps => match last {
            None => Some(now),
            Some(t) => Some(t.after(Seconds(frame_interval(fps)))),
        },
    }
}

fn earlier(a: Option<Seconds>, b: Option<Seconds>) -> Option<Seconds> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if y.get() < x.get() { y } else { x }),
        (x, None) => x,
        (None, y) => y,
    }
}

// ---------------------------------------------------------------------------
// CadenceEngine
// ---------------------------------------------------------------------------

/// One declared region: the app's spec (its `dirty` flag is the pending
/// flag), pending retained-cache bits, last paint instant.
#[derive(Clone, Debug, PartialEq)]
struct RegionSlot {
    spec: RegionSpec,
    bits: InvalidateBits,
    last: Option<Seconds>,
}

impl RegionSlot {
    fn fresh(spec: RegionSpec) -> Self {
        Self {
            spec: RegionSpec {
                dirty: true,
                ..spec
            },
            bits: InvalidateBits::ALL,
            last: None,
        }
    }

    /// Mark pending with `bits`; `true` if that changed anything.
    fn mark(&mut self, bits: InvalidateBits) -> bool {
        let merged = self.bits.union(bits);
        let changed = !self.spec.dirty || merged != self.bits;
        self.spec.dirty = true;
        self.bits = merged;
        changed
    }
}

#[derive(Clone, Debug)]
struct WindowCadence {
    tick: TickRate,
    regions: Vec<RegionSlot>,
    /// Pending invalidation of the implicit whole-window region (used only
    /// while no region is declared).
    whole: Option<InvalidateBits>,
    whole_last: Option<Seconds>,
    /// Every declared region goes into the next frame.
    force_all: bool,
}

impl WindowCadence {
    fn new(tick: TickRate) -> Self {
        Self {
            tick,
            regions: Vec::new(),
            whole: Some(InvalidateBits::ALL),
            whole_last: None,
            force_all: false,
        }
    }

    // Read doors for the native/web host briefs (F8+) - unused in-crate until then.
    #[allow(dead_code)]
    fn dirty(&self) -> bool {
        self.whole.is_some() || self.force_all || self.regions.iter().any(|r| r.spec.dirty)
    }

    fn invalidate(&mut self, region: Option<RegionId>, bits: InvalidateBits) -> bool {
        match region {
            None if self.regions.is_empty() => {
                let merged = self.whole.unwrap_or(InvalidateBits::NONE).union(bits);
                let changed = self.whole != Some(merged);
                self.whole = Some(merged);
                changed
            }
            None => {
                let mut changed = !self.force_all;
                self.force_all = true;
                for slot in &mut self.regions {
                    changed |= slot.mark(bits);
                }
                changed
            }
            Some(id) => self
                .regions
                .iter_mut()
                .find(|s| s.spec.id == id)
                .is_some_and(|slot| slot.mark(bits)),
        }
    }

    fn set_regions(&mut self, specs: Vec<RegionSpec>) -> bool {
        let old = std::mem::take(&mut self.regions);
        let carried = self.whole.take();
        let mut next = Vec::with_capacity(specs.len());
        for spec in specs {
            let slot = match old
                .iter()
                .find(|s| s.spec.id == spec.id && s.spec.rect == spec.rect)
            {
                Some(prev) => RegionSlot {
                    spec: RegionSpec {
                        dirty: prev.spec.dirty || spec.dirty,
                        ..spec
                    },
                    bits: prev.bits,
                    last: prev.last,
                },
                None => RegionSlot::fresh(spec),
            };
            next.push(slot);
        }
        let changed = next != old;
        self.regions = next;
        if self.regions.is_empty() {
            // Back to the implicit whole-window region.
            self.whole = if changed {
                Some(InvalidateBits::ALL)
            } else {
                carried
            };
        } else {
            if let Some(bits) = carried {
                for slot in &mut self.regions {
                    slot.mark(bits);
                }
            }
            if changed {
                self.force_all = true;
            }
        }
        // `carried` is only ever set while no region was declared, so moving
        // it into new slots always comes with a changed region set.
        changed
    }
}

/// Cross-engine reads the CadenceEngine needs to build frames and the wake:
/// window surfaces (from the WindowEngine's view), the clear colour (from
/// the theme) and whether any animator runs (from the AnimationEngine and
/// the layout snap-back).
#[derive(Clone, Copy, Debug)]
pub struct CadenceTickView<'a> {
    surfaces: &'a [WindowSurface],
    background: u32,
    animating: bool,
}

impl<'a> CadenceTickView<'a> {
    /// Assemble the view.
    pub fn new(surfaces: &'a [WindowSurface], background: u32, animating: bool) -> Self {
        Self {
            surfaces,
            background,
            animating,
        }
    }

    fn surface(&self, win: WindowId) -> Option<&'a WindowSurface> {
        self.surfaces.iter().find(|s| s.win == win)
    }
}

/// The CadenceEngine: see the module docs.
#[derive(Clone, Debug, Default)]
pub struct CadenceEngine {
    windows: BTreeMap<WindowId, WindowCadence>,
    wheel: DeadlineWheel,
    rev: Revision,
}

impl CadenceEngine {
    /// An engine with no windows and no deadlines.
    pub fn new() -> Self {
        Self::default()
    }

    /// The only mutation door. Bumps the revision iff state changed.
    pub fn apply(&mut self, op: CadenceOp) -> CadenceEffects {
        let mut fx = CadenceEffects::new();
        let changed = match op {
            CadenceOp::Open { win, tick } => match self.windows.entry(win) {
                Entry::Occupied(_) => false,
                Entry::Vacant(slot) => {
                    slot.insert(WindowCadence::new(tick));
                    true
                }
            },
            CadenceOp::Close(win) => {
                let removed = self.windows.remove(&win).is_some();
                let disarmed = self.wheel.disarm_window(win) > 0;
                removed || disarmed
            }
            CadenceOp::Cmd(cmd) => self.cmd(cmd, &mut fx),
            CadenceOp::Invalidate { win, region, bits } => self.invalidate(win, region, bits),
            CadenceOp::Arm { owner, at } => self.arm(owner, at, &mut fx),
            CadenceOp::Disarm(owner) => self.wheel.disarm(owner),
        };
        if changed {
            self.rev.bump();
        }
        fx
    }

    /// Phase 4a: fire every deadline due at `now`, earliest first.
    pub fn tick(&mut self, now: Seconds) -> CadenceEffects {
        let fired = self.wheel.drain_due(now);
        if !fired.is_empty() {
            self.rev.bump();
        }
        fired
            .into_iter()
            .map(|(token, owner)| CadenceEffect::Fired { token, owner })
            .collect()
    }

    /// Phase 7: one [`FrameRequest`] per window with a surface and at least
    /// one due region, ascending by window id. Painted regions are marked
    /// clean and stamped with `now`.
    pub fn frames(&mut self, now: Seconds, view: &CadenceTickView<'_>) -> Vec<FrameRequest> {
        let mut out = Vec::new();
        let mut changed = false;
        for (win, wc) in &mut self.windows {
            let Some(surface) = view.surface(*win) else {
                continue;
            };
            let mut req = FrameRequest {
                window: *win,
                size: surface.size,
                dpr: surface.dpr,
                background: view.background,
                regions: SmallVec::new(),
                invalidations: SmallVec::new(),
            };
            if wc.regions.is_empty() {
                if !due(wc.tick.target_fps(), wc.whole.is_some(), wc.whole_last, now) {
                    continue;
                }
                changed |= wc.whole.take().is_some();
                wc.whole_last = Some(now);
            } else {
                let force = wc.force_all;
                for slot in &mut wc.regions {
                    if !(force || due(slot.spec.target_fps, slot.spec.dirty, slot.last, now)) {
                        continue;
                    }
                    req.regions.push(RegionPlan {
                        id: Some(slot.spec.id),
                        rect: slot.spec.rect,
                    });
                    if slot.bits != InvalidateBits::NONE {
                        req.invalidations
                            .push((RetainedScope::Region(slot.spec.id.0), slot.bits));
                    }
                    changed |= slot.spec.dirty;
                    slot.spec.dirty = false;
                    slot.bits = InvalidateBits::NONE;
                    slot.last = Some(now);
                }
                changed |= force;
                wc.force_all = false;
                if req.regions.is_empty() {
                    continue;
                }
            }
            out.push(req);
        }
        if changed {
            self.rev.bump();
        }
        out
    }

    /// When the loop must run next, given the state after [`Self::frames`].
    /// Windows without a surface do not keep the loop awake.
    pub fn next_wake(&self, now: Seconds, view: &CadenceTickView<'_>) -> Wake {
        if view.animating {
            return Wake::Immediate;
        }
        let mut at: Option<Seconds> = None;
        for (win, wc) in &self.windows {
            if view.surface(*win).is_none() {
                continue;
            }
            if wc.regions.is_empty() {
                at = earlier(
                    at,
                    next_due(wc.tick.target_fps(), wc.whole.is_some(), wc.whole_last, now),
                );
            } else {
                for slot in &wc.regions {
                    let n = if wc.force_all {
                        Some(now)
                    } else {
                        next_due(slot.spec.target_fps, slot.spec.dirty, slot.last, now)
                    };
                    at = earlier(at, n);
                }
            }
        }
        at = earlier(at, self.wheel.earliest().filter(|t| t.get().is_finite()));
        match at {
            None => Wake::Idle,
            Some(t) if t.get() <= now.get() => Wake::Immediate,
            Some(t) => Wake::At(t),
        }
    }

    /// Bumped on every observable state change.
    pub fn revision(&self) -> Revision {
        self.rev
    }

    /// Read-only projection.
    pub fn view(&self) -> CadenceEngineView<'_> {
        CadenceEngineView { e: self }
    }

    // -- ops ----------------------------------------------------------------

    fn cmd(&mut self, cmd: CadenceCmd, fx: &mut CadenceEffects) -> bool {
        match cmd {
            CadenceCmd::SetTickRate { win, rate } => match self.windows.get_mut(&win) {
                Some(wc) if wc.tick != rate => {
                    wc.tick = rate;
                    true
                }
                _ => false,
            },
            CadenceCmd::SetRegions { win, regions } => self
                .windows
                .get_mut(&win)
                .is_some_and(|wc| wc.set_regions(regions)),
            CadenceCmd::Invalidate { win, region, bits } => self.invalidate(win, region, bits),
            CadenceCmd::WakeAt { token, at } => self.arm(TimerOwner::App(token), at, fx),
            CadenceCmd::Cancel { token } => self.wheel.disarm(TimerOwner::App(token)),
        }
    }

    fn invalidate(
        &mut self,
        win: WindowId,
        region: Option<RegionId>,
        bits: InvalidateBits,
    ) -> bool {
        self.windows
            .get_mut(&win)
            .is_some_and(|wc| wc.invalidate(region, bits))
    }

    fn arm(&mut self, owner: TimerOwner, at: Seconds, fx: &mut CadenceEffects) -> bool {
        match self.wheel.arm(owner, at) {
            Some(token) => {
                fx.push(CadenceEffect::Armed { owner, token });
                true
            }
            None => false,
        }
    }
}

/// Read-only projection of the [`CadenceEngine`].
#[derive(Clone, Copy, Debug)]
pub struct CadenceEngineView<'a> {
    e: &'a CadenceEngine,
}

// Read doors for the native/web host briefs (F8+) - unused in-crate until then.
#[allow(dead_code)]
impl<'a> CadenceEngineView<'a> {
    /// Scheduled windows, ascending.
    pub fn ids(&self) -> impl Iterator<Item = WindowId> + 'a {
        self.e.windows.keys().copied()
    }

    /// The published cadence of one window: tick rate and regions, each
    /// region's `dirty` flag meaning "goes into the next frame".
    pub fn window(&self, win: WindowId) -> Option<CadenceView> {
        self.e.windows.get(&win).map(|wc| CadenceView {
            tick: wc.tick,
            regions: wc
                .regions
                .iter()
                .map(|s| RegionSpec {
                    dirty: s.spec.dirty || wc.force_all,
                    ..s.spec
                })
                .collect(),
        })
    }

    /// Base tick rate of one window.
    pub fn tick_rate(&self, win: WindowId) -> Option<TickRate> {
        self.e.windows.get(&win).map(|wc| wc.tick)
    }

    /// The declared region of `win` that contains the centre of `rect`
    /// (the last declared one when regions overlap), or `None` when no
    /// region does or the window declares none. The kernel uses it to turn
    /// a widget rect (e.g. a blinking caret's field) into the narrowest
    /// invalidation the cadence model has; `None` then means "the whole
    /// window".
    pub fn region_containing(&self, win: WindowId, rect: Rect) -> Option<RegionId> {
        let cx = rect.x + rect.width / 2.0;
        let cy = rect.y + rect.height / 2.0;
        self.e
            .windows
            .get(&win)?
            .regions
            .iter()
            .rev()
            .find(|s| s.spec.rect.contains(cx, cy))
            .map(|s| s.spec.id)
    }

    /// Something of the window waits for a repaint.
    pub fn is_dirty(&self, win: WindowId) -> bool {
        self.e.windows.get(&win).is_some_and(WindowCadence::dirty)
    }

    /// The deadline wheel.
    pub fn wheel(&self) -> &'a DeadlineWheel {
        &self.e.wheel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ids::OverlaySlot;
    use crate::types::window::SizePx;
    use uzor::Rect;

    const W1: WindowId = WindowId(1);
    const W2: WindowId = WindowId(2);
    const BG: u32 = 0x1122_33ff;

    fn s(t: f64) -> Seconds {
        Seconds(t)
    }

    fn surface(win: WindowId) -> WindowSurface {
        WindowSurface {
            win,
            size: SizePx::new(800, 600),
            dpr: 1.0,
        }
    }

    fn open(e: &mut CadenceEngine, win: WindowId, tick: TickRate) {
        e.apply(CadenceOp::Open { win, tick });
    }

    fn region(id: u64, fps: u32) -> RegionSpec {
        RegionSpec {
            id: RegionId(id),
            rect: Rect::new(0.0, id as f64 * 10.0, 100.0, 10.0),
            target_fps: fps,
            dirty: false,
        }
    }

    fn region_ids(req: &FrameRequest) -> Vec<Option<u64>> {
        req.regions.iter().map(|r| r.id.map(|i| i.0)).collect()
    }

    // -- wheel --------------------------------------------------------------

    #[test]
    fn wheel_fires_in_time_order_with_stable_ties() {
        let mut w = DeadlineWheel::new();
        let a = w.arm(TimerOwner::App(1), s(3.0));
        let b = w.arm(TimerOwner::App(2), s(1.0));
        let c = w.arm(TimerOwner::App(3), s(2.0));
        let d = w.arm(TimerOwner::Tooltip { win: W1 }, s(2.0));
        assert_eq!(w.len(), 4);
        assert_eq!(w.earliest(), Some(s(1.0)));
        let fired = w.drain_due(s(2.0));
        let got: Vec<(Option<TimerToken>, TimerOwner)> =
            fired.iter().map(|(t, o)| (Some(*t), *o)).collect();
        assert_eq!(
            got,
            vec![
                (b, TimerOwner::App(2)),
                (c, TimerOwner::App(3)),
                (d, TimerOwner::Tooltip { win: W1 }),
            ]
        );
        assert_eq!(w.earliest(), Some(s(3.0)));
        assert!(w.drain_due(s(2.999)).is_empty());
        // `at == now` fires.
        let fired = w.drain_due(s(3.0));
        assert_eq!(
            fired.as_slice(),
            &[(a.unwrap_or(TimerToken(u64::MAX)), TimerOwner::App(1))]
        );
        assert!(w.is_empty());
        assert_eq!(w.earliest(), None);
    }

    #[test]
    fn wheel_rearm_same_owner_replaces() {
        let mut w = DeadlineWheel::new();
        let t1 = w.arm(TimerOwner::App(7), s(5.0));
        let t2 = w.arm(TimerOwner::App(7), s(2.0));
        assert!(t1.is_some() && t2.is_some());
        assert_ne!(t1, t2);
        assert_eq!(w.len(), 1);
        assert_eq!(
            w.deadline(TimerOwner::App(7)).map(|d| (d.0, Some(d.1))),
            Some((s(2.0), t2))
        );
        // Re-arming later also replaces (the earlier instant is gone).
        let t3 = w.arm(TimerOwner::App(7), s(9.0));
        assert!(w.drain_due(s(5.0)).is_empty());
        let fired = w.drain_due(s(9.0));
        assert_eq!(fired.len(), 1);
        assert_eq!(Some(fired[0].0), t3);
    }

    #[test]
    fn wheel_disarm_and_window_scope() {
        let mut w = DeadlineWheel::new();
        let auto = TimerOwner::OverlayAutoClose {
            win: W1,
            slot: OverlaySlot(4),
        };
        w.arm(auto, s(1.0));
        w.arm(TimerOwner::Tooltip { win: W1 }, s(1.0));
        w.arm(TimerOwner::Tooltip { win: W2 }, s(1.0));
        w.arm(TimerOwner::App(1), s(1.0));
        assert!(w.disarm(auto));
        assert!(!w.disarm(auto));
        w.arm(auto, s(1.0));
        assert_eq!(w.disarm_window(W1), 2);
        let owners: Vec<TimerOwner> = w.drain_due(s(1.0)).iter().map(|f| f.1).collect();
        assert_eq!(
            owners,
            vec![TimerOwner::Tooltip { win: W2 }, TimerOwner::App(1)]
        );
    }

    #[test]
    fn wheel_refuses_nan() {
        let mut w = DeadlineWheel::new();
        let t = w.arm(TimerOwner::App(1), s(1.0));
        assert_eq!(w.arm(TimerOwner::App(1), s(f64::NAN)), None);
        // The previous deadline is untouched.
        assert_eq!(w.deadline(TimerOwner::App(1)).map(|d| Some(d.1)), Some(t));
        assert!(w.drain_due(s(f64::NAN)).is_empty());
        assert_eq!(w.len(), 1);
    }

    // -- engine -------------------------------------------------------------

    #[test]
    fn new_window_paints_once_then_idles() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Dirty);
        assert_eq!(e.revision(), Revision(1));
        // Re-opening is a no-op.
        open(&mut e, W1, TickRate::Uncapped);
        assert_eq!(e.revision(), Revision(1));
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        assert_eq!(e.next_wake(s(0.0), &view), Wake::Immediate);
        let frames = e.frames(s(0.0), &view);
        assert_eq!(frames.len(), 1);
        let f = &frames[0];
        assert_eq!(
            (f.window, f.size, f.dpr, f.background),
            (W1, SizePx::new(800, 600), 1.0, BG)
        );
        assert!(f.regions.is_empty() && f.invalidations.is_empty());
        assert_eq!(e.revision(), Revision(2));
        // Clean now: nothing to paint, loop sleeps, no state change.
        assert!(e.frames(s(0.1), &view).is_empty());
        assert_eq!(e.next_wake(s(0.1), &view), Wake::Idle);
        assert_eq!(e.revision(), Revision(2));
        assert!(!e.view().is_dirty(W1));
    }

    #[test]
    fn window_without_surface_waits_without_spinning() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Uncapped);
        let view = CadenceTickView::new(&[], BG, false);
        assert!(e.frames(s(0.0), &view).is_empty());
        assert_eq!(e.next_wake(s(0.0), &view), Wake::Idle);
        assert!(e.view().is_dirty(W1));
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        assert_eq!(e.frames(s(0.0), &view).len(), 1);
    }

    #[test]
    fn tick_rates_drive_frames_and_wake() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Capped(10));
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        assert_eq!(e.frames(s(0.0), &view).len(), 1);
        assert_eq!(e.next_wake(s(0.05), &view), Wake::At(s(0.1)));
        assert!(e.frames(s(0.05), &view).is_empty());
        assert_eq!(e.frames(s(0.1), &view).len(), 1);
        assert_eq!(e.next_wake(s(0.25), &view), Wake::Immediate);
        // Same rate: no change. Uncapped: always due, always Immediate.
        let rev = e.revision();
        e.apply(CadenceOp::Cmd(CadenceCmd::SetTickRate {
            win: W1,
            rate: TickRate::Capped(10),
        }));
        assert_eq!(e.revision(), rev);
        e.apply(CadenceOp::Cmd(CadenceCmd::SetTickRate {
            win: W1,
            rate: TickRate::Uncapped,
        }));
        assert_eq!(e.revision(), rev.next());
        assert_eq!(e.view().tick_rate(W1), Some(TickRate::Uncapped));
        assert_eq!(e.frames(s(0.1), &view).len(), 1);
        assert_eq!(e.frames(s(0.1), &view).len(), 1);
        assert_eq!(e.next_wake(s(0.1), &view), Wake::Immediate);
        // Dirty-driven after paint: idle until invalidated.
        e.apply(CadenceOp::Cmd(CadenceCmd::SetTickRate {
            win: W1,
            rate: TickRate::Dirty,
        }));
        assert_eq!(e.next_wake(s(0.2), &view), Wake::Idle);
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: None,
            bits: InvalidateBits::MATERIAL,
        });
        assert_eq!(e.next_wake(s(0.2), &view), Wake::Immediate);
        assert_eq!(e.frames(s(0.2), &view).len(), 1);
        assert_eq!(e.next_wake(s(0.2), &view), Wake::Idle);
    }

    #[test]
    fn region_invalidations_map_to_retained_scopes() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Dirty);
        e.apply(CadenceOp::Cmd(CadenceCmd::SetRegions {
            win: W1,
            regions: vec![region(1, 0), region(2, 30)],
        }));
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        // New regions paint fully once.
        let frames = e.frames(s(0.0), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(1), Some(2)]);
        assert_eq!(
            frames[0].invalidations.as_slice(),
            &[
                (RetainedScope::Region(1), InvalidateBits::ALL),
                (RetainedScope::Region(2), InvalidateBits::ALL)
            ]
        );
        // Invalidate region 1 twice with different bits: merged.
        let rev = e.revision();
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: Some(RegionId(1)),
            bits: InvalidateBits::MATERIAL,
        });
        e.apply(CadenceOp::Cmd(CadenceCmd::Invalidate {
            win: W1,
            region: Some(RegionId(1)),
            bits: InvalidateBits::GEOMETRY,
        }));
        assert_eq!(e.revision(), Revision(rev.get() + 2));
        // Same bits again: no state change.
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: Some(RegionId(1)),
            bits: InvalidateBits::MATERIAL,
        });
        // Unknown region / window: ignored.
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: Some(RegionId(9)),
            bits: InvalidateBits::ALL,
        });
        e.apply(CadenceOp::Invalidate {
            win: W2,
            region: None,
            bits: InvalidateBits::ALL,
        });
        assert_eq!(e.revision(), Revision(rev.get() + 2));
        let dirty: Vec<bool> = e
            .view()
            .window(W1)
            .map(|v| v.regions.iter().map(|r| r.dirty).collect())
            .unwrap_or_default();
        assert_eq!(dirty, vec![true, false]);
        // Region 2 (30 fps, painted at 0.0) is not due at 0.01; only 1 paints.
        let frames = e.frames(s(0.01), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(1)]);
        assert_eq!(
            frames[0].invalidations.as_slice(),
            &[(
                RetainedScope::Region(1),
                InvalidateBits::MATERIAL.union(InvalidateBits::GEOMETRY)
            )]
        );
        // NONE bits: repaint, no retained invalidation.
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: Some(RegionId(1)),
            bits: InvalidateBits::NONE,
        });
        let frames = e.frames(s(0.02), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(1)]);
        assert!(frames[0].invalidations.is_empty());
        // Capped region 2 is due again at 1/30 s after its last paint.
        assert_eq!(e.next_wake(s(0.02), &view), Wake::At(s(1.0 / 30.0)));
        let frames = e.frames(s(0.04), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(2)]);
        assert!(frames[0].invalidations.is_empty());
    }

    #[test]
    fn window_wide_invalidation_forces_every_region() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Dirty);
        e.apply(CadenceOp::Cmd(CadenceCmd::SetRegions {
            win: W1,
            regions: vec![region(1, 0), region(2, 30)],
        }));
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        e.frames(s(0.0), &view);
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: None,
            bits: InvalidateBits::ALL,
        });
        assert_eq!(e.next_wake(s(0.001), &view), Wake::Immediate);
        let frames = e.frames(s(0.001), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(1), Some(2)]);
        assert_eq!(frames[0].invalidations.len(), 2);
        // Unchanged region set: no state change.
        let rev = e.revision();
        e.apply(CadenceOp::Cmd(CadenceCmd::SetRegions {
            win: W1,
            regions: vec![region(1, 0), region(2, 30)],
        }));
        assert_eq!(e.revision(), rev);
        // Moving region 2 repaints both (region set changed).
        let mut moved = region(2, 30);
        moved.rect = Rect::new(5.0, 5.0, 10.0, 10.0);
        e.apply(CadenceOp::Cmd(CadenceCmd::SetRegions {
            win: W1,
            regions: vec![region(1, 0), moved],
        }));
        assert_eq!(e.revision(), rev.next());
        let frames = e.frames(s(0.002), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(1), Some(2)]);
        assert_eq!(
            frames[0].invalidations.as_slice(),
            &[(RetainedScope::Region(2), InvalidateBits::ALL)]
        );
        // Dropping all regions returns to one whole-window frame.
        e.apply(CadenceOp::Cmd(CadenceCmd::SetRegions {
            win: W1,
            regions: Vec::new(),
        }));
        let frames = e.frames(s(0.003), &view);
        assert!(frames[0].regions.is_empty());
    }

    #[test]
    fn wake_reports_idle_capped_deadline_and_animation() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Dirty);
        let surfaces = [surface(W1)];
        let idle = CadenceTickView::new(&surfaces, BG, false);
        e.frames(s(0.0), &idle);
        // Dirty-driven, clean: Idle.
        assert_eq!(e.next_wake(s(0.0), &idle), Wake::Idle);
        // Deadline: At(t).
        e.apply(CadenceOp::Cmd(CadenceCmd::WakeAt {
            token: 1,
            at: s(2.0),
        }));
        assert_eq!(e.next_wake(s(0.0), &idle), Wake::At(s(2.0)));
        // Capped region earlier than the deadline wins.
        open(&mut e, W2, TickRate::Capped(4));
        let both = [surface(W1), surface(W2)];
        let view = CadenceTickView::new(&both, BG, false);
        e.frames(s(0.0), &view);
        assert_eq!(e.next_wake(s(0.0), &view), Wake::At(s(0.25)));
        // An overdue deadline is Immediate.
        assert_eq!(e.next_wake(s(3.0), &idle), Wake::Immediate);
        // Animation running: Immediate.
        let anim = CadenceTickView::new(&both, BG, true);
        assert_eq!(e.next_wake(s(0.0), &anim), Wake::Immediate);
        // An infinite deadline never wakes the loop.
        e.apply(CadenceOp::Cmd(CadenceCmd::WakeAt {
            token: 1,
            at: s(f64::INFINITY),
        }));
        assert_eq!(e.next_wake(s(0.0), &idle), Wake::Idle);
    }

    #[test]
    fn app_timers_fire_as_effects() {
        let mut e = CadenceEngine::new();
        let fx = e.apply(CadenceOp::Cmd(CadenceCmd::WakeAt {
            token: 7,
            at: s(1.0),
        }));
        assert!(matches!(
            fx.as_slice(),
            [CadenceEffect::Armed {
                owner: TimerOwner::App(7),
                ..
            }]
        ));
        let fx = e.apply(CadenceOp::Cmd(CadenceCmd::WakeAt {
            token: 7,
            at: s(0.5),
        }));
        let Some(CadenceEffect::Armed { token, .. }) = fx.first().copied() else {
            panic!("expected Armed");
        };
        assert_eq!(e.view().wheel().len(), 1);
        let rev = e.revision();
        assert!(e.tick(s(0.4)).is_empty());
        assert_eq!(e.revision(), rev);
        let fx = e.tick(s(0.5));
        assert_eq!(
            fx.as_slice(),
            &[CadenceEffect::Fired {
                token,
                owner: TimerOwner::App(7)
            }]
        );
        assert_eq!(e.revision(), rev.next());
        assert!(e.tick(s(5.0)).is_empty());
        // Cancel.
        e.apply(CadenceOp::Cmd(CadenceCmd::WakeAt {
            token: 8,
            at: s(6.0),
        }));
        let rev = e.revision();
        e.apply(CadenceOp::Cmd(CadenceCmd::Cancel { token: 8 }));
        e.apply(CadenceOp::Cmd(CadenceCmd::Cancel { token: 8 }));
        assert_eq!(e.revision(), rev.next());
        assert!(e.tick(s(10.0)).is_empty());
        // NaN arm: nothing armed, no effect, no bump.
        let rev = e.revision();
        let fx = e.apply(CadenceOp::Arm {
            owner: TimerOwner::App(9),
            at: s(f64::NAN),
        });
        assert!(fx.is_empty());
        assert_eq!(e.revision(), rev);
    }

    #[test]
    fn closing_a_window_drops_its_cadence_and_deadlines() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Dirty);
        let auto = TimerOwner::OverlayAutoClose {
            win: W1,
            slot: OverlaySlot(1),
        };
        e.apply(CadenceOp::Arm {
            owner: auto,
            at: s(1.0),
        });
        e.apply(CadenceOp::Arm {
            owner: TimerOwner::App(1),
            at: s(1.0),
        });
        e.apply(CadenceOp::Disarm(TimerOwner::Tooltip { win: W1 }));
        let rev = e.revision();
        e.apply(CadenceOp::Close(W1));
        assert_eq!(e.revision(), rev.next());
        assert!(e.view().window(W1).is_none());
        assert_eq!(e.view().wheel().deadline(auto), None);
        assert_eq!(e.view().wheel().len(), 1);
        e.apply(CadenceOp::Close(W1));
        assert_eq!(e.revision(), rev.next());
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        assert!(e.frames(s(0.0), &view).is_empty());
    }

    #[test]
    fn pending_whole_window_bits_carry_into_new_regions() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Dirty);
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        e.frames(s(0.0), &view);
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: None,
            bits: InvalidateBits::OPACITY,
        });
        let rev = e.revision();
        e.apply(CadenceOp::Cmd(CadenceCmd::SetRegions {
            win: W1,
            regions: vec![region(3, 0)],
        }));
        assert_eq!(e.revision(), rev.next());
        assert_eq!(e.view().ids().collect::<Vec<_>>(), vec![W1],);
        let frames = e.frames(s(0.1), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(3)]);
        assert_eq!(
            frames[0].invalidations.as_slice(),
            &[(RetainedScope::Region(3), InvalidateBits::ALL)]
        );
    }

    #[test]
    fn region_containing_maps_a_widget_rect_to_its_region() {
        let mut e = CadenceEngine::new();
        open(&mut e, W1, TickRate::Dirty);
        let field = Rect::new(10.0, 21.0, 30.0, 6.0);
        // No regions declared: the window is one implicit region.
        assert_eq!(e.view().region_containing(W1, field), None);
        e.apply(CadenceOp::Cmd(CadenceCmd::SetRegions {
            win: W1,
            regions: vec![region(1, 0), region(2, 0), region(3, 0)],
        }));
        // Region n spans y in [10n, 10n + 10); the field centre is y = 24.
        assert_eq!(e.view().region_containing(W1, field), Some(RegionId(2)));
        assert_eq!(
            e.view()
                .region_containing(W1, Rect::new(500.0, 500.0, 1.0, 1.0)),
            None
        );
        assert_eq!(e.view().region_containing(W2, field), None);
        // Invalidating just that region leaves the other regions clean.
        let surfaces = [surface(W1)];
        let view = CadenceTickView::new(&surfaces, BG, false);
        e.frames(s(0.0), &view);
        e.apply(CadenceOp::Invalidate {
            win: W1,
            region: e.view().region_containing(W1, field),
            bits: InvalidateBits::MATERIAL,
        });
        let frames = e.frames(s(0.1), &view);
        assert_eq!(region_ids(&frames[0]), vec![Some(2)]);
    }
}
