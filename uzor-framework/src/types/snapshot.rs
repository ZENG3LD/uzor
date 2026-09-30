//! The published visual snapshot: a consistent, `Send + Sync` picture of all
//! on-screen state as of the last publish phase.
//!
//! Produced by the kernel's publish phase only when some engine revision
//! moved; read lock-free through the handle by tools, tests and observers
//! outside the tick (at most one tick old). Hooks inside the tick read a live
//! borrowed view instead. The snapshot holds identities only, never the app's
//! panel values.

use std::fmt;

use uzor::input::cursor::CursorIcon;
use uzor::layout::docking::{FloatingWindowId, LeafId, SeparatorOrientation};
use uzor::layout::window::WindowKey;
use uzor::layout::{EdgeSide, OverlayKind, SlotId};
use uzor::render::TickRate;
use uzor::{Rect, WidgetId};

use crate::types::bus::RenderInfo;
use crate::types::frame::RegionSpec;
use crate::types::ids::{OverlaySlot, Revision, Seconds, WindowId};
use crate::types::spec::Spec;
use crate::types::window::WindowGeometry;

/// All on-screen state at one revision.
pub struct VisualSnapshot<S: Spec> {
    /// Monotonic; bumped iff any engine revision moved.
    pub revision: Revision,
    /// Host clock of the tick that published it.
    pub at: Seconds,
    /// One view per window, ordered by `WindowId`.
    pub windows: Vec<WindowView<S>>,
    /// Render facts echoed from the host.
    pub render: RenderInfo,
    /// Revision of the theme tokens.
    pub theme_rev: Revision,
}

impl<S: Spec> VisualSnapshot<S> {
    /// The snapshot before the first publish: no windows, revision zero.
    pub fn empty() -> Self {
        Self {
            revision: Revision::ZERO,
            at: Seconds::ZERO,
            windows: Vec::new(),
            render: RenderInfo::default(),
            theme_rev: Revision::ZERO,
        }
    }

    /// The view of one window, if it exists.
    pub fn window(&self, id: WindowId) -> Option<&WindowView<S>> {
        self.windows.iter().find(|w| w.id == id)
    }
}

/// On-screen state of one window.
pub struct WindowView<S: Spec> {
    /// The window.
    pub id: WindowId,
    /// Its app label.
    pub key: WindowKey,
    /// OS-facing geometry and flags.
    pub geometry: WindowGeometry,
    /// OS keyboard focus.
    pub focused: bool,
    /// Current cursor icon.
    pub cursor: CursorIcon,
    /// Chrome strip.
    pub chrome: ChromeView,
    /// Edge slots.
    pub edges: Vec<EdgeSlotView>,
    /// Dock tree.
    pub dock: DockView,
    /// Overlay stack, bottom to top.
    pub overlays: Vec<OverlayView<S::Overlay>>,
    /// Hover / press / focus / capture.
    pub input: InputView,
    /// Tick rate and regions.
    pub cadence: CadenceView,
}

// Manual impls: a derive would demand the bounds of the marker type `S`
// itself; only its associated types need them (given by `Spec`).
impl<S: Spec> Clone for VisualSnapshot<S> {
    fn clone(&self) -> Self {
        Self {
            revision: self.revision,
            at: self.at,
            windows: self.windows.clone(),
            render: self.render.clone(),
            theme_rev: self.theme_rev,
        }
    }
}

impl<S: Spec> fmt::Debug for VisualSnapshot<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VisualSnapshot")
            .field("revision", &self.revision)
            .field("at", &self.at)
            .field("windows", &self.windows)
            .field("render", &self.render)
            .field("theme_rev", &self.theme_rev)
            .finish()
    }
}

impl<S: Spec> PartialEq for VisualSnapshot<S> {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision
            && self.at == other.at
            && self.windows == other.windows
            && self.render == other.render
            && self.theme_rev == other.theme_rev
    }
}

impl<S: Spec> Clone for WindowView<S> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            key: self.key.clone(),
            geometry: self.geometry,
            focused: self.focused,
            cursor: self.cursor,
            chrome: self.chrome,
            edges: self.edges.clone(),
            dock: self.dock.clone(),
            overlays: self.overlays.clone(),
            input: self.input.clone(),
            cadence: self.cadence.clone(),
        }
    }
}

impl<S: Spec> fmt::Debug for WindowView<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowView")
            .field("id", &self.id)
            .field("key", &self.key)
            .field("geometry", &self.geometry)
            .field("focused", &self.focused)
            .field("cursor", &self.cursor)
            .field("chrome", &self.chrome)
            .field("edges", &self.edges)
            .field("dock", &self.dock)
            .field("overlays", &self.overlays)
            .field("input", &self.input)
            .field("cadence", &self.cadence)
            .finish()
    }
}

impl<S: Spec> PartialEq for WindowView<S> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.key == other.key
            && self.geometry == other.geometry
            && self.focused == other.focused
            && self.cursor == other.cursor
            && self.chrome == other.chrome
            && self.edges == other.edges
            && self.dock == other.dock
            && self.overlays == other.overlays
            && self.input == other.input
            && self.cadence == other.cadence
    }
}

