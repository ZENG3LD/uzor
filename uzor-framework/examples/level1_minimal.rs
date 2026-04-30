//! # Level 1 — uzor as a bare hit-tester
//!
//! Demonstrates: pull ONLY `InputCoordinator` from uzor; build winit + vello +
//! widget rendering + widget state entirely by hand.  The button below is
//! written ENTIRELY from scratch — uzor's button widget is NOT used.
//!
//! ```text
//! L1:  InputCoordinator only — user wires platform events manually
//! L2:  + ContextManager + LayoutManager + ready-made uzor widgets
//! L4:  uzor-framework — framework owns everything, user provides App trait
//! ```
//!
//! Key patterns demonstrated
//! ─────────────────────────
//! 1. `InputCoordinator::begin_frame` → register widgets → `end_frame`
//! 2. Manual `InputState` construction from winit events (no EventProcessor)
//! 3. `register_atomic(id, WidgetKind::Custom, rect, Sense::CLICK | Sense::HOVER, &layer)`
//! 4. Click detection via `WidgetResponse::clicked` in `end_frame` responses
//! 5. vello `Scene` built from scratch — background + rounded rect + no text hack
//!
//! Surface initialisation uses `vello::util::RenderContext` (the proven path
//! used by every uzor GPU backend).  This avoids the manual `wgpu::Instance` +
//! `SurfaceTargetUnsafe` pattern that caused STATUS_STACK_BUFFER_OVERRUN in the
//! previous version.
//!
//! # Run
//!
//! ```sh
//! cargo run --example level1_minimal -p uzor-framework
//! ```

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

// ── uzor L1 imports (the ENTIRE uzor footprint for this example) ──────────────
use uzor::input::core::coordinator::{InputCoordinator, LayerId};
use uzor::input::core::sense::Sense;
use uzor::input::core::widget_kind::WidgetKind;
use uzor::input::pointer::state::{InputState, MouseButton as UzorBtn};
use uzor::types::{Rect, WidgetId};

// ── vello scene building ──────────────────────────────────────────────────────
use vello::kurbo::{Affine, RoundedRect};
use vello::peniko::{Color, Fill};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};

// ── vello surface/device pool (proven init path, same as uzor-render-hub) ────
use vello::util::{RenderContext, RenderSurface};

// ── Window geometry ───────────────────────────────────────────────────────────

const WIN_W: u32 = 320;
const WIN_H: u32 = 200;

// ── Colours ───────────────────────────────────────────────────────────────────

/// Window background (dark charcoal).
const BG: Color = Color::from_rgb8(22, 22, 30);
/// Button normal state (steel blue).
const BTN_NORMAL: Color = Color::from_rgb8(55, 105, 175);
/// Button hover state (lighter blue).
const BTN_HOVER: Color = Color::from_rgb8(85, 145, 220);
/// Button pressed state (dark navy).
const BTN_PRESSED: Color = Color::from_rgb8(30, 65, 120);
/// Button border.
const BTN_BORDER: Color = Color::from_rgb8(110, 170, 255);

// ─────────────────────────────────────────────────────────────────────────────
// MyButton — hand-rolled widget (NO uzor button widget used)
// ─────────────────────────────────────────────────────────────────────────────

/// A completely hand-rolled button widget.
///
/// State is owned here; the coordinator only does hit-testing.
/// Every frame:
///   1. caller registers it via `coord.register_atomic(…)`
///   2. `end_frame()` returns hover/click responses
///   3. `update_from_response()` advances state
///   4. `draw()` paints using raw vello primitives
struct MyButton {
    /// Stable ID for the coordinator.
    id: WidgetId,
    /// Screen rectangle (logical pixels).
    rect: Rect,
    /// Whether pointer is currently over the button.
    hovered: bool,
    /// Whether the left button is held down over this widget.
    pressed: bool,
    /// How many times the button has been clicked since startup.
    clicks: u32,
}

impl MyButton {
    fn new(id: &str, rect: Rect) -> Self {
        Self {
            id: WidgetId::new(id),
            rect,
            hovered: false,
            pressed: false,
            clicks: 0,
        }
    }

    /// Register this widget with the coordinator for the current frame.
    ///
    /// Must be called after `coord.begin_frame()` and before `coord.end_frame()`.
    fn register(&self, coord: &mut InputCoordinator, layer: &LayerId) {
        coord.register_atomic(
            self.id.clone(),
            WidgetKind::Custom,
            self.rect,
            Sense::CLICK | Sense::HOVER,
            layer,
        );
    }

    /// Apply this frame's coordinator responses to button state.
    fn update_from_responses(&mut self, responses: &[(WidgetId, uzor::input::core::response::WidgetResponse)]) {
        for (id, resp) in responses {
            if *id == self.id {
                self.hovered = resp.hovered;
                if resp.clicked {
                    self.clicks += 1;
                    self.pressed = false;
                }
            }
        }
    }

