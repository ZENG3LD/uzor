# Scroll Animation Recipes

Scroll-driven and parallax animation presets for uzor-animation engine.

## Files Created

1. **mod.rs** (91 lines) - Module exports and documentation
2. **types.rs** (241 lines) - ScrollAnimation enum with 8 variants
3. **presets.rs** (303 lines) - 12 ready-to-use preset functions
4. **defaults.rs** (240 lines) - Default parameter structs
5. **builders.rs** (589 lines) - 8 builder structs with fluent API

**Total:** 1,464 lines

## Animation Variants

| Variant | Description |
|---------|-------------|
| ProgressBar | Horizontal/vertical/circular progress tracking scroll |
| ParallaxLayers | Multi-layer depth effect with different speeds |
| FadeOnScroll | Opacity transitions based on scroll position |
| RevealOnEnter | Slide + fade when element enters viewport |
| StickyHeader | Header shrinks/transforms on scroll |
| HorizontalScroll | Horizontal movement driven by vertical scroll |
| NumberCounter | Count up from 0 when entering viewport |
| ColorShift | Background color transitions through gradient |

## Preset Functions (12 total)

- `progress_bar_horizontal()` - 0-100% linear bar
- `progress_ring()` - Circular SVG progress
- `parallax_hero()` - 3-layer depth (0.3x, 0.6x, 1.0x)
- `fade_in_on_enter()` - Opacity 0→1 on entry 0-30%
- `slide_up_on_enter()` - Slide + fade combo
- `reveal_from_left()` - Horizontal slide-in
- `sticky_shrink_header()` - Height 80→48px
- `horizontal_pin_scroll()` - Pinned horizontal gallery
- `number_counter()` - Count 0→target over 2s
- `color_shift_sections()` - 4-stop gradient transition
- `scale_on_scroll()` - Scale 0.8→1.0
- `parallax_text()` - Text layers at different speeds

## Usage

### Using Presets
```rust
use uzor_animation::recipes::scroll::presets::*;

let progress = progress_bar_horizontal();
let parallax = parallax_hero();
```

### Using Builders
```rust
use uzor_animation::recipes::scroll::builders::*;

let counter = NumberCounterBuilder::new()
    .from(0.0)
    .to(1500.0)
    .threshold(0.6)
    .build();
```

## Research Sources

- GSAP ScrollTrigger demos
- CSS Scroll-Driven Animations spec (W3C)
- Locomotive Scroll library
- Apple product page animations
- Framer Motion scroll patterns
- Motion.dev examples

See `/research/recipes/06-scroll-parallax.md` for full research details.
