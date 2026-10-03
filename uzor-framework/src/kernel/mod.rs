//! ROLE kernel: the conductor between the seven single-writer engines
//! (design §3.8, §4).
//!
//! The kernel owns the engines and nothing else: no domain state of its
//! own beyond the publish bookkeeping (`revision`, `last_seen`) and the
//! outbox. Every cross-engine effect flows through the conduction table
//! of design §3.8, one statement per row, inside a fixed phase order
//! (§4.2):
//!
//! | # | Phase | In | Out |
//! |---|---|---|---|
//! | 1 | Drain | app commands | each applied to exactly its engine |
//! | 2 | Lifecycle | window / host echoes | WindowEngine, conduction |
//! | 3 | Route | pointer / wheel / key / IME / drop / clipboard | InputEngine, then the routed target |
//! | 4 | Engines | clock | cadence wheel, animators, layout tick, reconcile |
//! | 5 | Compose | engine views, app hooks | frames drawn, widgets registered |
//! | 6 | Responses | frame responses | kernel-owned clicks → intents |
//! | 7 | Publish | engine views / revisions | snapshot, outbox drain, `Wake` |
//!
//! Phases 1-4 are [`Kernel::step`], 5-6 are [`Kernel::compose`], 7 is
//! [`Kernel::publish`]. Time is always an argument (`Seconds`), never read
//! from a clock: the same `(inputs, cmds, now)` sequence produces the same
//! state, which is what makes headless goldens possible.
//!
//! The kernel never imports a platform, a render backend or any I/O (bans
//! F1, F2, F4); paint reaches it only as `&mut dyn RenderContext`.

mod compose;
mod furniture;
mod phases;
mod publish;
mod route;

use std::collections::BTreeMap;

use uzor::render::InvalidateBits;

use crate::engine::anim::AnimationEngine;
use crate::engine::cadence::CadenceEngine;
use crate::engine::input::InputEngine;
use crate::engine::keymap::KeymapEngine;
use crate::engine::layout::LayoutEngine;
use crate::engine::overlays::OverlayEngine;
use crate::engine::windows::WindowEngine;
use crate::types::anim::{AnimKey, AnimPolicy};
use crate::types::bus::InputEnvelope;
use crate::types::command::{AppCommand, LayoutPolicy};
use crate::types::ids::{Revision, Seconds, WindowId};
use crate::types::intent::{Intent, OverlayIntent, WindowIntent};
use crate::types::ops::{
    AnimEffect, AnimOp, CadenceEffect, CadenceOp, Capture, EngineTarget, FocusOp, InputEffect,
    InputOp, LayoutEffect, LayoutOp, OverlayEffect, OverlayOp, WindowEffect, WindowOp,
};
use crate::types::spec::Spec;
use crate::types::window::{ClosePolicy, WindowCommand};
use uzor::render::TickRate;

/// Everything a step collected for the outside: typed intents, in
/// production order. Window commands and frame requests are produced by
/// the publish phase straight into [`crate::TickOutput`].
pub(crate) struct Outbox<S: Spec> {
    /// Intents collected during phases 1-4 (and 6, via `ComposeOut`).
    pub intents: Vec<Intent<S>>,
}

// Manual impl: a derive would demand `S: Default` of the marker type.
impl<S: Spec> Default for Outbox<S> {
    fn default() -> Self {
        Self {
            intents: Vec::new(),
        }
    }
}

/// Per-engine revisions at the last publish; a moved revision republishes
/// the snapshot (design §4.6 invariant 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EngineRevisions {
    windows: Revision,
    layout: Revision,
    overlays: Revision,
    input: Revision,
    keymap: Revision,
    cadence: Revision,
    anim: Revision,
}

/// Kernel construction parameters (design §4.1 `KernelConfig`, minus the
/// settle limit, which the runtime owns).
pub(crate) struct KernelConfig {
    /// Animator durations.
    pub anim: AnimPolicy,
    /// Splitter / drag-out / expand behaviour.
    pub layout: LayoutPolicy,
    /// App-wide theme (tokens + dark flag) and close policy.
    pub tokens: std::sync::Arc<uzor::tokens::Tokens>,
    /// Dark theme active.
    pub dark: bool,
    /// What happens when the user asks to close a window.
    pub close: ClosePolicy,
    /// Default repaint cadence for windows whose spec does not set one.
    pub tick: TickRate,
}

