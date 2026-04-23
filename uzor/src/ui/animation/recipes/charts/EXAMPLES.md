# Chart Animation Recipes - Usage Examples

This document demonstrates how to use the chart animation recipes in uzor-animation.

## Quick Start

```rust
use uzor_animation::recipes::charts::*;

// Bar chart with stagger effect
let anim = bar_grow_stagger(10); // 10 bars
let duration = anim.total_duration_ms(); // 1050ms (600ms + 9*50ms stagger)

// Line chart draw-in
let anim = line_draw_in(500.0); // 500 pixels path length
// Duration: 1000ms

// Candlestick cascade
let anim = candlestick_cascade(20); // 20 candles
// Wick draws first, then body, cascading left to right

// Number counter
let anim = number_counter_up(12345.67, 2); // Count to 12345.67 with 2 decimals
// Duration: 1000ms

// Ticker flash on price movement
let anim_green = ticker_flash_green(); // Price went up
let anim_red = ticker_flash_red();     // Price went down
```

## Custom Animations with Builders

```rust
use uzor_animation::recipes::charts::*;
use uzor_animation::easing::Easing;

// Custom bar growth animation
let anim = BarGrowBuilder::new(8)
    .duration_ms(800)
    .stagger_delay_ms(60)
    .easing(Easing::EaseOutQuad)
    .build();

// Custom number counter with exponential easing
let anim = NumberCounterBuilder::new(0.0, 999.99)
    .duration_ms(500)
    .decimals(2)
    .easing(Easing::EaseOutExpo)
    .build();

// Custom candlestick reveal
let anim = CandlestickRevealBuilder::new(50)
    .wick_duration_ms(150)
    .body_duration_ms(250)
    .stagger_delay_ms(25)
    .build();

// Custom area chart fill
let anim = AreaFillBuilder::new(600.0) // path length
    .line_duration_ms(1200)
    .fill_duration_ms(600)
    .fill_delay_ms(800)
    .build();
```

## All Available Presets

### Bar Charts
- `bar_grow_stagger(count)` — Bars grow with cascade effect
- `bar_spring_update(count)` — Spring physics for data updates
- `volume_bars_cascade(count)` — Fast cascade for volume bars

### Line Charts
- `line_draw_in(path_length)` — Line draws left to right
- `sparkline_draw(path_length)` — Faster draw for mini charts

### Candlestick Charts
- `candlestick_cascade(count)` — Wick then body reveal

### Number Displays
- `number_counter_up(to, decimals)` — Count from 0 to value
- `number_counter_update(from, to, decimals)` — Transition between values

### Area Charts
- `area_fill_reveal(path_length)` — Line draws, then area fills

### Pie/Donut Charts
- `pie_slice_grow(count)` — Slices grow with slight bounce

### Heatmaps
- `heatmap_stagger(rows, cols)` — Cells fade from center

### Ticker Flashes
- `ticker_flash_green()` — Price increase flash
- `ticker_flash_red()` — Price decrease flash

### Data Transitions
- `data_crossfade(data_points)` — Smooth dataset transition
- `depth_chart_flow(data_points)` — Order book visualization

## Animation Properties

All animations return a `ChartAnimation` enum that provides:

```rust
// Get total duration in milliseconds
let duration_ms = anim.total_duration_ms();

// Get total duration as std::time::Duration
let duration = anim.total_duration();

// Match on variant to extract parameters
match anim {
    ChartAnimation::BarGrow { duration_ms, stagger_delay_ms, easing, count } => {
        // Use parameters
    },
    _ => {}
}
```

## Research-Based Timings

All presets use timings derived from production animation libraries:

- **Chart.js**: Bar stagger patterns, ease functions
- **D3.js**: Line stroke animations, data transitions
- **TradingView**: Candlestick reveals, ticker flashes
- **GSAP**: Pie chart growth, number counters
- **Framer Motion**: Spring physics parameters

See `research/recipes/08-charts-data.md` for full research documentation.

## Integration with uzor-animation Engine

These recipes are just configuration presets. To actually animate, integrate with:

```rust
use uzor_animation::{Timeline, Easing, Spring, StrokeAnimation, LinearStagger};
use std::time::Duration;

// Example: Bar growth with stagger
let bar_anim = bar_grow_stagger(10);

if let ChartAnimation::BarGrow { duration_ms, stagger_delay_ms, easing, count } = bar_anim {
    // Create stagger pattern
    let stagger = LinearStagger::new(Duration::from_millis(stagger_delay_ms));
    let delays = stagger.delays(count);

    // Animate each bar
    for (i, delay) in delays.iter().enumerate() {
        // Use timeline, tween, or direct engine animation here
        // bar[i].animate().from(0).to(value).duration(duration_ms).easing(easing);
    }
}
```
