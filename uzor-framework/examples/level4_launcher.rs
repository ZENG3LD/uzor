//! # Level 4 — Framework launcher (same concept as L2, but via App trait)
//!
//! This example demonstrates the **L4** API surface using `uzor-framework`:
//!
//! - `AppBuilder` owns winit, the event loop, the GPU surface, and the render loop.
//! - User only implements the [`App`] trait (one `ui` callback per frame).
//! - `register_chrome_default` wires the titlebar in one call.
//!
//! ## What the framework removes (compared to L2)
//!
//! At L2 the user writes ~120 LOC of wgpu/vello/winit boilerplate.
//! At L4 that is replaced by a single `AppBuilder::new(MyApp).run()` call.
//!
//! ## Drag region
//!
//! `WindowProvider::drag_window()` is available inside `on_event` to start
//! a native drag.  In a real app you'd call it when a `MouseInput::Pressed`
//! lands on a non-widget area of the chrome.  Here we omit it for brevity —
//! the chrome composite already handles drag internally via its registered
//! widget IDs.
//!
//! # Run
//!
//! ```sh
//! cargo run --example level4_launcher -p uzor-framework
//! ```

use uzor::layout::{LayoutManager, LayoutNodeId};
use uzor::types::WidgetState;
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
use uzor_framework::app::{App, NoPanel};
use uzor_framework::builder::AppBuilder;
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

// ── App ───────────────────────────────────────────────────────────────────────

struct LauncherApp {
    /// Track how many times the connect button was clicked.
    connect_clicks: u32,
}

impl LauncherApp {
    fn new() -> Self {
        Self { connect_clicks: 0 }
    }
}

impl App<NoPanel> for LauncherApp {
    fn init(&mut self, layout: &mut LayoutManager<NoPanel>) {
        // Configure chrome: 32 px strip, visible.
        // The framework calls this once before the first frame.
        layout.chrome_mut().height = 32.0;
        layout.chrome_mut().visible = true;
    }

    fn ui(
        &mut self,
        layout: &mut LayoutManager<NoPanel>,
        render_state: &mut WindowRenderState,
    ) {
        // ── Register the "Connect" button at L3 level ────────────────────────
        //
        // L3 = uzor::lm::build_button.
        // It takes the LayoutManager directly, extracts ContextManager internally,
        // pulls ButtonState from the registry, and calls draw_button.
        let btn_rect = uzor::types::Rect::new(80.0, 100.0, 160.0, 40.0);
        let view = ButtonView { icon: None, text: Some("Connect"), active: false, disabled: false, active_border: None, hover_chevron: None };
        render_state.with_render_context(|render| {
            uzor::lm::build_button(
                layout,
                render,
                LayoutNodeId::ROOT,
                "connect_btn",
                btn_rect,
                WidgetState::Normal,
                &view,
                &ButtonSettings::default(),
            );
        });

        // Collect responses
        let responses = layout.ctx_mut().end_frame();
        for (id, resp) in &responses {
            if resp.clicked && id.as_str() == "connect_btn" {
                self.connect_clicks += 1;
                println!("[L4 launcher] Connect clicked ({}x)", self.connect_clicks);
            }
        }
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // AppBuilder is the L4 entry point.
    // It owns winit, the event loop, window creation, GPU surface, and render loop.
    // User code only provides the App impl above.
    AppBuilder::new(LauncherApp::new())
        .title("uzor L4 — framework launcher")
        .size(320, 240)
        .decorations(false) // chromeless — we draw our own titlebar
        .background(0xFF161620)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;

    Ok(())
}
