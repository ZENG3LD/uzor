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
//! cargo run -p uzor-examples --bin l4-dashboard
//! ```

use uzor::core::types::Rect;
use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::platform::types::CornerStyle;
use uzor_desktop::AppRun as _;
use uzor_framework_macros::view;

struct DashboardApp;

impl DashboardApp {
    fn new() -> Self { Self }
}

impl App<NoPanel> for DashboardApp {
    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        let dock = win.layout.last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });

        let layout = &mut *win.layout;
        let render = &mut *win.render;
        view! {
            <col rect={dock}>
                <chrome show_new_window=true />
            </col>
        }
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
                .background(0xFF_F7_F7_F4)
                // Mirage-accent rounded corners + border on Windows 11.
                .corner_style(CornerStyle::Rounded)
                .border_color(0x00_FB_B2_6A),
        )
        .icon_from_png(include_bytes!("../../assets/icon.png"))?
        .tray("uzor — L4 dashboard")
        .tray_item("show",  "Show window")
        .tray_item("quit",  "Quit")
        .run()?;
    Ok(())
}
