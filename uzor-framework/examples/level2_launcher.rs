//! # Level 2 — uzor multi-widget demo
//!
//! Demonstrates 11 different atomic widgets rendered through the L2 API
//! (`register_context_manager_<X>`).  Every widget is visible in its normal
//! (non-hover) state — achieved by a custom `VisibleButtonTheme` for the
//! button and by the default themes of the other widgets which all draw solid
//! backgrounds.
//!
//! Layout is hardcoded (no flex/yoga).  Window is 680 × 440.
//!
//! # Run
//!
//! ```sh
//! cargo run --example level2_launcher -p uzor-framework
//! ```

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton as WinitMouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

// ── vello ──────────────────────────────────────────────────────────────────────
use vello::util::{RenderContext as VelloRenderCx, RenderSurface};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use vello::peniko::{Color, Fill};
use vello::kurbo::{Affine, RoundedRect};
use vello::wgpu;

// ── uzor L2 ───────────────────────────────────────────────────────────────────
use uzor::app_context::{ContextManager, layout::types::LayoutNode};
use uzor::input::core::coordinator::LayerId;
use uzor::input::pointer::state::{InputState, MouseButton};
use uzor::layout::LayoutManager;
use uzor::types::Rect;

// ── widgets ──────────────────────────────────────────────────────────────────
use uzor::ui::widgets::atomic::button::input::register_context_manager_button;
use uzor::ui::widgets::atomic::button::{ButtonSettings, ButtonTheme, ButtonView};

use uzor::ui::widgets::atomic::checkbox::input::register_context_manager_checkbox;
use uzor::ui::widgets::atomic::checkbox::settings::CheckboxSettings;
use uzor::ui::widgets::atomic::checkbox::types::{CheckboxRenderKind, CheckboxView};

use uzor::ui::widgets::atomic::toggle::input::register_context_manager_toggle;
use uzor::ui::widgets::atomic::toggle::settings::ToggleSettings;
use uzor::ui::widgets::atomic::toggle::types::{ToggleRenderKind, ToggleView};

use uzor::ui::widgets::atomic::radio::input::register_context_manager_radio;
use uzor::ui::widgets::atomic::radio::settings::RadioSettings;
use uzor::ui::widgets::atomic::radio::types::{
    DotShape, RadioDotView, RadioRenderKind,
};

use uzor::ui::widgets::atomic::slider::input::register_context_manager_slider;
use uzor::ui::widgets::atomic::slider::render::SliderView;
use uzor::ui::widgets::atomic::slider::settings::SliderSettings;
use uzor::ui::widgets::atomic::slider::types::SliderType;

use uzor::ui::widgets::atomic::scrollbar::input::register_context_manager_scrollbar;
use uzor::ui::widgets::atomic::scrollbar::settings::ScrollbarSettings;

use uzor::ui::widgets::atomic::separator::input::{
    register_context_manager_separator, SeparatorKind,
};
use uzor::ui::widgets::atomic::separator::render::SeparatorView;
use uzor::ui::widgets::atomic::separator::settings::SeparatorSettings;
use uzor::ui::widgets::atomic::separator::types::{SeparatorOrientation, SeparatorType};

use uzor::ui::widgets::atomic::text_input::input::register_context_manager_text_input;
use uzor::ui::widgets::atomic::text_input::render::InputView;
use uzor::ui::widgets::atomic::text_input::settings::TextInputSettings;
use uzor::ui::widgets::atomic::text_input::types::InputType;

use uzor::ui::widgets::atomic::color_swatch::input::register_context_manager_color_swatch;
use uzor::ui::widgets::atomic::color_swatch::settings::ColorSwatchSettings;
use uzor::ui::widgets::atomic::color_swatch::types::{ColorSwatchRenderKind, ColorSwatchView};

use uzor::ui::widgets::atomic::shape_selector::input::register_context_manager_shape_selector;
use uzor::ui::widgets::atomic::shape_selector::settings::ShapeSelectorSettings;
use uzor::ui::widgets::atomic::shape_selector::types::ShapeSelectorRenderKind;