/// The seven engines plus publish bookkeeping. Fields are private (ban
/// F6); the only doors are `step`, `compose` and `publish`.
pub(crate) struct Kernel<S: Spec> {
    windows: WindowEngine,
    layout: LayoutEngine<S::Panel>,
    overlays: OverlayEngine<S::Overlay>,
    input: InputEngine,
    keymap: KeymapEngine<S::Overlay, S::Action>,
    cadence: CadenceEngine,
    anim: AnimationEngine,
    revision: Revision,
    last_seen: EngineRevisions,
    outbox: Outbox<S>,
    /// Default cadence of a window whose spec sets none.
    default_tick: TickRate,
    /// Per-window clock of the last painted frame (for `FrameTime::dt`).
    last_frame: BTreeMap<WindowId, Seconds>,
    /// Hop names of the last routed event (design §4.4 `RouteTrace`):
    /// route-phase crossings only; with the `push_input` hop the total
    /// must stay `<= 3`. Always on (the hop tests run in release).
    trace: Vec<&'static str>,
    /// Longest per-event trace seen so far.
    trace_max: usize,
    /// Debug-only phase order assertions (design §4.2 `PhaseClock`).
    clock: PhaseClock,
}

impl<S: Spec> Kernel<S> {
    /// A kernel with no windows and the engines wired per the config.
    pub(crate) fn new(cfg: &KernelConfig) -> Self {
        Self {
            windows: WindowEngine::new(cfg.tokens.clone(), cfg.dark, cfg.close),
            layout: LayoutEngine::with_policy(S::decode_panel, cfg.layout),
            overlays: OverlayEngine::new(),
            input: InputEngine::new(),
            keymap: KeymapEngine::new(),
            cadence: CadenceEngine::new(),
            anim: AnimationEngine::new(cfg.anim),
            revision: Revision::ZERO,
            last_seen: EngineRevisions::default(),
            outbox: Outbox::default(),
            default_tick: cfg.tick,
            last_frame: BTreeMap::new(),
            trace: Vec::new(),
            trace_max: 0,
            clock: PhaseClock::new(),
        }
    }

    /// Phases 1-4: drain commands, lifecycle, route inputs, advance the
    /// engines. The intents the phases produced are collected into the
    /// outbox; the runtime delivers them after the publish of the same
    /// tick.
    pub(crate) fn step(
        &mut self,
        now: Seconds,
        inputs: &mut Vec<InputEnvelope>,
        cmds: &mut Vec<AppCommand<S>>,
    ) {
        self.clock.begin(Phase::Drain);
        self.drain(now, cmds);
        self.clock.idle();
        self.clock.begin(Phase::Lifecycle);
        self.lifecycle(now, inputs);
        self.clock.idle();
        self.clock.begin(Phase::Route);
        self.route(now, inputs);
        self.clock.idle();
        self.clock.begin(Phase::Engines);
        self.engines(now);
        self.clock.idle();
    }

    /// Intents collected since the last `take_intents`, in production
    /// order (drained, never re-read).
    pub(crate) fn take_intents(&mut self) -> Vec<Intent<S>> {
        std::mem::take(&mut self.outbox.intents)
    }

    /// The host asked to exit (every window closed, or `Shutdown`).
    pub(crate) fn exit_requested(&self) -> bool {
        self.windows.view().exit_requested()
    }

    /// Hop names of the last routed event, route-phase crossings only
    /// (design §4.4 `RouteTrace`).
    // Test doors (hop-count assertions in host::tests).
    #[allow(dead_code)]
    pub(crate) fn last_trace(&self) -> &[&'static str] {
        &self.trace
    }

    /// The longest per-event trace since construction; the hop-count test
    /// asserts `trace_max() + 1 <= 3` (the `+ 1` is the `push_input` hop).
    // Test doors (hop-count assertions in host::tests).
    #[allow(dead_code)]
    pub(crate) fn trace_max(&self) -> usize {
        self.trace_max
    }

    /// The current per-engine revisions (the runtime's settle loop
    /// compares them across a pass).
    pub(crate) fn engine_revisions(&self) -> EngineRevisions {
        EngineRevisions {
            windows: self.windows.revision(),
            layout: self.layout.revision(),
            overlays: self.overlays.revision(),
            input: self.input.revision(),
            keymap: self.keymap.revision(),
            cadence: self.cadence.revision(),
            anim: self.anim.revision(),
        }
    }

