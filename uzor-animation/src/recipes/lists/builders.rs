//! Builder pattern for list animations

use super::types::ListAnimation;
use super::defaults::*;
use crate::easing::Easing;
use crate::stagger::{GridOrigin, DistanceMetric};
use std::time::Duration;

/// Builder for CascadeFadeIn animation
#[derive(Debug, Clone)]
pub struct CascadeFadeInBuilder {
    per_item_delay: Duration,
    item_duration: Duration,
    easing: Easing,
    slide_distance: f64,
}

impl Default for CascadeFadeInBuilder {
    fn default() -> Self {
        let defaults = CascadeFadeInDefaults::default();
        Self {
            per_item_delay: defaults.per_item_delay,
            item_duration: defaults.item_duration,
            easing: defaults.easing,
            slide_distance: defaults.slide_distance,
        }
    }
}

impl CascadeFadeInBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn per_item_delay(mut self, delay: Duration) -> Self {
        self.per_item_delay = delay;
        self
    }

    pub fn item_duration(mut self, duration: Duration) -> Self {
        self.item_duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn slide_distance(mut self, distance: f64) -> Self {
        self.slide_distance = distance;
        self
    }

    pub fn build(self) -> ListAnimation {
        ListAnimation::CascadeFadeIn {
            per_item_delay: self.per_item_delay,
            item_duration: self.item_duration,
            easing: self.easing,
            slide_distance: self.slide_distance,
        }
    }
}

/// Builder for GridRipple animation
#[derive(Debug, Clone)]
pub struct GridRippleBuilder {
    rows: usize,
    cols: usize,
    delay_per_unit: Duration,
    item_duration: Duration,
    easing: Easing,
    metric: DistanceMetric,
}

impl GridRippleBuilder {
    pub fn new(rows: usize, cols: usize) -> Self {
        let defaults = GridRippleDefaults::default();
        Self {
            rows,
            cols,
            delay_per_unit: defaults.delay_per_unit,
            item_duration: defaults.item_duration,
            easing: defaults.easing,
            metric: defaults.metric,
        }
    }

    pub fn delay_per_unit(mut self, delay: Duration) -> Self {
        self.delay_per_unit = delay;
        self
    }

    pub fn item_duration(mut self, duration: Duration) -> Self {
        self.item_duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    pub fn build(self) -> ListAnimation {
        ListAnimation::GridRipple {
            rows: self.rows,
            cols: self.cols,
            delay_per_unit: self.delay_per_unit,
            item_duration: self.item_duration,
            easing: self.easing,
            metric: self.metric,
        }
    }
}

/// Builder for GridWave animation
#[derive(Debug, Clone)]
pub struct GridWaveBuilder {
    rows: usize,
    cols: usize,
    origin: GridOrigin,
    delay_per_unit: Duration,
    item_duration: Duration,
    easing: Easing,
    metric: DistanceMetric,
}

impl GridWaveBuilder {
    pub fn new(rows: usize, cols: usize) -> Self {
        let defaults = GridWaveDefaults::default();
        Self {
            rows,
            cols,
            origin: defaults.origin,
            delay_per_unit: defaults.delay_per_unit,
            item_duration: defaults.item_duration,
            easing: defaults.easing,
            metric: defaults.metric,
        }
    }

    pub fn origin(mut self, origin: GridOrigin) -> Self {
        self.origin = origin;
        self
    }

    pub fn delay_per_unit(mut self, delay: Duration) -> Self {
        self.delay_per_unit = delay;
        self
    }

