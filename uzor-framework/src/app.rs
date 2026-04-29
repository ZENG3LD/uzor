//! Application lifecycle trait, configuration, and the `NoPanel` default type.

use uzor::docking::panels::DockPanel;
use uzor::layout::LayoutManager;
use uzor::input::core::event_processor::PlatformEvent;

// ── NoPanel ───────────────────────────────────────────────────────────────────

/// Empty dock-panel type for apps that do not use dockable panels.
///
/// This is the default for the `App<P>` generic parameter. Consumer apps that
/// do not need tabbed/split panel docking can use `App` (or `App<NoPanel>`)
/// without specifying a concrete panel type.
#[derive(Clone, Debug, Default)]
pub struct NoPanel;

impl DockPanel for NoPanel {
    fn title(&self) -> &str {
        ""
    }

    fn type_id(&self) -> &'static str {
        "no_panel"
    }

    fn min_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn closable(&self) -> bool {
        false
    }
}

// ── App trait ─────────────────────────────────────────────────────────────────

/// Application lifecycle trait.
///
/// Implementors describe per-frame UI and react to platform events.
/// Most apps only need to override [`App::ui`]; the remaining methods have
/// empty default implementations.
///
/// # Generic parameter
///
/// `P` is the dock-panel type. Use the default `NoPanel` for apps that do not
/// use the docking system:
///
/// ```rust,ignore
/// struct MyApp { /* fields */ }
/// impl App for MyApp {          // P = NoPanel by default
///     fn ui(&mut self, layout: &mut LayoutManager<NoPanel>, render: &mut uzor_render_hub::WindowRenderState) { ... }
/// }
/// ```
///
/// For apps with a docking tree:
///
/// ```rust,ignore
/// impl App<ChartPanel> for ChartApp {
///     fn ui(&mut self, layout: &mut LayoutManager<ChartPanel>, render: &mut WindowRenderState) { ... }
/// }
/// ```
pub trait App<P: DockPanel = NoPanel>: Sized + 'static {
    /// Called once after the runtime initialises, before the first frame.
    ///
    /// Use to configure chrome height, add edge slots, push initial overlays,
    /// or register any one-time layout configuration.
    fn init(&mut self, _layout: &mut LayoutManager<P>) {}

    /// Called every frame after layout solve and before GPU submission.
    ///
    /// Register widgets via `layout.ctx_mut()` helpers. Build the vello scene
    /// or instanced draw calls via `render_state`.
    ///
    /// `render_state` is the [`uzor_render_hub::WindowRenderState`] for the
    /// current window. `begin_frame()` has already been called on it before
    /// this method is invoked; `submit_frame()` is called after it returns.
    fn ui(
        &mut self,
        layout: &mut LayoutManager<P>,
        render_state: &mut uzor_render_hub::WindowRenderState,
    );

    /// Called for each [`PlatformEvent`] before default input processing.
    ///
    /// Return `true` to consume the event (preventing the runtime's default
    /// handling). Return `false` to let the runtime pass it to
    /// [`uzor::input::core::event_processor::EventProcessor`].
    fn on_event(&mut self, _event: &PlatformEvent) -> bool {
        false
    }

    /// Called once when the runtime is about to exit after all windows close.
    ///
    /// Use for cleanup: flush pending I/O, save state, etc.
    fn shutdown(&mut self, _layout: &mut LayoutManager<P>) {}
}

// ── AppConfig ─────────────────────────────────────────────────────────────────

/// Per-app runtime configuration.
///
/// Passed to [`crate::builder::AppBuilder::config`] to override defaults.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Window title bar text.
    pub title: String,

    /// Initial logical window size `(width, height)` in pixels.
    pub initial_size: (u32, u32),

    /// Minimum logical window size. `None` = no minimum enforced.
    pub min_size: Option<(u32, u32)>,

    /// Whether to show OS-native window decorations (title bar, border).
    ///
    /// `false` = chromeless — the app draws its own window chrome.
    pub decorations: bool,

    /// When `true`, request VSync-enabled presentation mode.
    pub vsync: bool,

    /// Clear colour for each frame expressed as `0xAARRGGBB`.
    pub background: u32,

    /// Enable multi-window support. When `false` (default) only a single
    /// primary window is created and managed.
    pub multi_window: bool,

    /// FPS limit. `0` = unlimited (run as fast as possible / `Poll` control
    /// flow). Non-zero values set a `WaitUntil` deadline in `about_to_wait`.
    pub fps_limit: u32,

    /// MSAA sample count (1 = no MSAA, 4 = 4×MSAA).
    pub msaa_samples: u8,

    /// When `Some(name)`, call [`crate::utils::single_instance`] at startup
    /// and hold the guard for the process lifetime. The string is the Win32
    /// named-mutex name passed to `CreateMutexW`.
    pub single_instance: Option<String>,

    /// When `Some("#RRGGBB")`, set the Windows 11 DWM border color via
    /// `DwmSetWindowAttribute(DWMWA_BORDER_COLOR)`. Silently ignored on
    /// platforms / OS versions that do not support this attribute.
    pub dwm_border_color: Option<String>,

    /// Start the window invisible and reveal it only after the first GPU frame
    /// is presented. Eliminates the white-flash on startup.
    pub start_visible: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "uzor app".to_string(),
            initial_size: (1280, 720),
            min_size: Some((400, 300)),
            decorations: false,
            vsync: true,
            background: 0xFF181820,
            multi_window: false,
            fps_limit: 0,
            msaa_samples: 1,
            single_instance: None,
            dwm_border_color: None,
            start_visible: false,
        }
    }
}

// ── ClosureApp ────────────────────────────────────────────────────────────────

/// Adapter that implements [`App`] for a closure.
///
/// Created by [`run_closure`]; provides a quick way to prototype without
/// defining a full `App` implementation.
pub struct ClosureApp<P, F>
where
    P: DockPanel,
    F: FnMut(&mut LayoutManager<P>, &mut uzor_render_hub::WindowRenderState) + 'static,
{
    ui_fn: F,
    _phantom: std::marker::PhantomData<P>,
}

impl<P, F> ClosureApp<P, F>
where
    P: DockPanel + Default,
    F: FnMut(&mut LayoutManager<P>, &mut uzor_render_hub::WindowRenderState) + 'static,
{
    /// Wrap a closure into a `ClosureApp`.
    pub fn new(ui_fn: F) -> Self {
        Self {
            ui_fn,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<P, F> App<P> for ClosureApp<P, F>
where
    P: DockPanel + Default + 'static,
    F: FnMut(&mut LayoutManager<P>, &mut uzor_render_hub::WindowRenderState) + 'static,
{
    fn ui(
        &mut self,
        layout: &mut LayoutManager<P>,
        render_state: &mut uzor_render_hub::WindowRenderState,
    ) {
        (self.ui_fn)(layout, render_state);
    }
}
