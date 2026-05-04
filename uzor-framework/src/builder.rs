//! Fluent builder for constructing and launching an uzor app runtime.

use uzor::docking::panels::DockPanel;
use uzor_render_hub::{RenderBackend, RenderHub, RenderSurfaceFactory};

use uzor_window_hub::RgbaIcon;

use crate::app::{App, AppConfig, ClosureApp, NoPanel};
use crate::multi_window::{WindowSpec, WindowKey};
use crate::window_manager::{WindowManager, WindowManagerError};

/// Compatibility alias — old name retained for users who imported the
/// previous types.  Prefer `WindowManagerError` going forward.
pub type RuntimeError = WindowManagerError;

// ── BuildError ────────────────────────────────────────────────────────────────

/// Errors that can occur when calling [`AppBuilder::build`] or icon helpers.
#[derive(Debug)]
pub enum BuildError {
    /// No render backend was supplied via [`AppBuilder::backend`] or
    /// [`AppBuilder::render_hub`].
    MissingBackend,
    /// PNG icon bytes could not be decoded or converted to RGBA8.
    IconDecode(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::MissingBackend => {
                f.write_str("no render backend supplied — call .backend(...) or .render_hub(...)")
            }
            BuildError::IconDecode(msg) => {
                write!(f, "icon PNG decode failed: {msg}")
            }
        }
    }
}

impl std::error::Error for BuildError {}

impl From<BuildError> for WindowManagerError {
    fn from(e: BuildError) -> Self {
        WindowManagerError::Build(e)
    }
}

// ── AppBuilder ────────────────────────────────────────────────────────────────

/// Fluent builder for configuring and launching an uzor app.
///
/// # Backend selection
///
/// Two modes:
///
/// - **Mode A — fixed backend**: call [`.backend(RenderBackend::VelloGpu)`](Self::backend)
///   and [`.surface_factory(...)`](Self::surface_factory).  Simple, zero adapter probe
///   cost, no live switching.
///
/// - **Mode B — hub autodetect**: call [`.render_hub(RenderHub::autodetect())`](Self::render_hub).
///   Full pool + live switching + metrics.  Pays a brief adapter probe at construction.
///
/// If both `.backend()` and `.render_hub()` are called, `.render_hub()` wins (last
/// call wins).
///
/// # Typical usage — Mode A
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
/// # Typical usage — Mode B
///
/// ```rust,ignore
/// use uzor_framework::AppBuilder;
/// use uzor_render_hub::RenderHub;
///
/// AppBuilder::new(MyApp::new())
///     .title("my app")
///     .size(1280, 720)
///     .render_hub(RenderHub::autodetect())
///     .run()
///     .expect("runtime error");
/// ```
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
    hub: Option<RenderHub>,
    /// Optional tray spec — if set, the builder spawns a system-tray icon
    /// using the same RGBA icon as the window.
    tray: Option<TraySpec>,
    /// Window specs queued by `.window(...)` — at least one is required.
    /// If the builder is started via the legacy single-window API
    /// (`.title(...).size(...).run()`), a default spec is synthesised
    /// from `config` at run-time.
    windows: Vec<WindowSpec>,
    _phantom: std::marker::PhantomData<P>,
}

/// Spec for a system-tray icon spawned automatically by the builder.
pub(crate) struct TraySpec {
    pub(crate) tooltip: Option<String>,
    pub(crate) items:   Vec<(String, String, bool)>, // (id, label, enabled)
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
            hub: None,
            tray: None,
            windows: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Queue a window for the manager to create at startup.
    ///
    /// Multi-window apps queue several specs.  Single-window apps can use
    /// `.title(...).size(...)` plus a single implicit window — if no
    /// `.window(...)` calls are made, the builder synthesises a default
    /// spec from `AppConfig` at run-time.
    pub fn window(mut self, spec: WindowSpec) -> Self {
        self.windows.push(spec);
        self
    }

    /// Spawn a system-tray icon (with the window icon) when the runtime starts.
    ///
    /// The tooltip is shown when the user hovers the tray icon.  Menu items
    /// are added in declaration order; click events are emitted as
    /// [`crate::tray::TrayEvent::MenuClick { id }`].  Without a window icon
    /// configured (`.icon` / `.icon_from_png`) the tray icon will use the
    /// system default.
    pub fn tray(mut self, tooltip: impl Into<String>) -> Self {
        self.tray = Some(TraySpec {
            tooltip: Some(tooltip.into()),
            items:   Vec::new(),
        });
        self
    }

