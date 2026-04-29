//! Fluent builder for constructing and launching an uzor app runtime.

use uzor::docking::panels::DockPanel;
use uzor_render_hub::RenderBackend;
use uzor_window_hub::lifecycle::WindowProvider;

use crate::app::{App, AppConfig, ClosureApp, NoPanel};
use crate::runtime::{Runtime, RuntimeError};

// ── BuildError ────────────────────────────────────────────────────────────────

/// Errors that can occur when calling [`AppBuilder::build`].
#[derive(Debug)]
pub enum BuildError {
    /// No window provider was supplied via [`AppBuilder::window`].
    MissingWindow,
    /// No render backend was supplied via [`AppBuilder::backend`].
    MissingBackend,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::MissingWindow => {
                f.write_str("no window provider supplied — call .window(...)")
            }
            BuildError::MissingBackend => {
                f.write_str("no render backend supplied — call .backend(...)")
            }
        }
    }
}

impl std::error::Error for BuildError {}

impl From<BuildError> for RuntimeError {
    fn from(e: BuildError) -> Self {
        RuntimeError::Build(e)
    }
}

// ── AppBuilder ────────────────────────────────────────────────────────────────

/// Fluent builder for configuring and launching an uzor app.
///
/// # Typical usage
///
/// ```rust,ignore
/// use uzor_framework::{AppBuilder, AppConfig};
///
/// AppBuilder::new(MyApp::new())
///     .title("my app")
///     .size(1280, 720)
///     .backend(RenderBackend::VelloGpu)
///     .window(my_window_provider)
///     .run()
///     .expect("runtime error");
/// ```
///
/// # Generic parameters
///
/// - `A` — the app struct that implements [`App<P>`].
/// - `P` — the dock-panel type. Defaults to [`NoPanel`] for apps without
///   dockable panels.
pub struct AppBuilder<A, P = NoPanel>
where
    A: App<P>,
    P: DockPanel,
{
    app: A,
    config: AppConfig,
    window: Option<Box<dyn WindowProvider>>,
    backend: Option<RenderBackend>,
    _phantom: std::marker::PhantomData<P>,
}

impl<A, P> AppBuilder<A, P>
where
    A: App<P>,
    P: DockPanel + Default + Clone + 'static,
{
    /// Create a builder wrapping `app`.
    ///
    /// A default [`AppConfig`] is applied; override individual fields with the
    /// chaining helpers below.
    pub fn new(app: A) -> Self {
        Self {
            app,
            config: AppConfig::default(),
            window: None,
            backend: None,
            _phantom: std::marker::PhantomData,
        }
    }

    // ── Configuration setters ─────────────────────────────────────────────────

    /// Replace the entire [`AppConfig`] at once.
    pub fn config(mut self, config: AppConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the window title.
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.config.title = t.into();
        self
    }

    /// Set the initial logical window size.
    pub fn size(mut self, w: u32, h: u32) -> Self {
        self.config.initial_size = (w, h);
        self
    }

    /// Set the minimum logical window size. Pass `None` to remove the minimum.
    pub fn min_size(mut self, min: Option<(u32, u32)>) -> Self {
        self.config.min_size = min;
        self
    }

    /// Enable or disable OS-native window decorations.
    pub fn decorations(mut self, on: bool) -> Self {
        self.config.decorations = on;
        self
    }

    /// Enable or disable multi-window support.
    pub fn multi_window(mut self, on: bool) -> Self {
        self.config.multi_window = on;
        self
    }

    /// Set the FPS limit (`0` = unlimited).
    pub fn fps_limit(mut self, fps: u32) -> Self {
        self.config.fps_limit = fps;
        self
    }

    /// Set the clear colour as `0xAARRGGBB`.
    pub fn background(mut self, argb: u32) -> Self {
        self.config.background = argb;
        self
    }

    /// Enable VSync (default: true).
    pub fn vsync(mut self, on: bool) -> Self {
        self.config.vsync = on;
        self
    }

    /// Enforce single-instance via a Win32 named mutex.
    ///
    /// Pass the mutex name; `None` disables the guard.
    pub fn single_instance(mut self, name: Option<impl Into<String>>) -> Self {
        self.config.single_instance = name.map(Into::into);
        self
    }

    /// Set the Windows 11 DWM border colour (`"#RRGGBB"`).
    pub fn dwm_border_color(mut self, color: Option<impl Into<String>>) -> Self {
        self.config.dwm_border_color = color.map(Into::into);
        self
    }

    // ── Infrastructure setters ────────────────────────────────────────────────

    /// Supply the window provider (e.g. from `uzor-window-desktop`).
    ///
    /// Required — [`build`](Self::build) returns [`BuildError::MissingWindow`]
    /// if this is not called.
    pub fn window(mut self, provider: Box<dyn WindowProvider>) -> Self {
        self.window = Some(provider);
        self
    }

    /// Select the rendering backend.
    ///
    /// Required — [`build`](Self::build) returns [`BuildError::MissingBackend`]
    /// if this is not called.
    pub fn backend(mut self, backend: RenderBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    // ── Terminal methods ──────────────────────────────────────────────────────

    /// Consume the builder and produce a [`Runtime`] ready to run.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::MissingWindow`] or [`BuildError::MissingBackend`]
    /// if the required infrastructure was not supplied.
    pub fn build(self) -> Result<Runtime<A, P>, BuildError> {
        let window = self.window.ok_or(BuildError::MissingWindow)?;
        let backend = self.backend.ok_or(BuildError::MissingBackend)?;
        Ok(Runtime::new(self.app, self.config, window, backend))
    }

    /// Consume the builder, construct the runtime, and run the event loop.
    ///
    /// Blocks until all windows are closed.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Build`] if a required parameter is missing, or
    /// a [`RuntimeError::Window`] / [`RuntimeError::Backend`] variant on
    /// runtime failures.
    pub fn run(self) -> Result<(), RuntimeError> {
        self.build()?.run()
    }
}

// ── Convenience: run_closure ──────────────────────────────────────────────────

/// Quick prototype helper — build and run an app from a closure.
///
/// For fully-featured apps use [`AppBuilder::new`] with a concrete [`App`]
/// implementation instead.
///
/// # Example
///
/// ```rust,ignore
/// uzor_framework::run_closure(
///     |layout, render| { /* draw something */ },
///     AppConfig::default(),
///     window_provider,
///     RenderBackend::VelloGpu,
/// ).expect("runtime error");
/// ```
pub fn run_closure<P, F>(
    ui: F,
    config: AppConfig,
    window: Box<dyn WindowProvider>,
    backend: RenderBackend,
) -> Result<(), RuntimeError>
where
    P: DockPanel + Default + Clone + Send + Sync + 'static,
    F: FnMut(&mut uzor::layout::LayoutManager<P>, &mut uzor_render_hub::WindowRenderState)
        + 'static,
{
    AppBuilder::new(ClosureApp::<P, F>::new(ui))
        .config(config)
        .window(window)
        .backend(backend)
        .run()
}
