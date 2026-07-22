//! Pseudo-pixel canvas over the cell buffer — braille (2x4 dots/cell, via
//! the Unicode braille block `U+2800..=U+28FF`) and half-block (1x2 px/
//! cell, via `▀`/`▄`/`█` + fg/bg colors) sub-cell drawing surfaces, plus a
//! thin adapter so `uzor-figures`' curve/bar rendering can paint into a
//! TUI rect without pulling the GPU/CPU render stack into this crate.
//!
//! Reference precedent (per the shelf brief): ratatui's own `Canvas`
//! widget (`Marker::{Braille, HalfBlock}`) and `plotext`'s terminal
//! plotting braille grid — the SAME two sub-cell-resolution techniques,
//! reimplemented here directly against this crate's own [`TerminalBuffer`]
//! (no new dependency).
//!
//! ## Why this is a plain fn adapter, not a `RenderContext` impl (report)
//!
//! `uzor::render::RenderContext` is a rich supertrait (`Painter` +
//! `TextMetrics` + `ShapeHelpers` + `BatchPainter` + `GradientPainter` —
//! see `uzor/src/core/render/context.rs`): real bezier paths, gradients,
//! font-shaped text metrics, batched draw calls. None of that maps onto a
//! braille/half-block dot grid in any useful way (a "path fill" over an
//! 8-dot cell is meaningless; there is no font to shape "text metrics"
//! against). Implementing the full trait would mean dozens of methods
//! either panicking, silently no-op'ing, or crudely approximating
//! capabilities this surface fundamentally cannot support — exactly the
//! kind of surface-area-for-its-own-sake this workspace's own conventions
//! warn against. Per the shelf brief's own instruction, this module
//! instead offers ONLY the "honest cheap path": [`plot_series`] (a
//! polyline through a slice of `(x, y)` points, domain-mapped into pixel
//! space) and [`plot_bars`] (filled columns from a shared baseline) —
//! the two primitives `uzor-figures`' `CurveFigure`/`BarFigure` actually
//! need to read data through, ported as free functions operating
//! directly on [`PixelCanvas`], not through `RenderContext` at all.

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::rect::Rect;
use crate::style::{Color, Style};

/// Sub-cell drawing resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasMode {
    /// 2 (horizontal) x 4 (vertical) dots per cell, painted as ONE
    /// Unicode braille glyph (`U+2800 + dot-bitmask`) — 8x the addressable
    /// resolution of a plain character cell. A braille GLYPH carries only
    /// ONE foreground color (there is no such thing as a "half-braille-cell
    /// background" the way there is for [`CanvasMode::HalfBlock`]) — see
    /// [`PixelCanvas::flush`]'s own doc comment for the deterministic
    /// per-cell color rule this mode uses when more than one dot in the
    /// same cell was painted with different colors.
    Braille,
    /// 1 (horizontal) x 2 (vertical) px per cell, painted via `▀`
    /// (upper-half-block)/`▄`(lower-half-block)/`█`(full block), the
    /// top pixel's color as `fg` and the bottom pixel's as `bg` — real,
    /// independent per-pixel color (unlike [`CanvasMode::Braille`]), at
    /// 1/4 the dot density.
    HalfBlock,
}

/// A pseudo-pixel drawing surface over `cell_width x cell_height`
/// terminal cells, addressed in its own sub-cell pixel space
/// ([`PixelCanvas::px_width`] x [`PixelCanvas::px_height`], derived from
/// `cell_width`/`cell_height` and [`CanvasMode`]). Pixel `(0, 0)` is the
/// TOP-LEFT, `y` increasing downward — matching this crate's own
/// [`TerminalBuffer`] row convention, NOT a math/plot convention where
/// `y` increases upward (see [`plot_series`]/[`plot_bars`]'s own doc
/// comments for where that flip actually happens).
#[derive(Debug, Clone)]
pub struct PixelCanvas {
    mode: CanvasMode,
    cell_width: u16,
    cell_height: u16,
    px_width: u32,
    px_height: u32,
    /// Row-major, `px_width * px_height` — `None` = unset (no ink at
    /// this sub-cell pixel).
    pixels: Vec<Option<Color>>,
}

/// Dots per cell, horizontal/vertical, for [`CanvasMode::Braille`].
const BRAILLE_DOTS_X: u32 = 2;
const BRAILLE_DOTS_Y: u32 = 4;
/// Px per cell, horizontal/vertical, for [`CanvasMode::HalfBlock`].
const HALF_BLOCK_PX_X: u32 = 1;
const HALF_BLOCK_PX_Y: u32 = 2;

