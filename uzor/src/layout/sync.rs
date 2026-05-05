//! Synchronisation tagging for LM nodes.
//!
//! Every observable piece of LM state is classified into one of three
//! [`SyncMode`]s.  The classification answers the question "if I have
//! two windows attached, does this state live once on the LM root, or
//! once per window, or in a shared opt-in group?"
//!
//! ## The model (mirrors mlc's `tag_manager`)
//!
//! - **`Synced`** — single instance on the LM root.  Every window sees
//!   the same data.  Used for global preferences (style palette, z-layer
//!   ordering, panel-type registry).
//!
//! - **`Sometimes(Option<SyncGroupId>)`** — opt-in shared state.
//!   `None` = standalone (per-window).  `Some(group)` = the window has
//!   joined a sync group and shares state with every other member of the
//!   same group.  This is the natural mode for chart panels: two windows
//!   may share a symbol/timeframe/crosshair, two others may not.
//!
//! - **`Standalone`** — always per-window, never shareable.  Used for
//!   things tied physically to one OS window: pointer position, the dock
//!   tree's visual layout, modal positions, scroll offsets.
//!
//! ## What this module provides
//!
//! Today: just the type system + a registry that holds a [`SyncMode`]
//! per logical node id.  The actual *storage* unification (Synced state
//! living once on the root) is partly done — `styles` and `z_layers` are
//! already on `LayoutManager`, the rest of the state is on
//! `WindowBranch`.  This registry is the source of truth for *what*
//! should live where, used by the layout-tree visualiser and (later)
//! by tear-off / promote-to-shared operations.
//!
//! Future passes will move state between root and branch as the user
//! flips a node from `Standalone` → `Sometimes(group)` and back.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// SyncGroupId
// ---------------------------------------------------------------------------

/// Stable, unique identifier for a sync group.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncGroupId(pub u64);

static NEXT_SYNC_GROUP_ID: AtomicU64 = AtomicU64::new(1);

impl SyncGroupId {
    /// Allocate a fresh group id.
    pub fn generate() -> Self {
        Self(NEXT_SYNC_GROUP_ID.fetch_add(1, Ordering::SeqCst))
    }
}

// ---------------------------------------------------------------------------
// SyncMode
// ---------------------------------------------------------------------------

/// Classification of one LM-managed node's storage.
///
/// See module docs for semantics.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SyncMode {
    /// Single instance on the LM root, visible from every window.
    Synced,
    /// Opt-in shared state.  `None` ≡ standalone right now; `Some(group)`
    /// ≡ the window is a member of the named group and shares this node
    /// with every other member.
    Sometimes(Option<SyncGroupId>),
    /// Always per-window — explicitly never shareable.
    Standalone,
}

impl SyncMode {
    /// Short user-facing label for visualisers.
    pub fn label(&self) -> &'static str {
        match self {
            SyncMode::Synced            => "synced",
            SyncMode::Sometimes(None)   => "sometimes (alone)",
            SyncMode::Sometimes(Some(_))=> "sometimes (group)",
            SyncMode::Standalone        => "standalone",
        }
    }

    /// Colour hint for the visualiser (RGBA, 0..=1 floats).
    pub fn color(&self) -> [f32; 4] {
        match self {
            SyncMode::Synced            => [0.40, 0.85, 0.45, 1.0], // green
            SyncMode::Sometimes(None)   => [0.95, 0.85, 0.30, 1.0], // amber
            SyncMode::Sometimes(Some(_))=> [0.30, 0.65, 0.95, 1.0], // blue
            SyncMode::Standalone        => [0.85, 0.40, 0.40, 1.0], // red
        }
    }
}

// ---------------------------------------------------------------------------
// Default classifications
// ---------------------------------------------------------------------------

/// Logical id of a sync-classified node.
///
/// These are stable, hand-picked names so external tooling can look a
/// node up regardless of the internal struct it lives in.
pub mod node_ids {
    pub const STYLES:        &str = "styles";
    pub const Z_LAYERS:      &str = "z_layers";
    pub const FRAME_TIME:    &str = "frame_time_ms";
    pub const PANEL_TYPES:   &str = "panel_type_registry";

