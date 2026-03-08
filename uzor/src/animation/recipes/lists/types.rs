//! List animation types and configurations

use crate::animation::easing::Easing;
use crate::animation::stagger::{GridOrigin, DistanceMetric};
use std::time::Duration;

/// List and grid stagger animation variants
#[derive(Debug, Clone)]
pub enum ListAnimation {
    /// Sequential fade+slide in from bottom, each item staggered
    CascadeFadeIn {
        per_item_delay: Duration,
        item_duration: Duration,
        easing: Easing,
        slide_distance: f64,
    },

    /// Grid stagger from center with Euclidean distance (AnimeJS style)
    GridRipple {
        rows: usize,
        cols: usize,
        delay_per_unit: Duration,
        item_duration: Duration,
        easing: Easing,
        metric: DistanceMetric,
    },

    /// Grid stagger from edge/corner, wave propagation
    GridWave {
        rows: usize,
        cols: usize,
        origin: GridOrigin,
        delay_per_unit: Duration,
        item_duration: Duration,
        easing: Easing,
        metric: DistanceMetric,
    },

    /// Stagger along diagonal axis (row + col)
    DiagonalSweep {
        rows: usize,
        cols: usize,
        delay_per_step: Duration,
        item_duration: Duration,
        easing: Easing,
    },

    /// Stagger with random delays for masonry grid
    MasonryLoad {
        item_duration: Duration,
        stagger_delay: Duration,
        easing: Easing,
        slide_distance: f64,
    },

    /// Height animation for list items showing/hiding
    ExpandCollapse {
        duration: Duration,
        easing: Easing,
        from_height: f64,
        to_height: f64,
    },

    /// FLIP technique for reordering items
    FlipReorder {
        duration: Duration,
        easing: Easing,
    },

    /// Scale from 0 with stagger, back easing for overshoot
    ScalePopIn {
        per_item_delay: Duration,
        item_duration: Duration,
        easing: Easing,
        overshoot: f64,
    },

    /// Slide in from left/right with stagger
    SlideFromSide {
        per_item_delay: Duration,
        item_duration: Duration,
        easing: Easing,
        slide_distance: f64,
        from_left: bool,
    },

    /// Spiral traversal order stagger for grid
    SpiralReveal {
        rows: usize,
        cols: usize,
        delay_per_step: Duration,
        item_duration: Duration,
        easing: Easing,
    },

    /// Checkerboard pattern (even/odd tiles at different times)
    CheckerboardReveal {
        rows: usize,
        cols: usize,
        even_delay: Duration,
        odd_delay: Duration,
        item_duration: Duration,
        easing: Easing,
    },

    /// Framer Motion stagger children pattern
    FramerStagger {
        delay_children: Duration,
        stagger_children: Duration,
        item_duration: Duration,
        easing: Easing,
    },

    /// Snake/zigzag path through grid
    SnakePattern {
        rows: usize,
        cols: usize,
        delay_per_step: Duration,
        item_duration: Duration,
        easing: Easing,
    },
}

impl ListAnimation {
    /// Generate stagger delays for a linear list of items
    pub fn delays_for_count(&self, count: usize) -> Vec<Duration> {
        match self {
            ListAnimation::CascadeFadeIn { per_item_delay, .. } => {
                (0..count)
                    .map(|i| per_item_delay.mul_f32(i as f32))
                    .collect()
            }

            ListAnimation::MasonryLoad { stagger_delay, .. } => {
                (0..count)
                    .map(|i| stagger_delay.mul_f32(i as f32))
                    .collect()
            }

            ListAnimation::ScalePopIn { per_item_delay, .. } => {
                (0..count)
                    .map(|i| per_item_delay.mul_f32(i as f32))
                    .collect()
            }

            ListAnimation::SlideFromSide { per_item_delay, .. } => {
                (0..count)
                    .map(|i| per_item_delay.mul_f32(i as f32))
                    .collect()
            }

            ListAnimation::FramerStagger {
                delay_children,
                stagger_children,
                ..
            } => {
                (0..count)
                    .map(|i| *delay_children + stagger_children.mul_f32(i as f32))
                    .collect()
            }

            ListAnimation::ExpandCollapse { .. }
            | ListAnimation::FlipReorder { .. } => {
                vec![Duration::ZERO; count]
            }

            _ => {
                // Grid-based variants - return linear delays as fallback
                vec![Duration::ZERO; count]
            }
        }
    }

