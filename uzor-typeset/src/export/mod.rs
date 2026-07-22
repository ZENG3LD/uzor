//! PDF export adapter (Arc 4 Phase P5, design doc §6.1) — the ONLY place in
//! this crate (or anywhere in this workspace) allowed to bridge a composed
//! `uzor-typeset::Page` into `uzor-export`'s neutral PDF content model.
//! `uzor-export` itself stays engine-agnostic (never imports `uzor-text`/
//! `uzor-figures`/`uzor-typeset` — the dependency-boundary law this crate
//! must not violate from its own side either): this module is where that
//! boundary gets crossed, deliberately and in exactly one direction.
//!
//! See [`pdf_adapter`]'s own module doc comment for the hybrid-fidelity
//! conversion itself (raster background + real vector text).

pub mod pdf_adapter;

#[cfg(test)]
mod showcase;

pub use pdf_adapter::{pages_to_pdf, PdfExportOptions};
