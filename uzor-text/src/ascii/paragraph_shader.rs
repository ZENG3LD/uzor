//! [`ParagraphAsciiShader`] — bridges a real laid-out
//! [`crate::layout::ParagraphLayout`] into [`AsciiGrid`] for the first time
//! (Arc 2 Phase 4's net-new piece; `cell_shader.rs` itself only ever
//! rendered SDF/bitmap-procedural content, never real text — see the design
//! doc's §2.2).
//!
//! Coverage model: for each grid cell, sum the area of every glyph's
//! bounding box (`[glyph.x, glyph.x + glyph.width.max(glyph.advance)]`
//! horizontally, the owning [`crate::layout::LineBox`]'s
//! `[y_top, y_top + height]` vertically — the layout's own already-resolved
//! line geometry, not a fabricated ascent/descent ratio) that overlaps the
//! cell, divide by the cell's area, and clamp to `0.0..=1.0`.
//! [`super::density_char`] maps that coverage straight onto the existing
//! sparse→dense ramp (`" .:-=+*oOX#%@"`) — cells over glyph ink read denser,
//! cells over whitespace/short-line trailing margins read as blank (`' '`,
//! itself `DENSITY`'s own sparsest entry).

use uzor::render::RenderContext;

use crate::layout::ParagraphLayout;

use super::{density_char, AsciiGrid, Cell, CellShader, Coord, Cursor, GridContext};

/// Cell geometry + ink color shared by [`build_ascii_grid`]/
/// [`draw_ascii_grid`]. `cell_w`/`cell_h` must be the same pair passed to
/// both calls: the grid a [`ParagraphAsciiShader`] steps and the box
/// [`AsciiGrid::render`] paints it into have to agree on cell size, or the
/// two disagree about where in the paragraph a given cell samples from.
///
/// `color` is a **flat ink color** — every covered cell paints in this one
/// color regardless of its own coverage fraction (coverage only ever
/// chooses *which character* off `DENSITY`'s ramp, never the color's
/// lightness). Painting color *from* coverage is exactly what produced the
/// near-invisible first render of this bridge: a low-coverage cell would
/// pick both a sparse character AND a near-white color, so on this crate's
/// usual white proof background the whole grid washed out to white-on-white
/// even though the character choices themselves were correct. `color`
/// belongs on the style, not on the shader's own coverage math, so callers
/// pick contrast against their own background once, in one place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AsciiGridStyle {
    pub cell_w: f64,
    pub cell_h: f64,
    pub color: [u8; 3],
}

impl AsciiGridStyle {
    /// Square cells (`cell_w == cell_h == cell_px`), dark-ink default
    /// (`#111111` — matches [`crate::draw::draw_paragraph`]'s own default
    /// text color) suited to a light background. Use [`Self::with_color`]
    /// for a light-on-dark grid.
    pub fn square(cell_px: f64) -> Self {
        Self { cell_w: cell_px, cell_h: cell_px, color: [0x11, 0x11, 0x11] }
    }

    /// Builder: paint covered cells in `color` instead of the default dark ink.
    pub fn with_color(mut self, color: [u8; 3]) -> Self {
        self.color = color;
        self
    }
}

/// Grid dimensions (`cols`, `rows`) that exactly cover `layout` at
/// `cell_w × cell_h` px per cell: `ceil(layout.width / cell_w)` /
/// `ceil(layout.height / cell_h)`, each floored to a minimum of `1` so a
/// degenerate (empty or zero-size) layout still yields a paintable
/// single-cell grid instead of a `0×0` one.
pub fn grid_dims_for_layout(layout: &ParagraphLayout, cell_w: f64, cell_h: f64) -> (usize, usize) {
    let cols = if cell_w > 0.0 { (layout.width / cell_w).ceil().max(1.0) as usize } else { 1 };
    let rows = if cell_h > 0.0 { (layout.height / cell_h).ceil().max(1.0) as usize } else { 1 };
    (cols, rows)
}

/// A [`CellShader`] that samples glyph-box coverage from a real
/// [`ParagraphLayout`] instead of an SDF/bitmap procedural test — the bridge
/// this phase adds.
///
/// `cell_w`/`cell_h` must be the exact cell geometry [`grid_dims_for_layout`]
/// sized the target [`AsciiGrid`] with: [`GridContext::aspect`] alone (a
/// ratio) can't recover the absolute cell size a coordinate needs to map
/// back into the paragraph's pixel space, so this shader carries its own
/// copy rather than reading it back out of the grid context.
pub struct ParagraphAsciiShader<'a> {
    pub layout: &'a ParagraphLayout,
    pub cell_w: f64,
    pub cell_h: f64,
    /// Text color for covered cells. This bridge has no per-glyph color
    /// story yet (unlike [`crate::draw::draw_paragraph`]) — every covered
    /// cell paints in one flat color, only density/character varies.
    pub color: [u8; 3],
}

