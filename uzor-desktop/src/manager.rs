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
use uzor::input::InputState;
use uzor::layout::{LayoutManager, WindowHost};
use uzor::framework::multi_window::WindowSpec;
use uzor_render_hub::{
    RenderBackend, RenderHub, RenderSurfaceFactory, SurfaceSize, WindowRenderState,
    submit_frame, SubmitParams,
};
use uzor_window_hub::lifecycle::WindowProvider;

#[cfg(not(target_arch = "wasm32"))]
use uzor_window_desktop::WinitWindowProvider;
#[cfg(not(target_arch = "wasm32"))]
use winit::event::WindowEvent;
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

use uzor::framework::app::{App, AppConfig};
use uzor::framework::builder::{AnyFactory, BuildError, BuiltApp, TraySpec};
use uzor::framework::multi_window::{WindowCtx, WindowKey};

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
    pub input:           InputState,
    /// Last known cursor position in logical pixels.
    pub last_mouse_pos:  (f64, f64),
    pub last_frame:      std::time::Instant,
    pub initialised:     bool,
    pub close_requested: bool,
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

        let provider = WinitWindowProvider::new(std::sync::Arc::clone(&window));
        let raw_handle = provider.raw_window_handle()
            .ok_or_else(|| ManagerError::Window("no raw handle available".into()))?;
        let rect = provider.window_rect();
        let dpr  = provider.scale_factor();
        let size = SurfaceSize {
            width:  (rect.width  * dpr).max(1.0) as u32,
            height: (rect.height * dpr).max(1.0) as u32,
        };
        let factory = self.factory.as_ref()
            .ok_or_else(|| ManagerError::Backend("no surface factory supplied".into()))?;
        let render_state = factory
            .create_render_state(&raw_handle, self.backend, size)
            .map_err(|e| ManagerError::Backend(e.to_string()))?;

        window.set_visible(true);
        let id = window.id();
        let pw = PerWindow::<P> {
            key:             spec.key.clone(),
            spec:            spec.clone(),
            window,
            provider,
            render_state,
            layout:          LayoutManager::new(),
            input:           InputState::new(),
            last_mouse_pos:  (0.0, 0.0),
            last_frame:      std::time::Instant::now(),
            initialised:     false,
            close_requested: false,
        };
        self.windows.insert(id, pw);

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
                let _outcome = pw.layout.on_pointer_up(mx, my);
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

            // Populate InputState from the last known cursor position.
            // on_pointer_move already pushed it into L3's coordinator on the
            // last CursorMoved event; here we keep pw.input in sync for
            // begin_frame (which needs it for was_clicked / pointer_pos helpers).
            pw.input.time = now_secs;
            pw.input.pointer.pos = Some(pw.last_mouse_pos);

            for ev in pw.provider.poll_events() {
                let _ = self.app.on_event(&ev);
            }

            let rect = pw.provider.window_rect();
            pw.layout.solve(rect);
            let viewport = pw.layout.rect_for_dock_area().unwrap_or(rect);
            let input_snapshot = pw.input.clone();
            pw.layout.ctx_mut().begin_frame(input_snapshot, viewport);
            pw.layout.set_frame_time_ms(now_ms);

            let bg_color = argb_to_alpha_color(pw.spec.background);
            pw.render_state.begin_frame();
            {
                let key = &pw.key;
                let layout = &mut pw.layout;
                let render_state = &mut pw.render_state;
                let app = &mut self.app;
                render_state.with_render_context(|render_ctx| {
                    let mut ctx = WindowCtx::<P> {
                        key,
                        layout,
                        render: render_ctx,
                        rect,
                    };
                    app.ui(&mut ctx);
                });
            }
            let _responses = pw.layout.ctx_mut().end_frame();

            let outcome = submit_frame(
                &mut pw.render_state,
                SubmitParams { base_color: bg_color, msaa_samples: msaa },
            );

            pw.last_frame = std::time::Instant::now();
            pw.input.pointer.clicked = None;
            pw.input.pointer.double_clicked = None;
            pw.input.scroll_delta = (0.0, 0.0);
            pw.window.request_redraw();

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
