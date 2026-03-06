//! Chart animation presets
//!
//! Pre-configured animations for common chart patterns in trading terminals.
//! All timings are based on research from Chart.js, D3, TradingView, and GSAP.

use super::types::{ChartAnimation, TickerDirection};
use crate::easing::Easing;
use crate::spring::Spring;

/// Bar chart staggered reveal — bars grow from bottom with cascading effect
///
/// **Timing**: 600ms per bar, 50ms stagger
/// **Easing**: EaseOutCubic for smooth deceleration
/// **Use case**: Initial chart load, dramatic data reveal
pub fn bar_grow_stagger(count: usize) -> ChartAnimation {
    ChartAnimation::BarGrow {
        duration_ms: 600,
        stagger_delay_ms: 50,
        easing: Easing::EaseOutCubic,
        count,
    }
}

/// Bar chart spring update — bars bounce to new values
///
/// **Timing**: Spring physics (stiffness=120, damping=14), 20ms stagger
/// **Feel**: Responsive, natural bounce on data updates
/// **Use case**: Real-time data updates, live charts
pub fn bar_spring_update(count: usize) -> ChartAnimation {
    ChartAnimation::BarUpdate {
        spring: Spring::new().stiffness(120.0).damping(14.0),
        stagger_delay_ms: 20,
        count,
    }
}

/// Line chart progressive draw-in — line draws left to right
///
/// **Timing**: 1000ms constant speed
/// **Easing**: Linear for consistent drawing speed
/// **Use case**: Line chart entrance, trend reveal
pub fn line_draw_in(path_length: f64) -> ChartAnimation {
    ChartAnimation::LineDrawIn {
        duration_ms: 1000,
        easing: Easing::Linear,
        path_length,
    }
}

/// Candlestick chart cascade reveal — wick then body, left to right
///
/// **Timing**: Wick 200ms, Body 300ms (100ms overlap), 30ms stagger
/// **Easing**: EaseOutQuad for wick, EaseOutCubic for body
/// **Use case**: Candlestick chart initial display, OHLC data reveal
pub fn candlestick_cascade(count: usize) -> ChartAnimation {
    ChartAnimation::CandlestickReveal {
        wick_duration_ms: 200,
        body_duration_ms: 300,
        stagger_delay_ms: 30,
        wick_easing: Easing::EaseOutQuad,
        body_easing: Easing::EaseOutCubic,
        count,
    }
}

/// Number counter up from zero — digits roll to target value
///
/// **Timing**: 1000ms
/// **Easing**: EaseOutCubic for natural counting deceleration
/// **Use case**: Price reveals, volume counters, P&L displays
pub fn number_counter_up(to: f64, decimals: u8) -> ChartAnimation {
    ChartAnimation::NumberCounter {
        duration_ms: 1000,
        easing: Easing::EaseOutCubic,
        from: 0.0,
        to,
        decimals,
    }
}

/// Number counter update — smooth transition between values
///
/// **Timing**: 300ms (fast for responsive feel)
/// **Easing**: EaseOutCubic
/// **Use case**: Live price updates, ticker displays
pub fn number_counter_update(from: f64, to: f64, decimals: u8) -> ChartAnimation {
    ChartAnimation::NumberCounter {
        duration_ms: 300,
        easing: Easing::EaseOutCubic,
        from,
        to,
        decimals,
    }
}

/// Data crossfade — smooth morph between two datasets
///
/// **Timing**: 500ms
/// **Easing**: EaseInOutCubic for symmetrical transition
/// **Use case**: Chart type switching, timeframe changes
pub fn data_crossfade(data_points: usize) -> ChartAnimation {
    ChartAnimation::DataMorph {
        duration_ms: 500,
        easing: Easing::EaseInOutCubic,
        data_points,
    }
}

