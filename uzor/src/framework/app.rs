//! Application lifecycle trait, configuration, and the `NoPanel` default type.

use crate::layout::docking::DockPanel;
use crate::layout::LayoutManager;
use crate::input::core::event_processor::PlatformEvent;
use super::builder::RgbaIcon;
use super::multi_window::{WindowCtx, WindowKey, WindowSpec};
use crate::platform::types::CornerStyle;

// ── NoPanel ───────────────────────────────────────────────────────────────────

/// Empty dock-panel type for apps that do not use dockable panels.
#[derive(Clone, Debug, Default)]
pub struct NoPanel;

impl DockPanel for NoPanel {
    fn title(&self) -> &str { "" }
    fn type_id(&self) -> &'static str { "no_panel" }
    fn min_size(&self) -> (f32, f32) { (0.0, 0.0) }
    fn closable(&self) -> bool { false }
}

// ── App trait ─────────────────────────────────────────────────────────────────

/// Application lifecycle trait.
pub trait App<P: DockPanel = NoPanel>: Sized + 'static {
    fn init(&mut self, _key: &WindowKey, _layout: &mut LayoutManager<P>) {}

    fn ui(&mut self, win: &mut WindowCtx<'_, P>);

    /// Optional per-region paint schedule.
    ///
    /// Return one or more [`RenderRegion`]s describing sub-rects of the
    /// window with their own paint cadence (`target_fps = 0` for
    /// dirty-driven, `>0` for FPS-capped, `UNCAPPED_FPS` for continuous).
    /// The runtime collects these once per `ui()` call and uses them to
    /// schedule wake-ups via `ControlFlow::WaitUntil`.
    ///
    /// Default: empty — the whole window paints on every wake-up
    /// (mouse / event-driven, current behaviour).
    fn regions(&mut self) -> Vec<crate::render::RenderRegion> {
        Vec::new()
    }

    /// Per-region paint callback. The runtime calls this only for regions
    /// whose `RegionScheduleState` is "due" on the current event-loop
    /// wake-up.  The provided `WindowCtx::render` writes into a scene
    /// dedicated to `region_id`; non-due regions are appended from cache.
    ///
    /// Default: forwards to `ui()` so apps that don't split per region
    /// keep working unchanged.
    fn draw_region(&mut self, _region_id: &str, win: &mut WindowCtx<'_, P>) {
        self.ui(win);
    }

    fn take_pending_spawn(&mut self) -> Option<WindowSpec> { None }

    fn take_window_to_close(&mut self) -> Option<WindowKey> { None }

    fn on_chrome_new_window(&mut self, _source_window: &WindowKey) -> Option<WindowSpec> {
        None
    }

    fn on_event(&mut self, _event: &PlatformEvent) -> bool { false }

    fn shutdown(&mut self) {}

    fn on_dismiss(&mut self,
        _layout: &mut LayoutManager<P>,
        _overlay: crate::layout::OverlayHandle,
    ) {}

    fn on_modal_close(&mut self,
        _layout: &mut LayoutManager<P>,
        _modal: crate::layout::ModalHandle,
    ) {}

    fn on_modal_tab(&mut self,
        _layout: &mut LayoutManager<P>,
        _modal: crate::layout::ModalHandle,
        _index: usize,
    ) {}

    fn on_dropdown_item(&mut self,
        _layout: &mut LayoutManager<P>,
        _dropdown: crate::layout::DropdownHandle,
        _item_id: &str,
    ) {}

    fn on_toolbar_item(&mut self,
        _layout: &mut LayoutManager<P>,
        _toolbar: crate::layout::ToolbarHandle,
        _item_id: &str,
    ) {}

    fn on_context_menu_item(&mut self,
        _layout: &mut LayoutManager<P>,
        _menu: crate::layout::ContextMenuHandle,
        _index: usize,
    ) {}

    fn on_chrome_tab(&mut self,
        _layout: &mut LayoutManager<P>,
        _tab_index: usize,
    ) {}

    fn on_chrome_control(&mut self,
        _layout: &mut LayoutManager<P>,
        _control: crate::layout::ChromeWindowControl,
    ) {}

    fn on_dispatch(&mut self,
        _layout: &mut LayoutManager<P>,
        _event: crate::layout::DispatchEvent,
    ) {}

    fn on_unhandled_click(&mut self,
        _layout: &mut LayoutManager<P>,
        _widget_id: &crate::types::WidgetId,
    ) {}

    fn route_click(&mut self,
        layout: &mut LayoutManager<P>,
        x: f64,
        y: f64,
    ) -> bool {
        use crate::layout::{ClickOutcome, DispatchEvent};
        match layout.handle_click((x, y)) {
            ClickOutcome::DismissOverlay(handle) => {
                self.on_dismiss(layout, handle);
                true
            }
            ClickOutcome::DispatchEvent(ev) => {
                match ev {
                    DispatchEvent::ModalCloseRequested(h) => {
                        self.on_modal_close(layout, h);
                    }
                    DispatchEvent::ModalTabClicked { modal, index } => {
                        self.on_modal_tab(layout, modal, index);
                    }
                    DispatchEvent::DropdownItemClicked { dropdown, ref item_id } => {
                        self.on_dropdown_item(layout, dropdown.clone(), item_id);
                    }
                    DispatchEvent::ToolbarItemClicked { toolbar, ref item_id } => {
                        self.on_toolbar_item(layout, toolbar.clone(), item_id);
                    }
                    DispatchEvent::ContextMenuItemClicked { menu, item_index } => {
                        self.on_context_menu_item(layout, menu, item_index);
                    }
                    DispatchEvent::ChromeTabClicked { tab_index } => {
                        self.on_chrome_tab(layout, tab_index);
                    }
                    DispatchEvent::ChromeWindowControl { control } => {
                        self.on_chrome_control(layout, control);
                    }
                    DispatchEvent::Unhandled(ref id) => {
                        self.on_unhandled_click(layout, id);
                    }
                    other => {
                        self.on_dispatch(layout, other);
                    }
                }
                true
            }
            ClickOutcome::Unhandled { .. } => false,
        }
    }
}