    /// Generate stagger delays for a 2D grid
    pub fn delays_for_grid(&self, rows: usize, cols: usize) -> Vec<Duration> {
        let total_count = rows * cols;
        if total_count == 0 {
            return Vec::new();
        }

        match self {
            ListAnimation::GridRipple {
                delay_per_unit, ..
            } => {
                let center_row = rows as f64 / 2.0;
                let center_col = cols as f64 / 2.0;

                (0..total_count)
                    .map(|index| {
                        let row = index / cols;
                        let col = index % cols;
                        let dx = col as f64 - center_col;
                        let dy = row as f64 - center_row;
                        let distance = (dx * dx + dy * dy).sqrt();
                        delay_per_unit.mul_f64(distance)
                    })
                    .collect()
            }

            ListAnimation::GridWave {
                origin,
                delay_per_unit,
                metric,
                ..
            } => {
                let (origin_col, origin_row) = match origin {
                    GridOrigin::TopLeft => (0, 0),
                    GridOrigin::TopRight => (cols.saturating_sub(1), 0),
                    GridOrigin::BottomLeft => (0, rows.saturating_sub(1)),
                    GridOrigin::BottomRight => {
                        (cols.saturating_sub(1), rows.saturating_sub(1))
                    }
                    GridOrigin::Center => (cols / 2, rows / 2),
                    GridOrigin::Cell(c, r) => (*c, *r),
                };

                (0..total_count)
                    .map(|index| {
                        let row = index / cols;
                        let col = index % cols;
                        let distance = calculate_distance(
                            col,
                            row,
                            origin_col,
                            origin_row,
                            *metric,
                        );
                        delay_per_unit.mul_f64(distance)
                    })
                    .collect()
            }

            ListAnimation::DiagonalSweep { delay_per_step, .. } => {
                (0..total_count)
                    .map(|index| {
                        let row = index / cols;
                        let col = index % cols;
                        let diagonal_index = row + col;
                        delay_per_step.mul_f32(diagonal_index as f32)
                    })
                    .collect()
            }

            ListAnimation::SpiralReveal { delay_per_step, .. } => {
                let mut delays = vec![Duration::ZERO; total_count];
                let mut spiral_order = Vec::with_capacity(total_count);

                // Calculate spiral order
                for index in 0..total_count {
                    let row = index / cols;
                    let col = index % cols;
                    let center_col = cols as f64 / 2.0;
                    let center_row = rows as f64 / 2.0;
                    let dx = col as f64 - center_col;
                    let dy = row as f64 - center_row;
                    let angle = dy.atan2(dx);
                    let distance = (dx * dx + dy * dy).sqrt();
                    let spiral_index = distance + angle / (2.0 * std::f64::consts::PI);
                    spiral_order.push((index, spiral_index));
                }

                // Sort by spiral index
                spiral_order.sort_by(|a, b| {
                    a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                });

                // Assign delays
                for (position, (index, _)) in spiral_order.iter().enumerate() {
                    delays[*index] = delay_per_step.mul_f32(position as f32);
                }

                delays
            }

            ListAnimation::CheckerboardReveal {
                even_delay,
                odd_delay,
                ..
            } => {
                (0..total_count)
                    .map(|index| {
                        let row = index / cols;
                        let col = index % cols;
                        if (row + col).is_multiple_of(2) {
                            *even_delay
                        } else {
                            *odd_delay
                        }
                    })
                    .collect()
            }

            ListAnimation::SnakePattern { delay_per_step, .. } => {
                (0..total_count)
                    .map(|index| {
                        let row = index / cols;
                        let col = index % cols;
                        let snake_index = if row.is_multiple_of(2) {
                            // Even rows: left to right
                            row * cols + col
                        } else {
                            // Odd rows: right to left
                            row * cols + (cols - 1 - col)
                        };
                        delay_per_step.mul_f32(snake_index as f32)
                    })
                    .collect()
            }

            _ => {
                // Linear variants - return zeros
                vec![Duration::ZERO; total_count]
            }
        }
    }

    /// Get the duration of each individual item animation
    pub fn item_duration(&self) -> Duration {
        match self {
            ListAnimation::CascadeFadeIn { item_duration, .. }
            | ListAnimation::GridRipple { item_duration, .. }
            | ListAnimation::GridWave { item_duration, .. }
            | ListAnimation::DiagonalSweep { item_duration, .. }
            | ListAnimation::MasonryLoad { item_duration, .. }
            | ListAnimation::ScalePopIn { item_duration, .. }
            | ListAnimation::SlideFromSide { item_duration, .. }
            | ListAnimation::SpiralReveal { item_duration, .. }
            | ListAnimation::CheckerboardReveal { item_duration, .. }
            | ListAnimation::FramerStagger { item_duration, .. }
            | ListAnimation::SnakePattern { item_duration, .. } => *item_duration,

            ListAnimation::ExpandCollapse { duration, .. }
            | ListAnimation::FlipReorder { duration, .. } => *duration,
        }
    }