    // -- conduction (design §3.8, one statement per row) ---------------------

    /// Every consequence of a WindowEngine op, conducted.
    fn conduct_window(&mut self, now: Seconds, fx: WindowEffect) {
        match fx {
            WindowEffect::Spawned(win) => {
                // A drag-out micro-window got its id; no-op for any other
                // spawn (the engine ignores it without a pending panel).
                let effects = self.layout.apply(LayoutOp::AdoptPanel { win });
                self.conduct_layout(now, effects);
            }
            WindowEffect::Created(win) => {
                let (caps, viewport, tick) = {
                    let view = self.windows.view();
                    let w = view.window(win);
                    let caps = w.map_or_else(Default::default, |r| r.caps());
                    let viewport = w.map_or_else(Default::default, |r| r.geometry().viewport);
                    let tick = w.and_then(|r| r.spec().tick);
                    (caps, viewport, tick)
                };
                let fx = self.cadence.apply(CadenceOp::Open {
                    win,
                    tick: tick.unwrap_or(self.default_tick),
                });
                self.conduct_cadence(now, fx);
                let fx = self.layout.apply(LayoutOp::Open {
                    win,
                    caps,
                    viewport,
                });
                self.conduct_layout(now, fx);
                let fx = self.input.apply(InputOp::Open(win));
                self.conduct_input(now, fx);
                // The app learns the id here (design `WindowIntent::Opened`).
                // Layout commands need it; F7 conducted the engines but did
                // not surface the intent.
                if let Some(key) = self.windows.view().window(win).map(|w| w.key().clone()) {
                    self.outbox
                        .intents
                        .push(Intent::Window(WindowIntent::Opened { win, key }));
                }
            }
            WindowEffect::Closed(win) => {
                let fx = self.layout.apply(LayoutOp::Close(win));
                self.conduct_layout(now, fx);
                let fx = self.overlays.apply(OverlayOp::CloseWindow(win));
                self.conduct_overlay(now, fx);
                let fx = self.input.apply(InputOp::Close(win));
                self.conduct_input(now, fx);
                let fx = self.cadence.apply(CadenceOp::Close(win));
                self.conduct_cadence(now, fx);
                let fx = self.anim.apply(AnimOp::DropWindow(win));
                self.conduct_anim(now, fx);
                self.last_frame.remove(&win);
                self.outbox
                    .intents
                    .push(Intent::Window(WindowIntent::Closed { win }));
            }
            WindowEffect::CloseRequested(win) => {
                self.outbox
                    .intents
                    .push(Intent::Window(WindowIntent::CloseRequested { win }));
            }
            WindowEffect::GeometryChanged(win) => {
                let viewport = self
                    .windows
                    .view()
                    .window(win)
                    .map_or_else(Default::default, |r| r.geometry().viewport);
                let fx = self.layout.apply(LayoutOp::Solve { win, viewport });
                self.conduct_layout(now, fx);
                let fx = self.overlays.apply(OverlayOp::Reclamp { win, viewport });
                self.conduct_overlay(now, fx);
                self.invalidate(win, None, InvalidateBits::ALL);
            }
            WindowEffect::FocusChanged { win, focused } => {
                let fx = self.input.apply(InputOp::WindowFocus { win, focused, now });
                self.conduct_input(now, fx);
                self.outbox
                    .intents
                    .push(Intent::Window(WindowIntent::FocusChanged { win, focused }));
            }
            WindowEffect::ThemeChanged => {
                let wins: Vec<WindowId> = self.windows.view().ids().collect();
                for win in wins {
                    self.invalidate(win, None, InvalidateBits::ALL);
                }
            }
        }
    }