use uzor::ui::widgets::atomic::close_button::input::register_context_manager_close_button;
use uzor::ui::widgets::atomic::close_button::render::CloseButtonView;
use uzor::ui::widgets::atomic::close_button::settings::CloseButtonSettings;
use uzor::ui::widgets::atomic::close_button::types::CloseButtonRenderKind;

use uzor::ui::widgets::atomic::drag_handle::input::register_context_manager_drag_handle;
use uzor::ui::widgets::atomic::drag_handle::settings::DragHandleSettings;
use uzor::ui::widgets::atomic::drag_handle::types::{DragHandleRenderKind, DragHandleView};

// ── GPU render context ────────────────────────────────────────────────────────
use uzor_render_vello_gpu::VelloGpuRenderContext;

// ── framework helper ──────────────────────────────────────────────────────────
use uzor_framework::app::NoPanel;

// ─────────────────────────────────────────────────────────────────────────────
// Geometry + colours
// ─────────────────────────────────────────────────────────────────────────────

const WIN_W: u32 = 680;
const WIN_H: u32 = 440;

const BG: Color = Color::from_rgb8(0x16, 0x16, 0x1e);
const PANEL_BG: Color = Color::from_rgb8(0x1e, 0x22, 0x2d);
const LABEL_BG: Color = Color::from_rgba8(255, 255, 255, 18);

// ─────────────────────────────────────────────────────────────────────────────
// Custom visible button theme (default has bg_normal=transparent)
// ─────────────────────────────────────────────────────────────────────────────

struct VisibleButtonTheme;

impl ButtonTheme for VisibleButtonTheme {
    fn button_bg_normal(&self) -> &str { "#2962ff" }
    fn button_bg_hover(&self) -> &str { "#4080ff" }
    fn button_bg_pressed(&self) -> &str { "#1a40c8" }
    fn button_bg_active(&self) -> &str { "#10b981" }
    fn button_bg_disabled(&self) -> &str { "#2a2a2a" }
    fn button_text_normal(&self) -> &str { "#ffffff" }
    fn button_text_hover(&self) -> &str { "#ffffff" }
    fn button_text_active(&self) -> &str { "#ffffff" }
    fn button_text_disabled(&self) -> &str { "#4a4a4a" }
    fn button_icon_normal(&self) -> &str { "#ffffff" }
    fn button_icon_hover(&self) -> &str { "#ffffff" }
    fn button_icon_active(&self) -> &str { "#ffffff" }
    fn button_icon_disabled(&self) -> &str { "#4a4a4a" }
    fn button_border_normal(&self) -> &str { "#3a5aff" }
    fn button_border_hover(&self) -> &str { "#6080ff" }
    fn button_border_focused(&self) -> &str { "#2962ff" }
    fn button_accent(&self) -> &str { "#2962ff" }
    fn button_danger(&self) -> &str { "#ef5350" }
    fn button_success(&self) -> &str { "#10b981" }
    fn button_warning(&self) -> &str { "#f59e0b" }
    fn toolbar_item_bg_hover(&self) -> &str { "#2a2e39" }
    fn toolbar_item_bg_active(&self) -> &str { "#2196F3" }
    fn toolbar_item_text(&self) -> &str { "#d1d4dc" }
    fn toolbar_item_text_hover(&self) -> &str { "#ffffff" }
    fn toolbar_item_text_active(&self) -> &str { "#ffffff" }
    fn toolbar_separator(&self) -> &str { "#2a2e39" }
    fn toolbar_background(&self) -> &str { "#1e222d" }
    fn toolbar_accent(&self) -> &str { "#2962ff" }
    fn button_primary_bg(&self) -> &str { "#2962ff" }
    fn button_primary_bg_hover(&self) -> &str { "#4080ff" }
    fn button_danger_bg(&self) -> &str { "rgba(255,80,80,0.15)" }
    fn button_danger_bg_hover(&self) -> &str { "rgba(255,80,80,0.35)" }
    fn button_danger_border(&self) -> &str { "rgba(239,83,80,0.5)" }
    fn button_danger_border_hover(&self) -> &str { "rgba(239,83,80,0.75)" }
    fn button_danger_text(&self) -> &str { "#ef5350" }
    fn button_secondary_hover_bg(&self) -> &str { "rgba(255,255,255,0.12)" }
    fn button_secondary_text_muted(&self) -> &str { "rgba(254,255,238,0.7)" }
    fn button_secondary_text(&self) -> &str { "rgba(254,255,238,0.95)" }
    fn button_ghost_idle_bg(&self) -> &str { "#1e222d" }
    fn button_utility_bg(&self) -> &str { "#2a2e39" }
    fn button_utility_bg_hover(&self) -> &str { "#363a45" }
}

