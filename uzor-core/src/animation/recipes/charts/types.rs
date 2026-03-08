//! Chart animation type definitions

use crate::animation::easing::Easing;
use crate::animation::spring::Spring;
use std::time::Duration;

/// Chart animation variants
///
/// Each variant encapsulates parameters for a specific chart animation pattern.
#[derive(Debug, Clone)]
pub enum ChartAnimation {
    /// Bars grow from zero to value with stagger
    BarGrow {
        duration_ms: u64,
        stagger_delay_ms: u64,
        easing: Easing,
        count: usize,
    },

    /// Bars transition smoothly to new values (with spring)
    BarUpdate {
        spring: Spring,
        stagger_delay_ms: u64,
        count: usize,
    },

    /// Line chart draws in left-to-right (stroke animation)
    LineDrawIn {
        duration_ms: u64,
        easing: Easing,
        path_length: f64,
    },

    /// Candlesticks appear with stagger, body grows from open
    CandlestickReveal {
        wick_duration_ms: u64,
        body_duration_ms: u64,
        stagger_delay_ms: u64,
        wick_easing: Easing,
        body_easing: Easing,
        count: usize,
    },

    /// Animated number counting (for price displays, P&L)
    NumberCounter {
        duration_ms: u64,
        easing: Easing,
        from: f64,
        to: f64,
        decimals: u8,
    },

    /// Smooth transition between datasets
    DataMorph {
        duration_ms: u64,
        easing: Easing,
        data_points: usize,
    },

    /// Area under line fills in (opacity + clip)
    AreaFill {
        line_duration_ms: u64,
        fill_duration_ms: u64,
        fill_delay_ms: u64,
        line_easing: Easing,
        fill_easing: Easing,
        path_length: f64,
    },

    /// Pie/donut slices grow from 0 degrees
    PieSliceGrow {
        duration_ms: u64,
        stagger_delay_ms: u64,
        easing: Easing,
        count: usize,
    },

    /// Heatmap cells fade in with color transition
    HeatmapFade {
        cell_duration_ms: u64,
        stagger_delay_ms: u64,
        easing: Easing,
        rows: usize,
        cols: usize,
    },

    /// Price tick flash (green up, red down) + fade
    TickerFlash {
        flash_duration_ms: u64,
        fade_duration_ms: u64,
        easing: Easing,
        direction: TickerDirection,
    },
}

/// Price movement direction for ticker flash
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickerDirection {
    Up,
    Down,
}

impl ChartAnimation {
    /// Get total duration of animation in milliseconds
    pub fn total_duration_ms(&self) -> u64 {
        match self {
            ChartAnimation::BarGrow {
                duration_ms,
                stagger_delay_ms,
                count,
                ..
            } => duration_ms + stagger_delay_ms * (*count as u64).saturating_sub(1),

            ChartAnimation::BarUpdate {
                spring,
                stagger_delay_ms,
                count,
            } => {
                let spring_duration = (spring.estimated_duration() * 1000.0) as u64;
                spring_duration + stagger_delay_ms * (*count as u64).saturating_sub(1)
            }

            ChartAnimation::LineDrawIn { duration_ms, .. } => *duration_ms,

            ChartAnimation::CandlestickReveal {
                wick_duration_ms,
                body_duration_ms,
                stagger_delay_ms,
                count,
                ..
            } => {
                let per_candle = wick_duration_ms.max(body_duration_ms);
                per_candle + stagger_delay_ms * (*count as u64).saturating_sub(1)
            }

            ChartAnimation::NumberCounter { duration_ms, .. } => *duration_ms,

            ChartAnimation::DataMorph { duration_ms, .. } => *duration_ms,

            ChartAnimation::AreaFill {
                line_duration_ms,
                fill_duration_ms,
                fill_delay_ms,
                ..
            } => *line_duration_ms.max(&(fill_delay_ms + fill_duration_ms)),

            ChartAnimation::PieSliceGrow {
                duration_ms,
                stagger_delay_ms,
                count,
                ..
            } => duration_ms + stagger_delay_ms * (*count as u64).saturating_sub(1),

            ChartAnimation::HeatmapFade {
                cell_duration_ms,
                stagger_delay_ms,
                rows,
                cols,
                ..
            } => {
                let total_cells = rows * cols;
                cell_duration_ms + stagger_delay_ms * (total_cells as u64).saturating_sub(1)
            }

            ChartAnimation::TickerFlash {
                flash_duration_ms,
                fade_duration_ms,
                ..
            } => flash_duration_ms + fade_duration_ms,
        }
    }

    /// Convert duration to std::time::Duration
    pub fn total_duration(&self) -> Duration {
        Duration::from_millis(self.total_duration_ms())
    }
}
