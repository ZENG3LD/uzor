//! `GraphTheme` — every hardcoded paint color/font in `src/render.rs`,
//! routed through one caller-configurable struct (graph-strengthening arc
//! Wave G2 / 2D quality audit A3). Mirrors `uzor_figures::theme::FigureTheme`'s
//! own shape (flat fields, `dark()`/`light()`/`high_contrast()` presets,
//! `Default == dark()`) deliberately, rather than inventing a second,
//! unrelated theming vocabulary in this workspace — this crate already
//! depends on `uzor-figures` (the `FocusSet`/`fill_text_with_halo`/
//! `draw_tooltip` re-point precedent), and `GraphTheme::hover_card` embeds
//! a real `FigureTheme` verbatim rather than re-deriving a parallel set of
//! hover-card colors (`render::draw_hover_card` already delegates its
//! whole paint job to `uzor_figures::guide::tooltip::draw_tooltip`, which
//! only understands `FigureTheme`).
//!
//! **Doctrine**: [`GraphTheme::dark`] is byte-identical to the literals
//! `render.rs` hardcoded before this wave (proven by
//! `dark_matches_the_pre_existing_hardcoded_literals` below) — the crate's
//! single live consumer (`force_graph_demo`, `#0d0f14` canvas) sees zero
//! rendered-output change. [`GraphTheme::light`]/[`GraphTheme::high_contrast`]
//! are new, opt-in presets a caller must explicitly select.

use uzor_figures::scale::color::CategoricalScale;
use uzor_figures::theme::FigureTheme;

/// The crate's own pre-existing 10-color categorical palette (was a
/// private `const PALETTE` inside `render::category_color` before this
/// wave) — moved here so it can back both [`default_category_palette`]
/// (the default `GraphTheme::category_palette`) and
/// `uzor-figures::theme::FigureTheme`'s own doc-comment claim that the two
/// crates "share one visual identity" (that palette is copy-pasted there
/// too; kept independent on purpose — `uzor-figures` must not depend on
/// `uzor-graph`, see that crate's own Forbidden list).
pub const DEFAULT_CATEGORY_PALETTE: [&str; 10] =
    ["#4d90fe", "#e0703c", "#5cb87a", "#c94f7c", "#d9b64e", "#7e6bd9", "#3fb6c9", "#e0555a", "#8fbf5f", "#c78bd9"];

/// [`GraphTheme::dark`]/`light`/`high_contrast`'s own shared default
/// categorical palette — see [`DEFAULT_CATEGORY_PALETTE`]. A caller
/// wanting the colorblind-safe Okabe-Ito set instead selects
/// `CategoricalScale::default_palette()` via
/// `GraphTheme { category_palette: CategoricalScale::default_palette(), ..GraphTheme::dark() }`
/// (or any other explicit `CategoricalScale::new(..)`).
pub fn default_category_palette() -> CategoricalScale {
    CategoricalScale::new(DEFAULT_CATEGORY_PALETTE.iter().map(|&s| s.to_owned()).collect())
}

/// Every paint color/font `crate::render`'s 2D draw functions read, plus
/// the small set of ring/label offset+width geometry that's inseparable
/// from its own color decision (e.g. the selection ring's stroke width
/// and radius offset are as much a "how loud is this ring" choice as its
/// color). Threaded through [`crate::render::DrawContext::theme`] — every
/// public draw fn in `render.rs` reads from it instead of a hardcoded
/// literal.
///
/// `GraphEngine` owns one `theme: GraphTheme` field with a
/// `theme()`/`set_theme()` getter/setter pair, the exact same shape
/// [`crate::engine::GraphEngine::label_halo`]/`set_label_halo` already
/// established — see that field's own doc comment for why this crate
/// follows the "private field + accessor pair" convention instead of
/// exposing public struct fields directly on the engine.
#[derive(Debug, Clone)]
pub struct GraphTheme {
    // ── Selection / hover rings (`render::draw_nodes`) ─────────────────
    pub selection_ring_color: String,
    pub selection_ring_width: f64,
    /// Added to the node's own screen radius for the ring's paint radius.
    pub selection_ring_offset_px: f64,
    pub hover_ring_color: String,
    pub hover_ring_width: f64,
    pub hover_ring_offset_px: f64,

