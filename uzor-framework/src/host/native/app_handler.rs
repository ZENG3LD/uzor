//! Native winit `ApplicationHandler` (design §2.3).
//!
//! The host translates events and executes commands; it never decides
//! routing, layout, or intent delivery — those stay in the runtime.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use uzor::RenderBackend;
use uzor_render_hub::RenderHub;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId as WinitWindowId;

use super::clipboard::NativeClipboard;
use super::executor::{self, ExecutorFollowUp};
use super::paint::paint_frame;
use super::screenshot::{capture_screenshot, encode_png};
use super::single_instance::{self, SingleInstanceGuard};
use super::tray::{TrayBuilder, TrayEvent, TrayHandle, TraySpec};
use super::window::NativeWindow;
use super::window_create::{build_hub, create_window, geometry_echo};
use crate::runtime::{Runtime, RuntimeConfig};
use crate::types::bus::{
    HostCaps, HostEvent, InputEnvelope, InputEvent, PointerInput, RenderInfo, WindowInput,
};
use crate::types::error::FrameworkError;
use crate::types::frame::Wake;
use crate::types::ids::{Seconds, WindowId};
use crate::types::window::{RenderCmd, WindowCommand, WindowSpec};
use crate::{App, Spec, Waker};

/// User-event payload that wakes the loop from another thread.
#[derive(Debug, Clone, Copy)]
pub enum HostWake {
    /// A handle dispatch arrived; run a tick.
    Wake,
}

/// Native host: one runtime, a map of OS windows, a render hub.
pub struct NativeHost<S: Spec, A: App<Spec = S>> {
    runtime: Runtime<S, A>,
    windows: HashMap<WinitWindowId, NativeWindow>,
    by_id: HashMap<WindowId, WinitWindowId>,
    hub: RenderHub,
    clipboard: NativeClipboard,
    tray: Option<TrayHandle>,
    tray_spec: Option<TraySpec>,
    /// Pending spawn specs that could not be created yet (before resumed).
    pending: Vec<(WindowId, WindowSpec)>,
    start: Instant,
    /// When true, the next about_to_wait should exit after draining.
    exit: bool,
    /// Cursor locked: device mouse motion becomes RawDelta.
    raw_delta: bool,
}

impl<S: Spec, A: App<Spec = S>> NativeHost<S, A> {
    fn now(&self) -> Seconds {
        Seconds(self.start.elapsed().as_secs_f64())
    }

    fn caps() -> HostCaps {
        HostCaps {
            multi_window: true,
            #[cfg(target_os = "windows")]
            os_resize_bezel: true,
            #[cfg(not(target_os = "windows"))]
            os_resize_bezel: false,
            clipboard_async: false,
        }
    }

    fn push(&mut self, window: WindowId, event: InputEvent) {
        self.runtime.push_input(InputEnvelope {
            window,
            t: self.now(),
            event,
        });
    }

    fn execute_all(&mut self, el: &ActiveEventLoop, cmds: Vec<(WindowId, WindowCommand)>) {
        for (id, cmd) in cmds {
            self.execute_one(el, id, cmd);
        }
    }

