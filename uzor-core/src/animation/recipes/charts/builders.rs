//! Builder pattern for chart animations

use super::defaults::*;
use super::types::{ChartAnimation, TickerDirection};
use crate::animation::easing::Easing;
use crate::animation::spring::Spring;

/// Builder for bar growth animation
#[derive(Debug, Clone)]
pub struct BarGrowBuilder {
    duration_ms: u64,
    stagger_delay_ms: u64,
    easing: Easing,
    count: usize,
}

impl BarGrowBuilder {
    pub fn new(count: usize) -> Self {
        let defaults = BarGrowDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            stagger_delay_ms: defaults.stagger_delay_ms,
            easing: defaults.easing,
            count,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn stagger_delay_ms(mut self, delay: u64) -> Self {
        self.stagger_delay_ms = delay;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::BarGrow {
            duration_ms: self.duration_ms,
            stagger_delay_ms: self.stagger_delay_ms,
            easing: self.easing,
            count: self.count,
        }
    }
}

/// Builder for bar update animation
#[derive(Debug, Clone)]
pub struct BarUpdateBuilder {
    stiffness: f64,
    damping: f64,
    stagger_delay_ms: u64,
    count: usize,
}

impl BarUpdateBuilder {
    pub fn new(count: usize) -> Self {
        let defaults = BarUpdateDefaults::default();
        Self {
            stiffness: defaults.stiffness,
            damping: defaults.damping,
            stagger_delay_ms: defaults.stagger_delay_ms,
            count,
        }
    }

    pub fn stiffness(mut self, stiffness: f64) -> Self {
        self.stiffness = stiffness;
        self
    }

    pub fn damping(mut self, damping: f64) -> Self {
        self.damping = damping;
        self
    }

    pub fn stagger_delay_ms(mut self, delay: u64) -> Self {
        self.stagger_delay_ms = delay;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::BarUpdate {
            spring: Spring::new()
                .stiffness(self.stiffness)
                .damping(self.damping),
            stagger_delay_ms: self.stagger_delay_ms,
            count: self.count,
        }
    }
}

/// Builder for line draw-in animation
#[derive(Debug, Clone)]
pub struct LineDrawInBuilder {
    duration_ms: u64,
    easing: Easing,
    path_length: f64,
}

impl LineDrawInBuilder {
    pub fn new(path_length: f64) -> Self {
        let defaults = LineDrawInDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            path_length,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::LineDrawIn {
            duration_ms: self.duration_ms,
            easing: self.easing,
            path_length: self.path_length,
        }
    }
}

/// Builder for candlestick reveal animation
#[derive(Debug, Clone)]
pub struct CandlestickRevealBuilder {
    wick_duration_ms: u64,
    body_duration_ms: u64,
    stagger_delay_ms: u64,
    wick_easing: Easing,
    body_easing: Easing,
    count: usize,
}

impl CandlestickRevealBuilder {
    pub fn new(count: usize) -> Self {
        let defaults = CandlestickRevealDefaults::default();
        Self {
            wick_duration_ms: defaults.wick_duration_ms,
            body_duration_ms: defaults.body_duration_ms,
            stagger_delay_ms: defaults.stagger_delay_ms,
            wick_easing: defaults.wick_easing,
            body_easing: defaults.body_easing,
            count,
        }
    }

    pub fn wick_duration_ms(mut self, duration: u64) -> Self {
        self.wick_duration_ms = duration;
        self
    }

    pub fn body_duration_ms(mut self, duration: u64) -> Self {
        self.body_duration_ms = duration;
        self
    }

    pub fn stagger_delay_ms(mut self, delay: u64) -> Self {
        self.stagger_delay_ms = delay;
        self
    }

    pub fn wick_easing(mut self, easing: Easing) -> Self {
        self.wick_easing = easing;
        self
    }

    pub fn body_easing(mut self, easing: Easing) -> Self {
        self.body_easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::CandlestickReveal {
            wick_duration_ms: self.wick_duration_ms,
            body_duration_ms: self.body_duration_ms,
            stagger_delay_ms: self.stagger_delay_ms,
            wick_easing: self.wick_easing,
            body_easing: self.body_easing,
            count: self.count,
        }
    }
}

/// Builder for number counter animation
#[derive(Debug, Clone)]
pub struct NumberCounterBuilder {
    duration_ms: u64,
    easing: Easing,
    from: f64,
    to: f64,
    decimals: u8,
}

impl NumberCounterBuilder {
    pub fn new(from: f64, to: f64) -> Self {
        let defaults = NumberCounterDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            from,
            to,
            decimals: defaults.decimals,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn decimals(mut self, decimals: u8) -> Self {
        self.decimals = decimals;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::NumberCounter {
            duration_ms: self.duration_ms,
            easing: self.easing,
            from: self.from,
            to: self.to,
            decimals: self.decimals,
        }
    }
}

/// Builder for data morph animation
#[derive(Debug, Clone)]
pub struct DataMorphBuilder {
    duration_ms: u64,
    easing: Easing,
    data_points: usize,
}

impl DataMorphBuilder {
    pub fn new(data_points: usize) -> Self {
        let defaults = DataMorphDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            data_points,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::DataMorph {
            duration_ms: self.duration_ms,
            easing: self.easing,
            data_points: self.data_points,
        }
    }
}

/// Builder for area fill animation
#[derive(Debug, Clone)]
pub struct AreaFillBuilder {
    line_duration_ms: u64,
    fill_duration_ms: u64,
    fill_delay_ms: u64,
    line_easing: Easing,
    fill_easing: Easing,
    path_length: f64,
}

impl AreaFillBuilder {
    pub fn new(path_length: f64) -> Self {
        let defaults = AreaFillDefaults::default();
        Self {
            line_duration_ms: defaults.line_duration_ms,
            fill_duration_ms: defaults.fill_duration_ms,
            fill_delay_ms: defaults.fill_delay_ms,
            line_easing: defaults.line_easing,
            fill_easing: defaults.fill_easing,
            path_length,
        }
    }

