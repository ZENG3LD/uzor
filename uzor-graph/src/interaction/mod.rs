//! Interaction layer — node pick/drag, hover-neighborhood focus.
//!
//! `FocusSet` is re-pointed onto `uzor_figures::interact::FocusSet`
//! (Phase D, 2026-07-17) — this crate no longer owns a fork. `NodeIndex`/
//! `EdgeIndex` key into its flat `u64` space via the `From` impls in
//! `crate::graph`; the "1-hop neighborhood" behavior this crate's old
//! fork had (that `uzor-figures`'s generic version couldn't express,
//! having no node-link identity type of its own) moved into
//! `uzor_figures::interact::FocusSet::select_many` as a generic bulk-
//! replace-selection op — `crate::graph::Graph::neighborhood_focus_keys`
//! computes the actual key list from this crate's adjacency and hands
//! it to `select_many`.

pub mod drag;
pub mod pick;
