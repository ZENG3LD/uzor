//! [`PageMaster`] — page geometry: physical size + margins + optional
//! header/footer margin-box content + a page-number token (design doc
//! §4.1, extended this phase — P0 built size+margins only, see this
//! crate's `CLAUDE.md`).
//!
//! Moved here from `slice/pages.rs` this phase — P0's own divergence #5
//! deferred the move explicitly "until P2's header/footer/placeholder
//! extensions actually need to share code across `PageMaster`/
//! `SlideMaster`," which is exactly what's happening now
//! (`master::slide_master` is this module's new sibling). `slice::pages`
//! still owns [`crate::slice::slice_pages`] itself and this crate's
//! `slice`/top-level modules keep re-exporting this type, so every
//! existing import path (`crate::slice::PageMaster`,
//! `uzor_typeset::PageMaster`) still resolves unchanged.
//!
//! ## Header/footer are margin-BOX content (CSS Paged Media naming)
//!
//! A header/footer paints INSIDE the page's own margin band
//! ([`PageMaster::header_rect`] = the top margin strip,
//! [`PageMaster::footer_rect`] = the bottom margin strip) — never
//! stealing space from [`PageMaster::body_rect`], matching the CSS Paged
//! Media `@page { @top-center {} }` "margin box" convention the design
//! doc's own comment names verbatim ("margin-box content, own tiny
//! SceneSpec").

use uzor::types::Rect;

use crate::scene::BlockNode;

/// Page margins, one value per edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Margins {
    pub fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self { top, right, bottom, left }
    }

    /// Same margin on every edge.
    pub fn uniform(all: f64) -> Self {
        Self { top: all, right: all, bottom: all, left: all }
    }
}

/// How a page number renders (design doc §4.1's `PageNumberStyle`,
/// elaborated — the doc names the type without specifying its own
/// variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageNumberFormat {
    /// `"3"`.
    Bare,
    /// `"3 of 12"`.
    OfTotal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageNumberStyle {
    pub format: PageNumberFormat,
    /// The number printed for the FIRST sliced page (`Page::index == 0`)
    /// — lets a caller start numbering at `1` (the common case) or
    /// continue a running number from an earlier document section.
    pub start: u32,
}

impl PageNumberStyle {
    pub fn new(format: PageNumberFormat, start: u32) -> Self {
        Self { format, start }
    }

    /// The formatted string for `page_index` (0-based, matches
    /// `crate::slice::Page::index`) out of `total_pages`.
    pub fn format_for(&self, page_index: u32, total_pages: u32) -> String {
        let n = self.start + page_index;
        match self.format {
            PageNumberFormat::Bare => format!("{n}"),
            PageNumberFormat::OfTotal => {
                let last = self.start + total_pages.saturating_sub(1);
                format!("{n} of {last}")
            }
        }
    }
}

/// Width reserved at the right edge of [`PageMaster::footer_rect`] for the
/// page-number text, when [`PageMaster::page_number_token`] is `Some` —
/// keeps the number from ever overlapping footer BLOCK content, which
/// composes into the remaining left portion of the same zone (see
/// [`PageMaster::footer_content_rect`]/[`PageMaster::page_number_rect`]).
const PAGE_NUMBER_RESERVED_WIDTH: f64 = 90.0;

/// Page geometry: physical size + margins + optional header/footer
/// margin-box content + page-number token (design doc §4.1).
pub struct PageMaster<'a> {
    pub width: f64,
    pub height: f64,
    pub margins: Margins,
    /// Margin-box content painted inside the TOP margin band, unchanged
    /// on every page (design doc §4.1: "margin-box content, own tiny
    /// SceneSpec" — this crate has no `SceneSpec` yet, see this crate's
    /// `CLAUDE.md`, so plain `&'a [BlockNode<'a>]` is used directly, the
    /// same "build the type the phase actually needs" divergence P0/P1
    /// already established for `slice_pages`/`draw_page`).
    pub header: Option<&'a [BlockNode<'a>]>,
    /// Margin-box content painted inside the BOTTOM margin band.
    pub footer: Option<&'a [BlockNode<'a>]>,
    pub page_number_token: Option<PageNumberStyle>,
    /// Number of equal-width sub-columns [`crate::slice::slice_pages`]
    /// splits the page BODY into (design doc §3.1's "N equal sub-columns
    /// per page-row") — `1` (the default set by [`PageMaster::new`]) is
    /// the pre-column single-body-rect-per-page behavior, unchanged.
    pub columns: usize,
    /// Horizontal gap between adjacent columns, in the same units as
    /// every other rect in this crate — unread when `columns <= 1`.
    pub column_gap: f64,
}

