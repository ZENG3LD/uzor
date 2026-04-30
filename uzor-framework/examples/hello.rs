//! Minimal hello-world example for `uzor-framework`.
//!
//! Demonstrates the complete wiring between:
//! - [`AppBuilder`] — fluent builder (owns EventLoop + Window internally)
//! - [`VelloGpuSurfaceFactory`] — creates GPU surface from the window handle
//! - [`App`] trait — user-supplied frame callback
//!
//! # New in this revision
//!
//! - `AppBuilder::decorated(false)` — chromeless window (draws own titlebar).
//! - `AppBuilder::icon_from_png(...)` — load window icon from embedded PNG bytes.
//!   Uncomment the `.icon_from_png` line and provide a real PNG to exercise it.
//! - `uzor_icon::svg_bytes_to_rgba(...)` — SVG → RGBA at build/runtime for icons.
//!   Uncomment the `.icon` line and provide a real SVG to exercise it.
//! - `TrayBuilder` — system tray icon + menu.
//!   The tray portion is also in a commented block; it requires a tray-capable
//!   platform (Windows / macOS / Linux with system tray support).
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
    }
}

// ─── main ─────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Icon from SVG (uzor-icon) ─────────────────────────────────────────────
    //
    // Uncomment when you have a `logo.svg` next to this example file:
    //
    //   let icon_rgba = uzor_icon::svg_bytes_to_rgba(
    //       include_bytes!("logo.svg"),
    //       32,
    //   )?;
    //   let icon = uzor_window_hub::RgbaIcon::from_rgba(32, 32, icon_rgba);
    //
    // Then pass `.icon(icon)` to AppBuilder below.

    // ── Icon from PNG ─────────────────────────────────────────────────────────
    //
    // Uncomment when you have an `icon.png` next to this example file:
    //
    //   .icon_from_png(include_bytes!("icon.png"))?

    // ── System tray (TrayBuilder) ─────────────────────────────────────────────
    //
    // Uncomment for a tray icon with a Quit menu item:
    //
    //   use uzor_framework::{TrayBuilder, TrayEvent};
    //   let mut tray = TrayBuilder::new()
    //       .tooltip("uzor hello")
    //       .menu_item("quit", "Quit")
    //       .build()?;
    //
    // Then in a loop alongside the event loop:
    //   if let Some(TrayEvent::MenuClick(id)) = tray.next_event() {
    //       if id == "quit" { std::process::exit(0); }
    //   }

    AppBuilder::new(Hello)
        .title("uzor hello")
        .size(800, 600)
        // Pass `decorations(false)` for a chromeless window.
        // The app is responsible for drawing its own titlebar via
        // `uzor_framework::chrome::register_chrome_default`.
        .decorations(false)
        .background(0xFF181820) // dark navy — visible sign the GPU path works
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;

    Ok(())
}
