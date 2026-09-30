//! Layout presets for panel arrangements.
//!
//! Provides standard split/grid patterns for common use cases.

use super::PanelRect;

/// Default gap between panels in multi-panel layouts
pub const PANEL_GAP: f32 = 0.0;

/// How a separator drag reacts when it would push a neighbour below its
/// minimum size. Set per [`DockState`](crate::layout::DockState) with
/// `set_splitter_policy`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum SplitterPolicy {
    /// Today's behaviour (the default).
    ///
    /// Closed presets are untouched: single-axis splits cascade the delta
    /// through siblings down to their pixel minimums, and the two-axis
    /// presets move `cross_ratio` within `0.05..=0.95`. For a `rows × cols`
    /// grid line a move that would take either adjacent row / column below
    /// its minimum is rejected whole (ratios unchanged, `drag_separator`
    /// returns `false`) and a [`SnapBackAnimation`](super::SnapBackAnimation)
    /// is queued for the separator with the overshoot.
    #[default]
    RejectSnapBack,
    /// Stop at the limit: only the two children / tracks next to the
    /// separator change, and the one shrinking stops at its minimum —
    /// `min_frac` of the branch along the drag axis, or its pixel minimum
    /// if that is larger. Never rejects, never snaps back. For the two-axis
    /// presets (`Grid2x2`, L-shapes) the `cross_ratio` is kept within
    /// `min_frac..=1 - min_frac` (and never outside `0.05..=0.95`).
    Clamp {
        /// Smallest fraction of the branch a child may shrink to; values
        /// outside `0.0..0.5` are clamped (NaN counts as 0).
        min_frac: f64,
    },
}

/// How to split a container into sub-slots.
///
/// Naming reflects WHERE the new sibling appears:
/// `SplitRight` puts the new panel on the right of the original;
/// `SplitBottom` puts it below.  Old `Horizontal`/`Vertical` names are
/// kept as deprecated aliases for backwards compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplitKind {
    /// New panel appears on the RIGHT of the original (Left | Right, 2 slots).
    SplitRight,
    /// New panel appears BELOW the original (Top / Bottom, 2 slots).
    SplitBottom,
    /// 2×2 grid (4 slots).
    Grid2x2,
    /// 1 left + 2 stacked right (3 slots).
    OneLeftTwoRight,
    /// 2 stacked left + 1 right (3 slots).
    TwoLeftOneRight,
    /// 1 top + 2 side-by-side bottom (3 slots).
    OneTopTwoBottom,
    /// 2 side-by-side top + 1 bottom (3 slots).
    TwoTopOneBottom,
    /// 3 vertical columns (3 slots).
    ThreeColumns,
    /// 3 horizontal rows (3 slots).
    ThreeRows,
    /// 1 big + 3 small (4 slots).
    OneBig3Small,
}

impl SplitKind {
    /// Deprecated alias for [`SplitKind::SplitRight`].
    #[deprecated(note = "use SplitKind::SplitRight — clearer about new panel placement")]
    #[allow(non_upper_case_globals)]
    pub const Horizontal: SplitKind = SplitKind::SplitRight;
    /// Deprecated alias for [`SplitKind::SplitBottom`].
    #[deprecated(note = "use SplitKind::SplitBottom — clearer about new panel placement")]
    #[allow(non_upper_case_globals)]
    pub const Vertical: SplitKind = SplitKind::SplitBottom;
}

/// Preset layout patterns for panel arrangements
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum WindowLayout {
    /// Single panel (tabbed)
    #[default]
    Single,
    /// Two panels side by side
    SplitHorizontal,
    /// Two panels stacked
    SplitVertical,
    /// Four panels in grid
    Grid2x2,
    /// 2 stacked on left, 1 big on right
    TwoLeftOneRight,
    /// 1 big on left, 2 stacked on right
    OneLeftTwoRight,
    /// 2 side by side on top, 1 big on bottom
    TwoTopOneBottom,
    /// 1 big on top, 2 side by side on bottom
    OneTopTwoBottom,
    /// 3 vertical columns
    ThreeColumns,
    /// 3 horizontal rows
    ThreeRows,
    /// Custom layout (computed from child count)
    Custom,
}