// ─────────────────────────────────────────────────────────────────────────────
// App state
// ─────────────────────────────────────────────────────────────────────────────

struct AppState {
    window:    Arc<Window>,
    render_cx: VelloRenderCx,
    surface:   RenderSurface<'static>,
    renderer:  Renderer,
    scene:     Scene,

    ctx:    ContextManager,
    layout: LayoutManager<NoPanel>,

    mouse_pos:   Option<(f64, f64)>,
    clicked:     Option<MouseButton>,
    button_down: bool,

    // per-widget state mirrored here for demo logic
    connected:    bool,
    checked:      bool,
    toggled:      bool,
    radio_sel:    usize,
    slider_val:   f64,
    scroll_off:   f64,
    swatch_sel:   usize,
    shape_sel:    usize,
}

impl AppState {
    fn render(&mut self) {
        let (width, height) = {
            let s = &self.surface;
            (s.config.width, s.config.height)
        };
        let viewport = Rect::new(0.0, 0.0, width as f64, height as f64);

        self.layout.chrome_mut().visible = false;
        let _solved = self.layout.solve(viewport);

        let input = {
            let mut s = InputState::default();
            s.pointer.pos     = self.mouse_pos;
            s.pointer.clicked = self.clicked.take();
            if self.button_down {
                s.pointer.button_down = Some(MouseButton::Left);
            }
            s
        };
        self.ctx.begin_frame(input, viewport);

        // ── scene ─────────────────────────────────────────────────────────────
        self.scene.reset();

        // background
        self.scene.fill(
            Fill::NonZero, Affine::IDENTITY, BG, None,
            &vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        // column panels (visual grouping)
        for (px, pw) in [(12.0_f64, 310.0_f64), (336.0_f64, 330.0_f64)] {
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY, PANEL_BG, None,
                &RoundedRect::new(px, 12.0, px + pw, height as f64 - 12.0, 8.0),
            );
        }

        // ── widgets ───────────────────────────────────────────────────────────
        {
            let mut render = VelloGpuRenderContext::new(&mut self.scene, 0.0, 0.0);
            let layer = LayerId::main();

            // ── 1. Button ─────────────────────────────────────────────────────
            let btn_rect = Rect::new(28.0, 28.0, 130.0, 36.0);
            let btn_view = ButtonView {
                text:          Some(if self.connected { "Disconnect" } else { "Connect" }),
                icon:          None,
                active:        self.connected,
                disabled:      false,
                active_border: None,
            };
            register_context_manager_button(
                &mut self.ctx, &mut render,
                "btn-connect", btn_rect, &layer,
                &btn_view,
                &ButtonSettings::default().with_theme(Box::new(VisibleButtonTheme)),
            );

            // ── 2. Close button ───────────────────────────────────────────────
            let close_rect = Rect::new(278.0, 28.0, 24.0, 24.0);
            register_context_manager_close_button(
                &mut self.ctx, &mut render,
                "btn-close", close_rect, &layer,
                &CloseButtonView { hovered: false },
                &CloseButtonSettings::default(),
                &CloseButtonRenderKind::Default,
            );

            // ── 3. Checkbox ───────────────────────────────────────────────────
            let cb_rect = Rect::new(28.0, 88.0, 160.0, 22.0);
            register_context_manager_checkbox(
                &mut self.ctx, &mut render,
                "cb-setting-a", cb_rect, &layer,
                &CheckboxView { checked: self.checked, label: Some("Setting A") },
                &CheckboxSettings::default(),
                &CheckboxRenderKind::Standard,
                "13px sans-serif",
            );

            // ── 4. Toggle ─────────────────────────────────────────────────────
            let tog_rect = Rect::new(28.0, 130.0, 80.0, 24.0);
            register_context_manager_toggle(
                &mut self.ctx, &mut render,
                "tog-enable", tog_rect, &layer,
                &ToggleView { toggled: self.toggled, label: Some("ON"), disabled: false },
                &ToggleSettings::default(),
                &ToggleRenderKind::Switch,
            );

            // ── 5. Radio group (3 dots inline) ────────────────────────────────
            // Three separate Dot registrations sharing a logical group
            for (i, cx) in [28.0_f64, 68.0, 108.0].iter().enumerate() {
                let dot_rect = Rect::new(*cx, 175.0, 28.0, 28.0);
                register_context_manager_radio(
                    &mut self.ctx, &mut render,
                    format!("radio-opt-{i}"), dot_rect, &layer,
                    &RadioSettings::default(),
                    &RadioRenderKind::Dot {
                        shape: DotShape::Circle,
                        cx: cx + 14.0,
                        cy: 175.0 + 14.0,
                        view: RadioDotView { selected: self.radio_sel == i },
                    },
                );
            }

            // ── 6. Slider (horizontal) ────────────────────────────────────────
            let slid_rect = Rect::new(28.0, 228.0, 260.0, 24.0);
            register_context_manager_slider(
                &mut self.ctx, &mut render,
                "slider-main", slid_rect, &layer,
                &SliderView {
                    kind: SliderType::Single {
                        value: self.slider_val,
                        min: 0.0, max: 100.0, step: 1.0,
                    },
                    hovered: false,
                    disabled: false,
                    dragging_handle: None,
                },
                &SliderSettings::default(),
            );

            // ── 7. Separator (horizontal divider) ─────────────────────────────
            let sep_rect = Rect::new(28.0, 272.0, 260.0, 2.0);
            register_context_manager_separator(
                &mut self.ctx, &mut render,
                "sep-h", sep_rect, SeparatorKind::Divider, &layer,
                &SeparatorView {
                    kind: SeparatorType::Divider { orientation: SeparatorOrientation::Horizontal },
                    hovered: false,
                    dragging: false,
                },
                &SeparatorSettings::default(),
            );

            // ── 8. Text input ─────────────────────────────────────────────────
            let ti_rect = Rect::new(28.0, 292.0, 200.0, 28.0);
            register_context_manager_text_input(
                &mut self.ctx, &mut render,
                "text-search", ti_rect, &layer,
                &InputView {
                    text: "",
                    placeholder: "Search...",
                    cursor: 0,
                    selection: None,
                    focused: false,
                    disabled: false,
                    input_type: InputType::Search,
                },
                &TextInputSettings::with_config(
                    uzor::ui::widgets::atomic::text_input::state::TextFieldConfig::text(),
                ),
            );

            // ── 9. Color swatches (4 squares) ─────────────────────────────────
            let swatch_colors: [[u8; 4]; 4] = [
                [41, 98, 255, 255],
                [16, 185, 129, 255],
                [245, 158, 11, 255],
                [239, 83, 80, 255],
            ];
            for (i, color) in swatch_colors.iter().enumerate() {
                let sx = 28.0 + i as f64 * 34.0;
                let sw_rect = Rect::new(sx, 344.0, 26.0, 26.0);
                register_context_manager_color_swatch(
                    &mut self.ctx, &mut render,
                    format!("swatch-{i}"), sw_rect, &layer,
                    &ColorSwatchView {
                        color: *color,
                        hovered: false,
                        selected: self.swatch_sel == i,
                        show_transparency: false,
                        border_color_override: None,
                    },
                    &ColorSwatchSettings::default(),
                    &ColorSwatchRenderKind::Simple,
                );
            }

            // ── 10. Shape selector (3 buttons) ────────────────────────────────
            let shape_labels = ["Rect", "Circle", "Line"];
            for (i, _lbl) in shape_labels.iter().enumerate() {
                let shx = 28.0 + i as f64 * 80.0;
                let sh_rect = Rect::new(shx, 392.0, 72.0, 28.0);
                register_context_manager_shape_selector(
                    &mut self.ctx, &mut render,
                    format!("shape-{i}"), sh_rect, &layer,
                    &ShapeSelectorSettings::default(),
                    &ShapeSelectorRenderKind::UIStyle,
                );
            }

            // ── 11. Scrollbar (vertical, right column) ────────────────────────
            let sb_track = Rect::new(624.0, 28.0, 10.0, 380.0);
            let content_h = 1000.0_f64;
            let viewport_h = 380.0_f64;
            let thumb_ratio = (viewport_h / content_h).clamp(0.0, 1.0);
            let thumb_h = (thumb_ratio * sb_track.height).max(30.0);
            let scroll_range = sb_track.height - thumb_h;
            let thumb_y = sb_track.y
                + (self.scroll_off / (content_h - viewport_h).max(1.0)) * scroll_range;
            let sb_thumb = Rect::new(624.0, thumb_y, 10.0, thumb_h);

            register_context_manager_scrollbar(
                &mut self.ctx, &mut render,
                "sb-track", "sb-thumb",
                sb_track, sb_thumb,
                5.0, &layer,
                content_h, viewport_h, self.scroll_off,
                &ScrollbarSettings::default(),
            );

            // ── 12. Drag handle ───────────────────────────────────────────────
            let dh_rect = Rect::new(350.0, 28.0, 60.0, 24.0);
            register_context_manager_drag_handle(
                &mut self.ctx, &mut render,
                "drag-handle", dh_rect, &layer,
                &DragHandleView { rect: dh_rect },
                &DragHandleSettings::default(),
                &DragHandleRenderKind::GripDots,
            );
        }

        // ── shape label overlays (after render borrow released) ───────────────
        let shape_labels = ["Rect", "Circle", "Line"];
        for (i, lbl) in shape_labels.iter().enumerate() {
            let shx = 28.0 + i as f64 * 80.0;
            let lbl_w = lbl.len() as f64 * 6.0;
            let lbl_rect = vello::kurbo::Rect::new(
                shx + (72.0 - lbl_w) / 2.0,
                392.0 + 8.0,
                shx + (72.0 + lbl_w) / 2.0,
                392.0 + 20.0,
            );
            let lbl_alpha = if self.shape_sel == i { 180_u8 } else { 80_u8 };
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY,
                Color::from_rgba8(255, 255, 255, lbl_alpha),
                None, &lbl_rect,
            );
        }

