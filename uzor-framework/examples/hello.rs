//! Minimal hello-world example for `uzor-framework`.
//!
//! Demonstrates the complete wiring between:
//! - [`AppBuilder`] — fluent builder (owns EventLoop + Window internally)
//! - [`VelloGpuSurfaceFactory`] — creates GPU surface from the window handle
//! - [`App`] trait — user-supplied frame callback
//!
//! # Run
//!
//! ```sh
//! cargo run --example hello -p uzor-framework
//! ```
//!
//! Opens an 800 × 600 window and renders a solid dark-blue background frame
//! in a continuous loop.  Close the window to exit.
//!
//! # What this example exercises
//!
//! 1. `AppBuilder::run()` creates a winit `EventLoop` and `Window` internally.
//! 2. `VelloGpuSurfaceFactory::create_render_state` is called once on `Resumed`
//!    and now keeps the `RenderSurface` alive inside `WindowRenderState::Gpu`.
//! 3. `Runtime::tick()` is called each `RedrawRequested` and drives:
//!    `begin_frame → app.ui → submit_frame → request_redraw`.

use uzor::layout::LayoutManager;
use uzor_framework::app::{App, NoPanel};
use uzor_framework::builder::AppBuilder;
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

// ─── Hello app ────────────────────────────────────────────────────────────────

struct Hello;

impl App<NoPanel> for Hello {
    fn ui(
        &mut self,
        _layout: &mut LayoutManager<NoPanel>,
        _state: &mut WindowRenderState,
    ) {
        // No widgets yet — the clear colour from AppConfig is enough to verify
        // that the frame loop runs and the GPU surface is alive.
        // Future: draw a rectangle or text once the widget API is wired through
        // render_state.scene_mut().
    }
}

// ─── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(Hello)
        .title("uzor hello")
        .size(800, 600)
        .decorations(true)
        .background(0xFF181820) // dark navy — visible sign the GPU path works
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;

    Ok(())
}