    pub fn line_duration_ms(mut self, duration: u64) -> Self {
        self.line_duration_ms = duration;
        self
    }

    pub fn fill_duration_ms(mut self, duration: u64) -> Self {
        self.fill_duration_ms = duration;
        self
    }

    pub fn fill_delay_ms(mut self, delay: u64) -> Self {
        self.fill_delay_ms = delay;
        self
    }

    pub fn line_easing(mut self, easing: Easing) -> Self {
        self.line_easing = easing;
        self
    }

    pub fn fill_easing(mut self, easing: Easing) -> Self {
        self.fill_easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::AreaFill {
            line_duration_ms: self.line_duration_ms,
            fill_duration_ms: self.fill_duration_ms,
            fill_delay_ms: self.fill_delay_ms,
            line_easing: self.line_easing,
            fill_easing: self.fill_easing,
            path_length: self.path_length,
        }
    }
}

/// Builder for pie slice growth animation
#[derive(Debug, Clone)]
pub struct PieSliceGrowBuilder {
    duration_ms: u64,
    stagger_delay_ms: u64,
    easing: Easing,
    count: usize,
}

impl PieSliceGrowBuilder {
    pub fn new(count: usize) -> Self {
        let defaults = PieSliceGrowDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            stagger_delay_ms: defaults.stagger_delay_ms,
            easing: defaults.easing,
            count,
        }
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.duration_ms = duration;
        self
    }

    pub fn stagger_delay_ms(mut self, delay: u64) -> Self {
        self.stagger_delay_ms = delay;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::PieSliceGrow {
            duration_ms: self.duration_ms,
            stagger_delay_ms: self.stagger_delay_ms,
            easing: self.easing,
            count: self.count,
        }
    }
}

/// Builder for heatmap fade animation
#[derive(Debug, Clone)]
pub struct HeatmapFadeBuilder {
    cell_duration_ms: u64,
    stagger_delay_ms: u64,
    easing: Easing,
    rows: usize,
    cols: usize,
}

impl HeatmapFadeBuilder {
    pub fn new(rows: usize, cols: usize) -> Self {
        let defaults = HeatmapFadeDefaults::default();
        Self {
            cell_duration_ms: defaults.cell_duration_ms,
            stagger_delay_ms: defaults.stagger_delay_ms,
            easing: defaults.easing,
            rows,
            cols,
        }
    }

    pub fn cell_duration_ms(mut self, duration: u64) -> Self {
        self.cell_duration_ms = duration;
        self
    }

    pub fn stagger_delay_ms(mut self, delay: u64) -> Self {
        self.stagger_delay_ms = delay;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::HeatmapFade {
            cell_duration_ms: self.cell_duration_ms,
            stagger_delay_ms: self.stagger_delay_ms,
            easing: self.easing,
            rows: self.rows,
            cols: self.cols,
        }
    }
}

/// Builder for ticker flash animation
#[derive(Debug, Clone)]
pub struct TickerFlashBuilder {
    flash_duration_ms: u64,
    fade_duration_ms: u64,
    easing: Easing,
    direction: TickerDirection,
}

impl TickerFlashBuilder {
    pub fn new(direction: TickerDirection) -> Self {
        let defaults = TickerFlashDefaults::default();
        Self {
            flash_duration_ms: defaults.flash_duration_ms,
            fade_duration_ms: defaults.fade_duration_ms,
            easing: defaults.easing,
            direction,
        }
    }

    pub fn flash_duration_ms(mut self, duration: u64) -> Self {
        self.flash_duration_ms = duration;
        self
    }

    pub fn fade_duration_ms(mut self, duration: u64) -> Self {
        self.fade_duration_ms = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ChartAnimation {
        ChartAnimation::TickerFlash {
            flash_duration_ms: self.flash_duration_ms,
            fade_duration_ms: self.fade_duration_ms,
            easing: self.easing,
            direction: self.direction,
        }
    }
}
