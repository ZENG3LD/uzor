//! Fluent builder for constructing and launching an uzor app runtime.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use uzor::docking::panels::DockPanel;
use uzor_render_hub::{RenderBackend, RenderSurfaceFactory};
use uzor_window_hub::{WinitWindowProvider, WindowProvider};

use uzor_window_hub::RgbaIcon;

use crate::app::{App, AppConfig, ClosureApp, NoPanel};
use crate::runtime::{Runtime, RuntimeError};

// ── BuildError ────────────────────────────────────────────────────────────────

/// Errors that can occur when calling [`AppBuilder::build`] or icon helpers.
#[derive(Debug)]
pub enum BuildError {
    /// No render backend was supplied via [`AppBuilder::backend`].
    MissingBackend,
    /// PNG icon bytes could not be decoded or converted to RGBA8.
    IconDecode(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::MissingBackend => {
                f.write_str("no render backend supplied — call .backend(...)")
            }
            BuildError::IconDecode(msg) => {
                write!(f, "icon PNG decode failed: {msg}")
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
/// use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory};
///
/// AppBuilder::new(MyApp::new())
///     .title("my app")
///     .size(1280, 720)
///     .backend(RenderBackend::VelloGpu)
///     .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
///     .run()
///     .expect("runtime error");
/// ```
///
/// # Window creation
///
/// `AppBuilder::run()` creates the winit `EventLoop` and `Window` internally.
/// No `.window(...)` call is required.  If you need full control over the event
/// loop (e.g. for custom drag regions or tray icon integration), construct the
/// event loop yourself and call `Runtime::tick()` manually.
///
/// # Generic parameters
///
/// - `A` — the app struct that implements [`App<P>`].
/// - `P` — the dock-panel type.  Defaults to [`NoPanel`] for apps without
///   dockable panels.
pub struct AppBuilder<A, P = NoPanel>
where
    A: App<P>,
    P: DockPanel,
{
    app: A,
    config: AppConfig,
    backend: Option<RenderBackend>,
    factory: Option<Box<dyn RenderSurfaceFactory>>,
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
            backend: None,
            factory: None,
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

    /// Set the minimum logical window size.  Pass `None` to remove the minimum.
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

    /// Set the window icon from a pre-built [`RgbaIcon`].
    ///
    /// The icon is applied to the OS window at creation time (taskbar,
    /// Alt-Tab, window caption).
    pub fn icon(mut self, icon: RgbaIcon) -> Self {
        self.config.icon = Some(icon);
        self
    }

    /// Set the window icon by decoding a PNG byte slice.
    ///
    /// Decodes the PNG, converts to RGBA, and stores as an [`RgbaIcon`].
    ///
    /// # Errors
    ///
    /// Returns `Err(BuildError::IconDecode)` if the bytes are not valid PNG or
    /// if the decoded image cannot be converted to RGBA8.
    pub fn icon_from_png(mut self, png_bytes: &[u8]) -> Result<Self, BuildError> {
        use image::ImageDecoder;
        use std::io::Cursor;

        let decoder = image::codecs::png::PngDecoder::new(Cursor::new(png_bytes))
            .map_err(|e| BuildError::IconDecode(e.to_string()))?;

        let (width, height) = decoder.dimensions();
        let total_bytes = decoder.total_bytes() as usize;

        // Decode the raw bytes and convert to RGBA8.
        let mut raw = vec![0u8; total_bytes];
        decoder
            .read_image(&mut raw)
            .map_err(|e| BuildError::IconDecode(e.to_string()))?;

        // Ensure we have RGBA8; if the PNG is RGB or palette, convert.
        let rgba: Vec<u8> = if total_bytes == (width * height * 4) as usize {
            raw
        } else {
            // Re-decode via DynamicImage for format conversion.
            let img = image::load_from_memory_with_format(
                png_bytes,
                image::ImageFormat::Png,
            )
            .map_err(|e| BuildError::IconDecode(e.to_string()))?;
            img.into_rgba8().into_raw()
        };

        self.config.icon = Some(RgbaIcon::from_rgba(width, height, rgba));
        Ok(self)
    }

    // ── Infrastructure setters ────────────────────────────────────────────────

    /// Select the rendering backend.
    ///
    /// Required — [`build`](Self::build) returns [`BuildError::MissingBackend`]
    /// if this is not called.
    pub fn backend(mut self, backend: RenderBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Supply a [`RenderSurfaceFactory`] that converts the window handle into a
    /// [`uzor_render_hub::WindowRenderState`] at startup.
    pub fn surface_factory(mut self, factory: Box<dyn RenderSurfaceFactory>) -> Self {
        self.factory = Some(factory);
        self
    }

    // ── Terminal methods ──────────────────────────────────────────────────────

    /// Consume the builder and produce a [`Runtime`] ready to run.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::MissingBackend`] if no backend was supplied.
    pub fn build(self) -> Result<Runtime<A, P>, BuildError> {
        let backend = self.backend.ok_or(BuildError::MissingBackend)?;
        let mut runtime = Runtime::new(self.app, self.config, backend);
        if let Some(factory) = self.factory {
            runtime.set_surface_factory(factory);
        }
        Ok(runtime)
    }

    /// Consume the builder, create the winit event loop, and run until the
    /// window is closed.
    ///
    /// Blocks until all windows close.  Window creation is handled internally;
    /// no `.window(...)` call is needed.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Build`] if a required parameter is missing, a
    /// [`RuntimeError::Window`] variant if window or event-loop creation fails,
    /// or [`RuntimeError::Backend`] on GPU initialisation failure.
    pub fn run(self) -> Result<(), RuntimeError> {
        let config = self.config.clone();
        let runtime = self.build()?;

        // ── Single-instance guard ─────────────────────────────────────────────
        let _single_instance_guard = runtime
            .config
            .single_instance
            .as_deref()
            .map(crate::utils::single_instance::single_instance);

        let event_loop = EventLoop::new()
            .map_err(|e| RuntimeError::Window(e.to_string()))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut handler = UzorHandler {
            runtime,
            config,
            provider: None,
        };

        event_loop
            .run_app(&mut handler)
            .map_err(|e| RuntimeError::Window(e.to_string()))?;

        Ok(())
    }
}

// ── Internal ApplicationHandler ───────────────────────────────────────────────

/// Winit `ApplicationHandler` that drives the uzor runtime.
///
/// Created inside [`AppBuilder::run`] and not part of the public API.
struct UzorHandler<A: App<P>, P: DockPanel + Default + 'static> {
    runtime: Runtime<A, P>,
    config: AppConfig,
    provider: Option<WinitWindowProvider>,
}

impl<A, P> ApplicationHandler for UzorHandler<A, P>
where
    A: App<P>,
    P: DockPanel + Default + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.provider.is_some() {
            // Already have a window; nothing to do on subsequent resumes.
            return;
        }

        let (w, h) = self.config.initial_size;
        let mut attrs = Window::default_attributes()
            .with_title(&self.config.title)
            .with_inner_size(winit::dpi::LogicalSize::new(w, h))
            .with_decorations(self.config.decorations)
            .with_visible(false); // revealed after first GPU frame

        // Apply window icon at creation time when one is configured.
        if let Some(ref rgba) = self.config.icon {
            if let Ok(ic) = winit::window::Icon::from_rgba(
                rgba.pixels.clone(),
                rgba.width,
                rgba.height,
            ) {
                attrs = attrs.with_window_icon(Some(ic));
            }
        }

        if let Some((mw, mh)) = self.config.min_size {
            attrs = attrs.with_min_inner_size(winit::dpi::LogicalSize::new(mw, mh));
        }

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("[uzor-framework] window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };

        let mut provider = WinitWindowProvider::new(Arc::clone(&window));

        // Initialise GPU surface immediately while we have the handle.
        if let Err(e) = self.runtime.init_render_state(&provider) {
            eprintln!("[uzor-framework] render state init failed: {e}");
            event_loop.exit();
            return;
        }

        // Show the window after the first render state is ready.
        window.set_visible(true);

        // Schedule the first redraw.
        provider.request_redraw();

        self.provider = Some(provider);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ref mut provider) = self.provider else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                provider.mark_close();
                self.runtime.shutdown();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.runtime.tick(provider) {
                    eprintln!("[uzor-framework] tick error: {e}");
                    self.runtime.shutdown();
                    event_loop.exit();
                }
            }
            ref ev => {
                provider.push_winit_event(ev);
            }
        }
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
///     RenderBackend::VelloGpu,
///     Box::new(VelloGpuSurfaceFactory::new()),
/// ).expect("runtime error");
/// ```
pub fn run_closure<P, F>(
    ui: F,
    config: AppConfig,
    backend: RenderBackend,
    factory: Box<dyn RenderSurfaceFactory>,
) -> Result<(), RuntimeError>
where
    P: DockPanel + Default + Clone + Send + Sync + 'static,
    F: FnMut(&mut uzor::layout::LayoutManager<P>, &mut uzor_render_hub::WindowRenderState)
        + 'static,
{
    AppBuilder::new(ClosureApp::<P, F>::new(ui))
        .config(config)
        .backend(backend)
        .surface_factory(factory)
        .run()
}