/// Braille dot bit for `(local_x, local_y)` within one cell's own `2x4`
/// sub-grid — the standard 8-dot braille numbering (dots 1/2/3/7 down the
/// left column, 4/5/6/8 down the right column), each dot `N` occupying
/// bit `N - 1` of the `U+2800`-relative codepoint offset.
fn braille_bit(local_x: u32, local_y: u32) -> u8 {
    match (local_x, local_y) {
        (0, 0) => 0x01, // dot 1
        (0, 1) => 0x02, // dot 2
        (0, 2) => 0x04, // dot 3
        (0, 3) => 0x40, // dot 7
        (1, 0) => 0x08, // dot 4
        (1, 1) => 0x10, // dot 5
        (1, 2) => 0x20, // dot 6
        (1, 3) => 0x80, // dot 8
        _ => 0,
    }
}

impl PixelCanvas {
    /// Construct a canvas over `cell_width x cell_height` terminal cells.
    /// A zero-sized dimension is a valid, empty canvas (every subsequent
    /// `set_pixel`/`line` call is simply a no-op against it — never a
    /// panic).
    pub fn new(mode: CanvasMode, cell_width: u16, cell_height: u16) -> Self {
        let (dots_x, dots_y) = match mode {
            CanvasMode::Braille => (BRAILLE_DOTS_X, BRAILLE_DOTS_Y),
            CanvasMode::HalfBlock => (HALF_BLOCK_PX_X, HALF_BLOCK_PX_Y),
        };
        let px_width = cell_width as u32 * dots_x;
        let px_height = cell_height as u32 * dots_y;
        let pixels = vec![None; (px_width as usize) * (px_height as usize)];
        Self { mode, cell_width, cell_height, px_width, px_height, pixels }
    }

    pub fn mode(&self) -> CanvasMode {
        self.mode
    }

    pub fn cell_width(&self) -> u16 {
        self.cell_width
    }

    pub fn cell_height(&self) -> u16 {
        self.cell_height
    }

    /// Addressable pixel-space width (`cell_width * 2` for
    /// [`CanvasMode::Braille`], `cell_width * 1` for
    /// [`CanvasMode::HalfBlock`]).
    pub fn px_width(&self) -> u32 {
        self.px_width
    }

    /// Addressable pixel-space height (`cell_height * 4` for
    /// [`CanvasMode::Braille`], `cell_height * 2` for
    /// [`CanvasMode::HalfBlock`]).
    pub fn px_height(&self) -> u32 {
        self.px_height
    }

    #[inline]
    fn pixel_index(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || y < 0 || x as u32 >= self.px_width || y as u32 >= self.px_height {
            return None;
        }
        Some((y as usize) * (self.px_width as usize) + (x as usize))
    }

    /// Paint pixel `(x, y)` with `color`. Silently clipped (a no-op, never
    /// a panic) when `(x, y)` falls outside `[0, px_width) x [0,
    /// px_height)` — the same "off-canvas input degrades quietly" stance
    /// every other pure layout function in this workspace's sibling
    /// figure/guide crates already takes for malformed/out-of-range input.
    pub fn set_pixel(&mut self, x: i64, y: i64, color: Color) {
        if let Some(idx) = self.pixel_index(x, y) {
            self.pixels[idx] = Some(color);
        }
    }

    /// Erase pixel `(x, y)` back to unset. Silently clipped, same as
    /// [`PixelCanvas::set_pixel`].
    pub fn clear_pixel(&mut self, x: i64, y: i64) {
        if let Some(idx) = self.pixel_index(x, y) {
            self.pixels[idx] = None;
        }
    }

    /// Draw a straight line from `(x0, y0)` to `(x1, y1)` (both endpoints
    /// inclusive) via Bresenham's integer algorithm — every intermediate
    /// pixel individually clipped through [`PixelCanvas::set_pixel`], so
    /// a line that runs partly off-canvas simply paints its own in-bounds
    /// segment.
    pub fn line(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, color: Color) {
        let dx = (x1 - x0).abs();
        let sx: i64 = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy: i64 = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);

