//! # Level 4 — Dashboard (Settings + Painting panels)
//!
//! Main window split into two side-by-side panels:
//!   LEFT  (280 px fixed) — Settings: backend selector, vsync, msaa, fps.
//!   RIGHT (flex=1)       — Stub painting panel with static text.
//!
//! Chrome strip at top with "+" spawns extra windows.
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
use uzor::layout::LayoutNodeId;
use uzor::platform::types::CornerStyle;
use uzor::types::unsafe_widget_id;
use uzor::ui::widgets::atomic::button::input::register_layout_manager_button;
use uzor::ui::widgets::atomic::button::render::ButtonView;
use uzor::ui::widgets::atomic::button::settings::ButtonSettings;
use uzor_desktop::AppRun as _;
use uzor_framework_macros::view;

struct DashboardApp;

impl DashboardApp {
    fn new() -> Self { Self }
}

impl App<NoPanel> for DashboardApp {
    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        // ── Snapshot render-control state (no borrows held) ───────────────────
        let active_backend = win.render_control.active_backend();
        let available      = win.render_control.available_backends();
        let vsync_on       = win.render_control.vsync();
        let msaa           = win.render_control.msaa_samples();
        let fps            = win.render_control.fps_limit();

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

        // ── Compute below-chrome body rect ────────────────────────────────────
        let body = win.layout.last_solved()
            .map(|s| s.dock_area)
            .unwrap_or(Rect { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
        let chrome_h = win.layout.rect_for_chrome()
            .map(|r| r.height)
            .unwrap_or(32.0);
        let body = Rect {
            x:      body.x,
            y:      body.y + chrome_h,
            width:  body.width,
            height: (body.height - chrome_h).max(0.0),
        };

        let settings_w    = 280.0_f64;
        let settings_rect = Rect { x: body.x, y: body.y, width: settings_w, height: body.height };
        let paint_rect    = Rect {
            x:      body.x + settings_w,
            y:      body.y,
            width:  (body.width - settings_w).max(0.0),
            height: body.height,
        };

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
        {
            let layout = &mut *win.layout;
            let render = &mut *win.render;

            let row_h = 32.0_f64;
            let gap   = 6.0_f64;
            let pad   = 10.0_f64;
            let mut cy = settings_rect.y + pad;

            // Title
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: row_h };
            cy += row_h + gap;
            uzor::framework::widgets::lm::text(unsafe_widget_id("settings:title"), r, "Settings")
                .build(layout, render);

            // Sep
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 1.0 };
            cy += 1.0 + gap;
            uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep0"), r).build(layout, render);

            // Backend section label
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 20.0 };
            cy += 20.0 + 4.0;
            uzor::framework::widgets::lm::text(unsafe_widget_id("settings:backend_lbl"), r, "Backend").build(layout, render);

