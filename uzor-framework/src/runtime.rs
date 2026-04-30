//! Event-loop runner for uzor apps.
//!
//! [`Runtime`] owns the per-app state and drives the frame loop:
//!
//! ```text
//! init → loop { poll_events → solve layout → begin_frame → ui → submit } → shutdown
//! ```
//!
//! # Surface wiring TODO
//!
//! The render path (`begin_frame` / `submit_frame`) requires a fully-initialised
//! [`uzor_render_hub::WindowRenderState`], which in turn needs a vello
//! `Renderer` and a `RenderSurface` tied to the native window handle.
//!
//! Creating those requires a concrete winit `Window` (or equivalent), which is
//! not available through the abstract [`WindowProvider`] trait (the trait only
//! exposes [`RawHandle`]).
//!
//! The surface wiring will be completed in `uzor-window-desktop` once that crate
//! gains a factory function:
//!
//! ```rust,ignore
//! // Future API in uzor-window-desktop (or uzor-framework):
//! fn build_render_state(
//!     window: &winit::window::Window,
//!     render_cx: &mut vello::util::RenderContext,
//!     backend: RenderBackend,
//! ) -> Result<(WindowRenderState, RenderSurface<'static>), RuntimeError>
//! ```
//!
//! Until then, the `run()` method is structurally complete but returns
//! `RuntimeError::SurfaceWiringRequired` immediately.

use uzor::docking::panels::DockPanel;
use uzor::input::core::event_processor::EventProcessor;
use uzor::input::InputState;
use uzor::layout::LayoutManager;
use uzor_render_hub::{RenderBackend, RenderSurfaceFactory, SurfaceSize};
use uzor_window_hub::lifecycle::WindowProvider;

use crate::app::{App, AppConfig};
use crate::builder::BuildError;

// ── RuntimeError ──────────────────────────────────────────────────────────────

/// Errors emitted by the uzor app runtime.
#[derive(Debug)]
pub enum RuntimeError {
    /// A required parameter was not supplied to [`crate::builder::AppBuilder`].
    Build(BuildError),
    /// A window-provider error (e.g. OS window creation failed).
    Window(String),
    /// A render-backend error (e.g. GPU initialisation failed).
    Backend(String),
    /// GPU surface wiring is not yet implemented for the chosen window
    /// provider. See the module-level documentation for the planned API.
    SurfaceWiringRequired,
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Build(e) => write!(f, "build error: {e}"),
            RuntimeError::Window(s) => write!(f, "window provider error: {s}"),
            RuntimeError::Backend(s) => write!(f, "backend error: {s}"),
            RuntimeError::SurfaceWiringRequired => f.write_str(
                "GPU surface wiring is not yet implemented — \
                 use uzor-window-desktop's concrete runtime instead",
            ),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RuntimeError::Build(e) => Some(e),
            _ => None,
        }
    }
}

// ── Runtime ───────────────────────────────────────────────────────────────────

/// App runtime that owns the event loop and all per-window state.
///
/// Constructed by [`crate::builder::AppBuilder::build`]; started by calling
/// [`Runtime::run`].
pub struct Runtime<A: App<P>, P: DockPanel> {
    app: A,
    config: AppConfig,
    window: Box<dyn WindowProvider>,
    /// Backend selection. Used when the surface factory wires the concrete
    /// [`uzor_render_hub::WindowRenderState`] at startup.
    backend: RenderBackend,
    /// Optional factory that converts a [`uzor_window_hub::RawHandle`] +
    /// [`RenderBackend`] into a [`uzor_render_hub::WindowRenderState`].
    ///
    /// When `None`, [`Runtime::run`] returns
    /// [`RuntimeError::SurfaceWiringRequired`] (no factory supplied).
    factory: Option<Box<dyn RenderSurfaceFactory>>,
    layout: LayoutManager<P>,
    events: EventProcessor,
    /// Accumulated per-frame input snapshot.
    input: InputState,
    /// Monotonic start instant for `InputState::time` tracking.
    start: std::time::Instant,
}