/// Area chart fill reveal — line draws, then area fades in
///
/// **Timing**: Line 1500ms, Area 800ms (starts at 1000ms with overlap)
/// **Easing**: Linear for line, EaseInOutQuad for area fade
/// **Use case**: Area chart entrance, gradient emphasis
pub fn area_fill_reveal(path_length: f64) -> ChartAnimation {
    ChartAnimation::AreaFill {
        line_duration_ms: 1500,
        fill_duration_ms: 800,
        fill_delay_ms: 1000,
        line_easing: Easing::Linear,
        fill_easing: Easing::EaseInOutQuad,
        path_length,
    }
}

/// Pie chart slice growth — slices grow clockwise from center
///
/// **Timing**: 1000ms per slice, 100ms stagger
/// **Easing**: EaseOutBack for slight overshoot (bouncy feel)
/// **Use case**: Pie/donut chart entrance, portfolio breakdowns
pub fn pie_slice_grow(count: usize) -> ChartAnimation {
    ChartAnimation::PieSliceGrow {
        duration_ms: 1000,
        stagger_delay_ms: 100,
        easing: Easing::EaseOutBack,
        count,
    }
}

/// Heatmap stagger fade — cells fade in from center outward
///
/// **Timing**: 300ms per cell, 20ms stagger (fast for large grids)
/// **Easing**: EaseOutQuad
/// **Use case**: Correlation matrices, depth maps, volatility grids
pub fn heatmap_stagger(rows: usize, cols: usize) -> ChartAnimation {
    ChartAnimation::HeatmapFade {
        cell_duration_ms: 300,
        stagger_delay_ms: 20,
        easing: Easing::EaseOutQuad,
        rows,
        cols,
    }
}

/// Ticker flash green — price increase flash animation
///
/// **Timing**: 200ms flash, 400ms fade
/// **Easing**: EaseOutCubic
/// **Use case**: Price tick up, positive price movement
pub fn ticker_flash_green() -> ChartAnimation {
    ChartAnimation::TickerFlash {
        flash_duration_ms: 200,
        fade_duration_ms: 400,
        easing: Easing::EaseOutCubic,
        direction: TickerDirection::Up,
    }
}

/// Ticker flash red — price decrease flash animation
///
/// **Timing**: 200ms flash, 400ms fade
/// **Easing**: EaseOutCubic
/// **Use case**: Price tick down, negative price movement
pub fn ticker_flash_red() -> ChartAnimation {
    ChartAnimation::TickerFlash {
        flash_duration_ms: 200,
        fade_duration_ms: 400,
        easing: Easing::EaseOutCubic,
        direction: TickerDirection::Down,
    }
}

/// Volume bars cascade — bottom-up growth with fast stagger
///
/// **Timing**: 400ms per bar, 20ms stagger
/// **Easing**: EaseOutQuad
/// **Use case**: Volume bars below price chart, synchronized with candlesticks
pub fn volume_bars_cascade(count: usize) -> ChartAnimation {
    ChartAnimation::BarGrow {
        duration_ms: 400,
        stagger_delay_ms: 20,
        easing: Easing::EaseOutQuad,
        count,
    }
}

/// Depth chart flow — continuous flowing animation for order book
///
/// **Timing**: 800ms smooth transition
/// **Easing**: EaseInOutCubic
/// **Use case**: Order book depth visualization, bid/ask areas
pub fn depth_chart_flow(data_points: usize) -> ChartAnimation {
    ChartAnimation::DataMorph {
        duration_ms: 800,
        easing: Easing::EaseInOutCubic,
        data_points,
    }
}

/// Sparkline draw — fast stroke animation for inline charts
///
/// **Timing**: 500ms (faster than full line charts)
/// **Easing**: Linear
/// **Use case**: Small inline trend indicators, dashboard widgets
pub fn sparkline_draw(path_length: f64) -> ChartAnimation {
    ChartAnimation::LineDrawIn {
        duration_ms: 500,
        easing: Easing::Linear,
        path_length,
    }
}
