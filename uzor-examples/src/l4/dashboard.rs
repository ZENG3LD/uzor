//! # Level 4 — Dashboard (Settings + Painting panels)
//!
//! Main window split into two side-by-side panels:
//!   LEFT  (280 px fixed) — Settings: backend selector, vsync, msaa, fps, theme.
//!   RIGHT (flex=1)       — Stub painting panel with static text.
//!
//! Chrome strip at top with "+" spawns extra windows.
//!
//! **StyleManager demo**: two "Mirage Dark" / "Mirage Light" theme buttons in the
//! Settings panel switch the palette without touching any per-widget settings.
//! All chrome buttons + panel backgrounds pick up the change automatically
//! because `lm::*` builders re-read `layout.styles()` every frame.
//!
//! Run:
//!
//! ```sh
//! cargo run -p uzor-examples --bin l4-dashboard
//! ```

use uzor::core::types::Rect;
use uzor::docking::panels::DockPanel;
use uzor::framework::app::App;
use uzor::layout::{EdgeSide, EdgeSlot};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::layout::{MirageDarkPreset, MirageLightPreset};
use uzor::platform::types::CornerStyle;
use uzor::types::unsafe_widget_id;
use uzor_desktop::AppRun as _;
use uzor_framework_macros::view;

#[path = "tree_debug.rs"]
mod tree_debug;

/// Custom DockPanel for the Painting page — one leaf per cadence.
#[derive(Debug, Clone)]
struct PaintPanel {
    id:        &'static str,
    title:     &'static str,
    target_fps: u32,
}

impl Default for PaintPanel {
    fn default() -> Self {
        Self { id: "paint:default", title: "", target_fps: 0 }
    }
}

impl DockPanel for PaintPanel {
    fn title(&self) -> &str { self.title }
    fn type_id(&self) -> &'static str { self.id }
    fn min_size(&self) -> (f32, f32) { (120.0, 80.0) }
    fn closable(&self) -> bool { false }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Dark,
    Light,
}

struct DashboardApp {
    current_theme: ThemeMode,
    tick_counter:  u64,
    /// Per-cell rebuild counters (incremented every time the runtime calls
    /// draw_region for that cell). Demonstrates that cells with different
    /// target_fps values rebuild at different rates.
    cell_counts: [u64; 4],
    /// Per-cell composite panel state — each painting cell is a real
    /// `PanelState`, not just a `fill_rect` background.
    cell_panel_states: [uzor::ui::widgets::composite::panel::state::PanelState; 4],
    /// Live state for the tree-debug blackbox.  Registered with LM
    /// in `init` so HTTP `/blackbox/tree-debug/...` routes resolve.
    tree_debug_state: Option<std::sync::Arc<std::sync::Mutex<tree_debug::TreeDebugState>>>,
    /// State for the Settings composite Panel (scroll offset, hover
    /// flags).  Wrapping the edge-slot content in a Panel gives us a
    /// proper overflow guard, so the bottom rows clip / scroll
    /// instead of spilling past the pixmap on small windows.
    settings_panel_state: uzor::ui::widgets::composite::panel::state::PanelState,
}

impl DashboardApp {
    fn new() -> Self {
        use uzor::ui::widgets::composite::panel::state::PanelState;
        Self {
            current_theme: ThemeMode::Dark,
            tick_counter:  0,
            cell_counts:   [0; 4],
            cell_panel_states: [
                PanelState::default(),
                PanelState::default(),
                PanelState::default(),
                PanelState::default(),
            ],
            tree_debug_state: None,
            settings_panel_state: PanelState::default(),
        }
    }
}

