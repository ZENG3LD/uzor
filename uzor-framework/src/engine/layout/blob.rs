//! Layout blob codec (design §3.2.5): a postcard envelope with a `u16`
//! version, the dock structure and the floating windows.
//!
//! The structure is the library `LayoutSnapshot` (structure only; panel
//! values come back through the app's decoder), mirrored field by field in
//! [`WireDock`]: postcard is not self-describing, so the snapshot's
//! `skip_serializing_if` / `default` attributes (meant for its JSON form)
//! cannot be honoured on the wire, and the version prefix replaces them.
//! The same [`WireDock`] is the engine's "did the blob go stale" key: the
//! dock revision bumps exactly when it changes.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uzor::layout::docking::{
    DockPanel, DockingTree, FloatingWindow, FloatingWindowId, LayoutSnapshot, LeafId,
    SerializedGrid, SerializedNode, SerializedNodeType,
};

use crate::types::layout_blob::{LayoutBlob, LayoutCodecError, WindowGeometrySnapshot};
use crate::types::spec::PanelHome;

use super::{dock, Ctx, DockState, PanelDecoder, WindowLayout};

/// Envelope version this build writes and reads.
pub const BLOB_VERSION: u16 = 1;

/// Largest node count a blob may hold.
const MAX_NODES: usize = 4096;
/// Deepest branch nesting a blob may hold.
const MAX_DEPTH: usize = 64;
/// Most floating windows a blob may hold.
const MAX_FLOATING: usize = 256;