    /// Get the easing function for the animation
    pub fn easing(&self) -> Easing {
        match self {
            ListAnimation::CascadeFadeIn { easing, .. }
            | ListAnimation::GridRipple { easing, .. }
            | ListAnimation::GridWave { easing, .. }
            | ListAnimation::DiagonalSweep { easing, .. }
            | ListAnimation::MasonryLoad { easing, .. }
            | ListAnimation::ExpandCollapse { easing, .. }
            | ListAnimation::FlipReorder { easing, .. }
            | ListAnimation::ScalePopIn { easing, .. }
            | ListAnimation::SlideFromSide { easing, .. }
            | ListAnimation::SpiralReveal { easing, .. }
            | ListAnimation::CheckerboardReveal { easing, .. }
            | ListAnimation::FramerStagger { easing, .. }
            | ListAnimation::SnakePattern { easing, .. } => *easing,
        }
    }
}

/// Calculate distance between two grid cells
fn calculate_distance(
    col: usize,
    row: usize,
    origin_col: usize,
    origin_row: usize,
    metric: DistanceMetric,
) -> f64 {
    match metric {
        DistanceMetric::Euclidean => {
            let dx = (col as f64 - origin_col as f64).abs();
            let dy = (row as f64 - origin_row as f64).abs();
            (dx * dx + dy * dy).sqrt()
        }
        DistanceMetric::Manhattan => {
            let dx = (col as i32 - origin_col as i32).abs() as f64;
            let dy = (row as i32 - origin_row as i32).abs() as f64;
            dx + dy
        }
        DistanceMetric::Chebyshev => {
            let dx = (col as i32 - origin_col as i32).abs() as f64;
            let dy = (row as i32 - origin_row as i32).abs() as f64;
            dx.max(dy)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cascade_fade_in_delays() {
        let anim = ListAnimation::CascadeFadeIn {
            per_item_delay: Duration::from_millis(50),
            item_duration: Duration::from_millis(300),
            easing: Easing::EaseOutCubic,
            slide_distance: 30.0,
        };

        let delays = anim.delays_for_count(5);
        assert_eq!(delays.len(), 5);
        assert_eq!(delays[0], Duration::ZERO);
        assert!(delays[1].as_millis() == 50, "expected ~50ms, got {:?}", delays[1]);
        assert!(delays[4].as_millis() == 200, "expected ~200ms, got {:?}", delays[4]);
    }

    #[test]
    fn test_grid_ripple_delays() {
        let anim = ListAnimation::GridRipple {
            rows: 3,
            cols: 3,
            delay_per_unit: Duration::from_millis(50),
            item_duration: Duration::from_millis(600),
            easing: Easing::EaseInOutQuad,
            metric: DistanceMetric::Euclidean,
        };

        let delays = anim.delays_for_grid(3, 3);
        assert_eq!(delays.len(), 9);
        // Center (1,1) should have minimal delay
        assert!(delays[4] < delays[0]);
    }

    #[test]
    fn test_diagonal_sweep_delays() {
        let anim = ListAnimation::DiagonalSweep {
            rows: 3,
            cols: 3,
            delay_per_step: Duration::from_millis(50),
            item_duration: Duration::from_millis(600),
            easing: Easing::EaseInOutCubic,
        };

        let delays = anim.delays_for_grid(3, 3);
        assert_eq!(delays.len(), 9);
        // Top-left (0,0) should have zero delay
        assert_eq!(delays[0], Duration::ZERO);
        // Bottom-right (2,2) should have max delay (diagonal 4)
        assert!(delays[8].as_millis() == 200, "expected ~200ms, got {:?}", delays[8]);
    }

    #[test]
    fn test_checkerboard_delays() {
        let anim = ListAnimation::CheckerboardReveal {
            rows: 4,
            cols: 4,
            even_delay: Duration::from_millis(0),
            odd_delay: Duration::from_millis(400),
            item_duration: Duration::from_millis(400),
            easing: Easing::EaseInOutCubic,
        };

        let delays = anim.delays_for_grid(4, 4);
        assert_eq!(delays.len(), 16);
        // (0,0) is even
        assert_eq!(delays[0], Duration::ZERO);
        // (0,1) is odd
        assert_eq!(delays[1], Duration::from_millis(400));
    }

    #[test]
    fn test_snake_pattern_delays() {
        let anim = ListAnimation::SnakePattern {
            rows: 3,
            cols: 3,
            delay_per_step: Duration::from_millis(40),
            item_duration: Duration::from_millis(500),
            easing: Easing::EaseOutQuad,
        };

        let delays = anim.delays_for_grid(3, 3);
        assert_eq!(delays.len(), 9);

        // First row (even): left to right
        assert!(delays[0] < delays[1]);
        assert!(delays[1] < delays[2]);

        // Second row (odd): right to left
        assert!(delays[5] < delays[4]);
        assert!(delays[4] < delays[3]);
    }
}
