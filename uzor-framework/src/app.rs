//! Application lifecycle trait, configuration, and the `NoPanel` default type.

use uzor::docking::panels::DockPanel;
use uzor::layout::LayoutManager;
use uzor::input::core::event_processor::PlatformEvent;
use uzor_window_hub::RgbaIcon;

use crate::multi_window::{WindowCtx, WindowKey, WindowSpec};

// ── NoPanel ───────────────────────────────────────────────────────────────────

/// Empty dock-panel type for apps that do not use dockable panels.
///
/// This is the default for the `App<P>` generic parameter. Consumer apps that
/// do not need tabbed/split panel docking can use `App` (or `App<NoPanel>`)
/// without specifying a concrete panel type.
#[derive(Clone, Debug, Default)]
pub struct NoPanel;

impl DockPanel for NoPanel {
    fn title(&self) -> &str {
        ""
    }

    fn type_id(&self) -> &'static str {
        "no_panel"
    }

    fn min_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn closable(&self) -> bool {
        false
    }
}

// ── App trait ─────────────────────────────────────────────────────────────────

/// Application lifecycle trait.
///
/// Implementors describe per-frame UI and react to platform events.
/// Most apps only need to override [`App::ui`]; the remaining methods have
/// empty default implementations.
///
/// # Generic parameter
///
/// `P` is the dock-panel type. Use the default `NoPanel` for apps that do not
/// use the docking system:
///
/// ```rust,ignore
/// struct MyApp { /* fields */ }
/// impl App for MyApp {          // P = NoPanel by default
///     fn ui(&mut self, layout: &mut LayoutManager<NoPanel>, render: &mut uzor_render_hub::WindowRenderState) { ... }
/// }
/// ```
///
/// For apps with a docking tree:
///
/// ```rust,ignore
/// impl App<ChartPanel> for ChartApp {
///     fn ui(&mut self, layout: &mut LayoutManager<ChartPanel>, render: &mut WindowRenderState) { ... }
/// }
/// ```
pub trait App<P: DockPanel = NoPanel>: Sized + 'static {
    /// Called once for each window the moment its render state is ready,
    /// before the first frame for that window.
    ///
    /// `key` identifies which window is initialising — apps with multiple
    /// windows match on `key.as_str()` to route per-window setup (chrome
    /// height, edge slots, modals, …).  Single-window apps can ignore the
    /// argument.
    fn init(&mut self, _key: &WindowKey, _layout: &mut LayoutManager<P>) {}

    /// Called every frame for each open window in turn.
    ///
    /// Register widgets via `win.layout.ctx_mut()` helpers; build the vello
    /// scene or instanced draw calls via `win.render`.  Multi-window apps
    /// branch on `win.key.as_str()` to draw different content per window.
    ///
    /// `begin_frame()` has already been called on `win.render`; the runtime
    /// calls `submit_frame()` after this method returns.
    fn ui(&mut self, win: &mut WindowCtx<'_, P>);

    /// Drained by the runtime in `about_to_wait` after every event batch.
    /// Returning `Some(spec)` makes the runtime spawn an additional window
    /// on the next loop tick.  Apps that only need the windows registered
    /// at startup can ignore this hook.
    fn take_pending_spawn(&mut self) -> Option<WindowSpec> { None }

    /// Drained by the runtime to close a window without exiting the app.
    /// Returning `Some(key)` makes the runtime destroy the matching window;
    /// other windows continue running.
    fn take_window_to_close(&mut self) -> Option<WindowKey> { None }

    /// Called when the user clicked the chrome's "+" / new-window button
    /// inside `_source_window`.  Return `Some(spec)` to spawn the new
    /// window, or `None` to ignore.  Default ignores the click.
    fn on_chrome_new_window(&mut self, _source_window: &WindowKey) -> Option<WindowSpec> {
        None
    }

    /// Called for each [`PlatformEvent`] before default input processing.
    ///
    /// Return `true` to consume the event (preventing the runtime's default
    /// handling). Return `false` to let the runtime pass it to
    /// [`uzor::input::core::event_processor::EventProcessor`].
    fn on_event(&mut self, _event: &PlatformEvent) -> bool {
        false
    }

    /// Called once when the runtime is about to exit after all windows close.
    ///
    /// Use for cleanup: flush pending I/O, save state, etc.
    fn shutdown(&mut self) {}

    // ─── L4 typed dispatch hooks ──────────────────────────────────────────
    //
    // Optional callbacks the runtime invokes after `App::ui` once per frame
    // for each pointer event.  Override the ones your app needs; defaults
    // do nothing.  The runtime decodes the click via
    // `LayoutManager::handle_click` and routes the resulting `ClickOutcome`
    // here so the app does not write a giant `match` over `DispatchEvent`.
    //
    // For any event not covered below the runtime falls back to
    // [`Self::on_dispatch`] with the raw `DispatchEvent`.

    /// User dismissed an overlay by clicking outside it.
    fn on_dismiss(&mut self,
        _layout: &mut LayoutManager<P>,
        _overlay: uzor::layout::OverlayHandle,
    ) {}

    /// User clicked the close-X / a footer button on a modal.
    fn on_modal_close(&mut self,
        _layout: &mut LayoutManager<P>,
        _modal: uzor::layout::ModalHandle,
    ) {}

    /// User clicked a tab inside a modal.
    fn on_modal_tab(&mut self,
        _layout: &mut LayoutManager<P>,
        _modal: uzor::layout::ModalHandle,
        _index: usize,
    ) {}

    /// User clicked an item in a dropdown (`item_id` is the stable id from
    /// the `DropdownItem` row).
    fn on_dropdown_item(&mut self,
        _layout: &mut LayoutManager<P>,
        _dropdown: uzor::layout::DropdownHandle,
        _item_id: &str,
    ) {}

    /// User clicked an item in a toolbar (`item_id` is the stable id from
    /// the `ToolbarItem` row).
    fn on_toolbar_item(&mut self,
        _layout: &mut LayoutManager<P>,
        _toolbar: uzor::layout::ToolbarHandle,
        _item_id: &str,
    ) {}

    /// User clicked an item in a context menu.
    fn on_context_menu_item(&mut self,
        _layout: &mut LayoutManager<P>,
        _menu: uzor::layout::ContextMenuHandle,
        _index: usize,
    ) {}

    /// User clicked a chrome tab.  `tab_index` is the position in the
    /// `tabs` slice passed to `lm::chrome().tabs(...)`.
    fn on_chrome_tab(&mut self,
        _layout: &mut LayoutManager<P>,
        _tab_index: usize,
    ) {}

    /// User clicked one of the chrome window-control buttons (min / max
    /// / close / new-tab / menu / etc.).
    fn on_chrome_control(&mut self,
        _layout: &mut LayoutManager<P>,
        _control: uzor::layout::ChromeWindowControl,
    ) {}

    /// Catch-all for `DispatchEvent` variants without a typed hook above.
    /// Override when the typed callbacks aren't enough.
    fn on_dispatch(&mut self,
        _layout: &mut LayoutManager<P>,
        _event: uzor::layout::DispatchEvent,
    ) {}

    /// Catch-all for clicks that landed on a coordinator-registered widget
    /// but did not match any dispatcher pattern (raw widget id).
    fn on_unhandled_click(&mut self,
        _layout: &mut LayoutManager<P>,
        _widget_id: &uzor::types::WidgetId,
    ) {}

    // ─── Click routing entry point ────────────────────────────────────────

    /// Resolve a screen-space click via `LayoutManager::handle_click` and
    /// fan it out to the typed `on_*` callbacks above.  Apps call this from
    /// inside `ui()` (or from their input bridge) instead of writing a
    /// giant `match` on `ClickOutcome` / `DispatchEvent`.
    ///
    /// Returns `true` when at least one typed handler was invoked.
    fn route_click(&mut self,
        layout: &mut LayoutManager<P>,
        x: f64,
        y: f64,
    ) -> bool {
        use uzor::layout::{ClickOutcome, DispatchEvent};
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
///
/// Passed to [`crate::builder::AppBuilder::config`] to override defaults.
#[derive(Clone, Debug)]
pub struct AppConfig {
    /// Window title bar text.
    pub title: String,

    /// Initial logical window size `(width, height)` in pixels.
    pub initial_size: (u32, u32),

    /// Minimum logical window size. `None` = no minimum enforced.
    pub min_size: Option<(u32, u32)>,

    /// Whether to show OS-native window decorations (title bar, border).
    ///
    /// `false` = chromeless — the app draws its own window chrome.
    pub decorations: bool,

    /// When `true`, request VSync-enabled presentation mode.
    pub vsync: bool,

    /// Clear colour for each frame expressed as `0xAARRGGBB`.
    pub background: u32,

    /// Enable multi-window support. When `false` (default) only a single
    /// primary window is created and managed.
    pub multi_window: bool,

    /// FPS limit. `0` = unlimited (run as fast as possible / `Poll` control
    /// flow). Non-zero values set a `WaitUntil` deadline in `about_to_wait`.
    pub fps_limit: u32,

    /// MSAA sample count (1 = no MSAA, 4 = 4×MSAA).
    pub msaa_samples: u8,

    /// When `Some(name)`, call [`crate::utils::single_instance`] at startup
    /// and hold the guard for the process lifetime. The string is the Win32
    /// named-mutex name passed to `CreateMutexW`.
    pub single_instance: Option<String>,

    /// When `Some("#RRGGBB")`, set the Windows 11 DWM border color via
    /// `DwmSetWindowAttribute(DWMWA_BORDER_COLOR)`. Silently ignored on
    /// platforms / OS versions that do not support this attribute.
    pub dwm_border_color: Option<String>,

    /// Start the window invisible and reveal it only after the first GPU frame
    /// is presented. Eliminates the white-flash on startup.
    pub start_visible: bool,

    /// Optional window icon (taskbar, Alt-Tab, caption).
    ///
    /// When `Some`, the icon is applied immediately after window creation.
    /// Construct via [`uzor_window_hub::RgbaIcon::from_rgba`] or load from
    /// PNG/SVG using [`uzor_framework::AppBuilder::icon_from_png`].
    pub icon: Option<RgbaIcon>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "uzor app".to_string(),
            initial_size: (1280, 720),
            min_size: Some((400, 300)),
            decorations: false,
            vsync: true,
            background: 0xFF181820,
            multi_window: false,
            fps_limit: 0,
            msaa_samples: 1,
            single_instance: None,
            dwm_border_color: None,
            start_visible: false,
            icon: None,
        }
    }
}

// ── ClosureApp ────────────────────────────────────────────────────────────────

/// Adapter that implements [`App`] for a closure.
///
/// Created by [`run_closure`]; provides a quick way to prototype without
/// defining a full `App` implementation.
pub struct ClosureApp<P, F>
where
    P: DockPanel,
    F: FnMut(&mut LayoutManager<P>, &mut uzor_render_hub::WindowRenderState) + 'static,
{
    ui_fn: F,
    _phantom: std::marker::PhantomData<P>,
}

impl<P, F> ClosureApp<P, F>
where
    P: DockPanel + Default,
    F: FnMut(&mut LayoutManager<P>, &mut uzor_render_hub::WindowRenderState) + 'static,
{
    /// Wrap a closure into a `ClosureApp`.
    pub fn new(ui_fn: F) -> Self {
        Self {
            ui_fn,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<P, F> App<P> for ClosureApp<P, F>
where
    P: DockPanel + Default + 'static,
    F: FnMut(&mut LayoutManager<P>, &mut uzor_render_hub::WindowRenderState) + 'static,
{
    fn ui(&mut self, win: &mut WindowCtx<'_, P>) {
        (self.ui_fn)(win.layout, win.render);
    }
}
