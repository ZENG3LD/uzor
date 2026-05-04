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
//! resumed → create_window → factory::create_render_state (or hub)
//!         → loop {
//!               window_event(RedrawRequested) → Runtime::tick()
//!                   → FPS cap guard (WaitUntil)
//!                   → poll_events → solve layout → begin_frame
//!                   → app.ui       → submit_frame → request_redraw
//!           }
//!         → CloseRequested → exit
//! ```
//!
//! # FPS cap (mlc pattern)
//!
//! When `hub.settings().fps_limit > 0` (or `config.fps_limit > 0` in Mode A),
//! `tick` exits early before any GPU work if the target frame interval has not
//! elapsed and schedules a `WaitUntil` wake-up.  This replicates the pattern
//! from `mylittlechart` `chart-app-vello/src/main.rs` lines 2171–2184:
//!
//! ```text
//! if fps_limit > 0 {
//!     let target_dt = Duration::from_secs_f64(1.0 / fps_limit as f64);
//!     if last_frame.elapsed() < target_dt {
//!         event_loop.set_control_flow(ControlFlow::WaitUntil(last_frame + target_dt));
//!         return;
//!     }
//! }
//! ```

#[cfg(not(target_arch = "wasm32"))]
use winit::event_loop::{ActiveEventLoop, ControlFlow};

use uzor::docking::panels::DockPanel;
use uzor::input::InputState;
use uzor::layout::LayoutManager;
use uzor_render_hub::{
    RenderBackend, RenderHub, RenderSurfaceFactory, SurfaceSize, WindowRenderState,
    submit_frame, SubmitParams,
};
use uzor_window_hub::lifecycle::WindowProvider;

#[cfg(not(target_arch = "wasm32"))]
use uzor_window_desktop::WinitInputBridge;
#[cfg(not(target_arch = "wasm32"))]
use winit::event::WindowEvent;
#[cfg(not(target_arch = "wasm32"))]
use winit::window::Window;

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
    /// Optional render hub (Mode B).  `None` in Mode A (fixed backend).
    pub(crate) hub: Option<RenderHub>,
    /// Surface factory (held until `init_render_state` is called).
    pub(crate) factory: Option<Box<dyn RenderSurfaceFactory>>,
    /// Layout manager.
    pub(crate) layout: LayoutManager<P>,
    /// Desktop input bridge — forwards winit events to `InputCoordinator`.
    /// `None` on wasm where winit is absent.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) bridge: WinitInputBridge,
    /// Accumulated per-frame input snapshot — built from bridge state at
    /// `tick_inner`'s top.
    pub(crate) input: InputState,
    /// Monotonic start instant for `InputState::time` tracking.
    pub(crate) start: std::time::Instant,
    /// Whether `app.init` has been called.
    pub(crate) initialised: bool,
    /// Ready render state (set once the window + surface are available).
    pub(crate) render_state: Option<WindowRenderState>,
    /// Instant of the last completed frame (for FPS cap).
    pub(crate) last_frame: std::time::Instant,
}

