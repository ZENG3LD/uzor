//! The headless host (design §9.1): deterministic, clock-injected, no OS.
//!
//! Time never moves on its own — [`HeadlessHost::advance`] adds `dt` to
//! the host clock and runs exactly one tick (plus the settling tick the
//! host's own echo answers cause), painting every due frame request into
//! a recording context. Host-bound commands that need an OS answer are
//! answered automatically: `Spawn` gets a `Created` echo, `Close` gets
//! `Destroyed`. Every §9.2 script runs against this host.

use std::sync::Arc;

use uzor::testing::RecordingRenderContext;

use crate::runtime::{Runtime, RuntimeConfig, TickOutput};
use crate::types::bus::{HostCaps, InputEnvelope, InputEvent, WindowInput};
use crate::types::command::{AppCommand, WindowCmd};
use crate::types::ids::{Seconds, WindowId};
use crate::types::snapshot::VisualSnapshot;
use crate::types::spec::Spec;
use crate::types::window::{SizePx, WindowCommand, WindowSpec};
use crate::{App, Handle, Waker};

/// A runtime driven by hand: no event loop, no wall clock, no GPU.
pub struct HeadlessHost<S: Spec, A: App<Spec = S>> {
    rt: Runtime<S, A>,
    handle: Handle<S>,
    now: Seconds,
    /// Capabilities reported in every `Created` echo.
    pub caps: HostCaps,
    /// Device pixel ratio reported for auto-answered spawned windows.
    pub dpr: f64,
    /// The paint recorder of the last `advance` (draw ops are inspectable
    /// through it; it is reused, so it accumulates — call
    /// [`RecordingRenderContext`] accessors per test section).
    pub ctx: RecordingRenderContext,
    last: TickOutput,
    command_log: Vec<(WindowId, WindowCommand)>,
}

impl<S: Spec, A: App<Spec = S>> HeadlessHost<S, A> {
    /// A host and the app's handle, with a no-op waker (design §9.1).
    pub fn new(app: A, cfg: RuntimeConfig) -> (Self, Handle<S>) {
        let (rt, handle) = Runtime::new(app, cfg, Waker::noop());
        let host = Self {
            rt,
            handle: handle.clone(),
            now: Seconds::ZERO,
            caps: HostCaps {
                multi_window: true,
                os_resize_bezel: false,
                clipboard_async: false,
            },
            dpr: 1.0,
            ctx: RecordingRenderContext::new(),
            last: TickOutput::default(),
            command_log: Vec::new(),
        };
        (host, handle)
    }

    /// Open a window and run the ticks that settle its creation (the
    /// `Spawn` command is auto-answered with a `Created` echo carrying
    /// `size` and `dpr`). Returns the engine-assigned id.
    pub fn open_window(&mut self, size: SizePx, dpr: f64) -> WindowId {
        let n = self.snapshot().windows.len();
        let key = format!("win-{n}");
        let mut spec = WindowSpec::new(key, "headless", size);
        let _ = &mut spec; // spec travels whole; dpr rides the echo.
        self.handle
            .dispatch(AppCommand::Window(WindowCmd::Open(spec)))
            .unwrap_or_else(|e| panic!("headless inbox: {e}"));
        self.dpr = dpr;
        self.advance(0.0);
        self.snapshot()
            .windows
            .last()
            .unwrap_or_else(|| panic!("window did not open"))
            .id
    }

    /// Queue one input event for the next tick (hop 1 of §4.4).
    pub fn input(&mut self, win: WindowId, event: InputEvent) {
        self.rt.push_input(InputEnvelope {
            window: win,
            t: self.now,
            event,
        });
    }

    /// Move the host clock by `dt` seconds, run one tick (plus one
    /// settling tick when the tick's host-bound commands needed an echo
    /// answer), and paint every due frame request. The output of the whole
    /// advance is kept for [`HeadlessHost::window_commands`].
    pub fn advance(&mut self, dt: f64) -> &TickOutput {
        self.now = self.now.after(Seconds(dt));
        let mut out = self.rt.tick(self.now);

        // Answer the commands only an OS could: spawns get created with
        // the host's caps, closes get destroyed.
        let mut echoes = Vec::new();
        for (win, cmd) in &out.window_commands {
            match cmd {
                WindowCommand::Spawn(spec) => echoes.push((
                    *win,
                    WindowInput::Created {
                        size: spec.inner_size,
                        dpr: self.dpr,
                        // A known position: expand gutters and drag-out
                        // need a known outer rect.
                        position: Some((100, 100)),
                        caps: self.caps,
                    },
                )),
                WindowCommand::Close => echoes.push((*win, WindowInput::Destroyed)),
                _ => {}
            }
        }
        if !echoes.is_empty() {
            for (win, event) in echoes {
                self.rt.push_input(InputEnvelope {
                    window: win,
                    t: self.now,
                    event: InputEvent::Window(event),
                });
            }
            let more = self.rt.tick(self.now);
            out.window_commands.extend(more.window_commands);
            out.frames.extend(more.frames);
            out.wake = earliest(out.wake, more.wake);
        }

        let frames = std::mem::take(&mut out.frames);
        for req in &frames {
            self.rt.paint(req, &mut self.ctx);
        }
        out.frames = frames;
        self.command_log.extend(out.window_commands.iter().cloned());
        self.last = out;
        &self.last
    }

    /// The last published snapshot.
    pub fn snapshot(&self) -> Arc<VisualSnapshot<S>> {
        self.rt.snapshot()
    }

    /// The host-bound window commands of the last `advance`.
    pub fn window_commands(&self) -> &[(WindowId, WindowCommand)] {
        &self.last.window_commands
    }

    /// Every host-bound window command since construction, in order (the
    /// executor's own log — assertions over a whole script read this, not
    /// the last tick's slice).
    pub fn command_log(&self) -> &[(WindowId, WindowCommand)] {
        &self.command_log
    }

    /// The runtime (tests drive app-level tools through it).
    pub fn runtime(&self) -> &Runtime<S, A> {
        &self.rt
    }

    /// The runtime, mutably.
    pub fn runtime_mut(&mut self) -> &mut Runtime<S, A> {
        &mut self.rt
    }
}

/// The sooner of two wake hints (`Immediate` < `At(t)` < `Idle`).
fn earliest(
    a: crate::types::frame::Wake,
    b: crate::types::frame::Wake,
) -> crate::types::frame::Wake {
    use crate::types::frame::Wake;
    match (a, b) {
        (Wake::Immediate, _) | (_, Wake::Immediate) => Wake::Immediate,
        (Wake::At(x), Wake::At(y)) => Wake::At(if x.get() <= y.get() { x } else { y }),
        (Wake::At(x), Wake::Idle) | (Wake::Idle, Wake::At(x)) => Wake::At(x),
        (Wake::Idle, Wake::Idle) => Wake::Idle,
    }
}
