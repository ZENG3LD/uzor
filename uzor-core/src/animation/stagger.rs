//! Stagger patterns for coordinating animation timing across multiple elements
//!
//! Based on research from AnimeJS, GSAP, and Framer Motion stagger systems.
//! See `research/stagger-patterns.md` for implementation details.

use std::time::Duration;
use super::easing::Easing;

/// Where the stagger animation starts from in a linear list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaggerOrigin {
    /// First element (index 0)
    First,
    /// Last element
    Last,
    /// Center element
    Center,
    /// Specific index
    Index(usize),
}

/// Origin for grid stagger
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridOrigin {
    /// Top-left corner
    TopLeft,
    /// Top-right corner
    TopRight,
    /// Bottom-left corner
    BottomLeft,
    /// Bottom-right corner
    BottomRight,
    /// Center of grid
    Center,
    /// Specific cell (col, row)
    Cell(usize, usize),
}

/// Distance calculation method for grid stagger
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    /// sqrt(dx^2 + dy^2) — circular propagation (natural wave)
    Euclidean,
    /// |dx| + |dy| — diamond propagation (Manhattan distance)
    Manhattan,
    /// max(|dx|, |dy|) — square propagation (Chebyshev distance)
    Chebyshev,
}

/// Computes delay for each element in a linear list
///
/// # Example
/// ```
/// use std::time::Duration;
/// use uzor_core::animation::stagger::{LinearStagger, StaggerOrigin};
///
/// let stagger = LinearStagger::new(Duration::from_millis(100))
///     .from(StaggerOrigin::Center);
///
/// let delays = stagger.delays(10);
/// // Center element (index 5) has minimal delay,
/// // edges have maximum delay
/// ```
#[derive(Debug, Clone)]
pub struct LinearStagger {
    /// Base delay between consecutive elements
    pub delay: Duration,
    /// Where animation starts
    pub from: StaggerOrigin,
    /// Optional easing applied to delay distribution
    pub easing: Option<Easing>,
}

impl LinearStagger {
    /// Create new linear stagger with base delay
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            from: StaggerOrigin::First,
            easing: None,
        }
    }

    /// Set stagger origin
    pub fn from(mut self, origin: StaggerOrigin) -> Self {
        self.from = origin;
        self
    }

    /// Set easing function for delay distribution
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = Some(easing);
        self
    }

    /// Compute delays for N elements. Returns Vec<Duration> of length count.
    pub fn delays(&self, count: usize) -> Vec<Duration> {
        if count == 0 {
            return Vec::new();
        }

        if count == 1 {
            return vec![Duration::ZERO];
        }

        let origin_index = match self.from {
            StaggerOrigin::First => 0,
            StaggerOrigin::Last => count - 1,
            StaggerOrigin::Center => count / 2,
            StaggerOrigin::Index(idx) => idx.min(count - 1),
        };

        let mut delays = Vec::with_capacity(count);

        for i in 0..count {
            let distance = (i as i32 - origin_index as i32).abs() as f32;
            let max_distance = origin_index.max(count - 1 - origin_index) as f32;

            let delay = if let Some(easing) = self.easing {
                // Normalize to 0..1, apply easing, scale back
                let normalized = if max_distance > 0.0 {
                    distance / max_distance
                } else {
                    0.0
                };
                let eased = easing.ease_f32(normalized);
                self.delay.mul_f32(eased * max_distance)
            } else {
                self.delay.mul_f32(distance)
            };

            delays.push(delay);
        }

        delays
    }

    /// Compute delay for specific element
    pub fn delay_for(&self, index: usize, count: usize) -> Duration {
        if count == 0 || count == 1 {
            return Duration::ZERO;
        }

        let origin_index = match self.from {
            StaggerOrigin::First => 0,
            StaggerOrigin::Last => count - 1,
            StaggerOrigin::Center => count / 2,
            StaggerOrigin::Index(idx) => idx.min(count - 1),
        };

        let distance = (index as i32 - origin_index as i32).abs() as f32;
        let max_distance = origin_index.max(count - 1 - origin_index) as f32;

        if let Some(easing) = self.easing {
            let normalized = if max_distance > 0.0 {
                distance / max_distance
            } else {
                0.0
            };
            let eased = easing.ease_f32(normalized);
            self.delay.mul_f32(eased * max_distance)
        } else {
            self.delay.mul_f32(distance)
        }
    }
}

impl Default for LinearStagger {
    fn default() -> Self {
        Self::new(Duration::from_millis(100))
    }
}

