//! # Level 4 — Dashboard (stub)
//!
//! Polygon for the L4 framework architecture.  Will be fleshed out as the
//! `uzor-framework::lm` chainable builders gain enough plumbing (auto-anchor,
//! body-rect lookup, cursor/time frame context, app-router dispatch, dock-leaf
//! iteration) to express an L3-grade UI without rect math in app code.
//!
//! For now: a blank window with default chrome — proves the framework runtime
//! starts cleanly.  Subsequent commits replace the body with real composites
//! built via `uzor_framework::lm::*` builders.
//!
//! # Run
//!
//! ```sh
//! cargo run --example level4_dashboard -p uzor-framework
//! ```

use uzor::layout::LayoutManager;
use uzor_framework::{App, AppBuilder, NoPanel};
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

struct DashboardApp;

impl DashboardApp {
    fn new() -> Self {
        Self
    }
}

impl App<NoPanel> for DashboardApp {
    fn init(&mut self, _layout: &mut LayoutManager<NoPanel>) {}

    fn ui(&mut self, _layout: &mut LayoutManager<NoPanel>, _render: &mut WindowRenderState) {
        // Stub — fill in via lm::* builders as the framework layer matures.
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DashboardApp::new())
        .title("uzor — L4 Dashboard")
        .size(1400, 900)
        .min_size(Some((900, 600)))
        .decorations(true)
        .background(0xFFF7F7F4)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;
    Ok(())
}
