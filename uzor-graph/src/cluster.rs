//! Clustering / group-collapse seam — explicitly DEFERRED this run (next
//! milestone; see the top-level task scope and engine design doc §5).
//!
//! These types exist so a future collapse implementation has a stable
//! place to land without reshaping the `Graph`/`Layout`/render
//! contracts. Nothing in this crate constructs or reads them yet — no
//! `ActiveView` projection, no collapse/expand mechanism. When that
//! lands, the intended shape (design doc §5.1) is: `VizGraph`/`Graph`
//! stays the unmodified source of truth, and an `ActiveView` projects
//! `GroupNode::member_nodes` out of the simulated/rendered/hit-tested
//! set when `collapsed`, replacing them with one synthetic node whose
//! edges are re-routed and deduplicated. Membership predicate is
//! caller-supplied (mirrors vis.js's `joinCondition`); the engine only
//! owns the collapse mechanism.

use crate::graph::NodeIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub u32);

/// A caller-defined meta-node that would replace `member_nodes` in the
/// simulated/rendered/hit-tested set when `collapsed`. Aggregation
/// (`badge_count`, `aggregate`) is caller-supplied over the collapsing
/// members, matching vis.js's `processProperties` callback.
pub struct GroupNode<N> {
    pub id: GroupId,
    pub member_nodes: Vec<NodeIndex>,
    pub collapsed: bool,
    pub badge_count: u32,
    pub aggregate: N,
}
