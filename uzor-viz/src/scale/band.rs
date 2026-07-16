//! `BandScale` — ordinal/categorical axis (bar charts, histogram bins).
//!
//! NEW in V1 (no mlc source to harvest — mlc's index axis is the closest
//! relative but is bar-position-only, not a labeled-category ordinal
//! scale). Domain is `[0, category_count)`; each category owns an equal
//! band with symmetric inner padding.

use super::{Scale, Tick};

/// Ordinal scale — one band per category, normalized `[0, 1]` range.
#[derive(Debug, Clone)]
pub struct BandScale {
    pub categories: Vec<String>,
    /// Fraction of each band's width used as the gap between bands,
    /// `0.0` (no gap) to `0.9`.
    pub padding: f64,
}

impl BandScale {
    pub fn new(categories: Vec<String>, padding: f64) -> Self {
        Self { categories, padding: padding.clamp(0.0, 0.9) }
    }

    pub fn len(&self) -> usize {
        self.categories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.categories.is_empty()
    }

    fn step(&self) -> f64 {
        1.0 / self.categories.len().max(1) as f64
    }

    fn last_index(&self) -> usize {
        self.categories.len().max(1) - 1
    }

    /// Normalized `[0, 1]` extent of band `i`, inner padding applied
    /// symmetrically. Out-of-range `i` (or an empty scale) clamps to a
    /// zero-width band rather than panicking.
    pub fn band_range(&self, i: usize) -> (f64, f64) {
        if self.categories.is_empty() {
            return (0.0, 0.0);
        }
        let i = i.min(self.last_index());
        let step = self.step();
        let t0 = step * i as f64;
        let t1 = t0 + step;
        let gap = step * self.padding / 2.0;
        (t0 + gap, t1 - gap)
    }

    /// Normalized center of band `i`.
    pub fn band_center(&self, i: usize) -> f64 {
        let (t0, t1) = self.band_range(i);
        (t0 + t1) / 2.0
    }
}

impl Scale for BandScale {
    fn domain(&self) -> (f64, f64) {
        (0.0, self.categories.len() as f64)
    }

    fn map(&self, v: f64) -> f64 {
        if self.categories.is_empty() {
            return 0.5;
        }
        let i = v.round().clamp(0.0, self.last_index() as f64) as usize;
        self.band_center(i)
    }

    fn invert(&self, t: f64) -> f64 {
        if self.categories.is_empty() {
            return 0.0;
        }
        (t / self.step()).floor().clamp(0.0, self.last_index() as f64)
    }

    fn ticks(&self, _target_count: usize) -> Vec<Tick> {
        // One tick per category, always — a band scale's whole point is
        // labeling every category, not thinning to a target density.
        self.categories
            .iter()
            .enumerate()
            .map(|(i, label)| Tick { value: i as f64, label: label.clone() })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("c{i}")).collect()
    }

    #[test]
    fn band_range_math_no_padding() {
        let scale = BandScale::new(cats(4), 0.0);
        let (t0, t1) = scale.band_range(0);
        assert!((t0 - 0.0).abs() < 1e-9);
        assert!((t1 - 0.25).abs() < 1e-9);
        let (t0, t1) = scale.band_range(3);
        assert!((t0 - 0.75).abs() < 1e-9);
        assert!((t1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn band_range_shrinks_symmetrically_with_padding() {
        let scale = BandScale::new(cats(2), 0.2);
        let (t0, t1) = scale.band_range(0);
        // step = 0.5, gap = 0.5 * 0.2 / 2 = 0.05 each side.
        assert!((t0 - 0.05).abs() < 1e-9);
        assert!((t1 - 0.45).abs() < 1e-9);
    }

    #[test]
    fn map_returns_band_center() {
        let scale = BandScale::new(cats(4), 0.0);
        assert!((scale.map(0.0) - 0.125).abs() < 1e-9);
        assert!((scale.map(3.0) - 0.875).abs() < 1e-9);
    }

    #[test]
    fn empty_scale_does_not_panic() {
        let scale = BandScale::new(Vec::new(), 0.1);
        assert_eq!(scale.band_range(0), (0.0, 0.0));
        assert_eq!(scale.map(0.0), 0.5);
        assert!(scale.ticks(5).is_empty());
    }

    #[test]
    fn ticks_one_per_category() {
        let scale = BandScale::new(cats(5), 0.1);
        let ticks = scale.ticks(2); // target_count ignored by design
        assert_eq!(ticks.len(), 5);
        assert_eq!(ticks[2].label, "c2");
    }
}
