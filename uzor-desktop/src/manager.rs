//! `Manager` — L4's window-and-event watchdog (formerly `WindowManager`).
//!
//! Owns shared `App` state plus a `HashMap<winit::WindowId, PerWindow>` of
//! per-window state (window/provider/render_state/layout/input).
//! Implements winit's `ApplicationHandler` itself, so [`crate::AppRun::run`]
//! constructs a `Manager` and hands it straight to the event loop.

#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::{ActiveEventLoop, ControlFlow};

use std::collections::HashMap;

use uzor::core::types::Rect;
use uzor::docking::panels::DockPanel;
use uzor::layout::{LayoutManager, WindowHost};
use uzor::framework::multi_window::WindowSpec;
use uzor_render_hub::{
    RenderBackend, RenderHub, RenderSurfaceFactory, SurfaceSize, WindowRenderState,
    submit_frame, SubmitParams,
};
use uzor::layout::window::{WindowDecorations, WindowProvider};

#[cfg(not(target_arch = "wasm32"))]
use uzor_window_desktop::WinitWindowProvider;
#[cfg(not(target_arch = "wasm32"))]
use winit::event::WindowEvent;
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

use uzor::framework::app::{App, AppConfig};
use uzor::framework::builder::{AnyFactory, BuildError, BuiltApp, TraySpec};
use uzor::framework::multi_window::{WindowCtx, WindowKey};
use uzor::framework::render_control::RenderControl;

// ── HubControl ────────────────────────────────────────────────────────────────

/// Thin adapter that implements `RenderControl` by delegating to `&mut RenderHub`.
struct HubControl<'a> {
    hub: &'a mut RenderHub,
    fps_ema:           f32,
    last_frame_time_ms: f32,
    frame_count:       u64,
}

impl<'a> RenderControl for HubControl<'a> {
    fn active_backend(&self) -> uzor::platform::types::RenderBackend {
        self.hub.active()
    }
    fn available_backends(&self) -> Vec<uzor::platform::types::RenderBackend> {
        self.hub.available_backends()
    }
    fn set_backend(&mut self, b: uzor::platform::types::RenderBackend) {
        let _ = self.hub.set_active(b); // silent no-op if not in pool
    }
    fn fps_limit(&self) -> u32 {
        self.hub.settings().fps_limit
    }
    fn set_fps_limit(&mut self, fps: u32) {
        self.hub.set_fps_limit(fps);
    }
    fn msaa_samples(&self) -> u8 {
        self.hub.settings().msaa_samples
    }
    fn set_msaa_samples(&mut self, n: u8) {
        self.hub.set_msaa(n);
    }
    fn vsync(&self) -> bool {
        self.hub.settings().vsync
    }
    fn set_vsync(&mut self, on: bool) {
        self.hub.set_vsync(on);
    }
    fn measured_fps(&self) -> f32       { self.fps_ema }
    fn last_frame_time_ms(&self) -> f32 { self.last_frame_time_ms }
    fn frame_count(&self) -> u64        { self.frame_count }
}

// ── ManagerError ──────────────────────────────────────────────────────────────

/// Errors emitted by [`Manager`].
#[derive(Debug)]
pub enum ManagerError {
    Build(BuildError),
    Window(String),
    Backend(String),
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManagerError::Build(e)   => write!(f, "build error: {e}"),
            ManagerError::Window(s)  => write!(f, "window provider error: {s}"),
            ManagerError::Backend(s) => write!(f, "backend error: {s}"),
        }
    }
}

impl std::error::Error for ManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ManagerError::Build(e) => Some(e),
            _ => None,
        }
    }
}

// ── PerWindow ─────────────────────────────────────────────────────────────────

/// Per-window state owned by the manager.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct PerWindow<P: DockPanel> {
    pub key:             WindowKey,
    pub spec:            WindowSpec,
    pub window:          std::sync::Arc<Window>,
    pub provider:        WinitWindowProvider,
    pub render_state:    WindowRenderState,
    pub layout:          LayoutManager<P>,
    /// Last known cursor position in logical pixels.
    pub last_mouse_pos:  (f64, f64),
    pub last_frame:      std::time::Instant,
    pub initialised:     bool,
    pub close_requested: bool,

    /// Per-region paint schedule state keyed by `RenderRegion::id`.
    /// Populated each frame from `App::regions()`. Empty when the app
    /// uses the legacy single-region (event-driven) path.
    pub region_states: std::collections::HashMap<&'static str, uzor::render::RegionScheduleState>,

    /// Per-region cached `vello::Scene`s. On a region's "due" frame we
    /// clear and rebuild the scene; on a non-due frame we reuse the
    /// cached geometry. Composite step appends all of them into the
    /// main `render_state.scene` before GPU submit so a single draw
    /// call covers the full window.
    pub region_scenes: std::collections::HashMap<&'static str, vello::Scene>,

    /// Active dock-separator drag, if any. Set when the user clicks on a
    /// `dock-sep-N` hit-zone, cleared on mouse-up. Stores the cursor
    /// origin and the separator index so per-frame mouse-moves can call
    /// `panels_mut().drag_separator(idx, delta, w, h)`.
    pub dock_separator_drag: Option<DockSeparatorDrag>,
}