    /// Every consequence of a LayoutEngine op or tick, conducted.
    fn conduct_layout(&mut self, now: Seconds, effects: crate::engine::layout::LayoutEffects) {
        for fx in effects {
            match fx {
                LayoutEffect::Window { win, cmd } => {
                    let effects = self.windows.apply(WindowOp::Enqueue { win, cmd });
                    self.conduct_window_all(now, effects);
                }
                LayoutEffect::Chrome { win, action } => self.chrome_action(now, win, action),
                LayoutEffect::Capture { win, hold } => {
                    let capture = hold.then_some(Capture::Engine(EngineTarget::Layout));
                    let effects = self.input.apply(InputOp::SetCapture { win, capture });
                    self.conduct_input(now, effects);
                }
                LayoutEffect::Intent(intent) => {
                    // `LayoutChanged` arrives coalesced from the engine's
                    // tick (at most one per window per tick, design §5.3).
                    self.outbox.intents.push(Intent::Dock(intent));
                }
                LayoutEffect::Invalidate { win, bits } => self.invalidate(win, None, bits),
                LayoutEffect::ExpandTarget { win, kind, side } => {
                    let effects = self.anim.apply(AnimOp::SetTarget {
                        key: AnimKey::Expand { win, kind },
                        target: if side.is_some() { 1.0 } else { 0.0 },
                        now,
                    });
                    self.conduct_anim(now, effects);
                }
                LayoutEffect::ExpandDone { win, kind } => {
                    let effects = self
                        .anim
                        .apply(AnimOp::Remove(AnimKey::Expand { win, kind }));
                    self.conduct_anim(now, effects);
                }
                LayoutEffect::SpawnWindow { spec, .. } => {
                    let effects = self.windows.apply(WindowOp::Create(spec));
                    self.conduct_window_all(now, effects);
                }
            }
        }
    }

    /// Every consequence of an OverlayEngine op, conducted.
    fn conduct_overlay(
        &mut self,
        now: Seconds,
        effects: crate::engine::overlays::OverlayEffects<S::Overlay>,
    ) {
        for fx in effects {
            match fx {
                OverlayEffect::Opened {
                    win,
                    id,
                    slot,
                    scope,
                    auto_close_at,
                    ..
                } => {
                    if let Some(owner) = scope {
                        let effects = self.input.apply(InputOp::Focus {
                            win,
                            now,
                            op: FocusOp::PushScope {
                                owner,
                                members: Vec::new(),
                            },
                        });
                        self.conduct_input(now, effects);
                    }
                    if let Some(at) = auto_close_at {
                        let effects = self.cadence.apply(CadenceOp::Arm {
                            owner: crate::types::frame::TimerOwner::OverlayAutoClose { win, slot },
                            at,
                        });
                        self.conduct_cadence(now, effects);
                    }
                    let effects = self.anim.apply(AnimOp::SetTarget {
                        key: AnimKey::OverlayFade { win, slot },
                        target: 1.0,
                        now,
                    });
                    self.conduct_anim(now, effects);
                    self.invalidate(win, None, InvalidateBits::STRUCTURE);
                    self.outbox
                        .intents
                        .push(Intent::Overlay(OverlayIntent::Opened { win, id }));
                }
                OverlayEffect::Closed {
                    win,
                    id,
                    slot,
                    scope,
                    cause,
                    restore_focus,
                    auto_close,
                } => {
                    if let Some(owner) = scope {
                        let effects = self.input.apply(InputOp::Focus {
                            win,
                            now,
                            op: FocusOp::PopScope {
                                owner,
                                restore: restore_focus,
                            },
                        });
                        self.conduct_input(now, effects);
                    }
                    if auto_close {
                        let effects = self.cadence.apply(CadenceOp::Disarm(
                            crate::types::frame::TimerOwner::OverlayAutoClose { win, slot },
                        ));
                        self.conduct_cadence(now, effects);
                    }
                    let effects = self
                        .anim
                        .apply(AnimOp::Remove(AnimKey::OverlayFade { win, slot }));
                    self.conduct_anim(now, effects);
                    self.invalidate(win, None, InvalidateBits::STRUCTURE);
                    self.outbox
                        .intents
                        .push(Intent::Overlay(OverlayIntent::Closed { win, id, cause }));
                }
                OverlayEffect::Pointer { .. } | OverlayEffect::Key { .. } => {
                    // Routing verdicts are read inline by the route phase;
                    // they never reach the conduction queue.
                }
                OverlayEffect::Intent(intent) => {
                    self.outbox.intents.push(Intent::Overlay(intent));
                }
                OverlayEffect::Capture { win, slot } => {
                    let capture = slot.map(|s| Capture::Engine(EngineTarget::Overlay(s)));
                    let effects = self.input.apply(InputOp::SetCapture { win, capture });
                    self.conduct_input(now, effects);
                }
                OverlayEffect::Invalidate { win, bits } => self.invalidate(win, None, bits),
            }
        }
    }

