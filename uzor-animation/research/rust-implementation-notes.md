# Rust Implementation Notes for uzor-animation

## Overview

Practical notes for implementing animation system in Rust, covering trait design, generics, SIMD, render loop integration, memory layout, and threading. Based on research from existing Rust animation libraries and game engine patterns.

## 1. Generic Animatable Trait Design

### The Challenge

Animations need to work over diverse types:
- Primitives: `f32`, `f64`, `i32`, `u32`
- Math types: `Vec2`, `Vec3`, `Vec4`, `Quaternion`
- Graphics types: `Color`, `Rect`, `Transform`
- Custom types: `Opacity`, `BorderRadius`, etc.

### Core Trait: Animatable

**Minimal trait for types that can be animated:**

```rust
pub trait Animatable: Copy + Clone + Send + Sync + 'static {
    /// Linear interpolation between two values
    fn lerp(from: &Self, to: &Self, t: f32) -> Self;

    /// Distance between two values (for velocity calculations, spring physics)
    fn distance(from: &Self, to: &Self) -> f32 {
        // Default: not required for all animations
        0.0
    }

    /// Add a delta to this value (for velocity-based animations)
    fn add_delta(&self, delta: &Self) -> Self {
        // Default: return self unchanged
        *self
    }
}
```

**Why these bounds?**
- `Copy + Clone`: Efficient passing, no lifetime issues
- `Send + Sync`: Thread-safe for parallel animation
- `'static`: No borrowed references, can store in animator

### Implementations for Common Types

**Primitives:**
```rust
impl Animatable for f32 {
    fn lerp(from: &Self, to: &Self, t: f32) -> Self {
        from + (to - from) * t
    }

    fn distance(from: &Self, to: &Self) -> f32 {
        (to - from).abs()
    }

    fn add_delta(&self, delta: &Self) -> Self {
        self + delta
    }
}

// Similar for f64, i32, u32 (with appropriate casting)
```

**Vec2/Vec3:**
```rust
#[derive(Copy, Clone, Debug)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Animatable for Vec2 {
    fn lerp(from: &Self, to: &Self, t: f32) -> Self {
        Vec2 {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
        }
    }

    fn distance(from: &Self, to: &Self) -> f32 {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        (dx * dx + dy * dy).sqrt()
    }

    fn add_delta(&self, delta: &Self) -> Self {
        Vec2 {
            x: self.x + delta.x,
            y: self.y + delta.y,
        }
    }
}
```

**Color:**
```rust
#[derive(Copy, Clone, Debug)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Animatable for Color {
    fn lerp(from: &Self, to: &Self, t: f32) -> Self {
        Color {
            r: from.r + (to.r - from.r) * t,
            g: from.g + (to.g - from.g) * t,
            b: from.b + (to.b - from.b) * t,
            a: from.a + (to.a - from.a) * t,
        }
    }

    fn distance(from: &Self, to: &Self) -> f32 {
        // Euclidean distance in RGBA space
        let dr = to.r - from.r;
        let dg = to.g - from.g;
        let db = to.b - from.b;
        let da = to.a - from.a;
        (dr*dr + dg*dg + db*db + da*da).sqrt()
    }
}
```

**Rect:**
```rust
#[derive(Copy, Clone, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Animatable for Rect {
    fn lerp(from: &Self, to: &Self, t: f32) -> Self {
        Rect {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
            width: from.width + (to.width - from.width) * t,
            height: from.height + (to.height - from.height) * t,
        }
    }
}
```

### Using num-traits for Generic Numeric Code

**Repository:** https://github.com/rust-num/num-traits

**For algorithms that work over any numeric type:**

```rust
use num_traits::Float;

pub fn generic_spring_position<F: Float>(
    t: F,
    initial_pos: F,
    initial_vel: F,
    target: F,
    stiffness: F,
    damping: F,
    mass: F,
) -> F {
    let omega = (stiffness / mass).sqrt();
    let zeta = damping / (F::from(2.0).unwrap() * (stiffness * mass).sqrt());

    // ... spring physics calculations using Float trait methods
}
```

**Float trait provides:**
- Arithmetic operations
- `sqrt()`, `sin()`, `cos()`, `exp()`
- Constants like `PI`, `E`
- Comparison

**Use case:** Spring physics that works for both f32 and f64.

### Derive Macro for Animatable

**Goal:** Auto-implement Animatable for structs

