//! `uzor-figures-registry` — serde-able spec/IR registry for
//! `uzor-figures` (arc B item 1 of the approved plan; `nemo/docs/
//! uzor-engines/uzor_figures_engine_architecture.md` §3's own
//! `uzor-figures-registry` placement).
//!
//! `uzor-figures` itself is deliberately serde-free (its own `CLAUDE.md`
//! "Forbidden" line, and the architecture doc §2: "typed Rust layer-cake
//! of libraries, NOT a spec interpreter — no JSON grammar-of-graphics
//! compiler"). This crate is the ONE place that bridges that typed core
//! to AUTHORED CONTENT: a [`FigureSpec`] enum with one variant per shipped
//! figure kind, `#[derive(Serialize, Deserialize)]` all the way down,
//! [`parse_spec`]/[`spec_to_json`] for the JSON <-> spec round trip, and
//! [`render_spec`] to render a parsed spec immediately.
//!
//! # Why a SEPARATE crate, not serde on `uzor-figures` itself
//!
//! `mlc`'s own proven two-table pattern (`ChartTypeDef` + `DrawOps`, the
//! architecture doc's own §3 intro) keeps registry/dispatch machinery OUT
//! of the engine core it dispatches over — this crate mirrors that split:
//! `uzor-figures` stays a plain typed Rust API (any consumer can build a
//! `BarFigure` by hand, no JSON anywhere near it, no new dependency added
//! to that crate), and every `serde`/`serde_json` dependency lives HERE
//! only.
//!
//! # Owned specs, freshly-built figures (no lifetime leakage)
//!
//! Every `uzor-figures` figure struct owns its own data directly (e.g.
//! `BarFigure::categories: Vec<String>`) — there is no borrowed-slice
//! figure type to bridge in this crate's own public surface. [`FigureSpec`]
//! variants own their data the same way (`Vec`/`String`/`f64`, real
//! `#[derive(Deserialize)]` targets); [`render_spec`] takes `&FigureSpec`
//! (never consumes it — a spec is authored once but may be re-rendered,
//! e.g. at a different `Rect`/`FigureTheme`), builds a fresh, real
//! `uzor-figures` figure from CLONED owned spec data, and renders it
//! immediately in that same call — no lifetime parameter, no
//! borrowed-figure escape hatch, anywhere in this crate's public API.
//!
//! # The one string-keyed dispatch level (owner doctrine)
//!
//! [`FigureSpec`] is `#[serde(tag = "kind", content = "data")]` — `kind`
//! (`"bar"`/`"curve"`/.../`"kpi"`) is the ONE registry-level string-keyed
//! dispatch value this crate's wire format carries. Every field INSIDE a
//! variant's own `data` is a real typed field (typed enums for typed
//! things — [`SpecBarMode`], [`SpecNumberFormat`], [`SpecPointRadius`],
//! never a stringly config bag); a nested enum's own serde discriminant
//! (e.g. `SpecPointRadius`'s `"mode"` field) is ordinary Rust-enum serde
//! representation, not a second registry-level dispatch axis.
//!
//! See this crate's own `CLAUDE.md` for the full variant list, field-shape
//! decisions, and the `NumberFormat::Currency`/`&'static str` bridging
//! trade-off (the private `common::leak_currency_symbol` helper — see its
//! own doc comment in the crate source for the full reasoning).

pub mod common;
pub mod error;
pub mod render;
pub mod spec;

pub use common::{SpecAnnotation, SpecBarMode, SpecLegendPosition, SpecNumberFormat, SpecPointRadius, SpecWaterfallKind};
pub use error::RegistryError;
pub use render::render_spec;
pub use spec::{
    parse_spec, spec_to_json, BarSeriesSpec, BarSpec, BoxplotSpec, CurveSeriesSpec, CurveSpec, FigureSpec, HeatmapSpec, HistogramSpec, KpiSpec,
    PieSliceSpec, PieSpec, SankeyLinkSpec, SankeyNodeSpec, SankeySpec, ScatterPointSpec, ScatterSpec, TimelineEventSpec, TimelineSpec,
    WaterfallItemSpec, WaterfallSpec,
};
