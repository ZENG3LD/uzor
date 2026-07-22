//! `spec -> json -> spec == original` for every [`FigureSpec`] variant —
//! proves [`spec_to_json`]/[`parse_spec`] are real inverses, not just
//! individually well-formed.

use uzor_figures_registry::{
    parse_spec, spec_to_json, BarSeriesSpec, BarSpec, BoxplotSpec, CurveSeriesSpec, CurveSpec, FigureSpec, HeatmapSpec, HistogramSpec, KpiSpec,
    PieSliceSpec, PieSpec, SankeyLinkSpec, SankeyNodeSpec, SankeySpec, ScatterPointSpec, ScatterSpec, SpecAnnotation, SpecBarMode,
    SpecLegendPosition, SpecNumberFormat, SpecPointRadius, SpecWaterfallKind, TimelineEventSpec, TimelineSpec, WaterfallItemSpec, WaterfallSpec,
};

fn assert_round_trips(spec: FigureSpec) {
    let json = spec_to_json(&spec).expect("spec must serialize");
    let parsed = parse_spec(&json).expect("serialized spec must re-parse");
    assert_eq!(spec, parsed, "round trip must reproduce the exact same spec\nJSON:\n{json}");
}

#[test]
fn bar_spec_round_trips() {
    assert_round_trips(FigureSpec::Bar(BarSpec {
        categories: vec!["cat-a".to_owned(), "cat-b".to_owned()],
        series: vec![
            BarSeriesSpec { name: "alpha".to_owned(), values: vec![1.0, 2.0] },
            BarSeriesSpec { name: "beta".to_owned(), values: vec![3.0, -4.0] },
        ],
        mode: SpecBarMode::Stacked,
        title: Some("Bar spec".to_owned()),
        show_value_labels: true,
        legend_position: Some(SpecLegendPosition::Top),
        annotations: vec![SpecAnnotation::HLine { value: 2.5, color: None, label: Some("target".to_owned()) }],
    }));
}

#[test]
fn bar_spec_round_trips_with_every_optional_field_absent() {
    assert_round_trips(FigureSpec::Bar(BarSpec {
        categories: vec!["only".to_owned()],
        series: vec![BarSeriesSpec { name: String::new(), values: vec![1.0] }],
        mode: SpecBarMode::Grouped,
        title: None,
        show_value_labels: false,
        legend_position: None,
        annotations: Vec::new(),
    }));
}

#[test]
fn curve_spec_round_trips() {
    assert_round_trips(FigureSpec::Curve(CurveSpec {
        series: vec![
            CurveSeriesSpec { name: "a".to_owned(), points: vec![(0.0, 0.0), (1.0, 4.0)] },
            CurveSeriesSpec { name: "b".to_owned(), points: vec![(0.0, -2.0), (1.0, 3.0)] },
        ],
        title: Some("Curve spec".to_owned()),
        fill: true,
        legend_position: Some(SpecLegendPosition::Right),
        downsample: Some(100),
        annotations: vec![SpecAnnotation::Callout { x: 1.0, y: 4.0, text: "peak".to_owned() }],
    }));
}

#[test]
fn histogram_spec_round_trips() {
    assert_round_trips(FigureSpec::Histogram(HistogramSpec {
        samples: vec![1.0, 2.0, 2.0, 3.0, 5.0],
        bin_count: 4,
        title: Some("Histogram spec".to_owned()),
    }));
}

#[test]
fn timeline_spec_round_trips() {
    assert_round_trips(FigureSpec::Timeline(TimelineSpec {
        events: vec![
            TimelineEventSpec { ts: 100.0, end_ts: None, lane: 0, label: "point".to_owned(), kind: 0 },
            TimelineEventSpec { ts: 200.0, end_ts: Some(400.0), lane: 1, label: "interval".to_owned(), kind: 2 },
        ],
        lane_names: vec!["actor-a".to_owned(), "actor-b".to_owned()],
        title: Some("Timeline spec".to_owned()),
    }));
}

