//! 1-D flex solve for the `view!` macro (moved from `the old framework module::layout`).
//!
//! Pure: a parent [`Rect`] plus child specs, nothing else. Free space on the
//! main axis (after padding and gaps) is given to children in proportion to
//! their `flex` weights. A negative free space does not shrink `basis`
//! (children may overflow; that is the caller's layout).

use uzor::Rect;

/// Main axis of a flex solve.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FlexDir {
    /// Children run left to right.
    Row,
    /// Children run top to bottom.
    Col,
}

/// One child of a flex solve.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FlexChild {
    /// Preferred size along the main axis, in logical pixels. `0.0` is
    /// purely flex-driven.
    pub basis: f64,
    /// Grow weight. `0.0` stays at [`FlexChild::basis`].
    pub flex: f64,
}

/// Solve a 1-D flex layout. One [`Rect`] per child, declaration order.
pub fn flex_solve(
    parent: Rect,
    dir: FlexDir,
    gap: f64,
    pad: f64,
    children: &[FlexChild],
) -> Vec<Rect> {
    let n = children.len();
    if n == 0 {
        return Vec::new();
    }

    let (main_origin, main_len, cross_origin, cross_len, is_row) = match dir {
        FlexDir::Row => (parent.x, parent.width, parent.y, parent.height, true),
        FlexDir::Col => (parent.y, parent.height, parent.x, parent.width, false),
    };

    let inner_main = (main_len - 2.0 * pad - gap * (n.saturating_sub(1) as f64)).max(0.0);
    let inner_cross = (cross_len - 2.0 * pad).max(0.0);
    let basis_sum: f64 = children.iter().map(|c| c.basis).sum();
    let flex_sum: f64 = children.iter().map(|c| c.flex).sum();
    let free = (inner_main - basis_sum).max(0.0);

    let mut rects = Vec::with_capacity(n);
    let mut cursor = main_origin + pad;
    for (i, c) in children.iter().enumerate() {
        let extra = if flex_sum > 0.0 {
            free * (c.flex / flex_sum)
        } else {
            0.0
        };
        let size = c.basis + extra;
        let r = if is_row {
            Rect {
                x: cursor,
                y: cross_origin + pad,
                width: size,
                height: inner_cross,
            }
        } else {
            Rect {
                x: cross_origin + pad,
                y: cursor,
                width: inner_cross,
                height: size,
            }
        };
        rects.push(r);
        cursor += size;
        if i + 1 < n {
            cursor += gap;
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_empty() {
        let parent = Rect::new(0.0, 0.0, 100.0, 40.0);
        assert!(flex_solve(parent, FlexDir::Row, 0.0, 0.0, &[]).is_empty());
    }

    #[test]
    fn row_distributes_free_space_by_flex_and_keeps_the_gap() {
        let parent = Rect::new(10.0, 5.0, 100.0, 40.0);
        let kids = [
            FlexChild {
                basis: 20.0,
                flex: 1.0,
            },
            FlexChild {
                basis: 20.0,
                flex: 3.0,
            },
        ];
        // inner main = 100 - 8 - 4 = 88; free = 88 - 40 = 48; extras 12 and 36.
        let rects = flex_solve(parent, FlexDir::Row, 4.0, 4.0, &kids);
        assert_eq!(rects.len(), 2);
        assert!((rects[0].x - 14.0).abs() < 1e-9);
        assert!((rects[0].width - 32.0).abs() < 1e-9);
        assert!((rects[1].x - 50.0).abs() < 1e-9);
        assert!((rects[1].width - 56.0).abs() < 1e-9);
        assert!((rects[0].y - 9.0).abs() < 1e-9);
        assert!((rects[0].height - 32.0).abs() < 1e-9);
    }

    #[test]
    fn overflow_does_not_shrink_basis() {
        let parent = Rect::new(0.0, 0.0, 10.0, 10.0);
        let kids = [
            FlexChild {
                basis: 40.0,
                flex: 1.0,
            },
            FlexChild {
                basis: 40.0,
                flex: 1.0,
            },
        ];
        let rects = flex_solve(parent, FlexDir::Col, 0.0, 0.0, &kids);
        assert!((rects[0].height - 40.0).abs() < 1e-9);
        assert!((rects[1].y - 40.0).abs() < 1e-9);
    }
}
