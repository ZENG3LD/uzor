//! AnimationEngine: typed linear animators for framework-level animation
//! (design §3.7): window expand (outer / inner) and overlay fade.
//!
//! Widget-internal easing stays in the library widgets; this engine only
//! owns the animators the kernel conducts (`LayoutEffect::ExpandTarget`,
//! overlay open / close) and reports their values back as effects.
//!
//! ## Stepping
//!
//! [`Linear::step`] is the previous kernel's `tick_anim` verbatim: the value
//! moves toward the target by `dt / secs` (clamped at the target), `secs <= 0`
//! jumps to the target, a target within `f64::EPSILON` is "at rest", and the
//! timing base (`last_tick`) advances on every step, at rest or not.
//! [`AnimationEngine::tick`] steps every animator once per call in key order.
//!
//! Because the loop may sleep while nothing animates (`Wake::Idle`), an
//! animator that is at rest when it gets a new target restarts its timing
//! base at the op's `now`; the previous kernel ticked every frame, so its
//! base was always the last frame and the two agree. A target change while
//! in flight keeps the base (a reversal continues from the current value).
//!
//! ## Revision
//!
//! Bumped when observable state changes: an animator is added or removed,
//! a target or duration changes, or a value moves. The timing base alone is
//! not revisioned.

use std::collections::BTreeMap;

use smallvec::SmallVec;

use crate::types::anim::{AnimKey, AnimPolicy};
use crate::types::ids::{Revision, Seconds, WindowId};
use crate::types::ops::{AnimEffect, AnimOp};

/// Effects of one AnimationEngine op or tick.
pub type AnimEffects = SmallVec<[AnimEffect; 2]>;

/// Advance a linear animator value toward target; returns `(new_t, changed)`.
///
/// Ported verbatim from the previous kernel's `tick_anim`
/// (`engine/dock/anim.rs:65-82`).
fn tick_anim(t: f64, target: f64, last_tick_s: f64, now_s: f64, fade_secs: f64) -> (f64, bool) {
    if (target - t).abs() < f64::EPSILON {
        return (t, false);
    }
    if fade_secs <= 0.0 {
        return (target, true);
    }
    let dt = (now_s - last_tick_s).max(0.0);
    let step = dt / fade_secs;
    let new_t = if target > t {
        (t + step).min(target)
    } else {
        (t - step).max(target)
    };
    (new_t, true)
}

/// One linear animator: value `t` moving toward `target` over `secs` per
/// unit of distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Linear {
    t: f64,
    target: f64,
    last_tick: Seconds,
    secs: f64,
}

impl Linear {
    /// An animator at value `t`, heading for `target`, last stepped at
    /// `last_tick`, sweeping one unit in `secs`.
    pub fn new(t: f64, target: f64, last_tick: Seconds, secs: f64) -> Self {
        Self {
            t,
            target,
            last_tick,
            secs,
        }
    }

    /// Step to `now`; `true` while the animator was not at rest (the
    /// previous kernel's "changed" flag, which is `true` even for a zero
    /// `dt`). The timing base always advances to `now`.
    pub fn step(&mut self, now: Seconds) -> bool {
        let (t, changed) = tick_anim(
            self.t,
            self.target,
            self.last_tick.get(),
            now.get(),
            self.secs,
        );
        self.t = t;
        self.last_tick = now;
        changed
    }

    /// Current value.
    pub fn value(&self) -> f64 {
        self.t
    }

    /// Target value.
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Seconds per unit of distance.
    pub fn secs(&self) -> f64 {
        self.secs
    }

    /// Instant of the last step (the timing base).
    pub fn last_tick(&self) -> Seconds {
        self.last_tick
    }

    /// Value within `f64::EPSILON` of the target.
    pub fn at_rest(&self) -> bool {
        (self.target - self.t).abs() < f64::EPSILON
    }
}

/// The AnimationEngine: see the module docs.
#[derive(Clone, Debug)]
pub struct AnimationEngine {
    linear: BTreeMap<AnimKey, Linear>,
    policy: AnimPolicy,
    rev: Revision,
}

impl AnimationEngine {
    /// An engine with no animators and the given durations.
    pub fn new(policy: AnimPolicy) -> Self {
        Self {
            linear: BTreeMap::new(),
            policy,
            rev: Revision::ZERO,
        }
    }

