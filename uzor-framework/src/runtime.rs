//! ROLE runtime (shell): the tick loop around the kernel (design §2.5).
//!
//! The runtime owns the app, the kernel and the two channel ends; it runs
//! `tick` (kernel phases 1-4 and 7, intent delivery, settle passes) and
//! `paint` (phases 5-6 per due frame request). Hosts drive it; the app
//! never sees it (ban F8 — the app reaches on-screen state only through
//! the [`Handle`] and the hook contexts).

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use arc_swap::ArcSwap;
use uzor::render::{RenderContext, TickRate};
use uzor::tokens::Tokens;

use crate::handle::{App, Handle, INBOX_CAP, InitCx, IntentCx, Waker};
use crate::kernel::{Kernel, KernelConfig};
use crate::types::anim::AnimPolicy;
use crate::types::bus::InputEnvelope;
use crate::types::command::{AppCommand, LayoutPolicy, WindowCmd};
use crate::types::frame::{FrameRequest, Wake};
use crate::types::ids::{Revision, Seconds, WindowId};
use crate::types::snapshot::VisualSnapshot;
use crate::types::spec::Spec;
use crate::types::window::{ClosePolicy, WindowCommand, WindowSpec};

/// Settle passes a tick runs at most after the main pass (design §2.5
/// `MAX_SETTLE`): effects that feed back into commands or intents get
/// zero-input passes of phases 1, 3-4 and 7 until the tick is quiet.
pub const MAX_SETTLE: usize = 4;

/// Everything a tick hands to the host (design §2.5 `TickOutput`).
#[derive(Default)]
pub struct TickOutput {
    /// Host-bound window commands, in the order the engines enqueued them.
    pub window_commands: Vec<(WindowId, WindowCommand)>,
    /// Due frame requests; the host paints each with [`Runtime::paint`].
    pub frames: Vec<FrameRequest>,
    /// When the host should tick again.
    pub wake: Wake,
}

/// Runtime construction parameters (design §2.5 `RuntimeConfig`).
pub struct RuntimeConfig {
    /// Windows to open at startup (the app can open more from
    /// [`App::init`]).
    pub windows: Vec<WindowSpec>,
    /// Default repaint cadence of a window whose spec sets none.
    pub tick: TickRate,
    /// Animator durations.
    pub anim: AnimPolicy,
    /// Splitter / drag-out / expand behaviour.
    pub layout: LayoutPolicy,
    /// App-wide theme.
    pub tokens: Arc<Tokens>,
    /// Dark theme active.
    pub dark: bool,
    /// What happens when the user asks to close a window.
    pub close: ClosePolicy,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            tick: TickRate::default(),
            anim: AnimPolicy::default(),
            layout: LayoutPolicy::default(),
            tokens: Tokens::builtin(uzor::tokens::BuiltinSet::Dark),
            dark: true,
            close: ClosePolicy::default(),
        }
    }
}

/// The tick loop around the kernel. `S` is the app's vocabulary, `A` the
/// app itself.
pub struct Runtime<S: Spec, A: App<Spec = S>> {
    app: A,
    kernel: Kernel<S>,
    rx: Receiver<AppCommand<S>>,
    snap: Arc<ArcSwap<VisualSnapshot<S>>>,
    /// Inputs pushed since the last tick.
    inputs: Vec<InputEnvelope>,
    /// Commands waiting for the next pass: handle dispatches, hook and
    /// intent answers, startup commands.
    cmds: Vec<AppCommand<S>>,
    /// Host clock of the last tick (paints stamp their frames with it).
    now: Seconds,
    /// The last pass moved an engine revision or produced intents.
    last_pass_changed: bool,
}

impl<S: Spec, A: App<Spec = S>> Runtime<S, A> {
    /// Build the runtime and the app's handle (design §2.5). The app's
    /// [`App::init`] runs once, before the first tick; its commands and
    /// the config's startup windows land in the first tick's drain.
    pub fn new(mut app: A, cfg: RuntimeConfig, waker: Waker) -> (Self, Handle<S>) {
        let (tx, rx) = std::sync::mpsc::sync_channel(INBOX_CAP);
        let snap = Arc::new(ArcSwap::from_pointee(VisualSnapshot::empty()));
        let handle = Handle {
            tx,
            snap: Arc::clone(&snap),
            waker,
        };
        let kernel = Kernel::new(&KernelConfig {
            anim: cfg.anim,
            layout: cfg.layout,
            tokens: cfg.tokens,
            dark: cfg.dark,
            close: cfg.close,
            tick: cfg.tick,
        });
        let mut cmds = Vec::new();
        {
            let mut cx = InitCx { out: &mut cmds };
            app.init(&mut cx);
        }
        for spec in cfg.windows {
            cmds.push(AppCommand::Window(WindowCmd::Open(spec)));
        }
        let runtime = Self {
            app,
            kernel,
            rx,
            snap,
            inputs: Vec::new(),
            cmds,
            now: Seconds::ZERO,
            last_pass_changed: false,
        };
        (runtime, handle)
    }

