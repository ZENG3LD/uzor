use std::collections::HashMap;

use crate::docking::panels::{PanelDockingManager, PanelRect, DockPanel};
use crate::core::types::Rect;
use crate::app_context::{ContextManager, layout::types::LayoutNode};
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

// ---------------------------------------------------------------------------
// Per-frame composite registry — used by consume_event
// ---------------------------------------------------------------------------

/// The kind of a registered composite, used for event routing in
/// [`LayoutManager::consume_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeKind {
    Modal,
    Popup,
    Dropdown,
    Toolbar,
    Sidebar,
    ContextMenu,
    Chrome,
}

/// One entry in the per-frame composite registry.
///
/// Composites push one entry in their `register_layout_manager_*` helper.
/// [`LayoutManager::consume_event`] walks these in overlay-first order
/// (Modal → Popup → Dropdown → ContextMenu → Toolbar → Sidebar) to route
/// [`DispatchEvent`]s without the app spelling out the chain manually.
#[derive(Debug, Clone)]
pub struct CompositeRegistration {
    pub kind:       CompositeKind,
    /// The stable overlay or edge slot id (e.g. `"modal-overlay"`, `"top-toolbar"`).
    pub slot_id:    String,
    /// The widget id used when registering the composite with the coordinator
    /// (e.g. `"modal-widget"`, `"top-toolbar-widget"`).
    pub widget_id:  WidgetId,
    /// Frame rect of the composite (overlay rect or edge slot rect).
    pub frame_rect: Rect,
}

// ---------------------------------------------------------------------------
// Overlay dismiss registry
// ---------------------------------------------------------------------------

/// A single entry in the per-frame overlay dismiss registry.
///
/// Composites push one entry when they call their `register_layout_manager_*`
/// helper. The LayoutManager uses these entries to implement
/// [`LayoutManager::dismiss_topmost_at`].
#[derive(Clone)]
pub struct DismissFrame {
    /// Z-order priority. Higher = on top. The topmost open overlay is
    /// the one with the highest z value.  Ties are broken by insertion
    /// order: the most-recently-pushed entry wins.
    ///
    /// Recommended values:
    /// - 100 for context menu / popup
    /// - 50  for dropdown
    /// - 10  for modal
    pub z: u32,
    /// Screen-space rect of the overlay this frame.
    pub rect: Rect,
    /// Overlay slot id (e.g. `"modal-overlay"`, `"dd-file-overlay"`).
    ///
    /// This is the id the app matched in its dispatch block before the
    /// registry was introduced — the same strings still work, just with
    /// a single `match` in one place.
    pub overlay_id: WidgetId,
}

/// Map a composite `WidgetKind` to its coordinator `LayerId`, or `None` for atomics.
fn layer_for_widget_kind(kind: WidgetKind) -> Option<LayerId> {
    match kind {
        WidgetKind::Modal        => Some(LayerId::modal()),
        WidgetKind::ContextMenu  => Some(LayerId::new("context_menu")),
        WidgetKind::Popup        => Some(LayerId::popup()),
        WidgetKind::Dropdown     => Some(LayerId::new("dropdown")),
        WidgetKind::Tooltip      => Some(LayerId::tooltip()),
        WidgetKind::Chrome       => Some(LayerId::new("chrome")),
        _                        => None,
    }
}

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
    chrome: ChromeSlot,
    edges: EdgePanels,
    panels: PanelDockingManager<P>,
    overlays: OverlayStack,
    z_layers: ZLayerTable,
    tree: LayoutTree,
    last_solved: Option<LayoutSolved>,
    last_window: Option<Rect>,
    /// Retained-mode context manager — owned here so Level-3 composite
    /// helpers can access it directly via `layout.ctx_mut()`.
    ctx: ContextManager,
    /// Per-frame click dispatch table. Composites push patterns at register
    /// time; the app calls [`LayoutManager::dispatch_click`] when a hit
    /// resolves to a `WidgetId` and gets back a high-level
    /// [`super::DispatchEvent`].
    dispatcher: super::ClickDispatcher,
    /// Iterate every dock leaf and its solved screen-space rect.
    ///
    /// Use to drive per-leaf body rendering (`for (id, rect) in
    /// layout.dock_leaves()`) without reaching into `panels()`.
    /// (Pulled out of the impl block via inherent helpers below — this
    /// field is just the runtime-published frame timestamp.)
    pub(crate) frame_time_ms: f64,
    /// Per-frame overlay dismiss registry. Cleared by [`LayoutManager::begin_frame_widgets`].
    dismiss_frames: Vec<DismissFrame>,

    // -----------------------------------------------------------------------
    // Phase A — composite state ownership
    // -----------------------------------------------------------------------

    /// Persistent state for every modal instance keyed by widget id.
    pub(crate) modals: HashMap<WidgetId, ModalState>,
    /// Persistent state for every popup instance keyed by widget id.
    pub(crate) popups: HashMap<WidgetId, PopupState>,
    /// Persistent state for every dropdown instance keyed by widget id.
    pub(crate) dropdowns: HashMap<WidgetId, DropdownState>,
    /// Persistent state for every toolbar instance keyed by widget id.
    pub(crate) toolbars: HashMap<WidgetId, ToolbarState>,
    /// Persistent state for every sidebar instance keyed by widget id.
    pub(crate) sidebars: HashMap<WidgetId, SidebarState>,
    /// Persistent state for every context-menu instance keyed by widget id.
    pub(crate) context_menus: HashMap<WidgetId, ContextMenuState>,
    /// Single chrome state (there is at most one chrome per window).
    pub(crate) chrome_widget_state: ChromeState,

    /// Per-frame composite registry — cleared each frame by
    /// [`Self::dispatcher_begin_frame`] together with the dispatch table.
    /// Composites push one entry in their `register_layout_manager_*` helper
    /// so [`Self::consume_event`] can iterate them without app boilerplate.
    pub(crate) composite_registry: Vec<CompositeRegistration>,

    /// Centralised style/colour/size/texture registry.
    ///
    /// `lm::*` builders read from this when no per-call `Settings` were supplied.
    /// Per-call settings (`.settings(...)`) take priority — StyleManager is the
    /// *default supplier* (lowest priority).
    styles: StyleManager,

    // -----------------------------------------------------------------------
    // L3-owned input state — cleared each frame by inputs_begin_frame()
    // -----------------------------------------------------------------------

    /// The widget (if any) currently under the cursor — updated by on_pointer_move.
    last_hovered: Option<crate::types::WidgetId>,
    /// Left-click that landed this frame: (widget_id, position).
    last_click: Option<(crate::types::WidgetId, (f64, f64))>,
    /// Right-click that landed this frame: (widget_id, position).
    last_right_click: Option<(crate::types::WidgetId, (f64, f64))>,
    /// Last known pointer position.
    last_pointer_pos: Option<(f64, f64)>,
    /// Accumulated scroll delta this frame (reset each frame).
    last_scroll: (f64, f64),
    /// Widget hit by the most recent pointer-down (drag candidates).
    last_pressed: Option<crate::types::WidgetId>,
}