/// Computes delay for each element in a 2D grid
///
/// # Example
/// ```
/// use std::time::Duration;
/// use uzor_core::animation::stagger::{GridStagger, GridOrigin, DistanceMetric};
///
/// let stagger = GridStagger::new(Duration::from_millis(50), 14, 5)
///     .from(GridOrigin::Center)
///     .metric(DistanceMetric::Euclidean);
///
/// // Get delay for cell at column 7, row 2
/// let delay = stagger.delay_for(7, 2);
///
/// // Or get all delays at once
/// let all_delays = stagger.delays();
/// ```
#[derive(Debug, Clone)]
pub struct GridStagger {
    /// Base delay per unit distance
    pub delay: Duration,
    /// Grid dimensions (columns, rows)
    pub cols: usize,
    pub rows: usize,
    /// Where animation starts
    pub from: GridOrigin,
    /// Distance calculation method
    pub metric: DistanceMetric,
    /// Optional easing applied to delay distribution
    pub easing: Option<Easing>,
}

impl GridStagger {
    /// Create new grid stagger with base delay and dimensions
    pub fn new(delay: Duration, cols: usize, rows: usize) -> Self {
        Self {
            delay,
            cols,
            rows,
            from: GridOrigin::TopLeft,
            metric: DistanceMetric::Euclidean,
            easing: None,
        }
    }

    /// Set stagger origin
    pub fn from(mut self, origin: GridOrigin) -> Self {
        self.from = origin;
        self
    }

    /// Set distance metric
    pub fn metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Set easing function for delay distribution
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = Some(easing);
        self
    }

    /// Compute delays for all grid cells. Returns Vec<Duration> of length cols*rows.
    /// Index [row * cols + col] gives delay for cell (col, row).
    pub fn delays(&self) -> Vec<Duration> {
        if self.cols == 0 || self.rows == 0 {
            return Vec::new();
        }

        let total_cells = self.cols * self.rows;
        let mut delays = Vec::with_capacity(total_cells);

        let max_distance = self.calculate_max_distance();

        for row in 0..self.rows {
            for col in 0..self.cols {
                let distance = self.calculate_distance(col, row);

                let delay = if let Some(easing) = self.easing {
                    // Normalize, apply easing, scale back
                    let normalized = if max_distance > 0.0 {
                        distance / max_distance
                    } else {
                        0.0
                    };
                    let eased = easing.ease_f32(normalized);
                    self.delay.mul_f32(eased * max_distance)
                } else {
                    self.delay.mul_f32(distance)
                };

                delays.push(delay);
            }
        }

        delays
    }

    /// Compute delay for specific cell
    pub fn delay_for(&self, col: usize, row: usize) -> Duration {
        if self.cols == 0 || self.rows == 0 {
            return Duration::ZERO;
        }

        if col >= self.cols || row >= self.rows {
            return Duration::ZERO;
        }

        let distance = self.calculate_distance(col, row);
        let max_distance = self.calculate_max_distance();

        if let Some(easing) = self.easing {
            let normalized = if max_distance > 0.0 {
                distance / max_distance
            } else {
                0.0
            };
            let eased = easing.ease_f32(normalized);
            self.delay.mul_f32(eased * max_distance)
        } else {
            self.delay.mul_f32(distance)
        }
    }

    /// Resolve origin position to (col, row) coordinates
    fn resolve_origin(&self) -> (usize, usize) {
        match self.from {
            GridOrigin::TopLeft => (0, 0),
            GridOrigin::TopRight => (self.cols.saturating_sub(1), 0),
            GridOrigin::BottomLeft => (0, self.rows.saturating_sub(1)),
            GridOrigin::BottomRight => (
                self.cols.saturating_sub(1),
                self.rows.saturating_sub(1),
            ),
            GridOrigin::Center => (self.cols / 2, self.rows / 2),
            GridOrigin::Cell(col, row) => (
                col.min(self.cols.saturating_sub(1)),
                row.min(self.rows.saturating_sub(1)),
            ),
        }
    }

    /// Calculate distance from cell to origin using selected metric
    fn calculate_distance(&self, col: usize, row: usize) -> f32 {
        let (origin_col, origin_row) = self.resolve_origin();

        match self.metric {
            DistanceMetric::Euclidean => {
                let dx = (col as f32 - origin_col as f32).abs();
                let dy = (row as f32 - origin_row as f32).abs();
                (dx * dx + dy * dy).sqrt()
            }
            DistanceMetric::Manhattan => {
                let dx = (col as i32 - origin_col as i32).abs() as f32;
                let dy = (row as i32 - origin_row as i32).abs() as f32;
                dx + dy
            }
            DistanceMetric::Chebyshev => {
                let dx = (col as i32 - origin_col as i32).abs() as f32;
                let dy = (row as i32 - origin_row as i32).abs() as f32;
                dx.max(dy)
            }
        }
    }

    /// Calculate maximum possible distance in the grid
    fn calculate_max_distance(&self) -> f32 {
        if self.cols == 0 || self.rows == 0 {
            return 0.0;
        }

        let (origin_col, origin_row) = self.resolve_origin();

        // Check all four corners to find maximum distance
        let corners = [
            (0, 0),
            (self.cols.saturating_sub(1), 0),
            (0, self.rows.saturating_sub(1)),
            (self.cols.saturating_sub(1), self.rows.saturating_sub(1)),
        ];

        corners
            .iter()
            .map(|&(col, row)| {
                match self.metric {
                    DistanceMetric::Euclidean => {
                        let dx = (col as f32 - origin_col as f32).abs();
                        let dy = (row as f32 - origin_row as f32).abs();
                        (dx * dx + dy * dy).sqrt()
                    }
                    DistanceMetric::Manhattan => {
                        let dx = (col as i32 - origin_col as i32).abs() as f32;
                        let dy = (row as i32 - origin_row as i32).abs() as f32;
                        dx + dy
                    }
                    DistanceMetric::Chebyshev => {
                        let dx = (col as i32 - origin_col as i32).abs() as f32;
                        let dy = (row as i32 - origin_row as i32).abs() as f32;
                        dx.max(dy)
                    }
                }
            })
            .fold(0.0_f32, f32::max)
    }
}