```rust
#[derive(Animatable)]
#[animatable(lerp_fields)]
struct Transform {
    position: Vec2,
    rotation: f32,
    scale: Vec2,
}

// Expands to:
impl Animatable for Transform {
    fn lerp(from: &Self, to: &Self, t: f32) -> Self {
        Transform {
            position: Vec2::lerp(&from.position, &to.position, t),
            rotation: f32::lerp(&from.rotation, &to.rotation, t),
            scale: Vec2::lerp(&from.scale, &to.scale, t),
        }
    }
}
```

**Implementation sketch:**
```rust
// In proc-macro crate
#[proc_macro_derive(Animatable, attributes(animatable))]
pub fn derive_animatable(input: TokenStream) -> TokenStream {
    // Parse struct
    // Generate lerp implementation for each field
    // Handle attributes like #[animatable(skip)]
}
```

## 2. SIMD for Batch Easing Calculations

### When to Use SIMD

**Good candidates:**
- Evaluating same easing function for many values
- Batch spring calculations (8 springs at once)
- Color space conversions (RGBA → HSLA for 8 colors)

**Poor candidates:**
- Single animation updates (overhead not worth it)
- Branching easing functions (e.g., bounce)
- Mixed animation types

### Portable SIMD in Rust

**Crate:** `std::simd` (nightly, stabilizing)

**Basic example:**
```rust
#![feature(portable_simd)]
use std::simd::f32x8;

pub fn ease_in_out_cubic_batch(t: &[f32; 8]) -> [f32; 8] {
    let t_vec = f32x8::from_array(*t);
    let half = f32x8::splat(0.5);
    let two = f32x8::splat(2.0);
    let four = f32x8::splat(4.0);

    // Mask for t < 0.5
    let mask = t_vec.simd_lt(half);

    // Branch 1: 4 * t * t * t
    let t2 = t_vec * two;
    let branch1 = four * t_vec * t_vec * t_vec;

    // Branch 2: (t-1) * (2t-2) * (2t-2) + 1
    let one = f32x8::splat(1.0);
    let f = t_vec - one;
    let g = two * t_vec - two;
    let branch2 = f * g * g + one;

    // Select based on mask
    let result = mask.select(branch1, branch2);
    result.to_array()
}
```

**Performance:** 8× throughput for simple easings.

### Branch-Free Easing for SIMD

**Problem:** Branches (`if t < 0.5`) cause SIMD divergence.

**Solution:** Use masked select:
```rust
// Instead of:
if t < 0.5 { branch1 } else { branch2 }

// Use:
let mask = t_vec.simd_lt(half);
mask.select(branch1, branch2)
```

**Both branches execute, but only one result is selected per lane.**

**Trade-off:** More computation, but no divergence. Worth it for SIMD.

### SIMD Spring Physics

**Batch evaluate 8 springs:**
```rust
use std::simd::f32x8;

pub struct SpringBatchSOA {
    stiffness: f32x8,
    damping: f32x8,
    mass: f32x8,
    initial_pos: f32x8,
    initial_vel: f32x8,
    target: f32x8,
}

impl SpringBatchSOA {
    pub fn evaluate_at_time(&self, t: f32) -> [f32; 8] {
        let t_vec = f32x8::splat(t);

        // Calculate omega for all 8 springs at once
        let omega = (self.stiffness / self.mass).sqrt();

        // Calculate zeta
        let two = f32x8::splat(2.0);
        let zeta = self.damping / (two * (self.stiffness * self.mass).sqrt());

        // ... rest of spring physics using SIMD operations

        positions.to_array()
    }
}
```

**Use case:** Animating many UI elements with springs (buttons, cards, etc.)

### Performance Considerations

**SIMD overhead:**
- Loading data into SIMD registers: 1-2 cycles
- SIMD ops: 1-4 cycles each
- Extracting results: 1-2 cycles

**Break-even point:** ~4-8 elements. Below that, scalar is faster.

**Memory alignment:** SIMD performs best with 16/32-byte aligned data.

```rust
#[repr(align(32))]
pub struct AlignedSpringData {
    positions: [f32; 8],
    velocities: [f32; 8],
}
```

## 3. Animation Coordinator and Render Loop Integration

### Game Loop Patterns

**Reference:** https://gameprogrammingpatterns.com/game-loop.html

**Standard game loop:**
```
loop {
    processInput();
    update(deltaTime);
    render();
}
```

**Animation coordinator fits in `update()` phase.**

### Fixed Timestep vs Variable Timestep

**Variable timestep (simplest):**
```rust
let mut last_time = Instant::now();

loop {
    let now = Instant::now();
    let delta = now.duration_since(last_time).as_secs_f32();
    last_time = now;

    animation_coordinator.update(delta);
    render();
}
```

**Pros:** Simple, animations scale with actual time
**Cons:** Non-deterministic, can stutter if frame drops

