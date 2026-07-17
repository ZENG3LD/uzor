//! [`PdfFontCache`] — one document-wide font registration table, shared
//! between a document's own per-word paragraph text runs
//! (`uzor-typeset::export::pdf_adapter`) and any
//! [`super::render_context::PdfRenderContext`] drawing figures/tables/page
//! chrome directly into a page's content stream (typography-gap WAVE 1:
//! vector figures/tables/chrome). Both consumers resolve the SAME
//! `(FontFamily, bold, italic)` key through this ONE cache so a figure's
//! axis-label font and a paragraph's body font never register the same
//! logical font twice under two different `/Font` resource names on the
//! same page (design doc §6.1's "one embedded font resource per unique
//! `FontSpec` ... deduped" promise, now spanning figure/table/chrome text
//! too, not just paragraph text).
//!
//! Moved here (previously a private `FontCache` inside
//! `uzor-typeset::export::pdf_adapter`) so [`super::render_context::
//! PdfRenderContext`] — which lives in THIS crate, not `uzor-typeset` —
//! can share it without a dependency inversion (`uzor-export` must never
//! depend on `uzor-typeset`).

use uzor::fonts::FontFamily;

use super::{FontId, PdfBuilder};

/// One dedup'd font registration, keyed by the SAME `(family, bold,
/// italic)` triple [`uzor::fonts::font_bytes`] itself takes. A small `Vec`
/// (not a `HashMap`): `FontFamily` derives `PartialEq` but not `Hash`, and
/// a document realistically registers only a handful of distinct fonts,
/// so linear search is simpler and cheap enough (unchanged reasoning from
/// this type's original home in `pdf_adapter.rs`).
pub struct PdfFontCache(Vec<((FontFamily, bool, bool), FontId)>);

impl Default for PdfFontCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfFontCache {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Resolve `(family, bold, italic)` to a [`FontId`], registering it
    /// with `builder` the FIRST time this exact combination is seen
    /// across the whole document — every subsequent call (from a
    /// paragraph run OR a figure/chrome `fill_text` call) returns the
    /// SAME id.
    pub fn id_for(&mut self, family: FontFamily, bold: bool, italic: bool, builder: &mut PdfBuilder) -> FontId {
        let key = (family, bold, italic);
        if let Some(&(_, id)) = self.0.iter().find(|&&(k, _)| k == key) {
            return id;
        }
        let bytes = uzor::fonts::font_bytes(family, bold, italic);
        let id = builder.register_font(bytes);
        self.0.push((key, id));
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_family_bold_italic_combination_reuses_the_same_font_id() {
        let mut builder = PdfBuilder::new();
        let mut cache = PdfFontCache::new();
        let a = cache.id_for(FontFamily::Roboto, false, false, &mut builder);
        let b = cache.id_for(FontFamily::Roboto, false, false, &mut builder);
        assert_eq!(a, b, "the same (family, bold, italic) key must resolve to the SAME FontId across calls");
    }

    #[test]
    fn a_different_style_combination_registers_a_distinct_font_id() {
        let mut builder = PdfBuilder::new();
        let mut cache = PdfFontCache::new();
        let regular = cache.id_for(FontFamily::Roboto, false, false, &mut builder);
        let bold = cache.id_for(FontFamily::Roboto, true, false, &mut builder);
        assert_ne!(regular, bold, "a different bold/italic combination must register its own FontId");
    }
}