/// In-flight dock-separator drag state owned by the manager.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy)]
pub(crate) struct DockSeparatorDrag {
    pub sep_idx: usize,
    pub last_x:  f64,
    pub last_y:  f64,
}

// ── PerWindowHost ─────────────────────────────────────────────────────────────

/// Transient `WindowHost` adapter for one `PerWindow`.
///
/// Wraps the winit `Arc<Window>` and the `close_requested` flag so that
/// `LayoutManager::handle_chrome_press` can call host operations without
/// touching winit directly.
#[cfg(not(target_arch = "wasm32"))]
struct PerWindowHost<'a> {
    window:          &'a std::sync::Arc<Window>,
    close_requested: &'a mut bool,
    pending_spawns:  &'a mut Vec<WindowSpec>,
    /// Signals that close_app was requested — caller closes all windows.
    close_app: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> WindowHost for PerWindowHost<'a> {
    fn drag_window(&mut self) {
        let _ = self.window.drag_window();
    }

    fn drag_resize_window(&mut self, dir: uzor::platform::types::ResizeDirection) {
        use uzor::platform::types::ResizeDirection as D;
        use winit::window::ResizeDirection as W;
        let wd = match dir {
            D::North     => W::North,
            D::South     => W::South,
            D::East      => W::East,
            D::West      => W::West,
            D::NorthEast => W::NorthEast,
            D::NorthWest => W::NorthWest,
            D::SouthEast => W::SouthEast,
            D::SouthWest => W::SouthWest,
        };
        let _ = self.window.drag_resize_window(wd);
    }

    fn set_minimized(&mut self, on: bool) {
        self.window.set_minimized(on);
    }

    fn set_maximized(&mut self, on: bool) {
        self.window.set_maximized(on);
    }

    fn is_maximized(&self) -> bool {
        self.window.is_maximized()
    }

    fn close_window(&mut self) {
        *self.close_requested = true;
    }

    fn close_app(&mut self) {
        self.close_app = true;
    }

    fn request_spawn_window(&mut self, spec: WindowSpec) {
        self.pending_spawns.push(spec);
    }

    fn request_redraw(&mut self) {
        self.window.request_redraw();
    }
}

// ── Manager ───────────────────────────────────────────────────────────────────

/// L4 window manager — owns the app, drives the event loop, manages every
/// open window.
pub struct Manager<A: App<P>, P: DockPanel> {
    pub(crate) app:     A,
    pub(crate) config:  AppConfig,
    pub(crate) backend: RenderBackend,
    pub(crate) hub:     Option<RenderHub>,
    pub(crate) factory: Option<Box<dyn RenderSurfaceFactory>>,
    pub(crate) start:   std::time::Instant,
    /// Per-window state, keyed by `winit::WindowId` for fast event routing.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) windows: HashMap<winit::window::WindowId, PerWindow<P>>,
    /// Window specs queued by the builder + by `App::take_pending_spawn`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) pending_spawns: Vec<WindowSpec>,
    /// Window keys queued for destruction.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    pub(crate) pending_closes: Vec<WindowKey>,
    /// Optional tray spec — applied once when the first window is created.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) tray_spec: Option<TraySpec>,
    /// Live tray handle kept alive for the manager's lifetime.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) tray: Option<crate::tray::TrayHandle>,
    // ── Frame metrics (EMA, mlc pattern) ──
    pub(crate) fps_ema:           f32,
    pub(crate) last_frame_time_ms: f32,
    pub(crate) frame_count:       u64,
    pub(crate) last_frame_instant: std::time::Instant,
    _phantom: std::marker::PhantomData<P>,
}

