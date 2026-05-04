//! # Level 4 — Dashboard
//!
//! Multi-window L4 polygon: "main" dashboard + on-demand "settings" window
//! spawned via `App::take_pending_spawn` and routed through the framework's
//! `WindowManager`.  Both windows share state through `&mut self`.
//!
//! Run:
//!
//! ```sh
//! cargo run --example level4_dashboard -p uzor-framework
//! ```

use uzor::core::types::Rect;
use uzor::layout::LayoutManager;
use uzor_framework::tokens;
use uzor_framework::{
    view, App, AppBuilder, NoPanel, WindowCtx, WindowKey, WindowSpec,
};
use uzor_render_hub::{RenderBackend, VelloGpuSurfaceFactory};

struct DashboardApp {
    dark:        bool,
    sounds_on:   bool,
    save_clicks: u32,
    /// Set when the user clicks "Open settings" — drained by the runtime
    /// in `take_pending_spawn` to create a second window.
    spawn_settings: bool,
    /// Set when the settings window is open so we don't queue duplicates.
    settings_open: bool,
    /// Set when "Close settings" is clicked — drained by the runtime to
    /// destroy the matching window without exiting the app.
    close_settings: bool,
}

impl DashboardApp {
    fn new() -> Self {
        Self {
            dark: false, sounds_on: true, save_clicks: 0,
            spawn_settings: false, settings_open: false, close_settings: false,
        }
    }
}

impl App<NoPanel> for DashboardApp {
    fn init(&mut self, _key: &WindowKey, _layout: &mut LayoutManager<NoPanel>) {
        // No per-window setup needed — chrome height + edges live at default.
    }

    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        // dock_area is the full window (chrome is overlay, not a layout slot).
        // Carve the body content rect below the chrome strip.
        let dock = win.layout.last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
        let chrome_h = win.layout.chrome().height as f64;
        let body = Rect {
            x:      dock.x,
            y:      dock.y + chrome_h,
            width:  dock.width,
            height: (dock.height - chrome_h).max(0.0),
        };

        // Branch on which window we're drawing.
        match win.key.as_str() {
            "main" => {
                let layout  = &mut *win.layout;
                let render_state = &mut *win.render;
                render_state.with_render_context(|render| {
                    view! {
                        <col rect={body}>
                            <chrome />
                            <col gap=12 pad=24>
                                <text   text="L4 Dashboard" color={tokens::colors::fg::fg_0} />
                                <button text="Save"
                                        bind_count={&mut self.save_clicks}
                                        on_click={|| { /* save */ }} />
                                <button text="Open settings…"
                                        on_click={|| {
                                            if !self.settings_open {
                                                self.spawn_settings = true;
                                            }
                                        }} />
                                <checkbox bind={&mut self.dark}      label="Dark mode" />
                                <checkbox bind={&mut self.sounds_on} label="Sounds" />
                                <separator />
                                <text text="click Save to bump counter • no id strings"
                                      color={tokens::colors::fg::fg_2} />
                            </col>
                        </col>
                    }
                });
                let _ = layout;
            }
            "settings" => {
                let layout  = &mut *win.layout;
                let render_state = &mut *win.render;
                render_state.with_render_context(|render| {
                    view! {
                        <col rect={body}>
                            <chrome />
                            <col gap=12 pad=24>
                                <text text="Settings"        color={tokens::colors::fg::fg_0} />
                                <text text="(separate window, shared state)"
                                      color={tokens::colors::fg::fg_2} />
                                <separator />
                                <checkbox bind={&mut self.dark}      label="Dark mode" />
                                <checkbox bind={&mut self.sounds_on} label="Sounds" />
                                <button text="Close settings"
                                        on_click={|| { self.close_settings = true; }} />
                            </col>
                        </col>
                    }
                });
                let _ = layout;
            }
            _ => {}
        }
    }

    fn take_pending_spawn(&mut self) -> Option<WindowSpec> {
        if std::mem::take(&mut self.spawn_settings) {
            self.settings_open = true;
            Some(
                WindowSpec::new(WindowKey::new("settings"), "uzor — Settings")
                    .size(560, 420)
                    .min_size(420, 320)
                    .decorations(false)
                    .background(0xFF_F7_F7_F4),
            )
        } else {
            None
        }
    }

    fn take_window_to_close(&mut self) -> Option<WindowKey> {
        if std::mem::take(&mut self.close_settings) {
            self.settings_open = false;
            Some(WindowKey::new("settings"))
        } else {
            None
        }
    }
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
        // Window/Alt-Tab/taskbar icon (runtime).  Same RgbaIcon is reused
        // by `.tray(...)` below so the system tray matches the window.
        .icon_from_png(include_bytes!("assets/icon.png"))?
        .tray("uzor — L4 dashboard")
        .tray_item("show",  "Show window")
        .tray_item("about", "About uzor")
        .tray_item("quit",  "Quit")
        .backend(RenderBackend::VelloGpu)
        .surface_factory(Box::new(VelloGpuSurfaceFactory::new()))
        .run()?;
    Ok(())
}