            // Backend buttons
            for (i, &backend) in available.iter().enumerate() {
                let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: row_h };
                cy += row_h + gap;
                let is_active = backend == active_backend;
                let label = backend.label();
                let id_str = format!("settings:backend_btn:{}", i);
                let id = unsafe_widget_id(id_str.as_str());
                let ws = layout.ctx().input.widget_state(&id);
                let view = ButtonView { text: Some(label), icon: None, active: is_active, disabled: false, active_border: None, hover_chevron: None };
                register_layout_manager_button(layout, render, LayoutNodeId::ROOT, id, r, ws, &view, &ButtonSettings::default());
            }

            // Sep
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 1.0 };
            cy += 1.0 + gap;
            uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep1"), r).build(layout, render);

            // VSync
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 20.0 };
            cy += 20.0 + 4.0;
            uzor::framework::widgets::lm::text(unsafe_widget_id("settings:vsync_lbl"), r, "VSync").build(layout, render);

            let vsync_btn_w = (settings_rect.width - 2.0*pad - gap) / 2.0;
            let id = unsafe_widget_id("settings:vsync_on");
            let r = Rect { x: settings_rect.x + pad, y: cy, width: vsync_btn_w, height: row_h };
            let ws = layout.ctx().input.widget_state(&id);
            let view = ButtonView { text: Some("VSync ON"), icon: None, active: vsync_on, disabled: false, active_border: None, hover_chevron: None };
            register_layout_manager_button(layout, render, LayoutNodeId::ROOT, id, r, ws, &view, &ButtonSettings::default());

            let id = unsafe_widget_id("settings:vsync_off");
            let r = Rect { x: settings_rect.x + pad + vsync_btn_w + gap, y: cy, width: vsync_btn_w, height: row_h };
            let ws = layout.ctx().input.widget_state(&id);
            let view = ButtonView { text: Some("VSync OFF"), icon: None, active: !vsync_on, disabled: false, active_border: None, hover_chevron: None };
            register_layout_manager_button(layout, render, LayoutNodeId::ROOT, id, r, ws, &view, &ButtonSettings::default());
            cy += row_h + gap;

            // Sep
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 1.0 };
            cy += 1.0 + gap;
            uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep2"), r).build(layout, render);

            // MSAA
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 20.0 };
            cy += 20.0 + 4.0;
            uzor::framework::widgets::lm::text(unsafe_widget_id("settings:msaa_lbl"), r, "MSAA").build(layout, render);

            let msaa_options: &[(&str, u8)] = &[("Off", 0), ("4x", 4), ("8x", 8)];
            let n_msaa = msaa_options.len() as f64;
            let msaa_btn_w = (settings_rect.width - 2.0*pad - gap * (n_msaa - 1.0)) / n_msaa;
            for (i, &(label, n)) in msaa_options.iter().enumerate() {
                let r = Rect { x: settings_rect.x + pad + (msaa_btn_w + gap) * i as f64, y: cy, width: msaa_btn_w, height: row_h };
                let id_str = format!("settings:msaa_{}", n);
                let id = unsafe_widget_id(id_str.as_str());
                let ws = layout.ctx().input.widget_state(&id);
                let view = ButtonView { text: Some(label), icon: None, active: msaa == n, disabled: false, active_border: None, hover_chevron: None };
                register_layout_manager_button(layout, render, LayoutNodeId::ROOT, id, r, ws, &view, &ButtonSettings::default());
            }
            cy += row_h + gap;

            // Sep
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 1.0 };
            cy += 1.0 + gap;
            uzor::framework::widgets::lm::separator(unsafe_widget_id("settings:sep3"), r).build(layout, render);

            // FPS
            let r = Rect { x: settings_rect.x + pad, y: cy, width: settings_rect.width - 2.0*pad, height: 20.0 };
            cy += 20.0 + 4.0;
            uzor::framework::widgets::lm::text(unsafe_widget_id("settings:fps_lbl"), r, "FPS Limit").build(layout, render);

            let fps_options: &[(&str, u32)] = &[("30", 30), ("60", 60), ("120", 120), ("Unlim", 0)];
            let n_fps = fps_options.len() as f64;
            let fps_btn_w = (settings_rect.width - 2.0*pad - gap * (n_fps - 1.0)) / n_fps;
            for (i, &(label, limit)) in fps_options.iter().enumerate() {
                let r = Rect { x: settings_rect.x + pad + (fps_btn_w + gap) * i as f64, y: cy, width: fps_btn_w, height: row_h };
                let id_str = format!("settings:fps_{}", limit);
                let id = unsafe_widget_id(id_str.as_str());
                let ws = layout.ctx().input.widget_state(&id);
                let view = ButtonView { text: Some(label), icon: None, active: fps == limit, disabled: false, active_border: None, hover_chevron: None };
                register_layout_manager_button(layout, render, LayoutNodeId::ROOT, id, r, ws, &view, &ButtonSettings::default());
            }
            let _ = cy;
        }

        // ── Stub painting panel (right, flex=1) ───────────────────────────────
        {
            let layout = &mut *win.layout;
            let render = &mut *win.render;

            let pad   = 16.0_f64;
            let row_h = 24.0_f64;
            let gap   = 8.0_f64;
            let mut cy = paint_rect.y + pad;
            let w = (paint_rect.width - 2.0*pad).max(0.0);

            let lr = |cy: f64| Rect { x: paint_rect.x + pad, y: cy, width: w, height: row_h };

            uzor::framework::widgets::lm::text(unsafe_widget_id("paint:title"), lr(cy), "Painting panel").build(layout, render);
            cy += row_h + gap;

            let r = Rect { x: paint_rect.x + pad, y: cy, width: w, height: 1.0 };
            cy += 1.0 + gap;
            uzor::framework::widgets::lm::separator(unsafe_widget_id("paint:sep0"), r).build(layout, render);

            uzor::framework::widgets::lm::text(unsafe_widget_id("paint:line1"), lr(cy), "Chart / canvas area").build(layout, render);
            cy += row_h + gap;
            uzor::framework::widgets::lm::text(unsafe_widget_id("paint:line2"), lr(cy), "(stub — content goes here)").build(layout, render);
            cy += row_h + gap;

            let r = Rect { x: paint_rect.x + pad, y: cy, width: w, height: 1.0 };
            cy += 1.0 + gap;
            uzor::framework::widgets::lm::separator(unsafe_widget_id("paint:sep1"), r).build(layout, render);

            let btn_r = Rect { x: paint_rect.x + pad, y: cy, width: 160.0, height: 32.0 };
            uzor::framework::widgets::lm::button(unsafe_widget_id("paint:open_settings"), btn_r)
                .text("Open settings…")
                .build(layout, render);

            let _ = cy;
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
            .background(0xFF_F7_F7_F4),
        )
    }
}

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
