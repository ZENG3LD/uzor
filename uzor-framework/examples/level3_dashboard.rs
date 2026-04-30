//! # Level 3 — full uzor stack with LayoutManager + composite widgets.
//!
//! Uses `WinitInputBridge` for event boilerplate. Demonstrates:
//! - chrome titlebar with tab strip + window controls
//! - top toolbar (edge slot)
//! - left sidebar (edge slot)
//! - dock area with 2 panels
//! - modal — opens on sidebar button click
//! - popup — opens on toolbar item hover
//! - context menu — opens on right-click in dock area
//! - tooltip — provided by chrome composite
//!
//! All rects come from `LayoutManager::solve()`.  Atomic widgets inside
//! composites still get explicit rects (computed from the parent composite's
//! solved rect), but no hardcoded window-level rects are used.
//!
//! # Run
//!
//! ```sh
//! cargo run --example level3_dashboard -p uzor-framework
//! ```

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

// ── vello ──────────────────────────────────────────────────────────────────────
use vello::util::{RenderContext as VelloRenderCx, RenderSurface};
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use vello::peniko::{Color, Fill};
use vello::kurbo::Affine;

// ── uzor ──────────────────────────────────────────────────────────────────────
use uzor::docking::panels::{DockPanel, SplitKind};
use uzor::input::core::coordinator::{InputCoordinator, LayerId};
use uzor::input::pointer::state::{InputState, PointerState};
use uzor::layout::{EdgeSide, EdgeSlot, LayoutManager, OverlayEntry, OverlayKind};
use uzor::types::Rect;

// ── composite widgets ─────────────────────────────────────────────────────────
use uzor::ui::widgets::composite::chrome::input::register_layout_manager_chrome;
use uzor::ui::widgets::composite::chrome::settings::ChromeSettings;
use uzor::ui::widgets::composite::chrome::state::ChromeState;
use uzor::ui::widgets::composite::chrome::types::{
    ChromeAction, ChromeRenderKind, ChromeTabConfig, ChromeView,
};
use uzor::ui::widgets::composite::chrome::input::{chrome_hit_test, handle_chrome_action};

use uzor::ui::widgets::composite::toolbar::input::register_layout_manager_toolbar;
use uzor::ui::widgets::composite::toolbar::settings::ToolbarSettings;
use uzor::ui::widgets::composite::toolbar::state::ToolbarState;
use uzor::ui::widgets::composite::toolbar::types::{ToolbarItem, ToolbarRenderKind, ToolbarView};

use uzor::ui::widgets::composite::sidebar::input::register_layout_manager_sidebar;
use uzor::ui::widgets::composite::sidebar::settings::SidebarSettings;
use uzor::ui::widgets::composite::sidebar::state::SidebarState;
use uzor::ui::widgets::composite::sidebar::types::{
    HeaderAction, SidebarHeader, SidebarRenderKind, SidebarView,
};

use uzor::ui::widgets::composite::panel::input::register_layout_manager_panel;
use uzor::ui::widgets::composite::panel::settings::PanelSettings;
use uzor::ui::widgets::composite::panel::state::PanelState;
use uzor::ui::widgets::composite::panel::types::{PanelHeader, PanelRenderKind, PanelView};

use uzor::ui::widgets::composite::modal::input::register_layout_manager_modal;
use uzor::ui::widgets::composite::modal::settings::ModalSettings;
use uzor::ui::widgets::composite::modal::state::ModalState;
use uzor::ui::widgets::composite::modal::types::{BackdropKind, FooterBtn, FooterBtnStyle, ModalRenderKind, ModalView};

use uzor::ui::widgets::composite::context_menu::input::register_layout_manager_context_menu;
use uzor::ui::widgets::composite::context_menu::settings::ContextMenuSettings;
use uzor::ui::widgets::composite::context_menu::state::ContextMenuState;
use uzor::ui::widgets::composite::context_menu::types::{
    ContextMenuItem, ContextMenuRenderKind, ContextMenuView,
};

use uzor::ui::widgets::composite::popup::input::register_layout_manager_popup;
use uzor::ui::widgets::composite::popup::settings::PopupSettings;
use uzor::ui::widgets::composite::popup::state::PopupState;
use uzor::ui::widgets::composite::popup::types::{BackdropKind as PopupBackdrop, PopupRenderKind, PopupView, PopupViewKind};