impl WindowLayout {
    /// Get required number of panels for this layout
    pub fn panel_count(&self) -> usize {
        match self {
            WindowLayout::Single => 1,
            WindowLayout::SplitHorizontal | WindowLayout::SplitVertical => 2,
            WindowLayout::Grid2x2 => 4,
            WindowLayout::TwoLeftOneRight | WindowLayout::OneLeftTwoRight |
            WindowLayout::TwoTopOneBottom | WindowLayout::OneTopTwoBottom |
            WindowLayout::ThreeColumns | WindowLayout::ThreeRows => 3,
            WindowLayout::Custom => usize::MAX, // dynamic
        }
    }

    /// Calculate panel rectangles for this layout with default gap
    pub fn calculate_rects(&self, total_width: f32, total_height: f32, panel_count: usize) -> Vec<PanelRect> {
        self.calculate_rects_with_gap(total_width, total_height, panel_count, PANEL_GAP)
    }

    /// Calculate panel rectangles with custom gap between panels
    pub fn calculate_rects_with_gap(&self, total_width: f32, total_height: f32, panel_count: usize, gap: f32) -> Vec<PanelRect> {
        let mut rects = Vec::new();

        match self {
            WindowLayout::Single => {
                rects.push(PanelRect::new(0.0, 0.0, total_width, total_height));
            }
            WindowLayout::SplitHorizontal => {
                let half = (total_width - gap) / 2.0;
                rects.push(PanelRect::new(0.0, 0.0, half, total_height));
                if panel_count > 1 {
                    rects.push(PanelRect::new(half + gap, 0.0, half, total_height));
                }
            }
            WindowLayout::SplitVertical => {
                let half = (total_height - gap) / 2.0;
                rects.push(PanelRect::new(0.0, 0.0, total_width, half));
                if panel_count > 1 {
                    rects.push(PanelRect::new(0.0, half + gap, total_width, half));
                }
            }
            WindowLayout::Grid2x2 => {
                let half_w = (total_width - gap) / 2.0;
                let half_h = (total_height - gap) / 2.0;
                rects.push(PanelRect::new(0.0, 0.0, half_w, half_h));
                if panel_count > 1 { rects.push(PanelRect::new(half_w + gap, 0.0, half_w, half_h)); }
                if panel_count > 2 { rects.push(PanelRect::new(0.0, half_h + gap, half_w, half_h)); }
                if panel_count > 3 { rects.push(PanelRect::new(half_w + gap, half_h + gap, half_w, half_h)); }
            }
            WindowLayout::TwoLeftOneRight => {
                let left_w = (total_width - gap) * 0.4;
                let right_w = (total_width - gap) * 0.6;
                let half_h = (total_height - gap) / 2.0;
                rects.push(PanelRect::new(0.0, 0.0, left_w, half_h));
                if panel_count > 1 { rects.push(PanelRect::new(0.0, half_h + gap, left_w, half_h)); }
                if panel_count > 2 { rects.push(PanelRect::new(left_w + gap, 0.0, right_w, total_height)); }
            }
            WindowLayout::OneLeftTwoRight => {
                let left_w = (total_width - gap) * 0.6;
                let right_w = (total_width - gap) * 0.4;
                let half_h = (total_height - gap) / 2.0;
                rects.push(PanelRect::new(0.0, 0.0, left_w, total_height));
                if panel_count > 1 { rects.push(PanelRect::new(left_w + gap, 0.0, right_w, half_h)); }
                if panel_count > 2 { rects.push(PanelRect::new(left_w + gap, half_h + gap, right_w, half_h)); }
            }
            WindowLayout::TwoTopOneBottom => {
                let top_h = (total_height - gap) * 0.4;
                let bottom_h = (total_height - gap) * 0.6;
                let half_w = (total_width - gap) / 2.0;
                rects.push(PanelRect::new(0.0, 0.0, half_w, top_h));
                if panel_count > 1 { rects.push(PanelRect::new(half_w + gap, 0.0, half_w, top_h)); }
                if panel_count > 2 { rects.push(PanelRect::new(0.0, top_h + gap, total_width, bottom_h)); }
            }
            WindowLayout::OneTopTwoBottom => {
                let top_h = (total_height - gap) * 0.6;
                let bottom_h = (total_height - gap) * 0.4;
                let half_w = (total_width - gap) / 2.0;
                rects.push(PanelRect::new(0.0, 0.0, total_width, top_h));
                if panel_count > 1 { rects.push(PanelRect::new(0.0, top_h + gap, half_w, bottom_h)); }
                if panel_count > 2 { rects.push(PanelRect::new(half_w + gap, top_h + gap, half_w, bottom_h)); }
            }
            WindowLayout::ThreeColumns => {
                let col_w = (total_width - gap * 2.0) / 3.0;
                rects.push(PanelRect::new(0.0, 0.0, col_w, total_height));
                if panel_count > 1 { rects.push(PanelRect::new(col_w + gap, 0.0, col_w, total_height)); }
                if panel_count > 2 { rects.push(PanelRect::new(col_w * 2.0 + gap * 2.0, 0.0, col_w, total_height)); }
            }
            WindowLayout::ThreeRows => {
                let row_h = (total_height - gap * 2.0) / 3.0;
                rects.push(PanelRect::new(0.0, 0.0, total_width, row_h));
                if panel_count > 1 { rects.push(PanelRect::new(0.0, row_h + gap, total_width, row_h)); }
                if panel_count > 2 { rects.push(PanelRect::new(0.0, row_h * 2.0 + gap * 2.0, total_width, row_h)); }
            }
            WindowLayout::Custom => {
                // Dynamic NxM grid for any number of panels
                if panel_count == 0 {
                    return rects;
                }
                let cols = (panel_count as f32).sqrt().ceil() as usize;
                let rows = panel_count.div_ceil(cols); // ceil division
                let cell_w = (total_width - gap * (cols as f32 - 1.0).max(0.0)) / cols as f32;
                let cell_h = (total_height - gap * (rows as f32 - 1.0).max(0.0)) / rows as f32;

                // Guard against negative or zero dimensions
                if cell_w <= 0.0 || cell_h <= 0.0 {
                    // Fallback: single rect for all
                    rects.push(PanelRect::new(0.0, 0.0, total_width, total_height));
                    return rects;
                }

                for i in 0..panel_count {
                    let col = i % cols;
                    let row = i / cols;
                    let x = col as f32 * (cell_w + gap);
                    let y = row as f32 * (cell_h + gap);
                    rects.push(PanelRect::new(x, y, cell_w, cell_h));
                }
            }
        }

        rects
    }
}