        // ── status strip ──────────────────────────────────────────────────────
        let strip_color = if self.connected {
            Color::from_rgb8(0x18, 0x50, 0x28)
        } else {
            Color::from_rgb8(0x60, 0x20, 0x20)
        };
        self.scene.fill(
            Fill::NonZero, Affine::IDENTITY, strip_color, None,
            &vello::kurbo::Rect::new(0.0, height as f64 - 0.0, width as f64, height as f64),
        );

        // widget-count badge
        for (i, label) in ["12 widgets", "L2 demo"].iter().enumerate() {
            let lw = label.len() as f64 * 6.5;
            let bx = 340.0 + i as f64 * 100.0;
            let by = height as f64 - 32.0;
            self.scene.fill(
                Fill::NonZero, Affine::IDENTITY, LABEL_BG, None,
                &RoundedRect::new(bx, by, bx + lw + 12.0, by + 20.0, 4.0),
            );
        }

        // ── end_frame / responses ─────────────────────────────────────────────
        let responses = self.ctx.end_frame();
        for (id, resp) in &responses {
            if !resp.clicked { continue; }
            let id_str = id.0.as_str();

            if id_str == "btn-connect" {
                self.connected = !self.connected;
                println!("[L2] connected → {}", self.connected);
            } else if id_str == "btn-close" {
                println!("[L2] close clicked");
            } else if id_str == "cb-setting-a" {
                self.checked = !self.checked;
                println!("[L2] checked → {}", self.checked);
            } else if id_str == "tog-enable" {
                self.toggled = !self.toggled;
                println!("[L2] toggled → {}", self.toggled);
            } else if let Some(i) = id_str.strip_prefix("radio-opt-") {
                if let Ok(n) = i.parse::<usize>() {
                    self.radio_sel = n;
                    println!("[L2] radio → {n}");
                }
            } else if let Some(i) = id_str.strip_prefix("swatch-") {
                if let Ok(n) = i.parse::<usize>() {
                    self.swatch_sel = n;
                    println!("[L2] swatch → {n}");
                }
            } else if let Some(i) = id_str.strip_prefix("shape-") {
                if let Ok(n) = i.parse::<usize>() {
                    self.shape_sel = n;
                    println!("[L2] shape → {n}");
                }
            }
        }

