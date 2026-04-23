//! Default parameter sets for list animations

use crate::animation::easing::Easing;
use crate::animation::stagger::{GridOrigin, DistanceMetric};
use std::time::Duration;

/// Default parameters for CascadeFadeIn
#[derive(Debug, Clone, Copy)]
pub struct CascadeFadeInDefaults {
    pub per_item_delay: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
    pub slide_distance: f64,
}

impl Default for CascadeFadeInDefaults {
    fn default() -> Self {
        Self {
            per_item_delay: Duration::from_millis(50),
            item_duration: Duration::from_millis(300),
            easing: Easing::EaseOutCubic,
            slide_distance: 30.0,
        }
    }
}

/// Default parameters for GridRipple
#[derive(Debug, Clone, Copy)]
pub struct GridRippleDefaults {
    pub delay_per_unit: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
    pub metric: DistanceMetric,
}

impl Default for GridRippleDefaults {
    fn default() -> Self {
        Self {
            delay_per_unit: Duration::from_millis(50),
            item_duration: Duration::from_millis(600),
            easing: Easing::EaseInOutQuad,
            metric: DistanceMetric::Euclidean,
        }
    }
}

/// Default parameters for GridWave
#[derive(Debug, Clone, Copy)]
pub struct GridWaveDefaults {
    pub origin: GridOrigin,
    pub delay_per_unit: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
    pub metric: DistanceMetric,
}

impl Default for GridWaveDefaults {
    fn default() -> Self {
        Self {
            origin: GridOrigin::TopLeft,
            delay_per_unit: Duration::from_millis(80),
            item_duration: Duration::from_millis(500),
            easing: Easing::EaseInOutQuad,
            metric: DistanceMetric::Manhattan,
        }
    }
}

/// Default parameters for DiagonalSweep
#[derive(Debug, Clone, Copy)]
pub struct DiagonalSweepDefaults {
    pub delay_per_step: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
}

impl Default for DiagonalSweepDefaults {
    fn default() -> Self {
        Self {
            delay_per_step: Duration::from_millis(30),
            item_duration: Duration::from_millis(600),
            easing: Easing::EaseInOutCubic,
        }
    }
}

/// Default parameters for MasonryLoad
#[derive(Debug, Clone, Copy)]
pub struct MasonryLoadDefaults {
    pub item_duration: Duration,
    pub stagger_delay: Duration,
    pub easing: Easing,
    pub slide_distance: f64,
}

impl Default for MasonryLoadDefaults {
    fn default() -> Self {
        Self {
            item_duration: Duration::from_millis(400),
            stagger_delay: Duration::from_millis(30),
            easing: Easing::EaseOutCubic,
            slide_distance: 20.0,
        }
    }
}

/// Default parameters for ExpandCollapse
#[derive(Debug, Clone, Copy)]
pub struct ExpandCollapseDefaults {
    pub duration: Duration,
    pub easing: Easing,
    pub from_height: f64,
    pub to_height: f64,
}

impl Default for ExpandCollapseDefaults {
    fn default() -> Self {
        Self {
            duration: Duration::from_millis(300),
            easing: Easing::EaseInOutCubic,
            from_height: 0.0,
            to_height: 1.0,
        }
    }
}

/// Default parameters for FlipReorder
#[derive(Debug, Clone, Copy)]
pub struct FlipReorderDefaults {
    pub duration: Duration,
    pub easing: Easing,
}

impl Default for FlipReorderDefaults {
    fn default() -> Self {
        Self {
            duration: Duration::from_millis(400),
            easing: Easing::EaseInOutCubic,
        }
    }
}

/// Default parameters for ScalePopIn
#[derive(Debug, Clone, Copy)]
pub struct ScalePopInDefaults {
    pub per_item_delay: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
    pub overshoot: f64,
}

impl Default for ScalePopInDefaults {
    fn default() -> Self {
        Self {
            per_item_delay: Duration::from_millis(40),
            item_duration: Duration::from_millis(600),
            easing: Easing::EaseOutBack,
            overshoot: 1.2,
        }
    }
}

/// Default parameters for SlideFromSide
#[derive(Debug, Clone, Copy)]
pub struct SlideFromSideDefaults {
    pub per_item_delay: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
    pub slide_distance: f64,
}

impl Default for SlideFromSideDefaults {
    fn default() -> Self {
        Self {
            per_item_delay: Duration::from_millis(60),
            item_duration: Duration::from_millis(400),
            easing: Easing::EaseOutCubic,
            slide_distance: 20.0,
        }
    }
}

/// Default parameters for SpiralReveal
#[derive(Debug, Clone, Copy)]
pub struct SpiralRevealDefaults {
    pub delay_per_step: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
}

impl Default for SpiralRevealDefaults {
    fn default() -> Self {
        Self {
            delay_per_step: Duration::from_millis(30),
            item_duration: Duration::from_millis(600),
            easing: Easing::EaseOutElastic,
        }
    }
}

/// Default parameters for CheckerboardReveal
#[derive(Debug, Clone, Copy)]
pub struct CheckerboardRevealDefaults {
    pub even_delay: Duration,
    pub odd_delay: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
}

impl Default for CheckerboardRevealDefaults {
    fn default() -> Self {
        Self {
            even_delay: Duration::from_millis(0),
            odd_delay: Duration::from_millis(400),
            item_duration: Duration::from_millis(400),
            easing: Easing::EaseInOutCubic,
        }
    }
}

/// Default parameters for FramerStagger
#[derive(Debug, Clone, Copy)]
pub struct FramerStaggerDefaults {
    pub delay_children: Duration,
    pub stagger_children: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
}

impl Default for FramerStaggerDefaults {
    fn default() -> Self {
        Self {
            delay_children: Duration::from_millis(200),
            stagger_children: Duration::from_millis(100),
            item_duration: Duration::from_millis(400),
            easing: Easing::EaseOutCubic,
        }
    }
}

/// Default parameters for SnakePattern
#[derive(Debug, Clone, Copy)]
pub struct SnakePatternDefaults {
    pub delay_per_step: Duration,
    pub item_duration: Duration,
    pub easing: Easing,
}

impl Default for SnakePatternDefaults {
    fn default() -> Self {
        Self {
            delay_per_step: Duration::from_millis(40),
            item_duration: Duration::from_millis(500),
            easing: Easing::EaseOutQuad,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_defaults_instantiate() {
        let _d1 = CascadeFadeInDefaults::default();
        let _d2 = GridRippleDefaults::default();
        let _d3 = GridWaveDefaults::default();
        let _d4 = DiagonalSweepDefaults::default();
        let _d5 = MasonryLoadDefaults::default();
        let _d6 = ExpandCollapseDefaults::default();
        let _d7 = FlipReorderDefaults::default();
        let _d8 = ScalePopInDefaults::default();
        let _d9 = SlideFromSideDefaults::default();
        let _d10 = SpiralRevealDefaults::default();
        let _d11 = CheckerboardRevealDefaults::default();
        let _d12 = FramerStaggerDefaults::default();
        let _d13 = SnakePatternDefaults::default();
    }

    #[test]
    fn test_defaults_have_reasonable_values() {
        let cascade = CascadeFadeInDefaults::default();
        assert!(cascade.per_item_delay > Duration::ZERO);
        assert!(cascade.item_duration > Duration::ZERO);
        assert!(cascade.slide_distance > 0.0);

        let grid = GridRippleDefaults::default();
        assert!(grid.delay_per_unit > Duration::ZERO);
        assert!(grid.item_duration > Duration::ZERO);
    }
}
