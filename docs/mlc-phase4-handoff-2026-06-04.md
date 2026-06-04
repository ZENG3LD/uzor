# mlc Phase 4 → uzor: API ask

**From:** mylittlechart (mlc) Phase 4 S3–S6 sub-pane refactor
**Date:** 2026-06-04
**Status:** mlc S2 complete; S3–S6 plan written. One uzor API gap blocks S4/S6 close.
**mlc plan doc:** `nemo/mylittlechart/docs/plans/phase4-s3-s6-subpane-refactor-plan-2026-06-04.md`

---

## Summary

mlc is collapsing sub-pane geometry + input from hand-rolled `Vec<SubPane>` + `InnerLayout` + rect-cache hit-testing onto uzor's native docking + widget input. Most of the uzor API surface this requires already exists. **One method is missing.** Below is the ask plus three calls we wanted to verify and confirmed are already there.

If you can land Gap 1 in the next uzor minor bump, mlc S4+S6 unblock cleanly. Without it, MoveUp/MoveDown overlay buttons in sub-pane controls have to ship as stubs (gray-out) until the gap closes.

---

## Gap 1 (MISSING) — `reorder_children`

### What we need

```rust
impl<P: DockPanel> DockingTree<P> {
    /// Move the child at `from_idx` to `to_idx` within the branch identified
    /// by `branch_id`. Proportions move with the child (the moved child keeps
    /// its proportion; siblings shift). Returns true if the move was valid
    /// (branch exists, both indices in range, from != to).
    pub fn reorder_children(
        &mut self,
        branch_id: BranchId,
        from_idx: usize,
        to_idx: usize,
    ) -> bool;
}
```

### Why mlc needs it

Sub-pane overlay buttons include **Move Up** and **Move Down** — they reshuffle indicator panels vertically within a chart container's inner docking tree (the branch under the chart-container Branch).

Today (pre-S4) we do this on a `Vec<SubPane>` by swapping vec indices. After S4, sub-panes are uzor leaves inside an inner `DockState<ChartContainerLeaf>`. Without `reorder_children`, there is no in-place sibling permutation API — we'd have to `remove_leaf` + `split_leaf` re-insert, which churns the leaf id (breaks save/restore stable identity) and is far heavier than a vec swap.

### Suggested impl

The existing `Branch.children: Vec<NodeId>` is a Vec. `reorder_children` is a Vec swap + an equal-length swap on `proportions`. Roughly:

```rust
pub fn reorder_children(&mut self, branch_id: BranchId, from_idx: usize, to_idx: usize) -> bool {
    let Some(branch) = self.branch_mut(branch_id) else { return false };
    if from_idx >= branch.children.len() || to_idx >= branch.children.len() { return false }
    if from_idx == to_idx { return true }
    let moved_child = branch.children.remove(from_idx);
    branch.children.insert(to_idx, moved_child);
    let moved_prop = branch.proportions.remove(from_idx);
    branch.proportions.insert(to_idx, moved_prop);
    true
}
```

Plus an integration test in `tests/docking/` covering: swap adjacent, swap non-adjacent, from > to, from < to, from == to (no-op), out-of-range (false), single-child branch (false on any non-trivial move).

### Acceptance from mlc

Once landed, mlc S6 wires:

```rust
ChartAction::SubPaneMoveUp { instance_id } => {
    let (branch_id, idx) = chart.find_sub_pane_in_inner_docking(instance_id)?;
    if idx > 0 {
        chart.inner_docking.tree_mut().reorder_children(branch_id, idx, idx - 1);
    }
}
```

(SubPaneMoveDown symmetric.)

---

## Gap 2 (RESOLVED) — `remove_leaf` already exists

The mlc plan listed `remove_leaf` as a gap. Confirmed today it exists at `src/layout/docking/grid.rs:262`. No action.

mlc will use it for permanent indicator delete in S6 (vs `hide_leaf` which is reversible).

---

## Gap 3 (RESOLVED) — overlay hover query already exists

The mlc plan listed `InputCoordinator::is_hovered(&WidgetId) -> bool` as a gap to verify. Confirmed at `src/input/core/coordinator.rs:649`. Also `hovered_widget() -> Option<&WidgetId>` at `:681`.

mlc S6 uses these for the overlay-button hover highlight. No action.

---

## `find_branch_of_leaf` design-doc delta

mlc design doc referenced a hypothetical `tree.find_branch_of_leaf(leaf_id)`. Current uzor has `find_parent_of_leaf(leaf_id) -> Option<&Branch<P>>` at `grid.rs:607`. The `Branch.id` field is `pub`, so mlc can derive branch_id via `tree.find_parent_of_leaf(leaf).map(|b| b.id)`. No new uzor API needed for this — just a doc note.

If you want to expose a thin `branch_id_of_leaf(LeafId) -> Option<BranchId>` convenience method, mlc would use it; not blocking.

---

## What uzor commits mlc is already pinned to

- `0f39ceb` (Branch.preserve_if_empty + DockingTree::set_branch_preserve_if_empty). mlc relies on this for ChartContainer / TradingContainer / TagGroup branches surviving when their child count drops to 1.

`Cargo.lock` in mlc workspace currently points at uzor `1.3.0` released 2026-05-16. If `reorder_children` ships in a 1.3.x patch or 1.4.0 minor, mlc bumps the path-dep and proceeds with S6.

---

## Timeline

mlc is mid-S2 complete, S3 starts whenever owner says go. S3 + S4 do not depend on `reorder_children` — only the SubPaneMoveUp/Down action handlers in S6 do. So an ordering that works:

| Time | mlc | uzor |
|---|---|---|
| Now | S3 (outer ChartSubPanel → PanelLeaf) | Plan `reorder_children` |
| +1 session | S4 (inner_docking wiring, sep drag via uzor native) | Ship `reorder_children` |
| +2 sessions | S5 (render unification) | — |
| +3 sessions | S6 (widget input, MoveUp/Down wired against the new uzor API) | — |

If `reorder_children` slips, mlc S6 ships MoveUp/Down as gray-out stubs with a TODO. Owner-visible UX impact: the buttons render but the user can't reorder via them — they can still remove + re-add an indicator to relocate it. Tolerable.

---

## Contact / questions

mlc plan owner: see `mylittlechart/docs/plans/phase4-s3-s6-subpane-refactor-plan-2026-06-04.md` for full S3–S6 detail.

Risks / edge cases the mlc plan calls out for `reorder_children` correctness:

- Reorder must NOT trigger `collapse_single_children_branch` (it never collapses; child count is unchanged). Verify in tests.
- Reorder of a branch marked `preserve_if_empty = true` must preserve that flag (the flag is on Branch, not on children, so this should be free — verify).
- After reorder, the next call to `separators()` must reflect the new sibling order. Verify the layout pass invalidates whatever caches `separators()` reads from.

That's it. Light ask.
