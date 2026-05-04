//! `Manager` — L4's window-and-event watchdog (formerly `WindowManager`).
//!
//! Owns shared `App` state plus a `HashMap<winit::WindowId, PerWindow>` of
//! per-window state (window/provider/render_state/layout/bridge/input).
//! Implements winit's `ApplicationHandler` itself, so [`crate::AppRun::run`]
//! constructs a `Manager` and hands it straight to the event loop.

#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::{ActiveEventLoop, ControlFlow};

use std::collections::HashMap;

use uzor::core::types::Rect;
use uzor::docking::panels::DockPanel;
use uzor::input::InputState;
use uzor::layout::LayoutManager;
use uzor_render_hub::{
    RenderBackend, RenderHub, RenderSurfaceFactory, SurfaceSize, WindowRenderState,
    submit_frame, SubmitParams,
};
use uzor_window_hub::lifecycle::WindowProvider;

#[cfg(not(target_arch = "wasm32"))]
use uzor_window_desktop::{WinitInputBridge, WinitWindowProvider};
#[cfg(not(target_arch = "wasm32"))]
use winit::event::WindowEvent;
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

use uzor::framework::app::{App, AppConfig};
use uzor::framework::builder::{AnyFactory, BuildError, BuiltApp, TraySpec};
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};

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
    pub bridge:          WinitInputBridge,
    pub input:           InputState,
    pub last_frame:      std::time::Instant,
    pub initialised:     bool,
    pub close_requested: bool,
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
    pub fn from_built(built: BuiltApp<A, P>) -> Self {
        // Downcast opaque RenderHub to the concrete uzor_render_hub::RenderHub.
        let hub: Option<RenderHub> = built.hub.and_then(|opaque| {
            opaque.into_inner()
                .downcast::<RenderHub>()
                .ok()
                .map(|b| *b)
        });

        let mut mgr = Self::new(built.app, built.config, built.backend, hub);
        if let Some(any_factory) = built.factory {
            let any_box = any_factory.into_any();
            match any_box.downcast::<uzor_render_hub::VelloGpuSurfaceFactory>() {
                Ok(f) => { mgr.factory = Some(f as Box<dyn RenderSurfaceFactory>); }
                Err(any_box) => {
                    match any_box.downcast::<uzor_render_hub::VelloHybridSurfaceFactory>() {
                        Ok(f) => { mgr.factory = Some(f as Box<dyn RenderSurfaceFactory>); }
                        Err(any_box) => {
                            match any_box.downcast::<uzor_render_hub::WgpuInstancedSurfaceFactory>() {
                                Ok(f) => { mgr.factory = Some(f as Box<dyn RenderSurfaceFactory>); }
                                Err(any_box) => {
                                    match any_box.downcast::<uzor_render_hub::TinySkiaSurfaceFactory>() {
                                        Ok(f) => { mgr.factory = Some(f as Box<dyn RenderSurfaceFactory>); }
                                        Err(any_box) => {
                                            match any_box.downcast::<uzor_render_hub::VelloCpuSurfaceFactory>() {
                                                Ok(f) => { mgr.factory = Some(f as Box<dyn RenderSurfaceFactory>); }
                                                Err(_) => {
                                                    eprintln!("[uzor-desktop] from_built: unknown factory type — use Manager::set_surface_factory() directly");
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
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
            bridge:          WinitInputBridge::new(),
            input:           InputState::new(),
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

    /// Process one raw winit event for the given window. Mirrors L3 example.
    #[cfg(not(target_arch = "wasm32"))]
    fn handle_window_winit_event(&mut self, id: winit::window::WindowId, event: &WindowEvent) {
        let now_ms = self.start.elapsed().as_secs_f64() * 1000.0;

        let Some(pw) = self.windows.get_mut(&id) else { return };

        use uzor::ui::widgets::composite::chrome::{
            chrome_hit_test, handle_chrome_action, ChromeAction, ChromeRenderKind,
            ChromeSettings, ChromeView,
        };
        use winit::event::{ElementState, MouseButton as WMouseButton};

        // 1. Chrome / window-resize press handler — synchronous.
        if let WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: WMouseButton::Left,
            ..
        } = event
        {
            let (mx, my) = pw.bridge.last_mouse_pos;

            // Chrome zone (drag, min/max/close, in-strip resize edges).
            if let Some(chrome_rect) = pw.layout.rect_for_chrome() {
                let view = ChromeView {
                    tabs: &[],
                    active_tab_id: None,
                    show_new_tab_btn: false,
                    show_menu_btn: false,
                    show_new_window_btn: true,
                    show_close_window_btn: true,
                    is_maximized: pw.window.is_maximized(),
                    cursor_x: mx,
                    cursor_y: my,
                    time_ms: now_ms,
                };
                let settings = ChromeSettings::default();
                let kind = ChromeRenderKind::Default;
                let hit = chrome_hit_test(
                    pw.layout.chrome_state(), &view, &settings, &kind,
                    chrome_rect, (mx, my),
                );
                match handle_chrome_action(hit) {
                    ChromeAction::WindowDragStart => {
                        let _ = pw.window.drag_window();
                        return;
                    }
                    ChromeAction::Minimize => {
                        pw.window.set_minimized(true);
                        return;
                    }
                    ChromeAction::MaximizeRestore => {
                        pw.window.set_maximized(!pw.window.is_maximized());
                        return;
                    }
                    ChromeAction::CloseWindow => {
                        pw.close_requested = true;
                        return;
                    }
                    ChromeAction::CloseApp => {
                        drop(pw);
                        for p in self.windows.values_mut() {
                            p.close_requested = true;
                        }
                        return;
                    }
                    ChromeAction::NewWindow => {
                        let src = pw.key.clone();
                        if let Some(spec) = self.app.on_chrome_new_window(&src) {
                            self.pending_spawns.push(spec);
                        }
                        return;
                    }
                    ChromeAction::BeginResize(h) => {
                        use uzor::ui::widgets::composite::chrome::types::{ChromeHit, ResizeCorner};
                        use winit::window::ResizeDirection as W;
                        let dir = match h {
                            ChromeHit::ResizeTop      => Some(W::North),
                            ChromeHit::ResizeBottom   => Some(W::South),
                            ChromeHit::ResizeLeft     => Some(W::West),
                            ChromeHit::ResizeRight    => Some(W::East),
                            ChromeHit::ResizeCorner(ResizeCorner::TopLeft)     => Some(W::NorthWest),
                            ChromeHit::ResizeCorner(ResizeCorner::TopRight)    => Some(W::NorthEast),
                            ChromeHit::ResizeCorner(ResizeCorner::BottomLeft)  => Some(W::SouthWest),
                            ChromeHit::ResizeCorner(ResizeCorner::BottomRight) => Some(W::SouthEast),
                            _ => None,
                        };
                        if let Some(d) = dir {
                            let _ = pw.window.drag_resize_window(d);
                            return;
                        }
                    }
                    _ => {}
                }
            }

            // Edge-resize fallback for borderless windows.
            let win = pw.layout.last_window().unwrap_or_default();
            let bezel = 6.0_f64;
            if win.width > 0.0 && win.height > 0.0 {
                let on_left   = mx >= win.x                       && mx < win.x + bezel;
                let on_right  = mx >= win.x + win.width  - bezel  && mx < win.x + win.width;
                let on_top    = my >= win.y                       && my < win.y + bezel;
                let on_bottom = my >= win.y + win.height - bezel  && my < win.y + win.height;
                use winit::window::ResizeDirection as W;
                let dir = match (on_top, on_bottom, on_left, on_right) {
                    (true,  _,    true,  _   ) => Some(W::NorthWest),
                    (true,  _,    _,     true) => Some(W::NorthEast),
                    (_,     true, true,  _   ) => Some(W::SouthWest),
                    (_,     true, _,     true) => Some(W::SouthEast),
                    (true,  _,    _,     _   ) => Some(W::North),
                    (_,     true, _,     _   ) => Some(W::South),
                    (_,     _,    true,  _   ) => Some(W::West),
                    (_,     _,    _,     true) => Some(W::East),
                    _ => None,
                };
                if let Some(d) = dir {
                    let _ = pw.window.drag_resize_window(d);
                    return;
                }
            }
        }

        // 2. Bridge → InputCoordinator (L1).
        let focused = pw.layout.ctx_mut().input.focused_widget().cloned();
        let coord   = &mut pw.layout.ctx_mut().input;
        let out     = pw.bridge.handle_event(coord, focused.as_ref(), event);

        if let Some((x, y)) = out.cursor_moved {
            pw.input.pointer.pos = Some((x, y));
        }

        // 3. On left-up → route click via L3 dispatcher.
        if let Some(((x, y), _clicked_id)) = out.left_up {
            pw.input.pointer.pos = Some((x, y));
            let _ = self.app.route_click(&mut pw.layout, x, y);
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

            pw.input.time = now_secs;
            let (mx, my) = pw.bridge.last_mouse_pos;
            pw.input.pointer.pos = Some((mx, my));

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

