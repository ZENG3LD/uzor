//! [`Theme`] — 3-tier design tokens (design doc §5): a primitive
//! [`BrandTokens`] tier (raw report-chrome colors + a categorical
//! series-color palette + available font files — no semantic meaning
//! attached yet), a semantic tier ([`FontRole`]/[`ColorRole`] as lookup
//! keys, resolved via [`DesignTokens`]/`BrandTokens` directly), and a
//! component [`ComponentStyle`] tier (figure/table styling expressed
//! purely in terms of the semantic roles, never a third independent copy
//! of a raw color/font).
//!
//! Rendering a `Block::Figure` never constructs a `FigureTheme` directly
//! — it always goes through [`Theme::figure_theme`], so a theme edit
//! propagates to every figure block through the SAME master->layout->
//! instance chain text/tables already resolve through (design doc §5's
//! own closing paragraph).
//!
//! ## Divergences from the design doc (report, not silent)
//!
//! 1. **`DesignTokens`'s fields are NOT literally typed `FontRole`/
//!    `ColorRole`, as the doc's own §5 sketch shows.** A literal reading
//!    (`heading_font: FontRole`) is circular — `FontRole` is also the
//!    very enum [`Theme::font_spec`] takes as its OWN lookup key, so a
//!    "heading" field typed as that same enum resolves nothing on its
//!    own (no size, no reference back into `BrandTokens`). Implemented
//!    here instead: `FontRole`/`ColorRole` are plain semantic-tag enums
//!    used purely as lookup keys; `DesignTokens` maps each `FontRole` to
//!    a concrete `BrandTokens::font_files` slot index plus the one thing
//!    brand tier deliberately doesn't carry — point size (a brand font
//!    file is reused at several sizes). `Theme::font_spec(role)` still
//!    matches the doc's own top-level method signature exactly.
//! 2. **`BrandTokens` gained named chrome-color fields (`ink`/`muted`/
//!    `accent`/`background`) instead of the doc's literal single
//!    `palette: Vec<u32>` field.** Once actually wired to
//!    `Theme::figure_theme`, "palette" turned out to need TWO genuinely
//!    different color sets: `uzor_figures::FigureTheme::palette` (a
//!    categorical, per-series/category DATA color set, kept here as
//!    `BrandTokens::categorical_palette`, bridged verbatim — never a
//!    second independent copy) versus a small, fixed set of
//!    report/deck-CHROME colors (background/ink/muted/accent) that are
//!    NOT data-series colors at all. Conflating the two under one
//!    index-addressed `Vec<u32>` would mean an "accent" role and a
//!    "third category color" could collide by construction; naming the
//!    4 chrome roles as their own fields removes that ambiguity (a small,
//!    closed role set is exactly what a plain struct field expresses more
//!    precisely than a Vec index would — no semantic indirection needed
//!    for `ColorRole` at all, since brand tier already names each role
//!    1:1). `DesignTokens` therefore only resolves FONT roles (a genuine
//!    choice: which brand font slot + size to use); `Theme::color_rgb`
//!    reads straight from `BrandTokens`'s own named fields.

use uzor::fonts::FontFamily;
use uzor_figures::FigureTheme;
use uzor_text::FontSpec;

/// One available font choice in a brand's asset set — family + weight/
/// style, no size (size is a semantic-tier decision, see [`DesignTokens`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFileRef {
    pub family: FontFamily,
    pub bold: bool,
    pub italic: bool,
}

impl FontFileRef {
    pub fn new(family: FontFamily, bold: bool, italic: bool) -> Self {
        Self { family, bold, italic }
    }
}

/// Primitive tier (design doc §5): raw brand assets, no semantic meaning
/// attached yet. `categorical_palette` is a full categorical color set —
/// the SAME convention `uzor_figures::FigureTheme::palette` already uses
/// (indexed by series/category position) — bridged into
/// [`Theme::figure_theme`] verbatim, never re-derived. `ink`/`muted`/
/// `accent`/`background` are the small, fixed set of report/deck-chrome
/// colors (see this module's own "Divergences" doc comment for why they
/// are named fields, not a second Vec index).
#[derive(Debug, Clone)]
pub struct BrandTokens {
    pub ink: u32,
    pub muted: u32,
    pub accent: u32,
    pub background: u32,
    pub categorical_palette: Vec<u32>,
    pub font_files: Vec<FontFileRef>,
}

/// Semantic-tier lookup key: which named role a font resolves for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontRole {
    Heading,
    Body,
    Caption,
}