    /// The only mutation door. Bumps the revision iff state changed. Ops
    /// report nothing; values move only in [`Self::tick`].
    pub fn apply(&mut self, op: AnimOp) -> AnimEffects {
        let changed = match op {
            AnimOp::SetTarget { key, target, now } => self.set_target(key, target, now),
            AnimOp::Remove(key) => self.linear.remove(&key).is_some(),
            AnimOp::DropWindow(win) => {
                let before = self.linear.len();
                self.linear.retain(|k, _| k.window() != win);
                self.linear.len() != before
            }
            AnimOp::SetPolicy(policy) => {
                if policy == self.policy {
                    false
                } else {
                    self.policy = policy;
                    for (key, lin) in &mut self.linear {
                        lin.secs = policy.secs_for(*key);
                    }
                    true
                }
            }
        };
        if changed {
            self.rev.bump();
        }
        AnimEffects::new()
    }

    /// Phase 4b: step every animator to `now`, in key order. Reports a
    /// [`AnimEffect::Value`] for every value that moved and a following
    /// [`AnimEffect::Finished`] for every animator that reached its target
    /// in this step.
    pub fn tick(&mut self, now: Seconds) -> AnimEffects {
        let mut fx = AnimEffects::new();
        for (key, lin) in &mut self.linear {
            let before = lin.t;
            let was_at_rest = lin.at_rest();
            lin.step(now);
            if lin.t != before {
                fx.push(AnimEffect::Value {
                    key: *key,
                    t: lin.t,
                });
            }
            if !was_at_rest && lin.at_rest() {
                fx.push(AnimEffect::Finished {
                    key: *key,
                    t: lin.t,
                });
            }
        }
        if fx.iter().any(|f| matches!(f, AnimEffect::Value { .. })) {
            self.rev.bump();
        }
        fx
    }

    /// Bumped on every observable state change.
    pub fn revision(&self) -> Revision {
        self.rev
    }

    /// Read-only projection.
    pub fn view(&self) -> AnimationEngineView<'_> {
        AnimationEngineView { e: self }
    }

    fn set_target(&mut self, key: AnimKey, target: f64, now: Seconds) -> bool {
        if !target.is_finite() {
            return false;
        }
        let secs = self.policy.secs_for(key);
        match self.linear.get_mut(&key) {
            None => {
                self.linear.insert(key, Linear::new(0.0, target, now, secs));
                true
            }
            Some(lin) => {
                if lin.target == target {
                    return false;
                }
                if lin.at_rest() {
                    lin.last_tick = now;
                }
                lin.target = target;
                true
            }
        }
    }
}

/// Read-only projection of the [`AnimationEngine`].
#[derive(Clone, Copy, Debug)]
pub struct AnimationEngineView<'a> {
    e: &'a AnimationEngine,
}

