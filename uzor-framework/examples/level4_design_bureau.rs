//! # Level 4 — Design Bureau
//!
//! Single L4 polygon for **all** framework-level concerns: window control,
//! backend autodetect, chrome, tray, icons, theming, multi-pane app shell.
//! Built exclusively on top of the L3 framework API:
//!
//!   `uzor_framework::lm::build_*` for L3 widgets,
//!   `BlackboxHandler` for free-form panel content (chat / IDE / browser
//!   / render preview),
//!   `AppBuilder` + `RenderHub` for runtime + GPU.
//!
//! Zero `unsafe_widget_id`, zero `register_input_coordinator_*`, zero
//! `register_context_manager_*` outside blackbox handlers.
//!
//! ## Concept
//!
//! The Design Bureau is an in-progress shell for an "agent + IDE + browser
//! + render preview" workflow.  Phase by phase we fill it in:
//!
//! 1. App skeleton — chrome, window controls, tray, render-hub metrics.
//! 2. Workspace shell — sidebar (file tree), toolbar (actions), dock with
//!    multiple blackbox leaves (Chat / IDE / Browser / Render).
//! 3. Modal Settings (theme, backend switch, FPS limit) via `lm::build_modal`.
//! 4. Tray-driven minimize/restore.
//! 5. (later) Real agent backend, real webview embed, real Rust playground.
//!
//! Each gap discovered while building this app turns into a framework-level
//! addition (helper, builder method, runtime hook).
//!
//! # Run
//!
//! ```sh
//! cargo run --example level4_design_bureau -p uzor-framework
//! ```

use uzor::layout::{LayoutManager, LayoutNodeId};
use uzor_framework::{AppBuilder, App, NoPanel};
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

// ── App ───────────────────────────────────────────────────────────────────────

struct DesignBureauApp {
    // Stub fields — filled in over the next phases.
    _placeholder: (),
}

impl DesignBureauApp {
    fn new() -> Self {
        Self { _placeholder: () }
    }
}

impl App<NoPanel> for DesignBureauApp {
    fn init(&mut self, _layout: &mut LayoutManager<NoPanel>) {
        // Phase 1 — chrome / edges configuration goes here.
    }

    fn ui(&mut self, _layout: &mut LayoutManager<NoPanel>, _render_state: &mut WindowRenderState) {
        // Phase 1 — register chrome, toolbars, sidebar, dock leaves.
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DesignBureauApp::new())
        .title("uzor — Design Bureau")
        .size(1400, 900)
        .min_size(Some((900, 600)))
        .decorations(true)
        .background(0xFF12141A)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;
    Ok(())
}
