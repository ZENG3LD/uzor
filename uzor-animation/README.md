# uzor-animation

Premium animation engine for the uzor UI framework. Zero dependencies. 120fps target.

## Status

**[ENGINE COMPLETE]** — All 11 modules implemented, AnimationCoordinator integrated, 308 tests passing.

See the `research/` directory for deep technical dives into each subsystem.

## Stats

- **11 engine modules** + AnimationCoordinator
- **~5,000 lines** of engine code
- **308 tests** (298 unit + 11 doc-tests), all passing
- **0 external dependencies** (pure Rust, no_std compatible design)

---

## Why Not Existing Crates?

We're building this from scratch because the Rust ecosystem doesn't have what we need:

- **keyframe** (last update: 3+ years ago) — Abandoned, unmaintained
- **No spring physics** — Nobody has ported analytical spring solvers to Rust
- **No timeline orchestration** — No GSAP-style sequencing with position parameters
- **No grid stagger** — AnimeJS-style grid-aware delay distribution doesn't exist

We're not reinventing the wheel. We're bringing the wheel to Rust for the first time.

**Goal:** Framer Motion / GSAP quality, but in Rust, for GPU-rendered UIs.

---

## Architecture

uzor-animation is built from 11 independent, composable modules plus an integration coordinator:

### Base Modules (4)

#### 1. Easing (~750 lines)

**30 Penner functions + cubic-bezier solver + steps() + all CSS constants**

Robert Penner's classic equations (BSD licensed) covering all standard easing patterns:
- **Polynomial:** quad, cubic, quart, quint
- **Trigonometric:** sine, circular
- **Exponential:** expo
- **Special:** elastic (spring-like oscillation), bounce (physics simulation), back (anticipation/overshoot)

**Cubic-bezier solver** replicates browser behavior:
- Newton-Raphson for speed (4 iterations typical)
- Bisection fallback for robustness
- Precomputed sample table for initial guess
- Target: < 100ns per evaluation

**Performance targets:**
- Simple easing (cubic): < 5ns
- Complex easing (elastic): < 100ns
- Bezier solve: < 100ns
- SIMD batch (8× cubic): < 20ns total

#### 2. Spring (~480 lines)

**Real spring physics with analytical solutions**

Not Euler integration. Not RK4. **Closed-form mathematical solution** to the damped harmonic oscillator differential equation.

Three implementations for three damping regimes:
- **Under-damped** (ζ < 1): Bouncy, oscillates
- **Critically damped** (ζ = 1): No overshoot, fastest settling
- **Over-damped** (ζ > 1): Sluggish, no oscillation

**Parameters:**
```rust
Spring::new()
    .stiffness(100.0)   // Rigidity (higher = snappier)
    .damping(10.0)      // Friction (higher = less bouncy)
    .mass(1.0)          // Inertia (higher = more momentum)
```

**Why analytical?**
- Perfect accuracy, no drift or energy loss
- Frame-rate independent (works at any Hz)
- Pure function of time: `position = f(t)`, `velocity = g(t)`
- Resumable from any state
- Can precompute keyframes for GPU upload

