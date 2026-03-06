//! Chart animation recipes for data visualization
//!
//! Pre-configured animation patterns for trading terminal charts:
//! - Bar charts (stagger, spring updates)
//! - Line charts (progressive draw-in)
//! - Candlesticks (cascade reveal)
//! - Number counters (price tickers, P&L)
//! - Area charts (line + fill)
//! - Pie/Donut charts (slice growth)
//! - Heatmaps (grid fade-in)
//! - Ticker flashes (price movements)
//!
//! All timings are research-based from Chart.js, D3, TradingView, and GSAP.

pub mod builders;
pub mod defaults;
pub mod presets;
pub mod types;

pub use builders::*;
pub use defaults::*;
pub use presets::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::easing::Easing;

    #[test]
    fn test_bar_grow_stagger_preset() {
        let anim = bar_grow_stagger(10);

        match anim {
            ChartAnimation::BarGrow {
                duration_ms,
                stagger_delay_ms,
                easing,
                count,
            } => {
                assert_eq!(duration_ms, 600);
                assert_eq!(stagger_delay_ms, 50);
                assert_eq!(count, 10);
                assert!(matches!(easing, Easing::EaseOutCubic));
            }
            _ => panic!("Expected BarGrow variant"),
        }
    }

    #[test]
    fn test_bar_spring_update_preset() {
        let anim = bar_spring_update(5);

        match anim {
            ChartAnimation::BarUpdate {
                spring,
                stagger_delay_ms,
                count,
            } => {
                assert_eq!(spring.stiffness, 120.0);
                assert_eq!(spring.damping, 14.0);
                assert_eq!(stagger_delay_ms, 20);
                assert_eq!(count, 5);
            }
            _ => panic!("Expected BarUpdate variant"),
        }
    }

    #[test]
    fn test_line_draw_in_preset() {
        let anim = line_draw_in(500.0);

        match anim {
            ChartAnimation::LineDrawIn {
                duration_ms,
                easing,
                path_length,
            } => {
                assert_eq!(duration_ms, 1000);
                assert_eq!(path_length, 500.0);
                assert!(matches!(easing, Easing::Linear));
            }
            _ => panic!("Expected LineDrawIn variant"),
        }
    }

    #[test]
    fn test_candlestick_cascade_preset() {
        let anim = candlestick_cascade(20);

        match anim {
            ChartAnimation::CandlestickReveal {
                wick_duration_ms,
                body_duration_ms,
                stagger_delay_ms,
                wick_easing,
                body_easing,
                count,
            } => {
                assert_eq!(wick_duration_ms, 200);
                assert_eq!(body_duration_ms, 300);
                assert_eq!(stagger_delay_ms, 30);
                assert_eq!(count, 20);
                assert!(matches!(wick_easing, Easing::EaseOutQuad));
                assert!(matches!(body_easing, Easing::EaseOutCubic));
            }
            _ => panic!("Expected CandlestickReveal variant"),
        }
    }

    #[test]
    fn test_number_counter_up_preset() {
        let anim = number_counter_up(12345.67, 2);

        match anim {
            ChartAnimation::NumberCounter {
                duration_ms,
                easing,
                from,
                to,
                decimals,
            } => {
                assert_eq!(duration_ms, 1000);
                assert_eq!(from, 0.0);
                assert_eq!(to, 12345.67);
                assert_eq!(decimals, 2);
                assert!(matches!(easing, Easing::EaseOutCubic));
            }
            _ => panic!("Expected NumberCounter variant"),
        }
    }

    #[test]
    fn test_number_counter_update_preset() {
        let anim = number_counter_update(100.0, 150.0, 2);

        match anim {
            ChartAnimation::NumberCounter {
                duration_ms,
                from,
                to,
                decimals,
                ..
            } => {
                assert_eq!(duration_ms, 300);
                assert_eq!(from, 100.0);
                assert_eq!(to, 150.0);
                assert_eq!(decimals, 2);
            }
            _ => panic!("Expected NumberCounter variant"),
        }
    }

    #[test]
    fn test_ticker_flash_green() {
        let anim = ticker_flash_green();

        match anim {
            ChartAnimation::TickerFlash {
                flash_duration_ms,
                fade_duration_ms,
                direction,
                ..
            } => {
                assert_eq!(flash_duration_ms, 200);
                assert_eq!(fade_duration_ms, 400);
                assert_eq!(direction, TickerDirection::Up);
            }
            _ => panic!("Expected TickerFlash variant"),
        }
    }

    #[test]
    fn test_ticker_flash_red() {
        let anim = ticker_flash_red();

        match anim {
            ChartAnimation::TickerFlash { direction, .. } => {
                assert_eq!(direction, TickerDirection::Down);
            }
            _ => panic!("Expected TickerFlash variant"),
        }
    }

    #[test]
    fn test_total_duration_bar_grow() {
        let anim = bar_grow_stagger(10);
        let duration = anim.total_duration_ms();

        // 600ms base + (10-1)*50ms stagger = 1050ms
        assert_eq!(duration, 1050);
    }

    #[test]
    fn test_total_duration_candlestick() {
        let anim = candlestick_cascade(5);
        let duration = anim.total_duration_ms();

        // Per candle: max(200, 300) = 300ms
        // Total: 300 + (5-1)*30 = 420ms
        assert_eq!(duration, 420);
    }

    #[test]
    fn test_total_duration_area_fill() {
        let anim = area_fill_reveal(500.0);
        let duration = anim.total_duration_ms();

        // max(1500, 1000 + 800) = 1800ms
        assert_eq!(duration, 1800);
    }

    #[test]
    fn test_bar_grow_builder() {
        let anim = BarGrowBuilder::new(8)
            .duration_ms(800)
            .stagger_delay_ms(60)
            .easing(Easing::EaseOutQuad)
            .build();

        match anim {
            ChartAnimation::BarGrow {
                duration_ms,
                stagger_delay_ms,
                easing,
                count,
            } => {
                assert_eq!(duration_ms, 800);
                assert_eq!(stagger_delay_ms, 60);
                assert_eq!(count, 8);
                assert!(matches!(easing, Easing::EaseOutQuad));
            }
            _ => panic!("Expected BarGrow variant"),
        }
    }

    #[test]
    fn test_number_counter_builder() {
        let anim = NumberCounterBuilder::new(0.0, 999.99)
            .duration_ms(500)
            .decimals(2)
            .easing(Easing::EaseOutExpo)
            .build();

        match anim {
            ChartAnimation::NumberCounter {
                duration_ms,
                from,
                to,
                decimals,
                easing,
            } => {
                assert_eq!(duration_ms, 500);
                assert_eq!(from, 0.0);
                assert_eq!(to, 999.99);
                assert_eq!(decimals, 2);
                assert!(matches!(easing, Easing::EaseOutExpo));
            }
            _ => panic!("Expected NumberCounter variant"),
        }
    }

    #[test]
    fn test_defaults() {
        let bar_defaults = BarGrowDefaults::default();
        assert_eq!(bar_defaults.duration_ms, 600);
        assert_eq!(bar_defaults.stagger_delay_ms, 50);

        let counter_defaults = NumberCounterDefaults::default();
        assert_eq!(counter_defaults.duration_ms, 1000);
        assert_eq!(counter_defaults.decimals, 2);

        let ticker_defaults = TickerFlashDefaults::default();
        assert_eq!(ticker_defaults.flash_duration_ms, 200);
        assert_eq!(ticker_defaults.fade_duration_ms, 400);
    }

    #[test]
    fn test_heatmap_total_duration() {
        let anim = heatmap_stagger(5, 10);
        let duration = anim.total_duration_ms();

        // 50 cells total
        // 300 + (50-1)*20 = 1280ms
        assert_eq!(duration, 1280);
    }

    #[test]
    fn test_pie_slice_grow_preset() {
        let anim = pie_slice_grow(6);

        match anim {
            ChartAnimation::PieSliceGrow {
                duration_ms,
                stagger_delay_ms,
                count,
                ..
            } => {
                assert_eq!(duration_ms, 1000);
                assert_eq!(stagger_delay_ms, 100);
                assert_eq!(count, 6);
            }
            _ => panic!("Expected PieSliceGrow variant"),
        }
    }
}
