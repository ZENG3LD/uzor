//! End-to-end proof over REAL authored `.figure.json` FILES (not inline
//! JSON literals — `tests/assets/*.figure.json`), the actual "authored
//! content unlocked by this registry" workflow the task's own mission
//! describes. Each fixture is embedded at compile time via `include_str!`
//! (portable across OSes/CI, no `CARGO_MANIFEST_DIR`-relative filesystem
//! read needed at test time) and proven the same JSON -> spec -> render
//! -> non-blank PNG way `tests/proof.rs` proves every variant.

use std::path::PathBuf;

use uzor::types::Rect;
use uzor_export::{render_to_png, ExportSpec};
use uzor_figures::FigureTheme;
use uzor_figures_registry::{parse_spec, render_spec};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 500;

fn out_dir() -> PathBuf {
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

fn assert_non_blank_png(bytes: &[u8], expected_dims: (u32, u32)) {
    let (width, height, pixels) = decode_rgba(bytes);
    assert_eq!((width, height), expected_dims);
    assert!(pixels.len() >= 4);
    let first_pixel = &pixels[0..4];
    assert!(pixels.chunks_exact(4).any(|px| px != first_pixel), "rendered fixture must not be a flat, blank canvas");
}

fn prove_fixture(json: &str, name: &str) {
    let spec = parse_spec(json).expect("authored fixture file must parse");
    let theme = FigureTheme::dark();
    let export_spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: None };
    let rect = Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64);
    let bytes = render_to_png(&export_spec, |ctx| render_spec(&spec, ctx, rect, &theme)).expect("authored fixture must render");
    assert_non_blank_png(&bytes, (WIDTH, HEIGHT));
    write_proof_png(name, &bytes);
}

#[test]
fn revenue_bar_fixture_parses_and_renders() {
    prove_fixture(include_str!("assets/revenue_bar.figure.json"), "registry_fixture_revenue_bar.png");
}

#[test]
fn money_flow_fixture_parses_and_renders() {
    prove_fixture(include_str!("assets/money_flow.figure.json"), "registry_fixture_money_flow.png");
}

#[test]
fn kpi_dashboard_fixture_parses_and_renders() {
    prove_fixture(include_str!("assets/kpi_dashboard.figure.json"), "registry_fixture_kpi_dashboard.png");
}