// ── GPU render context ────────────────────────────────────────────────────────
use uzor_render_vello_gpu::VelloGpuRenderContext;

// ── winit input bridge ────────────────────────────────────────────────────────
use uzor_window_desktop::WinitInputBridge;

// ─────────────────────────────────────────────────────────────────────────────
// Window geometry
// ─────────────────────────────────────────────────────────────────────────────

const WIN_W: u32 = 1200;
const WIN_H: u32 = 800;

// ── Colours ───────────────────────────────────────────────────────────────────

const BG: Color = Color::from_rgb8(0x16, 0x16, 0x1e);

// ─────────────────────────────────────────────────────────────────────────────
// Minimal DockPanel implementation for this demo
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct DemoPanel {
    title: String,
}

impl DockPanel for DemoPanel {
    fn title(&self) -> &str {
        &self.title
    }
    fn type_id(&self) -> &'static str {
        "demo-panel"
    }
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

    // ── Layout + input ────────────────────────────────────────────────────────
    layout:    LayoutManager<DemoPanel>,
    coord:     InputCoordinator,
    bridge:    WinitInputBridge,
    start:     Instant,

    // ── Composite widget states ───────────────────────────────────────────────
    chrome_state:       ChromeState,
    toolbar_state:      ToolbarState,
    sidebar_state:      SidebarState,
    panel_a_state:      PanelState,
    panel_b_state:      PanelState,
    modal_state:        ModalState,
    popup_state:        PopupState,
    ctx_menu_state:     ContextMenuState,

    // ── Demo interaction state ────────────────────────────────────────────────
    selected_tab:       usize,
    modal_open:         bool,
    popup_open:         bool,

    // ── Dock leaf ids (set after first panel insertion) ───────────────────────
    leaf_a_id:          Option<String>,
    leaf_b_id:          Option<String>,
}

impl AppState {
    fn time_ms(&self) -> f64 {
        self.start.elapsed().as_millis() as f64
    }

    fn render(&mut self) {
        let (width, height) = {
            let s = &self.surface;
            (s.config.width, s.config.height)
        };
        let win_rect = Rect::new(0.0, 0.0, width as f64, height as f64);

        // ── 1. Solve layout ───────────────────────────────────────────────────
        self.layout.solve(win_rect);

        // ── 2. Build InputState for this frame ───────────────────────────────
        let (mx, my) = self.bridge.last_mouse_pos;
        let input = InputState {
            pointer: PointerState {
                pos: Some((mx, my)),
                ..PointerState::default()
            },
            ..InputState::default()
        };
        self.coord.begin_frame(input);

        // ── 3. Scene build + composite widget registration ────────────────────
        self.scene.reset();

        // Background
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            BG,
            None,
            &vello::kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
        );

        // Pre-compute values that borrow self immutably before the mutable scene borrow.
        let time_ms = self.time_ms();

        let mut render_ctx = VelloGpuRenderContext::new(&mut self.scene, 0.0, 0.0);

        // ── Chrome ────────────────────────────────────────────────────────────
        let tab_ids = ["tab-0", "tab-1", "tab-2"];
        let tabs = [
            ChromeTabConfig {
                id: tab_ids[0],
                label: "Dashboard",
                icon: None,
                color_tag: None,
                closable: true,
                active: self.selected_tab == 0,
            },
            ChromeTabConfig {
                id: tab_ids[1],
                label: "Charts",
                icon: None,
                color_tag: None,
                closable: true,
                active: self.selected_tab == 1,
            },
            ChromeTabConfig {
                id: tab_ids[2],
                label: "Settings",
                icon: None,
                color_tag: None,
                closable: false,
                active: self.selected_tab == 2,
            },
        ];
        let chrome_view = ChromeView {
            tabs: &tabs,
            active_tab_id: Some(tab_ids[self.selected_tab]),
            show_new_tab_btn: true,
            show_menu_btn:    false,
            is_maximized:     false,
            cursor_x:         mx,
            cursor_y:         my,
            time_ms,
        };
        let chrome_settings  = ChromeSettings::default();
        let chrome_kind      = ChromeRenderKind::Default;
        register_layout_manager_chrome(
            &mut self.layout,
            &mut render_ctx,
            "chrome",
            &mut self.chrome_state,
            &chrome_view,
            &chrome_settings,
            &chrome_kind,
            &LayerId::main(),
        );

