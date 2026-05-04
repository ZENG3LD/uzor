//! # Level 4 — Dashboard
//!
//! L4 polygon using JSX-mimicking `view!` macro on top of existing `lm::*`
//! builders.  Demonstrates: chrome strip, body widgets, opt-in settings modal.
//!
//! Run:
//!
//! ```sh
//! cargo run --example level4_dashboard -p uzor-framework
//! ```

use uzor::core::types::Rect;
use uzor::layout::{LayoutManager, ModalHandle};
use uzor::ui::widgets::composite::chrome::types::ChromeTabConfig;
use uzor_framework::tokens;
use uzor_framework::{view, App, AppBuilder, NoPanel};
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory, WindowRenderState};

struct DashboardApp {
    settings:    Option<ModalHandle>,
    show_modal:  bool,
    dark:        bool,
    sounds_on:   bool,
    save_clicks: u32,
}

impl DashboardApp {
    fn new() -> Self {
        Self { settings: None, show_modal: false, dark: false, sounds_on: true, save_clicks: 0 }
    }
}

impl App<NoPanel> for DashboardApp {
    fn init(&mut self, layout: &mut LayoutManager<NoPanel>) {
        self.settings = Some(layout.add_modal("settings"));
    }

    fn ui(&mut self, layout: &mut LayoutManager<NoPanel>, render_state: &mut WindowRenderState) {
        // Dock area = full window (chrome is an overlay, not a layout slot).
        // We carve the body content area below the chrome strip ourselves.
        let dock = layout
            .last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
        let chrome_h = layout.chrome().height as f64;
        let body = Rect {
            x:      dock.x,
            y:      dock.y + chrome_h,
            width:  dock.width,
            height: (dock.height - chrome_h).max(0.0),
        };

        let tabs = [
            ChromeTabConfig { id: "dashboard", label: "Dashboard", icon: None, color_tag: None, closable: false, active: true },
            ChromeTabConfig { id: "logs",      label: "Logs",      icon: None, color_tag: None, closable: true,  active: false },
        ];
        let modal_handle = self.settings.as_ref().expect("init() ran");

        render_state.with_render_context(|render| {
            view! {
                <col rect={body}>
                    <chrome tabs={&tabs} active_tab="dashboard" />
                    <col gap=12 pad=24>
                        <text   text="L4 Dashboard" color={tokens::colors::fg::fg_0} />
                        <button text="Save"
                                bind_count={&mut self.save_clicks}
                                on_click={|| { /* save */ }} />
                        <button text="Open settings…"
                                on_click={|| { self.show_modal = true; }} />
                        <checkbox bind={&mut self.dark}      label="Dark mode" />
                        <checkbox bind={&mut self.sounds_on} label="Sounds" />
                        <separator />
                        <text text="click Save to bump counter • no id strings"
                              color={tokens::colors::fg::fg_2} />
                    </col>

                    { if self.show_modal {
                        view! {
                            <modal handle={modal_handle} title="Settings" resizable=true gap=10 pad=20>
                                <text text="Settings (close to dismiss — backdrop click)"
                                      color={tokens::colors::fg::fg_1} />
                                <checkbox bind={&mut self.dark}      label="Dark mode" />
                                <checkbox bind={&mut self.sounds_on} label="Sounds" />
                            </modal>
                        }
                    } }
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
        .decorations(false) // borderless — our <chrome> handles drag/min/max/resize
        .background(0xFFF7F7F4)
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;
    Ok(())
}
