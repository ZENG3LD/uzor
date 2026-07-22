//! [`FigureSpec`] — one variant per shipped `uzor-figures` figure kind,
//! serde-able top to bottom. See this crate's own root docs (`lib.rs`)
//! for the wire-format shape and the "typed fields, ONE string-keyed
//! dispatch level" doctrine.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::{SpecAnnotation, SpecBarMode, SpecLegendPosition, SpecNumberFormat, SpecPointRadius, SpecWaterfallKind};
use crate::error::RegistryError;

/// One named bar series — mirrors [`uzor_figures::BarSeries`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarSeriesSpec {
    pub name: String,
    pub values: Vec<f64>,
}

/// Spec for [`uzor_figures::BarFigure`] — categorical bar / grouped /
/// stacked multi-series, mirroring
/// [`uzor_figures::BarFigure::with_series`]'s own field shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarSpec {
    pub categories: Vec<String>,
    pub series: Vec<BarSeriesSpec>,
    pub mode: SpecBarMode,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub show_value_labels: bool,
    #[serde(default)]
    pub legend_position: Option<SpecLegendPosition>,
    #[serde(default)]
    pub annotations: Vec<SpecAnnotation>,
}

/// One named line series — mirrors [`uzor_figures::CurveSeries`]. `points`
/// are `(x, y)` pairs, serialized as 2-element JSON arrays (serde's own
/// tuple representation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveSeriesSpec {
    pub name: String,
    pub points: Vec<(f64, f64)>,
}

/// Spec for [`uzor_figures::CurveFigure`] — multi-line, optional fill,
/// optional LTTB downsample budget
/// ([`uzor_figures::CurveFigure::with_downsample`]).
///
/// **Deferred (documented, not silently dropped)**: `with_x_scale` (e.g.
/// a [`uzor_figures::TimeScale`] X-axis override) has no spec field yet —
/// every OTHER builder option this figure has is covered; a time-axis
/// variant is a real, separate follow-up (it would need its own typed
/// `x_axis` field distinguishing "auto nice-linear" from "calendar time
/// over these UTC-second bounds"), out of this pass's own scope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveSpec {
    pub series: Vec<CurveSeriesSpec>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub fill: bool,
    #[serde(default)]
    pub legend_position: Option<SpecLegendPosition>,
    #[serde(default)]
    pub downsample: Option<usize>,
    #[serde(default)]
    pub annotations: Vec<SpecAnnotation>,
}

/// Spec for [`uzor_figures::HistogramFigure`] — raw samples binned at
/// render time (this figure has no annotation/legend builder options).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistogramSpec {
    pub samples: Vec<f64>,
    pub bin_count: usize,
    #[serde(default)]
    pub title: Option<String>,
}

/// One timeline event — mirrors [`uzor_figures::TimelineEvent`]. A point
/// event has `end_ts: None`; an interval event has `end_ts: Some(..)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEventSpec {
    pub ts: f64,
    #[serde(default)]
    pub end_ts: Option<f64>,
    pub lane: usize,
    pub label: String,
    pub kind: usize,
}

/// Spec for [`uzor_figures::TimelineFigure`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineSpec {
    pub events: Vec<TimelineEventSpec>,
    pub lane_names: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
}

/// One flow-stage node — mirrors [`uzor_figures::SankeyNode`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SankeyNodeSpec {
    pub id: String,
    pub label: String,
    pub stage: usize,
}

/// One weighted edge — mirrors [`uzor_figures::SankeyLink`]. `from`/`to`
/// are indices into the spec's own `nodes` list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SankeyLinkSpec {
    pub from: usize,
    pub to: usize,
    pub weight: f64,
    pub kind: usize,
}

/// Spec for [`uzor_figures::SankeyFigure`] — caller-supplied `stage`
/// columns, no automatic layering (matches the underlying figure's own
/// Phase C scope, see its module docs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SankeySpec {
    pub nodes: Vec<SankeyNodeSpec>,
    pub links: Vec<SankeyLinkSpec>,
    #[serde(default)]
    pub title: Option<String>,
}

/// One weighted slice — mirrors [`uzor_figures::PieSlice`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PieSliceSpec {
    pub label: String,
    pub value: f64,
}

