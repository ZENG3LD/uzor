//! Fluent builder for constructing an uzor app.

use crate::docking::panels::DockPanel;

use super::app::{App, AppConfig, NoPanel};
use super::multi_window::{WindowSpec, WindowKey};

// RgbaIcon, RenderBackend, and CornerStyle canonical definitions live in uzor::platform::types.
// Re-exported here so existing callers of `uzor::framework::builder::{RgbaIcon,RenderBackend}`
// keep working without changes.
pub use crate::platform::types::{CornerStyle, RgbaIcon, RenderBackend};

// ── AnyFactory ───────────────────────────────────────────────────────────────
//
// The factory is stored as `Box<dyn AnyFactory>` in `BuiltApp` so that
// platform crates (e.g. `uzor-desktop`) can downcast it back to the concrete
// type (e.g. `uzor_render_hub::VelloGpuSurfaceFactory`) and call the actual
// surface-creation methods without `uzor` needing to depend on `uzor-render-hub`.

/// Opaque factory wrapper stored in [`BuiltApp`].
///
/// Platform crates downcast this to their concrete factory type via
/// [`AnyFactory::into_any`] followed by `downcast::<ConcreteFactory>()`.
pub trait AnyFactory: Send + Sync + 'static {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send + Sync>;
}

/// Convenience alias used in [`AppBuilder::surface_factory`].
///
/// Any type that is `Send + Sync + 'static` can be wrapped in the builder.
/// Platform crates (e.g. `uzor-desktop`) recover the concrete type via
/// `built.factory.unwrap().into_any().downcast::<T>()`.
pub type RenderSurfaceFactory = dyn AnyFactory;

/// Blanket implementation — every `Send + Sync + 'static` type is an
/// [`AnyFactory`] automatically.
impl<T: Send + Sync + 'static> AnyFactory for T {
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send + Sync> {
        self
    }
}

// ── BuildError ────────────────────────────────────────────────────────────────

/// Errors that can occur when calling [`AppBuilder::build`] or icon helpers.
#[derive(Debug)]
pub enum BuildError {
    /// PNG icon bytes could not be decoded or converted to RGBA8.
    IconDecode(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::IconDecode(msg) => {
                write!(f, "icon PNG decode failed: {msg}")
            }
        }
    }
}

impl std::error::Error for BuildError {}

// ── TraySpec ──────────────────────────────────────────────────────────────────

/// Spec for a system-tray icon spawned automatically by the builder.
pub struct TraySpec {
    pub tooltip: Option<String>,
    pub items:   Vec<(String, String, bool)>, // (id, label, enabled)
}

// ── BuiltApp ──────────────────────────────────────────────────────────────────

/// The result of [`AppBuilder::build`].
///
/// Bundles all data needed to start the runtime.  Consumed by `uzor-desktop`
/// (or another platform crate) to create the event loop, window(s), and GPU
/// pipeline.
///
/// Fields are `pub` so that external platform crates (e.g. `uzor-desktop`)
/// can destructure them without reflection.
pub struct BuiltApp<A: App<P>, P: DockPanel> {
    #[doc(hidden)]
    pub app:     A,
    #[doc(hidden)]
    pub config:  AppConfig,
    /// `None` means "let the platform runtime autodetect".
    #[doc(hidden)]
    pub backend: Option<RenderBackend>,
    #[doc(hidden)]
    pub factory: Option<Box<dyn AnyFactory>>,
    #[doc(hidden)]
    pub tray:    Option<TraySpec>,
    #[doc(hidden)]
    pub windows: Vec<WindowSpec>,
    #[doc(hidden)]
    pub _phantom: std::marker::PhantomData<P>,
}

// ── AppBuilder ────────────────────────────────────────────────────────────────

/// Fluent builder for configuring an uzor app.
///
/// # Generic parameters
///
/// - `A` — the app struct that implements [`App<P>`].
/// - `P` — the dock-panel type.  Defaults to [`NoPanel`].
pub struct AppBuilder<A, P = NoPanel>
where
    A: App<P>,
    P: DockPanel,
{
    app: A,
    config: AppConfig,
    backend: Option<RenderBackend>,
    factory: Option<Box<dyn AnyFactory>>,
    tray: Option<TraySpec>,
    windows: Vec<WindowSpec>,
    _phantom: std::marker::PhantomData<P>,
}