    // ── Node labels (`render::draw_nodes`) ──────────────────────────────
    pub label_fill: String,
    pub label_font: String,
    /// Added to `node_screen_radius` for the label's anchor X (so it never
    /// starts drawing inside the node's own circle).
    pub label_offset_x: f64,
    pub label_offset_y: f64,

    // ── Cluster supernode overlay (`render::{draw_cluster_edges,
    // draw_cluster_supernodes}`) ────────────────────────────────────────
    pub cluster_accent: String,
    pub cluster_ring_inner_offset_px: f64,
    pub cluster_ring_outer_offset_px: f64,
    pub cluster_ring_width: f64,
    /// The "×N" member-count label's own fill color (distinct from
    /// [`GraphTheme::cluster_accent`]'s ring/edge stroke color).
    pub cluster_label_color: String,
    pub cluster_label_offset_x: f64,
    pub cluster_label_offset_y: f64,
    /// Upper bound on an aggregated cross-cluster edge's stroke width
    /// (`1.0 + summed_weight.sqrt()`, capped here) — was `.min(6.0)`.
    pub cluster_edge_width_cap: f64,

    // ── Dim/focus-fade paint (`render::{draw_edges, draw_nodes}`) ──────
    /// A dimmed (focus-inactive) node's fill color — distinct from an
    /// ordinary node's [`GraphTheme::category_palette`] color.
    pub dim_node_fill: String,
    /// Opacity applied to every dimmed node/edge while a [`crate::engine::
    /// GraphEngine::focus`] set is active (was the module-private
    /// `DIM_ALPHA` constant).
    pub dim_alpha: f64,

    // ── Edges (`render::draw_edges`) ────────────────────────────────────
    pub edge_color: String,
    pub edge_width: f64,
    pub edge_dim_color: String,
    pub edge_dim_width: f64,

    // ── Box-select rubber-band overlay (`render::draw_box_select_rect`) ─
    pub box_select_fill: String,
    pub box_select_fill_alpha: f64,
    pub box_select_border: String,
    pub box_select_border_width: f64,

    // ── Hover info card (`render::draw_hover_card`) ─────────────────────
    /// The full `uzor_figures::theme::FigureTheme` `draw_tooltip` paints
    /// the hover card with — was an unconditional `FigureTheme::dark()`
    /// literal inside `draw_hover_card` itself.
    pub hover_card: FigureTheme,

    // ── Node fill (`render::{category_color, draw_nodes}`) ─────────────
    /// Category -> color assignment (`render::category_color`'s hash ->
    /// palette-index lookup) — was a fixed, non-overridable 10-color
    /// `&'static [&'static str]` baked into the render layer. Default:
    /// [`default_category_palette`] (byte-identical to the pre-existing
    /// hardcoded palette). A caller wanting the colorblind-safe Okabe-Ito
    /// set assigns `CategoricalScale::default_palette()` here instead.
    pub category_palette: CategoricalScale,
}

impl GraphTheme {
    /// Dark canvas (the crate's own pre-existing, only-ever-shipped
    /// aesthetic — `force-graph-demo`'s `#0d0f14` background). Every field
    /// is byte-identical to the literal `render.rs` hardcoded before this
    /// wave — see `dark_matches_the_pre_existing_hardcoded_literals`.
    pub fn dark() -> Self {
        Self {
            selection_ring_color: "#ffffff".to_owned(),
            selection_ring_width: 2.0,
            selection_ring_offset_px: 2.0,
            hover_ring_color: "#ffd76a".to_owned(),
            hover_ring_width: 1.5,
            hover_ring_offset_px: 1.5,

            label_fill: "#e6e6ea".to_owned(),
            label_font: "11px sans-serif".to_owned(),
            label_offset_x: 4.0,
            label_offset_y: 4.0,

            cluster_accent: "#c9a94e".to_owned(),
            cluster_ring_inner_offset_px: 3.0,
            cluster_ring_outer_offset_px: 7.0,
            cluster_ring_width: 2.0,
            cluster_label_color: "#f0e6c0".to_owned(),
            cluster_label_offset_x: 10.0,
            cluster_label_offset_y: 4.0,
            cluster_edge_width_cap: 6.0,

            dim_node_fill: "#6b7280".to_owned(),
            dim_alpha: 0.15,

            edge_color: "#7c8496".to_owned(),
            edge_width: 1.7,
            edge_dim_color: "#5a6070".to_owned(),
            edge_dim_width: 1.3,

            box_select_fill: "#4d90fe".to_owned(),
            box_select_fill_alpha: 0.15,
            box_select_border: "#7fb2ff".to_owned(),
            box_select_border_width: 1.0,

            hover_card: FigureTheme::dark(),
            category_palette: default_category_palette(),
        }
    }