        // ── Toolbar (top edge slot) ───────────────────────────────────────────
        let toolbar_items = [
            ToolbarItem::TextButton {
                id: "tb-file",
                text: "File",
                active: false,
                tooltip: Some("Open file menu"),
            },
            ToolbarItem::TextButton {
                id: "tb-view",
                text: "View",
                active: false,
                tooltip: Some("Toggle view options"),
            },
            ToolbarItem::Separator,
            ToolbarItem::TextButton {
                id: "tb-help",
                text: "Help",
                active: false,
                tooltip: None,
            },
        ];
        use uzor::ui::widgets::composite::toolbar::types::ToolbarSection;
        let toolbar_view = ToolbarView {
            start: ToolbarSection { items: &toolbar_items },
            center: ToolbarSection::empty(),
            end: ToolbarSection::empty(),
            chrome: None,
        };
        let toolbar_settings = ToolbarSettings::default();
        let toolbar_kind     = ToolbarRenderKind::Horizontal;
        register_layout_manager_toolbar(
            &mut self.layout,
            &mut render_ctx,
            "main-toolbar",
            "toolbar-widget",
            &mut self.toolbar_state,
            &toolbar_view,
            &toolbar_settings,
            &toolbar_kind,
            &LayerId::main(),
        );

        // ── Sidebar (left edge slot) ──────────────────────────────────────────
        let sidebar_actions: &[HeaderAction<'_>] = &[];
        let sidebar_header = SidebarHeader {
            icon: None,
            title: "Panels",
            actions: sidebar_actions,
        };
        let modal_open = self.modal_open;
        let mut sidebar_view = SidebarView {
            header: sidebar_header,
            tabs: &[],
            active_tab: None,
            body: Box::new(move |render: &mut dyn uzor::render::RenderContext, body_rect: Rect, _coord: &mut InputCoordinator| {
                // Draw a simple "Open Modal" button placeholder inside sidebar
                let btn_rect = Rect::new(
                    body_rect.x + 8.0,
                    body_rect.y + 8.0,
                    body_rect.width - 16.0,
                    32.0,
                );
                let btn_color = if modal_open { "#10b981" } else { "#2962ff" };
                render.set_fill_color(btn_color);
                render.fill_rounded_rect(btn_rect.x, btn_rect.y, btn_rect.width, btn_rect.height, 4.0);
                render.set_fill_color("#ffffff");
                render.fill_text(
                    if modal_open { "Close Modal" } else { "Open Modal" },
                    btn_rect.x + btn_rect.width / 2.0,
                    btn_rect.y + btn_rect.height / 2.0,
                );
            }),
            show_scrollbar: false,
            content_height: 200.0,
        };
        let sidebar_settings = SidebarSettings::default();
        let sidebar_kind     = SidebarRenderKind::Left;
        register_layout_manager_sidebar(
            &mut self.layout,
            &mut render_ctx,
            "left-sidebar",
            "sidebar-widget",
            &mut self.sidebar_state,
            &mut sidebar_view,
            &sidebar_settings,
            &sidebar_kind,
            &LayerId::main(),
        );

        // ── Panel A (dock leaf "panel-a") ─────────────────────────────────────
        {
            let panel_a_header_actions: &[uzor::ui::widgets::composite::panel::types::HeaderAction<'_>] = &[];
            let panel_a_header = PanelHeader {
                title: "Market Overview",
                actions: panel_a_header_actions,
            };
            let mut panel_a_view = PanelView {
                header: Some(panel_a_header),
                columns: &[],
                body: Box::new(|render: &mut dyn uzor::render::RenderContext, body_rect: Rect, _coord: &mut InputCoordinator| {
                    // Draw placeholder content
                    render.set_fill_color("rgba(255,255,255,0.04)");
                    render.fill_rounded_rect(
                        body_rect.x + 8.0,
                        body_rect.y + 8.0,
                        body_rect.width - 16.0,
                        60.0,
                        4.0,
                    );
                    render.set_fill_color("rgba(255,255,255,0.5)");
                    render.fill_text(
                        "Right-click for context menu",
                        body_rect.x + body_rect.width / 2.0,
                        body_rect.y + 44.0,
                    );
                }),
                footer: None,
                show_scrollbar: false,
                content_height: 400.0,
            };
            let panel_a_settings = PanelSettings::default();
            let panel_a_kind     = PanelRenderKind::WithHeader;
            if let Some(ref id) = self.leaf_a_id.clone() {
                register_layout_manager_panel(
                    &mut self.layout,
                    &mut render_ctx,
                    id,
                    "panel-a-widget",
                    &mut self.panel_a_state,
                    &mut panel_a_view,
                    &panel_a_settings,
                    &panel_a_kind,
                    &LayerId::main(),
                );
            }
        }

        // ── Panel B (dock leaf "panel-b") ─────────────────────────────────────
        {
            let panel_b_header_actions: &[uzor::ui::widgets::composite::panel::types::HeaderAction<'_>] = &[];
            let panel_b_header = PanelHeader {
                title: "Trade History",
                actions: panel_b_header_actions,
            };
            let mut panel_b_view = PanelView {
                header: Some(panel_b_header),
                columns: &[],
                body: Box::new(|render: &mut dyn uzor::render::RenderContext, body_rect: Rect, _coord: &mut InputCoordinator| {
                    render.set_fill_color("rgba(255,255,255,0.04)");
                    render.fill_rounded_rect(
                        body_rect.x + 8.0,
                        body_rect.y + 8.0,
                        body_rect.width - 16.0,
                        40.0,
                        4.0,
                    );
                    render.set_fill_color("rgba(255,255,255,0.5)");
                    render.fill_text(
                        "Panel B — Trade History",
                        body_rect.x + body_rect.width / 2.0,
                        body_rect.y + 28.0,
                    );
                }),
                footer: None,
                show_scrollbar: false,
                content_height: 400.0,
            };
            let panel_b_settings = PanelSettings::default();
            let panel_b_kind     = PanelRenderKind::WithHeader;
            if let Some(ref id) = self.leaf_b_id.clone() {
                register_layout_manager_panel(
                    &mut self.layout,
                    &mut render_ctx,
                    id,
                    "panel-b-widget",
                    &mut self.panel_b_state,
                    &mut panel_b_view,
                    &panel_b_settings,
                    &panel_b_kind,
                    &LayerId::main(),
                );
            }
        }

        // ── Modal (overlay) ───────────────────────────────────────────────────
        if self.modal_open {
            let footer_btns = [
                FooterBtn { label: "Close", style: FooterBtnStyle::Ghost },
                FooterBtn { label: "Apply", style: FooterBtnStyle::Primary },
            ];
            let mut modal_view = ModalView {
                title: Some("Settings"),
                tabs: &[],
                footer_buttons: &footer_btns,
                wizard_pages: &[],
                backdrop: BackdropKind::Dim,
                body: Box::new(|render: &mut dyn uzor::render::RenderContext, body_rect: Rect, _coord: &mut InputCoordinator| {
                    render.set_fill_color("rgba(255,255,255,0.06)");
                    render.fill_rounded_rect(
                        body_rect.x + 12.0,
                        body_rect.y + 12.0,
                        body_rect.width - 24.0,
                        80.0,
                        4.0,
                    );
                    render.set_fill_color("#a0a0b0");
                    render.fill_text(
                        "Modal content goes here.",
                        body_rect.x + body_rect.width / 2.0,
                        body_rect.y + 60.0,
                    );
                }),
            };
            let modal_settings = ModalSettings::default();
            let modal_kind     = ModalRenderKind::WithHeaderFooter;
            // Push the overlay so rect_for_overlay returns a rect.
            self.layout.push_overlay(OverlayEntry {
                id: "modal-overlay".to_string(),
                kind: OverlayKind::Modal,
                rect: Rect::new(
                    width as f64 / 2.0 - 250.0,
                    height as f64 / 2.0 - 200.0,
                    500.0,
                    400.0,
                ),
                anchor: None,
            });
            self.coord.push_layer(LayerId::modal(), 10, true);
            register_layout_manager_modal(
                &mut self.layout,
                &mut render_ctx,
                "modal-overlay",
                "modal-widget",
                &mut self.modal_state,
                &mut modal_view,
                &modal_settings,
                &modal_kind,
                &LayerId::modal(),
            );
        }

        // ── Context Menu (overlay) ────────────────────────────────────────────
        if self.ctx_menu_state.is_open {
            let menu_items = [
                ContextMenuItem {
                    action: "ctx-zoom-in",
                    label: "Zoom In",
                    icon: None,
                    danger: false,
                    separator_after: false,
                    enabled: true,
                },
                ContextMenuItem {
                    action: "ctx-zoom-out",
                    label: "Zoom Out",
                    icon: None,
                    danger: false,
                    separator_after: true,
                    enabled: true,
                },
                ContextMenuItem {
                    action: "ctx-reset",
                    label: "Reset View",
                    icon: None,
                    danger: false,
                    separator_after: false,
                    enabled: true,
                },
            ];
            let mut ctx_menu_view = ContextMenuView {
                items: &menu_items,
                target_id: None,
                title: None,
            };
            let ctx_menu_settings = ContextMenuSettings::default();
            let ctx_menu_kind     = ContextMenuRenderKind::Minimal;
            let menu_h = menu_items.len() as f64 * 28.0 + 8.0;
            self.layout.push_overlay(OverlayEntry {
                id: "ctx-menu-overlay".to_string(),
                kind: OverlayKind::ContextMenu,
                rect: Rect::new(
                    self.ctx_menu_state.x,
                    self.ctx_menu_state.y,
                    160.0,
                    menu_h,
                ),
                anchor: None,
            });
            self.coord.push_layer(LayerId::popup(), 20, false);
            register_layout_manager_context_menu(
                &mut self.layout,
                &mut render_ctx,
                "ctx-menu-overlay",
                "ctx-menu-widget",
                &mut self.ctx_menu_state,
                &mut ctx_menu_view,
                &ctx_menu_settings,
                &ctx_menu_kind,
                &LayerId::popup(),
            );
        }

        // ── Popup (overlay, shown on toolbar hover) ───────────────────────────
        if self.popup_open {
            let popup_origin = (200.0, 70.0);
            let mut popup_view = PopupView {
                origin: popup_origin,
                anchor: None,
                backdrop: PopupBackdrop::None,
                kind: PopupViewKind::Plain {
                    body: Box::new(|render: &mut dyn uzor::render::RenderContext, body_rect: Rect, _coord: &mut InputCoordinator| {
                        render.set_fill_color("#1e222d");
                        render.fill_rounded_rect(body_rect.x, body_rect.y, body_rect.width, body_rect.height, 4.0);
                        render.set_fill_color("#d1d4dc");
                        render.fill_text("Toolbar popup tooltip", body_rect.x + body_rect.width / 2.0, body_rect.y + 18.0);
                    }),
                },
            };
            let popup_settings = PopupSettings::default();
            let popup_kind     = PopupRenderKind::Plain;
            self.layout.push_overlay(OverlayEntry {
                id: "popup-overlay".to_string(),
                kind: OverlayKind::Popup,
                rect: Rect::new(popup_origin.0, popup_origin.1, 180.0, 36.0),
                anchor: None,
            });
            self.coord.push_layer(LayerId::popup(), 15, false);
            register_layout_manager_popup(
                &mut self.layout,
                &mut render_ctx,
                "popup-overlay",
                "popup-widget",
                &mut self.popup_state,
                &mut popup_view,
                &popup_settings,
                popup_kind,
                &LayerId::popup(),
            );
        }

        // ── 5. end_frame ──────────────────────────────────────────────────────
        let responses = self.coord.end_frame();

        // ── 6. Process responses ──────────────────────────────────────────────
        for (id, resp) in &responses {
            // Sidebar "Open Modal" button — registered by the composite
            if resp.clicked && id.0.contains("sidebar") {
                self.modal_open = !self.modal_open;
            }
            // Modal close-X / footer close button
            if resp.clicked && (id.0.contains("modal-close") || id.0.contains("modal-footer-0")) {
                self.modal_open = false;
            }
            // Chrome tab clicks — processed via hit-test below
            // Context menu dismiss on any click outside
            if resp.clicked && self.ctx_menu_state.is_open && !id.0.contains("ctx-menu") {
                self.ctx_menu_state.close();
            }
        }

        // ── 7. Chrome hit-test on left-up ─────────────────────────────────────
        // (handled in window_event via process_click — chrome_hit_test called there)

        // ── 8. Popup hover: open when over "tb-view" toolbar item ─────────────
        let hovered = self.coord.hovered_widget().map(|id| id.0.clone());
        let was_popup_open = self.popup_open;
        self.popup_open = hovered.as_deref() == Some("tb-view");
        if was_popup_open != self.popup_open && !self.popup_open {
            // Close popup — remove overlay on next frame (layout cleared automatically)
        }

        // ── 9. GPU submit ─────────────────────────────────────────────────────
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
        let mut encoder = dev.device.create_command_encoder(
            &vello::wgpu::CommandEncoderDescriptor { label: Some("l3-blit") },
        );
        self.surface
            .blitter
            .copy(&dev.device, &mut encoder, &self.surface.target_view, &surface_view);
        dev.queue.submit([encoder.finish()]);
        surface_texture.present();

        // Clear per-frame overlays (they are pushed fresh each frame).
        self.layout.clear_overlays();

        self.window.request_redraw();
    }

    /// Handle chrome hit on left-release.
    fn on_left_up(&mut self, x: f64, y: f64) {
        // Check chrome hit.
        let chrome_settings = ChromeSettings::default();
        let chrome_kind     = ChromeRenderKind::Default;
        let tab_ids = ["tab-0", "tab-1", "tab-2"];
        let tabs = [
            ChromeTabConfig { id: tab_ids[0], label: "Dashboard", icon: None, color_tag: None, closable: true, active: self.selected_tab == 0 },
            ChromeTabConfig { id: tab_ids[1], label: "Charts",    icon: None, color_tag: None, closable: true, active: self.selected_tab == 1 },
            ChromeTabConfig { id: tab_ids[2], label: "Settings",  icon: None, color_tag: None, closable: false, active: self.selected_tab == 2 },
        ];
        let chrome_view = ChromeView {
            tabs: &tabs,
            active_tab_id: Some(tab_ids[self.selected_tab]),
            show_new_tab_btn: true,
            show_menu_btn: false,
            is_maximized: false,
            cursor_x: x,
            cursor_y: y,
            time_ms: self.time_ms(),
        };

        if let Some(chrome_rect) = self.layout.rect_for_chrome() {
            let hit = chrome_hit_test(
                &self.chrome_state,
                &chrome_view,
                &chrome_settings,
                &chrome_kind,
                chrome_rect,
                (x, y),
            );
            match handle_chrome_action(hit) {
                ChromeAction::SelectTab(i) => {
                    self.selected_tab = i;
                }
                ChromeAction::CloseApp => {
                    // Signal exit — handled by window event.
                }
                _ => {}
            }
        }

        // Toggle modal if sidebar button area clicked.
        if let Some(sidebar_rect) = self.layout.rect_for_edge_slot("left-sidebar") {
            let btn_x = sidebar_rect.x + 8.0;
            let btn_y = sidebar_rect.y + 8.0 + 40.0; // approx header height
            let btn_w = sidebar_rect.width - 16.0;
            let btn_h = 32.0;
            if x >= btn_x && x <= btn_x + btn_w && y >= btn_y && y <= btn_y + btn_h {
                self.modal_open = !self.modal_open;
            }
        }

        // Dismiss modal if clicking outside.
        if self.modal_open {
            let (width, height) = {
                let s = &self.surface;
                (s.config.width, s.config.height)
            };
            let modal_rect = Rect::new(
                width as f64 / 2.0 - 250.0,
                height as f64 / 2.0 - 200.0,
                500.0,
                400.0,
            );
            if !modal_rect.contains(x, y) {
                self.modal_open = false;
            }
        }

        // Dismiss context menu on any click.
        if self.ctx_menu_state.is_open {
            let menu_rect = Rect::new(self.ctx_menu_state.x, self.ctx_menu_state.y, 160.0, 100.0);
            if !menu_rect.contains(x, y) {
                self.ctx_menu_state.close();
            }
        }
    }

    /// Handle right-click — open context menu.
    fn on_right_up(&mut self, x: f64, y: f64) {
        let (width, height) = {
            let s = &self.surface;
            (s.config.width, s.config.height)
        };
        self.ctx_menu_state.open_smart(
            x, y,
            width as f64, height as f64,
            160.0, 100.0,
            None,
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Layout setup helper
// ─────────────────────────────────────────────────────────────────────────────

fn setup_layout(layout: &mut LayoutManager<DemoPanel>) -> (String, String) {
    // Chrome is enabled by default (32px).

    // Top toolbar edge slot.
    layout.edges_mut().add(EdgeSlot {
        id: "main-toolbar".to_string(),
        side: EdgeSide::Top,
        thickness: 36.0,
        visible: true,
        order: 0,
    });

    // Left sidebar edge slot.
    layout.edges_mut().add(EdgeSlot {
        id: "left-sidebar".to_string(),
        side: EdgeSide::Left,
        thickness: 200.0,
        visible: true,
        order: 0,
    });

    // Insert two dock panels side-by-side (horizontal split).
    // add_leaf pushes to root; two calls produce SplitHorizontal layout.
    let leaf_a = layout.panels_mut().tree_mut().add_leaf(
        DemoPanel { title: "Market Overview".to_string() },
    );
    let ids = layout.panels_mut().tree_mut().split_leaf(leaf_a, SplitKind::Horizontal, 0.0, 0.0);
    // split_leaf returns [orig_clone, new_leaf]; the second is our panel B slot.
    let leaf_a2 = ids[0];
    let leaf_b  = ids[1];
    // Update panel B's title (split copies panel A into all new leaves).
    if let Some(leaf) = layout.panels_mut().tree_mut().leaf_mut(leaf_b) {
        if let Some(panel) = leaf.panels.first_mut() {
            panel.title = "Trade History".to_string();
        }
    }

    (leaf_a2.to_string(), leaf_b.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// winit ApplicationHandler
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
            .with_title("uzor L3 — Dashboard")
            .with_inner_size(winit::dpi::LogicalSize::new(WIN_W, WIN_H))
            .with_resizable(true);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("window creation should succeed"),
        );

        let mut render_cx = VelloRenderCx::new();
        let size = window.inner_size();

        // SAFETY: `window` is Arc-owned and lives for the entire app lifetime.
        let surface: RenderSurface<'static> = pollster::block_on(async {
            render_cx
                .create_surface(
                    Arc::clone(&window),
                    size.width.max(1),
                    size.height.max(1),
                    vello::wgpu::PresentMode::AutoVsync,
                )
                .await
                .expect("vello surface creation should succeed")
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

        let mut layout = LayoutManager::<DemoPanel>::new();
        let (leaf_a_id, leaf_b_id) = setup_layout(&mut layout);

        let mut chrome_state = ChromeState::new();
        chrome_state.sync_tabs(&["tab-0", "tab-1", "tab-2"]);
        chrome_state.active_tab_id = Some("tab-0".to_string());

        window.request_redraw();

        self.state = Some(AppState {
            window,
            render_cx,
            surface,
            renderer,
            scene: Scene::new(),
            layout,
            coord: InputCoordinator::new(),
            bridge: WinitInputBridge::new(),
            start: Instant::now(),
            chrome_state,
            toolbar_state:  ToolbarState::default(),
            sidebar_state:  SidebarState::default(),
            panel_a_state:  PanelState::default(),
            panel_b_state:  PanelState::default(),
            modal_state:    ModalState::default(),
            popup_state:    PopupState::default(),
            ctx_menu_state: ContextMenuState::default(),
            selected_tab:   0,
            modal_open:     false,
            popup_open:     false,
            leaf_a_id:      Some(leaf_a_id),
            leaf_b_id:      Some(leaf_b_id),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let Some(ref mut app) = self.state else { return };

        match &event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::Resized(size) => {
                let w = size.width.max(1);
                let h = size.height.max(1);
                app.render_cx.resize_surface(&mut app.surface, w, h);
                app.window.request_redraw();
                return;
            }
            WindowEvent::RedrawRequested => {
                app.render();
                return;
            }
            _ => {}
        }

        // ── Route to bridge ───────────────────────────────────────────────────
        let focused = app.coord.focused_widget().cloned();
        let out = app.bridge.handle_event(&mut app.coord, focused.as_ref(), &event);

        if out.cursor_moved.is_some() || out.text_changed || out.focus_cleared {
            app.window.request_redraw();
        }

        if let Some((pos, _clicked_id)) = out.left_up {
            app.on_left_up(pos.0, pos.1);
            app.window.request_redraw();
        }

        if let Some(pos) = out.right_up {
            app.on_right_up(pos.0, pos.1);
            app.window.request_redraw();
        }

        if out.wheel.is_some() {
            app.window.request_redraw();
        }

        // Mouse button down — request redraw for visual feedback.
        if let WindowEvent::MouseInput { state: ElementState::Pressed, .. } = &event {
            app.window.request_redraw();
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
