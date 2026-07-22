//! Tagged-PDF structure tree (typography wave 5) — real `/MarkInfo` +
//! `/StructTreeRoot` built from a flat, adapter-registered element list,
//! the SAME "flat, adapter-constructed, engine-agnostic" convention
//! [`super::PdfOutlineEntry`]/`outline::write_outline_tree` already
//! established: `uzor-typeset::export::pdf_adapter` is the only caller of
//! [`super::PdfBuilder::register_struct_elem`], and this module itself
//! knows nothing about `Page`/`ParagraphLayout` — only plain PDF
//! structure-tree vocabulary (design law: engine-agnostic boundary, see
//! this crate's own `CLAUDE.md`).
//!
//! ## Element granularity: one struct element per PAGE occurrence
//!
//! A source block that splits across pages (a paragraph continuing onto
//! the next page, a table continuing across a page break) gets a
//! SEPARATE structure element per page fragment, never one element
//! spanning two pages — the adapter registers a fresh element every time
//! it starts collecting a fragment's own content, and each element ties
//! to exactly one page's own marked-content sequence. A real cross-
//! fragment PDF 2.0 `/Ref` link is not built (documented v1 scope limit,
//! see this crate's own `CLAUDE.md`).
//!
//! ## Container vs. leaf roles
//!
//! [`PdfTagRole::Table`]/[`PdfTagRole::TableRow`] are pure CONTAINERS — an
//! element with one of those roles never gets its own marked-content
//! occurrence, only children (other registered elements, via `/K` as an
//! array of indirect struct-element references). Every other role
//! ([`PdfTagRole::P`]/[`PdfTagRole::H1`]/[`PdfTagRole::H2`]/
//! [`PdfTagRole::Figure`]/[`PdfTagRole::TableCell`]) is a LEAF — it wraps
//! exactly one page's own marked-content sequence (one MCID) directly via
//! `/Pg` + a bare integer `/K`, never nested children.
//!
//! An element that ends up with NEITHER (a container with zero surviving
//! children, or a leaf nothing ever actually tagged at paint time) is
//! silently dropped from the written tree entirely — never a dangling or
//! empty dict object, never a panic (this crate's own "degrade
//! gracefully on malformed/incomplete input" convention).

use pdf_writer::types::StructRole as PdfWriterStructRole;
use pdf_writer::writers::StructTreeRoot;
use pdf_writer::{Name, Pdf, Ref, TextStr};

use super::RefAllocator;

/// Opaque handle to a structure element registered via
/// [`super::PdfBuilder::register_struct_elem`] — mirrors [`super::FontId`]'s
/// own "lightweight `Copy` id, not a borrow" convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructElemId(pub(super) u32);

/// PDF structure-tree role for one tagged region — typography wave 5's
/// own closed vocabulary (see this module's own doc comment for the
/// container-vs-leaf split). Named `PdfTagRole`, not the bare `StructRole`
/// `pdf_writer::types` already exports, so nothing in this crate's own
/// public API ever has to disambiguate two same-named types at a single
/// call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PdfTagRole {
    /// An ordinary paragraph.
    P,
    /// A first-level heading (from the document-outline pass's own
    /// `level == 1`).
    H1,
    /// A second-level heading (`level == 2`).
    H2,
    /// A figure or raster image, with its own `/Alt` description text.
    Figure,
    /// A whole table (container — see this module's own doc comment).
    Table,
    /// One table row (container, child of a [`PdfTagRole::Table`]).
    TableRow,
    /// One table cell (leaf, child of a [`PdfTagRole::TableRow`]).
    TableCell,
}

impl PdfTagRole {
    pub(super) fn is_container(self) -> bool {
        matches!(self, PdfTagRole::Table | PdfTagRole::TableRow)
    }

    fn pdf_writer_role(self) -> PdfWriterStructRole {
        match self {
            PdfTagRole::P => PdfWriterStructRole::P,
            PdfTagRole::H1 => PdfWriterStructRole::H1,
            PdfTagRole::H2 => PdfWriterStructRole::H2,
            PdfTagRole::Figure => PdfWriterStructRole::Figure,
            PdfTagRole::Table => PdfWriterStructRole::Table,
            PdfTagRole::TableRow => PdfWriterStructRole::TR,
            PdfTagRole::TableCell => PdfWriterStructRole::TD,
        }
    }

    /// The `BDC` tag name this role's own marked-content sequences use —
    /// the SAME name the element's own `/S` entry carries, so a reader
    /// that falls back to the raw BDC tag (some do, as a heuristic) still
    /// agrees with the real structure tree.
    pub(super) fn bdc_tag_name(self) -> &'static [u8] {
        match self {
            PdfTagRole::P => b"P",
            PdfTagRole::H1 => b"H1",
            PdfTagRole::H2 => b"H2",
            PdfTagRole::Figure => b"Figure",
            PdfTagRole::Table => b"Table",
            PdfTagRole::TableRow => b"TR",
            PdfTagRole::TableCell => b"TD",
        }
    }
}

/// One registered structure element — see
/// [`super::PdfBuilder::register_struct_elem`].
pub(super) struct StructElemSpec {
    pub role: PdfTagRole,
    pub parent: Option<StructElemId>,
    pub alt: Option<String>,
}