**Fixed timestep (better for consistency):**
```rust
const DT: f32 = 1.0 / 120.0;  // 120 Hz update rate

let mut accumulator = 0.0;
let mut last_time = Instant::now();

loop {
    let now = Instant::now();
    let frame_time = now.duration_since(last_time).as_secs_f32();
    last_time = now;

    accumulator += frame_time;

    while accumulator >= DT {
        animation_coordinator.update(DT);
        accumulator -= DT;
    }

    // Render with interpolation factor: accumulator / DT
    render(accumulator / DT);
}
```

**Pros:** Deterministic, stable animations
**Cons:** More complex, requires interpolation

### Animation Coordinator Architecture

```rust
pub struct AnimationCoordinator {
    animations: Vec<ActiveAnimation>,
    timelines: Vec<Timeline>,
    next_id: AnimationId,
}

pub struct ActiveAnimation {
    id: AnimationId,
    elapsed: f32,
    duration: f32,
    easing: Box<dyn EasingFunction>,
    target: AnimationTarget,
    from: Box<dyn Any>,  // Type-erased start value
    to: Box<dyn Any>,    // Type-erased end value
    on_update: Box<dyn FnMut(&dyn Any)>,
}

impl AnimationCoordinator {
    pub fn update(&mut self, delta: f32) {
        // Update all animations
        self.animations.retain_mut(|anim| {
            anim.elapsed += delta;
            let t = (anim.elapsed / anim.duration).min(1.0);
            let eased_t = anim.easing.ease(t);

            // Interpolate and call update callback
            // (type-erased, uses Any downcasting)
            anim.interpolate_and_update(eased_t);

            // Remove if complete
            anim.elapsed < anim.duration
        });

        // Update timelines
        for timeline in &mut self.timelines {
            timeline.tick(delta);
        }
    }

    pub fn animate<T: Animatable>(
        &mut self,
        from: T,
        to: T,
        duration: f32,
        easing: impl EasingFunction + 'static,
        on_update: impl FnMut(T) + 'static,
    ) -> AnimationId {
        // Create and store animation
    }
}
```

### Integration with Render Loop

**uzor-animation should provide:**

```rust
pub trait AnimationDriver {
    fn tick(&mut self, delta: f32);
    fn has_active_animations(&self) -> bool;
}

impl AnimationDriver for AnimationCoordinator {
    fn tick(&mut self, delta: f32) {
        self.update(delta);
    }

    fn has_active_animations(&self) -> bool {
        !self.animations.is_empty() || !self.timelines.is_empty()
    }
}
```

**App integration:**
```rust
let mut coordinator = AnimationCoordinator::new();

loop {
    let delta = calculate_delta_time();

    coordinator.tick(delta);

    if coordinator.has_active_animations() {
        request_redraw();  // Request another frame
    }

    render();
}
```

### Decoupling Animation Tick from Render

**For 120Hz animation with variable render rate:**

```rust
const ANIMATION_DT: f32 = 1.0 / 120.0;

let mut anim_accumulator = 0.0;

loop {
    let frame_delta = calculate_delta_time();
    anim_accumulator += frame_delta;

    // Update animations at fixed 120 Hz
    while anim_accumulator >= ANIMATION_DT {
        coordinator.update(ANIMATION_DT);
        anim_accumulator -= ANIMATION_DT;
    }

    // Render at display refresh rate (may be 60Hz, 144Hz, etc.)
    render();
}
```

**Benefit:** Smooth animations regardless of render rate.

## 4. Memory Layout: SOA vs AOS

### Research Summary

**Source:** https://en.algorithmica.org/hpc/cpu-cache/aos-soa/

**Key findings:**
- SOA can provide 10-100× performance improvement
- 40-60% gains typical in real workloads
- Depends heavily on access patterns

### Array of Structures (AOS)

**Intuitive, object-oriented:**
```rust
pub struct Animation {
    id: AnimationId,
    elapsed: f32,
    duration: f32,
    easing_type: EasingType,
    // ... more fields
}

pub struct Coordinator {
    animations: Vec<Animation>,  // AOS
}
```

**Memory layout:**
```
[Animation1][Animation2][Animation3]...
[id|elapsed|duration|easing][id|elapsed|duration|easing]...
```

**Cache behavior:**
- Fetching one field loads entire struct (64-byte cache line)
- If only updating `elapsed`, other fields waste cache

**When AOS is good:**
- Accessing all fields together
- Few animations (<100)
- Random access by ID

### Structure of Arrays (SOA)