// ── AppConfig ─────────────────────────────────────────────────────────────────

/// Per-app runtime configuration.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub title:           String,
    pub initial_size:    (u32, u32),
    pub min_size:        Option<(u32, u32)>,
    pub decorations:     bool,
    pub vsync:           bool,
    pub background:      u32,
    pub multi_window:    bool,
    pub fps_limit:       u32,
    pub msaa_samples:    u8,
    pub single_instance: Option<String>,
    /// App-level default border accent colour (`0x00RRGGBB`). Overridden per-window by
    /// [`WindowSpec::border_color`]. `None` = OS default.
    pub border_color:    Option<u32>,
    /// App-level default corner-rounding preference. Overridden per-window.
    pub corner_style:    CornerStyle,
    /// App-level default drop-shadow override. Overridden per-window. `None` = OS default.
    pub shadow:          Option<bool>,
    pub start_visible:   bool,
    pub icon:            Option<RgbaIcon>,
    /// If `Some(port)`, the platform window manager spawns a local
    /// HTTP control plane (uzor-agent-api) on `127.0.0.1:port`.  Used
    /// by external agents (LLMs, QA tooling, scripts) to read live
    /// LayoutManager state and inject input.
    pub agent_api_port:  Option<u16>,
    /// Baseline repaint cadence for windows that don't override it
    /// via [`crate::framework::multi_window::WindowSpec::tick_rate`].
    /// `Capped(60)` by default — every window gets a heartbeat so
    /// agent-driven state changes show up immediately.
    pub default_tick_rate: crate::render::TickRate,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title:           "uzor app".to_string(),
            initial_size:    (1280, 720),
            min_size:        Some((400, 300)),
            decorations:     false,
            vsync:           true,
            background:      0xFF181820,
            multi_window:    false,
            fps_limit:       0,
            msaa_samples:    1,
            single_instance: None,
            border_color:    None,
            corner_style:    CornerStyle::Default,
            shadow:          None,
            start_visible:   false,
            icon:            None,
            agent_api_port:  None,
            default_tick_rate: crate::render::TickRate::Capped(60),
        }
    }
}

// ── ClosureApp ────────────────────────────────────────────────────────────────

/// Adapter that wraps a closure into an [`App`] implementation.
pub struct ClosureApp<P, F>
where
    P: DockPanel,
    F: FnMut(&mut LayoutManager<P>, &mut dyn crate::render::RenderContext) + 'static,
{
    ui_fn: F,
    _phantom: std::marker::PhantomData<P>,
}

impl<P, F> ClosureApp<P, F>
where
    P: DockPanel + Default,
    F: FnMut(&mut LayoutManager<P>, &mut dyn crate::render::RenderContext) + 'static,
{
    pub fn new(ui_fn: F) -> Self {
        Self { ui_fn, _phantom: std::marker::PhantomData }
    }
}

impl<P, F> App<P> for ClosureApp<P, F>
where
    P: DockPanel + Default + 'static,
    F: FnMut(&mut LayoutManager<P>, &mut dyn crate::render::RenderContext) + 'static,
{
    fn ui(&mut self, win: &mut WindowCtx<'_, P>) {
        (self.ui_fn)(win.layout, win.render);
    }
}