/// A `rows × cols` grid shape with independently adjustable row and column
/// ratios, carried by a [`Branch`](super::Branch) in its `grid` field.
///
/// Children of a grid branch are laid out in row-major order: child
/// `r * cols + c` occupies row `r`, column `c`. `row_ratios` (len = `rows`)
/// sizes the rows top to bottom, `col_ratios` (len = `cols`) sizes the
/// columns left to right. Each block is normalised on read, so only the
/// relative weights inside a block matter, and changing the rows never
/// touches the columns (and vice versa).
///
/// A grid branch keeps `layout == WindowLayout::Custom`, so code that only
/// knows the closed presets still sees a valid layout value.
#[derive(Clone, Debug, PartialEq)]
pub struct GridSpec {
    /// Number of rows (≥ 1).
    pub rows: usize,
    /// Number of columns (≥ 1).
    pub cols: usize,
    /// Row weights, top to bottom. Length `rows`.
    pub row_ratios: Vec<f64>,
    /// Column weights, left to right. Length `cols`.
    pub col_ratios: Vec<f64>,
}

impl GridSpec {
    /// Equal rows and equal columns. `None` if `rows` or `cols` is zero.
    pub fn new(rows: usize, cols: usize) -> Option<Self> {
        if rows == 0 || cols == 0 {
            return None;
        }
        Some(Self {
            rows,
            cols,
            row_ratios: vec![1.0 / rows as f64; rows],
            col_ratios: vec![1.0 / cols as f64; cols],
        })
    }