    /// Track pressed state directly from winit (button_down is not tracked by
    /// the coordinator in the same way, so we do it ourselves).
    fn set_pressed(&mut self, pressed: bool) {
        // Only set pressed when the pointer is over us.
        if self.hovered {
            self.pressed = pressed;
        } else {
            self.pressed = false;
        }
    }

    /// Draw a filled rounded rect.  Text replaced by a simple inner rect
    /// (vello's text API requires a font stack; at L1 we skip that dependency).
    ///
    /// Visual: steel-blue rect, border, and a small white "counter" rect inside
    /// whose width grows with every click (capped at the button width).
    fn draw(&self, scene: &mut Scene) {
        let color = if self.pressed {
            BTN_PRESSED
        } else if self.hovered {
            BTN_HOVER
        } else {
            BTN_NORMAL
        };

        let r = &self.rect;
        let (rw, rh) = (r.width, r.height);
        let shape = RoundedRect::new(r.x, r.y, r.x + rw, r.y + rh, 6.0);

        // Filled background.
        scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &shape);

        // Border (stroke via two nested rects — vello Stroke needs glyph lib;
        // instead we draw a slightly larger rect beneath as a "border").
        let border_shape = RoundedRect::new(
            r.x - 1.0, r.y - 1.0,
            r.x + rw + 1.0, r.y + rh + 1.0,
            7.0,
        );
        // Draw border-colored outer rect first (paint-over approach):
        scene.fill(Fill::NonZero, Affine::IDENTITY, BTN_BORDER, None, &border_shape);
        // Then re-draw the fill on top to leave only the border visible.
        scene.fill(Fill::NonZero, Affine::IDENTITY, color, None, &shape);

        // Click counter visualisation: a narrow white bar at the bottom of the
        // button whose width maps to click count (capped at button width - 8px).
        let bar_w = ((self.clicks as f64 * 10.0).min(rw - 8.0)).max(0.0);
        if bar_w > 0.0 {
            let bar = RoundedRect::new(
                r.x + 4.0,
                r.y + rh - 8.0,
                r.x + 4.0 + bar_w,
                r.y + rh - 4.0,
                2.0,
            );
            scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &bar);
        }

        // Small dot in the centre so there is SOMETHING visible even at 0 clicks.
        let dot_r = 4.0;
        let cx = r.x + rw / 2.0;
        let cy = r.y + rh / 2.0 - 4.0; // slightly above centre
        let dot = vello::kurbo::Circle::new((cx, cy), dot_r);
        let dot_color = if self.pressed {
            Color::from_rgb8(255, 255, 255)
        } else {
            Color::from_rgb8(200, 230, 255)
        };
        scene.fill(Fill::NonZero, Affine::IDENTITY, dot_color, None, &dot);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// App — owns window, vello, and all state
// ─────────────────────────────────────────────────────────────────────────────

/// All live state for the running application.
struct AppState {
    window: Arc<Window>,

    // ── vello GPU surface (managed by vello::util::RenderContext) ────────────
    //
    // `RenderContext` is vello's name for the wgpu device pool.
    // `RenderSurface<'static>` is safe because `window` is Arc-owned and kept
    // alive by this struct for the whole duration of the app.
    render_cx: RenderContext,
    surface: RenderSurface<'static>,
    renderer: Renderer,

    // ── Vello scene ──────────────────────────────────────────────────────────
    scene: Scene,

    // ── uzor L1 ──────────────────────────────────────────────────────────────
    coord: InputCoordinator,
    layer: LayerId,
    button: MyButton,

    // ── Raw input collected from winit events this frame ─────────────────────
    mouse_pos: Option<(f64, f64)>,
    /// Set when left button is released (produces a "click" in uzor terms).
    clicked: Option<UzorBtn>,
    /// Whether left button is currently held down.
    button_down: bool,
}