/// Per-page marked-content bookkeeping [`super::write_page`] threads
/// through [`super::render_context::emit_ops`] and its own text-run
/// loop — one instance per page, reset per page (MCIDs are page-local,
/// per the PDF spec's own `/StructParents`-indexed-array convention this
/// module's [`write_struct_tree`] relies on).
#[derive(Default)]
pub(crate) struct PageTagState {
    next_mcid: i32,
    /// `(struct_elem_id, mcid)` pairs opened on this page, in mcid order
    /// (mcid is assigned strictly increasing, so this Vec's own position
    /// already equals its stored mcid — kept explicit anyway, cheap and
    /// defensive against a future refactor breaking that invariant).
    occurrences: Vec<(u32, i32)>,
}

impl PageTagState {
    /// Open a new marked-content sequence for `id`, returning the fresh
    /// MCID to write into the BDC operator's own property list.
    pub(crate) fn open(&mut self, id: StructElemId) -> i32 {
        let mcid = self.next_mcid;
        self.next_mcid += 1;
        self.occurrences.push((id.0, mcid));
        mcid
    }

    pub(crate) fn occurrences_is_empty(&self) -> bool {
        self.occurrences.is_empty()
    }

    pub(crate) fn into_occurrences(self) -> Vec<(u32, i32)> {
        self.occurrences
    }
}

/// Write the whole tagged-PDF structure tree (a `Document` root + every
/// SURVIVING registered element, see this module's own doc comment for
/// "surviving") plus the `/ParentTree` number tree mapping each page's
/// own `/StructParents` key (its 0-based page index) to an array of
/// element refs indexed by MCID. Returns the `/StructTreeRoot`'s own
/// [`Ref`] for [`super::PdfBuilder::finish`] to wire into the catalog.
pub(super) fn write_struct_tree(pdf: &mut Pdf, refs: &mut RefAllocator, elems: &[StructElemSpec], page_refs: &[Ref], page_occurrences: &[Vec<(u32, i32)>]) -> Ref {
    let n = elems.len();
    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, e) in elems.iter().enumerate() {
        if let Some(p) = e.parent {
            children_of[p.0 as usize].push(i);
        }
    }

    // Every leaf element's own (page, mcid) occurrence, if any.
    let mut occurrence: Vec<Option<(Ref, i32)>> = vec![None; n];
    for (page_idx, occs) in page_occurrences.iter().enumerate() {
        let Some(&page_ref) = page_refs.get(page_idx) else { continue };
        for &(elem_idx, mcid) in occs {
            if let Some(slot) = occurrence.get_mut(elem_idx as usize) {
                *slot = Some((page_ref, mcid));
            }
        }
    }

    // Bottom-up survival: a container survives iff at least one of its
    // own children survives; a leaf survives iff it has a real
    // occurrence. Children always have a STRICTLY HIGHER index than
    // their own parent (the adapter always registers a parent before any
    // of its children — `register_struct_elem`'s own doc comment), so a
    // single descending pass already knows every child's own answer
    // before its parent needs it.
    let mut survives = vec![false; n];
    for i in (0..n).rev() {
        survives[i] = if elems[i].role.is_container() { children_of[i].iter().any(|&c| survives[c]) } else { occurrence[i].is_some() };
    }

    let elem_refs: Vec<Option<Ref>> = survives.iter().map(|&s| s.then(|| refs.next())).collect();
    let document_ref = refs.next();
    let struct_tree_root_ref = refs.next();

    for i in 0..n {
        let Some(elem_ref) = elem_refs[i] else { continue };
        let spec = &elems[i];
        let parent_ref = spec.parent.and_then(|p| elem_refs[p.0 as usize]).unwrap_or(document_ref);

        let mut se = pdf.struct_element(elem_ref);
        se.kind(spec.role.pdf_writer_role());
        se.parent(parent_ref);
        if let Some(alt) = &spec.alt {
            se.alt(TextStr(alt));
        }
        match occurrence[i] {
            Some((page_ref, mcid)) => {
                se.page(page_ref);
                se.pair(Name(b"K"), mcid);
            }
            None => {
                let mut children = se.children();
                for &c in &children_of[i] {
                    if let Some(cref) = elem_refs[c] {
                        children.struct_element(cref);
                    }
                }
            }
        }
    }

    {
        let mut doc_elem = pdf.struct_element(document_ref);
        doc_elem.kind(PdfWriterStructRole::Document);
        doc_elem.parent(struct_tree_root_ref);
        let mut children = doc_elem.children();
        for i in 0..n {
            if elems[i].parent.is_none() {
                if let Some(cref) = elem_refs[i] {
                    children.struct_element(cref);
                }
            }
        }
    }

    // Pass 1: allocate + write every per-page ParentTree array object as
    // its own standalone indirect object FIRST — must happen before
    // `root` (below) takes its own mutable borrow of `pdf`, since a
    // `StructTreeRoot`/`NumberTree` writer holds that borrow for its
    // whole scope and `pdf.indirect(..)` cannot be called again while it
    // is alive.
    let mut parent_tree_entries: Vec<(i32, Ref)> = Vec::new();
    for (page_idx, occs) in page_occurrences.iter().enumerate() {
        if occs.is_empty() {
            continue;
        }
        let array_ref = refs.next();
        {
            let mut arr = pdf.indirect(array_ref).array();
            for &(elem_idx, _mcid) in occs {
                if let Some(r) = elem_refs[elem_idx as usize] {
                    arr.item(r);
                }
            }
        }
        parent_tree_entries.push((page_idx as i32, array_ref));
    }

    {
        let mut root: StructTreeRoot = pdf.indirect(struct_tree_root_ref).start();
        root.child(document_ref);
        let mut parent_tree = root.parent_tree();
        let mut nums = parent_tree.nums();
        for &(page_idx, array_ref) in &parent_tree_entries {
            nums.insert(page_idx, array_ref);
        }
    }

    struct_tree_root_ref
}
