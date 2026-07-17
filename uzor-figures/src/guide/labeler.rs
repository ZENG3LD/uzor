//! Bitmap-occupancy label placement — port of the "Legible Label Layout"
//! algorithm (arXiv 2405.10953, integrated as Vega-Lite's `label`
//! encoding channel; see
//! `nemo/docs/uzor-engines/research_dataviz_sota_2026.md` §2). A coarse
//! occupancy grid over a plot rect tracks which screen-space cells are
//! already claimed (by marks, or by previously-placed labels); each
//! label tries an ordered list of candidate rects and claims the FIRST
//! one entirely free — same greedy, first-fit, no-backtracking
//! philosophy [`crate::guide::axis`]'s 1D skip-only tick-label collision
//! already uses (design law: compare against what's ACTUALLY placed, not
//! re-shuffle earlier placements), generalized here to 2D + arbitrary
//! mark footprints instead of just "the previous tick label."
//!
//! **Bitmap params (this port's own choice, not the paper's raw
//! numbers)**: the paper's own bitmap is one bit per screen PIXEL, sized
//! for a JS 32-bit int canvas (`((y*w+x) mod 32)` bit index). This port
//! uses a coarser [`DEFAULT_CELL_PX`]-px cell grid instead, packed into
//! `u64` words (64 cells/word) — cheap enough at this crate's figure
//! sizes (a few hundred words total) while keeping the paper's core
//! property: an occupancy check/mark costs `O(cells the rect covers)`,
//! never `O(marks already placed)` or `O(labels already placed)`. The
//! paper's own "only mark the first row, last row, and every
//! `labelHeight_min`-th row" thinning optimization is deliberately NOT
//! ported — the coarser cell grid already keeps a full rect-mark cheap
//! enough that the extra bookkeeping isn't worth it at this crate's
//! scale (a handful of labels per figure, not thousands).

use uzor::types::Rect;

/// Cell size (px) for [`OccupancyBitmap::new`]'s default grid resolution
/// — coarse enough to keep the grid small, fine enough that a
/// few-pixel label overlap still gets caught.
pub const DEFAULT_CELL_PX: f64 = 4.0;

/// A coarse occupancy grid over one screen-space rect, packed into `u64`
/// words (64 cells/word). [`OccupancyBitmap::mark`] rasterizes a rect's
/// footprint as occupied; [`OccupancyBitmap::is_free`] checks whether a
/// candidate rect's ENTIRE footprint is still clear — both cost
/// `O(cells the rect covers)`, independent of how many other
/// marks/labels already exist in the grid.
pub struct OccupancyBitmap {
    origin: (f64, f64),
    cell_px: f64,
    cols: usize,
    rows: usize,
    words_per_row: usize,
    words: Vec<u64>,
}

impl OccupancyBitmap {
    /// A new, entirely-unoccupied grid covering `rect` at `cell_px`
    /// resolution (floored at `1.0` — a zero/negative cell size would
    /// divide-by-zero below).
    pub fn new(rect: Rect, cell_px: f64) -> Self {
        let cell_px = cell_px.max(1.0);
        let cols = ((rect.width / cell_px).ceil() as usize).max(1);
        let rows = ((rect.height / cell_px).ceil() as usize).max(1);
        let words_per_row = cols.div_ceil(64);
        Self { origin: (rect.x, rect.y), cell_px, cols, rows, words_per_row, words: vec![0u64; words_per_row * rows] }
    }

    /// `true` when `rect` is ENTIRELY inside this grid's own trackable
    /// bounds (never partially) — a candidate that clips even one edge
    /// past the grid is never considered "free" by [`Self::is_free`]:
    /// the bitmap can only vouch for what it tracks, and a candidate
    /// that partly falls outside a figure's own plot rect is never a
    /// legitimate placement regardless of what the trackable portion
    /// looks like.
    fn fully_within_bounds(&self, rect: Rect) -> bool {
        let grid_w = self.cols as f64 * self.cell_px;
        let grid_h = self.rows as f64 * self.cell_px;
        let local_x0 = rect.x - self.origin.0;
        let local_y0 = rect.y - self.origin.1;
        rect.width > 0.0
            && rect.height > 0.0
            && local_x0 >= 0.0
            && local_y0 >= 0.0
            && local_x0 + rect.width <= grid_w
            && local_y0 + rect.height <= grid_h
    }

