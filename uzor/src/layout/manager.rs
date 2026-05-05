use std::collections::HashMap;

use crate::layout::docking::{PanelRect, DockPanel};
use super::dock_state::DockState;
use super::window::{WindowKey, WindowProvider};
use crate::core::types::Rect;
use crate::input::core::coordinator::LayerId;
use crate::input::WidgetKind;
use crate::types::WidgetId;
use crate::ui::widgets::composite::modal::state::ModalState;
use crate::ui::widgets::composite::popup::state::PopupState;
use crate::ui::widgets::composite::dropdown::state::DropdownState;
use crate::ui::widgets::composite::toolbar::state::ToolbarState;
use crate::ui::widgets::composite::sidebar::state::SidebarState;
use crate::ui::widgets::composite::context_menu::state::ContextMenuState;
use crate::ui::widgets::composite::chrome::state::ChromeState;
use crate::app_context::ContextManager;
use super::chrome_slot::ChromeSlot;
use super::edge_panels::EdgePanels;
use super::handles::{
    ContextMenuHandle, DropdownHandle, ModalHandle, OverlayHandle,
    PopupHandle, SidebarHandle, ToolbarHandle,
};
use super::overlay_stack::{OverlayStack, OverlayEntry};
use super::tree::{LayoutNode as TreeLayoutNode, LayoutNodeId, LayoutTree};
use super::z_layers::ZLayerTable;
use super::types::{OverlayKind, LayoutSolved};
use super::solve::solve_layout;
use super::styles::StyleManager;
use super::branch::{WindowBranch, WindowSlot};
use super::registry::{
    CompositeKind, CompositeRegistration, DismissFrame,
    layer_for_widget_kind,
};

/// Convert a `Rect` (f64) to a `PanelRect` (f32) for the dock layout pass.
fn panel_rect_from_rect(r: Rect) -> PanelRect {
    PanelRect::new(r.x as f32, r.y as f32, r.width as f32, r.height as f32)
}

/// Convert a `PanelRect` (f32) to a `Rect` (f64) for rect-query results.
fn panel_rect_to_rect(pr: PanelRect) -> Rect {
    Rect::new(pr.x as f64, pr.y as f64, pr.width as f64, pr.height as f64)
}

/// Top-level macro layout owner for uzor.
///
/// `LayoutManager` is the single source of truth for all widget rects in the
/// window:
/// - **Chrome** — system titlebar/menubar strip.
/// - **Edges** — top/bottom toolbars and left/right sidebars outside the dock.
/// - **Dock** — panel docking tree (delegated to `PanelDockingManager`).
/// - **Floating** — floating panel windows (also in `PanelDockingManager`).
/// - **Overlays** — z-ordered stack: dropdown, popup, modal, context_menu, etc.
///
/// ## System vs. user access
///
/// - System slots (chrome, edges, overlays) are managed by uzor internals.
///   App developers configure but do not add/remove them at runtime.
/// - User slots (dock panels, floating panels) are accessed via `panels_mut()`,
///   which returns `&mut PanelDockingManager<P>`.
///
/// ## Frame usage
///
/// ```rust,ignore
/// layout_manager.solve(viewport);   // produces all rects
/// let dock = layout_manager.rect_for_dock_area().unwrap();
/// // register panels, chrome, edges with InputCoordinator using solved rects …
/// ```
pub struct LayoutManager<P: DockPanel> {
    /// All windows the layout manager owns. The platform layer (winit-driven
    /// `uzor_desktop::Manager`, web `WebWindowProvider`, mobile, …) creates
    /// each `Box<dyn WindowProvider>` and `attach_window`s it; LM addresses
    /// the window through its `WindowKey` from then on.
    pub(crate) windows: std::collections::HashMap<
        crate::layout::window::WindowKey,
        WindowSlot<P>,
    >,

    /// The window flat methods (`solve`, `on_pointer_*`, `chrome_mut`, etc)
    /// implicitly read/write.  Set by the platform layer before routing
    /// events/ticks for that window.  `None` outside a windowed session.
    pub(crate) current_window: Option<crate::layout::window::WindowKey>,

    // ── Synced root state — shared across all branches ────────────────────

    /// Z-layer ordering table — palette-level, applies everywhere.
    z_layers: ZLayerTable,

    /// Frame timestamp, set by the runtime once per frame.
    pub(crate) frame_time_ms: f64,

    /// Centralised style/colour/size/texture registry.  Synced root —
    /// every window reads from the same palette.
    styles: StyleManager,

    /// Per-node sync classification.  Tells the visualiser (and future
    /// promote/demote operations) which LM nodes are global, opt-in
    /// shared, or always per-window.  See [`super::sync::SyncRegistry`].
    pub(crate) sync_registry: super::sync::SyncRegistry,

    /// Append-only ring buffer of agent-visible state transitions.
    /// HTTP shims expose it via `GET /log?since=<seq>` so external
    /// agents can replay everything that happened without resorting
    /// to screenshots.  See [`super::agent::log`].
    pub(crate) agent_log: super::agent::AgentLog,
}

