//! Icon sheet renderer — proves what `uzor::render::draw_svg_icon` actually
//! draws, pixel-exact, no browser involved.
//!
//! Renders every icon in a design JSON file through the REAL
//! [`uzor::render::draw_svg_icon`] path (the same code every toolbar icon in
//! the app renders through) at 24px and 30px, at device pixel ratio 1 and 2,
//! and writes the result as a single PNG sheet: one row per icon, one column
//! per (size, dpr) combination, labelled with the icon id and each column's
//! size/dpr.
//!
//! # Usage
//!
//! ```text
//! cargo run --release -p uzor-render-tiny-skia --example icon_sheet -- \
//!     <icons.json> \
//!     [--stroke-width <W>] [--bg <#RRGGBB>] [--fg <#RRGGBB>] [--out <sheet.png>]
//! ```
//!
//! Defaults: `--stroke-width 1.5 --bg #131722 --fg #d1d4dc --out icon_sheet.png`.
//!
//! `icons.json` shape (unknown top-level fields, e.g. `_note`, are ignored):
//! ```json
//! {
//!   "groups": [
//!     { "title": "Smart Money", "icons": { "order_block": "<path d=\"...\"/>" } }
//!   ]
//! }
//! ```
//! Each icon value is an SVG BODY (no root `<svg>` element — just its child
//! elements concatenated) — this example wraps it in the root template
//! (`viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linecap="round"
//! stroke-linejoin="round"`, `stroke-width` from `--stroke-width`) before handing
//! it to `draw_svg_icon`.

use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Deserialize;

use uzor::render::{draw_svg_icon, Painter, RenderContext, ShapeHelpers, TextAlign, TextBaseline, TextRenderer};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

#[derive(Deserialize)]
struct IconSheetSpec {
    groups: Vec<IconGroup>,
}

#[derive(Deserialize)]
struct IconGroup {
    title: String,
    icons: BTreeMap<String, String>,
}

/// The (logical size px, device pixel ratio) combinations shown per icon, one
/// column each, left to right.
const VARIANTS: [(f64, f64); 4] = [(24.0, 1.0), (24.0, 2.0), (30.0, 1.0), (30.0, 2.0)];

const PADDING: f64 = 16.0;
const LABEL_COL_W: f64 = 170.0;
const CELL_COL_W: f64 = 76.0;
/// The largest physical cell size among `VARIANTS` (30px logical * dpr 2).
const CELL_MAX: f64 = 60.0;
const ROW_H: f64 = CELL_MAX + 12.0;
const HEADER_H: f64 = 22.0;
const GROUP_TITLE_H: f64 = 26.0;

/// Parsed CLI arguments — see the module doc comment for the usage line.
struct Args {
    input: PathBuf,
    stroke_width: f64,
    bg: String,
    fg: String,
    out: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut stroke_width = 1.5_f64;
    let mut bg = "#131722".to_string();
    let mut fg = "#d1d4dc".to_string();
    let mut out = PathBuf::from("icon_sheet.png");

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--stroke-width" => {
                let raw = iter.next().ok_or("--stroke-width needs a value")?;
                stroke_width = raw.parse().map_err(|e| format!("--stroke-width {raw:?}: {e}"))?;
            }
            "--bg" => bg = iter.next().ok_or("--bg needs a value")?,
            "--fg" => fg = iter.next().ok_or("--fg needs a value")?,
            "--out" => out = PathBuf::from(iter.next().ok_or("--out needs a value")?),
            other => positional.push(other.to_string()),
        }
    }

    let input = positional.into_iter().next().ok_or("missing <icons.json> path")?;
    Ok(Args { input: PathBuf::from(input), stroke_width, bg, fg, out })
}

/// Wrap a bare icon body (no root element) in this example's fixed root
/// template — `viewBox="0 0 24 24"`, stroke-only, round caps/joins, and the
/// CLI `--stroke-width`.
fn wrap_svg(body: &str, stroke_width: f64) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="{stroke_width}" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
    )
}