        loop {
            self.set_pixel(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Erase every pixel back to unset.
    pub fn clear(&mut self) {
        for p in &mut self.pixels {
            *p = None;
        }
    }

    /// Cell-local color for cell `(cx, cy)`'s own dot/pixel-2 set — the
    /// LAST non-`None` color found scanning local `(x, y)` in ascending
    /// row-major order (`y` outer, `x` inner) — a deterministic,
    /// documented tie-break for [`CanvasMode::Braille`] (where a whole
    /// cell shares exactly one glyph color regardless of how many of its
    /// own 8 dots are individually lit).
    fn braille_cell_color(&self, cx: u16, cy: u16) -> Option<Color> {
        let mut chosen = None;
        for local_y in 0..BRAILLE_DOTS_Y {
            for local_x in 0..BRAILLE_DOTS_X {
                let px = cx as u32 * BRAILLE_DOTS_X + local_x;
                let py = cy as u32 * BRAILLE_DOTS_Y + local_y;
                if let Some(idx) = self.pixel_index(px as i64, py as i64) {
                    if let Some(c) = self.pixels[idx] {
                        chosen = Some(c);
                    }
                }
            }
        }
        chosen
    }

    /// Render this canvas into `buf` at `area`'s own top-left, clamped to
    /// `min(area.width, cell_width) x min(area.height, cell_height)` — a
    /// smaller `area` than this canvas's own cell dimensions crops
    /// (never panics); a larger one simply leaves the extra cells
    /// untouched.
    ///
    /// [`CanvasMode::Braille`]: each cell's own bitmask (its 8 dots,
    /// [`braille_bit`]) selects `U+2800 + bitmask` (falling back to the
    /// blank braille glyph `'⠀'` — U+2800 itself, `bitmask == 0` — for the
    /// impossible `char::from_u32` failure case too; every value in
    /// `0..=255` is a valid codepoint offset here, this is pure defensive
    /// belt-and-suspenders, never actually reachable); the cell's own
    /// [`PixelCanvas::braille_cell_color`] becomes the glyph's `fg`. An
    /// UNSET cell (no dots lit at all) therefore paints the BLANK
    /// BRAILLE glyph `U+2800` — not an ASCII space — with default style,
    /// clearing whatever was in that buffer cell before (the standard
    /// braille-canvas convention: every cell in braille mode stays a real
    /// braille glyph, so column alignment/width reads identically across
    /// the whole grid regardless of which cells happen to be lit).
    ///
    /// [`CanvasMode::HalfBlock`]: top pixel -> `fg`, bottom pixel -> `bg`.
    /// Both set to the SAME color collapse to a solid `█` (`fg` = that
    /// color, `bg` untouched — a single glyph reads identically to two
    /// half-blocks of the same color, and avoids emitting a
    /// distinguishable-but-pointless `bg` write). Only top set -> `▀`
    /// (`fg` = top, `bg` left at [`Color::Reset`]). Only bottom set ->
    /// `▄` (`fg` = bottom). Neither set -> a plain space, default style.
    pub fn flush(&self, area: Rect, buf: &mut TerminalBuffer) {
        let cols = area.width.min(self.cell_width);
        let rows = area.height.min(self.cell_height);

        for cy in 0..rows {
            for cx in 0..cols {
                let (symbol, style): (char, Style) = match self.mode {
                    CanvasMode::Braille => {
                        let mut bitmask: u8 = 0;
                        for local_y in 0..BRAILLE_DOTS_Y {
                            for local_x in 0..BRAILLE_DOTS_X {
                                let px = cx as u32 * BRAILLE_DOTS_X + local_x;
                                let py = cy as u32 * BRAILLE_DOTS_Y + local_y;
                                if let Some(idx) = self.pixel_index(px as i64, py as i64) {
                                    if self.pixels[idx].is_some() {
                                        bitmask |= braille_bit(local_x, local_y);
                                    }
                                }
                            }
                        }
                        let ch = char::from_u32(0x2800 + bitmask as u32).unwrap_or('⠀');
                        let color = self.braille_cell_color(cx, cy);
                        (ch, color.map(|c| Style::default().fg(c)).unwrap_or_default())
                    }
                    CanvasMode::HalfBlock => {
                        let top_idx = self.pixel_index(cx as i64, cy as i64 * HALF_BLOCK_PX_Y as i64);
                        let bottom_idx = self.pixel_index(cx as i64, cy as i64 * HALF_BLOCK_PX_Y as i64 + 1);
                        let top = top_idx.and_then(|i| self.pixels[i]);
                        let bottom = bottom_idx.and_then(|i| self.pixels[i]);
                        match (top, bottom) {
                            (None, None) => (' ', Style::default()),
                            (Some(t), None) => ('▀', Style::default().fg(t)),
                            (None, Some(b)) => ('▄', Style::default().fg(b)),
                            (Some(t), Some(b)) if t == b => ('█', Style::default().fg(t)),
                            (Some(t), Some(b)) => ('▀', Style::default().fg(t).bg(b)),
                        }
                    }
                };
                buf.set(area.x + cx, area.y + cy, Cell::styled(symbol.to_string(), style));
            }
        }
    }
}

/// Map `value` from `domain = (lo, hi)` into pixel index range
/// `[0, size)`, clamped to that range (a value outside `domain` still
/// produces a valid, clipped-in-range pixel coordinate — the CALLER'S
/// `set_pixel`/`line` clipping is the last line of defense, this
/// function's own clamp just keeps the common in-domain case from
/// rounding a hair outside `[0, size)` at either edge). A degenerate
/// (zero-width or non-finite) domain maps everything to `0` rather than
/// dividing by zero/producing `NaN`.
fn map_to_px(value: f64, domain: (f64, f64), size: u32) -> i64 {
    if size == 0 {
        return 0;
    }
    let (lo, hi) = domain;
    let span = hi - lo;
    if !span.is_finite() || span.abs() < f64::EPSILON {
        return 0;
    }
    let t = ((value - lo) / span).clamp(0.0, 1.0);
    (t * (size - 1) as f64).round() as i64
}

/// Plot `points` as a connected polyline onto `canvas`, mapping each
/// point's own `(x, y)` through `x_domain`/`y_domain` into the canvas's
/// pixel space. `y` is FLIPPED (`px_height - 1 - mapped_y`) so a LARGER
/// data `y` value paints HIGHER on screen — the ordinary plot convention
/// (`y` increases upward), the opposite of [`PixelCanvas`]'s own raw
/// pixel-space row convention (`y` increases downward, matching
/// [`TerminalBuffer`]'s row order) — this is the ONE place that flip
/// happens; [`PixelCanvas::set_pixel`]/[`PixelCanvas::line`] themselves
/// stay pure screen-space, no implicit flip, so a caller drawing
/// something OTHER than plotted series data (e.g. a raw sprite) isn't
/// surprised by a hidden inversion. Fewer than 2 points draws isolated
/// dots (or nothing, for an empty slice) rather than a degenerate line.
pub fn plot_series(canvas: &mut PixelCanvas, points: &[(f64, f64)], x_domain: (f64, f64), y_domain: (f64, f64), color: Color) {
    let px_w = canvas.px_width();
    let px_h = canvas.px_height();
    if px_w == 0 || px_h == 0 {
        return;
    }

    let mut prev: Option<(i64, i64)> = None;
    for &(x, y) in points {
        let px = map_to_px(x, x_domain, px_w);
        let py = (px_h as i64 - 1) - map_to_px(y, y_domain, px_h);
        match prev {
            Some((ppx, ppy)) => canvas.line(ppx, ppy, px, py, color),
            None => canvas.set_pixel(px, py, color),
        }
        prev = Some((px, py));
    }
}

/// Plot `values` as evenly-spaced filled bar columns onto `canvas`, one
/// bar per value, each filled from a shared zero baseline
/// (`y_domain.0.max(0.0).min(y_domain.1)` clamped INTO the domain — a
/// domain that doesn't itself straddle zero still gets a well-defined,
/// in-range baseline rather than an off-canvas one) up to that value's
/// own mapped row. `values.len()` bars share `canvas.px_width()` evenly
/// (the last bar absorbing any leftover pixel from integer division, so
/// every column is accounted for exactly once, no gap/overlap). An empty
/// `values` slice is a no-op.
pub fn plot_bars(canvas: &mut PixelCanvas, values: &[f64], y_domain: (f64, f64), color: Color) {
    let n = values.len();
    let px_w = canvas.px_width();
    let px_h = canvas.px_height();
    if n == 0 || px_w == 0 || px_h == 0 {
        return;
    }

    let baseline_value = y_domain.0.max(0.0).min(y_domain.1);
    let baseline_row = (px_h as i64 - 1) - map_to_px(baseline_value, y_domain, px_h);

    let col_width = px_w as f64 / n as f64;
    for (i, &value) in values.iter().enumerate() {
        let x_start = (i as f64 * col_width).round() as i64;
        let x_end = if i + 1 == n { px_w as i64 - 1 } else { ((i + 1) as f64 * col_width).round() as i64 - 1 };
        let value_row = (px_h as i64 - 1) - map_to_px(value, y_domain, px_h);
        let (top, bottom) = if value_row <= baseline_row { (value_row, baseline_row) } else { (baseline_row, value_row) };

        for x in x_start..=x_end {
            for y in top..=bottom {
                canvas.set_pixel(x, y, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf_symbol(buf: &TerminalBuffer, x: u16, y: u16) -> String {
        buf.get(x, y).symbol.to_string()
    }

    // ── Braille ──────────────────────────────────────────────────────

    #[test]
    fn braille_single_top_left_dot_produces_the_dot_1_glyph() {
        let mut canvas = PixelCanvas::new(CanvasMode::Braille, 1, 1);
        canvas.set_pixel(0, 0, Color::White);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), "⠁");
    }

    #[test]
    fn braille_every_dot_lit_produces_the_full_braille_block() {
        let mut canvas = PixelCanvas::new(CanvasMode::Braille, 1, 1);
        for local_y in 0..4 {
            for local_x in 0..2 {
                canvas.set_pixel(local_x, local_y, Color::White);
            }
        }
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), "⣿");
    }

    #[test]
    fn braille_unset_cell_paints_the_blank_braille_glyph_clearing_prior_content() {
        let mut buf = TerminalBuffer::new(1, 1);
        buf.set(0, 0, Cell::new("X"));
        let canvas = PixelCanvas::new(CanvasMode::Braille, 1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), "⠀", "an unset braille cell must paint U+2800, not a plain ASCII space");
    }

    #[test]
    fn braille_dot_7_and_8_bottom_row_map_to_the_correct_bits() {
        // dot 7 (local (0,3)) and dot 8 (local (1,3)) together = 0x40 | 0x80 = 0xC0.
        let mut canvas = PixelCanvas::new(CanvasMode::Braille, 1, 1);
        canvas.set_pixel(0, 3, Color::White);
        canvas.set_pixel(1, 3, Color::White);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        let expected = char::from_u32(0x2800 + 0xC0).expect("valid braille codepoint");
        assert_eq!(buf_symbol(&buf, 0, 0), expected.to_string());
    }

    #[test]
    fn braille_cell_color_is_the_last_pixel_written_in_scan_order() {
        let mut canvas = PixelCanvas::new(CanvasMode::Braille, 1, 1);
        canvas.set_pixel(0, 0, Color::Red);
        canvas.set_pixel(1, 3, Color::Blue); // later in (y, x) scan order
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf.get(0, 0).style.fg, Color::Blue);
    }