    pub fn item_duration(mut self, duration: Duration) -> Self {
        self.item_duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    pub fn build(self) -> ListAnimation {
        ListAnimation::GridWave {
            rows: self.rows,
            cols: self.cols,
            origin: self.origin,
            delay_per_unit: self.delay_per_unit,
            item_duration: self.item_duration,
            easing: self.easing,
            metric: self.metric,
        }
    }
}

/// Builder for DiagonalSweep animation
#[derive(Debug, Clone)]
pub struct DiagonalSweepBuilder {
    rows: usize,
    cols: usize,
    delay_per_step: Duration,
    item_duration: Duration,
    easing: Easing,
}

impl DiagonalSweepBuilder {
    pub fn new(rows: usize, cols: usize) -> Self {
        let defaults = DiagonalSweepDefaults::default();
        Self {
            rows,
            cols,
            delay_per_step: defaults.delay_per_step,
            item_duration: defaults.item_duration,
            easing: defaults.easing,
        }
    }

    pub fn delay_per_step(mut self, delay: Duration) -> Self {
        self.delay_per_step = delay;
        self
    }

    pub fn item_duration(mut self, duration: Duration) -> Self {
        self.item_duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ListAnimation {
        ListAnimation::DiagonalSweep {
            rows: self.rows,
            cols: self.cols,
            delay_per_step: self.delay_per_step,
            item_duration: self.item_duration,
            easing: self.easing,
        }
    }
}

/// Builder for ScalePopIn animation
#[derive(Debug, Clone)]
pub struct ScalePopInBuilder {
    per_item_delay: Duration,
    item_duration: Duration,
    easing: Easing,
    overshoot: f64,
}

impl Default for ScalePopInBuilder {
    fn default() -> Self {
        let defaults = ScalePopInDefaults::default();
        Self {
            per_item_delay: defaults.per_item_delay,
            item_duration: defaults.item_duration,
            easing: defaults.easing,
            overshoot: defaults.overshoot,
        }
    }
}

impl ScalePopInBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn per_item_delay(mut self, delay: Duration) -> Self {
        self.per_item_delay = delay;
        self
    }

    pub fn item_duration(mut self, duration: Duration) -> Self {
        self.item_duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn overshoot(mut self, overshoot: f64) -> Self {
        self.overshoot = overshoot;
        self
    }

    pub fn build(self) -> ListAnimation {
        ListAnimation::ScalePopIn {
            per_item_delay: self.per_item_delay,
            item_duration: self.item_duration,
            easing: self.easing,
            overshoot: self.overshoot,
        }
    }
}

/// Builder for SlideFromSide animation
#[derive(Debug, Clone)]
pub struct SlideFromSideBuilder {
    per_item_delay: Duration,
    item_duration: Duration,
    easing: Easing,
    slide_distance: f64,
    from_left: bool,
}

impl Default for SlideFromSideBuilder {
    fn default() -> Self {
        let defaults = SlideFromSideDefaults::default();
        Self {
            per_item_delay: defaults.per_item_delay,
            item_duration: defaults.item_duration,
            easing: defaults.easing,
            slide_distance: defaults.slide_distance,
            from_left: true,
        }
    }
}

impl SlideFromSideBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn per_item_delay(mut self, delay: Duration) -> Self {
        self.per_item_delay = delay;
        self
    }

    pub fn item_duration(mut self, duration: Duration) -> Self {
        self.item_duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn slide_distance(mut self, distance: f64) -> Self {
        self.slide_distance = distance;
        self
    }

    pub fn from_left(mut self, from_left: bool) -> Self {
        self.from_left = from_left;
        self
    }

    pub fn build(self) -> ListAnimation {
        ListAnimation::SlideFromSide {
            per_item_delay: self.per_item_delay,
            item_duration: self.item_duration,
            easing: self.easing,
            slide_distance: self.slide_distance,
            from_left: self.from_left,
        }
    }
}

/// Builder for SpiralReveal animation
#[derive(Debug, Clone)]
pub struct SpiralRevealBuilder {
    rows: usize,
    cols: usize,
    delay_per_step: Duration,
    item_duration: Duration,
    easing: Easing,
}

impl SpiralRevealBuilder {
    pub fn new(rows: usize, cols: usize) -> Self {
        let defaults = SpiralRevealDefaults::default();
        Self {
            rows,
            cols,
            delay_per_step: defaults.delay_per_step,
            item_duration: defaults.item_duration,
            easing: defaults.easing,
        }
    }