/// Spec for [`uzor_figures::PieFigure`] — pie by default, donut when
/// `donut_inner_ratio` is set (mirrors
/// [`uzor_figures::PieFigure::donut`]'s own clamped-ratio semantics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PieSpec {
    pub slices: Vec<PieSliceSpec>,
    #[serde(default)]
    pub donut_inner_ratio: Option<f64>,
    #[serde(default)]
    pub legend_position: Option<SpecLegendPosition>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub top_n: Option<usize>,
}

/// One running-total bridge item — mirrors [`uzor_figures::WaterfallItem`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterfallItemSpec {
    pub label: String,
    pub value: f64,
    pub kind: SpecWaterfallKind,
}

/// Spec for [`uzor_figures::WaterfallFigure`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterfallSpec {
    pub items: Vec<WaterfallItemSpec>,
    #[serde(default)]
    pub title: Option<String>,
}

/// Spec for [`uzor_figures::HeatmapFigure`] — `values[row][col]` (`row`
/// indexes `y_labels`, `col` indexes `x_labels`, matching the underlying
/// figure's own constructor doc). Sequential color scale by default;
/// `diverging_mid: Some(mid)` switches to
/// [`uzor_figures::HeatmapFigure::with_diverging`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeatmapSpec {
    pub x_labels: Vec<String>,
    pub y_labels: Vec<String>,
    pub values: Vec<Vec<f64>>,
    #[serde(default)]
    pub diverging_mid: Option<f64>,
    #[serde(default)]
    pub title: Option<String>,
}

/// One point — mirrors [`uzor_figures::ScatterPoint`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScatterPointSpec {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub value: Option<f64>,
}

/// Spec for [`uzor_figures::ScatterFigure`]. `radius: None` uses the
/// underlying figure's own default ([`uzor_figures::PointRadius::
/// default`]) rather than this crate re-declaring that default's numeric
/// value a second time (stays correct automatically if the upstream
/// default ever changes).
///
/// **Deferred (documented)**: `with_x_scale` — same reasoning as
/// [`CurveSpec`]'s own doc comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScatterSpec {
    pub points: Vec<ScatterPointSpec>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub radius: Option<SpecPointRadius>,
    #[serde(default)]
    pub thinning: Option<usize>,
    #[serde(default)]
    pub annotations: Vec<SpecAnnotation>,
}

/// Spec for [`uzor_figures::BoxplotFigure`] — `categories[i]` labels
/// `samples[i]`'s own distribution (same mismatched-length tolerance the
/// underlying figure's own constructor documents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxplotSpec {
    pub categories: Vec<String>,
    pub samples: Vec<Vec<f64>>,
    #[serde(default)]
    pub title: Option<String>,
}

/// Spec for [`uzor_figures::KpiFigure`] — a single dashboard number tile.
/// `format: None` uses the underlying figure's own default
/// ([`uzor_figures::NumberFormat::default`]), same "don't re-declare the
/// upstream default" reasoning as [`ScatterSpec::radius`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KpiSpec {
    pub label: String,
    pub value: f64,
    #[serde(default)]
    pub previous_value: Option<f64>,
    #[serde(default)]
    pub sparkline: Vec<f64>,
    #[serde(default)]
    pub format: Option<SpecNumberFormat>,
}

/// The registry's own spec-IR: one variant per shipped `uzor-figures`
/// figure kind. Adjacently tagged (`{"kind": "...", "data": {...}}`) —
/// `kind` is the ONE string-keyed dispatch value this crate's wire format
/// carries (owner doctrine); every field inside `data` is typed.
///
/// Parse via [`parse_spec`] (not `serde_json::from_str::<FigureSpec>`
/// directly — see that function's own doc comment for why: precise,
/// typed error variants distinguishing an unknown `kind` from a malformed
/// field inside a recognized one).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum FigureSpec {
    Bar(BarSpec),
    Curve(CurveSpec),
    Histogram(HistogramSpec),
    Timeline(TimelineSpec),
    Sankey(SankeySpec),
    Pie(PieSpec),
    Waterfall(WaterfallSpec),
    Heatmap(HeatmapSpec),
    Scatter(ScatterSpec),
    Boxplot(BoxplotSpec),
    Kpi(KpiSpec),
}

