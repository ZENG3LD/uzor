//! Event-loop runner for uzor apps.
//!
//! [`Runtime`] owns the per-app state and exposes a [`tick`](Runtime::tick)
//! method that processes one frame's work.  The winit event loop is driven
//! externally (by [`crate::builder::AppBuilder::run`]) via an internal
//! `ApplicationHandler` that calls `tick` on every `RedrawRequested` event.
//!
//! # Frame lifecycle
//!
//! ```text
//! resumed → create_window → VelloGpuSurfaceFactory::create_render_state
//!         → loop {
//!               window_event(RedrawRequested) → Runtime::tick()
//!                   → poll_events → solve layout → begin_frame
//!                   → app.ui       → submit_frame → request_redraw
//!           }
//!         → CloseRequested → exit
//! ```

use uzor::docking::panels::DockPanel;
use uzor::input::core::event_processor::EventProcessor;
use uzor::input::InputState;
use uzor::layout::LayoutManager;
use uzor_render_hub::{
    RenderBackend, RenderSurfaceFactory, SurfaceSize, WindowRenderState,
    submit_frame, SubmitParams,
};
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
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Build(e) => write!(f, "build error: {e}"),
            RuntimeError::Window(s) => write!(f, "window provider error: {s}"),
            RuntimeError::Backend(s) => write!(f, "backend error: {s}"),
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

/// App runtime that owns per-app state and drives the frame loop.
///
/// Constructed by [`crate::builder::AppBuilder::build`].  The actual winit
/// event loop is owned by `AppBuilder::run()` — `Runtime` only contains the
/// application-level state and exposes [`tick`](Runtime::tick) for the handler
/// to call each frame.
pub struct Runtime<A: App<P>, P: DockPanel> {
    /// User application.
    pub(crate) app: A,
    /// Runtime configuration.
    pub(crate) config: AppConfig,
    /// Backend selection.
    pub(crate) backend: RenderBackend,
    /// Surface factory (held until `init_render_state` is called).
    pub(crate) factory: Option<Box<dyn RenderSurfaceFactory>>,
    /// Layout manager.
    pub(crate) layout: LayoutManager<P>,
    /// Platform-event processor.
    pub(crate) events: EventProcessor,
    /// Accumulated per-frame input snapshot.
    pub(crate) input: InputState,
    /// Monotonic start instant for `InputState::time` tracking.
    pub(crate) start: std::time::Instant,
    /// Whether `app.init` has been called.
    pub(crate) initialised: bool,
    /// Ready render state (set once the window + surface are available).
    pub(crate) render_state: Option<WindowRenderState>,
}

impl<A: App<P>, P: DockPanel + Default + 'static> Runtime<A, P> {
    /// Construct the runtime.
    ///
    /// Prefer [`crate::builder::AppBuilder::build`] over calling this directly.
    pub fn new(app: A, config: AppConfig, backend: RenderBackend) -> Self {
        Self {
            app,
            config,
            backend,
            factory: None,
            layout: LayoutManager::new(),
            events: EventProcessor::new(),
            input: InputState::new(),
            start: std::time::Instant::now(),
            initialised: false,
            render_state: None,
        }
    }

    /// Attach a [`RenderSurfaceFactory`] that will wire the GPU surface when
    /// [`init_render_state`](Runtime::init_render_state) is called.
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

    /// Initialise the render state from a live window provider.
    ///
    /// Called once from the `ApplicationHandler::resumed` / `window_created`
    /// path after the window has been created and a valid raw handle is
    /// available.  Safe to call more than once; subsequent calls are no-ops if
    /// the render state is already initialised.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Backend`] if the factory fails to create the
    /// surface (e.g. GPU not available), or [`RuntimeError::Window`] if the
    /// window handle is not yet available.
    pub fn init_render_state(
        &mut self,
        window: &dyn WindowProvider,
    ) -> Result<(), RuntimeError> {
        if self.render_state.is_some() {
            return Ok(());
        }

        // Run app init the first time.
        if !self.initialised {
            self.app.init(&mut self.layout);
            self.initialised = true;
        }

        let factory = self
            .factory
            .as_ref()
            .ok_or_else(|| RuntimeError::Backend("no surface factory supplied".into()))?;

        let raw_handle = window
            .raw_window_handle()
            .ok_or_else(|| RuntimeError::Window("no raw handle available".into()))?;

        let rect = window.window_rect();
        let dpr = window.scale_factor();
        let size = SurfaceSize {
            width: (rect.width * dpr).max(1.0) as u32,
            height: (rect.height * dpr).max(1.0) as u32,
        };

        let state = factory
            .create_render_state(&raw_handle, self.backend, size)
            .map_err(|e| RuntimeError::Backend(e.to_string()))?;

        self.render_state = Some(state);
        Ok(())
    }

    /// Process one frame: poll events, solve layout, begin/ui/submit.
    ///
    /// Called from the winit `ApplicationHandler::window_event(RedrawRequested)`
    /// path.  Does nothing if the render state has not yet been initialised.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Backend`] if GPU submission signals a lost
    /// surface.
    pub fn tick(&mut self, window: &mut dyn WindowProvider) -> Result<(), RuntimeError> {
        let render_state = match self.render_state.as_mut() {
            Some(s) => s,
            None => return Ok(()),
        };

        // 1. Drain OS events → feed into EventProcessor → accumulate InputState.
        let now_secs = self.start.elapsed().as_secs_f64();
        self.input.time = now_secs;

        for ev in window.poll_events() {
            if !self.app.on_event(&ev) {
                self.events.process(&ev, &mut self.input, now_secs);
            }
        }

        // 2. Solve macro layout with the current window rect.
        let rect = window.window_rect();
        self.layout.solve(rect);

        // 3. ContextManager begin_frame.
        let viewport = self.layout.rect_for_dock_area().unwrap_or(rect);
        self.layout.ctx_mut().begin_frame(self.input.clone(), viewport);

        // 4. begin_frame on render state (resets scene / cpu buffer).
        render_state.begin_frame();

        // 5. User UI callback.
        self.app.ui(&mut self.layout, render_state);

        // 6. Collect widget responses.
        let _responses = self.layout.ctx_mut().end_frame();

        // 7. GPU / CPU submit.
        let base_color = argb_to_alpha_color(self.config.background);
        let outcome = submit_frame(
            render_state,
            SubmitParams {
                base_color,
                msaa_samples: self.config.msaa_samples,
            },
        );
        if outcome.surface_lost {
            return Err(RuntimeError::Backend("wgpu surface lost (out of memory)".into()));
        }

        // 8. Reset per-frame fields.
        self.input.pointer.clicked = None;
        self.input.pointer.double_clicked = None;
        self.input.scroll_delta = (0.0, 0.0);

        // 9. Request next redraw.
        window.request_redraw();

        Ok(())
    }

    /// Shut down the application.  Called once after the event loop exits.
    pub fn shutdown(&mut self) {
        self.app.shutdown(&mut self.layout);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a packed `0xAARRGGBB` colour into a vello `AlphaColor<Srgb>`.
fn argb_to_alpha_color(argb: u32) -> vello::peniko::color::AlphaColor<vello::peniko::color::Srgb> {
    let a = ((argb >> 24) & 0xFF) as f32 / 255.0;
    let r = ((argb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((argb >>  8) & 0xFF) as f32 / 255.0;
    let b = ( argb        & 0xFF) as f32 / 255.0;
    vello::peniko::color::AlphaColor::new([r, g, b, a])
}
