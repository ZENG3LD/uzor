//! The app's vocabulary trait.

use std::fmt::Debug;
use std::hash::Hash;

use uzor::layout::docking::{DockPanel, FloatingWindowId, LeafId};

/// Where a panel being restored from a layout blob will live: the value
/// [`Spec::decode_panel`] is told, so the app can look up per-place state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PanelHome {
    /// A tab of this dock leaf.
    Leaf(LeafId),
    /// A tab of this in-window floating window.
    Floating(FloatingWindowId),
}

/// The three app-typed vocabularies the framework is generic over, so no
/// string dispatch exists anywhere: dock panel values, overlay identities and
/// keymap actions.
///
/// Implemented once by the app (usually on a unit struct); consumed by every
/// generic type in the crate (`AppCommand<S>`, `Intent<S>`,
/// `VisualSnapshot<S>`, and later the kernel and handle).
pub trait Spec: 'static {
    /// App-typed dock leaf payload (the lib docking trait supplies title,
    /// type id, minimum size and pin). `Clone + Send + Sync` come from
    /// `DockPanel`; `Debug` lets commands carrying panels be logged.
    type Panel: DockPanel + Debug + 'static;
    /// Identities of the app's modals, popups, dropdowns and menus.
    type Overlay: Copy + Eq + Hash + Debug + Send + Sync + 'static;
    /// Keymap targets; delivered back as `Intent::Action`.
    type Action: Copy + Eq + Hash + Debug + Send + Sync + 'static;

    /// Layout restore: the structure comes from the blob, the panel value from
    /// the app. Called once per stored panel with the place it goes into
    /// (a dock leaf or a floating window) and the stored
    /// `DockPanel::type_id`; `None` fails the restore and the window keeps
    /// the layout it had.
    fn decode_panel(home: PanelHome, type_id: &str) -> Option<Self::Panel>;
}