    pub fn delay_per_step(mut self, delay: Duration) -> Self {
        self.delay_per_step = delay;
        self
    }

    pub fn item_duration(mut self, duration: Duration) -> Self {
        self.item_duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ListAnimation {
        ListAnimation::SpiralReveal {
            rows: self.rows,
            cols: self.cols,
            delay_per_step: self.delay_per_step,
            item_duration: self.item_duration,
            easing: self.easing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cascade_builder() {
        let anim = CascadeFadeInBuilder::new()
            .per_item_delay(Duration::from_millis(100))
            .item_duration(Duration::from_millis(500))
            .easing(Easing::EaseInOutQuad)
            .slide_distance(50.0)
            .build();

        match anim {
            ListAnimation::CascadeFadeIn {
                per_item_delay,
                item_duration,
                ..
            } => {
                assert_eq!(per_item_delay, Duration::from_millis(100));
                assert_eq!(item_duration, Duration::from_millis(500));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_grid_ripple_builder() {
        let anim = GridRippleBuilder::new(10, 10)
            .delay_per_unit(Duration::from_millis(75))
            .metric(DistanceMetric::Manhattan)
            .build();

        match anim {
            ListAnimation::GridRipple {
                rows,
                cols,
                delay_per_unit,
                metric,
                ..
            } => {
                assert_eq!(rows, 10);
                assert_eq!(cols, 10);
                assert_eq!(delay_per_unit, Duration::from_millis(75));
                assert!(matches!(metric, DistanceMetric::Manhattan));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_grid_wave_builder() {
        let anim = GridWaveBuilder::new(8, 8)
            .origin(GridOrigin::Center)
            .delay_per_unit(Duration::from_millis(60))
            .build();

        match anim {
            ListAnimation::GridWave {
                rows,
                cols,
                origin,
                ..
            } => {
                assert_eq!(rows, 8);
                assert_eq!(cols, 8);
                assert!(matches!(origin, GridOrigin::Center));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_diagonal_sweep_builder() {
        let anim = DiagonalSweepBuilder::new(5, 5)
            .delay_per_step(Duration::from_millis(20))
            .easing(Easing::EaseOutExpo)
            .build();

        match anim {
            ListAnimation::DiagonalSweep {
                rows,
                cols,
                delay_per_step,
                ..
            } => {
                assert_eq!(rows, 5);
                assert_eq!(cols, 5);
                assert_eq!(delay_per_step, Duration::from_millis(20));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_scale_pop_builder() {
        let anim = ScalePopInBuilder::new()
            .per_item_delay(Duration::from_millis(30))
            .overshoot(1.5)
            .build();

        match anim {
            ListAnimation::ScalePopIn {
                per_item_delay,
                overshoot,
                ..
            } => {
                assert_eq!(per_item_delay, Duration::from_millis(30));
                assert_eq!(overshoot, 1.5);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_slide_from_side_builder() {
        let anim = SlideFromSideBuilder::new()
            .slide_distance(40.0)
            .from_left(false)
            .build();

        match anim {
            ListAnimation::SlideFromSide {
                slide_distance,
                from_left,
                ..
            } => {
                assert_eq!(slide_distance, 40.0);
                assert!(!from_left);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_spiral_reveal_builder() {
        let anim = SpiralRevealBuilder::new(12, 12)
            .delay_per_step(Duration::from_millis(25))
            .item_duration(Duration::from_millis(700))
            .build();

        match anim {
            ListAnimation::SpiralReveal {
                rows,
                cols,
                delay_per_step,
                item_duration,
                ..
            } => {
                assert_eq!(rows, 12);
                assert_eq!(cols, 12);
                assert_eq!(delay_per_step, Duration::from_millis(25));
                assert_eq!(item_duration, Duration::from_millis(700));
            }
            _ => panic!("Wrong variant"),
        }
    }
}
