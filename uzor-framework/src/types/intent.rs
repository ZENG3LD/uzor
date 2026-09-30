//! Typed intents: the events the kernel delivers to the app.
//!
//! Intents are produced in the route, engine and response phases and handed
//! to `App::intent` in production order after the publish phase of the same
//! tick. Hover and drag progress are not intents (the app reads them from the
//! live view); only starts and ends of engine-owned drags surface. An intent
//! never carries business data, and never a string the app must parse except
//! `WidgetId` item ids the app itself declared.

use std::fmt;
use std::sync::Arc;

use uzor::layout::docking::{DropZone, LeafId};
use uzor::layout::window::WindowKey;
use uzor::layout::ChromeWindowControl;
use uzor::WidgetId;

use crate::types::bus::{DroppedFile, KeyInput, PointerInput, WheelInput};
use crate::types::ids::{Revision, Ticket, TrayItemId, WindowId};
use crate::types::layout_blob::{LayoutBlob, LayoutCodecError};
use crate::types::spec::Spec;

/// Everything the framework tells the app.
pub enum Intent<S: Spec> {
    /// A key binding resolved (KeymapEngine).
    Action(S::Action),
    /// Overlay lifecycle and composite item picks (OverlayEngine).
    Overlay(OverlayIntent<S::Overlay>),
    /// Dock structure and layout persistence (LayoutEngine).
    Dock(DockIntent),
    /// Window lifecycle (WindowEngine).
    Window(WindowIntent),
    /// Text field edits (InputEngine).
    Text(TextIntent),
    /// Files dropped on a widget (InputEngine).
    Drop(DropIntent),
    /// An app timer armed with `CadenceCmd::WakeAt` fired (CadenceEngine).
    Timer(u64),
    /// A screenshot the app requested, as PNG bytes.
    Screenshot {
        /// The request's ticket.
        ticket: Ticket,
        /// Encoded PNG.
        png: Arc<[u8]>,
    },
    /// A tray menu entry was selected (native only).
    Tray(TrayItemId),
    /// Input nothing in the framework consumed; the one place the app may add
    /// behaviour the framework does not know.
    Unhandled(UnhandledInput),
}

// Manual impls: a derive would demand the bounds of the marker type `S`
// itself; only its associated types need them (given by `Spec`).
impl<S: Spec> Clone for Intent<S> {
    fn clone(&self) -> Self {
        match self {
            Intent::Action(a) => Intent::Action(*a),
            Intent::Overlay(i) => Intent::Overlay(i.clone()),
            Intent::Dock(i) => Intent::Dock(i.clone()),
            Intent::Window(i) => Intent::Window(i.clone()),
            Intent::Text(i) => Intent::Text(i.clone()),
            Intent::Drop(i) => Intent::Drop(i.clone()),
            Intent::Timer(n) => Intent::Timer(*n),
            Intent::Screenshot { ticket, png } => Intent::Screenshot {
                ticket: *ticket,
                png: Arc::clone(png),
            },
            Intent::Tray(id) => Intent::Tray(*id),
            Intent::Unhandled(u) => Intent::Unhandled(u.clone()),
        }
    }
}

impl<S: Spec> fmt::Debug for Intent<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Intent::Action(a) => f.debug_tuple("Action").field(a).finish(),
            Intent::Overlay(i) => f.debug_tuple("Overlay").field(i).finish(),
            Intent::Dock(i) => f.debug_tuple("Dock").field(i).finish(),
            Intent::Window(i) => f.debug_tuple("Window").field(i).finish(),
            Intent::Text(i) => f.debug_tuple("Text").field(i).finish(),
            Intent::Drop(i) => f.debug_tuple("Drop").field(i).finish(),
            Intent::Timer(n) => f.debug_tuple("Timer").field(n).finish(),
            Intent::Screenshot { ticket, png } => f
                .debug_struct("Screenshot")
                .field("ticket", ticket)
                .field("png_len", &png.len())
                .finish(),
            Intent::Tray(id) => f.debug_tuple("Tray").field(id).finish(),
            Intent::Unhandled(u) => f.debug_tuple("Unhandled").field(u).finish(),
        }
    }
}