    fn execute_one(&mut self, el: &ActiveEventLoop, id: WindowId, cmd: WindowCommand) {
        // Spawn / Exit / clipboard / screenshot / render do not need the
        // window surface first.
        match &cmd {
            WindowCommand::Spawn(spec) => {
                let spec = spec.clone();
                match create_window(el, &self.hub, id, spec) {
                    Ok(nw) => {
                        let (size, dpr, position) = geometry_echo(&nw.window);
                        let wid = nw.winit_id();
                        self.windows.insert(wid, nw);
                        self.by_id.insert(id, wid);
                        self.push(
                            id,
                            InputEvent::Window(WindowInput::Created {
                                size,
                                dpr,
                                position,
                                caps: Self::caps(),
                            }),
                        );
                        // Install tray on first successful window.
                        if self.tray.is_none() {
                            if let Some(spec) = self.tray_spec.take() {
                                match TrayBuilder::from_spec(spec).build() {
                                    Ok(t) => self.tray = Some(t),
                                    Err(e) => log::warn!("tray build failed: {e}"),
                                }
                            }
                        }
                    }
                    Err(e) => log::warn!("spawn window {id:?} failed: {e}"),
                }
                return;
            }
            WindowCommand::ExitApp => {
                self.exit = true;
                el.exit();
                return;
            }
            _ => {}
        }

        let Some(&wid) = self.by_id.get(&id) else {
            log::warn!("execute: unknown window {id:?}");
            return;
        };
        let follow = {
            let Some(nw) = self.windows.get_mut(&wid) else {
                return;
            };
            let mut os = nw.as_os();
            executor::execute(&mut os, cmd)
        };

        let Some(follow) = follow else {
            return;
        };
        match follow {
            ExecutorFollowUp::ClipboardWrite(text) => self.clipboard.write(&text),
            ExecutorFollowUp::ClipboardRead(ticket) => {
                let result = self.clipboard.read(ticket);
                self.push(id, InputEvent::Clipboard(result));
            }
            ExecutorFollowUp::Screenshot(ticket) => self.take_screenshot(id, ticket),
            ExecutorFollowUp::Spawn(spec) => {
                // Unreachable: Spawn handled above. Keep for exhaustiveness.
                let _ = spec;
            }
            ExecutorFollowUp::Close => {
                if let Some(wid) = self.by_id.remove(&id) {
                    self.windows.remove(&wid);
                    self.push(id, InputEvent::Window(WindowInput::Destroyed));
                }
                if self.windows.is_empty() {
                    self.exit = true;
                    el.exit();
                }
            }
            ExecutorFollowUp::ExitApp => {
                self.exit = true;
                el.exit();
            }
            ExecutorFollowUp::Render(cmd) => self.apply_render(cmd),
            ExecutorFollowUp::OuterRectEcho { position, size } => {
                self.push(
                    id,
                    InputEvent::Window(WindowInput::OuterRect { position, size }),
                );
            }
        }
    }

    fn apply_render(&mut self, cmd: RenderCmd) {
        match cmd {
            RenderCmd::SetBackend(b) => {
                if let Err(e) = self.hub.set_active(b) {
                    log::warn!("set_backend failed: {e}");
                }
            }
            RenderCmd::SetFpsLimit(fps) => self.hub.set_fps_limit(fps),
            RenderCmd::SetMsaa(n) => self.hub.set_msaa(n),
            RenderCmd::SetVsync(on) => self.hub.set_vsync(on),
        }
    }

    fn take_screenshot(&mut self, id: WindowId, ticket: crate::types::ids::Ticket) {
        let Some(&wid) = self.by_id.get(&id) else {
            return;
        };
        let Some(nw) = self.windows.get_mut(&wid) else {
            return;
        };
        let Some((device, queue, surface)) = nw.render_state.gpu_handles() else {
            log::warn!("screenshot: no GPU surface for {id:?}");
            return;
        };
        let Some((pixels, w, h)) = capture_screenshot(device, queue, surface, None) else {
            return;
        };
        let Some(png) = encode_png(&pixels, w, h) else {
            return;
        };
        self.push(
            id,
            InputEvent::Host(HostEvent::Screenshot {
                ticket,
                png: Arc::<[u8]>::from(png),
            }),
        );
    }

    fn drain_tray(&mut self) {
        let mut events = Vec::new();
        if let Some(tray) = self.tray.as_ref() {
            while let Some(ev) = tray.next_event() {
                events.push(ev);
            }
        }
        for ev in events {
            if let TrayEvent::MenuClick(id) = ev {
                // Tray has no window; attach to the first open window.
                if let Some((&win, _)) = self.by_id.iter().next() {
                    self.push(win, InputEvent::Host(HostEvent::Tray { id }));
                }
            }
        }
    }

    fn paint_due(&mut self, frames: Vec<crate::types::frame::FrameRequest>) {
        for req in frames {
            let Some(&wid) = self.by_id.get(&req.window) else {
                continue;
            };
            // Split borrows: take the window entry, paint, put metrics back.
            let Some(mut nw) = self.windows.remove(&wid) else {
                continue;
            };
            if let Err(e) = paint_frame(&mut self.runtime, &mut self.hub, &mut nw.render_state, &req)
            {
                log::warn!("paint failed: {e}");
            }
            // Report render facts.
            let m = *self.hub.metrics();
            let info = RenderInfo {
                backend: Some(self.hub.active()),
                available: self.hub.available_backends(),
                fps_ema: 0.0,
                frame_ms: m.submit_us as f64 / 1000.0,
                frame_count: 0,
            };
            self.windows.insert(wid, nw);
            self.push(req.window, InputEvent::Host(HostEvent::RenderInfo(info)));
        }
    }
}