/// Semantic-tier lookup key: which named report-chrome color role
/// resolves. Maps 1:1 onto [`BrandTokens`]'s own named chrome fields — see
/// this module's own "Divergences" doc comment for why color needs no
/// further semantic indirection the way fonts do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorRole {
    Ink,
    Muted,
    Accent,
    Background,
}

/// Semantic tier (design doc §5): which [`BrandTokens::font_files`] slot
/// + explicit point size each [`FontRole`] resolves to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignTokens {
    pub heading_font: usize,
    pub heading_size_px: f64,
    pub body_font: usize,
    pub body_size_px: f64,
    pub caption_font: usize,
    pub caption_size_px: f64,
}

/// A resolved font role's own font-file slot + size — everything
/// [`Theme::font_spec`] needs, bundled so [`Theme::figure_theme`]'s label
/// font can reuse the exact same resolution path.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedFontRole {
    file: FontFileRef,
    size_px: f64,
}

/// Component tier (design doc §5): figure styling expressed PURELY in
/// terms of the semantic roles above — never a third, independent copy of
/// a raw color/font.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FigureThemeTokens {
    pub background: ColorRole,
    pub axis_color: ColorRole,
    pub grid_color: ColorRole,
    pub label_color: ColorRole,
    pub label_font: FontRole,
    pub label_size_px: f64,
}

/// A run of text styled purely through the semantic tier — e.g. a
/// table's header row (design doc §5's own `ComponentStyle.table_header`
/// field). **Not auto-applied to any table row this phase** — `TableRow`
/// has no `is_header` flag yet (P1's own divergence log already noted
/// header-row semantics are unbuilt); this type exists, resolves, and is
/// tested, ready for whichever future phase marks a row as a header.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub font: FontRole,
    pub size_px: f64,
    pub color: ColorRole,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComponentStyle {
    pub figure_theme: FigureThemeTokens,
    pub table_header: TextStyle,
}

/// The full 3-tier theme (design doc §5). See [`Theme::figure_theme`]/
/// [`Theme::font_spec`]/[`Theme::color_hex`] for the resolution chain
/// every consumer (figures, paragraphs, page-number text) goes through.
#[derive(Debug, Clone)]
pub struct Theme {
    pub brand: BrandTokens,
    pub design: DesignTokens,
    pub components: ComponentStyle,
}

/// Fallback used ONLY when a `DesignTokens` font index points outside its
/// `BrandTokens::font_files` vec's bounds (a malformed `Theme`, never
/// produced by this crate's own [`Theme::light_report`]/
/// [`Theme::dark_deck`] presets or any test fixture) — a defensive floor,
/// not a feature: a render should degrade to a visible neutral rather
/// than panic mid-page.
const FALLBACK_FONT: FontFileRef = FontFileRef { family: FontFamily::Roboto, bold: false, italic: false };

/// Bare CSS family name for `family` — mirrors `uzor_text::FontSpec`'s own
/// private `family_name()` (the two crates share `uzor::fonts::FontFamily`
/// but neither the type itself nor `FontSpec` exposes a public
/// family-name-only accessor), needed here so
/// [`Theme::figure_theme`]'s own `label_font_family` field carries a bare
/// family string independent of `label_spec.to_css_font()`'s composed
/// `"<weight> <size>px <family>"` shape.
fn font_family_name(family: FontFamily) -> &'static str {
    match family {
        FontFamily::Roboto => "Roboto",
        FontFamily::PtRootUi => "PT Root UI",
        FontFamily::JetBrainsMono => "JetBrains Mono",
    }
}

impl Theme {
    fn resolve_font_role(&self, role: FontRole) -> ResolvedFontRole {
        let (idx, size_px) = match role {
            FontRole::Heading => (self.design.heading_font, self.design.heading_size_px),
            FontRole::Body => (self.design.body_font, self.design.body_size_px),
            FontRole::Caption => (self.design.caption_font, self.design.caption_size_px),
        };
        let file = self.brand.font_files.get(idx).copied().unwrap_or(FALLBACK_FONT);
        ResolvedFontRole { file, size_px }
    }

    /// Resolve `role` to a concrete `0xRRGGBB` value — reads straight
    /// from [`BrandTokens`]'s own named chrome fields (this module's own
    /// "Divergences" doc comment).
    pub fn color_rgb(&self, role: ColorRole) -> u32 {
        match role {
            ColorRole::Ink => self.brand.ink,
            ColorRole::Muted => self.brand.muted,
            ColorRole::Accent => self.brand.accent,
            ColorRole::Background => self.brand.background,
        }
    }