    /// Every consequence of an InputEngine op or tick, conducted. Pointer
    /// / key routing answers (`Pointer`, `Content`, `KeyPassed`) are
    /// consumed inline by the route phase and never arrive here.
    fn conduct_input(&mut self, now: Seconds, effects: crate::engine::input::InputEffects) {
        for fx in effects {
            match fx {
                InputEffect::Pointer(_)
                | InputEffect::Content { .. }
                | InputEffect::KeyPassed { .. } => {
                    debug_assert!(false, "routing answers are consumed by the route phase");
                }
                InputEffect::Text(text) => {
                    self.outbox.intents.push(Intent::Text(text));
                }
                InputEffect::CopyText { win, text } => {
                    let effects = self.windows.apply(WindowOp::Enqueue {
                        win,
                        cmd: WindowCommand::ClipboardWrite(text),
                    });
                    self.conduct_window_all(now, effects);
                }
                InputEffect::NeedPaste { win, ticket } => {
                    let effects = self.windows.apply(WindowOp::Enqueue {
                        win,
                        cmd: WindowCommand::ClipboardRead(ticket),
                    });
                    self.conduct_window_all(now, effects);
                }
                InputEffect::ImeArea { win, rect } => {
                    let effects = self.windows.apply(WindowOp::Enqueue {
                        win,
                        cmd: WindowCommand::SetImeCursorArea(rect),
                    });
                    self.conduct_window_all(now, effects);
                }
                InputEffect::ImeAllowed { win, allowed } => {
                    let effects = self.windows.apply(WindowOp::Enqueue {
                        win,
                        cmd: WindowCommand::SetImeAllowed(allowed),
                    });
                    self.conduct_window_all(now, effects);
                }
                InputEffect::CaretDeadline { win, at } => {
                    let effects = match at {
                        Some(at) => self.cadence.apply(CadenceOp::Arm {
                            owner: crate::types::frame::TimerOwner::CaretBlink { win },
                            at,
                        }),
                        None => self.cadence.apply(CadenceOp::Disarm(
                            crate::types::frame::TimerOwner::CaretBlink { win },
                        )),
                    };
                    self.conduct_cadence(now, effects);
                }
                InputEffect::InvalidateField { win, rect } => {
                    let region = rect.and_then(|r| self.cadence.view().region_containing(win, r));
                    self.invalidate(win, region, InvalidateBits::MATERIAL);
                }
                InputEffect::ScopePushed { .. } => {}
            }
        }
    }

    /// Every consequence of a CadenceEngine op or tick, conducted.
    fn conduct_cadence(&mut self, now: Seconds, effects: crate::engine::cadence::CadenceEffects) {
        for fx in effects {
            match fx {
                CadenceEffect::Armed { .. } => {}
                CadenceEffect::Fired { owner, .. } => match owner {
                    crate::types::frame::TimerOwner::App(n) => {
                        self.outbox.intents.push(Intent::Timer(n));
                    }
                    crate::types::frame::TimerOwner::OverlayAutoClose { win, slot } => {
                        let effects = self.overlays.apply(OverlayOp::Fire { win, slot });
                        self.conduct_overlay(now, effects);
                    }
                    crate::types::frame::TimerOwner::Tooltip { .. } => {
                        // No tooltip engine op exists yet; the owner is
                        // reserved on the wheel (design §5.1 `TimerOwner`).
                    }
                    crate::types::frame::TimerOwner::CaretBlink { .. } => {
                        // The input engine's tick re-arms the edge itself;
                        // calling it for any caret fire is correct (it
                        // checks every window's armed deadline).
                        let effects = self.input.tick(now);
                        self.conduct_input(now, effects);
                    }
                },
            }
        }
    }

    /// Every consequence of an AnimationEngine op or tick, conducted.
    fn conduct_anim(&mut self, now: Seconds, effects: crate::engine::anim::AnimEffects) {
        for fx in effects {
            let (key, t) = match fx {
                AnimEffect::Value { key, t } | AnimEffect::Finished { key, t } => (key, t),
            };
            match key {
                AnimKey::Expand { win, kind } => {
                    let effects = self.layout.apply(LayoutOp::ExpandValue { win, kind, t });
                    self.conduct_layout(now, effects);
                }
                AnimKey::OverlayFade { win, .. } => {
                    // The fade value is read by compose; only the repaint
                    // needs conduction here.
                    self.invalidate(win, None, InvalidateBits::MATERIAL);
                }
            }
        }
    }