        // ── GPU submit ─────────────────────────────────────────────────────────
        let dev = &self.render_cx.devices[self.surface.dev_id];
        let render_params = RenderParams {
            base_color: BG,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        self.renderer
            .render_to_texture(
                &dev.device, &dev.queue,
                &self.scene, &self.surface.target_view,
                &render_params,
            )
            .unwrap_or_default();

        let surface_texture = match self.surface.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = dev.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("level2-blit"),
        });
        self.surface.blitter.copy(
            &dev.device, &mut encoder,
            &self.surface.target_view, &surface_view,
        );
        dev.queue.submit([encoder.finish()]);
        surface_texture.present();
        self.window.request_redraw();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// winit handler
// ─────────────────────────────────────────────────────────────────────────────

struct Handler {
    state: Option<AppState>,
}

impl ApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("uzor L2 — multi-widget demo")
            .with_inner_size(winit::dpi::LogicalSize::new(WIN_W, WIN_H))
            .with_resizable(false);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("OS should be able to create a desktop window"),
        );

        let mut render_cx = VelloRenderCx::new();
        let size = window.inner_size();

        let surface: RenderSurface<'static> = pollster::block_on(async {
            render_cx
                .create_surface(
                    Arc::clone(&window),
                    size.width.max(1),
                    size.height.max(1),
                    wgpu::PresentMode::AutoVsync,
                )
                .await
                .expect("vello should be able to create a GPU surface on desktop")
        });

        let renderer = Renderer::new(
            &render_cx.devices[surface.dev_id].device,
            RendererOptions {
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
                ..RendererOptions::default()
            },
        )
        .expect("vello Renderer creation should succeed");

        window.request_redraw();

        let ctx = ContextManager::new(LayoutNode::new("l2-root"));

        self.state = Some(AppState {
            window,
            render_cx,
            surface,
            renderer,
            scene: Scene::new(),
            ctx,
            layout: LayoutManager::new(),
            mouse_pos: None,
            clicked: None,
            button_down: false,
            connected: false,
            checked: true,
            toggled: true,
            radio_sel: 1,
            slider_val: 40.0,
            scroll_off: 0.0,
            swatch_sel: 0,
            shape_sel: 0,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ref mut app) = self.state else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::CursorMoved { position, .. } => {
                app.mouse_pos = Some((position.x, position.y));
                app.window.request_redraw();
            }

            WindowEvent::CursorLeft { .. } => {
                app.mouse_pos = None;
                app.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: WinitMouseButton::Left,
                ..
            } => {
                app.button_down = true;
                app.window.request_redraw();
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: WinitMouseButton::Left,
                ..
            } => {
                app.button_down = false;
                app.clicked = Some(MouseButton::Left);
                app.window.request_redraw();
            }

            WindowEvent::RedrawRequested => app.render(),

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(ref app) = self.state {
            app.window.request_redraw();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut handler = Handler { state: None };
    event_loop.run_app(&mut handler)?;

    Ok(())
}