    /// Add a tray-menu item.  Requires `.tray(tooltip)` called first.
    pub fn tray_item(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        if let Some(ref mut t) = self.tray {
            t.items.push((id.into(), label.into(), true));
        }
        self
    }

    /// Add a disabled (greyed-out) tray-menu item.  Requires `.tray` first.
    pub fn tray_item_disabled(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        if let Some(ref mut t) = self.tray {
            t.items.push((id.into(), label.into(), false));
        }
        self
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
    ///
    /// Convenience pass-through — also available via
    /// [`render_hub`](Self::render_hub) + [`RenderHub::set_fps_limit`].
    pub fn fps_limit(mut self, fps: u32) -> Self {
        self.config.fps_limit = fps;
        // Propagate to hub if one is already attached.
        if let Some(ref mut h) = self.hub {
            h.set_fps_limit(fps);
        }
        self
    }

    /// Set the MSAA sample count.
    ///
    /// Convenience pass-through — also available via
    /// [`render_hub`](Self::render_hub) + [`RenderHub::set_msaa`].
    pub fn msaa(mut self, samples: u8) -> Self {
        self.config.msaa_samples = samples;
        if let Some(ref mut h) = self.hub {
            h.set_msaa(samples);
        }
        self
    }

    /// Enable or disable VSync.
    ///
    /// Convenience pass-through — also available via
    /// [`render_hub`](Self::render_hub) + [`RenderHub::set_vsync`].
    pub fn vsync(mut self, on: bool) -> Self {
        self.config.vsync = on;
        if let Some(ref mut h) = self.hub {
            h.set_vsync(on);
        }
        self
    }

    /// Set the clear colour as `0xAARRGGBB`.
    pub fn background(mut self, argb: u32) -> Self {
        self.config.background = argb;
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
    /// Alt-Tab, window caption) and is reused by the system tray when
    /// `.tray(...)` is configured.
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

    /// Select the rendering backend (Mode A — fixed, no live switching).
    ///
    /// Required when not using [`render_hub`](Self::render_hub).
    /// [`build`](Self::build) returns [`BuildError::MissingBackend`] if
    /// neither `.backend()` nor `.render_hub()` is called.
    ///
    /// If `.render_hub()` was already called, this replaces the hub with a
    /// single-backend stub.  Use `.render_hub()` for the full hub experience.
    pub fn backend(mut self, backend: RenderBackend) -> Self {
        self.backend = Some(backend);
        // Clear hub so the explicit fixed backend takes precedence.
        self.hub = None;
        self
    }

    /// Supply a [`RenderSurfaceFactory`] that converts the window handle into a
    /// [`uzor_render_hub::WindowRenderState`] at startup.
    pub fn surface_factory(mut self, factory: Box<dyn RenderSurfaceFactory>) -> Self {
        self.factory = Some(factory);
        self
    }

    /// Attach a [`RenderHub`] (Mode B — autodetect + live switching + metrics).
    ///
    /// The hub's active backend, fps_limit, and msaa settings are propagated
    /// to the runtime config.  Any previously set `.backend()` is discarded
    /// (render_hub wins when both are called).
    pub fn render_hub(mut self, hub: RenderHub) -> Self {
        // Sync hub settings into config.
        self.config.fps_limit = hub.settings().fps_limit;
        self.config.msaa_samples = hub.settings().msaa_samples;
        self.config.vsync = hub.settings().vsync;
        // Store active backend for factory selection.
        self.backend = Some(hub.active());
        self.hub = Some(hub);
        self
    }

    // ── Terminal methods ──────────────────────────────────────────────────────

    /// Consume the builder and produce a fully-configured [`WindowManager`].
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::MissingBackend`] if no backend was supplied.
    pub fn build(mut self) -> Result<WindowManager<A, P>, BuildError> {
        let backend = self.backend.ok_or(BuildError::MissingBackend)?;

        // Synthesise a default window spec from AppConfig if the caller
        // didn't queue any explicit windows — keeps the single-window
        // builder API working unchanged.
        if self.windows.is_empty() {
            let default = WindowSpec::new(
                WindowKey::new("main"),
                if self.config.title.is_empty() { "uzor".to_string() }
                else { self.config.title.clone() },
            )
            .size(self.config.initial_size.0, self.config.initial_size.1)
            .decorations(self.config.decorations)
            .background(self.config.background);
            let default = if let Some((mw, mh)) = self.config.min_size {
                default.min_size(mw, mh)
            } else {
                default
            };
            self.windows.push(default);
        }

        let mut wm = WindowManager::new(self.app, self.config, backend, self.hub);
        if let Some(factory) = self.factory {
            wm.set_surface_factory(factory);
        }
        if let Some(tray) = self.tray {
            #[cfg(not(target_arch = "wasm32"))]
            wm.set_tray_spec(tray);
            #[cfg(target_arch = "wasm32")]
            { let _ = tray; }
        }
        #[cfg(not(target_arch = "wasm32"))]
        for spec in self.windows {
            wm.queue_window_spec(spec);
        }
        #[cfg(target_arch = "wasm32")]
        { let _ = self.windows; }
        Ok(wm)
    }

    /// Consume the builder and run the application.
    ///
    /// On **native** targets this creates a winit event loop and blocks until
    /// the window is closed.
    ///
    /// On **wasm32** targets this installs a `requestAnimationFrame` callback
    /// and returns `Ok(())` immediately — control returns to the browser's JS
    /// runtime.  The app ticks once per animation frame until `should_close()`
    /// returns `true`.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Build`] if a required parameter is missing, a
    /// [`RuntimeError::Window`] variant if window or event-loop creation fails,
    /// or [`RuntimeError::Backend`] on GPU initialisation failure.
    pub fn run(self) -> Result<(), WindowManagerError> {
        #[cfg(target_arch = "wasm32")]
        {
            return self.run_wasm();
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let wm = self.build()?;
            // Single-instance guard lives for the duration of the event loop.
            let _single_instance_guard = wm
                .config.single_instance.as_deref()
                .map(crate::utils::single_instance::single_instance);
            wm.run()
        }
    }

    /// wasm32 entry-point: install a RAF loop and return immediately.
    #[cfg(target_arch = "wasm32")]
    fn run_wasm(self) -> Result<(), RuntimeError> {
        use std::cell::RefCell;
        use std::rc::Rc;
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast as _;

        let mut runtime = self.build()?;

        // Obtain the WebWindowProvider from the canvas (the caller must have
        // configured a Canvas2d backend; we look up the canvas by the config
        // title used as the element ID, or fall back to "canvas").
        let canvas_id = if runtime.config.title.is_empty() {
            "canvas".to_string()
        } else {
            runtime.config.title.clone()
        };

        let mut provider = uzor_window_web::WebWindowProvider::from_id(&canvas_id)
            .map_err(|e| RuntimeError::Window(e))?;

        // Initialise the render state (Canvas2d factory creates it from the handle).
        runtime.init_render_state(&provider)
            .map_err(|e| RuntimeError::Backend(e.to_string()))?;

        // Wrap everything in Rc<RefCell<>> for the RAF closure.
        let state: Rc<RefCell<(Runtime<A, P>, uzor_window_web::WebWindowProvider)>> =
            Rc::new(RefCell::new((runtime, provider)));

        // The RAF callback is self-referential: it captures an Rc clone of itself.
        // We use a two-step trick: store the closure in an Rc<RefCell<Option<...>>>
        // so the closure can schedule the next frame.
        let raf_handle: Rc<RefCell<Option<Closure<dyn FnMut()>>>> =
            Rc::new(RefCell::new(None));
        let raf_handle_clone = raf_handle.clone();

        let state_clone = state.clone();
        *raf_handle.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            let mut borrow = state_clone.borrow_mut();
            let (ref mut rt, ref mut prov) = *borrow;

            if prov.should_close() {
                rt.shutdown();
                return;
            }

            // One frame tick (no FPS cap guard on web — RAF handles vsync).
            rt.tick_web(prov);

            // Schedule the next frame.
            if let Some(win) = web_sys::window() {
                if let Some(ref cb) = *raf_handle_clone.borrow() {
                    let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
                }
            }
        }) as Box<dyn FnMut()>));

        // Kick off the first frame.
        if let Some(win) = web_sys::window() {
            if let Some(ref cb) = *raf_handle.borrow() {
                let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
            }
        }

        // Leak the RAF closure so it stays alive for the lifetime of the page.
        // This is intentional: the loop runs until the page is closed.
        std::mem::forget(raf_handle);

        Ok(())
    }
}

// ── Convenience: run_closure (desktop only) ───────────────────────────────────

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
#[cfg(not(target_arch = "wasm32"))]
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