Stolen from **wobble** (which reverse-engineered Apple's CASpringAnimation). Wobble's author investigated QuartzCore.framework and discovered Apple uses closed-form solutions, not numerical integration.

**Performance target:** Single spring evaluation < 50ns

#### 3. Timeline (~400 lines)

**GSAP-style sequencing with position parameter**

The secret sauce: animations don't just play sequentially, they can overlap, start at labels, and nest recursively.

**Position parameter examples:**
```rust
timeline
    .add(fade_in, "0ms")           // Absolute time
    .add(slide_up, "+=100ms")      // After previous ends
    .add(scale_up, "-=50ms")       // Overlap by 50ms
    .add_label("scene1")           // Named time marker
    .add(rotate, "scene1")         // At label
    .add(bounce, "scene1+=200ms")  // 200ms after label
```

**Core concept:** Every animation has a `start_time` on the timeline. The position parameter is syntactic sugar for calculating that start time.

**Nested timelines:**
```rust
let intro = Timeline::new()
    .add(logo_appear, "0ms")
    .add(tagline_fade, "+=200ms");

let master = Timeline::new()
    .add_timeline(intro, "0ms")
    .add(button_pop, "+=500ms");
```

Inner timelines maintain their own playback state but inherit timing from parent.

**Playback control:**
- `.play()`, `.pause()`, `.seek(time)`
- `.progress(0.0..1.0)` — Scrub by percentage
- `.time_scale(2.0)` — Speed multiplier

Inspired by GSAP's timeline API, simplified to essential features. No percentage positioning (`<50%`) in v1 — adds complexity for minimal gain.

#### 4. Stagger (~490 lines)

**Grid-aware delay distribution with from-center/from-edge patterns**

Sequential stagger is easy: each element delays by `n × base_delay`. Grid stagger is interesting: delay based on **distance in 2D grid space**.

**From center:**
```rust
Stagger::grid(5, 14)  // 5 rows, 14 cols
    .delay(50ms)
    .from(StaggerOrigin::Center)
    .metric(DistanceMetric::Euclidean)
```

For each element at `(col, row)`:
```
distance = sqrt((col - centerCol)² + (row - centerRow)²)
delay = distance × 50ms
```

**Creates circular propagation waves** from the center outward.

**Distance metrics:**
- **Euclidean:** `sqrt(dx² + dy²)` — Circular waves
- **Manhattan:** `|dx| + |dy|` — Diamond-shaped waves
- **Chebyshev:** `max(|dx|, |dy|)` — Square waves

**From edge:**
Invert the distance calculation: elements on perimeter animate first, center animates last.

**Easing on stagger:**
Apply easing to normalized distance before multiplying by delay:
```rust
normalized = distance / max_distance
eased = easing(normalized)  // 0..1 → 0..1
delay = eased × total_time
```

Non-linear propagation: slow-then-fast or fast-then-slow waves.

Stolen from **AnimeJS** (grid stagger with from parameter) and **GSAP** (distributeByPosition for arbitrary layouts).

### Tier 1: Extended Physics (4)

#### 5. Decay (~440 lines)

**Exponential inertia/friction — iOS flick scroll model**

Models momentum-based motion with exponential decay:
- `evaluate(t) → (position, velocity)`
- Naturally slows down over time
- **Bounded mode:** Snap back with spring when exceeding bounds
- Perfect for flick scrolling, swipe gestures, drag-release

Based on iOS scrolling physics and Rebound library.

#### 6. Color (~500 lines)

**OKLCH perceptual interpolation**

Perceptually uniform color space for smooth gradients:
- Full sRGB ↔ Oklab ↔ OKLCH conversion chain
- Gamut mapping to handle out-of-sRGB colors
- No muddy browns in blue→yellow transitions
- Hue interpolation handles wraparound correctly

Based on Björn Ottosson's Oklab color space (2020).

#### 7. Stroke (~390 lines)

**SVG line drawing animation**

Animate paths appearing/disappearing:
- Path length computation (bezier, arc, line segments)
- `stroke-dasharray` + `stroke-dashoffset` animation
- Works with complex SVG paths

The technique behind icon reveal animations.

#### 8. Blend (~430 lines)

**CompositeMode, AnimationLayer, InterruptionStrategy**

Layer multiple animations on the same property:
- **Replace:** New animation replaces old
- **Add:** Accumulate deltas
- **Accumulate:** Sum values
- **AnimationTransition:** Smooth crossfade between animations
- **AnimationSlot:** Named animation groups with interruption handling

Based on Web Animations API composite modes.

### Tier 2: Advanced Motion (3)

#### 9. Path (~575 lines)

**MotionPath with arc-length parameterization**

Animate elements along curved paths at constant speed:
- Cubic/quadratic bezier support
- Arc-length reparameterization for uniform motion
- Returns `PathSample { position, tangent, angle }`
- Auto-rotation to follow path direction

The technique behind curved motion paths in design tools.

#### 10. Layers (~420 lines)

**ManagedLayer with weight transitions, LayerStack with additive/override blending**

Multi-layer animation system:
- Named layers with priority
- Weight-based blending between layers
- Additive animations (e.g., breathing + walking)
- Override animations (e.g., hit reaction interrupts idle)

Based on animation blending in game engines (Unity, Unreal).

#### 11. Scroll

**ScrollTimeline, ViewTimeline, ParallaxLayer**

CSS Scroll-Driven Animations ported to Rust:
- Animate properties based on scroll position
- View-based triggers (enter/exit viewport)
- Parallax scrolling with configurable rates
- Works with any scrollable container

Based on CSS Scroll-Driven Animations specification.

### Integration

#### AnimationCoordinator (in uzor-core)

Bridges all 11 modules with the render loop:
- Supports Tween, Spring, and Decay drivers
- Auto-ticks in `Context.begin_frame()`
- Widget-scoped property animations
- Lifecycle management (start, update, complete, remove)

---

## Core Trait: Animatable

Animations work over any type that implements `Animatable`:

```rust
pub trait Animatable: Copy + Clone + Send + Sync + 'static {
    fn lerp(from: &Self, to: &Self, t: f32) -> Self;

    fn distance(from: &Self, to: &Self) -> f32 {
        0.0  // Optional: for velocity calculations
    }

    fn add_delta(&self, delta: &Self) -> Self {
        *self  // Optional: for velocity-based animations
    }
}
```

**Bounds explanation:**
- `Copy + Clone`: Efficient passing, no lifetime wrangling
- `Send + Sync`: Thread-safe for parallel animation
- `'static`: No borrowed references, can store in coordinator

**Built-in implementations:**
- Primitives: `f32`, `f64`, `i32`, `u32`
- Math types: `Vec2`, `Vec3`, `Vec4`, `Quaternion`
- Graphics types: `Color`, `Rect`

**User types:**
```rust
#[derive(Copy, Clone, Debug)]
struct BorderRadius {
    top_left: f32,
    top_right: f32,
    bottom_right: f32,
    bottom_left: f32,
}

impl Animatable for BorderRadius {
    fn lerp(from: &Self, to: &Self, t: f32) -> Self {
        BorderRadius {
            top_left: from.top_left + (to.top_left - from.top_left) * t,
            top_right: from.top_right + (to.top_right - from.top_right) * t,
            bottom_right: from.bottom_right + (to.bottom_right - from.bottom_right) * t,
            bottom_left: from.bottom_left + (to.bottom_left - from.bottom_left) * t,
        }
    }
}
```

Future: `#[derive(Animatable)]` macro to auto-generate field-wise lerp.

---

## API Design (Aspirational)

What the API **will** look like when implementation is complete:

### Simple Tween

```rust
use uzor_animation::prelude::*;

let anim = Tween::new(0.0, 1.0)
    .duration(Duration::from_millis(300))
    .easing(Easing::EaseOutCubic)
    .on_update(|value| {
        widget.set_opacity(value);
    });

coordinator.add(anim);
```

### Spring Animation

```rust
let spring = Spring::new(0.0, 1.0)
    .stiffness(100.0)
    .damping(10.0)
    .mass(1.0)
    .on_update(|value| {
        widget.set_scale(value);
    });

coordinator.add(spring);
```

Or duration-based (auto-calculates physics parameters):

```rust
let spring = Spring::with_duration(Duration::from_millis(500))
    .bounce(0.3)  // 0.0 = no overshoot, 1.0 = maximum bounce
    .target(100.0)
    .on_update(|value| {
        widget.set_y(value);
    });
```

### Timeline Sequencing

```rust
let timeline = Timeline::new()
    .add(button_fade, "0ms")
    .add(panel_slide, "+=100ms")      // 100ms after button_fade ends
    .add(content_appear, "-=50ms")    // 50ms before panel_slide ends (overlap)
    .add_label("complete")
    .add(success_icon, "complete");

timeline.play();
coordinator.add_timeline(timeline);
```

### Grid Stagger

```rust
let buttons: Vec<Widget> = get_button_grid();

let stagger = Stagger::grid(4, 6)  // 4 rows, 6 columns
    .delay(Duration::from_millis(50))
    .from(StaggerOrigin::Center)
    .metric(DistanceMetric::Euclidean)
    .easing(Easing::EaseOutQuad);

for (i, button) in buttons.iter().enumerate() {
    let delay = stagger.delay_for_index(i);

    let anim = Tween::new(0.0, 1.0)
        .duration(Duration::from_millis(300))
        .delay(delay)
        .easing(Easing::EaseOutCubic)
        .on_update(move |scale| {
            button.set_scale(scale);
        });

    coordinator.add(anim);
}
```

### Bezier Easing

```rust
// CSS cubic-bezier(0.42, 0, 0.58, 1) — ease-in-out
let easing = CubicBezier::new(0.42, 0.0, 0.58, 1.0);

let anim = Tween::new(start_color, end_color)
    .duration(Duration::from_millis(400))
    .easing(easing)
    .on_update(|color| {
        widget.set_background_color(color);
    });
```

---

## Integration with uzor

### AnimationCoordinator

Central hub that ticks animations and manages lifecycle:

```rust
pub struct AnimationCoordinator {
    animations: Vec<ActiveAnimation>,
    timelines: Vec<Timeline>,
    springs: Vec<SpringAnimation>,
}

impl AnimationCoordinator {
    pub fn tick(&mut self, delta_time: f32) {
        // Update all active animations
        // Remove completed ones
        // Call on_update callbacks
    }

    pub fn has_active_animations(&self) -> bool {
        !self.animations.is_empty() || !self.timelines.is_empty()
    }
}
```

### Render Loop Integration

```rust
const ANIMATION_DT: f32 = 1.0 / 120.0;  // 120 Hz animation tick

let mut coordinator = AnimationCoordinator::new();
let mut accumulator = 0.0;

loop {
    let frame_delta = calculate_delta_time();
    accumulator += frame_delta;

    // Fixed timestep animation updates
    while accumulator >= ANIMATION_DT {
        coordinator.tick(ANIMATION_DT);
        accumulator -= ANIMATION_DT;
    }

    // Request redraw if animations are active
    if coordinator.has_active_animations() {
        request_redraw();
    }

    // Render at display refresh rate
    render();
}
```

**120Hz animation tick** decoupled from render rate ensures smooth animations regardless of display (60Hz, 144Hz, variable).

### No Allocations in Hot Path

Design goal: Once animation is created, no allocations during update.

**Techniques:**
- Preallocate animation slots
- Use object pools for recycling
- Static dispatch via trait objects created once
- SOA (Structure of Arrays) memory layout for batch updates

**Benchmark target:** 1000 active animations < 1ms per frame (< 10% of 16.67ms budget at 60fps).

---

## Use Cases in uzor

What we'll build with this animation system:

### Button Hover/Press Transitions

```rust
// Hover
button.animate()
    .scale_to(1.05)
    .duration(Duration::from_millis(200))
    .easing(Easing::EaseOutCubic);

// Press
button.animate()
    .scale_to(0.95)
    .duration(Duration::from_millis(100))
    .easing(Easing::EaseInCubic);
```

### Panel Slide-In/Out

```rust
// Slide in from right with spring
panel.animate_spring()
    .x_to(0.0)
    .stiffness(100.0)
    .damping(10.0);

// Slide out to right
panel.animate()
    .x_to(screen_width)
    .duration(Duration::from_millis(300))
    .easing(Easing::EaseInCubic);
```

### Toast Notifications (Fade + Slide)

```rust
let timeline = Timeline::new()
    .add(
        toast.animate()
            .y_to(toast_y)
            .duration(Duration::from_millis(300))
            .easing(Easing::EaseOutCubic),
        "0ms"
    )
    .add(
        toast.animate()
            .opacity_to(1.0)
            .duration(Duration::from_millis(200))
            .easing(Easing::EaseInOutQuad),
        "0ms"  // Simultaneous with slide
    )
    .add_label("visible")
    .add(
        toast.animate()
            .opacity_to(0.0)
            .duration(Duration::from_millis(200)),
        "visible+=2000ms"  // Hold for 2 seconds, then fade
    );
```

### Dropdown Expand/Collapse

```rust
// Expand with spring
dropdown.animate_spring()
    .height_to(content_height)
    .stiffness(150.0)
    .damping(12.0);

// Collapse
dropdown.animate()
    .height_to(0.0)
    .duration(Duration::from_millis(200))
    .easing(Easing::EaseInCubic);
```

### Modal Backdrop Fade

```rust
backdrop.animate()
    .opacity_to(0.8)
    .duration(Duration::from_millis(250))
    .easing(Easing::EaseOut);
```

### Page Transitions

```rust
let timeline = Timeline::new()
    // Fade out old page
    .add(
        old_page.animate()
            .opacity_to(0.0)
            .duration(Duration::from_millis(200)),
        "0ms"
    )
    // Slide in new page
    .add(
        new_page.animate()
            .x_from(screen_width)
            .x_to(0.0)
            .duration(Duration::from_millis(300))
            .easing(Easing::EaseOutCubic),
        "-=100ms"  // Overlap by 100ms
    )
    .add(
        new_page.animate()
            .opacity_to(1.0)
            .duration(Duration::from_millis(200)),
        "<"  // Start of previous animation
    );
```

### Loading Spinner

```rust
// Continuous rotation
spinner.animate()
    .rotation_to(360.0)
    .duration(Duration::from_secs(1))
    .easing(Easing::Linear)
    .repeat(Repeat::Infinite);
```

### Chart Data Transitions (Future)

```rust
// Animate between datasets with stagger
let stagger = Stagger::simple(Duration::from_millis(20));

for (i, bar) in chart.bars.iter().enumerate() {
    let delay = stagger.delay_for_index(i);

    bar.animate_spring()
        .height_to(new_data[i])
        .delay(delay)
        .stiffness(80.0)
        .damping(8.0);
}
```

---

## Stolen from the Best

We're not inventing new animation techniques. We're porting battle-tested algorithms from the web animation ecosystem to Rust.

### Spring Physics: Framer Motion / wobble

**Analytical solution** to damped harmonic oscillator:
- **wobble** (by skevy): Reverse-engineered Apple's CASpringAnimation
- Under/critical/over-damped cases with closed-form equations
- Position and velocity as pure functions of time

**Sources:**
- [wobble on GitHub](https://github.com/skevy/wobble)
- [Framer Motion spring animations](https://www.framer.com/motion/)
- [The Physics Behind Spring Animations](https://blog.maximeheckel.com/posts/the-physics-behind-spring-animations/)

### Easing Functions: Robert Penner

**The OG.** Penner's equations (2001) are in every animation library:
- GSAP, AnimeJS, jQuery, CSS transitions — all use Penner
- 30 functions covering every easing pattern
- BSD licensed, freely available

**Sources:**
- [Robert Penner's Easing Functions](https://robertpenner.com/easing/)
- [easings.net](https://easings.net/) — Visual cheat sheet

### Cubic-Bezier: bezier-easing

**Based on browser implementations** (Firefox, Chrome):
- Newton-Raphson method for speed
- Bisection fallback for robustness
- Precomputed sample table (11 samples)

**Sources:**
- [bezier-easing on GitHub](https://github.com/gre/bezier-easing)
- [MDN: cubic-bezier()](https://developer.mozilla.org/en-US/docs/Web/CSS/easing-function/cubic-bezier)

### Timeline: GSAP Position Parameter

**The secret to GSAP's power:**
- Position parameter: `"+=100ms"`, `"-=50ms"`, `"label+=200ms"`
- Overlapping animations
- Nested timelines
- Clean, declarative sequencing

**Sources:**
- [GSAP Timeline Documentation](https://gsap.com/docs/v3/GSAP/Timeline/)
- [Understanding the Position Parameter](https://gsap.com/resources/position-parameter/)

### Stagger: AnimeJS Grid Stagger

**Grid-aware delay distribution:**
- Specify grid dimensions and origin
- Euclidean distance calculation
- From center, from edge, from arbitrary position
- Easing applied to stagger delays

**Sources:**
- [AnimeJS Stagger Documentation](https://animejs.com/documentation/#gridStagger)
- [AnimeJS Grid Demo](https://codepen.io/juliangarnier/pen/XvjWvx)

---

## Research

Deep technical dives are available in the `research/` directory:

- **`spring-physics.md`** — Analytical solutions vs numerical integration, wobble algorithm, damping regimes
- **`easing-functions.md`** — Penner's equations, cubic-bezier solver (Newton-Raphson + bisection), performance analysis
- **`timeline-architecture.md`** — GSAP vs AnimeJS vs Motion One, position parameter parsing, nested timelines
- **`stagger-patterns.md`** — Grid distance calculations, Euclidean vs Manhattan metrics, from-center vs from-edge
- **`rust-implementation-notes.md`** — Animatable trait design, SIMD optimization, SOA memory layout, render loop integration
- **`missing-engine-primitives.md`** — Analysis of what JS animation libraries have that we were missing, Tier 1/2/3 classification

Each research file includes:
- Algorithm explanations with formulas
- Rust implementation sketches
- Performance targets and benchmarks
- Links to reference implementations

---

## Roadmap

### Phase 1: Foundation — COMPLETE
- [x] Research spring physics, easing, timeline, stagger, decay, color, stroke, blend, path, layers, scroll
- [x] Define API surface
- [x] Implement Animatable trait
- [x] Implement 30 Penner easings + cubic-bezier solver + steps()
- [x] Implement spring physics (analytical, 3 damping regimes)

### Phase 2: Core Engine — COMPLETE
- [x] Implement Tween<T> and Timeline with position parameter
- [x] Implement stagger (linear + grid)
- [x] Implement decay (momentum/flick)
- [x] Implement OKLCH color interpolation
- [x] Implement stroke animation (SVG line drawing)
- [x] Implement blend/composition (CompositeMode, InterruptionStrategy)

### Phase 3: Advanced Motion — COMPLETE
- [x] Implement MotionPath with arc-length parameterization
- [x] Implement LayerStack with weight transitions
- [x] Implement ScrollTimeline / ViewTimeline / ParallaxLayer

### Phase 4: Integration — COMPLETE
- [x] AnimationCoordinator in uzor-core
- [x] Context.begin_frame() auto-ticking
- [x] Tween, Spring, Decay driver support

### Phase 5: Optimization — FUTURE
- [ ] SIMD batch easing
- [ ] SOA memory layout
- [ ] Benchmark 1000 animation scenario

### Phase 6: Preset Recipes — FUTURE
- [ ] Deep research of JS animation showcases (GSAP, Framer Motion, AnimeJS CodePen demos)
- [ ] Port best animations to uzor-animation recipes
- [ ] Widget-level convenience API (button.animate().fade_in())

### Phase 7: Advanced Features — FUTURE
- [ ] Keyframe animations (Lottie-style)
- [ ] Morphing (shape interpolation)
- [ ] Gesture velocity integration
- [ ] State machines (Rive-style)
- [ ] Procedural noise (Perlin/Simplex) for organic motion

---

## Performance Philosophy

**120fps is the north star.** Modern displays support 120Hz+. Animations should feel buttery smooth.

**Targets:**
- Single animation update: < 10ns
- 1000 active animations: < 1ms per frame
- Animation tick: < 10% of frame budget (< 1.6ms at 60fps)

**Techniques:**
- Fixed timestep updates (120Hz)
- No allocations in hot path
- SIMD for batch operations
- SOA memory layout for cache efficiency
- Analytical solutions (no iterative solving)

**Philosophy:** Pay upfront cost during animation creation, zero cost during playback.

---

## Contributing

Once implementation begins:
1. Check `research/` for technical details
2. Write tests for new easings/features
3. Benchmark performance-sensitive code
4. Match API examples shown above

---

## License

TBD — Likely MIT or Apache 2.0 to match uzor.

**Note on borrowed algorithms:**
- Penner easings: BSD 3-clause (attribution included)
- Wobble spring physics: MIT (attribution included)
- bezier-easing: MIT (attribution included)

All research properly attributes sources.