impl<'a> ParagraphAsciiShader<'a> {
    /// `layout` sampled at `cell_w × cell_h` px per cell, painted in a dark
    /// `#111111` default ink ([`build_ascii_grid`] always overrides this
    /// from the caller's [`AsciiGridStyle::color`] instead — this default
    /// only matters for direct callers of this constructor).
    pub fn new(layout: &'a ParagraphLayout, cell_w: f64, cell_h: f64) -> Self {
        Self { layout, cell_w, cell_h, color: [0x11, 0x11, 0x11] }
    }

    /// Builder: paint covered cells in `color` instead of the default dark ink.
    pub fn with_color(mut self, color: [u8; 3]) -> Self {
        self.color = color;
        self
    }

    /// Fraction (`0.0..=1.0`) of the grid cell at `coord` covered by glyph
    /// boxes, summed over every non-whitespace glyph on every line whose
    /// vertical span overlaps the cell.
    fn coverage(&self, coord: Coord) -> f64 {
        let cell_area = self.cell_w * self.cell_h;
        if cell_area <= 0.0 {
            return 0.0;
        }

        let cx0 = coord.x as f64 * self.cell_w;
        let cx1 = cx0 + self.cell_w;
        let cy0 = coord.y as f64 * self.cell_h;
        let cy1 = cy0 + self.cell_h;

        let mut covered = 0.0_f64;
        for line in &self.layout.lines {
            let ly0 = line.y_top;
            let ly1 = line.y_top + line.height;
            if ly1 <= cy0 || ly0 >= cy1 {
                continue;
            }
            let iy0 = ly0.max(cy0);
            let iy1 = ly1.min(cy1);

            for glyph in self.layout.glyphs.iter().filter(|g| g.line_index == line.line_index) {
                if glyph.cluster.trim().is_empty() {
                    continue;
                }
                let gx0 = glyph.x;
                let gx1 = glyph.x + glyph.width.max(glyph.advance);
                if gx1 <= cx0 || gx0 >= cx1 {
                    continue;
                }
                let ix0 = gx0.max(cx0);
                let ix1 = gx1.min(cx1);
                covered += (ix1 - ix0) * (iy1 - iy0);
            }
        }
        (covered / cell_area).clamp(0.0, 1.0)
    }
}

impl<'a> CellShader for ParagraphAsciiShader<'a> {
    fn main(&self, coord: Coord, _ctx: &GridContext, _cursor: &Cursor) -> Cell {
        let v = self.coverage(coord);
        if v <= 0.02 {
            return Cell { ch: ' ', color: self.color, alpha: 1.0, scale: 1.0 };
        }
        Cell { ch: density_char(v), color: self.color, alpha: 1.0, scale: 1.0 }
    }
}

/// Build a grid sized by [`grid_dims_for_layout`], sample every cell's
/// glyph-box coverage through [`ParagraphAsciiShader`], and return the
/// stepped grid ready to paint via [`draw_ascii_grid`].
///
/// One-shot, not per-frame (design law 5: layout/derived-render work is
/// recomputed only on content/width change) — steps with `time = 0.0`, no
/// glitch/cursor interaction; this bridge is a static ASCII render of a
/// paragraph, not a `GlitchLetter`-style live effect.
pub fn build_ascii_grid(layout: &ParagraphLayout, style: AsciiGridStyle) -> AsciiGrid {
    let (cols, rows) = grid_dims_for_layout(layout, style.cell_w, style.cell_h);
    let mut grid = AsciiGrid::new(cols, rows);
    let shader = ParagraphAsciiShader::new(layout, style.cell_w, style.cell_h).with_color(style.color);
    let aspect = if style.cell_h > 0.0 { style.cell_w / style.cell_h } else { 1.0 };
    grid.step(&shader, 0.0, aspect);
    grid
}