impl<P: DockPanel> LayoutManager<P> {
    /// Create a new `LayoutManager` with default chrome (32 px, visible) and
    /// empty edges, panels, and overlays.
    pub fn new() -> Self {
        Self {
            chrome: ChromeSlot::default(),
            edges: EdgePanels::new(),
            panels: PanelDockingManager::new(),
            overlays: OverlayStack::new(),
            z_layers: ZLayerTable::default(),
            tree: LayoutTree::new(),
            last_solved: None,
            last_window: None,
            ctx: ContextManager::new(LayoutNode::new("__layout_root__")),
            dispatcher: super::ClickDispatcher::new(),
            dismiss_frames: Vec::new(),
            frame_time_ms: 0.0,
            modals: HashMap::new(),
            popups: HashMap::new(),
            dropdowns: HashMap::new(),
            toolbars: HashMap::new(),
            sidebars: HashMap::new(),
            context_menus: HashMap::new(),
            chrome_widget_state: ChromeState::default(),
            composite_registry: Vec::new(),
            styles: StyleManager::default(),
            last_hovered: None,
            last_click: None,
            last_right_click: None,
            last_pointer_pos: None,
            last_scroll: (0.0, 0.0),
            last_pressed: None,
        }
    }

    // ------------------------------------------------------------------
    // System slots — uzor-internal
    // ------------------------------------------------------------------

    /// Read-only access to the chrome slot configuration.
    pub fn chrome(&self) -> &ChromeSlot {
        &self.chrome
    }

    /// Mutable access to the chrome slot configuration.
    pub fn chrome_mut(&mut self) -> &mut ChromeSlot {
        &mut self.chrome
    }

    /// Read-only access to the edge panel registry.
    pub fn edges(&self) -> &EdgePanels {
        &self.edges
    }

    /// Mutable access to the edge panel registry.
    pub fn edges_mut(&mut self) -> &mut EdgePanels {
        &mut self.edges
    }

    /// Read-only access to the overlay stack.
    pub fn overlays(&self) -> &OverlayStack {
        &self.overlays
    }

    /// Mutable access to the overlay stack.
    pub fn overlays_mut(&mut self) -> &mut OverlayStack {
        &mut self.overlays
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
        &self.ctx
    }

    /// Mutable access to the embedded `ContextManager`.
    ///
    /// Level-3 registration helpers call this internally to forward to
    /// `register_context_manager_*` without requiring the caller to hold a
    /// separate `ContextManager` reference.
    pub fn ctx_mut(&mut self) -> &mut ContextManager {
        &mut self.ctx
    }

