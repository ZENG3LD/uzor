# Loading Animation Recipes - Usage Examples

This module provides 20+ pre-configured loading animation patterns based on Material Design, iOS, SpinKit, and modern web best practices.

## Quick Start

```rust
use uzor_animation::recipes::loading::*;

// Use a preset
let spinner = material_circular();
let dots = three_bounce_dots();
let shimmer = shimmer();

// Customize with builder
let custom_spinner = SpinnerBuilder::new()
    .duration_ms(3000)
    .easing(Easing::EaseInOutQuad)
    .build();
```

## Available Presets

### Material Design

#### Material Circular Spinner
```rust
let anim = material_circular();
// Rotating arc with dynamic length
// Duration: 2000ms
// Infinite loop
```

#### Material Linear Progress
```rust
let anim = material_linear();
// Sliding bar with grow/shrink
// Duration: 2000ms
// Infinite loop
```

### Classic Patterns

#### Three Bouncing Dots
```rust
let anim = three_bounce_dots();
// 3 dots scaling in sequence
// Duration: 1200ms per cycle
// Stagger: 160ms between dots
```

#### Wave Bars (Equalizer Style)
```rust
let anim = wave_bars();
// 5 bars stretching up/down
// Duration: 1200ms
// Stagger: 100ms between bars
```

#### Pulse Ring
```rust
let anim = pulse_ring();
// Expanding + fading ring
// Duration: 1500ms
// Scale: 0.0 → 1.0, Opacity: 1.0 → 0.0
```

### Skeleton Loading

#### Shimmer Gradient
```rust
let anim = shimmer();
// Gradient sweep left to right
// Duration: 1500ms
// Linear easing for constant speed
```

#### Skeleton Pulse
```rust
let anim = skeleton_pulse();
// Subtle opacity pulsing
// Duration: 2000ms
```

### iOS-Style

#### Fading Dots Circle
```rust
let anim = fading_dots_circle();
// 8 dots in circle, fading in sequence
// Duration: 1200ms
// Stagger: 100ms
```

#### iOS Spinner
```rust
let anim = ios_spinner();
// 12-segment spinner
// Classic iOS loading indicator
```

### Progress Indicators (Determinate)

#### Progress Ring
```rust
let anim = progress_ring_determinate();
// Circular SVG stroke progress
// Duration: 350ms (per update)
// Radius: 52px, Stroke: 8px
```

#### Progress Bar
```rust
let anim = progress_bar_determinate();
// Linear horizontal bar
// Duration: 400ms (per update)
```

#### Progress with Shimmer
```rust
let anim = progress_with_shimmer();
// Determinate bar + shimmer overlay
// Shows both progress and activity
```

### Advanced Patterns

#### Ripple Rings
```rust
let anim = ripple_rings();
// 4 rings expanding outward
// Duration: 4000ms
// Stagger: 1000ms between rings
```

#### Wave Dots (Vertical)
```rust
let anim = wave_dots_vertical();
// 4 dots bouncing up/down with opacity
// Duration: 1400ms
// Stagger: 200ms
```

#### SVG Path Drawing
```rust
let path_length = 500.0; // Get from path.getTotalLength()
let anim = path_draw(path_length);
// Draws SVG path from start to finish
// Duration: 3000ms + 1000ms hold
```

#### Bouncing Ball
```rust
let anim = bouncing_ball();
// Single element vertical bounce
// Duration: 600ms
// Bounce distance: 16px
```

### SpinKit-Inspired

#### SpinKit Chase
```rust
let anim = spinkit_chase();
// 6 dots pulsing in circle
// Duration: 2000ms
```

#### Double Bounce
```rust
let anim = double_bounce();
// 2 overlapping circles
// Duration: 2000ms with 1000ms offset
```

#### Basic Spinner
```rust
let anim = basic_spinner();
// Simple constant rotation
// Duration: 2000ms
```

## Using Builders

All animation types have corresponding builder patterns for customization:

### Spinner Builder
```rust
let spinner = SpinnerBuilder::new()
    .duration_ms(3000)
    .easing(Easing::EaseInOutQuad)
    .build();
```

### Pulse Dots Builder
```rust
let dots = PulseDotsBuilder::new()
    .count(5)                    // Number of dots
    .stagger_delay_ms(150)       // Delay between dots
    .scale_range(0.2, 1.0)       // Min/max scale
    .duration_ms(1000)
    .easing(Easing::EaseOutBounce)
    .build();
```

### Bar Wave Builder
```rust
let bars = BarWaveBuilder::new()
    .count(7)
    .stagger_delay_ms(80)
    .scale_range(0.3, 1.0)
    .build();
```

### Progress Ring Builder
```rust
let ring = ProgressRingBuilder::new()
    .radius(60.0)
    .stroke_width(10.0)
    .duration_ms(300)
    .build();
```

### Shimmer Builder
```rust
let shimmer = ShimmerBuilder::new()
    .duration_ms(2000)
    .gradient_range(150.0, -150.0)
    .easing(Easing::Linear)
    .build();
```

### Pulse Ring Builder
```rust
let pulse = PulseRingBuilder::new()
    .scale_range(0.5, 2.0)
    .opacity_range(0.8, 0.0)
    .duration_ms(1200)
    .build();
```

## Animation Properties

### Infinite vs Determinate
```rust
// Check if animation loops infinitely
if anim.is_infinite() {
    // Spinner, shimmer, etc.
} else {
    // Progress indicators
}
```

### Duration
```rust
let duration_ms = anim.duration_ms();
let duration = anim.duration(); // std::time::Duration
```

### Element Count
```rust
// For multi-element animations (dots, bars, etc.)
let count = anim.element_count();
```

## Integration with uzor-animation Engine

These recipes are designed to work with the uzor-animation engine modules:

### With Timeline
```rust
use uzor_animation::{Timeline, Position};

let dots = three_bounce_dots();
let mut timeline = Timeline::new();

// Add animation to timeline
// (Implementation depends on animation type)
```

### With Stagger
```rust
use uzor_animation::stagger::LinearStagger;
use std::time::Duration;

let stagger = LinearStagger::new(Duration::from_millis(160));
let delays = stagger.delays(3); // For 3 dots
```

### With Stroke (for SVG animations)
```rust
use uzor_animation::StrokeAnimation;

let path_len = 500.0;
let stroke = StrokeAnimation::draw_in(path_len);
```

## Default Constants

Access default values directly:

```rust
use uzor_animation::recipes::loading::*;

// Spinner defaults
SpinnerDefaults::DURATION_MS;       // 2000
SpinnerDefaults::EASING;            // Linear

// Pulse dots defaults
PulseDotsDefaults::COUNT;           // 3
PulseDotsDefaults::STAGGER_DELAY_MS; // 160
PulseDotsDefaults::SCALE_FROM;      // 0.0
PulseDotsDefaults::SCALE_TO;        // 1.0

// And many more...
```

## Research References

These animations are based on research from:

- **Material Design 3 Progress Indicators**: https://m3.material.io/components/progress-indicators/specs
- **SpinKit**: https://tobiasahlin.com/spinkit/
- **Loading Shimmer Patterns**: DEV Community articles
- **SVG Stroke Techniques**: CSS-Tricks
- **iOS Loading Indicators**: Apple HIG

See `research/recipes/07-loading-progress.md` for full research documentation.
