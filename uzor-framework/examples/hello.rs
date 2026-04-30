//! Minimal hello-world example for `uzor-framework`.
//!
//! Demonstrates the complete wiring between:
//! - [`AppBuilder`] — fluent builder (owns EventLoop + Window internally)
//! - [`RenderHub::autodetect`] — probes GPU, picks best backend
//! - [`App`] trait — user-supplied frame callback
//!
//! # Run
//!
//! ```sh
//! cargo run --example hello -p uzor-framework
//! ```
//!
//! Opens an 800 × 600 chromeless window and renders a solid dark-blue
//! background frame in a continuous loop. Close the window to exit.

use uzor::layout::LayoutManager;
use uzor_framework::app::{App, NoPanel};
use uzor_framework::builder::AppBuilder;
use uzor_render_hub::{RenderHub, WindowRenderState};

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
    }
}

// ─── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(Hello)
        .title("uzor hello")
        .size(800, 600)
        .decorations(false)
        .background(0xFF181820) // dark navy — visible sign the GPU path works
        .render_hub(RenderHub::autodetect())
        .run()?;

    Ok(())
}