impl<S: Spec> PartialEq for Intent<S> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Intent::Action(a), Intent::Action(b)) => a == b,
            (Intent::Overlay(a), Intent::Overlay(b)) => a == b,
            (Intent::Dock(a), Intent::Dock(b)) => a == b,
            (Intent::Window(a), Intent::Window(b)) => a == b,
            (Intent::Text(a), Intent::Text(b)) => a == b,
            (Intent::Drop(a), Intent::Drop(b)) => a == b,
            (Intent::Timer(a), Intent::Timer(b)) => a == b,
            (
                Intent::Screenshot {
                    ticket: ta,
                    png: pa,
                },
                Intent::Screenshot {
                    ticket: tb,
                    png: pb,
                },
            ) => ta == tb && pa == pb,
            (Intent::Tray(a), Intent::Tray(b)) => a == b,
            (Intent::Unhandled(a), Intent::Unhandled(b)) => a == b,
            _ => false,
        }
    }
}

/// Why an overlay closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CloseCause {
    /// Escape key.
    Escape,
    /// Pointer-up outside the overlay.
    Outside,
    /// An `OverlayCmd::Close` / `CloseTop` / `Toggle`.
    Command,
    /// An item was picked (menu, dropdown) or the close button pressed.
    Item,
    /// The auto-close deadline fired.
    Timer,
    /// Its window closed.
    WindowClosed,
    /// An `Open` with the same app identity replaced this instance (the new
    /// instance's `Opened` follows).
    Replaced,
}

/// Overlay lifecycle and composite item picks. `O` is the app's overlay
/// identity type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverlayIntent<O> {
    /// The overlay opened.
    Opened {
        /// Its window.
        win: WindowId,
        /// App identity.
        id: O,
    },
    /// The overlay closed, for this reason.
    Closed {
        /// Its window.
        win: WindowId,
        /// App identity.
        id: O,
        /// Why.
        cause: CloseCause,
    },
    /// A modal tab was selected.
    ModalTab {
        /// Its window.
        win: WindowId,
        /// App identity.
        id: O,
        /// Tab index.
        index: usize,
    },
    /// A dropdown item was picked.
    DropdownItem {
        /// Its window.
        win: WindowId,
        /// App identity.
        id: O,
        /// The item id the app declared.
        item: WidgetId,
    },
    /// A context-menu item was picked.
    ContextMenuItem {
        /// Its window.
        win: WindowId,
        /// App identity.
        id: O,
        /// Item index.
        index: usize,
    },
    /// A toolbar item was clicked.
    ToolbarItem {
        /// Its window.
        win: WindowId,
        /// The toolbar.
        toolbar: WidgetId,
        /// The item id the app declared.
        item: WidgetId,
    },
    /// A chrome tab was selected.
    ChromeTab {
        /// Its window.
        win: WindowId,
        /// Tab index.
        index: usize,
    },
    /// A chrome window-control button was clicked.
    ChromeControl {
        /// Its window.
        win: WindowId,
        /// Which control.
        control: ChromeWindowControl,
    },
}