    /// Light canvas — the defect [`GraphTheme`] itself exists to close (2D
    /// audit A3): the pre-existing `dark()`-only literals (a `"#ffffff"`
    /// selection ring chief among them) are invisible against a light
    /// background. Every color below is chosen to contrast against a
    /// white/near-white canvas, mirroring `FigureTheme::light()`'s own
    /// dark-navy `highlight` fix for the identical defect class.
    pub fn light() -> Self {
        Self {
            selection_ring_color: "#1a1a2e".to_owned(),
            selection_ring_width: 2.0,
            selection_ring_offset_px: 2.0,
            hover_ring_color: "#b8790f".to_owned(),
            hover_ring_width: 1.5,
            hover_ring_offset_px: 1.5,

            label_fill: "#1f2430".to_owned(),
            label_font: "11px sans-serif".to_owned(),
            label_offset_x: 4.0,
            label_offset_y: 4.0,

            cluster_accent: "#8a6d1f".to_owned(),
            cluster_ring_inner_offset_px: 3.0,
            cluster_ring_outer_offset_px: 7.0,
            cluster_ring_width: 2.0,
            cluster_label_color: "#4a3d10".to_owned(),
            cluster_label_offset_x: 10.0,
            cluster_label_offset_y: 4.0,
            cluster_edge_width_cap: 6.0,

            dim_node_fill: "#9aa0ac".to_owned(),
            dim_alpha: 0.15,

            edge_color: "#5c6579".to_owned(),
            edge_width: 1.7,
            edge_dim_color: "#c7ccd6".to_owned(),
            edge_dim_width: 1.3,

            box_select_fill: "#4d90fe".to_owned(),
            box_select_fill_alpha: 0.15,
            box_select_border: "#2f6fd1".to_owned(),
            box_select_border_width: 1.0,

            hover_card: FigureTheme::light(),
            category_palette: default_category_palette(),
        }
    }

    /// High-contrast (accessibility) preset — pure-black background
    /// assumed, maximum-contrast ring/label ink. Mirrors
    /// `FigureTheme::high_contrast()`'s own "screen contrast and hue
    /// confusability are independent knobs" framing: `category_palette`
    /// stays the SAME shared default (a caller wanting colorblind-safe
    /// categorical hues alongside this preset pairs it with
    /// `CategoricalScale::default_palette()` explicitly, exactly like
    /// `FigureTheme::high_contrast`'s own doc comment recommends).
    pub fn high_contrast() -> Self {
        Self {
            selection_ring_color: "#ffff00".to_owned(),
            selection_ring_width: 2.0,
            selection_ring_offset_px: 2.0,
            hover_ring_color: "#00e5ff".to_owned(),
            hover_ring_width: 1.5,
            hover_ring_offset_px: 1.5,

            label_fill: "#ffffff".to_owned(),
            label_font: "11px sans-serif".to_owned(),
            label_offset_x: 4.0,
            label_offset_y: 4.0,

            cluster_accent: "#ff9d00".to_owned(),
            cluster_ring_inner_offset_px: 3.0,
            cluster_ring_outer_offset_px: 7.0,
            cluster_ring_width: 2.0,
            cluster_label_color: "#ffffff".to_owned(),
            cluster_label_offset_x: 10.0,
            cluster_label_offset_y: 4.0,
            cluster_edge_width_cap: 6.0,

            dim_node_fill: "#4d4d4d".to_owned(),
            dim_alpha: 0.15,

            edge_color: "#ffffff".to_owned(),
            edge_width: 1.7,
            edge_dim_color: "#808080".to_owned(),
            edge_dim_width: 1.3,

            box_select_fill: "#00aaff".to_owned(),
            box_select_fill_alpha: 0.25,
            box_select_border: "#00eaff".to_owned(),
            box_select_border_width: 1.5,

            hover_card: FigureTheme::high_contrast(),
            category_palette: default_category_palette(),
        }
    }
}