impl<'a> AnimationEngineView<'a> {
    /// One animator.
    pub fn get(&self, key: AnimKey) -> Option<&'a Linear> {
        self.e.linear.get(&key)
    }

    /// Current value of one animator.
    pub fn value(&self, key: AnimKey) -> Option<f64> {
        self.get(key).map(Linear::value)
    }

    /// Every animator, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (AnimKey, &'a Linear)> + 'a {
        self.e.linear.iter().map(|(k, l)| (*k, l))
    }

    /// Some animator is not at rest (the loop must not sleep).
    pub fn is_animating(&self) -> bool {
        self.e.linear.values().any(|l| !l.at_rest())
    }

    /// Windows with an animator in flight, ascending, without duplicates.
    pub fn animating_windows(&self) -> Vec<WindowId> {
        let mut v: Vec<WindowId> = self
            .e
            .linear
            .iter()
            .filter(|(_, l)| !l.at_rest())
            .map(|(k, _)| k.window())
            .collect();
        v.dedup();
        v
    }

    /// The durations in force.
    pub fn policy(&self) -> AnimPolicy {
        self.e.policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::anim::ExpandKind;
    use crate::types::ids::OverlaySlot;

    /// The previous kernel's `tick_anim`, copied byte for byte as the parity
    /// reference (tessera-kernel `engine/dock/anim.rs:65-82`).
    fn tessera_tick_anim(
        t: f64,
        target: f64,
        last_tick_s: f64,
        now_s: f64,
        fade_secs: f64,
    ) -> (f64, bool) {
        if (target - t).abs() < f64::EPSILON {
            return (t, false);
        }
        if fade_secs <= 0.0 {
            return (target, true);
        }
        let dt = (now_s - last_tick_s).max(0.0);
        let step = dt / fade_secs;
        let new_t = if target > t {
            (t + step).min(target)
        } else {
            (t - step).max(target)
        };
        (new_t, true)
    }

    /// Tessera's default `OverlayPalette::fade_secs`, and the call-site clamp
    /// it applied (`fade_secs.max(0.001)`).
    const TESSERA_FADE: f64 = 0.06;
    const TESSERA_MIN_FADE: f64 = 0.001;

    const W1: WindowId = WindowId(1);
    const OUTER: AnimKey = AnimKey::Expand {
        win: W1,
        kind: ExpandKind::Outer,
    };
    const INNER: AnimKey = AnimKey::Expand {
        win: W1,
        kind: ExpandKind::Inner,
    };
    const FADE: AnimKey = AnimKey::OverlayFade {
        win: WindowId(2),
        slot: OverlaySlot(1),
    };

    fn s(t: f64) -> Seconds {
        Seconds(t)
    }

    #[test]
    fn linear_step_parity_with_tessera_tick_anim() {
        // (t, target, last_tick, now, secs)
        let cases: &[(f64, f64, f64, f64, f64)] = &[
            // expand in at 60 / 144 Hz with the default fade
            (0.0, 1.0, 0.0, 1.0 / 60.0, TESSERA_FADE),
            (0.0, 1.0, 10.0, 10.0 + 1.0 / 144.0, TESSERA_FADE),
            (0.277_777, 1.0, 0.5, 0.516_666, TESSERA_FADE),
            // retract
            (1.0, 0.0, 2.0, 2.0 + 1.0 / 60.0, TESSERA_FADE),
            (0.3, 0.0, 2.0, 2.02, TESSERA_FADE),
            // overshoot clamps at the target, both directions
            (0.9, 1.0, 0.0, 0.5, TESSERA_FADE),
            (0.1, 0.0, 0.0, 0.5, TESSERA_FADE),
            // reversal mid-flight
            (0.6, 0.0, 1.0, 1.01, TESSERA_FADE),
            // non-binary targets
            (0.25, 0.75, 0.0, 0.01, 0.2),
            (0.75, 0.25, 0.0, 0.01, 0.2),
            // at rest and within epsilon: no change
            (1.0, 1.0, 0.0, 1.0, TESSERA_FADE),
            (0.0, f64::EPSILON / 2.0, 0.0, 1.0, TESSERA_FADE),
            // zero and negative dt (clock went back): changed, value stays
            (0.4, 1.0, 5.0, 5.0, TESSERA_FADE),
            (0.4, 1.0, 5.0, 4.0, TESSERA_FADE),
            // instant (secs <= 0) and the tessera minimum clamp
            (0.0, 1.0, 0.0, 0.0, 0.0),
            (0.7, 0.0, 0.0, 0.0, -1.0),
            (0.0, 1.0, 0.0, 0.0005, TESSERA_MIN_FADE),
            (0.0, 1.0, 0.0, 1.0 / 60.0, TESSERA_MIN_FADE),
        ];
        for &(t, target, last, now, secs) in cases {
            let (want_t, want_changed) = tessera_tick_anim(t, target, last, now, secs);
            let mut lin = Linear::new(t, target, s(last), secs);
            let changed = lin.step(s(now));
            assert_eq!(
                (lin.value().to_bits(), changed),
                (want_t.to_bits(), want_changed),
                "case {:?}",
                (t, target, last, now, secs)
            );
            assert_eq!(lin.last_tick(), s(now));
            assert_eq!(lin.target(), target);
        }
    }

    #[test]
    fn linear_step_sequence_matches_tessera_frame_loop() {
        // Tessera's frame loop: step every frame, last_tick = now each frame,
        // target flips to 0 half way through.
        let dt = 1.0 / 60.0;
        let (mut rt, mut rlast) = (0.0_f64, 0.0_f64);
        let mut lin = Linear::new(0.0, 1.0, s(0.0), TESSERA_FADE);
        for frame in 1..=12 {
            let now = f64::from(frame) * dt;
            let target = if frame <= 2 { 1.0 } else { 0.0 };
            if lin.target() != target {
                lin = Linear::new(lin.value(), target, lin.last_tick(), lin.secs());
            }
            let (nt, rc) = tessera_tick_anim(rt, target, rlast, now, TESSERA_FADE);
            rt = nt;
            rlast = now;
            let c = lin.step(s(now));
            assert_eq!(
                (lin.value().to_bits(), c),
                (rt.to_bits(), rc),
                "frame {frame}"
            );
        }
        assert_eq!(lin.value(), 0.0);
    }

    #[test]
    fn expand_completes_and_reports_once() {
        let mut e = AnimationEngine::new(AnimPolicy::default());
        e.apply(AnimOp::SetTarget {
            key: OUTER,
            target: 1.0,
            now: s(10.0),
        });
        assert_eq!(e.revision(), Revision(1));
        assert!(e.view().is_animating());
        assert_eq!(e.view().animating_windows(), vec![W1]);
        let dt = 1.0 / 60.0;
        let mut finished_at = None;
        let mut values = Vec::new();
        for frame in 1..=6 {
            let fx = e.tick(s(10.0 + f64::from(frame) * dt));
            for f in &fx {
                match *f {
                    AnimEffect::Value { key, t } => {
                        assert_eq!(key, OUTER);
                        values.push(t);
                    }
                    AnimEffect::Finished { key, t } => {
                        assert_eq!((key, t), (OUTER, 1.0));
                        assert!(finished_at.is_none(), "finished twice");
                        finished_at = Some(frame);
                    }
                }
            }
        }
        // 0.06 s at 60 Hz: 4 frames (the 4th clamps at 1.0).
        assert_eq!(finished_at, Some(4));
        assert_eq!(values.len(), 4);
        assert!(values.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(values.last().copied(), Some(1.0));
        assert!(!e.view().is_animating());
        // At rest: ticks change nothing and do not bump.
        let rev = e.revision();
        assert!(e.tick(s(11.0)).is_empty());
        assert_eq!(e.revision(), rev);
        assert_eq!(e.view().value(OUTER), Some(1.0));
    }

    #[test]
    fn restart_from_rest_uses_the_op_instant() {
        let mut e = AnimationEngine::new(AnimPolicy::default());
        e.apply(AnimOp::SetTarget {
            key: FADE,
            target: 1.0,
            now: s(0.0),
        });
        e.tick(s(1.0));
        assert_eq!(e.view().value(FADE), Some(1.0));
        // Long idle, then fade out: the first step measures from the op, not
        // from the last tick 99 s ago.
        e.apply(AnimOp::SetTarget {
            key: FADE,
            target: 0.0,
            now: s(100.0),
        });
        e.tick(s(100.03));
        let v = e.view().value(FADE).unwrap_or(f64::NAN);
        assert!((v - 0.5).abs() < 1e-9, "value {v}");
        // Reversal in flight keeps the timing base.
        e.apply(AnimOp::SetTarget {
            key: FADE,
            target: 1.0,
            now: s(500.0),
        });
        assert_eq!(e.view().get(FADE).map(Linear::last_tick), Some(s(100.03)));
    }

    #[test]
    fn revision_bumps_exactly_on_change() {
        let mut e = AnimationEngine::new(AnimPolicy::default());
        let set = |e: &mut AnimationEngine, key, target| {
            e.apply(AnimOp::SetTarget {
                key,
                target,
                now: s(0.0),
            });
        };
        set(&mut e, OUTER, 1.0);
        set(&mut e, OUTER, 1.0);
        set(&mut e, OUTER, f64::NAN);
        assert_eq!(e.revision(), Revision(1));
        set(&mut e, INNER, 0.0);
        assert_eq!(e.revision(), Revision(2));
        // INNER is created at rest at 0: not animating.
        assert_eq!(e.view().iter().filter(|(_, l)| !l.at_rest()).count(), 1);
        // A tick at the same instant moves nothing: no bump.
        let fx = e.tick(s(0.0));
        assert!(fx.is_empty());
        assert_eq!(e.revision(), Revision(2));
        // Policy.
        e.apply(AnimOp::SetPolicy(AnimPolicy::default()));
        assert_eq!(e.revision(), Revision(2));
        let p = AnimPolicy {
            expand_secs: 0.0,
            overlay_fade_secs: 0.2,
        };
        e.apply(AnimOp::SetPolicy(p));
        assert_eq!(e.revision(), Revision(3));
        assert_eq!(e.view().policy(), p);
        // expand_secs 0: instant on the next tick.
        let fx = e.tick(s(0.0));
        assert_eq!(
            fx.as_slice(),
            &[
                AnimEffect::Value { key: OUTER, t: 1.0 },
                AnimEffect::Finished { key: OUTER, t: 1.0 }
            ]
        );
        assert_eq!(e.revision(), Revision(4));
        // Removal.
        e.apply(AnimOp::Remove(OUTER));
        e.apply(AnimOp::Remove(OUTER));
        assert_eq!(e.revision(), Revision(5));
        set(&mut e, FADE, 1.0);
        e.apply(AnimOp::DropWindow(W1));
        assert_eq!(e.revision(), Revision(7));
        e.apply(AnimOp::DropWindow(W1));
        assert_eq!(e.revision(), Revision(7));
        assert_eq!(
            e.view().iter().map(|(k, _)| k).collect::<Vec<_>>(),
            vec![FADE]
        );
        assert!(e.apply(AnimOp::DropWindow(WindowId(2))).is_empty());
        assert_eq!(e.view().iter().count(), 0);
    }
}
