//! Typographic presets. A preset owns the page master and the type scale.
//! `--theme` is paint and does not name a preset.

use uzor::fonts::FontFamily;
use uzor_typeset::{CellPadding, Margins, PageNumberFormat, PageNumberStyle, TableRules};

use crate::press::{Format, Sizes};

/// One paper target: margins, furniture, and the named sizes the emitters share.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub margins: Margins,
    pub footer_from_edge: f64,
    pub sizes: Sizes,
    pub body_leading: f64,
    pub paragraph_spacing: f64,
    pub justify: bool,
    pub hyphenate: bool,
    pub h1_rule: bool,
    pub table_rules: TableRules,
    pub cell_padding: CellPadding,
    pub page_number: PageNumberStyle,
    pub baseline: Option<f64>,
    /// Fixed text-block width. Side margins are computed from the paper.
    pub measure: Option<f64>,
    pub columns: usize,
    pub column_gap: f64,
    pub first_indent: f64,
    /// Replaces the theme's text role. Headings stay on the theme display face.
    pub text_family: Option<FontFamily>,
    pub extras: SheetExtras,
}

/// Space around a heading in pt. The PDF realises `before` as a spacer above
/// the heading and whatever `after` exceeds the sheet's paragraph spacing as
/// none (paragraph spacing is the floor); the DOCX writes both verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct HeadingSpace {
    pub before: f64,
    pub after: f64,
}

/// Long-document refinements. `SheetExtras::default()` is "all off", the
/// behaviour every preset had before them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SheetExtras {
    /// Space before/after H1, H2, H3. All zero: no explicit heading spacing.
    pub heading_space: [HeadingSpace; 3],
    /// Line height of H2/H3 paragraphs. `None`: the font's natural height.
    pub heading_leading: Option<f64>,
    /// Every H1 starts a page (except the first block of the body).
    pub h1_page_break: bool,
    /// Line height of list items. `None`: the font's natural height.
    pub list_leading: Option<f64>,
    /// List items follow the sheet's `justify` / `hyphenate` flags.
    pub justify_lists: bool,
    /// DOCX tables take `cell_padding` as cell margins and a fixed layout.
    pub docx_cell_margins: bool,
}