/// The chrome strip of a window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChromeView {
    /// Chrome shown.
    pub visible: bool,
    /// Solved strip rect, window-local logical pixels.
    pub rect: Rect,
}

/// One edge slot of a window.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeSlotView {
    /// Slot id.
    pub id: SlotId,
    /// Which edge.
    pub side: EdgeSide,
    /// Slot shown.
    pub visible: bool,
    /// Solved rect, window-local logical pixels.
    pub rect: Rect,
}

/// The dock tree of a window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DockView {
    /// Bumps on tree structure or ratio change; tells the app when a stored
    /// layout blob is stale.
    pub dock_rev: Revision,
    /// Visible and hidden leaves.
    pub leaves: Vec<LeafView>,
    /// Splitters between dock children.
    pub separators: Vec<SeparatorView>,
    /// In-window floating windows.
    pub floating: Vec<FloatingView>,
    /// The leaf with the active panel, if any.
    pub active_leaf: Option<LeafId>,
}

/// One dock leaf (a tabbed panel container).
#[derive(Clone, Debug, PartialEq)]
pub struct LeafView {
    /// The leaf.
    pub leaf: LeafId,
    /// Solved rect, window-local logical pixels.
    pub rect: Rect,
    /// Its tabs, in order.
    pub panels: Vec<PanelRef>,
    /// Index of the active tab.
    pub active_tab: usize,
    /// Leaf is collapsed / not shown.
    pub hidden: bool,
}

/// Identity of one panel; the app owns the panel value itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelRef {
    /// Tab index in its leaf.
    pub index: usize,
    /// The panel's `DockPanel::type_id`.
    pub type_id: &'static str,
    /// The panel's title at publish time.
    pub title: String,
}

/// One splitter between dock children.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeparatorView {
    /// Separator index (as in a layout hit).
    pub index: usize,
    /// Solved rect, window-local logical pixels.
    pub rect: Rect,
    /// Vertical (`|`) or horizontal (`—`).
    pub orientation: SeparatorOrientation,
}

/// One in-window floating panel window.
#[derive(Clone, Debug, PartialEq)]
pub struct FloatingView {
    /// The floating window.
    pub id: FloatingWindowId,
    /// Its rect, window-local logical pixels.
    pub rect: Rect,
    /// Its tabs, in order.
    pub panels: Vec<PanelRef>,
    /// Index of the active tab.
    pub active_tab: usize,
}

/// One open overlay. `O` is the app's overlay identity type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayView<O> {
    /// App identity.
    pub id: O,
    /// Instance slot.
    pub slot: OverlaySlot,
    /// Composite kind.
    pub kind: OverlayKind,
    /// Rect, window-local logical pixels.
    pub rect: Rect,
    /// Blocks everything below it.
    pub modal: bool,
    /// Depth of the focus scope it opened (0 = none).
    pub scope_depth: usize,
}

/// Pointer and keyboard-focus state of a window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputView {
    /// Widget under the pointer.
    pub hovered: Option<WidgetId>,
    /// Widget being pressed.
    pub pressed: Option<WidgetId>,
    /// Widget with keyboard focus.
    pub focused: Option<WidgetId>,
    /// The pointer is captured (by a widget or an engine).
    pub captured: bool,
    /// Number of focus scopes open.
    pub scope_depth: usize,
}

/// Repaint cadence of a window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CadenceView {
    /// Base tick rate.
    pub tick: TickRate,
    /// Declared regions.
    pub regions: Vec<RegionSpec>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::layout::docking::DockPanel;

    #[derive(Clone, Debug)]
    struct P;
    impl DockPanel for P {
        fn title(&self) -> &str {
            "p"
        }
        fn type_id(&self) -> &'static str {
            "p"
        }
    }
    struct S;
    impl Spec for S {
        type Panel = P;
        type Overlay = u8;
        type Action = u8;
        fn decode_panel(_leaf: LeafId, _type_id: &str) -> Option<P> {
            None
        }
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn snapshot_is_send_sync() {
        assert_send_sync::<VisualSnapshot<S>>();
    }

    #[test]
    fn empty_snapshot_and_window_lookup() {
        let mut snap = VisualSnapshot::<S>::empty();
        assert_eq!(snap.revision, Revision::ZERO);
        assert!(snap.window(WindowId(1)).is_none());
        snap.windows.push(WindowView {
            id: WindowId(1),
            key: WindowKey::new("main"),
            geometry: WindowGeometry::default(),
            focused: false,
            cursor: CursorIcon::Default,
            chrome: ChromeView::default(),
            edges: Vec::new(),
            dock: DockView::default(),
            overlays: vec![OverlayView {
                id: 3u8,
                slot: OverlaySlot(1),
                kind: OverlayKind::Modal,
                rect: Rect::default(),
                modal: true,
                scope_depth: 1,
            }],
            input: InputView::default(),
            cadence: CadenceView::default(),
        });
        let copy = snap.clone();
        assert_eq!(copy, snap);
        assert_eq!(copy.window(WindowId(1)).map(|w| w.overlays.len()), Some(1));
        assert!(format!("{copy:?}").contains("WindowView"));
    }
}
