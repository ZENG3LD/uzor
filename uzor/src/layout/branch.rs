//! `WindowBranch<P>` — the per-window subtree owned by [`LayoutManager`].
//!
//! Each OS window the platform layer attaches becomes one `WindowBranch`
//! keyed by [`WindowKey`].  The branch holds *every* piece of state that
//! is logically per-window: the platform provider, chrome configuration,
//! edge panels, the per-window dock tree, the macro layout tree and its
//! solved rects, the retained-mode `ContextManager` (input coordinator +
//! retained widget tree), the click dispatch table, the per-frame
//! composite & dismiss registries, the overlay stack, pointer state, and
//! all persistent composite-instance state maps (modals, popups, …).
//!
//! State that is *Synced* across all windows — z-layer ordering, the
//! global style palette — lives on `LayoutManager` itself rather than
//! here.  See [`crate::layout::manager::LayoutManager`] for the root.

use std::collections::HashMap;

use crate::core::types::Rect;
use crate::app_context::{ContextManager, layout::types::LayoutNode};
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
use super::dock_state::DockState;
use super::overlay_stack::OverlayStack;
use super::tree::LayoutTree;
use super::types::LayoutSolved;
use super::window::WindowProvider;
use super::registry::{CompositeRegistration, DismissFrame};
use super::ClickDispatcher;
use crate::layout::docking::DockPanel;

/// Everything the layout manager needs to track for one OS window.
///
/// The provider is an opaque trait object so LM stays platform-agnostic.
/// Concrete kinds live in their crates (`uzor-window-desktop` for winit,
/// `uzor-window-web` for DOM canvas, …).
pub struct WindowBranch<P: DockPanel> {
    // ── platform handle ──────────────────────────────────────────────────
    /// Platform window handle wrapped in the trait LM talks to.
    pub provider: Box<dyn WindowProvider>,

    /// Cached logical rect captured at the last `solve_window`.
    pub rect: Rect,

    /// Has the runtime fired the per-window `App::init` hook yet?
    pub initialised: bool,

    /// Per-window tick counter — incremented every time the platform
    /// runtime runs a paint pass for this window.  `0` after attach
    /// means the window has never ticked (a strong "black-window"
    /// smell).  Mirrored into `BranchSnapshot.tick_count` so agents
    /// can sniff for stuck windows over the HTTP shim.
    pub tick_count: u64,

    /// Baseline repaint cadence — set by the platform layer from the
    /// resolved `WindowSpec::tick_rate` / `AppConfig::default_tick_rate`.
    pub tick_rate: crate::render::TickRate,

    // ── chrome / edges / dock subtree ────────────────────────────────────
    /// Per-window chrome strip configuration (visible/height/etc).
    pub chrome: ChromeSlot,

    /// Per-window edge panel registry (left/right/top/bottom toolbars +
    /// sidebars).
    pub edges: EdgePanels,

    /// Per-window docking state — tree, separators, panel rects, floating
    /// panels, drag state.  Each window has its own dock tree so panels
    /// torn off into another window keep their identity per-window.
    pub dock: DockState<P>,

    // ── solved layout ────────────────────────────────────────────────────
    /// Macro layout tree (chrome + edges + dock + overlay nodes) solved
    /// against the window rect each frame.
    pub tree: LayoutTree,

    /// Result of the most recent `solve_window`.
    pub last_solved: Option<LayoutSolved>,

    /// Window rect passed to the most recent `solve_window`.
    pub last_window: Option<Rect>,

    // ── input / dispatch / retained context ──────────────────────────────
    /// Retained-mode context manager (holds the input coordinator and the
    /// per-frame retained tree).  One per window so two windows don't
    /// share hover / focus / capture.
    pub ctx: ContextManager,

    /// Per-window click dispatch table — composites push patterns at
    /// register time; populated each frame by composite registration.
    pub dispatcher: ClickDispatcher,

    /// Per-frame composite registry — cleared at the top of each frame
    /// by `dispatcher_begin_frame`.
    pub composite_registry: Vec<CompositeRegistration>,

    /// Per-frame overlay dismiss registry.
    pub dismiss_frames: Vec<DismissFrame>,

    /// Overlay stack (modals, popups, dropdowns, context menus, tooltips).
    pub overlays: OverlayStack,

    // ── pointer state (one frame back) ───────────────────────────────────
    pub last_hovered:      Option<WidgetId>,
    pub last_click:        Option<(WidgetId, (f64, f64))>,
    pub last_right_click:  Option<(WidgetId, (f64, f64))>,
    pub last_pointer_pos:  Option<(f64, f64)>,
    pub last_scroll:       (f64, f64),
    pub last_pressed:      Option<WidgetId>,

    // ── persistent composite-instance state maps ─────────────────────────
    pub modals:        HashMap<WidgetId, ModalState>,
    pub popups:        HashMap<WidgetId, PopupState>,
    pub dropdowns:     HashMap<WidgetId, DropdownState>,
    pub toolbars:      HashMap<WidgetId, ToolbarState>,
    pub sidebars:      HashMap<WidgetId, SidebarState>,
    pub context_menus: HashMap<WidgetId, ContextMenuState>,
    pub chrome_widget_state: ChromeState,

    /// Map from a slot-bearing widget's `WidgetId` (panel / blackbox /
    /// future widgets that anchor to a layout slot) to the slot id
    /// (`lm::panel(slot_id, widget_id)` first arg).  Lets resize / drag
    /// dispatch resolve a widget hit back to the slot the layout
    /// manager actually knows about, without the caller having to make
    /// `widget_id == slot_id`.  Re-populated each frame the composite
    /// registers; entries for stale ids stay valid until overwritten.
    pub widget_to_slot: HashMap<WidgetId, String>,
}

impl<P: DockPanel> WindowBranch<P> {
    /// Create a fresh branch for a newly-attached window.
    pub fn new(provider: Box<dyn WindowProvider>, rect: Rect) -> Self {
        Self {
            provider,
            rect,
            initialised: false,
            tick_count: 0,
            tick_rate: crate::render::TickRate::Capped(60),
            chrome: ChromeSlot::default(),
            edges:  EdgePanels::new(),
            dock:   DockState::new(),
            tree:   LayoutTree::new(),
            last_solved: None,
            last_window: None,
            ctx: ContextManager::new(LayoutNode::new("__layout_root__")),
            dispatcher: ClickDispatcher::new(),
            composite_registry: Vec::new(),
            dismiss_frames: Vec::new(),
            overlays: OverlayStack::new(),
            last_hovered: None,
            last_click: None,
            last_right_click: None,
            last_pointer_pos: None,
            last_scroll: (0.0, 0.0),
            last_pressed: None,
            modals:        HashMap::new(),
            popups:        HashMap::new(),
            dropdowns:     HashMap::new(),
            toolbars:      HashMap::new(),
            sidebars:      HashMap::new(),
            context_menus: HashMap::new(),
            chrome_widget_state: ChromeState::default(),
            widget_to_slot: HashMap::new(),
        }
    }
}

/// Backwards-compat alias.  Old `WindowSlot<P>` was a thin holder; it is
/// now the fat [`WindowBranch<P>`].
pub type WindowSlot<P> = WindowBranch<P>;