/// Dock structure changes and layout persistence answers.
#[derive(Clone, Debug, PartialEq)]
pub enum DockIntent {
    /// The user closed a panel tab.
    PanelClosed {
        /// Its window.
        win: WindowId,
        /// The leaf.
        leaf: LeafId,
        /// Former tab index.
        index: usize,
        /// The panel's `DockPanel::type_id`.
        type_id: &'static str,
    },
    /// A tab was activated.
    TabActivated {
        /// Its window.
        win: WindowId,
        /// The leaf.
        leaf: LeafId,
        /// Tab index.
        index: usize,
    },
    /// A panel was dropped into another leaf (same or another window).
    PanelMoved {
        /// Source window and leaf (the leaf the drag started from; it may be
        /// gone after the drop).
        from: (WindowId, LeafId),
        /// Destination window and the leaf that holds the panel now.
        to: (WindowId, LeafId),
        /// Where it was dropped.
        zone: DropZone,
    },
    /// A panel was torn off its leaf: dropped where no dock target was, so
    /// it now floats in-window (drag-out disabled, a single-window host, or
    /// a drop inside the viewport).
    PanelTornOff {
        /// Its window.
        win: WindowId,
        /// The source leaf.
        leaf: LeafId,
        /// Tab index.
        index: usize,
    },
    /// A panel was dragged out of its window into a new micro-window
    /// (reported once the micro-window id is allocated).
    PanelDraggedOut {
        /// Source window and leaf (the leaf may be gone).
        from: (WindowId, LeafId),
        /// The micro-window that carries the panel now.
        to: WindowId,
    },
    /// The user asked for a new panel in a leaf ("+" button).
    NewPanelRequested {
        /// Its window.
        win: WindowId,
        /// The leaf.
        leaf: LeafId,
    },
    /// Structure or ratios changed; coalesced to at most one per window per
    /// tick. The app debounces and requests a blob.
    LayoutChanged {
        /// Its window.
        win: WindowId,
        /// Dock revision after the change.
        dock_rev: Revision,
    },
    /// Answer to `LayoutCmd::RequestBlob`.
    LayoutBlob {
        /// Its window.
        win: WindowId,
        /// The request's ticket.
        ticket: Ticket,
        /// The serialized layout, for the app to store.
        blob: LayoutBlob,
    },
    /// `LayoutCmd::RequestBlob` could not be answered.
    LayoutBlobFailed {
        /// Its window.
        win: WindowId,
        /// The request's ticket.
        ticket: Ticket,
        /// Why.
        error: LayoutCodecError,
    },
    /// `LayoutCmd::Restore` succeeded.
    LayoutRestored {
        /// Its window.
        win: WindowId,
    },
    /// `LayoutCmd::Restore` failed; the window keeps the layout it had.
    LayoutRestoreFailed {
        /// Its window.
        win: WindowId,
        /// Why.
        error: LayoutCodecError,
    },
}

/// Window lifecycle intents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowIntent {
    /// The user asked to close the window; under an ask-the-app policy the
    /// app answers with `WindowCmd::Close`.
    CloseRequested {
        /// The window.
        win: WindowId,
    },
    /// The chrome "new window" button was pressed.
    NewWindowRequested {
        /// The window it was pressed in.
        from: WindowId,
    },
    /// A window opened.
    Opened {
        /// The window.
        win: WindowId,
        /// Its app label.
        key: WindowKey,
    },
    /// A window closed.
    Closed {
        /// The window.
        win: WindowId,
    },
    /// OS focus changed.
    FocusChanged {
        /// The window.
        win: WindowId,
        /// Focus gained.
        focused: bool,
    },
}

/// Text field intents; the value itself is read from the hook view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextIntent {
    /// The field's text changed.
    Changed {
        /// Its window.
        win: WindowId,
        /// The field.
        field: WidgetId,
    },
    /// The field was submitted (Enter).
    Submitted {
        /// Its window.
        win: WindowId,
        /// The field.
        field: WidgetId,
    },
}

/// File-drop intents.
#[derive(Clone, Debug, PartialEq)]
pub enum DropIntent {
    /// Files were dropped on a widget that senses drops.
    Files {
        /// Its window.
        win: WindowId,
        /// The drop target.
        target: WidgetId,
        /// The files.
        files: Vec<DroppedFile>,
    },
}

/// Input nothing consumed.
#[derive(Clone, Debug, PartialEq)]
pub enum UnhandledInput {
    /// A key event no field, overlay or binding took.
    Key {
        /// Its window.
        win: WindowId,
        /// The event.
        key: KeyInput,
    },
    /// A pointer event no overlay, layout zone or widget took.
    Pointer {
        /// Its window.
        win: WindowId,
        /// The event.
        pointer: PointerInput,
    },
    /// A wheel event no overlay, panel or widget took.
    Wheel {
        /// Its window.
        win: WindowId,
        /// The event.
        wheel: WheelInput,
    },
}