/// Parse a `{"kind": "...", "data": {...}}` JSON document into a
/// [`FigureSpec`] — never panics on malformed input (see
/// [`RegistryError`]'s own variants for exactly which failure mode is
/// reported).
///
/// Two-phase (extract + match on `kind` first, THEN deserialize `data`
/// into that kind's own typed struct) rather than one
/// `serde_json::from_str::<FigureSpec>(json)` call — a single derived
/// call collapses "unknown kind" and "kind is fine but a field inside
/// `data` is wrong" into one opaque `serde_json::Error`; this crate's own
/// task brief asks for a typed error enum distinguishing them.
pub fn parse_spec(json: &str) -> Result<FigureSpec, RegistryError> {
    let value: Value = serde_json::from_str(json).map_err(RegistryError::InvalidJson)?;
    let obj = value.as_object().ok_or(RegistryError::NotAnObject)?;
    let kind = obj.get("kind").and_then(Value::as_str).ok_or(RegistryError::MissingKind)?;
    let data = obj.get("data").cloned().ok_or_else(|| RegistryError::MissingData(kind.to_owned()))?;

    macro_rules! parse_variant {
        ($variant:ident) => {
            serde_json::from_value(data)
                .map(FigureSpec::$variant)
                .map_err(|e| RegistryError::MalformedField { kind: kind.to_owned(), message: e.to_string() })
        };
    }

    match kind {
        "bar" => parse_variant!(Bar),
        "curve" => parse_variant!(Curve),
        "histogram" => parse_variant!(Histogram),
        "timeline" => parse_variant!(Timeline),
        "sankey" => parse_variant!(Sankey),
        "pie" => parse_variant!(Pie),
        "waterfall" => parse_variant!(Waterfall),
        "heatmap" => parse_variant!(Heatmap),
        "scatter" => parse_variant!(Scatter),
        "boxplot" => parse_variant!(Boxplot),
        "kpi" => parse_variant!(Kpi),
        other => Err(RegistryError::UnknownKind(other.to_owned())),
    }
}

/// Serialize a [`FigureSpec`] back to its `{"kind": ..., "data": ...}`
/// JSON form — round-trips with [`parse_spec`]
/// (`parse_spec(&spec_to_json(&spec)?)? == spec` for every variant, see
/// this crate's own `tests/roundtrip.rs`).
pub fn spec_to_json(spec: &FigureSpec) -> Result<String, RegistryError> {
    serde_json::to_string_pretty(spec).map_err(RegistryError::Serialize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_kind_is_a_typed_error_not_a_panic() {
        let err = parse_spec(r#"{"kind": "not_a_real_kind", "data": {}}"#).expect_err("unknown kind must error");
        assert!(matches!(err, RegistryError::UnknownKind(k) if k == "not_a_real_kind"));
    }

    #[test]
    fn missing_kind_is_a_typed_error() {
        let err = parse_spec(r#"{"data": {}}"#).expect_err("missing kind must error");
        assert!(matches!(err, RegistryError::MissingKind));
    }

    #[test]
    fn missing_data_is_a_typed_error() {
        let err = parse_spec(r#"{"kind": "bar"}"#).expect_err("missing data must error");
        assert!(matches!(err, RegistryError::MissingData(k) if k == "bar"));
    }

    #[test]
    fn non_object_top_level_is_a_typed_error() {
        let err = parse_spec(r#"[1, 2, 3]"#).expect_err("a bare array must error");
        assert!(matches!(err, RegistryError::NotAnObject));
    }

    #[test]
    fn invalid_json_syntax_is_a_typed_error_not_a_panic() {
        let err = parse_spec("{not json at all").expect_err("invalid syntax must error");
        assert!(matches!(err, RegistryError::InvalidJson(_)));
    }

    #[test]
    fn malformed_field_inside_a_recognized_kind_is_a_typed_error() {
        // "categories" must be an array of strings — a number is malformed.
        let err = parse_spec(r#"{"kind": "bar", "data": {"categories": 5, "series": [], "mode": "grouped"}}"#)
            .expect_err("a malformed field must error, not panic");
        assert!(matches!(err, RegistryError::MalformedField { kind, .. } if kind == "bar"));
    }

    #[test]
    fn spec_to_json_round_trips_through_parse_spec() {
        let spec = FigureSpec::Histogram(HistogramSpec { samples: vec![1.0, 2.0, 3.0], bin_count: 2, title: Some("t".to_owned()) });
        let json = spec_to_json(&spec).expect("spec must serialize");
        let parsed = parse_spec(&json).expect("serialized spec must re-parse");
        assert_eq!(spec, parsed);
    }
}