impl<A: App<P>, P: DockPanel + Default + 'static> Runtime<A, P> {
    /// Construct the runtime.
    ///
    /// Prefer [`crate::builder::AppBuilder::build`] over calling this directly.
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
            layout: LayoutManager::new(),
            #[cfg(not(target_arch = "wasm32"))]
            bridge: WinitInputBridge::new(),
            input: InputState::new(),
            start: std::time::Instant::now(),
            initialised: false,
            render_state: None,
            last_frame: std::time::Instant::now(),
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

    /// Returns a reference to the [`RenderHub`] if one was supplied (Mode B).
    pub fn hub(&self) -> Option<&RenderHub> {
        self.hub.as_ref()
    }

    /// Returns a mutable reference to the [`RenderHub`] if one was supplied.
    pub fn hub_mut(&mut self) -> Option<&mut RenderHub> {
        self.hub.as_mut()
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

    /// Effective FPS limit: hub's value (Mode B) or config value (Mode A).
    fn fps_limit(&self) -> u32 {
        self.hub
            .as_ref()
            .map(|h| h.settings().fps_limit)
            .unwrap_or(self.config.fps_limit)
    }

    /// Effective MSAA samples: hub's value (Mode B) or config value (Mode A).
    fn msaa_samples(&self) -> u8 {
        self.hub
            .as_ref()
            .map(|h| h.settings().msaa_samples)
            .unwrap_or(self.config.msaa_samples)
    }

    /// Forward a raw winit `WindowEvent` to the input pipeline.
    ///
    /// Called from `UzorHandler::window_event` for every event that isn't
    /// `RedrawRequested`/`CloseRequested`.  The runtime:
    ///
    /// 1. On left-button press, runs the chrome hit-test.  If the press lands
    ///    on a chrome drag/min/max/close zone, calls the matching
    ///    `Window::*` method directly (winit requires this to happen *during*
    ///    the press handler, while the button is still held — so we cannot
    ///    defer it to the next `tick`).
    /// 2. Otherwise delegates to `WinitInputBridge::handle_event`, which
    ///    routes the event into the L1 `InputCoordinator`.  Click / drag /
    ///    text-field state lives in coord state from there on.
    /// 3. Mirrors `clicked`/`button_down` flags into `Runtime::input` so the
    ///    next `tick_inner` can route the click via `App::route_click`.
    ///
    /// Returns `true` when the event was consumed by chrome (caller should
    /// stop further processing for this event).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn handle_winit_event(&mut self, event: &WindowEvent, window: &Window) -> bool {
        use uzor::ui::widgets::composite::chrome::{
            chrome_hit_test, handle_chrome_action, ChromeAction, ChromeRenderKind,
            ChromeSettings, ChromeView,
        };
        use uzor::input::pointer::state::MouseButton;
        use winit::event::{ElementState, MouseButton as WMouseButton};

        // 1. Chrome press handler (must run BEFORE bridge so drag_window()
        //    is called while the button is still pressed).
        if let WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: WMouseButton::Left,
            ..
        } = event
        {
            let (mx, my) = self.bridge.last_mouse_pos;
            // Chrome only does something useful if it has been registered
            // this frame — `rect_for_chrome()` returns None otherwise.
            if let Some(chrome_rect) = self.layout.rect_for_chrome() {
                // We don't have the live ChromeView here (the app rebuilds
                // it inside `App::ui`), but the geometric hit-test only
                // depends on `is_maximized`, drag/edge zones, and tab count.
                // For the chrome zones the empty-tabs view is sufficient
                // because tabs are an *opt-in* concept for the framework's
                // automatic chrome handling — apps that want clickable tabs
                // route them via `App::on_chrome_tab` (TODO).
                let view = ChromeView {
                    tabs: &[],
                    active_tab_id: None,
                    show_new_tab_btn: false,
                    show_menu_btn: false,
                    show_new_window_btn: false,
                    show_close_window_btn: true,
                    is_maximized: window.is_maximized(),
                    cursor_x: mx,
                    cursor_y: my,
                    time_ms: self.start.elapsed().as_secs_f64() * 1000.0,
                };
                let settings = ChromeSettings::default();
                let kind = ChromeRenderKind::Default;
                let hit = chrome_hit_test(
                    &self.layout.chrome_state(),
                    &view,
                    &settings,
                    &kind,
                    chrome_rect,
                    (mx, my),
                );
                match handle_chrome_action(hit) {
                    ChromeAction::WindowDragStart => {
                        let _ = window.drag_window();
                        return true;
                    }
                    ChromeAction::Minimize => {
                        window.set_minimized(true);
                        return true;
                    }
                    ChromeAction::MaximizeRestore => {
                        window.set_maximized(!window.is_maximized());
                        return true;
                    }
                    ChromeAction::CloseApp | ChromeAction::CloseWindow => {
                        // Caller (UzorHandler) will see the bridge's left_up
                        // next; for an immediate close we'd need to thread an
                        // exit signal back up.  Defer to App on_event for now.
                        return false;
                    }
                    ChromeAction::BeginResize(hit) => {
                        use uzor::ui::widgets::composite::chrome::types::{ChromeHit, ResizeCorner};
                        use winit::window::ResizeDirection as W;
                        let dir = match hit {
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
                            let _ = window.drag_resize_window(d);
                            return true;
                        }
                        return false;
                    }
                    _ => {}
                }
            }

            // Edge-resize fallback for borderless windows: chrome_hit_test
            // only covers the chrome strip, but the user can grab any of the
            // window's four edges.  Test against the full window rect using
            // a small bezel.
            let win = self.layout.last_window().unwrap_or_default();
            let bezel = 6.0_f64;
            if win.width > 0.0 && win.height > 0.0 {
                let on_left   = mx >= win.x          && mx < win.x + bezel;
                let on_right  = mx >= win.x + win.width - bezel && mx < win.x + win.width;
                let on_top    = my >= win.y          && my < win.y + bezel;
                let on_bottom = my >= win.y + win.height - bezel && my < win.y + win.height;
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
                    let _ = window.drag_resize_window(d);
                    return true;
                }
            }
        }

        // 2. Forward to the L1 bridge (drives InputCoordinator).
        let focused = self.layout.ctx_mut().input.focused_widget().cloned();
        let coord = &mut self.layout.ctx_mut().input;
        let out = self.bridge.handle_event(coord, focused.as_ref(), event);

        // 3. Mirror click flags into the per-frame InputState so route_click
        //    fires in tick_inner.
        if let Some(((x, y), _)) = out.left_up {
            self.input.pointer.pos = Some((x, y));
            self.input.pointer.clicked = Some(MouseButton::Left);
        }
        if let Some(((x, y), _)) = out.left_down {
            self.input.pointer.pos = Some((x, y));
            self.input.pointer.button_down = Some(MouseButton::Left);
        }
        if let Some(((x, y), _)) = out.wheel {
            self.input.pointer.pos = Some((x, y));
        }
        if let Some(((x, y), (dx, dy))) = out.wheel {
            let _ = (x, y);
            self.input.scroll_delta.0 += dx;
            self.input.scroll_delta.1 += dy;
        }
        if let Some((x, y)) = out.cursor_moved {
            self.input.pointer.pos = Some((x, y));
        }

        false
    }

    /// Process one frame: FPS cap guard → poll events → layout → begin/ui/submit.
    ///
    /// Called from the winit `ApplicationHandler::window_event(RedrawRequested)`
    /// path.  Does nothing if the render state has not yet been initialised.
    ///
    /// The `event_loop` parameter is needed only for the `WaitUntil` FPS cap.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::Backend`] if GPU submission signals a lost
    /// surface.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tick(
        &mut self,
        window: &mut dyn WindowProvider,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), RuntimeError> {
        // ── FPS cap guard (mlc pattern, main.rs:2171-2184) ────────────────────
        // winit wakes the event loop on every mouse event (CursorMoved at
        // 125-500 Hz), which preempts the WaitUntil timer.  We exit early so
        // no scene work or GPU submission happens until the target interval has
        // actually elapsed.
        let fps_limit = self.fps_limit();
        if fps_limit > 0 {
            let target_dt = std::time::Duration::from_secs_f64(1.0 / fps_limit as f64);
            if self.last_frame.elapsed() < target_dt {
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    self.last_frame + target_dt,
                ));
                return Ok(());
            }
        }

        self.tick_inner(window)
    }

    /// Process one frame driven by `requestAnimationFrame` (wasm32 only).
    ///
    /// Unlike [`tick`](Self::tick) this method does not perform an FPS cap
    /// guard — the browser's RAF callback already runs at the display refresh
    /// rate (typically 60 Hz).
    ///
    /// Does nothing if the render state has not yet been initialised.
    #[cfg(target_arch = "wasm32")]
    pub fn tick_web(&mut self, window: &mut dyn WindowProvider) {
        let _ = self.tick_inner(window);
    }

    /// Shared per-frame work called by both `tick` and `tick_web`.
    fn tick_inner(
        &mut self,
        window: &mut dyn WindowProvider,
    ) -> Result<(), RuntimeError> {
        if self.render_state.is_none() {
            return Ok(());
        }

        // 1. Build per-frame InputState snapshot from the bridge's accumulated
        //    state. Raw winit events are routed into InputCoordinator on
        //    arrival via `Runtime::handle_winit_event`; here we just freeze
        //    the cursor / time for the frame.
        let now_secs = self.start.elapsed().as_secs_f64();
        self.input.time = now_secs;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (mx, my) = self.bridge.last_mouse_pos;
            self.input.pointer.pos = Some((mx, my));
        }

        // Drain whatever the provider has buffered; we don't pipeline them
        // further — the bridge has already updated coord state. We still
        // forward to `App::on_event` for apps that watch for window-level
        // events (theme change, file drops, etc.).
        for ev in window.poll_events() {
            let _ = self.app.on_event(&ev);
        }

        // 2. Solve macro layout with the current window rect.
        let rect = window.window_rect();
        self.layout.solve(rect);

        // 3. ContextManager begin_frame.
        let viewport = self.layout.rect_for_dock_area().unwrap_or(rect);
        self.layout.ctx_mut().begin_frame(self.input.clone(), viewport);

        // Read all immutable values before the render_state mutable borrow.
        let base_color = argb_to_alpha_color(self.config.background);
        let msaa = self.msaa_samples();

        // 4-7. begin_frame, ui, end_frame, submit — render_state mutable borrow scope.
        let outcome = {
            let render_state = self.render_state.as_mut()
                .expect("render_state checked above — cannot be None here");

            // 4. begin_frame on render state (resets scene / cpu buffer).
            render_state.begin_frame();

            // 4b. Publish frame timestamp into LayoutManager so builders
            //     (chrome tooltip clock, animations) can read it without
            //     the app passing it explicitly.
            self.layout.set_frame_time_ms(self.start.elapsed().as_secs_f64() * 1000.0);

            // 5. User UI callback.
            self.app.ui(&mut self.layout, render_state);

            // 5b. Auto-route a left-click released this frame through
            //     `App::route_click`, which decodes via
            //     `LayoutManager::handle_click` and fans out to typed
            //     `on_*` callbacks.  App that needs raw access can override
            //     `route_click` to return early.
            if matches!(self.input.pointer.clicked, Some(uzor::input::MouseButton::Left)) {
                if let Some((x, y)) = self.input.pointer.pos {
                    let _ = self.app.route_click(&mut self.layout, x, y);
                }
            }

            // 6. Collect widget responses.
            let _responses = self.layout.ctx_mut().end_frame();

            // 7. GPU / CPU submit.
            submit_frame(render_state, SubmitParams { base_color, msaa_samples: msaa })
        };

        if outcome.surface_lost {
            return Err(RuntimeError::Backend("wgpu surface lost (out of memory)".into()));
        }

        // 8. Update hub metrics.
        if let Some(ref mut h) = self.hub {
            h.update_metrics(outcome.metrics);
        }

        // 9. Record frame completion for FPS cap.
        self.last_frame = std::time::Instant::now();

        // 10. Reset per-frame fields.
        self.input.pointer.clicked = None;
        self.input.pointer.double_clicked = None;
        self.input.scroll_delta = (0.0, 0.0);

        // 11. Request next redraw (no-op on wasm32 — RAF handles this).
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