    /// Resolve `role` to a `"#rrggbb"` CSS-hex string — the same
    /// convention every `RenderContext::set_fill_color`/`set_stroke_color`
    /// call site in this crate already takes.
    pub fn color_hex(&self, role: ColorRole) -> String {
        format!("#{:06x}", self.color_rgb(role) & 0xff_ffff)
    }

    /// Resolve `role` to a concrete [`uzor_text::FontSpec`] (design doc
    /// §5's own `Theme::font_spec` signature, verbatim).
    pub fn font_spec(&self, role: FontRole) -> FontSpec {
        let resolved = self.resolve_font_role(role);
        let mut spec = FontSpec::new(resolved.file.family, resolved.size_px);
        if resolved.file.bold {
            spec = spec.bold();
        }
        if resolved.file.italic {
            spec = spec.italic();
        }
        spec
    }

    /// Resolve this theme's component-tier figure styling into a real
    /// [`uzor_figures::FigureTheme`] (design doc §5's own
    /// `Theme::figure_theme` signature, verbatim) — every `Block::Figure`
    /// paints through THIS, never a hand-built `FigureTheme` (design doc
    /// §5's closing paragraph).
    pub fn figure_theme(&self) -> FigureTheme {
        let tokens = &self.components.figure_theme;
        let label = self.resolve_font_role(tokens.label_font);
        let mut label_spec = FontSpec::new(label.file.family, tokens.label_size_px);
        if label.file.bold {
            label_spec = label_spec.bold();
        }
        if label.file.italic {
            label_spec = label_spec.italic();
        }
        FigureTheme {
            background: self.color_hex(tokens.background),
            axis_color: self.color_hex(tokens.axis_color),
            grid_color: self.color_hex(tokens.grid_color),
            label_color: self.color_hex(tokens.label_color),
            label_font: label_spec.to_css_font(),
            // Bare family name only (no size/weight tokens) — resolved
            // through the SAME `label` role `label_spec` above already
            // derived from, so a figure that needs a differently-sized
            // variant of the SAME family (currently only `KpiFigure`'s
            // headline number) never has to re-parse `label_font`'s own
            // composed CSS string.
            label_font_family: font_family_name(label.file.family).to_owned(),
            palette: self.brand.categorical_palette.iter().map(|c| format!("#{:06x}", c & 0xff_ffff)).collect(),
            // `positive`/`negative` (business-chart semantic colors, added
            // additively to `uzor_figures::FigureTheme` for
            // `WaterfallFigure`) bridge through the SAME shared
            // categorical palette this theme already mirrors verbatim
            // (`shared_categorical_palette`, indices 2/7 are the
            // green/red hues) — never a second, unrelated color source.
            positive: self.brand.categorical_palette.get(2).map(|c| format!("#{:06x}", c & 0xff_ffff)).unwrap_or_else(|| "#5cb87a".to_owned()),
            negative: self.brand.categorical_palette.get(7).map(|c| format!("#{:06x}", c & 0xff_ffff)).unwrap_or_else(|| "#e0555a".to_owned()),
            // Hover/selection highlight — this theme carries no dedicated
            // brand role for it (no figure block in this crate's own
            // showcase drives a hover overlay), so it resolves to the
            // theme's own `ColorRole::Accent` (a real, already-resolved
            // brand color, never a second hardcoded literal).
            highlight: self.color_hex(ColorRole::Accent),
        }
    }

    /// The shared 10-color categorical set `uzor_figures::FigureTheme`
    /// itself ships (`uzor-figures/src/theme.rs`'s own `PALETTE` const) —
    /// reused verbatim here so `light_report`/`dark_deck` figures read
    /// identically to every other figure demo in this workspace.
    fn shared_categorical_palette() -> Vec<u32> {
        vec![0x4d90fe, 0xe0703c, 0x5cb87a, 0xc94f7c, 0xd9b64e, 0x7e6bd9, 0x3fb6c9, 0xe0555a, 0x8fbf5f, 0xc78bd9]
    }

    fn shared_design_tokens() -> DesignTokens {
        DesignTokens {
            heading_font: 0,
            heading_size_px: 22.0,
            body_font: 1,
            body_size_px: 14.0,
            caption_font: 1,
            caption_size_px: 11.0,
        }
    }

    fn shared_font_files() -> Vec<FontFileRef> {
        vec![FontFileRef::new(FontFamily::Roboto, true, false), FontFileRef::new(FontFamily::Roboto, false, false)]
    }

    fn shared_components() -> ComponentStyle {
        ComponentStyle {
            figure_theme: FigureThemeTokens {
                background: ColorRole::Background,
                axis_color: ColorRole::Muted,
                grid_color: ColorRole::Muted,
                label_color: ColorRole::Ink,
                label_font: FontRole::Caption,
                label_size_px: 11.0,
            },
            table_header: TextStyle { font: FontRole::Heading, size_px: 13.0, color: ColorRole::Ink },
        }
    }