/// Paint a stepped [`AsciiGrid`] at `origin` — a thin wrapper over
/// [`AsciiGrid::render`] (design law 6: no new drawing primitive; this only
/// spares the caller re-deriving `cell_w`/`cell_h` as two loose `f64`s after
/// already building the grid with one [`AsciiGridStyle`]).
pub fn draw_ascii_grid(ctx: &mut dyn RenderContext, origin: (f64, f64), grid: &AsciiGrid, style: AsciiGridStyle) {
    grid.render(ctx, origin.0, origin.1, style.cell_w, style.cell_h);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uzor::fonts::FontFamily;
    use uzor_export::{render_to_png, ExportSpec};

    use super::{build_ascii_grid, draw_ascii_grid, grid_dims_for_layout, AsciiGrid, AsciiGridStyle};
    use crate::layout::layout_text;
    use crate::model::FontSpec;
    use crate::shape::CosmicShaper;

    /// Fixed seeded sample paragraph — no lorem-ipsum RNG (design law 8).
    /// Deliberately ends on a short last line (ordinary prose wraps ragged)
    /// so the trailing-margin test below has a real short line to check.
    const PARAGRAPH: &str = "A short seeded paragraph for the uzor-text ASCII \
        bridge test, wrapped across a few lines by the greedy word-wrapper, \
        ending short.";

    fn layout_fixture(max_width: f64) -> crate::layout::ParagraphLayout {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        layout_text(PARAGRAPH, &font, max_width, &shaper)
    }

    #[test]
    fn grid_dims_match_ceil_of_layout_size_over_cell() {
        let layout = layout_fixture(200.0);
        let cell_w = 8.0;
        let cell_h = 14.0;

        let (cols, rows) = grid_dims_for_layout(&layout, cell_w, cell_h);

        assert_eq!(cols, (layout.width / cell_w).ceil().max(1.0) as usize);
        assert_eq!(rows, (layout.height / cell_h).ceil().max(1.0) as usize);
        assert!(cols >= 1 && rows >= 1);
    }

    #[test]
    fn degenerate_empty_layout_yields_a_paintable_single_cell_grid() {
        let layout = crate::layout::ParagraphLayout::default();
        let (cols, rows) = grid_dims_for_layout(&layout, 8.0, 14.0);
        assert_eq!((cols, rows), (1, 1));
    }

    #[test]
    fn every_cell_char_is_in_the_density_ramp() {
        let layout = layout_fixture(200.0);
        let style = AsciiGridStyle::square(10.0);
        let grid = build_ascii_grid(&layout, style);

        let allowed: Vec<char> = crate::ascii::DENSITY.iter().map(|&b| b as char).collect();
        for y in 0..grid.rows() {
            for x in 0..grid.cols() {
                let ch = grid.cell(x, y).ch;
                assert!(allowed.contains(&ch), "cell ({x},{y}) char {ch:?} not in the DENSITY ramp");
            }
        }
    }

    /// Every visual line's own row band contains at least one non-blank
    /// (glyph-covered) cell — text rows read as non-empty, not all-space.
    #[test]
    fn every_line_row_band_has_at_least_one_non_blank_cell() {
        let layout = layout_fixture(200.0);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        let cell_w = 8.0;
        let cell_h = 8.0;
        let grid = build_ascii_grid(&layout, AsciiGridStyle::square(cell_w));

        for line in &layout.lines {
            let row_top = (line.y_top / cell_h).floor() as usize;
            let row_bottom = ((line.y_top + line.height) / cell_h).ceil().max(row_top as f64 + 1.0) as usize;
            let mut has_ink = false;
            for y in row_top..row_bottom.min(grid.rows()) {
                for x in 0..grid.cols() {
                    if grid.cell(x, y).ch != ' ' {
                        has_ink = true;
                    }
                }
            }
            assert!(has_ink, "line {} row band [{row_top},{row_bottom}) has no non-blank cell", line.line_index);
        }
    }

    /// The last (ragged, shorter-than-widest) line's trailing columns —
    /// beyond its own `content_width` — read as blank margin, while its own
    /// text columns still carry ink.
    ///
    /// Uses its own fixture (not [`layout_fixture`]/`PARAGRAPH`): many
    /// identical repeated words, each filling a full line, followed by one
    /// tiny trailing word — guarantees the last line is short by
    /// construction, rather than hoping ordinary prose happens to wrap
    /// ragged at this exact `max_width`/font pairing.
    #[test]
    fn last_line_trailing_columns_beyond_its_content_are_blank_margin() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let text = "wide ".repeat(20) + "x.";
        let layout = layout_text(&text, &font, 150.0, &shaper);

        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        let last_line = layout.lines.last().expect("at least one line");
        assert!(
            last_line.content_width < layout.width - 1.0,
            "fixture's last line must be shorter than the widest line, got {} vs {}",
            last_line.content_width,
            layout.width
        );

        let cell_w = 8.0;
        let cell_h = 8.0;
        let grid = build_ascii_grid(&layout, AsciiGridStyle::square(cell_w));

        let row_top = (last_line.y_top / cell_h).floor() as usize;
        let row_bottom = ((last_line.y_top + last_line.height) / cell_h).ceil() as usize;
        // +2 cells of slack past the line's own content so a partially
        // covered boundary cell never counts as "margin".
        let margin_col_start = (last_line.content_width / cell_w).ceil() as usize + 2;

        let mut found_text_cell = false;
        let mut margin_all_blank = true;
        for y in row_top..row_bottom.min(grid.rows()) {
            for x in 0..grid.cols() {
                let ch = grid.cell(x, y).ch;
                if x < margin_col_start && ch != ' ' {
                    found_text_cell = true;
                }
                if x >= margin_col_start && ch != ' ' {
                    margin_all_blank = false;
                }
            }
        }
        assert!(found_text_cell, "expected non-blank cells over the last line's own text");
        assert!(margin_all_blank, "expected trailing margin cells beyond the last line's content to be blank");
    }

    // ── Headless proof (design law 8: one screenshot is never proof — this
    // pairs with `cell_shader`'s own GlitchLetter proof) ──────────────────

    const WIDTH: u32 = 600;
    const HEIGHT: u32 = 400;
    const MARGIN: f64 = 20.0;

    fn out_dir() -> PathBuf {
        // Fixed path — `uzor/out/` is the shared human-eyeball drop point
        // for every headless proof render in this workspace (matches
        // `crate::draw`'s own proof tests).
        PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof_png(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof PNG");
    }

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
    }

    /// Darkest single RGB channel value found in the decoded PNG — used to
    /// regression-test against a washed-out (near-`255`, i.e. near-white
    /// background) render: real ink against a white background always
    /// drives this well below `255`.
    fn decoded_png_darkest_channel(bytes: &[u8]) -> u8 {
        let decoder = png::Decoder::new(bytes);
        let mut reader = decoder.read_info().expect("valid PNG header");
        let mut buf = vec![0u8; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("decode PNG frame");
        let bytes_per_pixel = match info.color_type {
            png::ColorType::Rgba => 4,
            png::ColorType::Rgb => 3,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Grayscale | png::ColorType::Indexed => 1,
        };
        buf[..info.buffer_size()]
            .chunks(bytes_per_pixel)
            .flat_map(|px| px.iter().take(3).copied())
            .min()
            .unwrap_or(255)
    }

    fn ascii_paragraph_proof_grid() -> (crate::layout::ParagraphLayout, AsciiGridStyle, AsciiGrid) {
        let max_width = (WIDTH as f64) - 2.0 * MARGIN;
        let layout = layout_fixture(max_width);
        let style = AsciiGridStyle::square(9.0);
        let grid = build_ascii_grid(&layout, style);
        (layout, style, grid)
    }

    #[test]
    fn ascii_rendered_paragraph_renders_to_a_valid_png() {
        let (layout, style, grid) = ascii_paragraph_proof_grid();
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            draw_ascii_grid(ctx, (MARGIN, MARGIN), &grid, style);
        })
        .unwrap_or_else(|e| panic!("ascii paragraph proof render should succeed: {e}"));

        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("text_p4_ascii_paragraph.png", &bytes);
    }

    /// Regression test for the washed-out-on-white defect: the same proof
    /// render's darkest pixel channel must sit clearly below the white
    /// (`255`) background — catches a shader that paints coverage-derived
    /// near-white ink (this bridge's first cut used `Cell::default`'s own
    /// `[244, 244, 245]`, invisible against a white proof background) even
    /// though the *characters* chosen were already correct.
    #[test]
    fn ascii_rendered_paragraph_ink_is_not_washed_out_against_white_background() {
        let (layout, style, grid) = ascii_paragraph_proof_grid();
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");

        let spec = ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([255, 255, 255, 255]) };
        let bytes = render_to_png(&spec, |ctx| {
            draw_ascii_grid(ctx, (MARGIN, MARGIN), &grid, style);
        })
        .unwrap_or_else(|e| panic!("ascii paragraph proof render should succeed: {e}"));

        let darkest = decoded_png_darkest_channel(&bytes);
        assert!(
            darkest < 200,
            "expected at least one clearly non-background (dark ink) pixel, darkest channel found = {darkest}"
        );
    }
}