#[test]
fn sankey_spec_round_trips() {
    assert_round_trips(FigureSpec::Sankey(SankeySpec {
        nodes: vec![
            SankeyNodeSpec { id: "src-a".to_owned(), label: "src-a".to_owned(), stage: 0 },
            SankeyNodeSpec { id: "sink-a".to_owned(), label: "sink-a".to_owned(), stage: 1 },
        ],
        links: vec![SankeyLinkSpec { from: 0, to: 1, weight: 40.0, kind: 0 }],
        title: Some("Sankey spec".to_owned()),
    }));
}

#[test]
fn pie_spec_round_trips() {
    assert_round_trips(FigureSpec::Pie(PieSpec {
        slices: vec![PieSliceSpec { label: "a".to_owned(), value: 40.0 }, PieSliceSpec { label: "b".to_owned(), value: 60.0 }],
        donut_inner_ratio: Some(0.55),
        legend_position: Some(SpecLegendPosition::Right),
        title: Some("Pie spec".to_owned()),
        top_n: Some(5),
    }));
}

#[test]
fn waterfall_spec_round_trips() {
    assert_round_trips(FigureSpec::Waterfall(WaterfallSpec {
        items: vec![
            WaterfallItemSpec { label: "opening".to_owned(), value: 100.0, kind: SpecWaterfallKind::Total },
            WaterfallItemSpec { label: "gain".to_owned(), value: 20.0, kind: SpecWaterfallKind::Delta },
            WaterfallItemSpec { label: "subtotal".to_owned(), value: 0.0, kind: SpecWaterfallKind::Subtotal },
        ],
        title: Some("Waterfall spec".to_owned()),
    }));
}

#[test]
fn heatmap_spec_round_trips() {
    assert_round_trips(FigureSpec::Heatmap(HeatmapSpec {
        x_labels: vec!["x0".to_owned(), "x1".to_owned()],
        y_labels: vec!["y0".to_owned(), "y1".to_owned()],
        values: vec![vec![1.0, -2.0], vec![3.0, 4.0]],
        diverging_mid: Some(0.0),
        title: Some("Heatmap spec".to_owned()),
    }));
}

#[test]
fn scatter_spec_round_trips() {
    assert_round_trips(FigureSpec::Scatter(ScatterSpec {
        points: vec![
            ScatterPointSpec { x: 1.0, y: 2.0, value: Some(3.0) },
            ScatterPointSpec { x: 4.0, y: 5.0, value: None },
        ],
        title: Some("Scatter spec".to_owned()),
        radius: Some(SpecPointRadius::ValueMapped { min_radius: 2.0, max_radius: 8.0 }),
        thinning: Some(50),
        annotations: vec![SpecAnnotation::HBand { low: 1.0, high: 3.0, color: None, label: Some("band".to_owned()) }],
    }));
}

#[test]
fn scatter_spec_round_trips_with_fixed_radius() {
    assert_round_trips(FigureSpec::Scatter(ScatterSpec {
        points: vec![ScatterPointSpec { x: 1.0, y: 2.0, value: None }],
        title: None,
        radius: Some(SpecPointRadius::Fixed { radius: 4.0 }),
        thinning: None,
        annotations: Vec::new(),
    }));
}

#[test]
fn boxplot_spec_round_trips() {
    assert_round_trips(FigureSpec::Boxplot(BoxplotSpec {
        categories: vec!["group-a".to_owned(), "group-b".to_owned()],
        samples: vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0, 100.0]],
        title: Some("Boxplot spec".to_owned()),
    }));
}

#[test]
fn kpi_spec_round_trips_with_currency_format() {
    assert_round_trips(FigureSpec::Kpi(KpiSpec {
        label: "Revenue".to_owned(),
        value: 128_430.0,
        previous_value: Some(110_000.0),
        sparkline: vec![1.0, 2.0, 3.0, 4.0],
        format: Some(SpecNumberFormat::Currency { symbol: "$".to_owned() }),
    }));
}

#[test]
fn kpi_spec_round_trips_with_every_optional_field_absent() {
    assert_round_trips(FigureSpec::Kpi(KpiSpec {
        label: "Active Users".to_owned(),
        value: 48_213.0,
        previous_value: None,
        sparkline: Vec::new(),
        format: None,
    }));
}