    pub const CHROME_CFG:    &str = "branch.chrome_cfg";
    pub const EDGES:         &str = "branch.edges";
    pub const DOCK_TREE:     &str = "branch.dock_tree";
    pub const LAYOUT_TREE:   &str = "branch.layout_tree";
    pub const OVERLAYS:      &str = "branch.overlays";
    pub const DISPATCHER:    &str = "branch.dispatcher";
    pub const CTX_MANAGER:   &str = "branch.ctx";
    pub const POINTER_STATE: &str = "branch.pointer_state";

    pub const MODALS:        &str = "branch.modals";
    pub const POPUPS:        &str = "branch.popups";
    pub const DROPDOWNS:     &str = "branch.dropdowns";
    pub const TOOLBARS:      &str = "branch.toolbars";
    pub const SIDEBARS:      &str = "branch.sidebars";
    pub const CONTEXT_MENUS: &str = "branch.context_menus";
    pub const CHROME_STATE:  &str = "branch.chrome_widget_state";
}

// ---------------------------------------------------------------------------
// SyncRegistry
// ---------------------------------------------------------------------------

/// Map from node id (see [`node_ids`]) to its [`SyncMode`].
///
/// `LayoutManager` owns one of these and exposes it to the layout-tree
/// visualiser.  Apps may override the defaults at startup (e.g. promote
/// `dock_tree` from `Standalone` to `Sometimes`).
#[derive(Clone, Debug)]
pub struct SyncRegistry {
    modes: HashMap<&'static str, SyncMode>,
}

impl SyncRegistry {
    /// Construct the default classification used by uzor out of the box.
    pub fn defaults() -> Self {
        let mut modes = HashMap::new();
        // Root — single instance shared by every window.
        modes.insert(node_ids::STYLES,        SyncMode::Synced);
        modes.insert(node_ids::Z_LAYERS,      SyncMode::Synced);
        modes.insert(node_ids::FRAME_TIME,    SyncMode::Synced);
        modes.insert(node_ids::PANEL_TYPES,   SyncMode::Synced);

        // Tied to one physical window — never shared.
        modes.insert(node_ids::LAYOUT_TREE,   SyncMode::Standalone);
        modes.insert(node_ids::OVERLAYS,      SyncMode::Standalone);
        modes.insert(node_ids::DISPATCHER,    SyncMode::Standalone);
        modes.insert(node_ids::CTX_MANAGER,   SyncMode::Standalone);
        modes.insert(node_ids::POINTER_STATE, SyncMode::Standalone);

        // Default per-window but easy to opt-in into a group later.
        modes.insert(node_ids::CHROME_CFG,    SyncMode::Sometimes(None));
        modes.insert(node_ids::EDGES,         SyncMode::Sometimes(None));
        modes.insert(node_ids::DOCK_TREE,     SyncMode::Sometimes(None));

        // Per-instance composite state.  Modals + popups + dropdowns +
        // context menus are inherently per-window (they pop up on the
        // window the user clicked).  Toolbars & sidebars and the chrome
        // widget state are decoration — opt-in to share.
        modes.insert(node_ids::MODALS,        SyncMode::Standalone);
        modes.insert(node_ids::POPUPS,        SyncMode::Standalone);
        modes.insert(node_ids::DROPDOWNS,     SyncMode::Standalone);
        modes.insert(node_ids::CONTEXT_MENUS, SyncMode::Standalone);
        modes.insert(node_ids::TOOLBARS,      SyncMode::Sometimes(None));
        modes.insert(node_ids::SIDEBARS,      SyncMode::Sometimes(None));
        modes.insert(node_ids::CHROME_STATE,  SyncMode::Sometimes(None));

        Self { modes }
    }

    /// Look up the mode for a node, defaulting to `Standalone` for
    /// unknown ids so the visualiser still has something to show.
    pub fn get(&self, node_id: &str) -> SyncMode {
        self.modes
            .get(node_id)
            .copied()
            .unwrap_or(SyncMode::Standalone)
    }

    /// Override the mode for one node.  Apps call this in `App::init`
    /// to promote a node into a sync group (e.g. share dock layout
    /// across two analysis windows) or pin it standalone.
    pub fn set(&mut self, node_id: &'static str, mode: SyncMode) {
        self.modes.insert(node_id, mode);
    }

    /// Iterate every classified node id with its mode in arbitrary
    /// order.  Used by the visualiser.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, SyncMode)> + '_ {
        self.modes.iter().map(|(k, v)| (*k, *v))
    }
}

impl Default for SyncRegistry {
    fn default() -> Self {
        Self::defaults()
    }
}
