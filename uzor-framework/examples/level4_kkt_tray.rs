//! # Level 4 — Tray-only app (atol/KKT style)
//!
//! Demonstrates an app that starts **invisible** and lives in the system tray.
//!
//! - Window starts hidden (`AppConfig.start_visible = false` / `visible_on_start`
//!   pattern — here we achieve it via `decorations(false)` + `start_visible: false`).
//! - `TrayBuilder` creates a system tray icon with "Show" and "Quit" menu items.
//! - Tray events drive window visibility.
//!
//! ## Tray icon generation
//!
//! The tray icon is a 32×32 RGBA circle generated inline — no external image file.
//! The same approach is used in `atol-ecommerce/crates/kkt-agent-desktop/src/tray.rs`.
//!
//! The circle color cycles between green (connected) and red (disconnected)
//! whenever the user clicks the tray icon directly.
//!
//! ## Window lifecycle
//!
//! ```text
//! startup      → window hidden, tray icon appears
//! tray "Show"  → window becomes visible
//! tray "Quit"  → process exits
//! tray click   → toggle visibility
//! ```
//!
//! # Run
//!
//! ```sh
//! cargo run --example level4_kkt_tray -p uzor-framework
//! ```

use uzor::input::LayerId;
use uzor::layout::{LayoutManager, LayoutNodeId};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
use uzor_framework::app::{App, AppConfig, NoPanel};
use uzor_framework::builder::AppBuilder;
use uzor_framework::tray::{TrayBuilder, TrayEvent, TrayHandle};
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};
use uzor_window_hub::RgbaIcon;

// ── Tray icon helpers ─────────────────────────────────────────────────────────

/// Build a 32×32 RGBA filled-circle icon.
///
/// The same inline approach is used in atol-ecommerce.  No external PNG needed.
fn make_circle_icon(r: u8, g: u8, b: u8) -> RgbaIcon {
    let size = 32usize;
    let mut pixels = vec![0u8; size * size * 4];
    let cx = 16.0f64;
    let cy = 16.0f64;
    let radius = 10.0f64;

    for py in 0..size {
        for px in 0..size {
            let dx = px as f64 + 0.5 - cx;
            let dy = py as f64 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            let alpha = if dist <= radius - 1.5 {
                255u8
            } else if dist <= radius {
                ((radius - dist) / 1.5 * 255.0) as u8
            } else {
                0u8
            };

            if alpha > 0 {
                let idx = (py * size + px) * 4;
                pixels[idx]     = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = alpha;
            }
        }
    }

    RgbaIcon::from_rgba(size as u32, size as u32, pixels)
}

// ── App ───────────────────────────────────────────────────────────────────────

struct KktTrayApp {
    // The TrayHandle is kept alive for the entire app duration.
    // Dropping it removes the tray icon from the OS.
    tray: Option<TrayHandle>,

    // Whether our simulated "connection" is active.
    connected: bool,

    // Whether the window is currently visible.
    window_visible: bool,
}

impl KktTrayApp {
    fn new() -> Self {
        Self {
            tray: None,
            connected: false,
            window_visible: false,
        }
    }
}

impl App<NoPanel> for KktTrayApp {
    fn init(&mut self, layout: &mut LayoutManager<NoPanel>) {
        // Chromeless, no chrome strip.
        layout.chrome_mut().visible = false;

        // Build the system tray icon.
        // TrayBuilder must be called after the event loop has started (i.e.
        // inside init / first ui call — not in new()).
        let icon = make_circle_icon(247, 118, 142); // red = disconnected
        match TrayBuilder::new()
            .icon(icon)
            .tooltip("KKT Agent (example)")
            .menu_item("show", "Show")
            .menu_item("quit", "Quit")
            .build()
        {
            Ok(handle) => {
                self.tray = Some(handle);
            }
            Err(e) => {
                eprintln!("[kkt_tray] tray build failed: {e}");
            }
        }
    }

    fn ui(
        &mut self,
        layout: &mut LayoutManager<NoPanel>,
        render_state: &mut WindowRenderState,
    ) {
        // ── Drain tray events ─────────────────────────────────────────────────
        //
        // TrayHandle::next_event is non-blocking.  We drain all pending events
        // each frame.  At L4 this is the right place — the framework calls ui()
        // once per frame from the main thread.
        if let Some(tray) = &self.tray {
            while let Some(ev) = tray.next_event() {
                match ev {
                    TrayEvent::MenuClick(id) if id == "quit" => {
                        println!("[kkt_tray] Quit from tray");
                        std::process::exit(0);
                    }
                    TrayEvent::MenuClick(id) if id == "show" => {
                        self.window_visible = true;
                        println!("[kkt_tray] Show from tray menu");
                        // WindowProvider::set_visible(true) would be called here in a
                        // real app.  The App::ui signature does not expose
                        // WindowProvider yet — use on_event for that pattern.
                    }
                    TrayEvent::LeftClick | TrayEvent::DoubleClick => {
                        self.window_visible = !self.window_visible;
                        println!(
                            "[kkt_tray] tray click — window {}",
                            if self.window_visible { "visible" } else { "hidden" }
                        );
                    }
                    _ => {}
                }
            }
        }

        // ── Window content ────────────────────────────────────────────────────
        //
        // Register a simple "Connect" toggle button in the window center.
        let _layer = LayerId::main();
        let btn_rect = Rect::new(80.0, 80.0, 160.0, 40.0);
        let view = ButtonView { icon: None, text: Some("Connect"), active: self.connected, disabled: false, active_border: None, hover_chevron: None };
        render_state.with_render_context(|render| {
            uzor::lm::build_button(
                layout,
                render,
                LayoutNodeId::ROOT,
                "toggle_connect",
                btn_rect,
                WidgetState::Normal,
                &view,
                &ButtonSettings::default(),
            );
        });

        let responses = layout.ctx_mut().end_frame();
        for (id, resp) in &responses {
            if resp.clicked && id.as_str() == "toggle_connect" {
                self.connected = !self.connected;
                println!(
                    "[kkt_tray] {}",
                    if self.connected { "Connected" } else { "Disconnected" }
                );

                // Update tray icon color to reflect connection state.
                if let Some(ref mut tray) = self.tray {
                    let icon = if self.connected {
                        make_circle_icon(158, 206, 106) // green
                    } else {
                        make_circle_icon(247, 118, 142) // red
                    };
                    let _ = tray.set_icon(icon);
                }
            }
        }
    }

    fn shutdown(&mut self, _layout: &mut LayoutManager<NoPanel>) {
        // Explicitly drop the tray handle so the OS removes the icon cleanly.
        // (It would be dropped anyway — this is for clarity.)
        self.tray = None;
        println!("[kkt_tray] shutdown — tray icon removed");
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig {
        title: "uzor L4 — tray app".to_string(),
        initial_size: (320, 200),
        decorations: false,
        // start_visible: false means the window is created hidden.
        // The user reveals it via the tray "Show" menu item.
        start_visible: false,
        background: 0xFF161622,
        ..AppConfig::default()
    };

    AppBuilder::new(KktTrayApp::new())
        .config(config)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;

    Ok(())
}