**Cache-friendly, data-oriented:**
```rust
pub struct CoordinatorSOA {
    ids: Vec<AnimationId>,
    elapsed_times: Vec<f32>,
    durations: Vec<f32>,
    easing_types: Vec<EasingType>,
    // ... other parallel arrays
}
```

**Memory layout:**
```
[id1|id2|id3|id4|...]
[elapsed1|elapsed2|elapsed3|elapsed4|...]
[duration1|duration2|duration3|duration4|...]
```

**Cache behavior:**
- Updating `elapsed` loads only `elapsed_times` array
- 100% cache line utilization
- Better for sequential iteration

**When SOA is good:**
- Updating single field for many animations
- SIMD vectorization (process 8 `elapsed` values at once)
- Large number of animations (1000+)

### Hybrid: Array of Structures of Arrays (AoSoA)

**Tile-based approach:**
```rust
const TILE_SIZE: usize = 8;  // Match SIMD width

pub struct AnimationTile {
    elapsed: [f32; TILE_SIZE],
    duration: [f32; TILE_SIZE],
    easing: [EasingType; TILE_SIZE],
}

pub struct CoordinatorAoSoA {
    tiles: Vec<AnimationTile>,
}
```

**Benefits:**
- SIMD-friendly (8 elements per tile)
- Better cache locality than pure SOA
- Easier to manage than pure SOA

**Unity DOTS uses AoSoA for massive performance gains.**

### Recommendation for uzor-animation

**Start with AOS:**
- Simpler to implement
- Adequate for <1000 animations
- Easier to debug

**Benchmark, then optimize:**
- If profiling shows cache misses
- If animating 1000+ elements
- If SIMD optimization needed

**Then migrate hot paths to SOA/AoSoA.**

### SOA Implementation Example

```rust
pub struct AnimationCoordinatorSOA {
    // Hot data (updated every frame)
    elapsed_times: Vec<f32>,
    durations: Vec<f32>,

    // Warm data (read every frame)
    easing_functions: Vec<Box<dyn EasingFunction>>,

    // Cold data (rarely accessed)
    ids: Vec<AnimationId>,
    metadata: Vec<AnimationMetadata>,

    len: usize,
}

impl AnimationCoordinatorSOA {
    pub fn update(&mut self, delta: f32) {
        // SIMD-friendly loop: only access elapsed and durations
        for i in 0..self.len {
            self.elapsed_times[i] += delta;

            let t = (self.elapsed_times[i] / self.durations[i]).min(1.0);
            let eased_t = self.easing_functions[i].ease(t);

            // ... apply animation
        }

        // Remove completed (requires careful array manipulation)
        self.remove_completed();
    }

    fn remove_completed(&mut self) {
        // Swap-remove pattern to maintain SOA invariant
        let mut i = 0;
        while i < self.len {
            if self.elapsed_times[i] >= self.durations[i] {
                self.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    fn swap_remove(&mut self, index: usize) {
        let last = self.len - 1;
        self.elapsed_times.swap(index, last);
        self.durations.swap(index, last);
        self.easing_functions.swap(index, last);
        // ... swap all arrays

        self.elapsed_times.pop();
        self.durations.pop();
        self.easing_functions.pop();
        // ... pop all arrays

        self.len -= 1;
    }
}
```

**Complexity:** SOA requires maintaining invariants across multiple arrays. Error-prone.

**Helper crate:** Consider `soa_derive` crate for automatic SOA generation.

## 5. Thread Safety and Parallelism

### Animation on Main Thread or Separate?

**Main thread (recommended for UI):**
- Direct access to render state
- No synchronization overhead
- Simpler implementation
- Matches browser animation model

**Separate thread:**
- Offload work from main thread
- Can run animations while rendering blocked
- Requires Send + Sync on all types
- Needs message passing or atomic updates

### Send + Sync Constraints

**For thread-safe animation:**
```rust
pub trait Animatable: Copy + Clone + Send + Sync + 'static {
    // ...
}

pub trait EasingFunction: Send + Sync {
    fn ease(&self, t: f32) -> f32;
}
```

**Why Send + Sync?**
- `Send`: Can transfer ownership to another thread
- `Sync`: Can share references between threads

**Implication:** All animatable types and easing functions must be thread-safe.

### Parallel Animation Update

**Using rayon for data parallelism:**

```rust
use rayon::prelude::*;

impl AnimationCoordinator {
    pub fn update_parallel(&mut self, delta: f32) {
        self.animations.par_iter_mut().for_each(|anim| {
            anim.elapsed += delta;
            let t = (anim.elapsed / anim.duration).min(1.0);
            let eased_t = anim.easing.ease(t);

            // Update (must be thread-safe)
            anim.update(eased_t);
        });

        // Remove completed (serial, requires mutation)
        self.animations.retain(|a| a.elapsed < a.duration);
    }
}
```

