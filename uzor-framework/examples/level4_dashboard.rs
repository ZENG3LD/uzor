//! # Level 4 — E-commerce dashboard
//!
//! Demonstrates a typical desktop dashboard layout:
//!
//! - OS-decorated chrome (no custom titlebar needed here).
//! - Left sidebar (200 px) with navigation buttons.
//! - Right sidebar (180 px) with settings buttons.
//! - Main dock area: chart placeholder (colored rect with grid lines).
//!
//! ## Layout structure
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  Left sidebar  │   Main (chart area)    │  Right sidebar │
//! │  200 px        │   fills remaining      │  180 px        │
//! │  [Dashboard]   │   ┌──────────────────┐ │  [Settings]    │
//! │  [Orders]      │   │  Grid/Chart area │ │  [Theme]       │
//! │  [Products]    │   └──────────────────┘ │  [Notifications]│
//! └─────────────────────────────────────────┘
//! ```
//!
//! Edge slots are the L3 way to allocate strips around the dock area.
//! The left/right sidebars use `EdgeSide::Left` / `EdgeSide::Right`.
//!
//! The chart placeholder is registered as a `BlackboxPanel` composite —
//! this tells `is_over_ui()` to return `false` when the cursor is over it,
//! allowing the hypothetical chart's own input system to handle events.
//!
//! # Run
//!
//! ```sh
//! cargo run --example level4_dashboard -p uzor-framework
//! ```

use uzor::input::core::sense::Sense;
use uzor::input::core::widget_kind::WidgetKind;
use uzor::input::LayerId;
use uzor::layout::{EdgeSide, EdgeSlot, LayoutManager};
use uzor::types::{Rect, WidgetId, WidgetState};
use uzor::ui::widgets::atomic::button::input::register_layout_manager_button;
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
use uzor_framework::app::{App, NoPanel};
use uzor_framework::builder::AppBuilder;
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

// ── App ───────────────────────────────────────────────────────────────────────

struct DashboardApp {
    active_nav: String,
}

impl DashboardApp {
    fn new() -> Self {
        Self { active_nav: "nav_dashboard".to_string() }
    }
}

impl App<NoPanel> for DashboardApp {
    fn init(&mut self, layout: &mut LayoutManager<NoPanel>) {
        // Use OS-native window decorations for the dashboard.
        // No custom chrome needed — the OS titlebar is fine here.
        layout.chrome_mut().visible = false;

        // Left sidebar: 200 px wide
        layout.edges_mut().add(EdgeSlot {
            id: "left_sidebar".to_string(),
            side: EdgeSide::Left,
            thickness: 200.0,
            visible: true,
            order: 0,
        });

        // Right sidebar: 180 px wide
        layout.edges_mut().add(EdgeSlot {
            id: "right_sidebar".to_string(),
            side: EdgeSide::Right,
            thickness: 180.0,
            visible: true,
            order: 0,
        });
    }

    fn ui(
        &mut self,
        layout: &mut LayoutManager<NoPanel>,
        render_state: &mut WindowRenderState,
    ) {
        let layer = LayerId::main();
        let settings = ButtonSettings::default();
        let view = ButtonView { icon: None, text: None, active: false, disabled: false, active_border: None, hover_chevron: None };

        // ── Left sidebar navigation buttons ──────────────────────────────────
        if let Some(left_rect) = layout.rect_for_edge_slot("left_sidebar") {
            let btn_w = left_rect.width - 24.0;
            let btn_h = 36.0;
            let btn_x = left_rect.x + 12.0;

            let nav_items = [
                ("nav_dashboard", "Dashboard"),
                ("nav_orders",    "Orders"),
                ("nav_products",  "Products"),
                ("nav_customers", "Customers"),
            ];

            for (i, (id, _label)) in nav_items.iter().enumerate() {
                let btn_y = left_rect.y + 16.0 + i as f64 * (btn_h + 8.0);
                render_state.with_render_context(|render| {
                    register_layout_manager_button(
                        layout,
                        render,
                        *id,
                        Rect::new(btn_x, btn_y, btn_w, btn_h),
                        &layer,
                        WidgetState::Normal,
                        &view,
                        &settings,
                    );
                });
            }
        }

        // ── Right sidebar settings buttons ────────────────────────────────────
        if let Some(right_rect) = layout.rect_for_edge_slot("right_sidebar") {
            let btn_w = right_rect.width - 24.0;
            let btn_h = 36.0;
            let btn_x = right_rect.x + 12.0;

            let settings_items = [
                ("set_theme",         "Theme"),
                ("set_notifications", "Notifications"),
                ("set_export",        "Export data"),
            ];

            for (i, (id, _label)) in settings_items.iter().enumerate() {
                let btn_y = right_rect.y + 16.0 + i as f64 * (btn_h + 8.0);
                render_state.with_render_context(|render| {
                    register_layout_manager_button(
                        layout,
                        render,
                        *id,
                        Rect::new(btn_x, btn_y, btn_w, btn_h),
                        &layer,
                        WidgetState::Normal,
                        &view,
                        &settings,
                    );
                });
            }
        }

        // ── Main area: chart placeholder ──────────────────────────────────────
        //
        // Register the dock area as a BlackboxPanel.
        //
        // BlackboxPanel signals to the InputCoordinator:
        //   - "This region manages its own input."
        //   - `is_over_ui()` returns `false` when hovered.
        //   - Children cannot be registered inside it via `register_child`.
        //
        // This is the correct pattern for embedding a chart, webview, or
        // other self-contained canvas that has its own hit-testing.
        if let Some(dock) = layout.rect_for_dock_area() {
            layout.ctx_mut().input.register_composite(
                WidgetId::new("chart_canvas"),
                WidgetKind::BlackboxPanel,
                dock,
                Sense::NONE,
                &layer,
            );
        }

        // ── Collect responses ─────────────────────────────────────────────────
        let responses = layout.ctx_mut().end_frame();
        for (id, resp) in &responses {
            if resp.clicked {
                match id.0.as_str() {
                    id @ ("nav_dashboard" | "nav_orders" | "nav_products" | "nav_customers") => {
                        self.active_nav = id.to_string();
                        println!("[dashboard] nav: {id}");
                    }
                    "set_theme"         => println!("[dashboard] theme settings"),
                    "set_notifications" => println!("[dashboard] notification settings"),
                    "set_export"        => println!("[dashboard] export data"),
                    _ => {}
                }
            }
        }
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DashboardApp::new())
        .title("uzor L4 — e-commerce dashboard")
        .size(800, 600)
        .decorations(true) // OS chrome is fine for a dashboard
        .background(0xFF111118)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;

    Ok(())
}