impl Default for GridStagger {
    fn default() -> Self {
        Self::new(Duration::from_millis(50), 10, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to check if durations are approximately equal (within 1ms)
    fn approx_eq(a: Duration, b: Duration) -> bool {
        let diff = if a > b { a - b } else { b - a };
        diff < Duration::from_micros(1000)
    }

    #[test]
    fn test_linear_stagger_first() {
        let stagger = LinearStagger::new(Duration::from_millis(100))
            .from(StaggerOrigin::First);

        let delays = stagger.delays(5);
        assert_eq!(delays.len(), 5);
        assert_eq!(delays[0], Duration::ZERO);
        assert!(approx_eq(delays[1], Duration::from_millis(100)));
        assert!(approx_eq(delays[4], Duration::from_millis(400)));
    }

    #[test]
    fn test_linear_stagger_center() {
        let stagger = LinearStagger::new(Duration::from_millis(100))
            .from(StaggerOrigin::Center);

        let delays = stagger.delays(5);
        assert_eq!(delays[2], Duration::ZERO); // Center has zero delay
        assert!(approx_eq(delays[0], Duration::from_millis(200))); // 2 steps from center
        assert!(approx_eq(delays[4], Duration::from_millis(200))); // 2 steps from center
    }

    #[test]
    fn test_linear_stagger_edge_cases() {
        let stagger = LinearStagger::new(Duration::from_millis(100));

        assert_eq!(stagger.delays(0).len(), 0);
        assert_eq!(stagger.delays(1), vec![Duration::ZERO]);
    }

    #[test]
    fn test_grid_stagger_top_left() {
        let stagger = GridStagger::new(Duration::from_millis(50), 3, 3)
            .from(GridOrigin::TopLeft)
            .metric(DistanceMetric::Manhattan);

        // Top-left corner (0, 0) should have zero delay
        assert_eq!(stagger.delay_for(0, 0), Duration::ZERO);

        // Bottom-right corner (2, 2) should have max delay
        // Manhattan distance: |2-0| + |2-0| = 4
        assert!(approx_eq(stagger.delay_for(2, 2), Duration::from_millis(200)));
    }

    #[test]
    fn test_grid_stagger_center() {
        let stagger = GridStagger::new(Duration::from_millis(50), 5, 5)
            .from(GridOrigin::Center)
            .metric(DistanceMetric::Euclidean);

        // Center (2, 2) should have zero delay
        assert_eq!(stagger.delay_for(2, 2), Duration::ZERO);

        // Adjacent cells should have distance ~1.0
        let delay = stagger.delay_for(3, 2);
        assert!(delay >= Duration::from_millis(45) && delay <= Duration::from_millis(55));
    }

    #[test]
    fn test_grid_stagger_metrics() {
        let euclidean = GridStagger::new(Duration::from_millis(100), 5, 5)
            .metric(DistanceMetric::Euclidean);

        let manhattan = GridStagger::new(Duration::from_millis(100), 5, 5)
            .metric(DistanceMetric::Manhattan);

        let chebyshev = GridStagger::new(Duration::from_millis(100), 5, 5)
            .metric(DistanceMetric::Chebyshev);

        // Diagonal from origin (0,0) to (2,2)
        let euclidean_delay = euclidean.delay_for(2, 2);
        let manhattan_delay = manhattan.delay_for(2, 2);
        let chebyshev_delay = chebyshev.delay_for(2, 2);

        // Euclidean: sqrt(4+4) ≈ 2.828
        // Manhattan: 2+2 = 4
        // Chebyshev: max(2,2) = 2
        assert!(euclidean_delay > chebyshev_delay);
        assert!(manhattan_delay > euclidean_delay);
    }

    #[test]
    fn test_grid_stagger_zero_dimensions() {
        let stagger = GridStagger::new(Duration::from_millis(50), 0, 0);
        assert_eq!(stagger.delays().len(), 0);
        assert_eq!(stagger.delay_for(0, 0), Duration::ZERO);
    }

    #[test]
    fn test_grid_delays_length() {
        let stagger = GridStagger::new(Duration::from_millis(50), 4, 3);
        let delays = stagger.delays();
        assert_eq!(delays.len(), 12); // 4 cols × 3 rows
    }
}