impl<A: App<P>, P: DockPanel + Default + 'static> Runtime<A, P> {
    /// Construct the runtime.
    ///
    /// Prefer [`crate::builder::AppBuilder::build`] over calling this directly.
    /// To wire GPU surface creation, call
    /// [`set_surface_factory`](Self::set_surface_factory) after construction or
    /// supply a factory via [`crate::builder::AppBuilder::surface_factory`].
    pub fn new(
        app: A,
        config: AppConfig,
        window: Box<dyn WindowProvider>,
        backend: RenderBackend,
    ) -> Self {
        Self {
            app,
            config,
            window,
            backend,
            factory: None,
            layout: LayoutManager::new(),
            events: EventProcessor::new(),
            input: InputState::new(),
            start: std::time::Instant::now(),
        }
    }

    /// Attach a [`RenderSurfaceFactory`] that will wire the GPU surface on
    /// [`run`](Self::run).
    ///
    /// Without a factory, `run()` returns
    /// [`RuntimeError::SurfaceWiringRequired`].
    pub fn set_surface_factory(&mut self, factory: Box<dyn RenderSurfaceFactory>) {
        self.factory = Some(factory);
    }

    /// Returns the selected [`RenderBackend`].
    pub fn backend(&self) -> RenderBackend {
        self.backend
    }

    /// Returns a reference to the [`AppConfig`] this runtime was built with.
    pub fn app_config(&self) -> &AppConfig {
        &self.config
    }

    /// Run the event loop, blocking until all windows close.
    ///
    /// # Current status
    ///
    /// The loop structure is fully implemented. GPU submission requires a
    /// concrete `WindowRenderState` + `RenderSurface`, which depend on a
    /// native window handle not yet exposed through the abstract
    /// [`WindowProvider`] trait. This method currently returns
    /// [`RuntimeError::SurfaceWiringRequired`].
    ///
    /// The concrete platform runtime in `uzor-window-desktop` will provide a
    /// `run_desktop()` function that bypasses this trait-object limitation by
    /// working directly with `winit::Window`.
    pub fn run(mut self) -> Result<(), RuntimeError> {
        // ── Single-instance guard ─────────────────────────────────────────────
        // Hold the guard for the process lifetime (Option::take prevents Drop
        // from running on the guard while we still hold self).
        let _single_instance_guard = self
            .config
            .single_instance
            .as_deref()
            .map(crate::utils::single_instance::single_instance);

        // ── Init phase ────────────────────────────────────────────────────────
        self.app.init(&mut self.layout);

        // ── Surface wiring ────────────────────────────────────────────────────
        // Require a RenderSurfaceFactory (supplied via AppBuilder::surface_factory
        // or Runtime::set_surface_factory). Without one, the surface cannot be
        // initialized and we return an honest error.
        let factory = match self.factory.as_ref() {
            Some(f) => f,
            None => return Err(RuntimeError::SurfaceWiringRequired),
        };

        let raw_handle = self
            .window
            .raw_window_handle()
            .ok_or_else(|| RuntimeError::Window("no raw handle".into()))?;

        let rect = self.window.window_rect();
        let dpr = self.window.scale_factor();
        let size = SurfaceSize {
            width:  (rect.width  * dpr) as u32,
            height: (rect.height * dpr) as u32,
        };

        let _render_state = factory
            .create_render_state(&raw_handle, self.backend, size)
            .map_err(|e| RuntimeError::Backend(e.to_string()))?;

        // ── Main loop (structurally complete, unreachable until wiring done) ──
        #[allow(unreachable_code)]
        {
            let _dt_start = std::time::Instant::now();

            while !self.window.should_close() {
                // 1. Drain OS events → feed into EventProcessor → accumulate InputState.
                let now_secs = self.start.elapsed().as_secs_f64();
                self.input.time = now_secs;

                for ev in self.window.poll_events() {
                    if !self.app.on_event(&ev) {
                        self.events.process(&ev, &mut self.input, now_secs);
                    }
                }

                // 2. Solve macro layout with the current window rect.
                let rect = self.window.window_rect();
                self.layout.solve(rect);

                // 3. ContextManager begin_frame.
                //    Viewport defaults to the dock area; falls back to the full window.
                let viewport = self.layout.rect_for_dock_area().unwrap_or(rect);
                self.layout
                    .ctx_mut()
                    .begin_frame(self.input.clone(), viewport);

                // 4. begin_frame on render state.
                // TODO(surface-wiring): render_state.begin_frame();

                // 5. User UI callback.
                // TODO(surface-wiring): self.app.ui(&mut self.layout, &mut render_state);

                // 6. Collect widget responses.
                let _responses = self.layout.ctx_mut().end_frame();

                // 7. GPU submit.
                // TODO(surface-wiring):
                //   let outcome = render_state.submit_frame(SubmitParams { ... });
                //   if outcome.surface_lost { break; }

                // 8. First-frame reveal.
                // TODO(surface-wiring): window.set_visible(true) after first present.

                // 9. Reset per-frame fields.
                self.input.pointer.clicked = None;
                self.input.pointer.double_clicked = None;
                self.input.scroll_delta = (0.0, 0.0);

                // 10. Request next redraw.
                self.window.request_redraw();

                // 11. FPS cap.
                if self.config.fps_limit > 0 {
                    let frame_budget =
                        std::time::Duration::from_secs_f64(1.0 / self.config.fps_limit as f64);
                    let elapsed = _dt_start.elapsed();
                    if let Some(remaining) = frame_budget.checked_sub(elapsed) {
                        std::thread::sleep(remaining);
                    }
                }
            }

            // ── Shutdown ──────────────────────────────────────────────────────
            self.app.shutdown(&mut self.layout);
            Ok(())
        }
    }
}