impl App<PaintPanel> for DashboardApp {
    fn init(&mut self, _key: &WindowKey, layout: &mut uzor::layout::LayoutManager<PaintPanel>) {
        use uzor::docking::panels::SplitKind;

        // Register the tree-debug blackbox with LM so the agent API
        // routes `/blackbox/tree-debug/...` reach it.
        self.tree_debug_state = Some(tree_debug::register(layout, "tree-debug"));
        // Build a 2×2 dock tree once.  `split_leaf` keeps the original
        // leaf in place (left / top) and adds the new panel as the right
        // / bottom neighbour — the original leaf id stays valid for
        // further splits.
        let tree = layout.panels_mut().tree_mut();
        let l0 = tree.add_leaf(PaintPanel {
            id: "paint:r0_dirty",
            title: "fps = 0 (dirty)",
            target_fps: 0,
        });
        // l0 (left column)  → r0_dirty,  l1 (right column) → r1_30fps
        let l1 = tree.split_leaf(l0, SplitKind::SplitRight, PaintPanel {
            id: "paint:r1_30fps",
            title: "fps = 30",
            target_fps: 30,
        });
        // bottom of left column → r2_120fps
        tree.split_leaf(l0, SplitKind::SplitBottom, PaintPanel {
            id: "paint:r2_120fps",
            title: "fps = 120",
            target_fps: 120,
        });
        // bottom of right column → r3_uncap
        if let Some(l1) = l1 {
            tree.split_leaf(l1, SplitKind::SplitBottom, PaintPanel {
                id: "paint:r3_uncap",
                title: "uncapped",
                target_fps: uzor::render::UNCAPPED_FPS,
            });
        }
    }

