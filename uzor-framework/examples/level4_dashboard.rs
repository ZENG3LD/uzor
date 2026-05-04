//! # Level 4 — Dashboard
//!
//! First polygon for the L4 framework architecture using the JSX-mimicking
//! `view!` macro on top of the existing `lm::*` builders.
//!
//! Run:
//!
//! ```sh
//! cargo run --example level4_dashboard -p uzor-framework
//! ```

use uzor::core::types::Rect;
use uzor::layout::LayoutManager;
use uzor_framework::{view, App, AppBuilder, NoPanel};
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

struct DashboardApp {
    dark:        bool,
    sounds_on:   bool,
    save_clicks: u32,
}

impl DashboardApp {
    fn new() -> Self {
        Self { dark: false, sounds_on: true, save_clicks: 0 }
    }
}

impl App<NoPanel> for DashboardApp {
    fn init(&mut self, _layout: &mut LayoutManager<NoPanel>) {}

    fn ui(&mut self, layout: &mut LayoutManager<NoPanel>, render_state: &mut WindowRenderState) {
        // Frame area (whole window for this stub).
        let body: Rect = layout
            .last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });

        render_state.with_render_context(|render| {
            view! {
                <col rect={body} gap=12 pad=24>
                    <text   text="L4 Dashboard" color="#1a1a1a" />
                    <button text="Save"
                            bind_count={&mut self.save_clicks}
                            on_click={|| { /* save */ }} />
                    <checkbox bind={&mut self.dark}      label="Dark mode" />
                    <checkbox bind={&mut self.sounds_on} label="Sounds" />
                    <separator />
                    <text text="↑ click Save to bump counter (no id strings)" color="#666" />
                </col>
            }
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DashboardApp::new())
        .title("uzor — L4 Dashboard (view!)")
        .size(1400, 900)
        .min_size(Some((900, 600)))
        .decorations(true)
        .background(0xFFF7F7F4)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;
    Ok(())
}
