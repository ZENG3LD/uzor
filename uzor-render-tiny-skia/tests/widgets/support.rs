//! Shared golden-grid test support (H1 Brief 10a-1, design doc §5): iterate
//! every built-in token set, render a widget on that set's own real
//! background (never white — H0's original button goldens got this wrong),
//! and compose one labelled review-sheet PNG per widget for a human to
//! eyeball every set × state at a glance.

#![cfg(feature = "golden")]

use std::path::PathBuf;
use std::sync::Arc;

use tiny_skia::{Color, IntSize, Pixmap};

use uzor::render::{Painter, TextAlign, TextBaseline, TextRenderer};
use uzor::tokens::{BuiltinSet, Tokens};
use uzor_render_tiny_skia::golden::{compare_or_bless, GoldenError, GoldenTolerance};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

/// Runs `f` once per built-in token set (H1 §1 D5: the 4 generic sets).
pub fn for_each_set(mut f: impl FnMut(BuiltinSet, Arc<Tokens>)) {
    for &set in BuiltinSet::ALL {
        f(set, Tokens::builtin(set));
    }
}

/// Lowercase set name matching the golden directory layout
/// (`tests/goldens/<widget>/<set>/<state>.png`).
pub fn set_name(set: BuiltinSet) -> &'static str {
    match set {
        BuiltinSet::Dark => "dark",
        BuiltinSet::Light => "light",
        BuiltinSet::HighContrast => "high_contrast",
        BuiltinSet::HighContrastMono => "high_contrast_mono",
    }
}

/// A blank `width`×`height` canvas cleared to `set`'s real
/// `semantic.surface_app_chrome` — widgets are drawn on their real
/// background, never white.
pub fn canvas(width: u32, height: u32, set: BuiltinSet) -> TinySkiaCpuRenderContext {
    let tokens = Tokens::builtin(set);
    let mut ctx = TinySkiaCpuRenderContext::new(width, height, 1.0);
    let [r, g, b, a] = tokens.semantic.surface_app_chrome.to_rgba8();
    ctx.clear(Color::from_rgba8(r, g, b, a));
    ctx
}

/// Compares/blesses `<widget>/<set>/<state>` against the committed golden
/// PNG (`UZOR_BLESS=1` (re)writes it — see `uzor_render_tiny_skia::golden`).
pub fn golden(
    widget: &str,
    set: BuiltinSet,
    state: &str,
    ctx: &TinySkiaCpuRenderContext,
) -> Result<(), GoldenError> {
    let name = format!("{widget}/{}/{state}", set_name(set));
    compare_or_bless(&name, ctx.pixels(), ctx.width(), ctx.height(), GoldenTolerance::default())
}

/// `<workspace root>/target` — the same resolution rule as
/// `uzor_render_tiny_skia::golden`'s own (crate-private) target directory
/// helper: `CARGO_TARGET_DIR` if set, else this crate's manifest dir's
/// parent (this crate is a direct member of the `uzor-next` workspace).
fn target_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|workspace_root| workspace_root.join("target"))
        .unwrap_or_else(|| PathBuf::from("target"))
}

const LABEL_HEIGHT: u32 = 16;
const LABEL_FONT: &str = "11px monospace";

/// Composes every set × state render for one widget into a single labelled
/// PNG — a review artefact for a human to eyeball, never committed and
/// never diffed against (unlike the per-cell goldens `golden()` compares).
///
/// Assumes every cell added for one sheet shares the same pixel dimensions
/// (true for every widget in this grid: the canvas size is fixed per
/// widget, only the token set and interaction state vary).
#[derive(Default)]
pub struct ReviewSheet {
    /// One row per state (insertion order), `BuiltinSet::ALL`-ordered
    /// columns within each row: `(set_label, width, height, rgba_pixels)`.
    rows: Vec<(String, Vec<(String, u32, u32, Vec<u8>)>)>,
}

impl ReviewSheet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one already-rendered, unlabelled cell. Never mutates the
    /// buffer used for golden comparison — the label is only ever baked
    /// into this sheet's own composed copy at [`Self::save`] time.
    pub fn add(&mut self, set: BuiltinSet, state: &str, ctx: &TinySkiaCpuRenderContext) {
        let row = match self.rows.iter().position(|(s, _)| s == state) {
            Some(idx) => idx,
            None => {
                self.rows.push((state.to_string(), Vec::new()));
                self.rows.len() - 1
            }
        };
        self.rows[row].1.push((
            set_name(set).to_string(),
            ctx.width(),
            ctx.height(),
            ctx.pixels().to_vec(),
        ));
    }

    /// Renders a small label badge — dark background strip plus white text,
    /// always legible regardless of the cell's own colour set (including
    /// `high_contrast_mono`, which is otherwise fully grayscale).
    fn label_strip(text: &str, width: u32) -> TinySkiaCpuRenderContext {
        let mut ctx = TinySkiaCpuRenderContext::new(width.max(1), LABEL_HEIGHT, 1.0);
        ctx.clear(Color::BLACK);
        ctx.set_font(LABEL_FONT);
        ctx.set_fill_color("#ffffff");
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Middle);
        ctx.fill_text(text, 3.0, LABEL_HEIGHT as f64 / 2.0);
        ctx
    }

    /// Composes the accumulated cells into `<target>/golden-sheets/<widget>.png`.
    pub fn save(&self, widget: &str) {
        if self.rows.is_empty() {
            return;
        }
        let cols = self.rows[0].1.len();
        if cols == 0 {
            return;
        }
        let (_, cell_w, cell_h, _) = &self.rows[0].1[0];
        let (cell_w, cell_h) = (*cell_w, *cell_h);
        let sheet_w = cell_w * cols as u32;
        let sheet_h = (cell_h + LABEL_HEIGHT) * self.rows.len() as u32;

        let mut buf = vec![0u8; (sheet_w as usize) * (sheet_h as usize) * 4];
        let stride = sheet_w as usize * 4;

        for (row_idx, (state, cells)) in self.rows.iter().enumerate() {
            let row_y = row_idx as u32 * (cell_h + LABEL_HEIGHT);
            for (col_idx, (set, w, h, pixels)) in cells.iter().enumerate() {
                let col_x = col_idx as u32 * cell_w;
                let label = Self::label_strip(&format!("{set}/{state}"), cell_w);
                blit(&mut buf, stride, col_x, row_y, cell_w, LABEL_HEIGHT, label.pixels());
                blit(&mut buf, stride, col_x, row_y + LABEL_HEIGHT, *w, *h, pixels);
            }
        }

        let path = target_dir().join("golden-sheets").join(format!("{widget}.png"));
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Some(size) = IntSize::from_wh(sheet_w, sheet_h) else { return };
        if let Some(pixmap) = Pixmap::from_vec(buf, size) {
            let _ = pixmap.save_png(&path);
        }
    }
}

/// Copies a `w`×`h` premultiplied-RGBA8 block into `dst` (row stride
/// `dst_stride` bytes) at pixel offset `(x, y)`. Plain replace, not an
/// alpha blend — every cell tiles the sheet with no overlap.
fn blit(dst: &mut [u8], dst_stride: usize, x: u32, y: u32, w: u32, h: u32, src: &[u8]) {
    let src_stride = w as usize * 4;
    for row in 0..h {
        let dst_start = (y + row) as usize * dst_stride + x as usize * 4;
        let src_start = row as usize * src_stride;
        dst[dst_start..dst_start + src_stride].copy_from_slice(&src[src_start..src_start + src_stride]);
    }
}
