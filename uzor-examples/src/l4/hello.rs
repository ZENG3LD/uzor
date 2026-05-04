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
//! cargo run -p uzor-examples --bin l4-hello
//! ```
//!
//! Opens an 800 × 600 chromeless window and renders a solid dark-blue
//! background frame in a continuous loop. Close the window to exit.

use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::WindowCtx;
use uzor_desktop::AppRun as _;

// ─── Hello app ────────────────────────────────────────────────────────────────

struct Hello;

impl App<NoPanel> for Hello {
    fn ui(&mut self, _win: &mut WindowCtx<'_, NoPanel>) {
        // No widgets — the per-window background colour from WindowSpec is
        // enough to verify the frame loop runs and the GPU surface is alive.
    }
}

// ─── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(Hello)
        .title("uzor hello")
        .size(800, 600)
        .decorations(false)
        .background(0xFF181820) // dark navy — visible sign the GPU path works
        .run()?;

    Ok(())
}