    /// `rect`'s footprint as an inclusive-exclusive cell range `(col0,
    /// col1, row0, row1)`, clamped to this grid's own bounds. `None` when
    /// `rect` falls entirely outside the grid — used only by
    /// [`Self::mark`] (a best-effort clamp-and-mark; harmless since an
    /// out-of-bounds rect can never be queried "free" again anyway, see
    /// [`Self::fully_within_bounds`]).
    fn cell_range(&self, rect: Rect) -> Option<(usize, usize, usize, usize)> {
        let grid_w = self.cols as f64 * self.cell_px;
        let grid_h = self.rows as f64 * self.cell_px;
        let local_x0 = rect.x - self.origin.0;
        let local_y0 = rect.y - self.origin.1;
        let local_x1 = local_x0 + rect.width;
        let local_y1 = local_y0 + rect.height;
        if local_x1 <= 0.0 || local_y1 <= 0.0 || local_x0 >= grid_w || local_y0 >= grid_h || rect.width <= 0.0 || rect.height <= 0.0 {
            return None;
        }
        let c0 = (local_x0.max(0.0) / self.cell_px).floor() as usize;
        let c1 = (((local_x1.min(grid_w)) / self.cell_px).ceil() as usize).max(c0 + 1).min(self.cols);
        let r0 = (local_y0.max(0.0) / self.cell_px).floor() as usize;
        let r1 = (((local_y1.min(grid_h)) / self.cell_px).ceil() as usize).max(r0 + 1).min(self.rows);
        Some((c0, c1, r0, r1))
    }

    /// `true` when `rect` is fully within bounds AND every cell in its
    /// own footprint is currently unoccupied.
    pub fn is_free(&self, rect: Rect) -> bool {
        if !self.fully_within_bounds(rect) {
            return false;
        }
        let Some((c0, c1, r0, r1)) = self.cell_range(rect) else { return false };
        for row in r0..r1 {
            let base = row * self.words_per_row;
            let mut col = c0;
            while col < c1 {
                let word_idx = col / 64;
                let bit_start = col % 64;
                let bit_end = (c1 - word_idx * 64).min(64);
                let mask = word_mask(bit_start, bit_end);
                if self.words[base + word_idx] & mask != 0 {
                    return false;
                }
                col = word_idx * 64 + bit_end;
            }
        }
        true
    }

    /// Mark `rect`'s own footprint as occupied.
    pub fn mark(&mut self, rect: Rect) {
        let Some((c0, c1, r0, r1)) = self.cell_range(rect) else { return };
        for row in r0..r1 {
            let base = row * self.words_per_row;
            let mut col = c0;
            while col < c1 {
                let word_idx = col / 64;
                let bit_start = col % 64;
                let bit_end = (c1 - word_idx * 64).min(64);
                let mask = word_mask(bit_start, bit_end);
                self.words[base + word_idx] |= mask;
                col = word_idx * 64 + bit_end;
            }
        }
    }
}

/// A `[bit_start, bit_end)` mask within one 64-bit word (both the
/// "fully covered word" and "partially covered word" cases from the
/// paper's own §2 description collapse into this one formula: a fully
/// covered word is just `bit_start=0, bit_end=64`).
fn word_mask(bit_start: usize, bit_end: usize) -> u64 {
    if bit_start >= bit_end {
        return 0;
    }
    if bit_end - bit_start >= 64 {
        return u64::MAX;
    }
    ((1u64 << (bit_end - bit_start)) - 1) << bit_start
}

/// Greedy first-fit label placement over a shared [`OccupancyBitmap`]:
/// for each label index `0..rects.len()`, try `candidates_fn(index,
/// natural_rect)`'s own ordered candidate rects in turn and claim the
/// FIRST one entirely free (marking it occupied before moving to the
/// next label) — the same greedy, no-backtracking philosophy every
/// other collision pass in this crate uses
/// ([`crate::guide::axis`]'s skip-only greedy,
/// [`crate::figure::timeline::layout_point_labels`]), generalized from
/// "1 candidate, skip on collision" to "N candidates, skip only once
/// every candidate is exhausted."
///
/// `rects[i]` is each label's own natural/default rect — handed to
/// `candidates_fn` so it can derive alternate positions from it (e.g.
/// [`anchor_candidates`]) without the caller needing a second parallel
/// array.
///
/// Returns one `Option<Rect>` per label: `Some(rect)` is the first free
/// candidate claimed; `None` means every candidate collided and this
/// label should fall back to being skipped — never overlap, matching
/// this crate's existing axis/timeline greedy-skip convention.
pub fn place_labels(occupancy: &mut OccupancyBitmap, rects: &[Rect], candidates_fn: impl Fn(usize, Rect) -> Vec<Rect>) -> Vec<Option<Rect>> {
    rects
        .iter()
        .enumerate()
        .map(|(i, &natural)| {
            for candidate in candidates_fn(i, natural) {
                if occupancy.is_free(candidate) {
                    occupancy.mark(candidate);
                    return Some(candidate);
                }
            }
            None
        })
        .collect()
}