    /// A light, report-shaped preset (white background, dark ink) — one
    /// of this crate's own two headless-proof themes (see `render.rs`'s
    /// P2 test). Real, usable defaults, not test-only scaffolding.
    pub fn light_report() -> Self {
        Self {
            brand: BrandTokens {
                ink: 0x111111,
                muted: 0x4a5060,
                accent: 0x0b5fff,
                background: 0xffffff,
                categorical_palette: Self::shared_categorical_palette(),
                font_files: Self::shared_font_files(),
            },
            design: Self::shared_design_tokens(),
            components: Self::shared_components(),
        }
    }

    /// A dark, deck-shaped preset (near-black background, light ink) —
    /// this crate's other headless-proof theme. Shares EVERY font-size
    /// field with [`Theme::light_report`] on purpose (design doc §7 P2
    /// risk note: "theme never changes metrics") — only
    /// [`BrandTokens`]'s chrome colors differ.
    pub fn dark_deck() -> Self {
        Self {
            brand: BrandTokens {
                ink: 0xe9ecf5,
                muted: 0x9aa0ac,
                accent: 0x5cc9ff,
                background: 0x14161c,
                categorical_palette: Self::shared_categorical_palette(),
                font_files: Self::shared_font_files(),
            },
            design: Self::shared_design_tokens(),
            components: Self::shared_components(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_report_and_dark_deck_share_identical_font_sizes_but_different_colors() {
        let light = Theme::light_report();
        let dark = Theme::dark_deck();

        assert_eq!(light.design.heading_size_px, dark.design.heading_size_px);
        assert_eq!(light.design.body_size_px, dark.design.body_size_px);
        assert_eq!(
            light.font_spec(FontRole::Body).size_px,
            dark.font_spec(FontRole::Body).size_px,
            "a theme must never change layout-affecting metrics on its own"
        );

        assert_ne!(light.color_hex(ColorRole::Background), dark.color_hex(ColorRole::Background));
        assert_ne!(light.color_hex(ColorRole::Ink), dark.color_hex(ColorRole::Ink));
    }

    #[test]
    fn font_spec_resolves_bold_heading_and_regular_body_from_the_same_theme() {
        let theme = Theme::light_report();
        assert!(theme.font_spec(FontRole::Heading).bold);
        assert!(!theme.font_spec(FontRole::Body).bold);
    }

    #[test]
    fn color_hex_is_a_six_digit_css_hash_string() {
        let theme = Theme::light_report();
        let hex = theme.color_hex(ColorRole::Ink);
        assert_eq!(hex.len(), 7);
        assert!(hex.starts_with('#'));
        assert_eq!(hex, "#111111");
    }

    #[test]
    fn figure_theme_palette_length_matches_brand_categorical_palette_length() {
        let theme = Theme::light_report();
        assert_eq!(theme.figure_theme().palette.len(), theme.brand.categorical_palette.len());
    }

    #[test]
    fn figure_theme_never_constructs_its_own_colors_it_always_resolves_through_the_theme() {
        let light = Theme::light_report();
        let dark = Theme::dark_deck();
        assert_ne!(light.figure_theme().background, dark.figure_theme().background, "figure theme must follow the page theme");
    }

    #[test]
    fn an_out_of_bounds_design_token_font_index_falls_back_rather_than_panicking() {
        let mut theme = Theme::light_report();
        theme.design.body_font = 99; // deliberately out of range
        let spec = theme.font_spec(FontRole::Body); // must not panic
        assert_eq!(spec.family, FontFamily::Roboto);
    }

    #[test]
    fn figure_theme_label_font_family_is_bare_never_carries_a_size_token() {
        let theme = Theme::light_report();
        let figure_theme = theme.figure_theme();
        assert!(!figure_theme.label_font_family.contains("px"), "label_font_family must be a bare family name, got {}", figure_theme.label_font_family);
        assert!(figure_theme.label_font.ends_with(&figure_theme.label_font_family), "label_font's own composed string must end with the same bare family");
    }

    #[test]
    fn figure_theme_highlight_resolves_through_the_theme_never_a_hardcoded_literal() {
        let light = Theme::light_report();
        let dark = Theme::dark_deck();
        assert_eq!(light.figure_theme().highlight, light.color_hex(ColorRole::Accent));
        assert_eq!(dark.figure_theme().highlight, dark.color_hex(ColorRole::Accent));
    }
}
