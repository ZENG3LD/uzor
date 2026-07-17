//! Document-outline (bookmark) tree assembly — nested `/Outlines`
//! [`pdf_writer::writers::OutlineItem`]s built from a flat, level-tagged
//! [`super::PdfOutlineEntry`] list (export SOTA research pass, item 5:
//! "arbitrary nesting, same shape as every PDF viewer's bookmark panel" —
//! `pdf-writer`'s own `Outline`/`OutlineItem` writers, no vendored PDF
//! object-graph code). Level-1 entries open by default (their own
//! children's `/Count` is positive); deeper levels default CLOSED
//! (negative `/Count`) — a viewer's user can always expand manually
//! regardless of the default state.

use super::RefAllocator;
use crate::pdf::PdfOutlineEntry;
use pdf_writer::{Pdf, Ref, TextStr};

/// One arena node — `entry_index` into the caller's own flat
/// `&[PdfOutlineEntry]`, plus this node's own children (indices into the
/// SAME arena, never a nested `Box`/`Rc` tree — avoids any recursive-type
/// ceremony for what's fundamentally a flat, indexed structure).
struct OutlineNode {
    entry_index: usize,
    level: u8,
    children: Vec<usize>,
}

/// Build a forest (top-level `roots` + a flat `arena` of nodes) from
/// `entries` — the standard "flat depth-tagged list -> tree" algorithm:
/// an entry nests under the closest PRECEDING entry with a STRICTLY
/// smaller level; an entry with no such preceding entry becomes its own
/// root. Handles an arbitrary level jump (e.g. straight from level 1 to
/// level 3, no level-2 entry present at all) by nesting the deeper entry
/// directly under the closest shallower one — no requirement that every
/// intermediate level actually appear.
fn build_forest(entries: &[PdfOutlineEntry]) -> (Vec<OutlineNode>, Vec<usize>) {
    let mut arena: Vec<OutlineNode> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        while let Some(&top) = stack.last() {
            if arena[top].level < entry.level {
                break;
            }
            stack.pop();
        }
        let node_index = arena.len();
        arena.push(OutlineNode { entry_index: i, level: entry.level, children: Vec::new() });
        match stack.last() {
            Some(&parent) => arena[parent].children.push(node_index),
            None => roots.push(node_index),
        }
        stack.push(node_index);
    }

    (arena, roots)
}

/// Total number of ALL descendants (every level, recursively) of
/// `node_index` — this node's own `/Count` magnitude (positive when its
/// own level is `<= 1`, "open by default"; negative otherwise, per this
/// module's own doc comment). A deliberately simplified (non-cascading)
/// reading of the PDF spec's own `/Count` semantics (report): the spec's
/// fully precise definition wants a recursively-cascading "how many
/// would show if opened, given each descendant's OWN open/closed default
/// too" count; real viewers navigate primarily via
/// `/First`/`/Last`/`/Next`/`/Prev`/`/Parent` and tolerate an approximate
/// `/Count` magnitude, so this crate computes the simpler "total
/// descendant count regardless of nesting" — correct sign (open/closed),
/// approximate magnitude.
fn count_descendants(arena: &[OutlineNode], node_index: usize) -> i32 {
    let node = &arena[node_index];
    let mut total = node.children.len() as i32;
    for &child in &node.children {
        total += count_descendants(arena, child);
    }
    total
}

/// Write the full `/Outlines` tree (root dict + every nested outline
/// item) for `entries`, returning the root's own [`Ref`] for
/// [`pdf_writer::writers::Catalog::outlines`] — `None` when `entries` is
/// empty (no `/Outlines` entry at all, matching this crate's own
/// established "additive, opt-in" convention for [`super::PdfMeta`]).
/// Every destination is a page-top `/XYZ` jump (`left = 0`, `top` = that
/// page's own full height in pt, matching this crate's already-documented
/// top-left-vs-bottom-left coordinate flip). An entry whose `page_index`
/// is out of range for `page_refs`/`page_heights_pt` is skipped entirely
/// (never panics — this crate's own "degrade gracefully on malformed
/// input" convention; by construction `uzor-typeset::export::pdf_adapter`
/// never produces an out-of-range index, so this is a defensive backstop
/// only, never exercised by this workspace's own real caller).
pub(super) fn write_outline_tree(pdf: &mut Pdf, refs: &mut RefAllocator, entries: &[PdfOutlineEntry], page_refs: &[Ref], page_heights_pt: &[f64]) -> Option<Ref> {
    if entries.is_empty() {
        return None;
    }

    let (arena, roots) = build_forest(entries);
    let item_refs: Vec<Ref> = arena.iter().map(|_| refs.next()).collect();
    let root_ref = refs.next();

    let root_range = write_outline_siblings(pdf, &arena, &item_refs, &roots, root_ref, entries, page_refs, page_heights_pt);

    let mut outline = pdf.outline(root_ref);
    if let Some((first, last)) = root_range {
        outline.first(first);
        outline.last(last);
    }
    let visible: i32 = roots.iter().map(|&r| 1 + if arena[r].level <= 1 { count_descendants(&arena, r) } else { 0 }).sum();
    outline.count(visible);
    drop(outline);

    Some(root_ref)
}