impl<A, P> AppBuilder<A, P>
where
    A: App<P>,
    P: DockPanel + Default + Clone + 'static,
{
    /// Create a builder wrapping `app`.
    pub fn new(app: A) -> Self {
        Self {
            app,
            config: AppConfig::default(),
            backend: None,
            factory: None,
            tray: None,
            windows: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Queue a window for the manager to create at startup.
    pub fn window(mut self, spec: WindowSpec) -> Self {
        self.windows.push(spec);
        self
    }

    /// Spawn a system-tray icon when the runtime starts.
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

    /// Set the minimum logical window size.
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

    /// Set the MSAA sample count.
    pub fn msaa(mut self, samples: u8) -> Self {
        self.config.msaa_samples = samples;
        self
    }

    /// Enable or disable VSync.
    pub fn vsync(mut self, on: bool) -> Self {
        self.config.vsync = on;
        self
    }

    /// Set the clear colour as `0xAARRGGBB`.
    pub fn background(mut self, argb: u32) -> Self {
        self.config.background = argb;
        self
    }

    /// Enforce single-instance via a Win32 named mutex.
    pub fn single_instance(mut self, name: Option<impl Into<String>>) -> Self {
        self.config.single_instance = name.map(Into::into);
        self
    }

    /// Set the app-level border accent colour (`0x00RRGGBB` ARGB). `None` = OS default.
    ///
    /// Per-window [`WindowSpec::border_color`] overrides this value.
    pub fn border_color(mut self, color: Option<u32>) -> Self {
        self.config.border_color = color;
        self
    }

    /// Set the app-level corner-rounding preference.
    ///
    /// Per-window [`WindowSpec::corner_style`] overrides this value.
    pub fn corner_style(mut self, style: CornerStyle) -> Self {
        self.config.corner_style = style;
        self
    }

    /// Set the app-level drop-shadow override. `None` = OS default.
    ///
    /// Per-window [`WindowSpec::shadow`] overrides this value.
    pub fn shadow(mut self, on: bool) -> Self {
        self.config.shadow = Some(on);
        self
    }

    /// Set the window icon from a pre-built [`RgbaIcon`].
    pub fn icon(mut self, icon: RgbaIcon) -> Self {
        self.config.icon = Some(icon);
        self
    }

    /// Set the window icon by decoding a PNG byte slice.
    ///
    /// # Errors
    ///
    /// Returns `Err(BuildError::IconDecode)` if the bytes are not valid PNG.
    pub fn icon_from_png(mut self, png_bytes: &[u8]) -> Result<Self, BuildError> {
        let icon = decode_png_to_rgba(png_bytes)
            .map_err(|e| BuildError::IconDecode(e))?;
        self.config.icon = Some(icon);
        Ok(self)
    }

    // ── Infrastructure setters ────────────────────────────────────────────────

    /// Select the rendering backend (override — skips autodetect).
    ///
    /// When omitted, `uzor-desktop` will call `RenderHub::autodetect()` at
    /// startup and pick the best available backend automatically.
    pub fn backend(mut self, backend: RenderBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Supply a surface factory (override — skips the hub's built-in factory).
    ///
    /// Pass any concrete factory (e.g. `VelloGpuSurfaceFactory`) boxed as
    /// `Box<T>` where `T: Send + Sync + 'static`.  Platform crates (e.g.
    /// `uzor-desktop`) recover the concrete type via
    /// `built.factory.unwrap().into_any().downcast::<T>()`.
    pub fn surface_factory<T: Send + Sync + 'static>(mut self, factory: Box<T>) -> Self {
        self.factory = Some(factory as Box<dyn AnyFactory>);
        self
    }

    // ── Terminal method ───────────────────────────────────────────────────────

    /// Consume the builder and produce a [`BuiltApp`] ready for a platform
    /// runtime (e.g. `uzor-desktop`) to consume.
    ///
    /// When neither `.backend(...)` nor `.surface_factory(...)` was called, the
    /// platform runtime (e.g. `uzor-desktop`) will call
    /// `RenderHub::autodetect()` automatically — no explicit backend selection
    /// is required.
    pub fn build(mut self) -> Result<BuiltApp<A, P>, BuildError> {
        // Synthesise a default window spec from AppConfig if the caller
        // didn't queue any explicit windows.
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

        Ok(BuiltApp {
            app:     self.app,
            config:  self.config,
            backend: self.backend,
            factory: self.factory,
            tray:    self.tray,
            windows: self.windows,
            _phantom: std::marker::PhantomData,
        })
    }
}

// ── PNG decode helper (no uzor-render-hub dep) ────────────────────────────────

#[cfg(feature = "framework-png")]
fn decode_png_to_rgba(png_bytes: &[u8]) -> Result<RgbaIcon, String> {
    use image::ImageDecoder;
    use std::io::Cursor;

    let decoder = image::codecs::png::PngDecoder::new(Cursor::new(png_bytes))
        .map_err(|e| e.to_string())?;

    let (width, height) = decoder.dimensions();
    let total_bytes = decoder.total_bytes() as usize;

    let mut raw = vec![0u8; total_bytes];
    decoder
        .read_image(&mut raw)
        .map_err(|e| e.to_string())?;

    let rgba: Vec<u8> = if total_bytes == (width * height * 4) as usize {
        raw
    } else {
        let img = image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        img.into_rgba8().into_raw()
    };

    Ok(RgbaIcon::from_rgba(width, height, rgba))
}

#[cfg(not(feature = "framework-png"))]
fn decode_png_to_rgba(_png_bytes: &[u8]) -> Result<RgbaIcon, String> {
    Err("PNG icon decoding requires the 'framework-png' feature".to_string())
}