    /// Read-only access to the click dispatch table.
    pub fn dispatcher(&self) -> &super::ClickDispatcher {
        &self.dispatcher
    }

    /// Default opening size for a sidebar kind, computed from the most
    /// recent viewport `LayoutManager` saw via `solve(window_rect)`.
    ///
    /// L/R sidebars get `frac * viewport.width`, T/B get `frac * viewport.height`.
    /// Returns `None` until the first `solve()` has been called.
    ///
    /// Callers use this to seed `EdgeSlot.thickness` and the first
    /// `SidebarState.width` so demo / spawned sidebars open at a sensible
    /// fraction (default 20%) instead of a hardcoded pixel constant.
    pub fn sidebar_default_size(&self, is_horizontal_kind: bool, frac: f64) -> Option<f64> {
        let win = self.last_window?;
        let axis = if is_horizontal_kind { win.width } else { win.height };
        Some(axis * frac)
    }

    /// Mutable access to the click dispatch table.
    ///
    /// Composites call this in their `register_layout_manager_*` helpers
    /// to install the patterns that translate raw `WidgetId` hits into
    /// semantic [`super::DispatchEvent`]s. The app may also register its
    /// own patterns to override a composite's default behaviour.
    pub fn dispatcher_mut(&mut self) -> &mut super::ClickDispatcher {
        &mut self.dispatcher
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
        let clicked = self.ctx.input.process_click(x, y)?;
        Some(
            self.dispatcher
                .dispatch(&clicked)
                .unwrap_or(super::DispatchEvent::Unhandled(clicked)),
        )
    }

    /// Translate a pre-resolved `WidgetId` into a high-level event.
    ///
    /// Use this when the click was already routed (e.g. by `WinitInputBridge`
    /// which calls `process_click` itself and returns `Option<WidgetId>`).
    /// Avoids running hit-test twice.
    pub fn dispatch_widget(&self, id: &crate::types::WidgetId) -> super::DispatchEvent {
        self.dispatcher
            .dispatch(id)
            .unwrap_or_else(|| super::DispatchEvent::Unhandled(id.clone()))
    }

    /// Clear the dispatch table, the overlay dismiss registry, and the
    /// per-frame composite registry.
    ///
    /// Composites re-register their patterns each frame, mirroring the
    /// immediate-mode model of widget registration. Call this once per
    /// frame before re-running composite registration.
    pub fn dispatcher_begin_frame(&mut self) {
        self.dispatcher.clear();
        self.dismiss_frames.clear();
        self.composite_registry.clear();
    }

    // -----------------------------------------------------------------------
    // Phase A+C — add_* factory methods (return typed handles)
    // -----------------------------------------------------------------------

    /// Register a modal by id and return a typed handle.
    ///
    /// Creates default state if not already present.  Call once at app init or
    /// the first time the modal is about to be shown.  Subsequent calls with
    /// the same id are idempotent — they return a handle to the existing state.
    pub fn add_modal(&mut self, id: &str) -> ModalHandle {
        let widget_id = WidgetId(id.to_owned());
        self.modals.entry(widget_id.clone()).or_default();
        ModalHandle { id: widget_id }
    }

    /// Register a popup and return a typed handle.
    pub fn add_popup(&mut self, id: &str) -> PopupHandle {
        let widget_id = WidgetId(id.to_owned());
        self.popups.entry(widget_id.clone()).or_default();
        PopupHandle { id: widget_id }
    }

    /// Register a dropdown and return a typed handle.
    pub fn add_dropdown(&mut self, id: &str) -> DropdownHandle {
        let widget_id = WidgetId(id.to_owned());
        self.dropdowns.entry(widget_id.clone()).or_default();
        DropdownHandle { id: widget_id }
    }

    /// Register a toolbar and return a typed handle.
    pub fn add_toolbar(&mut self, id: &str) -> ToolbarHandle {
        let widget_id = WidgetId(id.to_owned());
        self.toolbars.entry(widget_id.clone()).or_default();
        ToolbarHandle { id: widget_id }
    }

    /// Register a sidebar and return a typed handle.
    pub fn add_sidebar(&mut self, id: &str) -> SidebarHandle {
        let widget_id = WidgetId(id.to_owned());
        self.sidebars.entry(widget_id.clone()).or_default();
        SidebarHandle { id: widget_id }
    }

