//! Headless proof: for every [`FigureSpec`] variant, parse an AUTHORED
//! JSON literal (simulating a real `.figure.json` file), render it via
//! [`render_spec`], and assert a valid, NON-BLANK PNG comes out — same
//! headless discipline `uzor-figures`' own `proof_tests` module uses
//! (deterministic fixtures, `uzor-export` for headless rasterization),
//! plus a "not just a flat background" pixel check this task's own gate
//! asks for. Every PNG is ALSO written to `nemo/uzor/out/registry_
//! <kind>.png` for a human to eyeball.

use std::path::PathBuf;

use uzor::types::Rect;
use uzor_export::{render_to_png, ExportSpec};
use uzor_figures::FigureTheme;
use uzor_figures_registry::{parse_spec, render_spec};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 500;

fn out_dir() -> PathBuf {
    // Fixed path (not CARGO_MANIFEST_DIR-relative) — `uzor/out/` is the
    // shared human-eyeball drop point for every headless proof render in
    // this workspace (see `uzor-figures`' own `proof_tests` module).
    PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
}

fn write_proof_png(name: &str, bytes: &[u8]) {
    let dir = out_dir();
    std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
    std::fs::write(dir.join(name), bytes).expect("write proof PNG");
}

fn decode_rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().expect("valid PNG header");
    let info = reader.info();
    let (width, height) = (info.width, info.height);
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame_info = reader.next_frame(&mut buf).expect("decode PNG frame");
    buf.truncate(frame_info.buffer_size());
    (width, height, buf)
}

/// Asserts the decoded PNG has the expected dimensions AND contains at
/// least two distinct pixel colors — a figure that silently failed to
/// draw anything would still produce a valid, uniformly-background-
/// colored PNG, which this check catches and the dimension check alone
/// would not.
fn assert_non_blank_png(bytes: &[u8], expected_dims: (u32, u32)) {
    let (width, height, pixels) = decode_rgba(bytes);
    assert_eq!((width, height), expected_dims, "decoded PNG dimensions must match the export spec");
    assert!(pixels.len() >= 4, "decoded PNG must carry at least one RGBA pixel");
    let first_pixel = &pixels[0..4];
    let has_distinct_pixel = pixels.chunks_exact(4).any(|px| px != first_pixel);
    assert!(has_distinct_pixel, "rendered figure must not be a flat, blank canvas");
}

/// Parse `json`, render it at the shared proof resolution, assert a
/// non-blank PNG, and write it to `nemo/uzor/out/<name>`.
fn parse_render_and_prove(json: &str, name: &str) {
    let spec = parse_spec(json).expect("authored spec JSON must parse");
    let theme = FigureTheme::dark();
    let export_spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: None };
    let rect = Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64);

    let bytes = render_to_png(&export_spec, |ctx| {
        render_spec(&spec, ctx, rect, &theme);
    })
    .expect("figure must render to a valid PNG");

    assert_non_blank_png(&bytes, (WIDTH, HEIGHT));
    write_proof_png(name, &bytes);
}

#[test]
fn bar_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "bar",
        "data": {
            "categories": ["north", "south", "east", "west"],
            "series": [
                {"name": "2025", "values": [42.0, 55.0, 31.0, 47.0]},
                {"name": "2026", "values": [48.0, 51.0, 39.0, 52.0]}
            ],
            "mode": "grouped",
            "title": "Regional volume (authored)",
            "legend_position": "top"
        }
    }"#;
    parse_render_and_prove(json, "registry_bar.png");
}

#[test]
fn curve_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "curve",
        "data": {
            "series": [
                {"name": "signal", "points": [[0.0, 0.0], [1.0, 4.0], [2.0, 2.0], [3.0, 9.0], [4.0, 6.0], [5.0, 12.0]]}
            ],
            "title": "Signal over time (authored)",
            "fill": true
        }
    }"#;
    parse_render_and_prove(json, "registry_curve.png");
}

#[test]
fn histogram_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "histogram",
        "data": {
            "samples": [1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 5.0, 6.0, 6.0, 7.0, 8.0, 8.0, 8.0, 9.0, 10.0],
            "bin_count": 6,
            "title": "Sample distribution (authored)"
        }
    }"#;
    parse_render_and_prove(json, "registry_histogram.png");
}

