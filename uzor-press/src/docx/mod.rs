//! DOCX writer — a second, independent renderer over the same
//! `parse::Document` AST (see `docs/uzor-engines/plans/
//! press-docx-and-neutral-themes-2026-09-10.md`'s "Decision 2"). Reads
//! `parse::Document` directly and emits native OOXML flow content; it does
//! NOT run through `uzor_typeset::compose`/`slice_pages` — Word paginates
//! the result itself. `mod.rs` holds only re-exports, this crate's own
//! "flat single-purpose files" convention extended to a module subtree.

mod body;
mod build;
mod error;
mod media;
mod numbering;
mod package;
mod section;
mod styles;
mod units;

pub(crate) use build::build_docx;