impl<P: DockPanel> LayoutManager<P> {
    /// Create a new empty `LayoutManager`.
    ///
    /// Windows are added by the platform layer via `attach_window`; until
    /// at least one is attached, the flat methods (`solve`, `on_pointer_*`,
    /// etc) panic — they require `current_window` to be set.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            current_window: None,
            z_layers: ZLayerTable::default(),
            frame_time_ms: 0.0,
            styles: StyleManager::default(),
            sync_registry: super::sync::SyncRegistry::defaults(),
            agent_log: super::agent::AgentLog::default(),
        }
    }

    /// Read-only access to the agent log.
    pub fn agent_log(&self) -> &super::agent::AgentLog {
        &self.agent_log
    }

    /// Push one entry into the shared agent log.  Anyone holding
    /// `&mut LayoutManager` (LM internals, app code, blackbox-panel
    /// handlers) calls this to add a breadcrumb.  Convention:
    /// dotted-lowercase category like `"app.theme.changed"`,
    /// `"chart.crosshair"`, `"lm.click"`.
    pub fn agent_log_push(
        &mut self,
        category: impl Into<String>,
        payload: serde_json::Value,
    ) -> u64 {
        let ts = self.frame_time_ms;
        let win = self.current_window.as_ref().map(|k| k.as_str().to_owned());
        self.agent_log.push(ts, win, category, payload)
    }

    /// Convenience: push a `note` (free-form message string) into the
    /// log under category `"note"`.
    pub fn agent_log_note(&mut self, message: impl Into<String>) -> u64 {
        let payload = serde_json::json!({ "message": message.into() });
        self.agent_log_push("note", payload)
    }

    /// Apply a named style preset and write a `lm.style.preset` entry
    /// to the agent log.  Apps should call this instead of
    /// `styles_mut().apply(...)` so external agents see the
    /// transition without polling.
    pub fn apply_style_preset<Pr: super::styles::Preset + ?Sized>(
        &mut self,
        preset: &Pr,
        name: impl Into<String>,
    ) {
        let name_string = name.into();
        self.styles.apply_named(preset, name_string.clone());
        self.agent_log_push(
            "lm.style.preset",
            serde_json::json!({ "name": name_string }),
        );
    }

    /// Read-only access to the sync-mode registry.
    pub fn sync_registry(&self) -> &super::sync::SyncRegistry {
        &self.sync_registry
    }

    /// Mutable access to the sync-mode registry.
    pub fn sync_registry_mut(&mut self) -> &mut super::sync::SyncRegistry {
        &mut self.sync_registry
    }

    /// Set the window the flat-API methods route to.  Platform layer
    /// calls this before forwarding events / running a tick for a window.
    pub fn set_current_window(&mut self, key: crate::layout::window::WindowKey) {
        self.current_window = Some(key);
    }

    /// Currently-routed window, if any.
    pub fn current_window(&self) -> Option<&crate::layout::window::WindowKey> {
        self.current_window.as_ref()
    }

    /// Branch for the currently-routed window — panics if no window is set.
    fn cur_branch(&self) -> &WindowBranch<P> {
        let key = self.current_window.as_ref()
            .expect("LayoutManager: no current_window — platform layer must call set_current_window");
        self.windows.get(key)
            .unwrap_or_else(|| panic!("LayoutManager: current_window {:?} not attached", key))
    }

    fn cur_branch_mut(&mut self) -> &mut WindowBranch<P> {
        let key = self.current_window.clone()
            .expect("LayoutManager: no current_window — platform layer must call set_current_window");
        self.windows.get_mut(&key)
            .unwrap_or_else(|| panic!("LayoutManager: current_window not attached"))
    }

    // ------------------------------------------------------------------
    // System slots — uzor-internal
    // ------------------------------------------------------------------

    /// Read-only access to the chrome slot configuration.
    pub fn chrome(&self) -> &ChromeSlot {
        &self.cur_branch().chrome
    }

    /// Mutable access to the chrome slot configuration.
    pub fn chrome_mut(&mut self) -> &mut ChromeSlot {
        &mut self.cur_branch_mut().chrome
    }

    /// Read-only access to the edge panel registry.
    pub fn edges(&self) -> &EdgePanels {
        &self.cur_branch().edges
    }

    /// Mutable access to the edge panel registry.
    pub fn edges_mut(&mut self) -> &mut EdgePanels {
        &mut self.cur_branch_mut().edges
    }

    /// Read-only access to the overlay stack.
    pub fn overlays(&self) -> &OverlayStack {
        &self.cur_branch().overlays
    }

    /// Mutable access to the overlay stack.
    pub fn overlays_mut(&mut self) -> &mut OverlayStack {
        &mut self.cur_branch_mut().overlays
    }

    /// Read-only access to the z-layer table.
    pub fn z_layers(&self) -> &ZLayerTable {
        &self.z_layers
    }

    /// Mutable access to the z-layer table.
    pub fn z_layers_mut(&mut self) -> &mut ZLayerTable {
        &mut self.z_layers
    }

    /// Read-only access to the embedded `ContextManager`.
    pub fn ctx(&self) -> &ContextManager {
        &self.cur_branch().ctx
    }

    /// Mutable access to the embedded `ContextManager`.
    ///
    /// Level-3 registration helpers call this internally to forward to
    /// `register_context_manager_*` without requiring the caller to hold a
    /// separate `ContextManager` reference.
    pub fn ctx_mut(&mut self) -> &mut ContextManager {
        &mut self.cur_branch_mut().ctx
    }

    /// Read-only access to the click dispatch table.
    pub fn dispatcher(&self) -> &super::ClickDispatcher {
        &self.cur_branch().dispatcher
    }

    /// Default opening size for a sidebar kind, computed from the most
    /// recent viewport `LayoutManager` saw via `solve(window_rect)`.
    ///
    /// L/R sidebars get `frac * viewport.width`, T/B get `frac * viewport.height`.
    /// Returns `None` until the first `solve()` has been called.
    pub fn sidebar_default_size(&self, is_horizontal_kind: bool, frac: f64) -> Option<f64> {
        let win = self.cur_branch().last_window?;
        let axis = if is_horizontal_kind { win.width } else { win.height };
        Some(axis * frac)
    }

    /// Mutable access to the click dispatch table.
    pub fn dispatcher_mut(&mut self) -> &mut super::ClickDispatcher {
        &mut self.cur_branch_mut().dispatcher
    }

    /// Run a click coordinate through the embedded `InputCoordinator`,
    /// then translate the resulting `WidgetId` (if any) via the dispatch
    /// table into a high-level [`super::DispatchEvent`].
    ///
    /// Returns:
    /// - `Some(event)` — a registered widget was hit; pattern matched.
    /// - `Some(DispatchEvent::Unhandled(id))` — widget was hit, but no
    ///   pattern matched it. The app may still react to the raw id.
    /// - `None` — no widget was hit (outside-click; close menus, etc.).
    pub fn dispatch_click(&mut self, x: f64, y: f64) -> Option<super::DispatchEvent> {
        let b = self.cur_branch_mut();
        let clicked = b.ctx.input.process_click(x, y)?;
        Some(
            b.dispatcher
                .dispatch(&clicked)
                .unwrap_or(super::DispatchEvent::Unhandled(clicked)),
        )
    }

    /// Translate a pre-resolved `WidgetId` into a high-level event.
    pub fn dispatch_widget(&self, id: &crate::types::WidgetId) -> super::DispatchEvent {
        self.cur_branch().dispatcher
            .dispatch(id)
            .unwrap_or_else(|| super::DispatchEvent::Unhandled(id.clone()))
    }

    /// Same as `dispatch_widget` but also writes the resolved event
    /// to the agent log.  Use from runtime call sites that want the
    /// dispatch to show up in `GET /log`.
    pub fn dispatch_widget_logged(&mut self, id: &crate::types::WidgetId) -> super::DispatchEvent {
        let event = self.cur_branch().dispatcher
            .dispatch(id)
            .unwrap_or_else(|| super::DispatchEvent::Unhandled(id.clone()));
        let window = self.current_window.as_ref().map(|k| k.as_str().to_owned());
        let ts = self.frame_time_ms;
        self.agent_log.push(
            ts,
            window,
            "lm.dispatch",
            serde_json::json!({
                "widget_id": id.as_str(),
                "event": format!("{:?}", event),
            }),
        );
        event
    }

    /// Clear the dispatch table, the overlay dismiss registry, and the
    /// per-frame composite registry of the current window.
    pub fn dispatcher_begin_frame(&mut self) {
        let b = self.cur_branch_mut();
        b.dispatcher.clear();
        b.dismiss_frames.clear();
        b.composite_registry.clear();
    }

    // -----------------------------------------------------------------------
    // Phase A+C — add_* factory methods (return typed handles)
    // -----------------------------------------------------------------------

    pub fn add_modal(&mut self, id: &str) -> ModalHandle {
        let widget_id = WidgetId(id.to_owned());
        self.cur_branch_mut().modals.entry(widget_id.clone()).or_default();
        ModalHandle { id: widget_id }
    }

    pub fn add_popup(&mut self, id: &str) -> PopupHandle {
        let widget_id = WidgetId(id.to_owned());
        self.cur_branch_mut().popups.entry(widget_id.clone()).or_default();
        PopupHandle { id: widget_id }
    }

    pub fn add_dropdown(&mut self, id: &str) -> DropdownHandle {
        let widget_id = WidgetId(id.to_owned());
        self.cur_branch_mut().dropdowns.entry(widget_id.clone()).or_default();
        DropdownHandle { id: widget_id }
    }

    pub fn add_toolbar(&mut self, id: &str) -> ToolbarHandle {
        let widget_id = WidgetId(id.to_owned());
        self.cur_branch_mut().toolbars.entry(widget_id.clone()).or_default();
        ToolbarHandle { id: widget_id }
    }

    pub fn add_sidebar(&mut self, id: &str) -> SidebarHandle {
        let widget_id = WidgetId(id.to_owned());
        self.cur_branch_mut().sidebars.entry(widget_id.clone()).or_default();
        SidebarHandle { id: widget_id }
    }

    pub fn add_context_menu(&mut self, id: &str) -> ContextMenuHandle {
        let widget_id = WidgetId(id.to_owned());
        self.cur_branch_mut().context_menus.entry(widget_id.clone()).or_default();
        ContextMenuHandle { id: widget_id }
    }

    // ── direct map access for composite input handlers ───────────────────
    // Composite `consume_event` helpers take/insert state from the maps to
    // avoid double-borrowing the layout manager.

    pub fn modals_map_mut(&mut self) -> &mut HashMap<WidgetId, ModalState>          { &mut self.cur_branch_mut().modals }
    pub fn popups_map_mut(&mut self) -> &mut HashMap<WidgetId, PopupState>          { &mut self.cur_branch_mut().popups }
    pub fn dropdowns_map_mut(&mut self) -> &mut HashMap<WidgetId, DropdownState>    { &mut self.cur_branch_mut().dropdowns }
    pub fn toolbars_map_mut(&mut self) -> &mut HashMap<WidgetId, ToolbarState>      { &mut self.cur_branch_mut().toolbars }
    pub fn sidebars_map_mut(&mut self) -> &mut HashMap<WidgetId, SidebarState>      { &mut self.cur_branch_mut().sidebars }
    pub fn context_menus_map_mut(&mut self) -> &mut HashMap<WidgetId, ContextMenuState> { &mut self.cur_branch_mut().context_menus }
    pub fn chrome_widget_state_mut(&mut self) -> &mut ChromeState { &mut self.cur_branch_mut().chrome_widget_state }

    // -----------------------------------------------------------------------
    // Phase A+C — handle-based state accessors (typed handles)
    // -----------------------------------------------------------------------

    /// Read-only access to a modal's state via its typed handle.
    ///
    /// # Panics
    ///
    /// Panics if the handle's state was removed from the registry
    /// (should not happen in normal usage — handles are stable).
    pub fn modal(&self, h: &ModalHandle) -> &ModalState {
        self.cur_branch().modals.get(&h.id)
            .expect("modal handle invalidated — state dropped from registry")
    }

    pub fn modal_mut(&mut self, h: &ModalHandle) -> &mut ModalState {
        self.cur_branch_mut().modals.get_mut(&h.id)
            .expect("modal handle invalidated — state dropped from registry")
    }

    pub fn popup(&self, h: &PopupHandle) -> &PopupState {
        self.cur_branch().popups.get(&h.id)
            .expect("popup handle invalidated — state dropped from registry")
    }

    pub fn popup_mut(&mut self, h: &PopupHandle) -> &mut PopupState {
        self.cur_branch_mut().popups.get_mut(&h.id)
            .expect("popup handle invalidated — state dropped from registry")
    }

    pub fn dropdown(&self, h: &DropdownHandle) -> &DropdownState {
        self.cur_branch().dropdowns.get(&h.id)
            .expect("dropdown handle invalidated — state dropped from registry")
    }

    pub fn dropdown_mut(&mut self, h: &DropdownHandle) -> &mut DropdownState {
        self.cur_branch_mut().dropdowns.get_mut(&h.id)
            .expect("dropdown handle invalidated — state dropped from registry")
    }

    pub fn toolbar(&self, h: &ToolbarHandle) -> &ToolbarState {
        self.cur_branch().toolbars.get(&h.id)
            .expect("toolbar handle invalidated — state dropped from registry")
    }

    pub fn toolbar_mut(&mut self, h: &ToolbarHandle) -> &mut ToolbarState {
        self.cur_branch_mut().toolbars.get_mut(&h.id)
            .expect("toolbar handle invalidated — state dropped from registry")
    }

    /// Frame timestamp in milliseconds since runtime start.  Set by the
    /// framework runtime once per frame; defaults to `0.0` when running
    /// outside `uzor-framework`.
    pub fn frame_time_ms(&self) -> f64 {
        self.frame_time_ms
    }

    /// Set the frame timestamp.  Called by `uzor-framework::Runtime` at
    /// the top of each frame.  Apps should not call this directly.
    pub fn set_frame_time_ms(&mut self, t: f64) {
        self.frame_time_ms = t;
    }

    /// Cursor position in screen coordinates of the currently-routed window.
    pub fn cursor_pos(&self) -> Option<(f64, f64)> {
        self.cur_branch().ctx.input.pointer_pos()
    }

    // -----------------------------------------------------------------------
    // L3-owned input accessors — forward to current branch
    // -----------------------------------------------------------------------

    pub fn hovered_widget(&self) -> Option<&crate::types::WidgetId> {
        self.cur_branch().last_hovered.as_ref()
    }

    pub fn was_pressed(&self, id: &WidgetId) -> bool {
        self.cur_branch().last_pressed.as_ref() == Some(id)
    }

    pub fn last_pressed_widget(&self) -> Option<&WidgetId> {
        self.cur_branch().last_pressed.as_ref()
    }

    pub fn pointer_pos(&self) -> Option<(f64, f64)> {
        self.cur_branch().last_pointer_pos
    }

    pub fn scroll_delta(&self) -> (f64, f64) {
        self.cur_branch().last_scroll
    }

    pub fn inputs_begin_frame(&mut self) {
        let b = self.cur_branch_mut();
        b.last_click       = None;
        b.last_right_click = None;
        b.last_pressed     = None;
        b.last_scroll      = (0.0, 0.0);
    }

    pub fn begin_frame(&mut self, time_ms: f64, viewport: crate::core::types::Rect) {
        self.cur_branch_mut().ctx.begin_frame_widgets_only(time_ms / 1000.0, viewport);
    }

    /// Per-frame teardown — clears one-shot input flags.  Call AFTER
    /// `App::ui` so `was_clicked` / `was_pressed` see the click that
    /// arrived just before this frame.
    pub fn end_frame_inputs(&mut self) {
        self.inputs_begin_frame();
    }

    /// Was the given widget id clicked (left-button) in the current frame?
    ///
    /// Reads from the L3-owned `last_click` field which is set by
    /// `on_pointer_up` — correct regardless of when `begin_frame` was
    /// called relative to the click event.
    pub fn was_clicked(&self, id: &WidgetId) -> bool {
        self.cur_branch().last_click.as_ref().map_or(false, |(c, _)| c == id)
    }

    /// Iterate every dock leaf and its solved screen-space rect for the
    /// currently-routed window.
    pub fn dock_leaves(&self) -> impl Iterator<Item = (crate::layout::docking::LeafId, Rect)> + '_ {
        self.cur_branch().dock.panel_rects().iter().map(|(&id, &pr)| {
            (id, panel_rect_to_rect(pr))
        }).collect::<Vec<_>>().into_iter()
    }

    pub fn sidebar(&self, h: &SidebarHandle) -> &SidebarState {
        self.cur_branch().sidebars.get(&h.id)
            .expect("sidebar handle invalidated — state dropped from registry")
    }

    pub fn sidebar_mut(&mut self, h: &SidebarHandle) -> &mut SidebarState {
        self.cur_branch_mut().sidebars.get_mut(&h.id)
            .expect("sidebar handle invalidated — state dropped from registry")
    }

    pub fn context_menu(&self, h: &ContextMenuHandle) -> &ContextMenuState {
        self.cur_branch().context_menus.get(&h.id)
            .expect("context_menu handle invalidated — state dropped from registry")
    }

    pub fn context_menu_mut(&mut self, h: &ContextMenuHandle) -> &mut ContextMenuState {
        self.cur_branch_mut().context_menus.get_mut(&h.id)
            .expect("context_menu handle invalidated — state dropped from registry")
    }

    pub fn chrome_state(&self) -> &ChromeState {
        &self.cur_branch().chrome_widget_state
    }

    pub fn chrome_state_mut(&mut self) -> &mut ChromeState {
        &mut self.cur_branch_mut().chrome_widget_state
    }

    /// Read-only access to the centralised style/colour/size/texture registry.
    ///
    /// `lm::*` builders call this to fall back to global palette tokens when no
    /// per-call `Settings` were supplied via `.settings(...)`.
    pub fn styles(&self) -> &StyleManager {
        &self.styles
    }

    /// Mutable access to the style registry.
    ///
    /// Apps call this in `App::init` (or on theme-switch events) to configure
    /// the global palette:
    ///
    /// ```ignore
    /// layout.styles_mut().set_color("accent", "#FBB26A");
    /// layout.styles_mut().apply(&MirageDarkPreset);
    /// ```
    pub fn styles_mut(&mut self) -> &mut StyleManager {
        &mut self.styles
    }

    /// Push a composite registration entry so [`Self::consume_event`] can
    /// route events without the app maintaining the chain manually.
    ///
    /// Called internally by each `register_layout_manager_*` helper.
    pub fn push_composite_registration(&mut self, reg: CompositeRegistration) {
        self.cur_branch_mut().composite_registry.push(reg);
    }

    // -----------------------------------------------------------------------
    // Phase A — consume_event
    // -----------------------------------------------------------------------

    /// Route a [`DispatchEvent`] through all registered composites in
    /// overlay-first priority order:
    ///
    /// Modal → Popup → Dropdown → ContextMenu → Toolbar → Sidebar
    ///
    /// Each composite gets to inspect the event and either consume it (return
    /// `None`, stopping the chain) or pass it through (`Some(event)`).
    ///
    /// Returns `None` when a composite consumed the event; returns the
    /// original event wrapped in `Some` when nothing consumed it.
    ///
    /// `cursor`   — current pointer position in screen coordinates.
    /// `viewport` — window `(width, height)` for resize-cap computation.
    pub fn consume_event(
        &mut self,
        event: super::DispatchEvent,
        cursor: (f64, f64),
        viewport: (f64, f64),
    ) -> Option<super::DispatchEvent> {
        use super::DispatchEvent;
        use crate::ui::widgets::composite::modal::input as modal_input;
        use crate::ui::widgets::composite::popup::input as popup_input;
        use crate::ui::widgets::composite::dropdown::input as dropdown_input;
        use crate::ui::widgets::composite::toolbar::input as toolbar_input;
        use crate::ui::widgets::composite::sidebar::input as sidebar_input;

        let registry: Vec<CompositeRegistration> = self.cur_branch().composite_registry.clone();

        let mut opt_ev: Option<DispatchEvent> = Some(event);

        for priority in [
            CompositeKind::Modal,
            CompositeKind::Popup,
            CompositeKind::Dropdown,
            CompositeKind::ContextMenu,
            CompositeKind::Toolbar,
            CompositeKind::Sidebar,
        ] {
            for reg in registry.iter().filter(|r| r.kind == priority) {
                let ev = match opt_ev.take() {
                    Some(e) => e,
                    None => return None,
                };
                opt_ev = match priority {
                    CompositeKind::Modal => {
                        let mut st = self.cur_branch_mut().modals.remove(&reg.widget_id).unwrap_or_default();
                        let result = modal_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            modal_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.cur_branch_mut().modals.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Popup => {
                        let mut st = self.cur_branch_mut().popups.remove(&reg.widget_id).unwrap_or_default();
                        let result = popup_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            popup_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.cur_branch_mut().popups.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Dropdown => {
                        let mut st = self.cur_branch_mut().dropdowns.remove(&reg.widget_id).unwrap_or_default();
                        let result = dropdown_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            dropdown_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.cur_branch_mut().dropdowns.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::ContextMenu => Some(ev),
                    CompositeKind::Toolbar => {
                        let mut st = self.cur_branch_mut().toolbars.remove(&reg.widget_id).unwrap_or_default();
                        let result = toolbar_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            toolbar_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.cur_branch_mut().toolbars.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Sidebar => {
                        let mut st = self.cur_branch_mut().sidebars.remove(&reg.widget_id).unwrap_or_default();
                        let result = sidebar_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            sidebar_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.cur_branch_mut().sidebars.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Chrome => Some(ev),
                };
            }
        }

        opt_ev
    }

    /// Read-only access to the macro layout tree of the current window.
    pub fn tree(&self) -> &LayoutTree {
        &self.cur_branch().tree
    }

    pub fn tree_mut(&mut self) -> &mut LayoutTree {
        &mut self.cur_branch_mut().tree
    }

    pub fn begin_frame_widgets(&mut self) {
        let b = self.cur_branch_mut();
        b.tree.clear_widgets();
        b.dismiss_frames.clear();
    }

    // ------------------------------------------------------------------
    // Overlay dismiss registry
    // ------------------------------------------------------------------

    /// Register an overlay frame for outside-click dismiss resolution.
    ///
    /// Composite `register_layout_manager_*` helpers call this after they
    /// have resolved the overlay rect. The registry is cleared each frame
    /// by [`Self::begin_frame_widgets`].
    ///
    /// Only open overlays should push a frame — if the overlay is closed,
    /// skip the call entirely so `dismiss_topmost_at` ignores it.
    pub fn push_dismiss_frame(&mut self, frame: DismissFrame) {
        self.cur_branch_mut().dismiss_frames.push(frame);
    }

    pub fn overlay_kind_for(&self, overlay_id: &str) -> Option<super::OverlayKind> {
        self.cur_branch().overlays.get(overlay_id).map(|e| e.kind)
    }

    pub fn dismiss_topmost_at(&self, pos: (f64, f64)) -> Option<WidgetId> {
        let b = self.cur_branch();
        if b.dismiss_frames.is_empty() {
            return None;
        }
        let topmost = b.dismiss_frames
            .iter()
            .enumerate()
            .rev()
            .max_by_key(|(i, f)| (f.z, *i))?
            .1;
        if topmost.rect.contains(pos.0, pos.1) { None }
        else { Some(topmost.overlay_id.clone()) }
    }

    pub fn compute_layer_for(&self, node_id: LayoutNodeId) -> LayerId {
        let tree = &self.cur_branch().tree;
        let chain = tree.parent_chain(node_id);
        let mut effective = LayerId::main();
        for ancestor_id in &chain {
            if let Some(entry) = tree.entry(*ancestor_id) {
                match &entry.node {
                    TreeLayoutNode::Widget(ref w) => {
                        if let Some(layer) = layer_for_widget_kind(w.kind) {
                            effective = layer;
                        }
                    }
                    TreeLayoutNode::System(ref s) => {
                        use super::tree::SystemNodeKind;
                        match s {
                            SystemNodeKind::OverlayStack => {}
                            SystemNodeKind::Chrome => { effective = LayerId::new("chrome"); }
                            SystemNodeKind::DockRoot | SystemNodeKind::EdgeSide { .. } => { effective = LayerId::main(); }
                            SystemNodeKind::FloatingLayer => { effective = LayerId::new("floating"); }
                        }
                    }
                }
            }
        }
        effective
    }

    // ------------------------------------------------------------------
    // User-facing dock + floating panels (current window)
    // ------------------------------------------------------------------

    pub fn dock(&self) -> &DockState<P> {
        &self.cur_branch().dock
    }

    pub fn dock_mut(&mut self) -> &mut DockState<P> {
        &mut self.cur_branch_mut().dock
    }

    #[doc(hidden)]
    pub fn panels(&self) -> &DockState<P> {
        &self.cur_branch().dock
    }

    #[doc(hidden)]
    pub fn panels_mut(&mut self) -> &mut DockState<P> {
        &mut self.cur_branch_mut().dock
    }

    // ------------------------------------------------------------------
    // Window registry — LM is the application root
    // ------------------------------------------------------------------

    /// Register a new OS window with the layout manager.
    ///
    /// The platform layer (winit, web, mobile) wraps its native handle in a
    /// `Box<dyn WindowProvider>` and hands it over.  From this point on LM
    /// addresses the window through `key`; the platform layer only forwards
    /// raw OS events.
    pub fn attach_window(
        &mut self,
        key:      crate::layout::window::WindowKey,
        provider: Box<dyn WindowProvider>,
    ) {
        let rect = provider.window_rect();
        let key_str = key.as_str().to_owned();
        self.windows.insert(key, WindowSlot::new(provider, rect));
        self.agent_log.push(
            self.frame_time_ms,
            Some(key_str.clone()),
            "lm.window.attach",
            serde_json::json!({ "window": key_str }),
        );
    }

    /// Drop a window from the registry.  Called by the platform layer on
    /// `WindowEvent::CloseRequested` (or equivalent) after teardown.
    pub fn detach_window(&mut self, key: &crate::layout::window::WindowKey) -> Option<WindowSlot<P>> {
        let removed = self.windows.remove(key);
        if removed.is_some() {
            self.agent_log.push(
                self.frame_time_ms,
                Some(key.as_str().to_owned()),
                "lm.window.detach",
                serde_json::json!({ "window": key.as_str() }),
            );
        }
        removed
    }

    /// Read-only access to a registered window's slot, by key.
    pub fn window(&self, key: &crate::layout::window::WindowKey) -> Option<&WindowSlot<P>> {
        self.windows.get(key)
    }

    /// Mutable access to a registered window's slot.
    pub fn window_mut(&mut self, key: &crate::layout::window::WindowKey) -> Option<&mut WindowSlot<P>> {
        self.windows.get_mut(key)
    }

    /// `true` when at least one window is currently attached.
    pub fn has_windows(&self) -> bool {
        !self.windows.is_empty()
    }

    /// Iterate registered window keys in arbitrary order.
    pub fn window_keys(&self) -> impl Iterator<Item = &crate::layout::window::WindowKey> {
        self.windows.keys()
    }

    // ==================================================================
    // Per-window API — all events / per-frame state pass through these.
    //
    // These resolve a `WindowKey` into the corresponding `WindowBranch`
    // and read/write the branch's own fields.  The flat fields on the
    // LM root are kept in sync on the active branch by these methods so
    // the legacy single-window API keeps working during the migration.
    // ==================================================================

    /// Mutable reference to the branch for `key`.  Panics with a clear
    /// message if `key` is not attached — this is a programmer error in
    /// the platform layer.
    fn branch_mut(&mut self, key: &WindowKey) -> &mut WindowBranch<P> {
        self.windows.get_mut(key)
            .unwrap_or_else(|| panic!("LayoutManager: no window attached for key {:?}", key))
    }

    /// Read-only branch lookup.  Same panic contract as `branch_mut`.
    fn branch(&self, key: &WindowKey) -> &WindowBranch<P> {
        self.windows.get(key)
            .unwrap_or_else(|| panic!("LayoutManager: no window attached for key {:?}", key))
    }

    // ── per-frame solve ───────────────────────────────────────────────

    /// Recompute all macro-level rects for `key`'s window.
    ///
    /// Drives the dock layout pass for that window's tree.  Returns a
    /// reference to the freshly computed `LayoutSolved`.
    pub fn solve_window(&mut self, key: &WindowKey, window: Rect) -> &LayoutSolved {
        let b = self.branch_mut(key);
        let solved = solve_layout(window, &b.chrome, &b.edges, &mut b.tree);
        let dock_pr = panel_rect_from_rect(solved.dock_area);
        b.dock.layout(dock_pr);
        b.last_solved = Some(solved);
        b.last_window = Some(window);
        b.rect = window;
        b.last_solved.as_ref().expect("just assigned")
    }

    /// Solved layout for `key`, or `None` if never solved.
    pub fn last_solved_for(&self, key: &WindowKey) -> Option<&LayoutSolved> {
        self.windows.get(key).and_then(|b| b.last_solved.as_ref())
    }

    /// Window rect from the most recent `solve_window` for `key`.
    pub fn last_window_for(&self, key: &WindowKey) -> Option<Rect> {
        self.windows.get(key).and_then(|b| b.last_window)
    }

    // ── per-window pointer events ─────────────────────────────────────

    pub fn on_pointer_move_in(&mut self, key: &WindowKey, x: f64, y: f64) {
        let b = self.branch_mut(key);
        b.ctx.input.set_cursor_pos(x, y);
        b.last_pointer_pos = Some((x, y));
        b.last_hovered = b.ctx.input.process_click(x, y)
            .or_else(|| b.ctx.input.process_hover(x, y));
    }

    pub fn on_pointer_down_in(&mut self, key: &WindowKey, x: f64, y: f64) {
        let b = self.branch_mut(key);
        b.ctx.input.set_cursor_pos(x, y);
        b.last_pointer_pos = Some((x, y));
        b.last_pressed = b.ctx.input.process_drag_press(x, y);
    }

    pub fn on_pointer_up_in(&mut self, key: &WindowKey, x: f64, y: f64) -> PointerUpOutcome {
        {
            let b = self.branch_mut(key);
            b.ctx.input.set_cursor_pos(x, y);
            b.last_pointer_pos = Some((x, y));
            let hit = b.ctx.input.process_click(x, y);
            if let Some(ref id) = hit {
                b.last_click = Some((id.clone(), (x, y)));
            }
        }
        match self.handle_click_in(key, (x, y)) {
            ClickOutcome::DismissOverlay(h) => PointerUpOutcome::DismissedOverlay(h),
            ClickOutcome::DispatchEvent(ev) => {
                let id = match &ev {
                    super::DispatchEvent::Unhandled(id) => id.clone(),
                    _ => crate::types::WidgetId::new(""),
                };
                PointerUpOutcome::Click(id, ev)
            }
            ClickOutcome::Unhandled { .. } => PointerUpOutcome::Unhandled,
        }
    }

    pub fn on_pointer_right_up_in(&mut self, key: &WindowKey, x: f64, y: f64) {
        let b = self.branch_mut(key);
        b.ctx.input.set_cursor_pos(x, y);
        b.last_pointer_pos = Some((x, y));
        let hit = b.ctx.input.process_right_click(x, y);
        if let Some(id) = hit {
            b.last_right_click = Some((id, (x, y)));
        }
    }

    pub fn on_scroll_in(&mut self, key: &WindowKey, dx: f64, dy: f64) {
        let b = self.branch_mut(key);
        b.last_scroll.0 += dx;
        b.last_scroll.1 += dy;
    }

    /// Combined dismiss-or-dispatch click entry, per-window variant.
    pub fn handle_click_in(&mut self, key: &WindowKey, pos: (f64, f64)) -> ClickOutcome {
        if let Some(overlay_id) = self.dismiss_topmost_at_in(key, pos) {
            let kind = self.overlay_kind_for_in(key, overlay_id.0.as_str());
            let handle = self.make_overlay_handle_in(key, overlay_id, kind);
            return ClickOutcome::DismissOverlay(handle);
        }

        let clicked = {
            let b = self.branch_mut(key);
            b.ctx.input.process_click(pos.0, pos.1)
        };
        match clicked {
            Some(id) => {
                let event = self.dispatch_widget_in(key, &id);
                ClickOutcome::DispatchEvent(event)
            }
            None => ClickOutcome::Unhandled { pos },
        }
    }

    pub fn dispatch_widget_in(&self, key: &WindowKey, id: &WidgetId) -> super::DispatchEvent {
        self.branch(key).dispatcher
            .dispatch(id)
            .unwrap_or_else(|| super::DispatchEvent::Unhandled(id.clone()))
    }

    pub fn dismiss_topmost_at_in(&self, key: &WindowKey, pos: (f64, f64)) -> Option<WidgetId> {
        let b = self.branch(key);
        if b.dismiss_frames.is_empty() {
            return None;
        }
        let topmost = b
            .dismiss_frames
            .iter()
            .enumerate()
            .rev()
            .max_by_key(|(i, f)| (f.z, *i))?
            .1;
        if topmost.rect.contains(pos.0, pos.1) {
            None
        } else {
            Some(topmost.overlay_id.clone())
        }
    }

    pub fn overlay_kind_for_in(&self, key: &WindowKey, overlay_id: &str) -> Option<super::OverlayKind> {
        self.branch(key).overlays.get(overlay_id).map(|e| e.kind)
    }

    fn make_overlay_handle_in(
        &self,
        key: &WindowKey,
        overlay_id: WidgetId,
        kind: Option<super::OverlayKind>,
    ) -> OverlayHandle {
        let slot = overlay_id.0.as_str();
        let b = self.branch(key);
        if let Some(reg) = b.composite_registry.iter().find(|r| r.slot_id == slot) {
            let wid = &reg.widget_id;
            return match reg.kind {
                CompositeKind::Modal => OverlayHandle::Modal(ModalHandle { id: wid.clone() }),
                CompositeKind::Popup => OverlayHandle::Popup(PopupHandle { id: wid.clone() }),
                CompositeKind::Dropdown => OverlayHandle::Dropdown(DropdownHandle { id: wid.clone() }),
                CompositeKind::ContextMenu => OverlayHandle::ContextMenu(ContextMenuHandle { id: wid.clone() }),
                _ => OverlayHandle::Other { overlay_id, kind },
            };
        }
        OverlayHandle::Other { overlay_id, kind }
    }

    // ── per-window frame lifecycle ────────────────────────────────────

    pub fn dispatcher_begin_frame_in(&mut self, key: &WindowKey) {
        let b = self.branch_mut(key);
        b.dispatcher.clear();
        b.dismiss_frames.clear();
        b.composite_registry.clear();
    }

    pub fn begin_frame_widgets_in(&mut self, key: &WindowKey) {
        let b = self.branch_mut(key);
        b.tree.clear_widgets();
        b.dismiss_frames.clear();
    }

    pub fn begin_frame_in(&mut self, key: &WindowKey, time_ms: f64, viewport: Rect) {
        let b = self.branch_mut(key);
        b.ctx.begin_frame_widgets_only(time_ms / 1000.0, viewport);
    }

    pub fn inputs_begin_frame_in(&mut self, key: &WindowKey) {
        let b = self.branch_mut(key);
        b.last_click       = None;
        b.last_right_click = None;
        b.last_pressed     = None;
        b.last_scroll      = (0.0, 0.0);
    }

    pub fn end_frame_inputs_in(&mut self, key: &WindowKey) {
        self.inputs_begin_frame_in(key);
    }

    // ── per-window accessors (state) ──────────────────────────────────

    pub fn chrome_for(&self, key: &WindowKey)       -> &ChromeSlot       { &self.branch(key).chrome }
    pub fn chrome_mut_for(&mut self, key: &WindowKey) -> &mut ChromeSlot { &mut self.branch_mut(key).chrome }
    pub fn edges_for(&self, key: &WindowKey)        -> &EdgePanels       { &self.branch(key).edges }
    pub fn edges_mut_for(&mut self, key: &WindowKey) -> &mut EdgePanels  { &mut self.branch_mut(key).edges }
    pub fn dock_for(&self, key: &WindowKey)         -> &DockState<P>     { &self.branch(key).dock }
    pub fn dock_mut_for(&mut self, key: &WindowKey) -> &mut DockState<P> { &mut self.branch_mut(key).dock }
    pub fn tree_for(&self, key: &WindowKey)         -> &LayoutTree       { &self.branch(key).tree }
    pub fn tree_mut_for(&mut self, key: &WindowKey) -> &mut LayoutTree   { &mut self.branch_mut(key).tree }
    pub fn ctx_for(&self, key: &WindowKey)          -> &ContextManager   { &self.branch(key).ctx }
    pub fn ctx_mut_for(&mut self, key: &WindowKey)  -> &mut ContextManager { &mut self.branch_mut(key).ctx }
    pub fn overlays_for(&self, key: &WindowKey)     -> &OverlayStack     { &self.branch(key).overlays }
    pub fn overlays_mut_for(&mut self, key: &WindowKey) -> &mut OverlayStack { &mut self.branch_mut(key).overlays }
    pub fn dispatcher_for(&self, key: &WindowKey)   -> &super::ClickDispatcher { &self.branch(key).dispatcher }
    pub fn dispatcher_mut_for(&mut self, key: &WindowKey) -> &mut super::ClickDispatcher { &mut self.branch_mut(key).dispatcher }
    pub fn chrome_state_for(&self, key: &WindowKey) -> &ChromeState      { &self.branch(key).chrome_widget_state }
    pub fn chrome_state_mut_for(&mut self, key: &WindowKey) -> &mut ChromeState { &mut self.branch_mut(key).chrome_widget_state }

    pub fn rect_for_chrome_in(&self, key: &WindowKey) -> Option<Rect> {
        self.branch(key).last_solved.as_ref().and_then(|s| s.chrome)
    }
    pub fn rect_for_dock_area_in(&self, key: &WindowKey) -> Option<Rect> {
        self.branch(key).last_solved.as_ref().map(|s| s.dock_area)
    }

    pub fn last_pressed_widget_in(&self, key: &WindowKey) -> Option<&WidgetId> {
        self.branch(key).last_pressed.as_ref()
    }
    pub fn hovered_widget_in(&self, key: &WindowKey) -> Option<&WidgetId> {
        self.branch(key).last_hovered.as_ref()
    }
    pub fn pointer_pos_in(&self, key: &WindowKey) -> Option<(f64, f64)> {
        self.branch(key).last_pointer_pos
    }
    pub fn was_clicked_in(&self, key: &WindowKey, id: &WidgetId) -> bool {
        self.branch(key).last_click.as_ref().map_or(false, |(c, _)| c == id)
    }

    // ── per-window composite-state accessors ─────────────────────────

    pub fn modal_in(&self, key: &WindowKey, h: &ModalHandle) -> &ModalState {
        self.branch(key).modals.get(&h.id).expect("modal handle invalidated")
    }
    pub fn modal_mut_in(&mut self, key: &WindowKey, h: &ModalHandle) -> &mut ModalState {
        self.branch_mut(key).modals.get_mut(&h.id).expect("modal handle invalidated")
    }
    pub fn popup_in(&self, key: &WindowKey, h: &PopupHandle) -> &PopupState {
        self.branch(key).popups.get(&h.id).expect("popup handle invalidated")
    }
    pub fn popup_mut_in(&mut self, key: &WindowKey, h: &PopupHandle) -> &mut PopupState {
        self.branch_mut(key).popups.get_mut(&h.id).expect("popup handle invalidated")
    }
    pub fn dropdown_in(&self, key: &WindowKey, h: &DropdownHandle) -> &DropdownState {
        self.branch(key).dropdowns.get(&h.id).expect("dropdown handle invalidated")
    }
    pub fn dropdown_mut_in(&mut self, key: &WindowKey, h: &DropdownHandle) -> &mut DropdownState {
        self.branch_mut(key).dropdowns.get_mut(&h.id).expect("dropdown handle invalidated")
    }
    pub fn toolbar_in(&self, key: &WindowKey, h: &ToolbarHandle) -> &ToolbarState {
        self.branch(key).toolbars.get(&h.id).expect("toolbar handle invalidated")
    }
    pub fn toolbar_mut_in(&mut self, key: &WindowKey, h: &ToolbarHandle) -> &mut ToolbarState {
        self.branch_mut(key).toolbars.get_mut(&h.id).expect("toolbar handle invalidated")
    }
    pub fn sidebar_in(&self, key: &WindowKey, h: &SidebarHandle) -> &SidebarState {
        self.branch(key).sidebars.get(&h.id).expect("sidebar handle invalidated")
    }
    pub fn sidebar_mut_in(&mut self, key: &WindowKey, h: &SidebarHandle) -> &mut SidebarState {
        self.branch_mut(key).sidebars.get_mut(&h.id).expect("sidebar handle invalidated")
    }
    pub fn context_menu_in(&self, key: &WindowKey, h: &ContextMenuHandle) -> &ContextMenuState {
        self.branch(key).context_menus.get(&h.id).expect("context_menu handle invalidated")
    }
    pub fn context_menu_mut_in(&mut self, key: &WindowKey, h: &ContextMenuHandle) -> &mut ContextMenuState {
        self.branch_mut(key).context_menus.get_mut(&h.id).expect("context_menu handle invalidated")
    }

    // ── per-window add_* (registers state slot in branch) ────────────

    pub fn add_modal_in(&mut self, key: &WindowKey, id: &str) -> ModalHandle {
        let widget_id = WidgetId(id.to_owned());
        self.branch_mut(key).modals.entry(widget_id.clone()).or_default();
        ModalHandle { id: widget_id }
    }
    pub fn add_popup_in(&mut self, key: &WindowKey, id: &str) -> PopupHandle {
        let widget_id = WidgetId(id.to_owned());
        self.branch_mut(key).popups.entry(widget_id.clone()).or_default();
        PopupHandle { id: widget_id }
    }
    pub fn add_dropdown_in(&mut self, key: &WindowKey, id: &str) -> DropdownHandle {
        let widget_id = WidgetId(id.to_owned());
        self.branch_mut(key).dropdowns.entry(widget_id.clone()).or_default();
        DropdownHandle { id: widget_id }
    }
    pub fn add_toolbar_in(&mut self, key: &WindowKey, id: &str) -> ToolbarHandle {
        let widget_id = WidgetId(id.to_owned());
        self.branch_mut(key).toolbars.entry(widget_id.clone()).or_default();
        ToolbarHandle { id: widget_id }
    }
    pub fn add_sidebar_in(&mut self, key: &WindowKey, id: &str) -> SidebarHandle {
        let widget_id = WidgetId(id.to_owned());
        self.branch_mut(key).sidebars.entry(widget_id.clone()).or_default();
        SidebarHandle { id: widget_id }
    }
    pub fn add_context_menu_in(&mut self, key: &WindowKey, id: &str) -> ContextMenuHandle {
        let widget_id = WidgetId(id.to_owned());
        self.branch_mut(key).context_menus.entry(widget_id.clone()).or_default();
        ContextMenuHandle { id: widget_id }
    }

    pub fn push_composite_registration_in(&mut self, key: &WindowKey, reg: CompositeRegistration) {
        self.branch_mut(key).composite_registry.push(reg);
    }
    pub fn push_dismiss_frame_in(&mut self, key: &WindowKey, frame: DismissFrame) {
        self.branch_mut(key).dismiss_frames.push(frame);
    }
    pub fn push_overlay_in(&mut self, key: &WindowKey, entry: OverlayEntry) {
        self.branch_mut(key).overlays.push(entry);
    }

    // ------------------------------------------------------------------
    // Per-frame solve
    // ------------------------------------------------------------------

    /// Recompute all macro-level rects given the current window size.
    ///
    /// Must be called each frame or on resize. Drives the dock layout pass
    /// internally. Returns a reference to the freshly computed `LayoutSolved`.
    pub fn solve(&mut self, window: Rect) -> &LayoutSolved {
        let b = self.cur_branch_mut();
        let solved = solve_layout(window, &b.chrome, &b.edges, &mut b.tree);
        let dock_pr = panel_rect_from_rect(solved.dock_area);
        b.dock.layout(dock_pr);
        b.last_solved = Some(solved);
        b.last_window = Some(window);
        b.rect = window;
        b.last_solved.as_ref().expect("just assigned")
    }

    pub fn last_solved(&self) -> Option<&LayoutSolved> {
        self.cur_branch().last_solved.as_ref()
    }

    pub fn last_window(&self) -> Option<Rect> {
        self.cur_branch().last_window
    }

    // ------------------------------------------------------------------
    // Rect accessors (current window)
    // ------------------------------------------------------------------

    pub fn rect_for_chrome(&self) -> Option<Rect> {
        self.cur_branch().last_solved.as_ref().and_then(|s| s.chrome)
    }

    pub fn rect_for_dock_area(&self) -> Option<Rect> {
        self.cur_branch().last_solved.as_ref().map(|s| s.dock_area)
    }

    pub fn rect_for_floating_area(&self) -> Option<Rect> {
        self.cur_branch().last_solved.as_ref().map(|s| s.floating_area)
    }

    pub fn rect_for_overlay(&self, id: &str) -> Option<Rect> {
        self.cur_branch().overlays.get(id).map(|e| e.rect)
    }

    // ------------------------------------------------------------------
    // Handle-friendly overlay rect lookups (chrome-aware body rect)
    // ------------------------------------------------------------------

    /// Full overlay rect for a modal handle (frame, including chrome strip).
    pub fn modal_rect(&self, h: &ModalHandle) -> Option<Rect> {
        self.rect_for_overlay(h.id_str())
    }

    /// Full overlay rect for a popup handle.
    pub fn popup_rect(&self, h: &PopupHandle) -> Option<Rect> {
        self.rect_for_overlay(h.id_str())
    }

    /// Full overlay rect for a dropdown handle.
    pub fn dropdown_rect(&self, h: &DropdownHandle) -> Option<Rect> {
        self.rect_for_overlay(h.id_str())
    }

    /// Full overlay rect for a context-menu handle.
    pub fn context_menu_rect(&self, h: &ContextMenuHandle) -> Option<Rect> {
        self.rect_for_overlay(h.id_str())
    }

    /// Body rect of a modal — overlay rect minus header / footer / padding,
    /// using default `ModalSettings` + `WithHeader` geometry.
    ///
    /// Apps that override `kind` / `settings` on the modal builder should
    /// compute the body rect themselves using the matching helpers in
    /// `uzor::ui::widgets::composite::modal::render::body_rect`.
    pub fn modal_body_rect(&self, h: &ModalHandle) -> Option<Rect> {
        use crate::ui::widgets::composite::modal::{
            render::body_rect as modal_body_rect_fn, settings::ModalSettings,
            types::{BackdropKind, ModalRenderKind, ModalView},
        };
        let frame = self.modal_rect(h)?;
        let view = ModalView {
            title: None,
            tabs: &[],
            footer_buttons: &[],
            wizard_pages: &[],
            backdrop: BackdropKind::Dim,
            overflow: crate::types::OverflowMode::Clip,
            resizable: false,
        };
        let settings = ModalSettings::default();
        let kind = ModalRenderKind::WithHeader;
        Some(modal_body_rect_fn(frame, &view, &settings, &kind))
    }

    /// Body rect of a popup — overlay rect minus padding, using default
    /// `PopupSettings`.
    pub fn popup_body_rect(&self, h: &PopupHandle) -> Option<Rect> {
        use crate::ui::widgets::composite::popup::{
            render::body_rect as popup_body_rect_fn, settings::PopupSettings,
        };
        let frame = self.popup_rect(h)?;
        let settings = PopupSettings::default();
        Some(popup_body_rect_fn(frame, &settings))
    }

    /// Rect for a named edge slot, or `None` if not present or not yet solved.
    ///
    /// Looks up the slot in `edges` (matching by `id`), determines its position
    /// within the solved edge rects array, and returns the corresponding `Rect`.
    pub fn rect_for_edge_slot(&self, id: &str) -> Option<Rect> {
        let b = self.cur_branch();
        let solved = b.last_solved.as_ref()?;
        let slot = b.edges.get(id)?;

        use super::types::EdgeSide;
        let visible: Vec<_> = b.edges.slots_for(slot.side).collect();
        let idx = visible.iter().position(|s| s.id == id)?;

        let rects = match slot.side {
            EdgeSide::Top    => &solved.edges.top,
            EdgeSide::Bottom => &solved.edges.bottom,
            EdgeSide::Left   => &solved.edges.left,
            EdgeSide::Right  => &solved.edges.right,
        };
        rects.get(idx).copied()
    }

    pub fn rect_for(&self, slot_id: &str) -> Option<Rect> {
        if slot_id == "chrome" {
            return self.rect_for_chrome();
        }
        if let Some(r) = self.rect_for_edge_slot(slot_id) {
            return Some(r);
        }
        if let Some(r) = self.rect_for_overlay(slot_id) {
            return Some(r);
        }
        if let Some(pr) = self.cur_branch().dock.rect_for_leaf_str(slot_id) {
            return Some(panel_rect_to_rect(pr));
        }
        None
    }

    // ------------------------------------------------------------------
    // Overlay helpers (current window)
    // ------------------------------------------------------------------

    pub fn push_overlay(&mut self, entry: OverlayEntry) {
        self.cur_branch_mut().overlays.push(entry);
    }

    pub fn clear_overlays(&mut self) {
        self.cur_branch_mut().overlays.clear();
    }

    pub fn overlays_in_draw_order(&mut self) -> &[OverlayEntry] {
        let z = self.z_layers.clone();
        let b = self.cur_branch_mut();
        b.overlays.sort_by_z(&z);
        b.overlays.entries()
    }

    /// Return the z value for the given overlay kind from the current `ZLayerTable`.
    pub fn z_for(&self, kind: OverlayKind) -> i32 {
        self.z_layers.z_for(kind)
    }

    // ------------------------------------------------------------------
    // Dock separators — register as drag-handle widgets
    // ------------------------------------------------------------------

    /// Register every dock separator as a coord drag-handle widget on `layer`.
    ///
    /// Call once per frame, after `solve(...)` and after composite registration
    /// (so overlay layers are pushed first and outrank the main layer in
    /// hit-testing). The separators participate in z-ordered hit-test, so a
    /// click under an open dropdown / popup / modal will be claimed by the
    /// overlay, not the separator.
    ///
    /// Each separator is registered with id `"dock-sep-{idx}"` where `idx`
    /// matches `panels().separators()[idx]`. A dispatcher pattern is also
    /// installed so a hit translates to
    /// [`super::DispatchEvent::DockSeparatorDragStarted`].
    pub fn register_dock_separators(&mut self, layer: &LayerId) {
        use crate::layout::docking::SeparatorOrientation as SepOrient;
        let b = self.cur_branch_mut();
        let sep_rects: Vec<(usize, Rect)> = b
            .dock
            .separators()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let thickness = s.thickness_for_state() as f64;
                let (x, y, w, h) = match s.orientation {
                    SepOrient::Vertical => (
                        s.position as f64 - thickness / 2.0,
                        s.start as f64,
                        thickness,
                        s.length as f64,
                    ),
                    SepOrient::Horizontal => (
                        s.start as f64,
                        s.position as f64 - thickness / 2.0,
                        s.length as f64,
                        thickness,
                    ),
                };
                (i, Rect::new(x, y, w, h))
            })
            .collect();

        let coord = &mut b.ctx.input;
        for (i, rect) in &sep_rects {
            coord.register_atomic(
                WidgetId::new(format!("dock-sep-{i}")),
                WidgetKind::DragHandle,
                *rect,
                crate::input::Sense::DRAG | crate::input::Sense::CLICK,
                layer,
            );
        }

        b.dispatcher.on_prefix(
            "dock-sep-",
            super::EventBuilder::DockSeparatorFromSuffix,
        );
    }

    // ------------------------------------------------------------------
    // Unified click entry point
    // ------------------------------------------------------------------

    /// Combined dismiss-or-dispatch click entry.
    ///
    /// Resolves a screen-space click at `pos` to exactly one of three outcomes:
    ///
    /// 1. **`DismissOverlay`** — the topmost open overlay did NOT contain `pos`.
    ///    The caller must close the identified overlay and return.
    ///
    /// 2. **`DispatchEvent`** — no overlay needs dismissing; a widget was hit.
    ///
    /// 3. **`Unhandled`** — no overlay needs dismissing and no widget was hit.
    pub fn handle_click(&mut self, pos: (f64, f64)) -> ClickOutcome {
        if let Some(overlay_id) = self.dismiss_topmost_at(pos) {
            let kind = self.overlay_kind_for(overlay_id.0.as_str());
            let handle = self.make_overlay_handle(overlay_id, kind);
            return ClickOutcome::DismissOverlay(handle);
        }

        let clicked = self.cur_branch_mut().ctx.input.process_click(pos.0, pos.1);
        match clicked {
            Some(id) => {
                let event = self.dispatch_widget_logged(&id);
                ClickOutcome::DispatchEvent(event)
            }
            None => ClickOutcome::Unhandled { pos },
        }
    }

    fn make_overlay_handle(
        &self,
        overlay_id: WidgetId,
        kind: Option<super::OverlayKind>,
    ) -> OverlayHandle {
        let slot = overlay_id.0.as_str();
        if let Some(reg) = self.cur_branch().composite_registry.iter().find(|r| r.slot_id == slot) {
            let wid = &reg.widget_id;
            return match reg.kind {
                CompositeKind::Modal => OverlayHandle::Modal(ModalHandle { id: wid.clone() }),
                CompositeKind::Popup => OverlayHandle::Popup(PopupHandle { id: wid.clone() }),
                CompositeKind::Dropdown => OverlayHandle::Dropdown(DropdownHandle { id: wid.clone() }),
                CompositeKind::ContextMenu => OverlayHandle::ContextMenu(ContextMenuHandle { id: wid.clone() }),
                _ => OverlayHandle::Other { overlay_id, kind },
            };
        }
        OverlayHandle::Other { overlay_id, kind }
    }

}

// =============================================================================
// PointerUpOutcome — per-frame snapshot returned by on_pointer_up
// =============================================================================

/// Per-frame snapshot returned by [`LayoutManager::on_pointer_up`].
#[derive(Debug, Clone)]
pub enum PointerUpOutcome {
    /// Pointer-up dismissed an overlay. Runtime should not treat it as a click.
    DismissedOverlay(super::handles::OverlayHandle),
    /// Pointer-up resolved to a widget. Runtime may call `App::on_*` hooks.
    Click(crate::types::WidgetId, super::DispatchEvent),
    /// Pointer-up landed somewhere unhandled (outside widgets/overlays).
    Unhandled,
}

/// Outcome of [`LayoutManager::handle_click`].
///
/// The application inspects this enum to decide what to do with a click.
#[derive(Debug, Clone)]
pub enum ClickOutcome {
    /// Click landed outside the topmost open overlay — the caller must close it.
    ///
    /// The typed [`OverlayHandle`] identifies which composite to close without
    /// string matching.  For multiple open overlays of the same kind (e.g. 7
    /// dropdowns), compare the `DropdownHandle` inside the variant.
    DismissOverlay(OverlayHandle),
    /// Click landed on a registered widget (overlay was not dismissed).
    DispatchEvent(super::DispatchEvent),
    /// No widget was hit and no overlay was dismissed.
    Unhandled { pos: (f64, f64) },
}

impl<P: DockPanel> LayoutManager<P> {
    // ------------------------------------------------------------------
    // L3 pointer bridge — called by L4 instead of touching ctx directly
    // ------------------------------------------------------------------

    /// Push a pointer-move event into the coordinator and update L3 hover state.
    pub fn on_pointer_move(&mut self, x: f64, y: f64) {
        let b = self.cur_branch_mut();
        b.ctx.input.set_cursor_pos(x, y);
        b.last_pointer_pos = Some((x, y));
        b.last_hovered = b.ctx.input.process_click(x, y)
            .or_else(|| b.ctx.input.process_hover(x, y));
    }

    pub fn on_pointer_down(&mut self, x: f64, y: f64) {
        let b = self.cur_branch_mut();
        b.ctx.input.set_cursor_pos(x, y);
        b.last_pointer_pos = Some((x, y));
        b.last_pressed = b.ctx.input.process_drag_press(x, y);
    }

    pub fn on_pointer_up(&mut self, x: f64, y: f64) -> PointerUpOutcome {
        let mut clicked_id: Option<String> = None;
        let window_key: Option<String> = self.current_window.as_ref().map(|k| k.as_str().to_owned());
        {
            let b = self.cur_branch_mut();
            b.ctx.input.set_cursor_pos(x, y);
            b.last_pointer_pos = Some((x, y));
            let hit = b.ctx.input.process_click(x, y);
            if let Some(ref id) = hit {
                b.last_click = Some((id.clone(), (x, y)));
                clicked_id = Some(id.as_str().to_owned());
            }
        }
        if let Some(id) = clicked_id {
            let ts = self.frame_time_ms;
            self.agent_log.push(
                ts,
                window_key,
                "lm.click",
                serde_json::json!({ "widget_id": id, "x": x, "y": y }),
            );
        }
        match self.handle_click((x, y)) {
            ClickOutcome::DismissOverlay(h) => PointerUpOutcome::DismissedOverlay(h),
            ClickOutcome::DispatchEvent(ev) => {
                let id = match &ev {
                    super::DispatchEvent::Unhandled(id) => id.clone(),
                    _ => crate::types::WidgetId::new(""),
                };
                PointerUpOutcome::Click(id, ev)
            }
            ClickOutcome::Unhandled { .. } => PointerUpOutcome::Unhandled,
        }
    }

    pub fn on_pointer_right_up(&mut self, x: f64, y: f64) {
        let b = self.cur_branch_mut();
        b.ctx.input.set_cursor_pos(x, y);
        b.last_pointer_pos = Some((x, y));
        let hit = b.ctx.input.process_right_click(x, y);
        if let Some(id) = hit {
            b.last_right_click = Some((id, (x, y)));
        }
    }

    pub fn on_scroll(&mut self, dx: f64, dy: f64) {
        let b = self.cur_branch_mut();
        b.last_scroll.0 += dx;
        b.last_scroll.1 += dy;
    }

    /// Handle a pointer-down event on the chrome strip.
    ///
    /// Runs `chrome_hit_test` + `handle_chrome_action`, then calls the
    /// appropriate `WindowHost` method. Returns `true` if consumed.
    pub fn handle_chrome_press<H: super::host::WindowHost + ?Sized>(
        &mut self,
        x: f64,
        y: f64,
        host: &mut H,
        time_ms: f64,
    ) -> bool {
        use crate::ui::widgets::composite::chrome::{
            chrome_hit_test, handle_chrome_action, ChromeAction, ChromeRenderKind,
            ChromeSettings, ChromeView,
        };
        use crate::ui::widgets::composite::chrome::types::{ChromeHit, ResizeCorner};
        use crate::platform::types::ResizeDirection;

        // Chrome zone hit-test.
        if let Some(chrome_rect) = self.rect_for_chrome() {
            let view = ChromeView {
                tabs: &[],
                active_tab_id: None,
                show_new_tab_btn: false,
                show_menu_btn: false,
                show_new_window_btn: true,
                show_close_window_btn: true,
                is_maximized: host.is_maximized(),
                cursor_x: x,
                cursor_y: y,
                time_ms,
            };
            let settings = ChromeSettings::default();
            let kind = ChromeRenderKind::Default;
            let hit = chrome_hit_test(
                self.chrome_state(), &view, &settings, &kind,
                chrome_rect, (x, y),
            );
            match handle_chrome_action(hit) {
                ChromeAction::WindowDragStart => {
                    host.drag_window();
                    return true;
                }
                ChromeAction::Minimize => {
                    host.set_minimized(true);
                    return true;
                }
                ChromeAction::MaximizeRestore => {
                    host.set_maximized(!host.is_maximized());
                    return true;
                }
                ChromeAction::CloseWindow => {
                    host.close_window();
                    return true;
                }
                ChromeAction::CloseApp => {
                    host.close_app();
                    return true;
                }
                ChromeAction::NewWindow => {
                    // L3 doesn't know App; host is responsible for acting on this.
                    // Signal via a dummy WindowSpec placeholder — host ignores spec
                    // if it uses on_chrome_new_window() from App directly.
                    // We do NOT push a spec here; just return true so L4 can intercept.
                    // The L4 manager's handle_chrome_press override calls app.on_chrome_new_window.
                    return false; // let L4 handle NewWindow via its own override
                }
                ChromeAction::BeginResize(h) => {
                    let dir = match h {
                        ChromeHit::ResizeTop    => Some(ResizeDirection::North),
                        ChromeHit::ResizeBottom => Some(ResizeDirection::South),
                        ChromeHit::ResizeLeft   => Some(ResizeDirection::West),
                        ChromeHit::ResizeRight  => Some(ResizeDirection::East),
                        ChromeHit::ResizeCorner(ResizeCorner::TopLeft)     => Some(ResizeDirection::NorthWest),
                        ChromeHit::ResizeCorner(ResizeCorner::TopRight)    => Some(ResizeDirection::NorthEast),
                        ChromeHit::ResizeCorner(ResizeCorner::BottomLeft)  => Some(ResizeDirection::SouthWest),
                        ChromeHit::ResizeCorner(ResizeCorner::BottomRight) => Some(ResizeDirection::SouthEast),
                        _ => None,
                    };
                    if let Some(d) = dir {
                        host.drag_resize_window(d);
                        return true;
                    }
                }
                _ => {}
            }
        }

        // Bezel-based edge-resize fallback for borderless windows.
        let win = self.last_window().unwrap_or_default();
        let bezel = 6.0_f64;
        if win.width > 0.0 && win.height > 0.0 {
            let on_left   = x >= win.x                      && x < win.x + bezel;
            let on_right  = x >= win.x + win.width  - bezel && x < win.x + win.width;
            let on_top    = y >= win.y                      && y < win.y + bezel;
            let on_bottom = y >= win.y + win.height - bezel && y < win.y + win.height;
            let dir = match (on_top, on_bottom, on_left, on_right) {
                (true,  _,    true,  _   ) => Some(ResizeDirection::NorthWest),
                (true,  _,    _,     true) => Some(ResizeDirection::NorthEast),
                (_,     true, true,  _   ) => Some(ResizeDirection::SouthWest),
                (_,     true, _,     true) => Some(ResizeDirection::SouthEast),
                (true,  _,    _,     _   ) => Some(ResizeDirection::North),
                (_,     true, _,     _   ) => Some(ResizeDirection::South),
                (_,     _,    true,  _   ) => Some(ResizeDirection::West),
                (_,     _,    _,     true) => Some(ResizeDirection::East),
                _ => None,
            };
            if let Some(d) = dir {
                host.drag_resize_window(d);
                return true;
            }
        }

        false
    }
}

impl<P: DockPanel> Default for LayoutManager<P> {
    fn default() -> Self {
        Self::new()
    }
}