/// The blob-relevant state of one dock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct WireDock {
    root_id: u64,
    active_leaf: Option<u64>,
    nodes: Vec<WireNode>,
    floating: Vec<WireFloating>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WireNode {
    id: u64,
    kind: WireKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum WireKind {
    Leaf {
        panel_type_ids: Vec<String>,
        active_tab: u64,
        hidden: bool,
        color_tag: Option<u8>,
    },
    Branch {
        children: Vec<u64>,
        layout: String,
        proportions: Vec<f64>,
        cross_ratio: Option<(f64, f64)>,
        magnetic: bool,
        grid: Option<WireGrid>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WireGrid {
    rows: u64,
    cols: u64,
    row_ratios: Vec<f64>,
    col_ratios: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct WireFloating {
    id: u64,
    panel_type_ids: Vec<String>,
    active_tab: u64,
    rect: (f32, f32, f32, f32),
}

/// Everything after the version prefix.
#[derive(Serialize, Deserialize)]
struct Envelope {
    dock: WireDock,
    geometry: Option<WindowGeometrySnapshot>,
}

impl WireDock {
    /// The blob-relevant state of `dock`.
    pub(super) fn capture<P: DockPanel>(dock: &DockState<P>) -> Self {
        let snap = LayoutSnapshot::from_tree(dock.tree(), "");
        Self {
            root_id: snap.root_id,
            active_leaf: snap.active_leaf_id,
            nodes: snap.nodes.into_iter().map(WireNode::from).collect(),
            floating: dock
                .floating_windows()
                .iter()
                .map(|f| WireFloating {
                    id: f.id.0,
                    panel_type_ids: f.panels.iter().map(|p| p.type_id().to_string()).collect(),
                    active_tab: f.active_tab as u64,
                    rect: (f.x, f.y, f.width, f.height),
                })
                .collect(),
        }
    }

    /// Test helper: the same structure with the root listing itself as a
    /// child (a cycle the validator must refuse).
    #[cfg(test)]
    pub(super) fn with_root_cycle(&self) -> Self {
        let mut out = self.clone();
        let root = out.root_id;
        for n in &mut out.nodes {
            if let (true, WireKind::Branch { children, .. }) = (n.id == root, &mut n.kind) {
                children.push(root);
            }
        }
        out
    }

    fn to_snapshot(&self) -> LayoutSnapshot {
        LayoutSnapshot {
            version: "1.0".to_string(),
            name: String::new(),
            nodes: self.nodes.iter().map(SerializedNode::from).collect(),
            root_id: self.root_id,
            active_leaf_id: self.active_leaf,
        }
    }
}

impl From<SerializedNode> for WireNode {
    fn from(n: SerializedNode) -> Self {
        let kind = match n.node_type {
            SerializedNodeType::Leaf {
                panel_type_ids,
                active_tab,
                hidden,
                color_tag,
            } => WireKind::Leaf {
                panel_type_ids,
                active_tab: active_tab as u64,
                hidden,
                color_tag,
            },
            SerializedNodeType::Branch {
                children,
                layout,
                proportions,
                cross_ratio,
                magnetic,
                grid,
            } => WireKind::Branch {
                children,
                layout,
                proportions,
                cross_ratio,
                magnetic,
                grid: grid.map(|g| WireGrid {
                    rows: g.rows as u64,
                    cols: g.cols as u64,
                    row_ratios: g.row_ratios,
                    col_ratios: g.col_ratios,
                }),
            },
        };
        Self { id: n.id, kind }
    }
}

impl From<&WireNode> for SerializedNode {
    fn from(n: &WireNode) -> Self {
        let node_type = match &n.kind {
            WireKind::Leaf {
                panel_type_ids,
                active_tab,
                hidden,
                color_tag,
            } => SerializedNodeType::Leaf {
                panel_type_ids: panel_type_ids.clone(),
                active_tab: to_usize(*active_tab),
                hidden: *hidden,
                color_tag: *color_tag,
            },
            WireKind::Branch {
                children,
                layout,
                proportions,
                cross_ratio,
                magnetic,
                grid,
            } => SerializedNodeType::Branch {
                children: children.clone(),
                layout: layout.clone(),
                proportions: proportions.clone(),
                cross_ratio: *cross_ratio,
                magnetic: *magnetic,
                grid: grid.as_ref().map(|g| SerializedGrid {
                    rows: to_usize(g.rows),
                    cols: to_usize(g.cols),
                    row_ratios: g.row_ratios.clone(),
                    col_ratios: g.col_ratios.clone(),
                }),
            },
        };
        Self {
            id: n.id,
            node_type,
        }
    }
}

fn to_usize(v: u64) -> usize {
    usize::try_from(v).unwrap_or(usize::MAX)
}

fn malformed(why: impl Into<String>) -> LayoutCodecError {
    LayoutCodecError::Malformed(why.into())
}

/// Encode one dock (and optional window geometry) into a blob.
pub(super) fn encode(
    dock: &WireDock,
    geometry: Option<WindowGeometrySnapshot>,
) -> Result<LayoutBlob, LayoutCodecError> {
    let env = Envelope {
        dock: dock.clone(),
        geometry,
    };
    postcard::to_allocvec(&(BLOB_VERSION, &env))
        .map(LayoutBlob::from_bytes)
        .map_err(|e| LayoutCodecError::Encode(e.to_string()))
}

/// Decode a blob's envelope: version first, then the body; trailing bytes
/// are malformed.
fn decode(blob: &LayoutBlob) -> Result<Envelope, LayoutCodecError> {
    let (found, rest) = postcard::take_from_bytes::<u16>(blob.as_bytes())
        .map_err(|e| malformed(format!("version: {e}")))?;
    if found != BLOB_VERSION {
        return Err(LayoutCodecError::Version {
            found,
            supported: BLOB_VERSION,
        });
    }
    let (env, tail) =
        postcard::take_from_bytes::<Envelope>(rest).map_err(|e| malformed(e.to_string()))?;
    if !tail.is_empty() {
        return Err(malformed(format!("{} trailing bytes", tail.len())));
    }
    Ok(env)
}

/// The structure must be a tree the library can rebuild without
/// recursing forever: unique ids, a branch root, every child present and
/// referenced once, bounded size and depth.
fn validate(w: &WireDock) -> Result<(), LayoutCodecError> {
    if w.nodes.len() > MAX_NODES {
        return Err(malformed(format!(
            "{} nodes (max {MAX_NODES})",
            w.nodes.len()
        )));
    }
    if w.floating.len() > MAX_FLOATING {
        return Err(malformed(format!(
            "{} floating windows (max {MAX_FLOATING})",
            w.floating.len()
        )));
    }
    let mut by_id: HashMap<u64, &WireNode> = HashMap::new();
    for n in &w.nodes {
        if n.id == u64::MAX {
            return Err(malformed("node id out of range"));
        }
        if by_id.insert(n.id, n).is_some() {
            return Err(malformed(format!("duplicate node id {}", n.id)));
        }
    }
    let Some(root) = by_id.get(&w.root_id) else {
        return Err(malformed(format!("root node {} missing", w.root_id)));
    };
    if !matches!(root.kind, WireKind::Branch { .. }) {
        return Err(malformed("root is not a branch"));
    }
    let mut seen: HashSet<u64> = HashSet::from([w.root_id]);
    let mut stack: Vec<(u64, usize)> = vec![(w.root_id, 0)];
    while let Some((id, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            return Err(malformed(format!("nesting deeper than {MAX_DEPTH}")));
        }
        let Some(node) = by_id.get(&id) else {
            return Err(malformed(format!("node {id} missing")));
        };
        if let WireKind::Branch { children, .. } = &node.kind {
            for c in children {
                if !seen.insert(*c) {
                    return Err(malformed(format!("node {c} referenced twice")));
                }
                stack.push((*c, depth + 1));
            }
        }
    }
    let mut fids = HashSet::new();
    for f in &w.floating {
        if f.id == u64::MAX || !fids.insert(f.id) {
            return Err(malformed(format!("bad floating window id {}", f.id)));
        }
        if f.panel_type_ids.is_empty() {
            return Err(malformed(format!("floating window {} has no panels", f.id)));
        }
    }
    Ok(())
}

/// Rebuild a dock from the wire form; every panel through `decode`.
fn rebuild<P: DockPanel>(
    w: &WireDock,
    decode: PanelDecoder<P>,
) -> Result<DockState<P>, LayoutCodecError> {
    validate(w)?;
    let mut failed: Option<(u64, String)> = None;
    let tree: Result<DockingTree<P>, String> =
        w.to_snapshot().restore_tree_with_id(|leaf, type_id| {
            let panel = decode(PanelHome::Leaf(LeafId(leaf)), type_id);
            if panel.is_none() && failed.is_none() {
                failed = Some((leaf, type_id.to_string()));
            }
            panel
        });
    let tree = match (tree, failed) {
        (Ok(t), _) => t,
        (Err(_), Some((leaf, type_id))) => {
            return Err(LayoutCodecError::Panel {
                home: PanelHome::Leaf(LeafId(leaf)),
                type_id,
            })
        }
        (Err(e), None) => return Err(malformed(e)),
    };
    let mut dock = DockState::from_tree(tree);
    for f in &w.floating {
        let id = FloatingWindowId(f.id);
        let mut panels = Vec::with_capacity(f.panel_type_ids.len());
        for t in &f.panel_type_ids {
            match decode(PanelHome::Floating(id), t) {
                Some(p) => panels.push(p),
                None => {
                    return Err(LayoutCodecError::Panel {
                        home: PanelHome::Floating(id),
                        type_id: t.clone(),
                    })
                }
            }
        }
        let (x, y, width, height) = f.rect;
        if ![x, y, width, height].iter().all(|v| v.is_finite()) {
            return Err(malformed(format!("floating window {} rect", f.id)));
        }
        let fw = FloatingWindow::new(id, panels, to_usize(f.active_tab), x, y, width, height);
        if dock.insert_floating(fw).is_none() {
            return Err(malformed(format!("floating window {} refused", f.id)));
        }
    }
    Ok(dock)
}

/// `LayoutCmd::Restore` for one window: decode, rebuild, and swap the dock
/// in only when everything succeeded (any error leaves the window as it
/// was). The live pointer session is dropped with the old dock. Returns the
/// window geometry stored in the blob, if any.
pub(super) fn restore<P: DockPanel>(
    w: &mut WindowLayout<P>,
    blob: &LayoutBlob,
    ctx: &Ctx<P>,
) -> Result<Option<WindowGeometrySnapshot>, LayoutCodecError> {
    let env = decode(blob)?;
    let mut dock = rebuild(&env.dock, ctx.decode)?;
    dock::configure(&mut dock, &ctx.policy);
    w.dock = dock;
    w.session = None;
    Ok(env.geometry)
}