    fn ui(&mut self, win: &mut WindowCtx<'_, PaintPanel>) {
        // ── Edge slot: settings panel reserves 280 px on the left so the
        //    dock area shrinks to whatever is left for the painting grid.
        win.layout.edges_mut().clear();
        win.layout.edges_mut().add(EdgeSlot {
            id:        "settings".to_string(),
            side:      EdgeSide::Left,
            thickness: 280.0,
            visible:   true,
            order:     0,
            ..Default::default()
        });
        // Re-solve so the freshly-declared edge slot is reflected in
        // dock_area + edge rects (which the docking panels then layout
        // themselves into).
        let win_rect = win.layout.last_window().unwrap_or(uzor::types::Rect::new(0.0, 0.0, 0.0, 0.0));
        if win_rect.width > 0.0 && win_rect.height > 0.0 {
            win.layout.solve(win_rect);
        }

        // ── Apply theme if it changed ────────────────────────────────────────
        {
            let theme_dark_id  = unsafe_widget_id("settings:theme_dark");
            let theme_light_id = unsafe_widget_id("settings:theme_light");
            if win.layout.was_clicked(&theme_dark_id) && self.current_theme != ThemeMode::Dark {
                self.current_theme = ThemeMode::Dark;
                // Use the LM helper so the agent log records both the
                // preset apply and a complementary `app.theme.changed`
                // breadcrumb the agent can grep on.
                win.layout.apply_style_preset(&MirageDarkPreset, "mirage_dark");
                win.layout.agent_log_push(
                    "app.theme.changed",
                    serde_json::json!({ "theme": "dark", "preset": "mirage_dark" }),
                );
            }
            if win.layout.was_clicked(&theme_light_id) && self.current_theme != ThemeMode::Light {
                self.current_theme = ThemeMode::Light;
                win.layout.apply_style_preset(&MirageLightPreset, "mirage_light");
                win.layout.agent_log_push(
                    "app.theme.changed",
                    serde_json::json!({ "theme": "light", "preset": "mirage_light" }),
                );
            }
        }

        // ── Snapshot render-control state (no borrows held) ───────────────────
        let active_backend = win.render_control.active_backend();
        let available      = win.render_control.available_backends();
        let vsync_on       = win.render_control.vsync();
        let msaa           = win.render_control.msaa_samples();
        let fps            = win.render_control.fps_limit();
        let measured_fps   = win.render_control.measured_fps();
        let frame_time_ms  = win.render_control.last_frame_time_ms();
        let frame_count    = win.render_control.frame_count();

        // ── Chrome strip ──────────────────────────────────────────────────────
        let dock = win.layout.last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
        {
            let layout = &mut *win.layout;
            let render = &mut *win.render;
            view! {
                <col rect={dock}>
                    <chrome show_new_window=true />
                </col>
            }
        }

        // ── Settings rect: edge slot the docking engine reserved earlier.
        //    Painting rect: the remaining dock_area (already shrunk by the
        //    edge slot + chrome compression in solve()).
        let settings_rect = win.layout.rect_for_edge_slot("settings")
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
        let paint_rect = win.layout.last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });

        // ── Process click events BEFORE painting (needs was_clicked) ──────────
        // Check backend button clicks.
        for (i, &backend) in available.iter().enumerate() {
            let id_str = format!("settings:backend_btn:{}", i);
            let id = unsafe_widget_id(id_str.as_str());
            if win.layout.was_clicked(&id) {
                win.render_control.set_backend(backend);
            }
        }
        {
            let id = unsafe_widget_id("settings:vsync_on");
            if win.layout.was_clicked(&id) { win.render_control.set_vsync(true); }
        }
        {
            let id = unsafe_widget_id("settings:vsync_off");
            if win.layout.was_clicked(&id) { win.render_control.set_vsync(false); }
        }
        for &(_, n) in [("Off", 0u8), ("4x", 4), ("8x", 8)].iter() {
            let id_str = format!("settings:msaa_{}", n);
            let id = unsafe_widget_id(id_str.as_str());
            if win.layout.was_clicked(&id) { win.render_control.set_msaa_samples(n); }
        }
        for &(_, limit) in [("30", 30u32), ("60", 60), ("120", 120), ("∞", 0)].iter() {
            let id_str = format!("settings:fps_{}", limit);
            let id = unsafe_widget_id(id_str.as_str());
            if win.layout.was_clicked(&id) { win.render_control.set_fps_limit(limit); }
        }

        // ── Settings panel (left 280px) ───────────────────────────────────────
        // Single `build_with_body` call: panel chrome + scrollbar guard +
        // body content all wired through one builder.  The renderer is
        // already clipped to the body rect and translated by the active
        // scroll offset by the time `body(...)` runs, so child widgets
        // draw in body-local coordinates without manual scroll math.
        {
            use uzor::ui::widgets::composite::panel::types::PanelRenderKind;
            use uzor::types::OverflowMode;
            let layout = &mut *win.layout;
            let render = &mut *win.render;
            let content_h    = 600.0_f64;
            let current_theme = self.current_theme;
            uzor::framework::widgets::lm::panel("settings", "settings:panel")
                .state(&mut self.settings_panel_state)
                .kind(PanelRenderKind::Plain)
                .overflow(OverflowMode::Scrollbar)
                .show_scrollbar(true)
                .content_height(content_h)
                .build_with_body(layout, render, |layout, render, body_rect| {
                    let row_h = 32.0_f64;
                    let gap   = 6.0_f64;
                    let pad   = 10.0_f64;
                    let mut cy = body_rect.y + pad;
                    let inner_w = body_rect.width - 2.0 * pad;

                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: row_h };
                    cy += row_h + gap;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:title"), r, "Settings")
                        .build(layout, render);

                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 1.0 };
                    cy += 1.0 + gap;
                    uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep0"), r).build(layout, render);

                    // Theme section
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 20.0 };
                    cy += 20.0 + 4.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:theme_lbl"), r, "Theme").build(layout, render);

                    let theme_btn_w = (inner_w - gap) / 2.0;
                    uzor::framework::widgets::lm::button(
                        unsafe_widget_id("settings:theme_dark"),
                        Rect { x: body_rect.x + pad, y: cy, width: theme_btn_w, height: row_h },
                    )
                    .text("Mirage Dark")
                    .active(current_theme == ThemeMode::Dark)
                    .build(layout, render);

                    uzor::framework::widgets::lm::button(
                        unsafe_widget_id("settings:theme_light"),
                        Rect { x: body_rect.x + pad + theme_btn_w + gap, y: cy, width: theme_btn_w, height: row_h },
                    )
                    .text("Mirage Light")
                    .active(current_theme == ThemeMode::Light)
                    .build(layout, render);
                    cy += row_h + gap;

                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 1.0 };
                    cy += 1.0 + gap;
                    uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep_theme"), r).build(layout, render);

                    // Backend section
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 20.0 };
                    cy += 20.0 + 4.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:backend_lbl"), r, "Backend").build(layout, render);

                    for (i, &backend) in available.iter().enumerate() {
                        let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: row_h };
                        cy += row_h + gap;
                        let is_active = backend == active_backend;
                        let label = backend.label();
                        let id_str = format!("settings:backend_btn:{}", i);
                        uzor::framework::widgets::lm::button(unsafe_widget_id(id_str.as_str()), r)
                            .text(label)
                            .active(is_active)
                            .build(layout, render);
                    }

                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 1.0 };
                    cy += 1.0 + gap;
                    uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep1"), r).build(layout, render);

                    // VSync
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 20.0 };
                    cy += 20.0 + 4.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:vsync_lbl"), r, "VSync").build(layout, render);

                    let vsync_btn_w = (inner_w - gap) / 2.0;
                    uzor::framework::widgets::lm::button(
                        unsafe_widget_id("settings:vsync_on"),
                        Rect { x: body_rect.x + pad, y: cy, width: vsync_btn_w, height: row_h },
                    )
                    .text("VSync ON")
                    .active(vsync_on)
                    .build(layout, render);

                    uzor::framework::widgets::lm::button(
                        unsafe_widget_id("settings:vsync_off"),
                        Rect { x: body_rect.x + pad + vsync_btn_w + gap, y: cy, width: vsync_btn_w, height: row_h },
                    )
                    .text("VSync OFF")
                    .active(!vsync_on)
                    .build(layout, render);
                    cy += row_h + gap;

                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 1.0 };
                    cy += 1.0 + gap;
                    uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep2"), r).build(layout, render);

                    // MSAA
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 20.0 };
                    cy += 20.0 + 4.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:msaa_lbl"), r, "MSAA").build(layout, render);

                    let msaa_options: &[(&str, u8)] = &[("Off", 0), ("4x", 4), ("8x", 8)];
                    let n_msaa = msaa_options.len() as f64;
                    let msaa_btn_w = (inner_w - gap * (n_msaa - 1.0)) / n_msaa;
                    for (_i, &(label, n)) in msaa_options.iter().enumerate() {
                        let r = Rect { x: body_rect.x + pad + (msaa_btn_w + gap) * (_i as f64), y: cy, width: msaa_btn_w, height: row_h };
                        let id_str = format!("settings:msaa_{}", n);
                        uzor::framework::widgets::lm::button(unsafe_widget_id(id_str.as_str()), r)
                            .text(label)
                            .active(msaa == n)
                            .build(layout, render);
                    }
                    cy += row_h + gap;

                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 1.0 };
                    cy += 1.0 + gap;
                    uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep3"), r).build(layout, render);

                    // Metrics
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 20.0 };
                    cy += 20.0 + 2.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:metrics_lbl"), r, "Metrics")
                        .build(layout, render);

                    let backend_line = format!("Backend: {}", active_backend.label());
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 18.0 };
                    cy += 18.0 + 2.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:metrics_backend"), r, backend_line.as_str())
                        .build(layout, render);

                    let fps_line = format!("FPS: {:.1}  ({:.2} ms)", measured_fps, frame_time_ms);
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 18.0 };
                    cy += 18.0 + 2.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:metrics_fps"), r, fps_line.as_str())
                        .build(layout, render);

                    let count_line = format!("Frames: {}", frame_count);
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 18.0 };
                    cy += 18.0 + gap;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:metrics_count"), r, count_line.as_str())
                        .build(layout, render);

                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 1.0 };
                    cy += 1.0 + gap;
                    uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep_metrics"), r).build(layout, render);

                    // FPS
                    let r = Rect { x: body_rect.x + pad, y: cy, width: inner_w, height: 20.0 };
                    cy += 20.0 + 4.0;
                    uzor::framework::widgets::lm::text(unsafe_widget_id("settings:fps_lbl"), r, "FPS Limit").build(layout, render);

                    let fps_options: &[(&str, u32)] = &[("30", 30), ("60", 60), ("120", 120), ("Unlim", 0)];
                    let n_fps = fps_options.len() as f64;
                    let fps_btn_w = (inner_w - gap * (n_fps - 1.0)) / n_fps;
                    for (i, &(label, limit)) in fps_options.iter().enumerate() {
                        let r = Rect { x: body_rect.x + pad + (fps_btn_w + gap) * i as f64, y: cy, width: fps_btn_w, height: row_h };
                        let id_str = format!("settings:fps_{}", limit);
                        uzor::framework::widgets::lm::button(unsafe_widget_id(id_str.as_str()), r)
                            .text(label)
                            .active(fps == limit)
                            .build(layout, render);
                    }
                    let _ = cy;
                });
            let _ = settings_rect;
        }

        // Edge handles are now owned by each `lm::panel(...)` cell —
        // hit zones registered in the composite, visual highlight on
        // hover / drag.  No more manual `register_dock_separators` here.
        let _ = paint_rect;
    }

    fn regions(&mut self) -> Vec<uzor::render::RenderRegion> {
        // Each region builds its own vello sub-scene at its own cadence.
        // The runtime composites them every frame on the GPU.
        let zero = Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
        vec![
            // Chrome strip + Settings panel: dirty-driven (rebuilds only on events).
            uzor::render::RenderRegion::dirty_driven("dashboard:chrome_settings", zero),
            // Painting cells, each at its own cadence.
            uzor::render::RenderRegion::dirty_driven("paint:r0_dirty",   zero),
            uzor::render::RenderRegion::capped     ("paint:r1_30fps",   zero, 30),
            uzor::render::RenderRegion::capped     ("paint:r2_120fps",  zero, 120),
            uzor::render::RenderRegion::uncapped   ("paint:r3_uncap",   zero),
        ]
    }

    fn draw_region(&mut self, region_id: &str, win: &mut WindowCtx<'_, PaintPanel>) {
        match region_id {
            "dashboard:chrome_settings" => self.draw_chrome_settings(win, Rect::new(0.0, 0.0, 0.0, 0.0), Rect::new(0.0, 0.0, 0.0, 0.0)),
            "paint:r0_dirty"  |
            "paint:r1_30fps"  |
            "paint:r2_120fps" |
            "paint:r3_uncap"  => {
                // Find the leaf whose panel.id matches this region and
                // grab its rect from the docking engine.  Falls back to
                // an empty rect when the leaf isn't in the tree (the
                // first paint frames of a freshly opened window).
                let rect_opt = win.layout.panels()
                    .panel_rects()
                    .iter()
                    .find_map(|(leaf_id, panel_rect)| {
                        let leaf = win.layout.panels().tree().leaf(*leaf_id)?;
                        let panel = leaf.panels.first()?;
                        if panel.id == region_id {
                            Some(Rect::new(
                                panel_rect.x      as f64,
                                panel_rect.y      as f64,
                                panel_rect.width  as f64,
                                panel_rect.height as f64,
                            ))
                        } else {
                            None
                        }
                    });
                if let Some(rect) = rect_opt {
                    if region_id == "paint:r0_dirty" {
                        if let Some(state_arc) = self.tree_debug_state.as_ref() {
                            if let Ok(mut state) = state_arc.lock() {
                                tree_debug::render_layout_tree(
                                    rect, &mut *state, &*win.layout, &mut *win.render,
                                );
                            }
                        }
                    } else {
                        let target_fps = match region_id {
                            "paint:r1_30fps"  => 30,
                            "paint:r2_120fps" => 120,
                            _                 => uzor::render::UNCAPPED_FPS,
                        };
                        self.draw_paint_cell(win, region_id, rect, target_fps);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_chrome_new_window(&mut self, _source: &WindowKey) -> Option<WindowSpec> {
        Some(
            WindowSpec::new(
                WindowKey::new(format!("extra-{}", random_suffix())),
                "uzor — extra",
            )
            .size(560, 420)
            .min_size(420, 320)
            .decorations(false)
            .background(0xFF_0d_11_17),
        )
    }
}

impl DashboardApp {
    /// Build chrome strip + Settings panel. The full ui() body already does
    /// this (plus paints the painting bg) — delegate to it.  When the
    /// per-region scheduler decides this region is due, the runtime calls
    /// us with the chrome+settings region's dedicated scene.
    fn draw_chrome_settings(
        &mut self,
        win: &mut WindowCtx<'_, PaintPanel>,
        _dock: Rect,
        _settings_rect: Rect,
    ) {
        self.ui(win);
    }

    /// Build one painting-grid cell as a real composite Panel.
    /// Increments a per-cell counter every rebuild so the user can see
    /// different cadences side-by-side.
    fn draw_paint_cell(
        &mut self,
        win: &mut WindowCtx<'_, PaintPanel>,
        cell_id: &str,
        rect: Rect,
        target_fps: u32,
    ) {
        use uzor::ui::widgets::composite::panel::types::PanelRenderKind;

        let idx = match cell_id {
            "paint:r0_dirty"  => 0,
            "paint:r1_30fps"  => 1,
            "paint:r2_120fps" => 2,
            "paint:r3_uncap"  => 3,
            _ => return,
        };
        self.cell_counts[idx] = self.cell_counts[idx].wrapping_add(1);
        let count = self.cell_counts[idx];
        let measured_fps = win.render_control.measured_fps();

        let header = match target_fps {
            0                          => format!("Region target_fps = 0 (dirty-driven)"),
            uzor::render::UNCAPPED_FPS => format!("Region uncapped"),
            f                          => format!("Region target_fps = {}", f),
        };
        let panel_id_str = format!("paint_panel:{}", cell_id);

        // L3-only: lm::panel composite owns the chrome (background +
        // header) and clips the body callback to the panel's body
        // rect.  Counter / fps text widgets register inside the
        // body closure so they live on the same LayoutManager as
        // the panel itself.
        {
            let layout = &mut *win.layout;
            let render = &mut *win.render;
            let pad   = 12.0_f64;
            let row_h = 22.0_f64;
            let cell_id_owned = cell_id.to_string();
            uzor::framework::widgets::lm::panel(
                cell_id,
                panel_id_str.as_str(),
            )
            .state(&mut self.cell_panel_states[idx])
            .header_title(header.as_str())
            .kind(PanelRenderKind::WithHeader)
            .overflow(uzor::types::OverflowMode::Clip)
            // Drag any edge of the cell to resize the dock split.  The LM
            // resolves (host_id, edge) onto the shared dock separator, so
            // the neighbouring cell moves with it.
            .edge_handles(uzor::ui::widgets::composite::panel::style::EdgeHandlesConfig::all())
            .build_with_body(layout, render, |layout, render, body_rect| {
                let lr = |cy: f64, h: f64| Rect {
                    x: body_rect.x + pad, y: cy,
                    width: body_rect.width - 2.0 * pad, height: h,
                };
                let counter_line = format!("rebuilds: {}", count);
                uzor::framework::widgets::lm::text(
                    unsafe_widget_id(format!("{}:counter", cell_id_owned).as_str()),
                    lr(body_rect.y + pad, row_h), counter_line.as_str(),
                ).build(layout, render);

                let measured_line = format!("window FPS: {:.1}", measured_fps);
                uzor::framework::widgets::lm::text(
                    unsafe_widget_id(format!("{}:meas", cell_id_owned).as_str()),
                    lr(body_rect.y + pad + row_h + 4.0, row_h), measured_line.as_str(),
                ).build(layout, render);
            });
        }
    }
}

fn random_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_micros() as u64).unwrap_or(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DashboardApp::new())
        .agent_api(17480)
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor — L4 Dashboard")
                .size(1400, 900)
                .min_size(900, 600)
                .decorations(false)
                .background(0xFF_0d_11_17)
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