    /// Feed one host input event; the next tick routes it.
    pub fn push_input(&mut self, input: InputEnvelope) {
        self.inputs.push(input);
    }

    /// One tick (design §2.5): drain the inbox, phases 1-4 and 7, deliver
    /// the intents (their commands feed the settle passes), then zero-input
    /// settle passes until quiet, at most [`MAX_SETTLE`].
    pub fn tick(&mut self, now: Seconds) -> TickOutput {
        self.now = now;
        while let Ok(cmd) = self.rx.try_recv() {
            self.cmds.push(cmd);
        }
        let mut out = TickOutput::default();
        self.pass(now, &mut out, true);
        let mut settles = 0;
        while (self.last_pass_changed || !self.cmds.is_empty()) && settles < MAX_SETTLE {
            settles += 1;
            self.pass(now, &mut out, false);
        }
        // Work outlived the settle budget: the loop must not sleep on it
        // (the cadence hint only covers frames and timers).
        if self.last_pass_changed || !self.cmds.is_empty() {
            out.wake = Wake::Immediate;
        }
        out
    }

    /// One pass: phases 1-4, publish, intent delivery.
    fn pass(&mut self, now: Seconds, out: &mut TickOutput, with_inputs: bool) {
        let mut inputs = if with_inputs {
            std::mem::take(&mut self.inputs)
        } else {
            Vec::new()
        };
        let mut cmds = std::mem::take(&mut self.cmds);
        let before = self.kernel.engine_revisions();
        self.kernel.step(now, &mut inputs, &mut cmds);
        let published = self.kernel.publish(now);
        out.window_commands.extend(published.window_commands);
        out.frames.extend(published.frames);
        out.wake = earliest(out.wake, published.wake);
        if let Some(snapshot) = published.snapshot {
            self.snap.store(Arc::new(snapshot));
        }
        // Intents deliver AFTER the publish of the same pass: an intent
        // handler reading the handle sees the state the intent reports.
        let intents = self.kernel.take_intents();
        self.last_pass_changed = !intents.is_empty() || before != self.kernel.engine_revisions();
        if !intents.is_empty() {
            let snapshot = self.snap.load_full();
            let mut answers = Vec::new();
            for intent in intents {
                let mut cx = IntentCx {
                    view: &snapshot,
                    out: &mut answers,
                };
                self.app.intent(intent, &mut cx);
            }
            self.cmds.extend(answers);
        }
    }

    /// Paint one due frame request (phases 5-6 for its window). The
    /// commands the hooks declare apply from the next tick's drain.
    pub fn paint(&mut self, req: &FrameRequest, ctx: &mut dyn RenderContext) {
        let out = self.kernel.compose(self.now, req.window, &mut self.app, ctx);
        self.cmds.extend(out.commands);
    }

    /// The last published snapshot (same read as the handle's).
    pub fn snapshot(&self) -> Arc<VisualSnapshot<S>> {
        self.snap.load_full()
    }

    /// The published revision.
    pub fn revision(&self) -> Revision {
        self.snap.load().revision
    }

    /// The host should exit its loop (every window closed or `Shutdown`).
    pub fn should_exit(&self) -> bool {
        self.kernel.exit_requested()
    }

    /// The app, for hosts that drive app-level tools (tests).
    pub fn app(&self) -> &A {
        &self.app
    }

    /// The app, mutably, outside any hook (tests, host tools).
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }

    /// The kernel (crate-internal: the §9.2 tests read the `RouteTrace`).
    pub(crate) fn kernel(&self) -> &Kernel<S> {
        &self.kernel
    }
}

/// The earlier of two wakes (`Immediate` < `At(t)` < `Idle`).
fn earliest(a: Wake, b: Wake) -> Wake {
    match (a, b) {
        (Wake::Immediate, _) | (_, Wake::Immediate) => Wake::Immediate,
        (Wake::At(x), Wake::At(y)) => Wake::At(if x.get() <= y.get() { x } else { y }),
        (Wake::At(t), Wake::Idle) | (Wake::Idle, Wake::At(t)) => Wake::At(t),
        (Wake::Idle, Wake::Idle) => Wake::Idle,
    }
}