    /// Grid with explicit weights (the shape is taken from the lengths).
    /// `None` unless both blocks are non-empty and every weight is finite
    /// and positive.
    pub fn with_ratios(row_ratios: Vec<f64>, col_ratios: Vec<f64>) -> Option<Self> {
        if !Self::valid_block(&row_ratios) || !Self::valid_block(&col_ratios) {
            return None;
        }
        Some(Self {
            rows: row_ratios.len(),
            cols: col_ratios.len(),
            row_ratios,
            col_ratios,
        })
    }

    /// `true` when `ratios` is non-empty and every entry is finite and > 0.
    pub(crate) fn valid_block(ratios: &[f64]) -> bool {
        !ratios.is_empty() && ratios.iter().all(|r| r.is_finite() && *r > 0.0)
    }

    /// Number of cells (`rows * cols`) — the child count the grid lays out.
    pub fn cell_count(&self) -> usize {
        self.rows * self.cols
    }

    /// Row-major child index of cell (`row`, `col`).
    pub fn cell_index(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }

    /// Row weights normalised to sum 1 (equal rows if the stored block is
    /// the wrong length or invalid).
    pub fn normalized_rows(&self) -> Vec<f64> {
        Self::normalize(&self.row_ratios, self.rows)
    }

    /// Column weights normalised to sum 1 (equal columns if the stored block
    /// is the wrong length or invalid).
    pub fn normalized_cols(&self) -> Vec<f64> {
        Self::normalize(&self.col_ratios, self.cols)
    }

    fn normalize(ratios: &[f64], n: usize) -> Vec<f64> {
        if n == 0 {
            return Vec::new();
        }
        if ratios.len() != n || !Self::valid_block(ratios) {
            return vec![1.0 / n as f64; n];
        }
        let sum: f64 = ratios.iter().sum();
        ratios.iter().map(|r| r / sum).collect()
    }

    /// Cell rects for the grid laid out in `area`, row-major, with `gap`
    /// pixels between adjacent rows and columns.
    pub fn cell_rects(&self, area: PanelRect, gap: f32) -> Vec<PanelRect> {
        let (xs, ws) = Self::tracks(&self.normalized_cols(), area.x, area.width, gap);
        let (ys, hs) = Self::tracks(&self.normalized_rows(), area.y, area.height, gap);
        let mut out = Vec::with_capacity(self.cell_count());
        for r in 0..self.rows {
            for c in 0..self.cols {
                out.push(PanelRect::new(xs[c], ys[r], ws[c], hs[r]));
            }
        }
        out
    }

    /// Start offsets and extents of the tracks along one axis. Track edges
    /// come from the cumulative fraction, so the last track ends exactly at
    /// the far edge.
    pub(crate) fn tracks(fracs: &[f64], start: f32, extent: f32, gap: f32) -> (Vec<f32>, Vec<f32>) {
        let n = fracs.len();
        let available = (extent - gap * n.saturating_sub(1) as f32).max(0.0) as f64;
        let mut starts = Vec::with_capacity(n);
        let mut sizes = Vec::with_capacity(n);
        let mut acc = 0.0_f64;
        for (i, f) in fracs.iter().enumerate() {
            let a = start + (acc * available) as f32 + gap * i as f32;
            acc += f;
            let b = if i + 1 == n {
                start + extent
            } else {
                start + (acc * available) as f32 + gap * i as f32
            };
            starts.push(a);
            sizes.push((b - a).max(0.0));
        }
        (starts, sizes)
    }
}