impl Sheet {
    pub fn margins_for(&self, page_width: f64) -> Margins {
        match self.measure {
            Some(measure) => {
                let side = ((page_width - measure) / 2.0).max(0.0);
                Margins::new(self.margins.top, side, self.margins.bottom, side)
            }
            None => self.margins,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Preset {
    pub page: Sheet,
    pub slides: Option<Sheet>,
}

impl Preset {
    pub fn sheet(&self, format: Format) -> Option<&Sheet> {
        match format {
            Format::Doc | Format::Report => Some(&self.page),
            Format::Deck => self.slides.as_ref(),
        }
    }

    pub fn column() -> Self {
        Self {
            page: Sheet {
                margins: Margins::new(64.0, 64.0, 72.0, 64.0),
                footer_from_edge: 18.0,
                sizes: Sizes {
                    h1: 24.0,
                    h2: 18.0,
                    h3: 15.0,
                    body: 13.0,
                    list_item: 13.0,
                    table_header: 11.0,
                    table_cell: 11.0,
                    caption: 10.5,
                    footer: 9.0,
                    callout: 13.0,
                    kpi_value: 32.0,
                    kpi_label: 10.5,
                    cover_title: 44.0,
                    cover_sub: 16.0,
                    cover_meta: 11.0,
                    diagram_title: 12.0,
                    diagram_subtitle: 10.5,
                },
                body_leading: 18.0,
                paragraph_spacing: 18.0,
                justify: false,
                hyphenate: false,
                h1_rule: true,
                table_rules: TableRules::Horizontal,
                cell_padding: CellPadding::new(5.0, 7.0),
                page_number: PageNumberStyle::new(PageNumberFormat::Bare, 1).with_suppress_if_single(true).with_suppress_title(true),
                baseline: Some(18.0),
                measure: None,
                columns: 1,
                column_gap: 0.0,
                first_indent: 0.0,
                text_family: None,
                extras: SheetExtras::default(),
            },
            slides: Some(Sheet {
                margins: Margins::new(48.0, 56.0, 56.0, 56.0),
                footer_from_edge: 16.0,
                sizes: Sizes {
                    h1: 30.0,
                    h2: 20.0,
                    h3: 16.0,
                    body: 15.0,
                    list_item: 15.0,
                    table_header: 13.0,
                    table_cell: 13.0,
                    caption: 11.0,
                    footer: 10.0,
                    callout: 15.0,
                    kpi_value: 44.0,
                    kpi_label: 12.0,
                    cover_title: 56.0,
                    cover_sub: 22.0,
                    cover_meta: 13.0,
                    diagram_title: 13.0,
                    diagram_subtitle: 10.5,
                },
                body_leading: 20.0,
                paragraph_spacing: 10.0,
                justify: false,
                hyphenate: false,
                h1_rule: true,
                table_rules: TableRules::Horizontal,
                cell_padding: CellPadding::new(6.0, 8.0),
                page_number: PageNumberStyle::new(PageNumberFormat::OfTotal, 1).with_suppress_if_single(true).with_suppress_title(true),
                baseline: Some(20.0),
                measure: None,
                columns: 1,
                column_gap: 0.0,
                first_indent: 0.0,
                text_family: None,
                extras: SheetExtras::default(),
            }),
        }
    }

    pub fn notice() -> Self {
        let sizes = Sizes {
            h1: 36.0,
            h2: 27.0,
            h3: 18.0,
            body: 14.25,
            list_item: 14.25,
            table_header: 14.25,
            table_cell: 14.25,
            caption: 12.0,
            footer: 12.0,
            callout: 14.25,
            kpi_value: 36.0,
            kpi_label: 12.0,
            cover_title: 36.0,
            cover_sub: 18.0,
            cover_meta: 12.0,
            diagram_title: 12.0,
            diagram_subtitle: 10.5,
        };
        Self {
            page: Sheet {
                margins: Margins::new(72.0, 72.0, 72.0, 72.0),
                footer_from_edge: 20.0,
                sizes: sizes.clone(),
                body_leading: 18.75,
                paragraph_spacing: 15.0,
                justify: false,
                hyphenate: false,
                h1_rule: false,
                table_rules: TableRules::Horizontal,
                cell_padding: CellPadding::new(15.0, 7.5),
                page_number: PageNumberStyle::new(PageNumberFormat::OfTotal, 1).with_suppress_if_single(true).with_suppress_title(true),
                baseline: Some(18.75),
                measure: None,
                columns: 1,
                column_gap: 0.0,
                first_indent: 0.0,
                text_family: None,
                extras: SheetExtras::default(),
            },
            slides: Some(Sheet {
                margins: Margins::new(48.0, 48.0, 48.0, 48.0),
                footer_from_edge: 16.0,
                sizes,
                body_leading: 18.75,
                paragraph_spacing: 12.0,
                justify: false,
                hyphenate: false,
                h1_rule: false,
                table_rules: TableRules::Horizontal,
                cell_padding: CellPadding::new(15.0, 7.5),
                page_number: PageNumberStyle::new(PageNumberFormat::OfTotal, 1).with_suppress_if_single(true).with_suppress_title(true),
                baseline: Some(18.75),
                measure: None,
                columns: 1,
                column_gap: 0.0,
                first_indent: 0.0,
                text_family: None,
                extras: SheetExtras::default(),
            }),
        }
    }

    pub fn filing() -> Self {
        Self {
            page: Sheet {
                margins: Margins::new(56.0, 44.0, 64.0, 44.0),
                footer_from_edge: 28.0,
                sizes: Sizes {
                    h1: 16.0,
                    h2: 13.0,
                    h3: 11.0,
                    body: 11.0,
                    list_item: 11.0,
                    table_header: 8.0,
                    table_cell: 8.0,
                    caption: 10.0,
                    footer: 9.0,
                    callout: 11.0,
                    kpi_value: 20.0,
                    kpi_label: 10.0,
                    cover_title: 16.0,
                    cover_sub: 13.0,
                    cover_meta: 11.0,
                    diagram_title: 11.0,
                    diagram_subtitle: 10.0,
                },
                body_leading: 15.0,
                paragraph_spacing: 0.0,
                justify: false,
                hyphenate: true,
                h1_rule: false,
                table_rules: TableRules::Horizontal,
                cell_padding: CellPadding::new(6.0, 3.0),
                page_number: PageNumberStyle::new(PageNumberFormat::Bare, 1).with_suppress_if_single(true).with_suppress_title(true),
                baseline: Some(15.0),
                measure: None,
                columns: 2,
                column_gap: 18.0,
                first_indent: 14.0,
                text_family: Some(FontFamily::SourceSerif4),
                extras: SheetExtras::default(),
            },
            slides: None,
        }
    }

    /// Justified one-column document. Margins are 20, 18, 20, 22 mm
    /// (top, right, bottom, left). Body is 11 pt on a 1.15 × 1.2 em line,
    /// headings are 16 / 13 / 12, page numbers are bare, and there is no
    /// slide sheet.
    pub fn long() -> Self {
        const MM: f64 = 72.0 / 25.4;
        let body = 11.0;
        let line = body * 1.2 * 1.15;
        Self {
            page: Sheet {
                margins: Margins::new(20.0 * MM, 18.0 * MM, 20.0 * MM, 22.0 * MM),
                footer_from_edge: 18.0,
                sizes: Sizes {
                    h1: 16.0,
                    h2: 13.0,
                    h3: 12.0,
                    body,
                    list_item: 11.0,
                    table_header: 10.0,
                    table_cell: 10.0,
                    caption: 9.0,
                    footer: 9.0,
                    callout: 11.0,
                    kpi_value: 20.0,
                    kpi_label: 10.0,
                    cover_title: 22.0,
                    cover_sub: 14.0,
                    cover_meta: 11.0,
                    diagram_title: 12.0,
                    diagram_subtitle: 10.0,
                },
                body_leading: line,
                paragraph_spacing: 8.0,
                justify: true,
                hyphenate: true,
                h1_rule: false,
                table_rules: TableRules::Horizontal,
                cell_padding: CellPadding::new(4.0, 6.0),
                page_number: PageNumberStyle::new(PageNumberFormat::Bare, 1),
                baseline: None,
                measure: None,
                columns: 1,
                column_gap: 0.0,
                first_indent: 0.0,
                text_family: None,
                extras: SheetExtras {
                    heading_space: [HeadingSpace::default(); 3],
                    heading_leading: Some(line),
                    h1_page_break: false,
                    list_leading: Some(line),
                    justify_lists: false,
                    docx_cell_margins: false,
                },
            },
            slides: None,
        }
    }

    /// One dense column: 48 pt margins, 10 pt body on a 13 pt line,
    /// headings 14 / 12 / 11, hyphenated, no slide sheet.
    pub fn dense() -> Self {
        Self {
            page: Sheet {
                margins: Margins::new(48.0, 48.0, 48.0, 48.0),
                footer_from_edge: 16.0,
                sizes: Sizes {
                    h1: 14.0,
                    h2: 12.0,
                    h3: 11.0,
                    body: 10.0,
                    list_item: 10.0,
                    table_header: 9.0,
                    table_cell: 9.0,
                    caption: 9.0,
                    footer: 8.0,
                    callout: 10.0,
                    kpi_value: 24.0,
                    kpi_label: 9.0,
                    cover_title: 28.0,
                    cover_sub: 14.0,
                    cover_meta: 10.0,
                    diagram_title: 11.0,
                    diagram_subtitle: 9.0,
                },
                body_leading: 13.0,
                paragraph_spacing: 4.0,
                justify: false,
                hyphenate: true,
                h1_rule: false,
                table_rules: TableRules::Horizontal,
                cell_padding: CellPadding::new(4.0, 3.0),
                page_number: PageNumberStyle::new(PageNumberFormat::Bare, 1).with_suppress_if_single(true).with_suppress_title(true),
                baseline: Some(13.0),
                measure: None,
                columns: 1,
                column_gap: 0.0,
                first_indent: 0.0,
                text_family: None,
                extras: SheetExtras::default(),
            },
            slides: None,
        }
    }
}
