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
///
/// LM owns the logical state (provider, chrome, dock-tree, modals,
/// pointer state).  This struct holds only what stays on the platform
/// side: the raw winit handle (for `request_redraw` and surface
/// re-creation) and the GPU render state (because `WindowRenderState`
/// is wgpu-bound and LM stays platform-agnostic).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct PerWindow<P: DockPanel> {
    pub key:             WindowKey,
    pub spec:            WindowSpec,
    pub window:          std::sync::Arc<Window>,
    pub render_state:    WindowRenderState,
    /// Last known cursor position in logical pixels.
    pub last_mouse_pos:  (f64, f64),
    pub last_frame:      std::time::Instant,
    pub initialised:     bool,
    pub close_requested: bool,
    /// Baseline repaint cadence for this window (resolved from
    /// `WindowSpec::tick_rate` or `AppConfig::default_tick_rate`).
    pub tick_rate:       uzor::render::TickRate,

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

    pub _phantom: std::marker::PhantomData<P>,
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

/// L4 window manager — thin winit→LayoutManager event pump.
///
/// Owns the App, the wgpu render hub, and a winit→WindowKey routing map.
/// The single `LayoutManager<P>` lives here too — all window-level state
/// (rect, provider, dock-tree, drag, init flag) is its responsibility,
/// addressed by `WindowKey`.  Render state stays in the per-window slot
/// because it's GPU-tied.
pub struct Manager<A: App<P>, P: DockPanel> {
    pub(crate) app:     A,
    pub(crate) config:  AppConfig,
    pub(crate) backend: RenderBackend,
    pub(crate) hub:     Option<RenderHub>,
    pub(crate) factory: Option<Box<dyn RenderSurfaceFactory>>,
    pub(crate) start:   std::time::Instant,

    /// The one and only LayoutManager. Holds dock tree, separators,
    /// overlays, edges, AND the registered windows (`layout.attach_window`).
    pub(crate) layout: LayoutManager<P>,

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

    // ── Agent API plumbing ──
    /// Snapshot + command-channel pair.  `None` until
    /// `enable_agent_api(port)` is called.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) agent_bus: Option<crate::agent::AgentBus>,
    /// Owned axum-server handle.  Dropped at shutdown.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) agent_handle: Option<uzor_agent_api::AgentApiHandle>,

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
            layout: LayoutManager::<P>::new(),
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
            #[cfg(not(target_arch = "wasm32"))]
            agent_bus: None,
            #[cfg(not(target_arch = "wasm32"))]
            agent_handle: None,
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