    /// Register a context menu and return a typed handle.
    pub fn add_context_menu(&mut self, id: &str) -> ContextMenuHandle {
        let widget_id = WidgetId(id.to_owned());
        self.context_menus.entry(widget_id.clone()).or_default();
        ContextMenuHandle { id: widget_id }
    }

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
        self.modals.get(&h.id)
            .expect("modal handle invalidated — state dropped from registry")
    }

    /// Mutable access to a modal's state via its typed handle.
    pub fn modal_mut(&mut self, h: &ModalHandle) -> &mut ModalState {
        self.modals.get_mut(&h.id)
            .expect("modal handle invalidated — state dropped from registry")
    }

    /// Read-only access to a popup's state via its typed handle.
    pub fn popup(&self, h: &PopupHandle) -> &PopupState {
        self.popups.get(&h.id)
            .expect("popup handle invalidated — state dropped from registry")
    }

    /// Mutable access to a popup's state via its typed handle.
    pub fn popup_mut(&mut self, h: &PopupHandle) -> &mut PopupState {
        self.popups.get_mut(&h.id)
            .expect("popup handle invalidated — state dropped from registry")
    }

    /// Read-only access to a dropdown's state via its typed handle.
    pub fn dropdown(&self, h: &DropdownHandle) -> &DropdownState {
        self.dropdowns.get(&h.id)
            .expect("dropdown handle invalidated — state dropped from registry")
    }

    /// Mutable access to a dropdown's state via its typed handle.
    pub fn dropdown_mut(&mut self, h: &DropdownHandle) -> &mut DropdownState {
        self.dropdowns.get_mut(&h.id)
            .expect("dropdown handle invalidated — state dropped from registry")
    }

    /// Read-only access to a toolbar's state via its typed handle.
    pub fn toolbar(&self, h: &ToolbarHandle) -> &ToolbarState {
        self.toolbars.get(&h.id)
            .expect("toolbar handle invalidated — state dropped from registry")
    }

    /// Mutable access to a toolbar's state via its typed handle.
    pub fn toolbar_mut(&mut self, h: &ToolbarHandle) -> &mut ToolbarState {
        self.toolbars.get_mut(&h.id)
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

    /// Cursor position in screen coordinates, if the pointer is inside
    /// the window.  Forwarded from the input coordinator for builder
    /// convenience (`chrome` reads it for tooltip routing).
    pub fn cursor_pos(&self) -> Option<(f64, f64)> {
        self.ctx.input.pointer_pos()
    }

    // -----------------------------------------------------------------------
    // L3-owned input accessors — read from fields maintained by on_pointer_*
    // -----------------------------------------------------------------------

    /// Widget currently under the cursor (updated on every pointer-move).
    ///
    /// This is the single authoritative hover source for all of L3+.
    /// Composite helpers (`chrome`, `dropdown`, etc.) must read from this
    /// instead of reaching into `coord.hovered_widget()` directly.
    pub fn hovered_widget(&self) -> Option<&crate::types::WidgetId> {
        self.last_hovered.as_ref()
    }

    /// Was the given widget pressed (pointer-down) this frame?
    pub fn was_pressed(&self, id: &WidgetId) -> bool {
        self.last_pressed.as_ref() == Some(id)
    }

    /// Last known pointer position (persists across frames).
    pub fn pointer_pos(&self) -> Option<(f64, f64)> {
        self.last_pointer_pos
    }

    /// Accumulated scroll delta this frame (reset by `inputs_begin_frame`).
    pub fn scroll_delta(&self) -> (f64, f64) {
        self.last_scroll
    }

    /// Clear one-shot input fields.  Call once per frame BEFORE widget registration.
    ///
    /// Clears `last_click`, `last_right_click`, `last_pressed`, `last_scroll`.
    /// Does NOT clear `last_hovered` or `last_pointer_pos` — those persist
    /// between events so hover is always current.
    pub fn inputs_begin_frame(&mut self) {
        self.last_click       = None;
        self.last_right_click = None;
        self.last_pressed     = None;
        self.last_scroll      = (0.0, 0.0);
    }

    /// Full per-frame setup: clear one-shot input flags, then call
    /// `ctx.begin_frame_widgets_only` (which clears widget registrations
    /// but preserves pointer state).
    ///
    /// Call this once per frame BEFORE calling `App::ui`.  Replaces the
    /// old `ctx.begin_frame(input_snapshot, viewport)` pattern; pointer
    /// state is now pushed incrementally via `on_pointer_*`.
    ///
    /// `time_ms`  — milliseconds since runtime start (for animations).
    /// `viewport` — the content area rect (used for layout re-compute).
    pub fn begin_frame(&mut self, time_ms: f64, viewport: crate::core::types::Rect) {
        // NOTE: do NOT call inputs_begin_frame() here — it would erase a
        // click that arrived between the previous tick's end_frame and this
        // tick's begin_frame.  L4 is responsible for calling
        // `inputs_end_frame()` after `App::ui` returns (or use
        // `LayoutManager::end_frame` shortcut).
        self.ctx.begin_frame_widgets_only(time_ms / 1000.0, viewport);
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
        self.last_click.as_ref().map_or(false, |(c, _)| c == id)
    }

    /// Iterate every dock leaf and its solved screen-space rect.
    ///
    /// Use to drive per-leaf body rendering (`for (id, rect) in
    /// layout.dock_leaves()`) without reaching into `panels()`.
    pub fn dock_leaves(&self) -> impl Iterator<Item = (crate::docking::panels::LeafId, Rect)> + '_ {
        self.panels.panel_rects().iter().map(|(&id, &pr)| {
            (id, panel_rect_to_rect(pr))
        })
    }

    /// Read-only access to a sidebar's state via its typed handle.
    pub fn sidebar(&self, h: &SidebarHandle) -> &SidebarState {
        self.sidebars.get(&h.id)
            .expect("sidebar handle invalidated — state dropped from registry")
    }

    /// Mutable access to a sidebar's state via its typed handle.
    pub fn sidebar_mut(&mut self, h: &SidebarHandle) -> &mut SidebarState {
        self.sidebars.get_mut(&h.id)
            .expect("sidebar handle invalidated — state dropped from registry")
    }

    /// Read-only access to a context menu's state via its typed handle.
    pub fn context_menu(&self, h: &ContextMenuHandle) -> &ContextMenuState {
        self.context_menus.get(&h.id)
            .expect("context_menu handle invalidated — state dropped from registry")
    }

    /// Mutable access to a context menu's state via its typed handle.
    pub fn context_menu_mut(&mut self, h: &ContextMenuHandle) -> &mut ContextMenuState {
        self.context_menus.get_mut(&h.id)
            .expect("context_menu handle invalidated — state dropped from registry")
    }

    /// Read-only access to the chrome widget state.
    pub fn chrome_state(&self) -> &ChromeState {
        &self.chrome_widget_state
    }

    /// Mutable access to the chrome widget state.
    pub fn chrome_state_mut(&mut self) -> &mut ChromeState {
        &mut self.chrome_widget_state
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
        self.composite_registry.push(reg);
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

        // Snapshot the registry so we can iterate it without holding a borrow
        // on `self` while we also borrow the individual state maps.
        let registry: Vec<CompositeRegistration> = self.composite_registry.clone();

        let mut opt_ev: Option<DispatchEvent> = Some(event);

        // Overlay-first order: Modal → Popup → Dropdown → ContextMenu → Toolbar → Sidebar
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
                        let mut st = self.modals.remove(&reg.widget_id).unwrap_or_default();
                        let result = modal_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            modal_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.modals.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Popup => {
                        let mut st = self.popups.remove(&reg.widget_id).unwrap_or_default();
                        let result = popup_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            popup_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.popups.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Dropdown => {
                        let mut st = self.dropdowns.remove(&reg.widget_id).unwrap_or_default();
                        let result = dropdown_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            dropdown_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.dropdowns.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::ContextMenu => {
                        // ContextMenu has no consume_event — pass through.
                        Some(ev)
                    }
                    CompositeKind::Toolbar => {
                        let mut st = self.toolbars.remove(&reg.widget_id).unwrap_or_default();
                        let result = toolbar_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            toolbar_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.toolbars.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Sidebar => {
                        let mut st = self.sidebars.remove(&reg.widget_id).unwrap_or_default();
                        let result = sidebar_input::consume_event(
                            ev, &mut st, &reg.widget_id,
                            sidebar_input::ConsumeEventCtx { cursor, frame_rect: reg.frame_rect, viewport },
                        );
                        self.sidebars.insert(reg.widget_id.clone(), st);
                        result
                    }
                    CompositeKind::Chrome => Some(ev),
                };
            }
        }

        opt_ev
    }

    /// Read-only access to the macro layout tree (solved node rects).
    pub fn tree(&self) -> &LayoutTree {
        &self.tree
    }

    /// Mutable access to the macro layout tree.
    ///
    /// L3 registration helpers call this to insert widget nodes each frame.
    pub fn tree_mut(&mut self) -> &mut LayoutTree {
        &mut self.tree
    }

    /// Clear all per-frame widget nodes from the tree and the overlay dismiss registry.
    ///
    /// Call at the start of each frame, before any L3 widget registration.
    /// System nodes (chrome, edges, dock, overlay) are preserved.
    pub fn begin_frame_widgets(&mut self) {
        self.tree.clear_widgets();
        self.dismiss_frames.clear();
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
        self.dismiss_frames.push(frame);
    }

    /// Return the `OverlayKind` for a given overlay slot id, if it is registered.
    pub fn overlay_kind_for(&self, overlay_id: &str) -> Option<super::OverlayKind> {
        self.overlays.get(overlay_id).map(|e| e.kind)
    }

    /// Resolve which overlay (if any) should close when a click lands at `pos`.
    ///
    /// Walks the registered frames in z-order (top first). The first frame
    /// whose rect does **not** contain `pos` is the topmost overlay being
    /// clicked outside — return its `overlay_id`.
    ///
    /// Returns `None` when:
    /// - No overlay frames are registered (no overlay is open).
    /// - `pos` is inside the topmost overlay (the click belongs to that
    ///   overlay's own widgets — let the normal dispatch chain handle it).
    ///
    /// Behaviour:
    /// 1. Find the entry with the highest `z` value (most-recently-pushed
    ///    wins ties — i.e. last-registered beats earlier at the same z).
    /// 2. If `pos` is inside that entry's rect → click is inside the topmost
    ///    overlay → return `None` (let dispatch chain handle).
    /// 3. If `pos` is outside → return `Some(overlay_id)` for that entry.
    ///    Only ONE overlay closes per click — callers must `return` after
    ///    acting on the result.
    pub fn dismiss_topmost_at(&self, pos: (f64, f64)) -> Option<WidgetId> {
        if self.dismiss_frames.is_empty() {
            return None;
        }
        // Find the topmost frame: highest z, with ties broken by last push order.
        // We iterate in reverse to give last-registered priority at equal z.
        let topmost = self
            .dismiss_frames
            .iter()
            .enumerate()
            .rev()
            .max_by_key(|(i, f)| (f.z, *i))?
            .1;

        if topmost.rect.contains(pos.0, pos.1) {
            // Click is inside the topmost overlay — not an outside-click.
            None
        } else {
            Some(topmost.overlay_id.clone())
        }
    }

    /// Compute the effective `LayerId` for a node based on its parent chain.
    ///
    /// Walks ancestors from root down. The deepest ancestor with a
    /// layer-determining kind wins. Falls back to `LayerId::main()`.
    pub fn compute_layer_for(&self, node_id: LayoutNodeId) -> LayerId {
        let chain = self.tree.parent_chain(node_id);
        let mut effective = LayerId::main();
        for ancestor_id in &chain {
            if let Some(entry) = self.tree.entry(*ancestor_id) {
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
    // User-facing dock + floating panels
    // ------------------------------------------------------------------

    /// Read-only access to the panel docking manager.
    pub fn panels(&self) -> &PanelDockingManager<P> {
        &self.panels
    }

    /// Mutable access to the panel docking manager.
    ///
    /// App developers use this to add/remove panels, perform drag operations,
    /// and query panel rects.
    pub fn panels_mut(&mut self) -> &mut PanelDockingManager<P> {
        &mut self.panels
    }

    // ------------------------------------------------------------------
    // Per-frame solve
    // ------------------------------------------------------------------

    /// Recompute all macro-level rects given the current window size.
    ///
    /// Must be called each frame or on resize. Drives the dock layout pass
    /// internally. Returns a reference to the freshly computed `LayoutSolved`.
    pub fn solve(&mut self, window: Rect) -> &LayoutSolved {
        let solved = solve_layout(window, &self.chrome, &self.edges, &mut self.tree);

        // Drive the dock layout pass with the computed dock area.
        let dock_pr = panel_rect_from_rect(solved.dock_area);
        self.panels.layout(dock_pr);

        self.last_solved = Some(solved);
        self.last_window = Some(window);

        self.last_solved.as_ref()
            .expect("last_solved is Some — we just assigned it")
    }

    /// The result of the most recent `solve` call, or `None` if never solved.
    pub fn last_solved(&self) -> Option<&LayoutSolved> {
        self.last_solved.as_ref()
    }

    /// The window rect passed to the most recent `solve` call.
    pub fn last_window(&self) -> Option<Rect> {
        self.last_window
    }

    // ------------------------------------------------------------------
    // Rect accessors
    // ------------------------------------------------------------------

    /// Rect of the chrome strip, or `None` if not yet solved or chrome hidden.
    pub fn rect_for_chrome(&self) -> Option<Rect> {
        self.last_solved.as_ref().and_then(|s| s.chrome)
    }

    /// Dock content area, or `None` if not yet solved.
    pub fn rect_for_dock_area(&self) -> Option<Rect> {
        self.last_solved.as_ref().map(|s| s.dock_area)
    }

    /// Floating panel area (same as dock area, z-above dock), or `None` if not yet solved.
    pub fn rect_for_floating_area(&self) -> Option<Rect> {
        self.last_solved.as_ref().map(|s| s.floating_area)
    }

    /// Rect for a named overlay entry, or `None` if not present.
    pub fn rect_for_overlay(&self, id: &str) -> Option<Rect> {
        self.overlays.get(id).map(|e| e.rect)
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
        let solved = self.last_solved.as_ref()?;
        let slot = self.edges.get(id)?;

        // Find the index of this slot among visible slots on the same side
        // (order matches the per-side Vec in `solved.edges`).
        use super::types::EdgeSide;
        let visible: Vec<_> = self.edges.slots_for(slot.side).collect();
        let idx = visible.iter().position(|s| s.id == id)?;

        let rects = match slot.side {
            EdgeSide::Top    => &solved.edges.top,
            EdgeSide::Bottom => &solved.edges.bottom,
            EdgeSide::Left   => &solved.edges.left,
            EdgeSide::Right  => &solved.edges.right,
        };
        rects.get(idx).copied()
    }

    /// Resolve a slot id to a rect by checking each layer in order:
    ///
    /// 1. `"chrome"` → chrome strip rect.
    /// 2. Edge slot id → edge slot rect via `rect_for_edge_slot`.
    /// 3. Overlay id → overlay rect via `rect_for_overlay`.
    /// 4. Dock leaf id (string form of `LeafId` display, e.g. `"Leaf(42)"`) →
    ///    dock panel rect via `panels.rect_for_leaf_str`.
    /// 5. Otherwise `None`.
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
        if let Some(pr) = self.panels.rect_for_leaf_str(slot_id) {
            return Some(panel_rect_to_rect(pr));
        }
        None
    }

    // ------------------------------------------------------------------
    // Overlay helpers
    // ------------------------------------------------------------------

    /// Push an overlay entry onto the stack, replacing any existing entry with the same id.
    pub fn push_overlay(&mut self, entry: OverlayEntry) {
        self.overlays.push(entry);
    }

    /// Remove all overlay entries from the stack.
    pub fn clear_overlays(&mut self) {
        self.overlays.clear();
    }

    /// Return overlay entries sorted ascending by z (lowest z first — topmost drawn last).
    ///
    /// Sorts the internal stack in-place before returning the slice.
    pub fn overlays_in_draw_order(&mut self) -> &[OverlayEntry] {
        self.overlays.sort_by_z(&self.z_layers);
        self.overlays.entries()
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
        use crate::docking::panels::SeparatorOrientation as SepOrient;
        let sep_rects: Vec<(usize, Rect)> = self
            .panels
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

        let coord = &mut self.ctx.input;
        for (i, rect) in &sep_rects {
            coord.register_atomic(
                WidgetId::new(format!("dock-sep-{i}")),
                WidgetKind::DragHandle,
                *rect,
                crate::input::Sense::DRAG | crate::input::Sense::CLICK,
                layer,
            );
        }

        self.dispatcher.on_prefix(
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
            // Build a typed OverlayHandle from the overlay slot id.
            let handle = self.make_overlay_handle(overlay_id, kind);
            return ClickOutcome::DismissOverlay(handle);
        }

        let clicked = self.ctx.input.process_click(pos.0, pos.1);
        match clicked {
            Some(id) => {
                let event = self.dispatch_widget(&id);
                ClickOutcome::DispatchEvent(event)
            }
            None => ClickOutcome::Unhandled { pos },
        }
    }

    /// Build a typed [`OverlayHandle`] from a raw overlay slot id + kind.
    ///
    /// Matches the slot id against the registered state maps to produce the
    /// specific typed variant.  Falls back to `OverlayHandle::Other` for
    /// unknown ids (e.g. chrome, tooltips).
    fn make_overlay_handle(
        &self,
        overlay_id: WidgetId,
        kind: Option<super::OverlayKind>,
    ) -> OverlayHandle {
        // The overlay slot id is like "modal-overlay", "dd-file-overlay", etc.
        // We need to find the widget_id that registered under that slot.
        // The DismissFrame carries the slot_id; we correlate through overlay stack.
        // Strategy: walk state maps and find the first entry whose key WidgetId
        // matches a known pattern or walk composite_registry for the slot.
        // Simplest: scan composite_registry (populated each frame) for slot_id match.
        let slot = overlay_id.0.as_str();
        if let Some(reg) = self.composite_registry.iter().find(|r| r.slot_id == slot) {
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
        self.ctx.input.set_cursor_pos(x, y);
        self.last_pointer_pos = Some((x, y));
        // Hit-test for hover (reuses process_click which finds topmost click-sense widget).
        self.last_hovered = self.ctx.input.process_click(x, y);
    }

    /// Push a pointer-down event and record the pressed widget.
    pub fn on_pointer_down(&mut self, x: f64, y: f64) {
        self.ctx.input.set_cursor_pos(x, y);
        self.last_pointer_pos = Some((x, y));
        self.last_pressed = self.ctx.input.process_drag_press(x, y);
    }

    /// Push a pointer-up event, run hit-test + dispatch, and return the outcome.
    ///
    /// Also records the click in `last_click` so `was_clicked` works correctly
    /// regardless of when `begin_frame` is called.
    pub fn on_pointer_up(&mut self, x: f64, y: f64) -> PointerUpOutcome {
        self.ctx.input.set_cursor_pos(x, y);
        self.last_pointer_pos = Some((x, y));
        // Record click before handle_click (which may clear state internally).
        let hit = self.ctx.input.process_click(x, y);
        if let Some(ref id) = hit {
            self.last_click = Some((id.clone(), (x, y)));
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

    /// Push a right pointer-up event and record the right-click.
    pub fn on_pointer_right_up(&mut self, x: f64, y: f64) {
        self.ctx.input.set_cursor_pos(x, y);
        self.last_pointer_pos = Some((x, y));
        let hit = self.ctx.input.process_right_click(x, y);
        if let Some(id) = hit {
            self.last_right_click = Some((id, (x, y)));
        }
    }

    /// Push a scroll delta for this frame.  Accumulates until `inputs_begin_frame`.
    pub fn on_scroll(&mut self, dx: f64, dy: f64) {
        self.last_scroll.0 += dx;
        self.last_scroll.1 += dy;
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{EdgeSlot, EdgeSide};

    // Minimal DockPanel impl for generic tests.
    #[derive(Clone, Debug)]
    struct DummyPanel;

    impl DockPanel for DummyPanel {
        fn title(&self) -> &str { "dummy" }
        fn type_id(&self) -> &'static str { "dummy" }
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, w, h)
    }

    // ------------------------------------------------------------------
    // solve tests
    // ------------------------------------------------------------------

    #[test]
    fn solve_chrome_only() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        // Default chrome is 32 px visible.
        let solved = lm.solve(rect(0.0, 0.0, 1920.0, 1080.0));

        let chrome = solved.chrome.expect("chrome should be Some");
        assert_eq!(chrome.y, 0.0);
        assert_eq!(chrome.height, 32.0);

        let dock = solved.dock_area;
        assert_eq!(dock.y, 32.0);
        assert_eq!(dock.height, 1048.0);
        assert_eq!(dock.width, 1920.0);
    }

    #[test]
    fn solve_with_edges() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        // Chrome 32 px (default).
        lm.edges_mut().add(EdgeSlot {
            id: "toolbar".to_string(),
            side: EdgeSide::Top,
            thickness: 40.0,
            visible: true,
            order: 0,
            ..Default::default()
        });
        lm.edges_mut().add(EdgeSlot {
            id: "sidebar".to_string(),
            side: EdgeSide::Left,
            thickness: 200.0,
            visible: true,
            order: 0,
            ..Default::default()
        });

        let solved = lm.solve(rect(0.0, 0.0, 1920.0, 1080.0));

        // chrome 32 + toolbar 40 = 72 consumed from top
        // sidebar 200 consumed from left
        let dock = solved.dock_area;
        assert_eq!(dock.x, 200.0, "dock starts after sidebar");
        assert_eq!(dock.y, 72.0,  "dock starts after chrome+toolbar");
        assert_eq!(dock.width,  1720.0);
        assert_eq!(dock.height, 1008.0);
    }

    #[test]
    fn solve_chrome_hidden() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        lm.chrome_mut().visible = false;

        let solved = lm.solve(rect(0.0, 0.0, 800.0, 600.0));
        assert!(solved.chrome.is_none(), "hidden chrome yields None");
        assert_eq!(solved.dock_area.y, 0.0);
        assert_eq!(solved.dock_area.height, 600.0);
    }

    // ------------------------------------------------------------------
    // z-table tests
    // ------------------------------------------------------------------

    #[test]
    fn z_table_default_ordering() {
        let lm = LayoutManager::<DummyPanel>::new();
        // modal(4) < context_menu(5) < tooltip(7)
        assert!(lm.z_for(OverlayKind::Modal) < lm.z_for(OverlayKind::ContextMenu));
        assert!(lm.z_for(OverlayKind::ContextMenu) < lm.z_for(OverlayKind::Tooltip));
    }

    #[test]
    fn z_table_override() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        lm.z_layers_mut().set(OverlayKind::Modal, 10);
        assert_eq!(lm.z_for(OverlayKind::Modal), 10);
    }

    // ------------------------------------------------------------------
    // Overlay tests
    // ------------------------------------------------------------------

    #[test]
    fn overlay_sort_by_z() {
        use super::super::overlay_stack::OverlayEntry;

        let mut lm = LayoutManager::<DummyPanel>::new();
        // push in reverse z order: tooltip(7), dropdown(2), modal(4)
        lm.push_overlay(OverlayEntry { id: "tip".to_string(), kind: OverlayKind::Tooltip,  rect: rect(0.0,0.0,1.0,1.0), anchor: None });
        lm.push_overlay(OverlayEntry { id: "dd".to_string(),  kind: OverlayKind::Dropdown, rect: rect(0.0,0.0,1.0,1.0), anchor: None });
        lm.push_overlay(OverlayEntry { id: "m".to_string(),   kind: OverlayKind::Modal,    rect: rect(0.0,0.0,1.0,1.0), anchor: None });

        let ordered = lm.overlays_in_draw_order();
        // ascending z: dropdown(2), modal(4), tooltip(7)
        assert_eq!(ordered[0].id, "dd");
        assert_eq!(ordered[1].id, "m");
        assert_eq!(ordered[2].id, "tip");
    }

    // ------------------------------------------------------------------
    // Panels accessor test
    // ------------------------------------------------------------------

    #[test]
    fn panels_accessor_compiles() {
        let mut lm = LayoutManager::<DummyPanel>::new();
        let _panels: &mut PanelDockingManager<DummyPanel> = lm.panels_mut();
    }
}