impl AppState {
    /// Build a complete frame: input → coordinator → scene → GPU submit.
    fn render(&mut self) {
        let (width, height) = {
            let s = &self.surface;
            (s.config.width, s.config.height)
        };

        // ── 1. Build InputState from this frame's events ──────────────────────
        let mut input = InputState::default();
        input.pointer.pos = self.mouse_pos;
        input.pointer.clicked = self.clicked.take();
        // button_down is used by coordinator for drag detection; also consumed
        // by our manual `set_pressed` call below.
        if self.button_down {
            input.pointer.button_down = Some(UzorBtn::Left);
        }

        // ── 2. begin_frame ────────────────────────────────────────────────────
        self.coord.begin_frame(input);

        // ── 3. Register widgets ───────────────────────────────────────────────
        self.button.register(&mut self.coord, &self.layer);

        // ── 4. Build scene ────────────────────────────────────────────────────
        self.scene = Scene::new();

        // Background fill.
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            BG,
            None,
            &vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        // Draw the hand-rolled button.
        self.button.draw(&mut self.scene);

        // ── 5. end_frame — collect hover / click responses ────────────────────
        let responses = self.coord.end_frame();
        self.button.update_from_responses(&responses);
        self.button.set_pressed(self.button_down);

        // ── 6. Submit to GPU via vello (mlc-style two-step) ───────────────────
        //
        // Step A: render scene → off-screen Rgba8Unorm `target_view` (owned
        //         by RenderSurface, format matches vello's compute shaders).
        // Step B: blit `target_view` → swapchain `surface_view` (Bgra8Unorm,
        //         OS-presentable format).  This is the same path mlc and our
        //         framework runtime use; render_to_texture directly on a
        //         swapchain view fails with format mismatch.
        let dev = &self.render_cx.devices[self.surface.dev_id];
        let render_params = RenderParams {
            base_color: BG,
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        self.renderer
            .render_to_texture(
                &dev.device,
                &dev.queue,
                &self.scene,
                &self.surface.target_view,
                &render_params,
            )
            .unwrap_or_default();

        let surface_texture = match self.surface.surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let surface_view = surface_texture
            .texture
            .create_view(&vello::wgpu::TextureViewDescriptor::default());
        let mut encoder = dev
            .device
            .create_command_encoder(&vello::wgpu::CommandEncoderDescriptor {
                label: Some("level1-blit"),
            });
        self.surface
            .blitter
            .copy(&dev.device, &mut encoder, &self.surface.target_view, &surface_view);
        dev.queue.submit([encoder.finish()]);
        surface_texture.present();
        self.window.request_redraw();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// winit ApplicationHandler
// ─────────────────────────────────────────────────────────────────────────────

/// Outer shell — wraps `Option<AppState>` so we can lazily init on `resumed`.
struct Handler {
    state: Option<AppState>,
}

impl ApplicationHandler for Handler {
    /// Called once (or on Android resume).  Create window + GPU surface here.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("uzor L1 — bare hit-tester")
            .with_inner_size(winit::dpi::LogicalSize::new(WIN_W, WIN_H))
            .with_resizable(false);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("OS should be able to create a window on a desktop machine"),
        );

        // ── vello::util::RenderContext (the proven init path) ─────────────────
        //
        // `RenderContext` manages the wgpu instance + adapters internally.
        // This avoids the manual `wgpu::Instance` + `SurfaceTargetUnsafe` path
        // that caused STATUS_STACK_BUFFER_OVERRUN in the previous version.
        let mut render_cx = RenderContext::new();

        let size = window.inner_size();

        // SAFETY: `window` is wrapped in `Arc` and stored in `AppState` for the
        // entire lifetime of this app.  The raw handle is therefore valid as
        // long as `AppState` is alive, which satisfies `RenderSurface<'static>`.
        let surface: RenderSurface<'static> = pollster::block_on(async {
            render_cx
                .create_surface(
                    Arc::clone(&window),
                    size.width.max(1),
                    size.height.max(1),
                    vello::wgpu::PresentMode::AutoVsync,
                )
                .await
                .expect("vello should be able to create a GPU surface on a desktop machine")
        });

        let renderer = Renderer::new(
            &render_cx.devices[surface.dev_id].device,
            RendererOptions {
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
                ..RendererOptions::default()
            },
        )
        .expect("vello renderer creation should succeed");

        window.request_redraw();

        // Centre the button in the window.
        let btn_w = 140.0_f64;
        let btn_h = 44.0_f64;
        let btn_x = (WIN_W as f64 - btn_w) / 2.0;
        let btn_y = (WIN_H as f64 - btn_h) / 2.0;

        self.state = Some(AppState {
            window,
            render_cx,
            surface,
            renderer,
            scene: Scene::new(),
            coord: InputCoordinator::new(),
            layer: LayerId::main(),
            button: MyButton::new("demo_btn", Rect::new(btn_x, btn_y, btn_w, btn_h)),
            mouse_pos: None,
            clicked: None,
            button_down: false,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ref mut app) = self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            // Translate winit cursor position → uzor pointer position.
            // At L2+ `EventProcessor` does this automatically.
            WindowEvent::CursorMoved { position, .. } => {
                app.mouse_pos = Some((position.x, position.y));
                app.window.request_redraw();
            }

            WindowEvent::CursorLeft { .. } => {
                app.mouse_pos = None;
                app.window.request_redraw();
            }

            // Track button-down for `pressed` visual state.
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                app.button_down = true;
                app.window.request_redraw();
            }

            // A release produces the "clicked" signal for the coordinator.
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                app.button_down = false;
                app.clicked = Some(UzorBtn::Left);
                app.window.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                app.render();
            }

            _ => {}
        }
    }

    /// Keep redrawing continuously (prevents stale frame after OS compositor events).
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
    // `Wait` = only wake when the OS or our code asks for a redraw.
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut handler = Handler { state: None };
    event_loop.run_app(&mut handler)?;

    Ok(())
}