impl<A: App<P>, P: DockPanel + Default + 'static> Manager<A, P> {
    pub fn new(
        app: A,
        config: AppConfig,
        backend: RenderBackend,
        hub: Option<RenderHub>,
    ) -> Self {
        Self {
            app,
            config,
            backend,
            hub,
            factory: None,
            start: std::time::Instant::now(),
            #[cfg(not(target_arch = "wasm32"))]
            windows: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_spawns: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            pending_closes: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            tray_spec: None,
            #[cfg(not(target_arch = "wasm32"))]
            tray: None,
            fps_ema: 60.0,
            last_frame_time_ms: 16.0,
            frame_count: 0,
            last_frame_instant: std::time::Instant::now(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Construct a `Manager` from a pre-built [`BuiltApp`].
    ///
    /// This is the primary entry point used by [`crate::AppRun::run`].
    ///
    /// **Autodetect** is the default:
    /// - Neither backend nor factory set → `RenderHub::autodetect()` probes the
    ///   GPU and selects the best available backend + factory.
    /// - Backend set, factory not set → `RenderHub::fixed(backend)` + factory
    ///   from the hub's pool.
    /// - Both set → use the caller-supplied backend + factory; wrap backend in
    ///   `RenderHub::fixed` for metrics bookkeeping.
    pub fn from_built(built: BuiltApp<A, P>) -> Self {
        // ── Phase 1: resolve hub, backend, factory ────────────────────────────
        let hub = match (built.backend, built.factory.is_some()) {
            (None, false) => RenderHub::autodetect(),
            (Some(b), _) => RenderHub::fixed(b),
            (None, true) => {
                eprintln!(
                    "[uzor-desktop] from_built: factory supplied without backend — \
                     running autodetect; factory may mismatch active backend"
                );
                RenderHub::autodetect()
            }
        };

        let active_backend = hub.active();

        // ── Phase 2: resolve factory ──────────────────────────────────────────
        let factory: Option<Box<dyn RenderSurfaceFactory>> = if let Some(any_factory) = built.factory {
            // Caller explicitly supplied a factory — downcast it.
            downcast_any_factory(any_factory)
        } else {
            // Let the hub produce a fresh factory for the active backend.
            hub.factory_for(active_backend)
        };

        // ── Phase 3: build manager ────────────────────────────────────────────
        let mut mgr = Self::new(built.app, built.config, active_backend, Some(hub));

        if let Some(f) = factory {
            mgr.factory = Some(f);
        }

        if let Some(tray) = built.tray {
            #[cfg(not(target_arch = "wasm32"))]
            mgr.set_tray_spec(tray);
        }
        #[cfg(not(target_arch = "wasm32"))]
        for spec in built.windows {
            mgr.queue_window_spec(spec);
        }
        mgr
    }

    pub fn set_surface_factory(&mut self, factory: Box<dyn RenderSurfaceFactory>) {
        self.factory = Some(factory);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn set_tray_spec(&mut self, spec: TraySpec) {
        self.tray_spec = Some(spec);
    }

    pub fn backend(&self) -> RenderBackend { self.backend }
    pub fn app_config(&self) -> &AppConfig { &self.config }
    pub fn hub(&self) -> Option<&RenderHub> { self.hub.as_ref() }
    pub fn hub_mut(&mut self) -> Option<&mut RenderHub> { self.hub.as_mut() }

    fn fps_limit(&self) -> u32 {
        self.hub.as_ref()
            .map(|h| h.settings().fps_limit)
            .unwrap_or(self.config.fps_limit)
    }

    fn msaa_samples(&self) -> u8 {
        self.hub.as_ref()
            .map(|h| h.settings().msaa_samples)
            .unwrap_or(self.config.msaa_samples)
    }

    /// Queue an initial window spec (called by the builder before `run`).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn queue_window_spec(&mut self, spec: WindowSpec) {
        self.pending_spawns.push(spec);
    }

    /// Run the manager — takes ownership and drives the winit event loop
    /// until all windows close.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn run(mut self) -> Result<(), ManagerError> {
        use winit::event_loop::EventLoop;
        // Drain any spawn requests the App pushed in its constructor before
        // the event loop runs (rare, but cheap to do).
        while let Some(s) = self.app.take_pending_spawn() {
            self.pending_spawns.push(s);
        }

        let event_loop = EventLoop::new()
            .map_err(|e| ManagerError::Window(e.to_string()))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        event_loop.run_app(&mut self)
            .map_err(|e| ManagerError::Window(e.to_string()))?;

        Ok(())
    }

    // ── Window lifecycle ──────────────────────────────────────────────────────

    #[cfg(not(target_arch = "wasm32"))]
    fn create_window_from_spec(
        &mut self,
        event_loop: &ActiveEventLoop,
        spec: WindowSpec,
    ) -> Result<(), ManagerError> {
        let mut attrs = Window::default_attributes()
            .with_title(&spec.title)
            .with_inner_size(winit::dpi::LogicalSize::new(spec.size.0, spec.size.1))
            .with_decorations(spec.decorations)
            .with_visible(false);

        if let Some(ref rgba) = spec.icon.clone().or_else(|| self.config.icon.clone()) {
            if let Ok(ic) = winit::window::Icon::from_rgba(
                rgba.pixels.clone(), rgba.width, rgba.height,
            ) {
                attrs = attrs.with_window_icon(Some(ic));
            }
        }
        if let Some((mw, mh)) = spec.min_size {
            attrs = attrs.with_min_inner_size(winit::dpi::LogicalSize::new(mw, mh));
        }

        let window = std::sync::Arc::new(
            event_loop.create_window(attrs)
                .map_err(|e| ManagerError::Window(e.to_string()))?
        );

        let mut provider = WinitWindowProvider::new(std::sync::Arc::clone(&window));
        let raw_handle = provider.raw_window_handle()
            .ok_or_else(|| ManagerError::Window("no raw handle available".into()))?;
        let rect = provider.window_rect();
        let dpr  = provider.scale_factor();
        let size = SurfaceSize {
            width:  (rect.width  * dpr).max(1.0) as u32,
            height: (rect.height * dpr).max(1.0) as u32,
        };
        // Each window gets a fresh factory for the hub's CURRENT active
        // backend. Reusing self.factory ties every spawned window to the
        // backend that was active when from_built ran, which breaks
        // backends switched at runtime via render_control.set_backend.
        let active = self.hub.as_ref().map(|h| h.active()).unwrap_or(self.backend);
        let mut factory: Box<dyn RenderSurfaceFactory> = if let Some(hub) = self.hub.as_ref() {
            hub.factory_for(active)
                .ok_or_else(|| ManagerError::Backend(
                    format!("hub has no factory for backend {:?}", active)
                ))?
        } else {
            return Err(ManagerError::Backend("no hub initialised".into()));
        };

        // CPU backends (TinySkia, VelloCpu) need a per-window
        // SoftwarePresenter — the hub's factory comes empty and accepts
        // a fresh presenter via its .with_presenter() ctor. Build a new
        // factory per window using the provider's softbuffer impl.
        if matches!(active, RenderBackend::TinySkia | RenderBackend::VelloCpu) {
            if let Some(presenter) = provider.create_software_presenter() {
                factory = match active {
                    RenderBackend::TinySkia => {
                        Box::new(uzor_render_hub::TinySkiaSurfaceFactory::with_presenter(presenter))
                    }
                    RenderBackend::VelloCpu => {
                        Box::new(uzor_render_hub::VelloCpuSurfaceFactory::with_presenter(1.0, presenter))
                    }
                    _ => unreachable!(),
                };
            }
        }

        let mut render_state = factory
            .create_render_state(&raw_handle, active, size)
            .map_err(|e| ManagerError::Backend(format!("create_render_state({:?}): {}", active, e)))?;

        window.set_visible(true);

        // Push the initial size into the render state so software presenters
        // and CPU pixmaps are sized for the very first frame — winit only
        // sends a `Resized` event later, after the first paint.
        render_state.resize_surface(size.width, size.height);

        // Apply OS window decorations (corner rounding, border colour, shadow).
        // Spec values win over AppConfig defaults. Non-Windows targets no-op silently.
        {
            let style = if spec.corner_style != uzor::platform::types::CornerStyle::Default {
                spec.corner_style
            } else {
                self.config.corner_style
            };
            if style != uzor::platform::types::CornerStyle::Default {
                provider.set_corner_style(style);
            }

            let color = spec.border_color.or(self.config.border_color);
            if color.is_some() {
                provider.set_border_color(color);
            }

            let shadow = spec.shadow.or(self.config.shadow);
            if let Some(s) = shadow {
                provider.set_shadow(s);
            }
        }

        let id = window.id();
        let pw = PerWindow::<P> {
            key:             spec.key.clone(),
            spec:            spec.clone(),
            window,
            provider,
            render_state,
            layout:          LayoutManager::new(),
            last_mouse_pos:  (0.0, 0.0),
            last_frame:      std::time::Instant::now(),
            initialised:     false,
            close_requested: false,
            region_states:   std::collections::HashMap::new(),
            region_scenes:   std::collections::HashMap::new(),
            dock_separator_drag: None,
        };
        self.windows.insert(id, pw);

        // Kick off the first paint: winit only sends Resized/RedrawRequested
        // events to *future* state changes; without an explicit request the
        // freshly-spawned second window never ticks until the user moves the
        // mouse over it.
        if let Some(pw) = self.windows.get(&id) {
            pw.window.request_redraw();
        }

        // Apply tray spec on first window creation only.
        if self.tray.is_none() {
            if let Some(spec) = self.tray_spec.take() {
                let mut tb = crate::tray::TrayBuilder::new();
                if let Some(ref icon) = self.config.icon {
                    tb = tb.icon(icon.clone());
                }
                if let Some(t) = spec.tooltip { tb = tb.tooltip(t); }
                for (id, label, enabled) in spec.items {
                    tb = if enabled { tb.menu_item(id, label) }
                         else        { tb.menu_item_disabled(id, label) };
                }
                match tb.build() {
                    Ok(handle) => self.tray = Some(handle),
                    Err(e) => eprintln!("[uzor-desktop] tray init failed: {e}"),
                }
            }
        }

        Ok(())
    }

    /// Look up a window's `winit::WindowId` by app-supplied `WindowKey`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn window_id_for(&self, key: &WindowKey) -> Option<winit::window::WindowId> {
        self.windows.iter().find_map(|(id, pw)| (pw.key == *key).then_some(*id))
    }

    // ── Per-window event handling ─────────────────────────────────────────────

    /// Process one raw winit event for the given window.
    /// Only talks to `LayoutManager` via its L3 surface — no direct coord/bridge access.
    #[cfg(not(target_arch = "wasm32"))]
    fn handle_window_winit_event(&mut self, id: winit::window::WindowId, event: &WindowEvent) {
        let now_ms = self.start.elapsed().as_secs_f64() * 1000.0;

        use winit::event::{ElementState, MouseButton as WMouseButton};

        match event {
            // ── Cursor moved ─────────────────────────────────────────────────
            WindowEvent::CursorMoved { position, .. } => {
                let Some(pw) = self.windows.get_mut(&id) else { return };
                let dpr = pw.provider.scale_factor();
                let lx = position.x / dpr;
                let ly = position.y / dpr;
                pw.last_mouse_pos = (lx, ly);
                pw.layout.on_pointer_move(lx, ly);

                // Drive an in-flight dock-separator drag.  panels_mut()
                // applies the per-pixel delta to the underlying split
                // ratio — the next frame's solve picks up the new sizes.
                if let Some(drag) = pw.dock_separator_drag.as_mut() {
                    use uzor::docking::panels::SeparatorOrientation as SO;
                    let orient = pw.layout.panels()
                        .separators()
                        .get(drag.sep_idx)
                        .map(|s| s.orientation);
                    if let Some(orient) = orient {
                        let win = pw.layout.last_window().unwrap_or(uzor::types::Rect::new(0.0, 0.0, 1.0, 1.0));
                        let delta = match orient {
                            SO::Vertical   => (lx - drag.last_x) as f32,
                            SO::Horizontal => (ly - drag.last_y) as f32,
                        };
                        pw.layout.panels_mut().drag_separator(
                            drag.sep_idx,
                            delta,
                            win.width  as f32,
                            win.height as f32,
                        );
                    }
                    drag.last_x = lx;
                    drag.last_y = ly;
                }

                // Force a redraw so the next frame paints the hover state.
                pw.window.request_redraw();
            }

            // ── Mouse button pressed ─────────────────────────────────────────
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: WMouseButton::Left,
                ..
            } => {
                let Some(pw) = self.windows.get_mut(&id) else { return };
                let (mx, my) = pw.last_mouse_pos;
                pw.layout.on_pointer_down(mx, my);

                // Dock-separator drag start.  on_pointer_down already
                // wrote `last_pressed` via process_drag_press; check it
                // for the `dock-sep-N` id pattern.
                if let Some(pressed) = pw.layout.last_pressed_widget() {
                    if let Some(suffix) = pressed.as_str().strip_prefix("dock-sep-") {
                        if let Ok(idx) = suffix.parse::<usize>() {
                            pw.dock_separator_drag = Some(DockSeparatorDrag {
                                sep_idx: idx,
                                last_x:  mx,
                                last_y:  my,
                            });
                            return;
                        }
                    }
                }

                // Try chrome / bezel resize via L3. If consumed — done.
                // For NewWindow we need App, so we handle that case here.
                // Build a transient host that delegates to winit.
                let mut host = PerWindowHost {
                    window:          &pw.window,
                    close_requested: &mut pw.close_requested,
                    pending_spawns:  &mut self.pending_spawns,
                    close_app:       false,
                };
                let consumed = pw.layout.handle_chrome_press(mx, my, &mut host, now_ms);

                // If NewWindow was signalled (handle_chrome_press returns false for it),
                // let the App resolve it.
                // Note: handle_chrome_press returns false for NewWindow so we check
                // whether the chrome hit was NewWindow by re-testing here.
                // Simpler: unconditionally ask the App if nothing else consumed.
                if host.close_app {
                    for p in self.windows.values_mut() {
                        p.close_requested = true;
                    }
                    return;
                }
                if consumed {
                    return;
                }

                // Not consumed by chrome — check if it's a NewWindow hit.
                // We do this by calling chrome_hit_test again only if chrome is present.
                let pw2 = match self.windows.get_mut(&id) { Some(p) => p, None => return };
                if let Some(chrome_rect) = pw2.layout.rect_for_chrome() {
                    use uzor::ui::widgets::composite::chrome::{
                        chrome_hit_test, handle_chrome_action, ChromeAction,
                        ChromeRenderKind, ChromeSettings, ChromeView,
                    };
                    let view = ChromeView {
                        tabs: &[],
                        active_tab_id: None,
                        show_new_tab_btn: false,
                        show_menu_btn: false,
                        show_new_window_btn: true,
                        show_close_window_btn: true,
                        is_maximized: pw2.window.is_maximized(),
                        cursor_x: mx,
                        cursor_y: my,
                        time_ms: now_ms,
                    };
                    let settings = ChromeSettings::default();
                    let kind = ChromeRenderKind::Default;
                    let hit = chrome_hit_test(
                        pw2.layout.chrome_state(), &view, &settings, &kind,
                        chrome_rect, (mx, my),
                    );
                    let action = handle_chrome_action(hit);
                    if matches!(action, ChromeAction::NewWindow) {
                        let src = pw2.key.clone();
                        if let Some(spec) = self.app.on_chrome_new_window(&src) {
                            self.pending_spawns.push(spec);
                        }
                        return;
                    }
                }

                // Not a chrome press — forward as a regular click outcome to L3.
                // (Actual click resolution happens on pointer-up.)
            }

            // ── Mouse button released ────────────────────────────────────────
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: WMouseButton::Left,
                ..
            } => {
                let Some(pw) = self.windows.get_mut(&id) else { return };
                let (mx, my) = pw.last_mouse_pos;
                pw.dock_separator_drag = None;
                // L3 records the click in last_click; no pw.input write needed.
                let _outcome = pw.layout.on_pointer_up(mx, my);
                pw.window.request_redraw();
                // App hooks on DispatchEvent / DismissedOverlay are called by
                // App::ui each frame via consume_event — no immediate callback here.
            }

            // ── All other events — no direct coord / bridge access ───────────
            _ => {}
        }
    }

    /// Tick one window — FPS-cap-guarded paint pass.
    #[cfg(not(target_arch = "wasm32"))]
    fn tick_window(
        &mut self,
        id: winit::window::WindowId,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), ManagerError> {
        let fps_limit = self.fps_limit();
        if fps_limit > 0 {
            if let Some(pw) = self.windows.get(&id) {
                let target_dt = std::time::Duration::from_secs_f64(1.0 / fps_limit as f64);
                if pw.last_frame.elapsed() < target_dt {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(
                        pw.last_frame + target_dt,
                    ));
                    return Ok(());
                }
            }
        }
        self.tick_window_inner(id)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn tick_window_inner(&mut self, id: winit::window::WindowId) -> Result<(), ManagerError> {
        let now_secs = self.start.elapsed().as_secs_f64();
        let now_ms   = now_secs * 1000.0;
        let msaa     = self.msaa_samples();

        // ── Frame metrics (EMA, mlc pattern: α = 0.1) ──
        let now_inst = std::time::Instant::now();
        let dt = now_inst.duration_since(self.last_frame_instant);
        self.last_frame_instant = now_inst;
        let dt_ms = dt.as_secs_f64() * 1000.0;
        if dt_ms > 0.1 && dt_ms < 1000.0 {
            let instant_fps = 1000.0 / dt_ms;
            self.fps_ema = (self.fps_ema as f64 * 0.9 + instant_fps * 0.1) as f32;
            self.last_frame_time_ms = dt_ms as f32;
        }
        self.frame_count = self.frame_count.wrapping_add(1);

        // Init hook — runs once per window before its first frame.
        if let Some(pw) = self.windows.get_mut(&id) {
            if !pw.initialised {
                let key = pw.key.clone();
                self.app.init(&key, &mut pw.layout);
                pw.initialised = true;
            }
        }

        let outcome = {
            let pw = match self.windows.get_mut(&id) {
                Some(p) => p,
                None    => return Ok(()),
            };

            for ev in pw.provider.poll_events() {
                let _ = self.app.on_event(&ev);
            }

            let rect = pw.provider.window_rect();
            pw.layout.solve(rect);
            let viewport = pw.layout.rect_for_dock_area().unwrap_or(rect);
            // begin_frame clears one-shot input flags and refreshes widget registrations
            // WITHOUT overwriting the pointer state that on_pointer_* already set.
            pw.layout.begin_frame(now_ms, viewport);
            pw.layout.set_frame_time_ms(now_ms);

            let bg_color = argb_to_alpha_color(pw.spec.background);
            pw.render_state.begin_frame();

            let regions = self.app.regions();
            let now_inst = std::time::Instant::now();

            // Pick path: per-region rebuild + composite (VelloGpu only for now)
            // OR legacy whole-window single-pass.
            let backend_supports_regions =
                matches!(pw.render_state.backend(), uzor::platform::types::RenderBackend::VelloGpu);
            let use_regions = !regions.is_empty() && backend_supports_regions;

            {
                let key = &pw.key;
                let layout = &mut pw.layout;
                let render_state = &mut pw.render_state;
                let region_states = &mut pw.region_states;
                let region_scenes = &mut pw.region_scenes;
                let app = &mut self.app;
                let fps_ema = self.fps_ema;
                let last_frame_time_ms = self.last_frame_time_ms;
                let frame_count = self.frame_count;
                let hub = self.hub.as_mut().expect("hub initialised");
                let mut hub_ctrl = HubControl { hub, fps_ema, last_frame_time_ms, frame_count };

                if use_regions {
                    // 1. Rebuild only the regions whose schedule is due.
                    for region in &regions {
                        let state = region_states
                            .entry(region.id)
                            .or_insert_with(uzor::render::RegionScheduleState::default);
                        if !state.due(region, now_inst) { continue; }
                        let scene = region_scenes
                            .entry(region.id)
                            .or_insert_with(vello::Scene::new);
                        scene.reset();
                        render_state.with_scene_render_context(scene, |render_ctx| {
                            let mut ctx = WindowCtx::<P> {
                                key,
                                layout,
                                render: render_ctx,
                                rect,
                                render_control: &mut hub_ctrl,
                            };
                            app.draw_region(region.id, &mut ctx);
                        });
                        state.last_painted = Some(now_inst);
                    }
                    // 2. Composite all cached region scenes into the main scene
                    //    in declaration order — first-declared paints first
                    //    (under), last-declared on top.
                    for region in &regions {
                        if let Some(rs) = region_scenes.get(region.id) {
                            render_state.append_region_scene(rs);
                        }
                    }
                } else {
                    render_state.with_render_context(|render_ctx| {
                        let mut ctx = WindowCtx::<P> {
                            key,
                            layout,
                            render: render_ctx,
                            rect,
                            render_control: &mut hub_ctrl,
                        };
                        app.ui(&mut ctx);
                    });
                }
            }
            let _responses = pw.layout.ctx_mut().end_frame();
            // Clear one-shot input flags AFTER app.ui consumed them.
            pw.layout.end_frame_inputs();

            let outcome = submit_frame(
                &mut pw.render_state,
                SubmitParams { base_color: bg_color, msaa_samples: msaa },
            );

            let now_inst = std::time::Instant::now();
            pw.last_frame = now_inst;

            // Mark all currently-known regions as painted at this instant.
            // Dirty-driven regions are kept dirty=true until cleared by app.
            // FPS-capped regions use last_painted to schedule next wake-up.
            for state in pw.region_states.values_mut() {
                state.last_painted = Some(now_inst);
            }

            // Legacy event-driven path: when the app declares no regions,
            // fall back to the always-redraw loop (mouse/event driven).
            if pw.region_states.is_empty() {
                pw.window.request_redraw();
            }

            outcome
        };

        if outcome.surface_lost {
            return Err(ManagerError::Backend("wgpu surface lost".into()));
        }
        if let Some(ref mut h) = self.hub {
            h.update_metrics(outcome.metrics);
        }
        Ok(())
    }
}

