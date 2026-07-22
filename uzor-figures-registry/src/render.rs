//! Build a real, fresh `uzor-figures` figure from an owned [`FigureSpec`]
//! and render it immediately — no borrowed-figure type, no lifetime
//! parameter, anywhere in this module's public surface (see this crate's
//! root docs for why).

use uzor::render::RenderContext;
use uzor::types::Rect;
use uzor_figures::{
    BarFigure, BarSeries, BoxplotFigure, CurveFigure, CurveSeries, FigureTheme, HeatmapFigure, HistogramFigure, KpiFigure, PieFigure, PieSlice,
    SankeyFigure, SankeyLink, SankeyNode, ScatterFigure, ScatterPoint, TimelineEvent, TimelineFigure, WaterfallFigure, WaterfallItem,
};

use crate::common::to_annotations;
use crate::spec::{
    BarSpec, BoxplotSpec, CurveSpec, FigureSpec, HeatmapSpec, HistogramSpec, KpiSpec, PieSpec, SankeySpec, ScatterSpec, TimelineSpec, WaterfallSpec,
};

/// Build the real figure `spec` describes, from CLONED owned spec data,
/// and render it into `rect` of `ctx` using `theme` — immediately, in
/// this one call. `spec` is only ever borrowed (never consumed) — a spec
/// is typically authored once and may be rendered many times (different
/// `rect`/`theme`, different report pages).
pub fn render_spec(spec: &FigureSpec, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
    match spec {
        FigureSpec::Bar(s) => build_bar(s).render(ctx, rect, theme),
        FigureSpec::Curve(s) => build_curve(s).render(ctx, rect, theme),
        FigureSpec::Histogram(s) => build_histogram(s).render(ctx, rect, theme),
        FigureSpec::Timeline(s) => build_timeline(s).render(ctx, rect, theme),
        FigureSpec::Sankey(s) => build_sankey(s).render(ctx, rect, theme),
        FigureSpec::Pie(s) => build_pie(s).render(ctx, rect, theme),
        FigureSpec::Waterfall(s) => build_waterfall(s).render(ctx, rect, theme),
        FigureSpec::Heatmap(s) => build_heatmap(s).render(ctx, rect, theme),
        FigureSpec::Scatter(s) => build_scatter(s).render(ctx, rect, theme),
        FigureSpec::Boxplot(s) => build_boxplot(s).render(ctx, rect, theme),
        FigureSpec::Kpi(s) => build_kpi(s).render(ctx, rect, theme),
    }
}

fn build_bar(spec: &BarSpec) -> BarFigure {
    let series = spec.series.iter().map(|s| BarSeries { name: s.name.clone(), values: s.values.clone() }).collect();
    let mut figure = BarFigure::with_series(spec.categories.clone(), series, spec.mode.into());
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure = figure.with_value_labels(spec.show_value_labels);
    if let Some(position) = spec.legend_position {
        figure = figure.with_legend(position.into());
    }
    figure.with_annotations(to_annotations(&spec.annotations))
}

fn build_curve(spec: &CurveSpec) -> CurveFigure {
    let series = spec.series.iter().map(|s| CurveSeries { name: s.name.clone(), points: s.points.clone() }).collect();
    let mut figure = CurveFigure::with_series(series);
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure = figure.with_fill(spec.fill);
    if let Some(position) = spec.legend_position {
        figure = figure.with_legend(position.into());
    }
    if let Some(max_points) = spec.downsample {
        figure = figure.with_downsample(max_points);
    }
    figure.with_annotations(to_annotations(&spec.annotations))
}

fn build_histogram(spec: &HistogramSpec) -> HistogramFigure {
    let mut figure = HistogramFigure::new(spec.samples.clone(), spec.bin_count);
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure
}

fn build_timeline(spec: &TimelineSpec) -> TimelineFigure {
    let events =
        spec.events.iter().map(|e| TimelineEvent { ts: e.ts, end_ts: e.end_ts, lane: e.lane, label: e.label.clone(), kind: e.kind }).collect();
    let mut figure = TimelineFigure::new(events, spec.lane_names.clone());
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure
}

fn build_sankey(spec: &SankeySpec) -> SankeyFigure {
    let nodes = spec.nodes.iter().map(|n| SankeyNode { id: n.id.clone(), label: n.label.clone(), stage: n.stage }).collect();
    let links = spec.links.iter().map(|l| SankeyLink { from: l.from, to: l.to, weight: l.weight, kind: l.kind }).collect();
    let mut figure = SankeyFigure::new(nodes, links);
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure
}

fn build_pie(spec: &PieSpec) -> PieFigure {
    let slices = spec.slices.iter().map(|s| PieSlice { label: s.label.clone(), value: s.value }).collect();
    let mut figure = PieFigure::new(slices);
    if let Some(ratio) = spec.donut_inner_ratio {
        figure = figure.donut(ratio);
    }
    if let Some(position) = spec.legend_position {
        figure = figure.with_legend(position.into());
    }
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    if let Some(n) = spec.top_n {
        figure = figure.top_n(n);
    }
    figure
}

fn build_waterfall(spec: &WaterfallSpec) -> WaterfallFigure {
    let items = spec.items.iter().map(|i| WaterfallItem { label: i.label.clone(), value: i.value, kind: i.kind.into() }).collect();
    let mut figure = WaterfallFigure::new(items);
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure
}

fn build_heatmap(spec: &HeatmapSpec) -> HeatmapFigure {
    let mut figure = HeatmapFigure::new(spec.x_labels.clone(), spec.y_labels.clone(), spec.values.clone());
    if let Some(mid) = spec.diverging_mid {
        figure = figure.with_diverging(mid);
    }
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure
}

fn build_scatter(spec: &ScatterSpec) -> ScatterFigure {
    let points = spec.points.iter().map(|p| ScatterPoint { x: p.x, y: p.y, value: p.value }).collect();
    let mut figure = ScatterFigure::new(points);
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    if let Some(radius) = spec.radius {
        figure = figure.with_radius(radius.into());
    }
    if let Some(max_points) = spec.thinning {
        figure = figure.with_thinning(max_points);
    }
    figure.with_annotations(to_annotations(&spec.annotations))
}

fn build_boxplot(spec: &BoxplotSpec) -> BoxplotFigure {
    let mut figure = BoxplotFigure::new(spec.categories.clone(), spec.samples.clone());
    if let Some(title) = &spec.title {
        figure = figure.with_title(title.clone());
    }
    figure
}

fn build_kpi(spec: &KpiSpec) -> KpiFigure {
    let mut figure = KpiFigure::new(spec.label.clone(), spec.value);
    if let Some(previous) = spec.previous_value {
        figure = figure.with_previous_value(previous);
    }
    if !spec.sparkline.is_empty() {
        figure = figure.with_sparkline(spec.sparkline.clone());
    }
    if let Some(format) = &spec.format {
        figure = figure.with_format(format.into());
    }
    figure
}