**When worth it?** 1000+ animations. Below that, overhead > gains.

### Atomic Updates for Cross-Thread Communication

**Scenario:** Animation thread updates values, render thread reads.

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

pub struct AnimatedValue {
    value: Arc<AtomicU32>,  // f32 stored as u32 bits
}

impl AnimatedValue {
    pub fn set(&self, val: f32) {
        self.value.store(val.to_bits(), Ordering::Relaxed);
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.value.load(Ordering::Relaxed))
    }
}
```

**For Vec2, Color, etc., use multiple atomics or Mutex:**
```rust
use parking_lot::Mutex;  // Faster than std Mutex

pub struct AnimatedVec2 {
    value: Arc<Mutex<Vec2>>,
}
```

### Recommendation for uzor-animation

**Start single-threaded:**
- All updates on main thread
- No synchronization complexity
- Profile first

**If profiling shows animation tick is bottleneck:**
- Try parallel iteration (rayon) for batch updates
- Keep state on main thread, just parallelize computation

**Avoid separate animation thread unless necessary:**
- Adds complexity
- Synchronization overhead
- Browser animations run on main thread, works fine

## 6. Crate Dependencies

### Essential

- `num-traits` - Generic numeric operations
- `serde` (optional) - Serialize animation definitions

### SIMD

- `std::simd` (nightly) or `wide` (stable) - SIMD operations

### Parallel

- `rayon` - Data parallelism for batch updates

### Utilities

- `parking_lot` - Faster Mutex/RwLock if threading needed
- `smallvec` - Stack-allocated vectors for small animations

### Procedural Macros

- `syn`, `quote`, `proc-macro2` - For derive macros

## 7. Performance Targets

### Micro-benchmarks

**Single animation update (f32 lerp):** < 5ns
**Single animation with cubic easing:** < 10ns
**Single spring evaluation (analytical):** < 50ns
**Bezier solve:** < 100ns

### Batch operations

**1000 simple animations:** < 50μs (50ns each)
**SIMD batch (8× cubic easing):** < 50ns total
**Timeline with 100 tweens:** < 5μs per tick

### Real-world targets

**Update 1000 UI elements with springs:** < 1ms
**60fps budget:** 16.67ms per frame
**Animation should use < 10% of frame budget:** < 1.6ms

## 8. Testing Strategy

### Unit tests

- Animatable trait implementations (lerp correctness)
- Easing functions (compare against reference implementations)
- Spring physics (verify against known solutions)

### Integration tests

- Timeline sequencing (verify start times, overlaps)
- Stagger patterns (verify delay calculations)
- Coordinator lifecycle (add/remove animations)

### Benchmarks

Use `criterion` crate:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_cubic_easing(c: &mut Criterion) {
    c.bench_function("ease_in_out_cubic", |b| {
        b.iter(|| {
            ease_in_out_cubic(black_box(0.5))
        })
    });
}

criterion_group!(benches, bench_cubic_easing);
criterion_main!(benches);
```

### Property-based testing

Use `proptest`:
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_lerp_bounds(t in 0.0f32..=1.0) {
        let from = 0.0;
        let to = 100.0;
        let result = f32::lerp(&from, &to, t);

        prop_assert!(result >= from);
        prop_assert!(result <= to);
    }
}
```

## Sources

- [lerp crate documentation](https://docs.rs/lerp)
- [num-traits documentation](https://docs.rs/num-traits)
- [anim-rs - Framework independent animation](https://github.com/Joylei/anim-rs)
- [numeric-array for SIMD](https://docs.rs/numeric-array)
- [Game Programming Patterns: Game Loop](https://gameprogrammingpatterns.com/game-loop.html)
- [Integration Basics - Gaffer on Games](https://gafferongames.com/post/integration_basics/)
- [AOS and SOA - Algorithmica](https://en.algorithmica.org/hpc/cpu-cache/aos-soa/)
- [SOA vs AOS Deep Dive](https://medium.com/@azad217/structure-of-arrays-soa-vs-array-of-structures-aos-in-c-a-deep-dive-into-cache-optimized-13847588232e)
- [Optimizing Wulverblade: Threading Animation Updates](http://wulverblade.com/optimizing-games-threading-animation-updates/)
- [Unreal Engine Animation Optimization](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-optimization-in-unreal-engine)
- [SIMD Vectorization Guide](https://en.algorithmica.org/hpc/simd/auto-vectorization/)
- [Rust SIMD Documentation](https://doc.rust-lang.org/std/simd/index.html)