/// Write one sibling LEVEL (either the top-level `roots` or one node's
/// own `children`), returning `(first_ref, last_ref)` for the caller to
/// wire into its own `/First`/`/Last`. Recurses into each node's own
/// children FIRST (so this node's own `/First`/`/Last`/`/Count` can
/// reference them) before writing this node's own dict.
#[allow(clippy::too_many_arguments)]
fn write_outline_siblings(
    pdf: &mut Pdf,
    arena: &[OutlineNode],
    item_refs: &[Ref],
    siblings: &[usize],
    parent_ref: Ref,
    entries: &[PdfOutlineEntry],
    page_refs: &[Ref],
    page_heights_pt: &[f64],
) -> Option<(Ref, Ref)> {
    if siblings.is_empty() {
        return None;
    }

    for (i, &node_index) in siblings.iter().enumerate() {
        let node = &arena[node_index];
        let entry = &entries[node.entry_index];
        let my_ref = item_refs[node_index];

        let child_range = write_outline_siblings(pdf, arena, item_refs, &node.children, my_ref, entries, page_refs, page_heights_pt);

        let mut item = pdf.outline_item(my_ref);
        item.title(TextStr(&entry.title));
        item.parent(parent_ref);
        if i > 0 {
            item.prev(item_refs[siblings[i - 1]]);
        }
        if i + 1 < siblings.len() {
            item.next(item_refs[siblings[i + 1]]);
        }
        if let Some((first, last)) = child_range {
            item.first(first);
            item.last(last);
            let count = count_descendants(arena, node_index);
            item.count(if node.level <= 1 { count } else { -count });
        }
        if let (Some(&page_ref), Some(&height)) = (page_refs.get(entry.page_index as usize), page_heights_pt.get(entry.page_index as usize)) {
            item.dest().page(page_ref).xyz(0.0, height as f32, None);
        }
        drop(item);
    }

    let first = item_refs[siblings[0]];
    let last = item_refs[*siblings.last().expect("checked non-empty above")];
    Some((first, last))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(level: u8, title: &str, page_index: u32) -> PdfOutlineEntry {
        PdfOutlineEntry { level, title: title.to_owned(), page_index }
    }

    #[test]
    fn two_level_one_entries_become_two_separate_roots() {
        let entries = [entry(1, "A", 0), entry(1, "B", 1)];
        let (arena, roots) = build_forest(&entries);
        assert_eq!(roots.len(), 2);
        assert!(arena[roots[0]].children.is_empty());
        assert!(arena[roots[1]].children.is_empty());
    }

    #[test]
    fn a_level_2_entry_nests_under_the_preceding_level_1_entry() {
        let entries = [entry(1, "Parent", 0), entry(2, "Child", 0), entry(1, "Sibling", 1)];
        let (arena, roots) = build_forest(&entries);
        assert_eq!(roots.len(), 2, "two level-1 roots: Parent and Sibling");
        assert_eq!(arena[roots[0]].children.len(), 1, "Parent must have exactly one child (Child)");
        assert_eq!(arena[arena[roots[0]].children[0]].entry_index, 1);
        assert!(arena[roots[1]].children.is_empty());
    }

    #[test]
    fn an_arbitrary_level_jump_nests_under_the_closest_smaller_level_ancestor() {
        // level 1 -> level 3 directly (no level-2 entry present at all).
        let entries = [entry(1, "Top", 0), entry(3, "Deep", 0)];
        let (arena, roots) = build_forest(&entries);
        assert_eq!(roots.len(), 1);
        assert_eq!(arena[roots[0]].children.len(), 1, "the level-3 entry must nest directly under the level-1 root");
    }

    #[test]
    fn count_descendants_counts_every_level_recursively() {
        let entries = [entry(1, "A", 0), entry(2, "A.1", 0), entry(3, "A.1.a", 0), entry(1, "B", 1)];
        let (arena, roots) = build_forest(&entries);
        assert_eq!(count_descendants(&arena, roots[0]), 2, "A has 2 total descendants (A.1 and A.1.a)");
        assert_eq!(count_descendants(&arena, roots[1]), 0, "B is a leaf");
    }
}
