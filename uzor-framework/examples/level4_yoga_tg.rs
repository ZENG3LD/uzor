//! # Level 4 — Telegram mini-app style portrait window
//!
//! Demonstrates a portrait-orientation app (600×800) with:
//!
//! - Top toolbar (40 px) with navigation buttons.
//! - Main content panel (scrollable placeholder).
//! - Bottom toolbar (56 px) with tab bar.
//!
//! This maps directly to the layout Telegram uses for mini-apps inside
//! their mobile client. The same code runs on desktop for development.
//!
//! ## Layout structure
//!
//! ```text
//! ┌────────────────────────────┐  ← top toolbar (edge slot, 40 px)
//! │  Back  │  Title  │  Share │
//! ├────────────────────────────┤
//! │                            │
//! │         Main panel         │  ← dock area (fills remaining height)
//! │     (content placeholder)  │
//! │                            │
//! ├────────────────────────────┤
//! │  Home  │  Search │ Profile │  ← bottom toolbar (edge slot, 56 px)
//! └────────────────────────────┘
//! ```
//!
//! Edge slots are the L3 way to carve out top/bottom strips from the dock area.
//! The dock area is then available for panel docking.
//!
//! # Run
//!
//! ```sh
//! cargo run --example level4_yoga_tg -p uzor-framework
//! ```

use uzor::input::LayerId;
use uzor::layout::{EdgeSide, EdgeSlot, LayoutManager};
use uzor::types::{Rect, WidgetId, WidgetState};
use uzor::ui::widgets::atomic::button::input::register_layout_manager_button;
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonView};
use uzor_framework::app::{App, NoPanel};
use uzor_framework::builder::AppBuilder;
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

// ── App ───────────────────────────────────────────────────────────────────────

struct YogaTgApp {
    active_tab: usize,
}

impl YogaTgApp {
    fn new() -> Self {
        Self { active_tab: 0 }
    }
}

impl App<NoPanel> for YogaTgApp {
    fn init(&mut self, layout: &mut LayoutManager<NoPanel>) {
        // No custom chrome — use OS decorations for this example.
        layout.chrome_mut().visible = false;

        // Top navigation toolbar: 40 px
        layout.edges_mut().add(EdgeSlot {
            id: "top_bar".to_string(),
            side: EdgeSide::Top,
            thickness: 40.0,
            visible: true,
            order: 0,
        });

        // Bottom tab bar: 56 px
        layout.edges_mut().add(EdgeSlot {
            id: "bottom_bar".to_string(),
            side: EdgeSide::Bottom,
            thickness: 56.0,
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

        // ── Top toolbar buttons ───────────────────────────────────────────────
        //
        // Register buttons relative to the top_bar edge rect.
        if let Some(top_rect) = layout.rect_for_edge_slot("top_bar") {
            let btn_h = top_rect.height - 8.0;
            let btn_y = top_rect.y + 4.0;

            // Back button (left edge)
            render_state.with_render_context(|render| {
                register_layout_manager_button(
                    layout,
                    render,
                    "top_back",
                    Rect::new(top_rect.x + 8.0, btn_y, 64.0, btn_h),
                    &layer,
                    WidgetState::Normal,
                    &view,
                    &settings,
                );
            });

            // Share button (right edge)
            render_state.with_render_context(|render| {
                register_layout_manager_button(
                    layout,
                    render,
                    "top_share",
                    Rect::new(top_rect.x + top_rect.width - 72.0, btn_y, 64.0, btn_h),
                    &layer,
                    WidgetState::Normal,
                    &view,
                    &settings,
                );
            });
        }

        // ── Bottom tab buttons ────────────────────────────────────────────────
        //
        // Three equal-width tab buttons across the bottom bar.
        if let Some(bot_rect) = layout.rect_for_edge_slot("bottom_bar") {
            let tab_w = bot_rect.width / 3.0;
            let tab_h = bot_rect.height - 8.0;
            let tab_y = bot_rect.y + 4.0;

            let tabs = ["tab_home", "tab_search", "tab_profile"];
            for (i, id) in tabs.iter().enumerate() {
                let tab_x = bot_rect.x + i as f64 * tab_w + 4.0;
                render_state.with_render_context(|render| {
                    register_layout_manager_button(
                        layout,
                        render,
                        *id,
                        Rect::new(tab_x, tab_y, tab_w - 8.0, tab_h),
                        &layer,
                        WidgetState::Normal,
                        &view,
                        &settings,
                    );
                });
            }
        }

        // ── Collect responses ─────────────────────────────────────────────────
        let responses = layout.ctx_mut().end_frame();
        for (id, resp) in &responses {
            if resp.clicked {
                match id.0.as_str() {
                    "top_back"      => println!("[yoga_tg] Back"),
                    "top_share"     => println!("[yoga_tg] Share"),
                    "tab_home"      => { self.active_tab = 0; println!("[yoga_tg] Home tab"); }
                    "tab_search"    => { self.active_tab = 1; println!("[yoga_tg] Search tab"); }
                    "tab_profile"   => { self.active_tab = 2; println!("[yoga_tg] Profile tab"); }
                    _ => {}
                }
            }
        }

        // Register the main content area as a BlackboxPanel placeholder.
        // BlackboxPanel tells the coordinator "I manage my own hit-testing".
        // A real app would render chart/list content here.
        if let Some(dock) = layout.rect_for_dock_area() {
            layout.ctx_mut().input.register_composite(
                WidgetId::new("main_content"),
                uzor::input::core::widget_kind::WidgetKind::BlackboxPanel,
                dock,
                uzor::input::core::sense::Sense::NONE,
                &LayerId::main(),
            );
        }
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(YogaTgApp::new())
        .title("uzor L4 — Telegram mini-app style")
        .size(600, 800)
        .decorations(true) // use OS chrome for this example
        .background(0xFF0F1117)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;

    Ok(())
}
