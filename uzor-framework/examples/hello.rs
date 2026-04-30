//! Minimal hello-world example for `uzor-framework`.
//!
//! Shows the structurally correct wiring between:
//! - [`winit::event_loop::EventLoop`] (owns the OS event pump)
//! - [`uzor_window_desktop::WinitWindowProvider`] (implements `WindowProvider`)
//! - [`uzor_render_vello_gpu::VelloGpuSurfaceFactory`] (implements `RenderSurfaceFactory`)
//! - [`uzor_framework::AppBuilder`] (fluent builder)
//!
//! # Compile
//!
//! ```sh
//! cargo build --example hello -p uzor-framework
//! ```
//!
//! # Runtime status (v1)
//!
//! The example compiles and constructs all objects. `AppBuilder::run()` will
//! enter the event loop, create the vello GPU renderer via the factory, and
//! then return `RuntimeError::SurfaceWiringRequired` because `Runtime::run()`
//! does not yet hold the `RenderSurface` (the wgpu swapchain handle) alongside
//! the `WindowRenderState`. Full GPU frame submission is the next milestone.
//!
//! See `uzor-framework/src/runtime.rs` module-level docs for the planned API.
//!
//! # Event-loop integration note (winit 0.30)
//!
//! winit 0.30 uses `ApplicationHandler` — the event loop is run with
//! `EventLoop::run_app(&mut handler)`. `WinitWindowProvider` is designed to be
//! fed events from inside the handler callback via `push_winit_event`. The
//! `AppBuilder` path (used here) wraps this integration internally.
//!
//! If you need full control over the event loop (e.g. for chromeless windows,
//! custom drag regions, or tray icon integration), construct the event loop
//! yourself, create the window in `ApplicationHandler::resumed`, wrap it with
//! `WinitWindowProvider::new(Arc::clone(&window))`, and feed events manually.

use std::sync::Arc;

use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use uzor_window_hub::{WinitWindowProvider, WindowProvider};

// ─── Direct event-loop variant (preferred for v1) ────────────────────────────
//
// The AppBuilder path (below) calls Runtime::run() which immediately returns
// SurfaceWiringRequired. The direct path shows how to wire WinitWindowProvider
// manually so consumers can see the full control flow.

struct HelloHandler {
    provider: Option<WinitWindowProvider>,
}

impl winit::application::ApplicationHandler for HelloHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.provider.is_none() {
            let attrs = Window::default_attributes()
                .with_title("uzor hello")
                .with_inner_size(winit::dpi::LogicalSize::new(800_u32, 600_u32));
            match event_loop.create_window(attrs) {
                Ok(w) => {
                    self.provider = Some(WinitWindowProvider::new(Arc::new(w)));
                }
                Err(e) => eprintln!("window creation failed: {e}"),
            }
        }
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
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Frame callback: poll_events → solve layout → ui → submit.
                // Full implementation once Runtime gains a tick() method or the
                // surface-wiring gap is closed.
                let _events = provider.poll_events();
            }
            ref ev => {
                provider.push_winit_event(ev);
            }
        }
    }
}

// ─── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Option A: AppBuilder path ────────────────────────────────────────────
    //
    // This is the intended consumer API once Runtime::run() is complete.
    // For v1 it returns RuntimeError::SurfaceWiringRequired immediately after
    // creating the renderer.
    //
    // Uncomment to test:
    //
    // let event_loop = EventLoop::new()?;
    // let window = event_loop.create_window(Window::default_attributes()
    //     .with_title("uzor hello")
    //     .with_inner_size(winit::dpi::LogicalSize::new(800u32, 600u32)))?;
    // let provider = WinitWindowProvider::new(Arc::new(window));
    //
    // let result = AppBuilder::new(Hello { counter: 0 })
    //     .title("uzor hello")
    //     .size(800, 600)
    //     .backend(RenderBackend::VelloGpu)
    //     .window(Box::new(provider))
    //     .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
    //     .run();
    //
    // eprintln!("AppBuilder result: {result:?}");

    // ── Option B: Direct event-loop (works today) ────────────────────────────
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = HelloHandler { provider: None };
    event_loop.run_app(&mut handler)?;

    Ok(())
}