#[test]
fn timeline_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "timeline",
        "data": {
            "events": [
                {"ts": 1704067200.0, "lane": 0, "label": "first-contact", "kind": 0},
                {"ts": 1704153600.0, "end_ts": 1704499200.0, "lane": 1, "label": "burst-window", "kind": 2},
                {"ts": 1704672000.0, "lane": 0, "label": "checkpoint", "kind": 1}
            ],
            "lane_names": ["actor-a", "actor-b"],
            "title": "Registry timeline (authored)"
        }
    }"#;
    parse_render_and_prove(json, "registry_timeline.png");
}

#[test]
fn sankey_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "sankey",
        "data": {
            "nodes": [
                {"id": "src-a", "label": "src-a", "stage": 0},
                {"id": "mixer-a", "label": "mixer-a", "stage": 1},
                {"id": "sink-a", "label": "sink-a", "stage": 2}
            ],
            "links": [
                {"from": 0, "to": 1, "weight": 40.0, "kind": 0},
                {"from": 1, "to": 2, "weight": 35.0, "kind": 0}
            ],
            "title": "Registry flow (authored)"
        }
    }"#;
    parse_render_and_prove(json, "registry_sankey.png");
}

#[test]
fn pie_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "pie",
        "data": {
            "slices": [
                {"label": "product-a", "value": 400.0},
                {"label": "product-b", "value": 220.0},
                {"label": "product-c", "value": 90.0}
            ],
            "title": "Revenue mix (authored)",
            "legend_position": "right"
        }
    }"#;
    parse_render_and_prove(json, "registry_pie.png");
}

#[test]
fn waterfall_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "waterfall",
        "data": {
            "items": [
                {"label": "opening", "value": 100.0, "kind": "total"},
                {"label": "gain", "value": 30.0, "kind": "delta"},
                {"label": "loss", "value": -15.0, "kind": "delta"},
                {"label": "closing", "value": 0.0, "kind": "total"}
            ],
            "title": "Registry bridge (authored)"
        }
    }"#;
    parse_render_and_prove(json, "registry_waterfall.png");
}

#[test]
fn heatmap_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "heatmap",
        "data": {
            "x_labels": ["wk-0", "wk-1", "wk-2", "wk-3"],
            "y_labels": ["region-0", "region-1", "region-2"],
            "values": [
                [1.0, -3.0, 4.0, 2.0],
                [-2.0, 5.0, -1.0, 3.0],
                [0.0, 1.0, 2.0, -4.0]
            ],
            "diverging_mid": 0.0,
            "title": "Registry heatmap (authored)"
        }
    }"#;
    parse_render_and_prove(json, "registry_heatmap.png");
}

#[test]
fn scatter_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "scatter",
        "data": {
            "points": [
                {"x": 1.0, "y": 4.0, "value": 3.0},
                {"x": 2.0, "y": 2.0},
                {"x": 3.0, "y": 6.0, "value": 8.0},
                {"x": 4.0, "y": 3.0, "value": 5.0},
                {"x": 5.0, "y": 7.0}
            ],
            "title": "Registry scatter (authored)",
            "radius": {"mode": "value_mapped", "min_radius": 2.0, "max_radius": 8.0}
        }
    }"#;
    parse_render_and_prove(json, "registry_scatter.png");
}

#[test]
fn boxplot_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "boxplot",
        "data": {
            "categories": ["group-a", "group-b"],
            "samples": [[1.0, 2.0, 3.0, 4.0, 5.0], [10.0, 12.0, 11.0, 13.0, 25.0]],
            "title": "Registry boxplot (authored)"
        }
    }"#;
    parse_render_and_prove(json, "registry_boxplot.png");
}

#[test]
fn kpi_renders_a_non_blank_proof_png() {
    let json = r#"{
        "kind": "kpi",
        "data": {
            "label": "Revenue",
            "value": 128430.0,
            "previous_value": 110000.0,
            "format": {"kind": "currency", "symbol": "$"}
        }
    }"#;
    parse_render_and_prove(json, "registry_kpi.png");
}
