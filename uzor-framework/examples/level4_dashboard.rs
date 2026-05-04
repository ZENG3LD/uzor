//! # Level 4 — Dashboard (multi-window proof)
//!
//! Polygon: chrome strip with the "+" button spawns a second window.
//! No body content yet — content rendering inside the window doesn't work
//! reliably; this example only validates the framework's window/chrome
//! plumbing.
//!
//! Run:
//!
//! ```sh
//! cargo run --example level4_dashboard -p uzor-framework
//! ```

use uzor::core::types::Rect;
use uzor_framework::{
    view, App, AppBuilder, NoPanel, WindowCtx, WindowKey, WindowSpec,
};
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory};

struct DashboardApp;

impl DashboardApp {
    fn new() -> Self { Self }
}

impl App<NoPanel> for DashboardApp {
    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        let dock = win.layout.last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });

        let layout       = &mut *win.layout;
        let render_state = &mut *win.render;
        render_state.with_render_context(|render| {
            view! {
                <col rect={dock}>
                    <chrome show_new_window=true />
                </col>
            }
        });
        let _ = layout;
    }

    fn on_chrome_new_window(&mut self, _source: &WindowKey) -> Option<WindowSpec> {
        // Always spawn a fresh extra window on each "+" click.
        // No de-duplication — we want to verify the manager handles many.
        Some(
            WindowSpec::new(
                WindowKey::new(format!("extra-{}", random_suffix())),
                "uzor — extra",
            )
            .size(560, 420)
            .min_size(420, 320)
            .decorations(false)
            .background(0xFF_F7_F7_F4),
        )
    }
}

/// Tiny unique-ish suffix to give each spawned WindowKey a different id.
fn random_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_micros() as u64).unwrap_or(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DashboardApp::new())
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor — L4 Dashboard")
                .size(1400, 900)
                .min_size(900, 600)
                .decorations(false)
                .background(0xFF_F7_F7_F4),
        )
        .icon_from_png(include_bytes!("assets/icon.png"))?
        .tray("uzor — L4 dashboard")
        .tray_item("show",  "Show window")
        .tray_item("quit",  "Quit")
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;
    Ok(())
}