impl<'a> PageMaster<'a> {
    /// `width`/`height`/`margins` only — no header/footer/page-number
    /// (P0's own constructor, kept byte-for-byte so every existing call
    /// site compiles unchanged; additive law).
    pub fn new(width: f64, height: f64, margins: Margins) -> Self {
        Self { width, height, margins, header: None, footer: None, page_number_token: None, columns: 1, column_gap: 0.0 }
    }

    /// Builder: attach top-margin-box header content.
    pub fn with_header(mut self, header: &'a [BlockNode<'a>]) -> Self {
        self.header = Some(header);
        self
    }

    /// Builder: attach bottom-margin-box footer content.
    pub fn with_footer(mut self, footer: &'a [BlockNode<'a>]) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Builder: attach a page-number token.
    pub fn with_page_number(mut self, token: PageNumberStyle) -> Self {
        self.page_number_token = Some(token);
        self
    }

    /// Builder: split the page body into `columns` equal sub-columns
    /// separated by `gap` (design doc §3.1) — the SAME builder shape
    /// every other `PageMaster` extension in this crate already uses
    /// (`with_header`/`with_footer`/`with_page_number`). `columns <= 1`
    /// reproduces the pre-column single-body-rect-per-page behavior
    /// exactly (see [`crate::region::PageRegionSequence::with_columns`]).
    pub fn with_columns(mut self, columns: usize, gap: f64) -> Self {
        self.columns = columns.max(1);
        self.column_gap = gap;
        self
    }

    /// This master's own column `index` (0-based) rect within
    /// [`PageMaster::body_rect`] — the SAME geometry
    /// [`crate::region::PageRegionSequence::with_columns`] drives
    /// internally, exposed here so a caller building column-width-sized
    /// content (e.g. a `Paragraph` at exactly one column's own width)
    /// never has to duplicate the split-column formula.
    pub fn column_rect(&self, index: usize) -> Rect {
        crate::region::column_rect(self.body_rect(), self.columns, self.column_gap, index)
    }

    /// This master's own per-column width — `column_rect(0).width`,
    /// which is identical for every column index (equal-width columns).
    pub fn column_width(&self) -> f64 {
        self.column_rect(0).width
    }

    /// The page body rect (page size, inset by `margins`) — the one
    /// region [`crate::region::PageRegionSequence`] hands back on every
    /// page. UNCHANGED by header/footer — margin-box content lives
    /// INSIDE the margin band, never subtracted from the body (this
    /// module's own doc comment).
    pub fn body_rect(&self) -> Rect {
        Rect::new(
            self.margins.left,
            self.margins.top,
            (self.width - self.margins.left - self.margins.right).max(0.0),
            (self.height - self.margins.top - self.margins.bottom).max(0.0),
        )
    }

    /// The top margin band — [`PageMaster::header`]'s own margin box.
    pub fn header_rect(&self) -> Rect {
        Rect::new(self.margins.left, 0.0, (self.width - self.margins.left - self.margins.right).max(0.0), self.margins.top)
    }

    /// The full bottom margin band (both footer content AND, when
    /// [`PageMaster::page_number_token`] is `Some`, the reserved
    /// page-number slice at its right edge).
    pub fn footer_rect(&self) -> Rect {
        Rect::new(
            self.margins.left,
            (self.height - self.margins.bottom).max(0.0),
            (self.width - self.margins.left - self.margins.right).max(0.0),
            self.margins.bottom,
        )
    }

    /// The portion of [`PageMaster::footer_rect`] that [`PageMaster::
    /// footer`]'s own content composes into — the full zone, minus the
    /// page-number's reserved right-edge slice when a page-number token is
    /// present (so footer text and the page number never overlap).
    pub fn footer_content_rect(&self) -> Rect {
        let full = self.footer_rect();
        if self.page_number_token.is_some() {
            Rect::new(full.x, full.y, (full.width - PAGE_NUMBER_RESERVED_WIDTH).max(0.0), full.height)
        } else {
            full
        }
    }

    /// The reserved right-edge slice of [`PageMaster::footer_rect`] the
    /// page-number text paints into. Always the SAME rect regardless of
    /// whether a page-number token is actually configured (a caller only
    /// reads this when it is).
    pub fn page_number_rect(&self) -> Rect {
        let full = self.footer_rect();
        let reserved = PAGE_NUMBER_RESERVED_WIDTH.min(full.width);
        Rect::new(full.x + full.width - reserved, full.y, reserved, full.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_rect_is_unaffected_by_attaching_a_header() {
        let body_before = PageMaster::new(400.0, 600.0, Margins::uniform(40.0)).body_rect();

        let header: [BlockNode<'_>; 0] = [];
        let master_with_header = PageMaster::new(400.0, 600.0, Margins::uniform(40.0)).with_header(&header);
        assert_eq!(master_with_header.body_rect(), body_before, "margin-box header/footer must never shrink the body rect");
    }

    #[test]
    fn header_and_footer_rects_sit_inside_their_own_margin_bands() {
        let master = PageMaster::new(400.0, 600.0, Margins::uniform(40.0));
        let header = master.header_rect();
        let footer = master.footer_rect();

        assert_eq!(header.height, 40.0);
        assert_eq!(header.y, 0.0);
        assert_eq!(footer.height, 40.0);
        assert_eq!(footer.y, 560.0);
    }

    #[test]
    fn footer_content_rect_leaves_room_for_the_page_number_when_a_token_is_configured() {
        let master = PageMaster::new(400.0, 600.0, Margins::uniform(40.0)).with_page_number(PageNumberStyle::new(PageNumberFormat::Bare, 1));
        let content = master.footer_content_rect();
        let number = master.page_number_rect();

        assert!(content.x + content.width <= number.x + 0.01, "footer content must not overlap the reserved page-number slice");
        assert!((content.width + number.width - master.footer_rect().width).abs() < 1e-9);
    }

    #[test]
    fn footer_content_rect_is_the_full_zone_when_no_page_number_token_is_configured() {
        let master = PageMaster::new(400.0, 600.0, Margins::uniform(40.0));
        assert_eq!(master.footer_content_rect(), master.footer_rect());
    }

    #[test]
    fn page_number_style_formats_bare_and_of_total() {
        let bare = PageNumberStyle::new(PageNumberFormat::Bare, 1);
        assert_eq!(bare.format_for(0, 5), "1");
        assert_eq!(bare.format_for(4, 5), "5");

        let of_total = PageNumberStyle::new(PageNumberFormat::OfTotal, 1);
        assert_eq!(of_total.format_for(0, 5), "1 of 5");
        assert_eq!(of_total.format_for(4, 5), "5 of 5");
    }

    #[test]
    fn page_number_style_honors_a_non_default_start() {
        let of_total = PageNumberStyle::new(PageNumberFormat::OfTotal, 10);
        assert_eq!(of_total.format_for(0, 3), "10 of 12");
        assert_eq!(of_total.format_for(2, 3), "12 of 12");
    }

    #[test]
    fn default_columns_is_one_and_column_rect_reproduces_the_body_rect() {
        let master = PageMaster::new(400.0, 600.0, Margins::uniform(40.0));
        assert_eq!(master.columns, 1);
        assert_eq!(master.column_rect(0), master.body_rect());
        assert_eq!(master.column_width(), master.body_rect().width);
    }

    #[test]
    fn with_columns_splits_the_body_into_equal_width_columns_with_the_gap_between_them() {
        let master = PageMaster::new(424.0 + 80.0, 600.0, Margins::uniform(40.0)).with_columns(2, 24.0);
        let body = master.body_rect();
        assert_eq!(body.width, 424.0);

        let col0 = master.column_rect(0);
        let col1 = master.column_rect(1);
        assert_eq!(col0.width, 200.0);
        assert_eq!(col1.width, 200.0);
        assert_eq!(master.column_width(), 200.0);
        assert!((col1.x - (col0.x + col0.width + 24.0)).abs() < 1e-9, "column 1 must sit exactly one gap past column 0's own right edge");
    }
}