impl<S: Spec, A: App<Spec = S>> ApplicationHandler<HostWake> for NativeHost<S, A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let pending = std::mem::take(&mut self.pending);
        for (id, spec) in pending {
            self.execute_one(el, id, WindowCommand::Spawn(spec));
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WinitWindowId, ev: WindowEvent) {
        let Some(nw) = self.windows.get_mut(&id) else {
            return;
        };
        let framework_id = nw.id;

        // Scale factor: update mapper and emit a real Resized with current size.
        if let WindowEvent::ScaleFactorChanged { scale_factor, .. } = &ev {
            let _ = nw.mapper.map(&ev);
            let (size, _, _) = geometry_echo(&nw.window);
            self.push(
                framework_id,
                InputEvent::Window(WindowInput::Resized {
                    size,
                    dpr: *scale_factor,
                }),
            );
            return;
        }

        if matches!(ev, WindowEvent::RedrawRequested) {
            // Force an immediate tick+paint for this window by waking.
            // about_to_wait does the real work; request a poll.
            el.set_control_flow(ControlFlow::Poll);
            return;
        }

        let events = nw.mapper.map(&ev);
        for e in events {
            self.push(framework_id, e);
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if !self.raw_delta {
            return;
        }
        if let DeviceEvent::MouseMotion { delta } = event {
            if let Some((&win, _)) = self.by_id.iter().next() {
                self.push(
                    win,
                    InputEvent::Pointer(PointerInput::RawDelta {
                        dx: delta.0,
                        dy: delta.1,
                    }),
                );
            }
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, _event: HostWake) {
        // about_to_wait will tick; nothing else to do.
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.drain_tray();
        let out = self.runtime.tick(self.now());
        self.execute_all(el, out.window_commands);
        self.paint_due(out.frames);

        if self.exit || self.runtime.should_exit() {
            el.exit();
            return;
        }

        el.set_control_flow(match out.wake {
            Wake::Idle => ControlFlow::Wait,
            Wake::Immediate => ControlFlow::Poll,
            Wake::At(t) => {
                let now = self.now().get();
                let target = t.get();
                if target <= now {
                    ControlFlow::Poll
                } else {
                    let dur = std::time::Duration::from_secs_f64(target - now);
                    ControlFlow::WaitUntil(Instant::now() + dur)
                }
            }
        });
    }
}

/// Prefer a fixed backend when set; otherwise autodetection.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderPreference {
    /// Explicit backend, or `None` for autodetection.
    pub backend: Option<RenderBackend>,
}

/// Extra native-only startup options (design §6.1).
#[derive(Clone, Debug, Default)]
pub struct NativeOptions {
    /// Optional single-instance mutex name.
    pub single_instance: Option<String>,
    /// Optional system tray.
    pub tray: Option<TraySpec>,
    /// Render backend preference.
    pub render: RenderPreference,
}

/// Run the app on the native host (feature `native`).
pub fn run_native<S, A>(
    app: A,
    cfg: RuntimeConfig,
    native: NativeOptions,
) -> Result<(), FrameworkError>
where
    S: Spec,
    A: App<Spec = S>,
{
    let _guard: Option<SingleInstanceGuard> = native
        .single_instance
        .as_deref()
        .map(single_instance::single_instance);

    let event_loop = EventLoop::<HostWake>::with_user_event()
        .build()
        .map_err(|e| FrameworkError::Host(format!("event loop: {e}")))?;
    let proxy = event_loop.create_proxy();
    let waker = Waker::new(move || {
        let _ = proxy.send_event(HostWake::Wake);
    });

    let (runtime, _handle) = Runtime::new(app, cfg, waker);
    let hub = build_hub(native.render.backend);

    let mut host = NativeHost {
        runtime,
        windows: HashMap::new(),
        by_id: HashMap::new(),
        hub,
        clipboard: NativeClipboard::new(),
        tray: None,
        tray_spec: native.tray,
        pending: Vec::new(),
        start: Instant::now(),
        exit: false,
        raw_delta: false,
    };

    // The first tick drains Open commands into Spawn follow-ups. Run it
    // before the loop so resumed() can create the windows.
    let out = host.runtime.tick(host.now());
    for (id, cmd) in out.window_commands {
        if let WindowCommand::Spawn(spec) = cmd {
            host.pending.push((id, spec));
        }
    }

    event_loop
        .run_app(&mut host)
        .map_err(|e| FrameworkError::Host(format!("event loop run: {e}")))?;
    Ok(())
}

/// Convenience: run with default native options.
pub fn run_native_default<S, A>(app: A, cfg: RuntimeConfig) -> Result<(), FrameworkError>
where
    S: Spec,
    A: App<Spec = S>,
{
    run_native(app, cfg, NativeOptions::default())
}