impl Default for GraphTheme {
    /// Dark — matches the crate's own only-ever-shipped demo aesthetic
    /// (same convention `FigureTheme::default()` already follows).
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_matches_the_pre_existing_hardcoded_literals() {
        let theme = GraphTheme::dark();
        assert_eq!(theme.selection_ring_color, "#ffffff");
        assert_eq!(theme.selection_ring_width, 2.0);
        assert_eq!(theme.selection_ring_offset_px, 2.0);
        assert_eq!(theme.hover_ring_color, "#ffd76a");
        assert_eq!(theme.hover_ring_width, 1.5);
        assert_eq!(theme.hover_ring_offset_px, 1.5);
        assert_eq!(theme.label_fill, "#e6e6ea");
        assert_eq!(theme.label_font, "11px sans-serif");
        assert_eq!(theme.label_offset_x, 4.0);
        assert_eq!(theme.label_offset_y, 4.0);
        assert_eq!(theme.cluster_accent, "#c9a94e");
        assert_eq!(theme.cluster_ring_inner_offset_px, 3.0);
        assert_eq!(theme.cluster_ring_outer_offset_px, 7.0);
        assert_eq!(theme.cluster_ring_width, 2.0);
        assert_eq!(theme.cluster_label_color, "#f0e6c0");
        assert_eq!(theme.cluster_label_offset_x, 10.0);
        assert_eq!(theme.cluster_label_offset_y, 4.0);
        assert_eq!(theme.cluster_edge_width_cap, 6.0);
        assert_eq!(theme.dim_node_fill, "#6b7280");
        assert_eq!(theme.dim_alpha, 0.15);
        assert_eq!(theme.edge_color, "#7c8496");
        assert_eq!(theme.edge_width, 1.7);
        assert_eq!(theme.edge_dim_color, "#5a6070");
        assert_eq!(theme.edge_dim_width, 1.3);
        assert_eq!(theme.box_select_fill, "#4d90fe");
        assert_eq!(theme.box_select_fill_alpha, 0.15);
        assert_eq!(theme.box_select_border, "#7fb2ff");
        assert_eq!(theme.box_select_border_width, 1.0);
        assert_eq!(theme.hover_card.background, FigureTheme::dark().background);
    }

    #[test]
    fn default_is_dark() {
        assert_eq!(GraphTheme::default().selection_ring_color, GraphTheme::dark().selection_ring_color);
    }

    #[test]
    fn light_fixes_the_invisible_white_on_white_selection_ring_defect() {
        let light = GraphTheme::light();
        assert_ne!(light.selection_ring_color, "#ffffff", "a white ring must not survive onto the light preset");
        assert_ne!(light.label_fill, "#e6e6ea", "the near-white label fill must not survive onto the light preset either");
        assert_eq!(light.hover_card.background, FigureTheme::light().background);
    }

    #[test]
    fn high_contrast_uses_pure_black_chrome_ink() {
        let hc = GraphTheme::high_contrast();
        assert_eq!(hc.label_fill, "#ffffff");
        assert_eq!(hc.hover_card.background, FigureTheme::high_contrast().background);
        assert_ne!(hc.selection_ring_color, hc.hover_ring_color, "selection and hover rings must stay visually distinct");
    }

    #[test]
    fn every_preset_carries_the_same_default_category_palette() {
        let dark = GraphTheme::dark();
        let light = GraphTheme::light();
        let hc = GraphTheme::high_contrast();
        assert_eq!(dark.category_palette.len(), light.category_palette.len());
        assert_eq!(dark.category_palette.color_for(0), light.category_palette.color_for(0));
        assert_eq!(dark.category_palette.color_for(0), hc.category_palette.color_for(0));
    }

    #[test]
    fn a_caller_can_swap_in_the_okabe_ito_colorblind_safe_palette() {
        let theme = GraphTheme { category_palette: CategoricalScale::default_palette(), ..GraphTheme::dark() };
        assert_eq!(theme.category_palette.len(), 8);
        assert_eq!(theme.category_palette.color_for(0), CategoricalScale::default_palette().color_for(0));
        assert_ne!(theme.category_palette.color_for(0), default_category_palette().color_for(0));
    }

    #[test]
    fn default_category_palette_matches_the_named_constant_in_order() {
        let palette = default_category_palette();
        assert_eq!(palette.len(), DEFAULT_CATEGORY_PALETTE.len());
        for (i, &expected) in DEFAULT_CATEGORY_PALETTE.iter().enumerate() {
            assert_eq!(palette.color_for(i), expected);
        }
    }
}