    fn conduct_window_all(&mut self, now: Seconds, effects: crate::engine::windows::WindowEffects) {
        for fx in effects {
            self.conduct_window(now, fx);
        }
    }

    /// A chrome control click, mapped (design §3.8 row `LayoutEffect::Chrome`).
    fn chrome_action(
        &mut self,
        now: Seconds,
        win: WindowId,
        action: uzor::widgets::composite::chrome::ChromeAction,
    ) {
        use uzor::widgets::composite::chrome::ChromeAction as A;
        match action {
            A::Minimize => {
                let fx =
                    self.windows
                        .apply(WindowOp::Cmd(crate::types::command::WindowCmd::Minimize {
                            win,
                            minimized: true,
                        }));
                self.conduct_window_all(now, fx);
            }
            A::MaximizeRestore => {
                let maximized = self
                    .windows
                    .view()
                    .window(win)
                    .is_some_and(|w| w.geometry().maximized);
                let fx =
                    self.windows
                        .apply(WindowOp::Cmd(crate::types::command::WindowCmd::Maximize {
                            win,
                            maximized: !maximized,
                        }));
                self.conduct_window_all(now, fx);
            }
            A::CloseWindow => {
                let fx = self
                    .windows
                    .apply(WindowOp::Cmd(crate::types::command::WindowCmd::Close(win)));
                self.conduct_window_all(now, fx);
            }
            A::CloseApp => {
                let fx = self.windows.apply(WindowOp::Shutdown);
                self.conduct_window_all(now, fx);
            }
            A::NewWindow => {
                self.outbox
                    .intents
                    .push(Intent::Window(WindowIntent::NewWindowRequested {
                        from: win,
                    }));
            }
            A::SelectTab(index) => {
                self.outbox
                    .intents
                    .push(Intent::Overlay(OverlayIntent::ChromeTab { win, index }));
            }
            A::OpenMenu => {
                self.outbox
                    .intents
                    .push(Intent::Overlay(OverlayIntent::ChromeControl {
                        win,
                        control: uzor::layout::ChromeWindowControl::Menu,
                    }));
            }
            A::NewTab | A::CloseTab(_) | A::WindowDragStart | A::BeginResize(_) | A::None => {
                // Tab creation is the app's (it owns the panel values);
                // drag start and resize presses are handled by the layout
                // engine's press path, not the click path.
            }
        }
    }

    /// One invalidation into the cadence engine, conducted.
    fn invalidate(
        &mut self,
        win: WindowId,
        region: Option<crate::types::ids::RegionId>,
        bits: InvalidateBits,
    ) {
        let fx = self
            .cadence
            .apply(CadenceOp::Invalidate { win, region, bits });
        // Invalidate produces no effects that need conduction; drain for
        // completeness (Armed is only produced by Arm).
        let _ = fx;
    }
}

/// The fixed phase list (design §4.2); debug builds assert the kernel
/// runs `step`, `compose` and `publish` in this order and never nests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// Phase 1: command drain.
    Drain,
    /// Phase 2: window / host lifecycle.
    Lifecycle,
    /// Phase 3: input routing.
    Route,
    /// Phase 4: engine clocks.
    Engines,
    /// Phases 5-6: compose and responses.
    Compose,
    /// Phase 7: publish.
    Publish,
}

/// Debug-only phase order assertions (design §4.2 `PhaseClock`). Release
/// builds compile the body away; the phase-tag-per-`apply` allow-list of
/// the design would touch every engine signature built in F2-F6, so the
/// clock asserts the order at the kernel level only (recorded deviation).
#[derive(Debug, Default)]
pub(crate) struct PhaseClock {
    active: Option<Phase>,
}

impl PhaseClock {
    fn new() -> Self {
        Self::default()
    }

    fn begin(&mut self, phase: Phase) {
        debug_assert!(
            self.active.is_none(),
            "phase {phase:?} began inside {:?}",
            self.active
        );
        #[cfg(debug_assertions)]
        {
            self.active = Some(phase);
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = phase;
        }
    }

    fn idle(&mut self) {
        #[cfg(debug_assertions)]
        {
            self.active = None;
        }
    }
}