/// Render one icon at `size` logical px, device pixel ratio `dpr`, through the
/// REAL `draw_svg_icon` path, on a fully opaque `bg` fill. Returns the
/// resulting physical RGBA8 pixels (straight alpha — opaque throughout, since
/// the background fill covers the whole cell before the icon is drawn on top)
/// and the physical pixel side length (`size * dpr`, rounded).
fn render_variant(svg: &str, size: f64, dpr: f64, bg: &str, fg: &str) -> (Vec<u8>, u32) {
    let px = (size * dpr).round().max(1.0) as u32;
    let mut ctx = TinySkiaCpuRenderContext::new(px, px, dpr);
    // Map logical coordinates (0..size) onto the physical px*px pixmap — the
    // same integration pattern a real backend caller uses, and the one
    // `draw_svg_icon`'s own `ctx.dpr()` pixel-grid snapping (item 2) assumes.
    ctx.scale(dpr, dpr);
    ctx.set_fill_color(bg);
    ctx.fill_rect(0.0, 0.0, size, size);
    draw_svg_icon(&mut ctx, svg, 0.0, 0.0, size, size, fg);
    (ctx.pixels().to_vec(), px)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("icon_sheet: {e}");
            eprintln!(
                "usage: icon_sheet <icons.json> [--stroke-width W] [--bg #RRGGBB] [--fg #RRGGBB] [--out sheet.png]"
            );
            return ExitCode::FAILURE;
        }
    };

    let raw = match std::fs::read_to_string(&args.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("icon_sheet: failed to read {}: {e}", args.input.display());
            return ExitCode::FAILURE;
        }
    };
    let spec: IconSheetSpec = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("icon_sheet: failed to parse {}: {e}", args.input.display());
            return ExitCode::FAILURE;
        }
    };

    let icon_count: usize = spec.groups.iter().map(|g| g.icons.len()).sum();
    if icon_count == 0 {
        eprintln!("icon_sheet: {} has no icons", args.input.display());
        return ExitCode::FAILURE;
    }

    // Layout pass — compute the sheet size before allocating it.
    let mut sheet_h = PADDING;
    for group in &spec.groups {
        sheet_h += GROUP_TITLE_H + HEADER_H;
        sheet_h += group.icons.len() as f64 * ROW_H;
    }
    sheet_h += PADDING;
    let sheet_w = LABEL_COL_W + VARIANTS.len() as f64 * CELL_COL_W + PADDING * 2.0;

    let mut sheet = TinySkiaCpuRenderContext::new(sheet_w.ceil() as u32, sheet_h.ceil() as u32, 1.0);
    sheet.set_fill_color(&args.bg);
    sheet.fill_rect(0.0, 0.0, sheet_w, sheet_h);

    let mut y = PADDING;
    for group in &spec.groups {
        sheet.set_fill_color(&args.fg);
        sheet.set_font("bold 14px sans-serif");
        sheet.set_text_align(TextAlign::Left);
        sheet.set_text_baseline(TextBaseline::Alphabetic);
        sheet.fill_text(&group.title, PADDING, y + GROUP_TITLE_H - 8.0);
        y += GROUP_TITLE_H;

        sheet.set_font("11px sans-serif");
        sheet.set_text_align(TextAlign::Center);
        for (i, (size, dpr)) in VARIANTS.iter().enumerate() {
            let label = format!("{size:.0}px@{dpr:.0}x");
            let cx = PADDING + LABEL_COL_W + i as f64 * CELL_COL_W + CELL_COL_W / 2.0;
            sheet.fill_text(&label, cx, y + HEADER_H - 6.0);
        }
        y += HEADER_H;

        for (id, body) in &group.icons {
            let svg = wrap_svg(body, args.stroke_width);

            sheet.set_fill_color(&args.fg);
            sheet.set_font("12px sans-serif");
            sheet.set_text_align(TextAlign::Left);
            sheet.set_text_baseline(TextBaseline::Middle);
            sheet.fill_text(id, PADDING, y + ROW_H / 2.0);

            for (i, (size, dpr)) in VARIANTS.iter().enumerate() {
                let (rgba, px) = render_variant(&svg, *size, *dpr, &args.bg, &args.fg);
                let cell_x = PADDING + LABEL_COL_W + i as f64 * CELL_COL_W + (CELL_COL_W - px as f64) / 2.0;
                let cell_y = y + (ROW_H - px as f64) / 2.0;
                // 1:1 blit (dst size == src size) — no resampling, pixel-exact.
                if let Some(painter) = sheet.image_painter() {
                    painter.draw_image_rgba(&rgba, px, px, cell_x, cell_y, px as f64, px as f64);
                }
            }
            y += ROW_H;
        }
    }

    if let Some(parent) = args.out.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("icon_sheet: failed to create {}: {e}", parent.display());
                return ExitCode::FAILURE;
            }
        }
    }
    if let Err(e) = sheet.pixmap().save_png(&args.out) {
        eprintln!("icon_sheet: failed to write {}: {e}", args.out.display());
        return ExitCode::FAILURE;
    }

    println!(
        "icon_sheet: wrote {icon_count} icon(s) across {} group(s) -> {}",
        spec.groups.len(),
        args.out.display()
    );
    ExitCode::SUCCESS
}