    /// Enable the local agent-API HTTP server on `127.0.0.1:port`.
    ///
    /// Spawns a tokio runtime in a dedicated thread and binds axum.
    /// The server stays alive for the manager's lifetime; dropping the
    /// manager closes it.  Errors only on bind failure (port taken).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn enable_agent_api(&mut self, port: u16) -> std::io::Result<()> {
        let bus = crate::agent::AgentBus::new();
        let control = bus.control();
        let handle = uzor_agent_api::spawn_server(
            control as std::sync::Arc<dyn uzor::layout::agent::AgentControl>,
            port,
        )?;
        self.agent_bus = Some(bus);
        self.agent_handle = Some(handle);
        Ok(())
    }

    /// Drain queued agent commands and apply them.  Called from
    /// `about_to_wait` so all writes land on the winit thread and on
    /// the same tick they were submitted.
    #[cfg(not(target_arch = "wasm32"))]
    fn drain_agent_commands(&mut self) {
        let mut batch = Vec::new();
        if let Some(bus) = self.agent_bus.as_mut() {
            while let Ok(item) = bus.cmd_rx.try_recv() {
                batch.push(item);
            }
        }
        for (cmd, reply) in batch {
            let result = self.apply_agent_command(cmd);
            let _ = reply.send(result);
        }
        // Pull tick_rate from LM branch into PerWindow so the
        // scheduler in `about_to_wait` honours `SetTickRate`
        // updates that landed this tick.
        let updates: Vec<(winit::window::WindowId, uzor::render::TickRate)> = self.windows
            .iter()
            .filter_map(|(id, pw)| {
                self.layout.window(&pw.key).map(|s| (*id, s.tick_rate))
            })
            .collect();
        for (id, rate) in updates {
            if let Some(pw) = self.windows.get_mut(&id) {
                pw.tick_rate = rate;
            }
        }
    }

    /// Apply one agent command and return a reply.
    ///
    /// First tries [`uzor::layout::agent::LmAgent::try_apply`]; if that
    /// returns `None`, the command needs platform handling (real OS
    /// window spawn / close, true synthetic input that also requests
    /// a window redraw).
    #[cfg(not(target_arch = "wasm32"))]
    fn apply_agent_command(
        &mut self,
        cmd: uzor::layout::agent::Command,
    ) -> uzor::layout::agent::CommandReply {
        use uzor::layout::agent::{Command, CommandReply, LmAgent};

        if let Some(reply) = LmAgent::<P>::try_apply(&mut self.layout, &cmd) {
            // A semantic LM op was handled — kick a redraw on the
            // affected window so the change paints next frame.
            if let Some(window_key) = command_window_key(&cmd) {
                let key = uzor::framework::multi_window::WindowKey::new(window_key);
                if let Some(id) = self.window_id_for(&key) {
                    if let Some(pw) = self.windows.get(&id) {
                        pw.window.request_redraw();
                    }
                }
            }
            return reply;
        }

        // WM-side commands (synthetic input that needs redraw, OS
        // window lifecycle).  We compute the reply, then log a
        // matching `AgentCommand` entry so the agent log mirrors what
        // the LM-side side already records via `LmAgent::log_command`.
        let reply = match cmd.clone() {
            Command::InjectHover { window, x, y } => {
                let key = uzor::framework::multi_window::WindowKey::new(window);
                if !self.layout.window_keys().any(|k| k == &key) {
                    return CommandReply::err("unknown window");
                }
                self.layout.set_current_window(key.clone());
                self.layout.on_pointer_move(x, y);
                if let Some(id) = self.window_id_for(&key) {
                    if let Some(pw) = self.windows.get(&id) { pw.window.request_redraw(); }
                }
                CommandReply::ok()
            }
            Command::InjectClick { window, x, y, button: _ } => {
                let key = uzor::framework::multi_window::WindowKey::new(window);
                if !self.layout.window_keys().any(|k| k == &key) {
                    return CommandReply::err("unknown window");
                }
                self.layout.set_current_window(key.clone());
                self.layout.on_pointer_move(x, y);
                self.layout.on_pointer_down(x, y);
                let _ = self.layout.on_pointer_up(x, y);
                if let Some(id) = self.window_id_for(&key) {
                    if let Some(pw) = self.windows.get(&id) { pw.window.request_redraw(); }
                }
                CommandReply::ok()
            }
            Command::InjectScroll { window, dx, dy } => {
                let key = uzor::framework::multi_window::WindowKey::new(window);
                if !self.layout.window_keys().any(|k| k == &key) {
                    return CommandReply::err("unknown window");
                }
                self.layout.set_current_window(key.clone());
                self.layout.on_scroll(dx, dy);
                if let Some(id) = self.window_id_for(&key) {
                    if let Some(pw) = self.windows.get(&id) { pw.window.request_redraw(); }
                }
                CommandReply::ok()
            }
            Command::SpawnWindow { key, title, width, height, background, decorations } => {
                let mut spec = uzor::framework::multi_window::WindowSpec::new(
                    uzor::framework::multi_window::WindowKey::new(key),
                    &title,
                )
                .size(width, height);
                if let Some(bg) = background { spec = spec.background(bg); }
                if let Some(dec) = decorations { spec = spec.decorations(dec); }
                self.pending_spawns.push(spec);
                CommandReply::ok()
            }
            Command::CloseWindow { key } => {
                let target = uzor::framework::multi_window::WindowKey::new(key);
                if let Some(id) = self.window_id_for(&target) {
                    if let Some(pw) = self.windows.get_mut(&id) { pw.close_requested = true; }
                    CommandReply::ok()
                } else {
                    CommandReply::err("unknown window")
                }
            }
            // Anything else is one of the LM-routable commands handled
            // above by `try_apply`, so we shouldn't reach here.
            _ => CommandReply::err("internal: unhandled command"),
        };

        uzor::layout::agent::LmAgent::<P>::log_command(&mut self.layout, &cmd, &reply);
        reply
    }

    /// Rebuild the snapshot the HTTP server's `GET` endpoints read.
    #[cfg(not(target_arch = "wasm32"))]
    fn rebuild_agent_snapshot(&mut self) {
        let Some(bus) = self.agent_bus.as_ref() else { return };
        let snap = crate::agent::build_snapshot(
            &self.layout,
            self.fps_ema,
            self.frame_count,
            self.start.elapsed().as_secs_f64() * 1000.0,
        );
        if let Ok(mut w) = bus.snapshot.write() { *w = snap; }
        let widgets = crate::agent::build_widget_list(&self.layout);
        if let Ok(mut w) = bus.widgets.write() { *w = widgets; }
        // Publish agent-log mirror.  Cheap: ring buffer is bounded.
        let log_entries = self.layout.agent_log().snapshot();
        if let Ok(mut w) = bus.log.write() { *w = log_entries; }
        // Publish blackbox registry mirror.  Each entry is just an
        // `Arc` clone — cheap, no lock contention with the surface.
        let bb_clone: std::collections::HashMap<_, _> = self.layout.blackbox_slots()
            .into_iter()
            .filter_map(|slot| {
                self.layout.find_blackbox_agent(&slot).map(|s| (slot, s))
            })
            .collect();
        if let Ok(mut w) = bus.blackboxes.write() { *w = bb_clone; }
    }

    /// Drain queued screenshot requests and produce PNG bytes from
    /// the last-rendered frame of the named window.
    ///
    /// GPU pipeline: ensure `target_texture` has `COPY_SRC` usage,
    /// run `crate::utils::capture_screenshot` to read pixels back,
    /// then `encode_png`.  Returns `None` for software-presented
    /// windows or unknown keys.
    #[cfg(not(target_arch = "wasm32"))]
    fn drain_agent_screenshots(&mut self) {
        let mut batch: Vec<crate::agent::ScreenshotRequest> = Vec::new();
        if let Some(bus) = self.agent_bus.as_ref() {
            while let Ok(req) = bus.shot_rx.try_recv() {
                batch.push(req);
            }
        }
        for req in batch {
            let png = self.capture_window_png(&req.window);
            let _ = req.reply.send(png);
        }
    }

    /// Synchronous PNG capture for the named window.  Patches the
    /// vello target texture with `COPY_SRC` on first call, then reads
    /// it back through a staging buffer.
    #[cfg(not(target_arch = "wasm32"))]
    fn capture_window_png(&mut self, window: &str) -> Option<Vec<u8>> {
        use crate::utils::screenshot::{
            add_copy_src_to_target_texture, capture_screenshot, encode_png,
        };
        let key = uzor::framework::multi_window::WindowKey::new(window);
        let id = self.window_id_for(&key)?;
        let pw = self.windows.get_mut(&id)?;
        // Patch the target texture lazily — once it has COPY_SRC the
        // flag persists across resizes (until vello replaces the
        // surface itself, in which case we re-patch).
        {
            let (device, _queue, surface) = pw.render_state.gpu_handles_mut()?;
            let needs_patch = !surface
                .target_texture
                .usage()
                .contains(wgpu::TextureUsages::COPY_SRC);
            if needs_patch {
                add_copy_src_to_target_texture(surface, device);
                // Force one more frame so the freshly-replaced
                // texture is filled.  Without this the buffer would
                // contain undefined pixels.
                pw.window.request_redraw();
                return None;
            }
        }
        let (device, queue, surface) = pw.render_state.gpu_handles()?;
        let (pixels, w, h) = capture_screenshot(device, queue, surface, None)?;
        encode_png(&pixels, w, h)
    }

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

        // Activate the agent control plane if the AppConfig requested it.
        if let Some(port) = self.config.agent_api_port {
            if let Err(e) = self.enable_agent_api(port) {
                eprintln!("[uzor-desktop] agent-api bind on :{port} failed: {e}");
            }
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
        // mlc-aligned policy: every window gets a wgpu swapchain.  Even
        // CPU backends (TinySkia, VelloCpu) render into a pixmap and
        // then upload it as a texture for blit + present through the
        // GPU.  This used to fork into a softbuffer-backed
        // `SurfaceMode::Software` path for CPU, which works for the
        // first window but loses pixels on subsequent spawns
        // (windows came up black).  The unified GPU swapchain path
        // also makes the screenshot endpoint usable on every backend.
        let active = self.hub.as_ref().map(|h| h.active()).unwrap_or(self.backend);
        let factory: Box<dyn RenderSurfaceFactory> = if let Some(hub) = self.hub.as_ref() {
            hub.factory_for(active)
                .ok_or_else(|| ManagerError::Backend(
                    format!("hub has no factory for backend {:?}", active)
                ))?
        } else {
            return Err(ManagerError::Backend("no hub initialised".into()));
        };

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
        let tick_rate = spec.tick_rate.unwrap_or(self.config.default_tick_rate);
        let pw = PerWindow::<P> {
            key:             spec.key.clone(),
            spec:            spec.clone(),
            window:          std::sync::Arc::clone(&window),
            render_state,
            last_mouse_pos:  (0.0, 0.0),
            last_frame:      std::time::Instant::now(),
            initialised:     false,
            close_requested: false,
            tick_rate,
            region_states:   std::collections::HashMap::new(),
            region_scenes:   std::collections::HashMap::new(),
            dock_separator_drag: None,
            _phantom:        std::marker::PhantomData,
        };
        self.windows.insert(id, pw);

        // Hand the provider over to LM — from now on LM owns the trait
        // object and routes everything (resize, redraw, drag, decorations,
        // platform-event queue) through the trait.
        self.layout.attach_window(spec.key.clone(), Box::new(provider));
        if let Some(slot) = self.layout.window_mut(&spec.key) {
            slot.tick_rate = tick_rate;
        }

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

        // Route LM to this window before any per-window state mutations.
        let key = match self.windows.get(&id) {
            Some(pw) => pw.key.clone(),
            None => return,
        };
        self.layout.set_current_window(key.clone());

        use winit::event::{ElementState, MouseButton as WMouseButton};

        match event {
            // ── Cursor moved ─────────────────────────────────────────────────
            WindowEvent::CursorMoved { position, .. } => {
                let Some(pw) = self.windows.get_mut(&id) else { return };
                let dpr = self.layout.window(&key)
                    .map(|s| s.provider.scale_factor())
                    .unwrap_or(1.0);
                let lx = position.x / dpr;
                let ly = position.y / dpr;
                pw.last_mouse_pos = (lx, ly);
                self.layout.on_pointer_move(lx, ly);

                // Drive an in-flight dock-separator drag.  panels_mut()
                // applies the per-pixel delta to the underlying split
                // ratio — the next frame's solve picks up the new sizes.
                if let Some(drag) = pw.dock_separator_drag.as_mut() {
                    use uzor::docking::panels::SeparatorOrientation as SO;
                    let orient = self.layout.panels()
                        .separators()
                        .get(drag.sep_idx)
                        .map(|s| s.orientation);
                    if let Some(orient) = orient {
                        let win = self.layout.last_window().unwrap_or(uzor::types::Rect::new(0.0, 0.0, 1.0, 1.0));
                        let delta = match orient {
                            SO::Vertical   => (lx - drag.last_x) as f32,
                            SO::Horizontal => (ly - drag.last_y) as f32,
                        };
                        self.layout.panels_mut().drag_separator(
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
                self.layout.on_pointer_down(mx, my);

                // Dock-separator drag start.  on_pointer_down already
                // wrote `last_pressed` via process_drag_press; check it
                // for the `dock-sep-N` id pattern.
                if let Some(pressed) = self.layout.last_pressed_widget() {
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
                let consumed = self.layout.handle_chrome_press(mx, my, &mut host, now_ms);

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
                if let Some(chrome_rect) = self.layout.rect_for_chrome() {
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
                        self.layout.chrome_state(), &view, &settings, &kind,
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
                let _outcome = self.layout.on_pointer_up(mx, my);
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

        // Route LM to this window for the duration of the tick.
        let active_backend = self.hub.as_ref().map(|h| h.active()).unwrap_or(self.backend);
        if let Some(pw) = self.windows.get_mut(&id) {
            // Sync per-window active backend with the global hub
            // setting and lazily build the matching renderer / CPU
            // context if the user just switched backends.
            if pw.render_state.backend() != active_backend {
                pw.render_state.set_active(active_backend);
            } else {
                pw.render_state.ensure_backend_slot(active_backend);
            }
            self.layout.set_current_window(pw.key.clone());
        } else {
            return Ok(());
        }

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
                self.app.init(&key, &mut self.layout);
                pw.initialised = true;
                if let Some(slot) = self.layout.window_mut(&key) {
                    slot.initialised = true;
                }
                self.layout.agent_log_push(
                    "lm.window.first_tick",
                    serde_json::json!({ "window": key.as_str() }),
                );
            }
        }
        // Bump LM-side tick counter so /state/tree reflects ticks.
        if let Some(pw) = self.windows.get(&id) {
            let key = pw.key.clone();
            if let Some(slot) = self.layout.window_mut(&key) {
                slot.tick_count = slot.tick_count.wrapping_add(1);
            }
        }

        let outcome = {
            let pw = match self.windows.get_mut(&id) {
                Some(p) => p,
                None    => return Ok(()),
            };

            let key = pw.key.clone();
            let events = self.layout.window_mut(&key)
                .map(|s| s.provider.poll_events())
                .unwrap_or_default();
            for ev in events {
                let _ = self.app.on_event(&ev);
            }

            let rect = self.layout.window(&key)
                .map(|s| s.provider.window_rect())
                .unwrap_or_default();
            self.layout.solve(rect);
            let viewport = self.layout.rect_for_dock_area().unwrap_or(rect);
            // Clear last frame's widget nodes BEFORE the app re-registers
            // them.  Without this the retained tree grows unbounded (one
            // dup per tick × per widget) — snapshot ends up with stale
            // labels at the front and current ones at the back.
            self.layout.begin_frame_widgets();
            // begin_frame clears one-shot input flags and refreshes widget registrations
            // WITHOUT overwriting the pointer state that on_pointer_* already set.
            self.layout.begin_frame(now_ms, viewport);
            self.layout.set_frame_time_ms(now_ms);

            let bg_color = argb_to_alpha_color(pw.spec.background);
            pw.render_state.begin_frame();

            let regions = self.app.regions();
            let now_inst = std::time::Instant::now();

            // Pick path:
            // - VelloGpu: per-region scene + composite (mlc pattern).
            // - CPU rasterisers (VelloCpu, TinySkia) and others:
            //   render every region into the SAME context in
            //   declaration order — pixmap accumulates, no scene
            //   compositing.  Without this dock leaves and other
            //   region-only widgets stay invisible on CPU backends.
            let active_backend = pw.render_state.backend();
            let supports_scene_compose = matches!(
                active_backend, uzor::platform::types::RenderBackend::VelloGpu,
            );
            let use_regions_compose = !regions.is_empty() && supports_scene_compose;
            let use_regions_inline  = !regions.is_empty() && !supports_scene_compose;

            {
                let key = &pw.key;
                let layout = &mut self.layout;
                let render_state = &mut pw.render_state;
                let region_states = &mut pw.region_states;
                let region_scenes = &mut pw.region_scenes;
                let app = &mut self.app;
                let fps_ema = self.fps_ema;
                let last_frame_time_ms = self.last_frame_time_ms;
                let frame_count = self.frame_count;
                let hub = self.hub.as_mut().expect("hub initialised");
                let mut hub_ctrl = HubControl { hub, fps_ema, last_frame_time_ms, frame_count };

                if use_regions_compose {
                    // GPU path: per-region scene + composite.
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
                    for region in &regions {
                        if let Some(rs) = region_scenes.get(region.id) {
                            render_state.append_region_scene(rs);
                        }
                    }
                } else if use_regions_inline {
                    // CPU path: paint every region into the shared
                    // pixmap in declaration order.  Each region
                    // handler is expected to know its own rect (from
                    // `WindowCtx.layout`), since CPU rasterisers
                    // don't support per-region scene compositing.
                    render_state.with_render_context(|render_ctx| {
                        let mut ctx = WindowCtx::<P> {
                            key,
                            layout,
                            render: render_ctx,
                            rect,
                            render_control: &mut hub_ctrl,
                        };
                        for region in &regions {
                            app.draw_region(region.id, &mut ctx);
                        }
                    });
                    for region in &regions {
                        let state = region_states
                            .entry(region.id)
                            .or_insert_with(uzor::render::RegionScheduleState::default);
                        state.last_painted = Some(now_inst);
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
            let _responses = self.layout.ctx_mut().end_frame();
            // Clear one-shot input flags AFTER app.ui consumed them.
            self.layout.end_frame_inputs();

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
                let key = self.windows.get(&id).map(|pw| pw.key.clone());
                if let Some(key) = key {
                    if let Some(platform_ev) = uzor_window_desktop::map_winit_event(ev, 1.0) {
                        if let Some(slot) = self.layout.window_mut(&key) {
                            slot.provider.push_platform_event(platform_ev);
                        }
                    }
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

        // 2.5. Drain agent-API write commands and apply them on this
        //      thread before any spawn / paint work picks up the new
        //      state.  Each command resolves a oneshot the HTTP handler
        //      is awaiting.
        self.drain_agent_commands();
        self.drain_agent_screenshots();

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

        // 3.5. Rebuild the agent snapshot after spawn/close so HTTP
        //      readers see the current window list and per-branch state.
        self.rebuild_agent_snapshot();

        // 4. Paint scheduler.
        //    a) Per-region cadence (mlc pattern) — drives intra-window
        //       sub-region rebuilds when the app declares them.
        //    b) Per-window baseline `TickRate` — guarantees every
        //       window keeps painting at its declared heartbeat even
        //       when no winit events fire.  Without this the agent API
        //       can mutate state but the user never sees it because
        //       `was_clicked` etc only fire while the window paints.
        let regions = self.app.regions();
        let now = std::time::Instant::now();
        let mut next_due: Option<std::time::Instant> = None;

        for pw in self.windows.values_mut() {
            let mut any_due_now = false;

            // (a) intra-window region cadence.
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

            // (b) per-window baseline tick.
            match pw.tick_rate {
                uzor::render::TickRate::Dirty => {}
                uzor::render::TickRate::Uncapped => {
                    any_due_now = true;
                }
                uzor::render::TickRate::Capped(fps) if fps > 0 => {
                    let target = std::time::Duration::from_secs_f64(1.0 / fps as f64);
                    let elapsed = now.saturating_duration_since(pw.last_frame);
                    if elapsed >= target {
                        any_due_now = true;
                    } else {
                        let nd = pw.last_frame + target;
                        next_due = Some(match next_due {
                            None => nd,
                            Some(cur) => cur.min(nd),
                        });
                    }
                }
                uzor::render::TickRate::Capped(_) => {} // fps == 0 acts like Dirty
            }

            if any_due_now {
                pw.window.request_redraw();
            }
        }

        // Decide control-flow: if anyone is uncapped or already due,
        // poll continuously; otherwise sleep until the soonest next-due.
        let any_uncapped = self.windows.values().any(|p|
            matches!(p.tick_rate, uzor::render::TickRate::Uncapped)
        );
        if any_uncapped {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else if let Some(t) = next_due {
            if t > now {
                event_loop.set_control_flow(ControlFlow::WaitUntil(t));
            } else {
                event_loop.set_control_flow(ControlFlow::Poll);
            }
        } else if !regions.is_empty() {
            event_loop.set_control_flow(ControlFlow::Poll);
        }
        // No change otherwise — ControlFlow::Wait remains for the
        // pure event-driven (Dirty) case.
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

/// Extract the affected window key from a command, if it carries one.
/// Used by the manager to request a redraw on the right OS window
/// after applying an LM-routable agent command.
#[cfg(not(target_arch = "wasm32"))]
fn command_window_key(cmd: &uzor::layout::agent::Command) -> Option<String> {
    use uzor::layout::agent::Command as C;
    match cmd {
        C::InjectHover  { window, .. }
        | C::InjectClick  { window, .. }
        | C::InjectScroll { window, .. }
        | C::ClickWidget  { window, .. }
        | C::HoverWidget  { window, .. }
        | C::OpenModal    { window, .. }
        | C::CloseModal   { window, .. }
        | C::OpenPopup    { window, .. }
        | C::ClosePopup   { window, .. }
        | C::OpenDropdown { window, .. }
        | C::CloseDropdown { window, .. }
        | C::ToggleSidebar { window, .. } => Some(window.clone()),
        C::SpawnWindow { key, .. } | C::CloseWindow { key } => Some(key.clone()),
        C::BlackboxClickWidget { window, .. } => Some(window.clone()),
        C::LogPush { window, .. } => window.clone(),
        C::SetTickRate { window, .. } => Some(window.clone()),
        C::SetSyncMode { .. } | C::ApplyStylePreset { .. } => None,
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