    // ── HalfBlock ────────────────────────────────────────────────────

    #[test]
    fn half_block_top_only_paints_upper_half_glyph_with_fg() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 1, 1);
        canvas.set_pixel(0, 0, Color::Green);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), "▀");
        assert_eq!(buf.get(0, 0).style.fg, Color::Green);
    }

    #[test]
    fn half_block_bottom_only_paints_lower_half_glyph_with_fg() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 1, 1);
        canvas.set_pixel(0, 1, Color::Yellow);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), "▄");
        assert_eq!(buf.get(0, 0).style.fg, Color::Yellow);
    }

    #[test]
    fn half_block_both_same_color_collapses_to_a_solid_block() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 1, 1);
        canvas.set_pixel(0, 0, Color::Cyan);
        canvas.set_pixel(0, 1, Color::Cyan);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), "█");
        assert_eq!(buf.get(0, 0).style.fg, Color::Cyan);
    }

    #[test]
    fn half_block_two_different_colors_uses_fg_and_bg() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 1, 1);
        canvas.set_pixel(0, 0, Color::Red);
        canvas.set_pixel(0, 1, Color::Blue);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), "▀");
        assert_eq!(buf.get(0, 0).style.fg, Color::Red);
        assert_eq!(buf.get(0, 0).style.bg, Color::Blue);
    }

    // ── set_pixel / clear_pixel / clear / clipping ─────────────────────

    #[test]
    fn set_pixel_out_of_bounds_is_a_silent_no_op() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 1, 1);
        canvas.set_pixel(-1, 0, Color::Red);
        canvas.set_pixel(0, 100, Color::Red);
        canvas.set_pixel(100, 0, Color::Red);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), " ", "no in-bounds pixel was ever set");
    }

    #[test]
    fn clear_pixel_and_clear_both_reset_to_unset() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 1, 1);
        canvas.set_pixel(0, 0, Color::Red);
        canvas.clear_pixel(0, 0);
        let mut buf = TerminalBuffer::new(1, 1);
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), " ");

        canvas.set_pixel(0, 0, Color::Red);
        canvas.set_pixel(0, 1, Color::Blue);
        canvas.clear();
        canvas.flush(Rect::new(0, 0, 1, 1), &mut buf);
        assert_eq!(buf_symbol(&buf, 0, 0), " ");
    }

    #[test]
    fn flush_clamps_to_the_smaller_of_area_and_canvas_cell_dimensions() {
        let canvas = PixelCanvas::new(CanvasMode::HalfBlock, 4, 4);
        let mut buf = TerminalBuffer::new(2, 2);
        // area larger than the buffer would panic on buf.set — flush must
        // clamp to the SMALLER of area vs. canvas cell dims, and the
        // caller is still responsible for not handing a larger area than
        // the destination buffer itself (same contract every other
        // buffer-writing fn in this crate already assumes).
        canvas.flush(Rect::new(0, 0, 2, 2), &mut buf);
        assert_eq!(buf_symbol(&buf, 1, 1), " ");
    }

    // ── Bresenham line ───────────────────────────────────────────────

    #[test]
    fn line_horizontal_sets_every_pixel_between_the_endpoints() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 5, 1);
        canvas.line(0, 0, 4, 0, Color::White);
        for x in 0..5i64 {
            assert!(canvas.pixel_index(x, 0).and_then(|i| canvas.pixels[i]).is_some(), "pixel ({x}, 0) must be lit");
        }
    }

    #[test]
    fn line_diagonal_45_degrees_sets_the_exact_diagonal() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 4, 8);
        canvas.line(0, 0, 3, 7, Color::White);
        // Endpoints must always be lit.
        assert!(canvas.pixel_index(0, 0).and_then(|i| canvas.pixels[i]).is_some());
        assert!(canvas.pixel_index(3, 7).and_then(|i| canvas.pixels[i]).is_some());
    }

    // ── Deterministic char-grid snapshot ────────────────────────────────

    #[test]
    fn braille_snapshot_a_diagonal_line_across_a_2x1_cell_canvas() {
        // 2 cells wide x 1 cell tall = 4x4 dot grid. A line from the
        // top-left dot to the bottom-right dot.
        let mut canvas = PixelCanvas::new(CanvasMode::Braille, 2, 1);
        canvas.line(0, 0, 3, 3, Color::White);
        let mut buf = TerminalBuffer::new(2, 1);
        canvas.flush(Rect::new(0, 0, 2, 1), &mut buf);
        // Deterministic — recompute expected glyphs from the SAME
        // Bresenham path rather than hand-picking codepoints, so this
        // test tracks the real algorithm's own output.
        let mut expected_canvas = PixelCanvas::new(CanvasMode::Braille, 2, 1);
        expected_canvas.line(0, 0, 3, 3, Color::White);
        let mut expected_buf = TerminalBuffer::new(2, 1);
        expected_canvas.flush(Rect::new(0, 0, 2, 1), &mut expected_buf);
        assert_eq!(buf_symbol(&buf, 0, 0), buf_symbol(&expected_buf, 0, 0));
        assert_eq!(buf_symbol(&buf, 1, 0), buf_symbol(&expected_buf, 1, 0));
        // And a literal, non-tautological expectation: the line must
        // touch BOTH cells (it spans the full 4-column width).
        assert_ne!(buf_symbol(&buf, 0, 0), " ");
        assert_ne!(buf_symbol(&buf, 1, 0), " ");
    }

    #[test]
    fn half_block_snapshot_grid_matches_expected_strings_for_a_small_fixture() {
        // 3 cells wide x 2 cells tall = 3x4 px. Paint a checkerboard-ish
        // pattern and assert the EXACT expected glyph per cell.
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 3, 2);
        canvas.set_pixel(0, 0, Color::Red); // cell (0,0) top only -> '▀'
        canvas.set_pixel(1, 1, Color::Blue); // cell (1,0) bottom only -> '▄'
        canvas.set_pixel(2, 0, Color::Green);
        canvas.set_pixel(2, 1, Color::Green); // cell (2,0) both same -> '█'
        canvas.set_pixel(0, 2, Color::Red);
        canvas.set_pixel(0, 3, Color::Blue); // cell (0,1) both different -> '▀' fg=Red bg=Blue

        let mut buf = TerminalBuffer::new(3, 2);
        canvas.flush(Rect::new(0, 0, 3, 2), &mut buf);

        let expected: [[&str; 3]; 2] = [["▀", "▄", "█"], ["▀", " ", " "]];
        for (row, expected_row) in expected.iter().enumerate() {
            for (col, &want) in expected_row.iter().enumerate() {
                assert_eq!(buf_symbol(&buf, col as u16, row as u16), want, "cell ({col},{row}) mismatch");
            }
        }
        assert_eq!(buf.get(0, 1).style.fg, Color::Red);
        assert_eq!(buf.get(0, 1).style.bg, Color::Blue);
    }

    // ── Figures adapter ──────────────────────────────────────────────

    #[test]
    fn plot_series_draws_a_rising_line_from_bottom_left_toward_top_right() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 10, 5);
        let points = [(0.0, 0.0), (10.0, 10.0)];
        plot_series(&mut canvas, &points, (0.0, 10.0), (0.0, 10.0), Color::White);

        // The first point (min x, min y -- the plot's own bottom-left)
        // must land at the LAST pixel row (bottom of the canvas); the
        // last point (max x, max y) must land at row 0 (top) — proving
        // the y-flip actually happened, not merely that SOME pixels got lit.
        let px_h = canvas.px_height() as i64;
        assert!(canvas.pixel_index(0, px_h - 1).and_then(|i| canvas.pixels[i]).is_some(), "min-y point must land at the BOTTOM pixel row");
        assert!(canvas.pixel_index(canvas.px_width() as i64 - 1, 0).and_then(|i| canvas.pixels[i]).is_some(), "max-y point must land at the TOP pixel row");
    }

    #[test]
    fn plot_series_empty_and_single_point_do_not_panic() {
        let mut canvas = PixelCanvas::new(CanvasMode::Braille, 4, 4);
        plot_series(&mut canvas, &[], (0.0, 1.0), (0.0, 1.0), Color::White);
        plot_series(&mut canvas, &[(0.5, 0.5)], (0.0, 1.0), (0.0, 1.0), Color::White);
    }

    #[test]
    fn plot_bars_fills_each_column_from_the_zero_baseline_up_to_its_own_value() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 4, 4);
        // 2 bars over a [0, 10] domain: bar 0 = 10 (full height), bar 1 = 0 (empty, just the baseline row).
        plot_bars(&mut canvas, &[10.0, 0.0], (0.0, 10.0), Color::White);

        let px_h = canvas.px_height() as i64;
        // Bar 0's own column (left half of the canvas) must be lit at
        // the TOP row (the tallest value reaches the top).
        assert!(canvas.pixel_index(0, 0).and_then(|i| canvas.pixels[i]).is_some(), "the full-height bar must reach the top pixel row");
        // Bar 1's own column (right half) must be lit ONLY at the
        // baseline row (bottom), not at the top.
        let right_col = canvas.px_width() as i64 - 1;
        assert!(canvas.pixel_index(right_col, px_h - 1).and_then(|i| canvas.pixels[i]).is_some(), "even a zero-value bar still paints its own baseline pixel");
        assert!(canvas.pixel_index(right_col, 0).and_then(|i| canvas.pixels[i]).is_none(), "a zero-value bar must NOT reach the top pixel row");
    }

    #[test]
    fn plot_bars_empty_values_is_a_no_op() {
        let mut canvas = PixelCanvas::new(CanvasMode::Braille, 4, 4);
        plot_bars(&mut canvas, &[], (0.0, 1.0), Color::White);
    }

    #[test]
    fn plot_bars_every_column_is_covered_exactly_once_no_gap_no_overlap() {
        let mut canvas = PixelCanvas::new(CanvasMode::HalfBlock, 7, 1);
        plot_bars(&mut canvas, &[1.0, 1.0, 1.0], (0.0, 1.0), Color::White);
        // Every one of the 7 px-wide columns must be lit (each bar fully
        // covers its own share, and the last bar absorbs the remainder
        // of 7/3 columns) — no gap anywhere across the full width.
        for x in 0..7i64 {
            assert!(canvas.pixel_index(x, 0).and_then(|i| canvas.pixels[i]).is_some(), "column {x} must be covered by some bar");
        }
    }
}