/// Classic 8-position candidate rect list for a label anchored around
/// `anchor`, sized `label_size = (width, height)`, cleared `gap` px past
/// a marker footprint of `radius` px in every direction — this crate's
/// port of the paper's "4 corners + 4 edge-midpoints" candidate model,
/// reordered edges-first (right, left, above, below, THEN the four
/// diagonal corners) to match this crate's own existing convention of
/// preferring the plain reading-order default and treating diagonals as
/// the fallback (mirrors [`crate::figure::timeline`]'s existing
/// right/left mirror-on-clip behavior, now given two more axes to try
/// before falling back to a skip).
pub fn anchor_candidates(anchor: (f64, f64), label_size: (f64, f64), radius: f64, gap: f64) -> [Rect; 8] {
    let (ax, ay) = anchor;
    let (w, h) = label_size;
    let off = radius + gap;
    [
        Rect::new(ax + off, ay - h / 2.0, w, h),     // right
        Rect::new(ax - off - w, ay - h / 2.0, w, h), // left
        Rect::new(ax - w / 2.0, ay - off - h, w, h), // above
        Rect::new(ax - w / 2.0, ay + off, w, h),     // below
        Rect::new(ax + off, ay - off - h, w, h),     // top-right
        Rect::new(ax + off, ay + off, w, h),         // bottom-right
        Rect::new(ax - off - w, ay - off - h, w, h), // top-left
        Rect::new(ax - off - w, ay + off, w, h),     // bottom-left
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plot_rect() -> Rect {
        Rect::new(0.0, 0.0, 200.0, 200.0)
    }

    #[test]
    fn fresh_bitmap_is_free_everywhere() {
        let occ = OccupancyBitmap::new(plot_rect(), DEFAULT_CELL_PX);
        assert!(occ.is_free(Rect::new(10.0, 10.0, 20.0, 20.0)));
        // Flush against the bottom-right edge but still FULLY inside the
        // 200x200 grid (right/bottom edge exactly at the bound, not past
        // it).
        assert!(occ.is_free(Rect::new(180.0, 180.0, 20.0, 20.0)));
    }

    #[test]
    fn mark_then_is_free_reports_occupied_and_word_boundary_is_handled() {
        // 200px / 4px cell = 50 cols, well past one 64-bit word boundary
        // is not exercised here, so also check a WIDE rect crossing a
        // word boundary directly with a wider grid.
        let mut occ = OccupancyBitmap::new(Rect::new(0.0, 0.0, 2000.0, 100.0), 4.0);
        let r = Rect::new(500.0, 0.0, 40.0, 20.0); // spans a 64-cell word boundary (col 125 = word 1, bit 61)
        assert!(occ.is_free(r));
        occ.mark(r);
        assert!(!occ.is_free(r));
        // A rect just past the marked one, in the SAME row, must still
        // be free — proves the mark didn't spuriously bleed into
        // neighboring words.
        assert!(occ.is_free(Rect::new(600.0, 0.0, 20.0, 20.0)));
        // A rect overlapping only the tail end of the marked rect must
        // also report occupied (partial-word overlap, not just exact
        // match).
        assert!(!occ.is_free(Rect::new(530.0, 0.0, 20.0, 20.0)));
    }

    #[test]
    fn out_of_bounds_rect_is_never_free_and_marking_it_is_a_safe_no_op() {
        let mut occ = OccupancyBitmap::new(plot_rect(), DEFAULT_CELL_PX);
        // Fully outside the grid.
        let outside = Rect::new(-100.0, -100.0, 10.0, 10.0);
        assert!(!occ.is_free(outside), "a candidate entirely outside the trackable grid must never be reported free");
        occ.mark(outside); // must not panic
        assert!(occ.is_free(Rect::new(0.0, 0.0, 5.0, 5.0)));

        // PARTIALLY outside the grid (clips the left edge by a few px) —
        // must also be rejected, not silently checked against only its
        // on-grid portion (the real bug this test guards: a label
        // candidate that half-clips the plot's own edge must never win
        // just because its clipped, checkable portion happens to be
        // clear).
        let clipping = Rect::new(-5.0, 10.0, 20.0, 10.0);
        assert!(!occ.is_free(clipping), "a candidate clipping the grid's own edge must never be reported free");
    }

    #[test]
    fn two_overlapping_at_default_labels_get_separated_to_different_candidates() {
        let mut occ = OccupancyBitmap::new(plot_rect(), DEFAULT_CELL_PX);
        // Two labels with the IDENTICAL natural rect (would collide if
        // both placed there) — each label's candidates_fn offers its own
        // natural position first, then shifted alternates.
        let natural = Rect::new(50.0, 50.0, 30.0, 12.0);
        let rects = [natural, natural];
        let candidates_fn = |_i: usize, r: Rect| vec![r, Rect::new(r.x + 40.0, r.y, r.width, r.height), Rect::new(r.x, r.y + 20.0, r.width, r.height)];

        let placed = place_labels(&mut occ, &rects, candidates_fn);
        let first = placed[0].expect("first label must place at its own natural rect (grid starts empty)");
        let second = placed[1].expect("second label must find a free alternate candidate");
        assert_eq!(first, natural, "first label wins the contested natural position");
        assert_ne!(second, natural, "second label must NOT land on the same rect as the first");
        // The two claimed rects must not overlap on the grid.
        assert!(!rects_overlap(first, second));
    }

    #[test]
    fn full_region_saturation_degrades_to_skip_never_overlap() {
        let region = Rect::new(0.0, 0.0, 40.0, 40.0); // small — easy to fully saturate
        let mut occ = OccupancyBitmap::new(region, DEFAULT_CELL_PX);
        // Pre-saturate the ENTIRE region with one big mark.
        occ.mark(region);

        let rects = [Rect::new(5.0, 5.0, 10.0, 10.0); 3];
        // Every candidate this closure offers falls inside the already-
        // saturated region — none should ever be placeable.
        let candidates_fn = |_i: usize, r: Rect| vec![r, Rect::new(r.x + 5.0, r.y, r.width, r.height), Rect::new(r.x, r.y + 5.0, r.width, r.height)];
        let placed = place_labels(&mut occ, &rects, candidates_fn);
        assert!(placed.iter().all(Option::is_none), "every label must degrade to skip on a fully saturated region: {placed:?}");
    }

    #[test]
    fn place_labels_is_deterministic_across_repeated_runs() {
        let make_occ = || OccupancyBitmap::new(plot_rect(), DEFAULT_CELL_PX);
        let rects = [Rect::new(10.0, 10.0, 20.0, 10.0), Rect::new(15.0, 10.0, 20.0, 10.0), Rect::new(20.0, 10.0, 20.0, 10.0)];
        let candidates_fn = |_i: usize, r: Rect| {
            anchor_candidates((r.x, r.y), (r.width, r.height), 0.0, 2.0).to_vec()
        };

        let mut occ_a = make_occ();
        let placed_a = place_labels(&mut occ_a, &rects, candidates_fn);
        let mut occ_b = make_occ();
        let placed_b = place_labels(&mut occ_b, &rects, candidates_fn);
        assert_eq!(placed_a, placed_b);
    }

    #[test]
    fn anchor_candidates_are_ordered_right_left_above_below_then_diagonals() {
        let candidates = anchor_candidates((100.0, 100.0), (20.0, 10.0), 5.0, 2.0);
        // right: fully to the right of the anchor x.
        assert!(candidates[0].x > 100.0);
        // left: fully to the left of the anchor x (its right edge <= anchor x - radius - gap).
        assert!(candidates[1].right() <= 100.0 - 5.0);
        // above: vertically centered on x, above anchor y.
        assert!(candidates[2].bottom() <= 100.0 - 5.0);
        // below: vertically centered on x, below anchor y.
        assert!(candidates[3].y >= 100.0 + 5.0);
    }

    fn rects_overlap(a: Rect, b: Rect) -> bool {
        a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
    }
}