// ── ApplicationHandler ────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
impl<A, P> winit::application::ApplicationHandler for Manager<A, P>
where
    A: App<P>,
    P: DockPanel + Default + 'static,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // First resume: drain whatever the builder queued.
        let queued = std::mem::take(&mut self.pending_spawns);
        for spec in queued {
            if let Err(e) = self.create_window_from_spec(event_loop, spec) {
                eprintln!("[uzor-desktop] window create failed: {e}");
                event_loop.exit();
                return;
            }
        }

        if self.windows.is_empty() {
            eprintln!("[uzor-desktop] no windows queued — exiting");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(pw) = self.windows.get_mut(&id) {
                    pw.close_requested = true;
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    if let Some(pw) = self.windows.get_mut(&id) {
                        pw.render_state.resize_surface(size.width, size.height);
                    }
                }
                if let Some(pw) = self.windows.get(&id) {
                    pw.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.tick_window(id, event_loop) {
                    eprintln!("[uzor-desktop] tick error: {e}");
                    event_loop.exit();
                }
            }
            ref ev => {
                self.handle_window_winit_event(id, ev);
                if let Some(pw) = self.windows.get_mut(&id) {
                    pw.provider.push_winit_event(ev);
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // 1. Collect close requests.
        let mut to_close: Vec<winit::window::WindowId> = self.windows.iter()
            .filter_map(|(id, pw)| pw.close_requested.then_some(*id))
            .collect();
        while let Some(key) = self.app.take_window_to_close() {
            if let Some(id) = self.window_id_for(&key) {
                to_close.push(id);
            }
        }
        for id in to_close {
            if let Some(pw) = self.windows.remove(&id) {
                pw.window.set_visible(false);
            }
        }

        // 2. Exit when no windows left.
        if self.windows.is_empty() {
            self.app.shutdown();
            event_loop.exit();
            return;
        }

        // 3. Drain spawn requests.
        while let Some(s) = self.app.take_pending_spawn() {
            self.pending_spawns.push(s);
        }
        let queued = std::mem::take(&mut self.pending_spawns);
        for spec in queued {
            if let Err(e) = self.create_window_from_spec(event_loop, spec) {
                eprintln!("[uzor-desktop] window spawn failed: {e}");
            }
        }

        // 4. Per-region paint scheduler (mlc-pattern).
        //    Ask the app what regions it wants painted at what cadence.
        //    For each region whose schedule is due now, request a redraw
        //    on its window; otherwise compute the soonest next-due moment
        //    across all windows and ask winit to wake up exactly then.
        //    Apps that don't override `App::regions()` return an empty
        //    vec — control flow stays at `Wait` and the legacy
        //    event-driven (mouse / explicit `request_redraw`) path
        //    continues unchanged.
        let regions = self.app.regions();
        if !regions.is_empty() {
            let now = std::time::Instant::now();
            let mut next_due: Option<std::time::Instant> = None;
            // Build a map id → region for state lookup.
            for pw in self.windows.values_mut() {
                let mut any_due_now = false;
                for region in &regions {
                    let state = pw.region_states
                        .entry(region.id)
                        .or_insert_with(uzor::render::RegionScheduleState::default);
                    if state.due(region, now) {
                        any_due_now = true;
                    } else if let Some(nd) = state.next_due(region, now) {
                        next_due = Some(match next_due {
                            None => nd,
                            Some(cur) => cur.min(nd),
                        });
                    }
                }
                if any_due_now {
                    pw.window.request_redraw();
                }
            }
            match next_due {
                Some(t) if t > now => event_loop.set_control_flow(ControlFlow::WaitUntil(t)),
                _                   => event_loop.set_control_flow(ControlFlow::Poll),
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Downcast an opaque `Box<dyn AnyFactory>` to `Box<dyn RenderSurfaceFactory>`.
///
/// Tries each known concrete factory type in turn.  Returns `None` and prints a
/// warning if the concrete type is unrecognised.
fn downcast_any_factory(any_factory: Box<dyn AnyFactory>) -> Option<Box<dyn RenderSurfaceFactory>> {
    let any_box = any_factory.into_any();
    macro_rules! try_downcast {
        ($box:expr, $($T:ty),+) => {{
            let mut b = $box;
            $(
                b = match b.downcast::<$T>() {
                    Ok(f) => return Some(f as Box<dyn RenderSurfaceFactory>),
                    Err(b) => b,
                };
            )+
            eprintln!(
                "[uzor-desktop] from_built: unknown factory type — \
                 use Manager::set_surface_factory() directly"
            );
            None
        }};
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        try_downcast!(
            any_box,
            uzor_render_hub::VelloGpuSurfaceFactory,
            uzor_render_hub::VelloHybridSurfaceFactory,
            uzor_render_hub::WgpuInstancedSurfaceFactory,
            uzor_render_hub::TinySkiaSurfaceFactory,
            uzor_render_hub::VelloCpuSurfaceFactory,
            uzor_render_hub::Canvas2dSurfaceFactory
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        try_downcast!(any_box, uzor_render_hub::Canvas2dSurfaceFactory)
    }
}

fn argb_to_alpha_color(argb: u32) -> vello::peniko::color::AlphaColor<vello::peniko::color::Srgb> {
    let a = ((argb >> 24) & 0xFF) as f32 / 255.0;
    let r = ((argb >> 16) & 0xFF) as f32 / 255.0;
    let g = ((argb >>  8) & 0xFF) as f32 / 255.0;
    let b = ( argb        & 0xFF) as f32 / 255.0;
    vello::peniko::color::AlphaColor::new([r, g, b, a])
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn _suppress_unused(_: &dyn WindowProvider, _: &Rect) {}
