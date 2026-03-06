//! Layout presets for panel arrangements.
//!
//! Provides standard split/grid patterns for common use cases.

use crate::PanelRect;

/// Default gap between panels in multi-panel layouts
pub const PANEL_GAP: f32 = 4.0;

/// How to split a container into sub-slots
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplitKind {
    /// Left | Right (2 slots)
    Horizontal,
    /// Top / Bottom (2 slots)
    Vertical,
    /// 2x2 grid (4 slots)
    Grid2x2,
    /// 1 left + 2 stacked right (3 slots)
    OneLeftTwoRight,
    /// 2 stacked left + 1 right (3 slots)
    TwoLeftOneRight,
    /// 1 top + 2 side-by-side bottom (3 slots)
    OneTopTwoBottom,
    /// 2 side-by-side top + 1 bottom (3 slots)
    TwoTopOneBottom,
    /// 3 vertical columns (3 slots)
    ThreeColumns,
    /// 3 horizontal rows (3 slots)
    ThreeRows,
    /// 1 big + 3 small (4 slots)
    OneBig3Small,
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
                let rows = (panel_count + cols - 1) / cols; // ceil division
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
