# Easing Functions Implementation Research

## Overview

Research into easing function implementations from Robert Penner's equations, browser cubic-bezier solvers, and major animation libraries. Goal: identify the best formulas and performance characteristics for uzor-animation.

## 1. Robert Penner's Easing Equations (The OG)

**Original Source:** https://robertpenner.com/easing/

### History

Created by Robert Penner, discussed in Chapter 7 of "Programming Macromedia Flash MX" (2001). Released under BSD 3-clause "New" or "Revised" License.

### Complete Function Set

**30 total functions** across these categories:

**Basic:**
- Linear

**Polynomial:**
- Quadratic (Quad) - t²
- Cubic - t³
- Quartic (Quart) - t⁴
- Quintic (Quint) - t⁵

**Trigonometric:**
- Sinusoidal (Sine)
- Circular (Circ)

**Exponential:**
- Exponential (Expo)

**Special:**
- Elastic - Damped oscillation
- Bounce - Bouncing ball physics
- Back - Overshoot and return

**Variants:** Each (except Linear) has In, Out, InOut versions = 3 × 10 + 1 = 31 functions

### Parameter Convention

All functions use:
- `t` - Current time (normalized 0-1)
- `b` - Beginning value
- `c` - Change in value (delta)
- `d` - Duration

**Return value:** `b + c * easing(t/d)`

Modern APIs simplify to: `easing(t) -> t'` where both are 0-1.

### Source Code Availability

**Original equations:** https://robertpenner.com/scripts/easing_equations.txt

**Implementations available in:**
- JavaScript
- ActionScript
- Java
- Lua
- C#
- C++
- C

### Educational Resources

- "Understanding Easing: Explaining Penner's Equations"
- "Interpolation Tricks"
- Chapter 7 of Penner's book (PDF available)

## 2. Exact Mathematical Formulas

### easeInOutCubic

**Definition:** Accelerates until halfway, then decelerates

```javascript
function easeInOutCubic(t) {
    if (t < 0.5) {
        return 4 * t * t * t;
    } else {
        return (t - 1) * (2 * t - 2) * (2 * t - 2) + 1;
    }
}
```

**Alternative formulation:**
```javascript
function easeInOutCubic(t) {
    return t < 0.5
        ? Math.pow(t * 2, 3) / 2
        : (1 - Math.pow(1 - (t * 2 - 1), 3)) / 2 + 0.5;
}
```

**Rust implementation:**
```rust
fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = t - 1.0;
        let g = 2.0 * t - 2.0;
        f * g * g + 1.0
    }
}
```

**Performance:** Branch-free version possible using `select()` intrinsic on some platforms.

**Characteristics:**
- Smooth acceleration/deceleration
- Symmetric
- C2 continuous (smooth second derivative)
- Popular for UI animations

### easeOutElastic

**Definition:** Exponentially decaying sine wave, like a spring

**Constants:**
```javascript
const c4 = (2 * Math.PI) / 3;
```

**Formula:**
```javascript
function easeOutElastic(x) {
    if (x === 0) return 0;
    if (x === 1) return 1;

    return Math.pow(2, -10 * x) * Math.sin((x * 10 - 0.75) * c4) + 1;
}
```

**Simplified version with period parameter:**
```javascript
function easeOutElastic(t, p = 0.3) {
    return Math.pow(2, -10 * t) * Math.sin((t - p / 4) * (2 * Math.PI) / p) + 1;
}
```

**Rust implementation:**
```rust
fn ease_out_elastic(t: f32) -> f32 {
    const C4: f32 = (2.0 * std::f32::consts::PI) / 3.0;

    if t == 0.0 { return 0.0; }
    if t == 1.0 { return 1.0; }

    f32::powf(2.0, -10.0 * t) * f32::sin((t * 10.0 - 0.75) * C4) + 1.0
}
```

**Performance cost:**
- 1x `powf` (expensive, ~20-50 cycles)
- 1x `sin` (expensive, ~50-100 cycles)
- Branch overhead for edge cases

**Characteristics:**
- Overshoots target, oscillates back
- Natural spring-like feel
- Period can be adjusted
- Good for attention-grabbing animations

### easeOutBounce

**Definition:** Simulates bouncing ball physics

**Constants:**
```javascript
const n1 = 7.5625;
const d1 = 2.75;
```

**Formula:**
```javascript
function easeOutBounce(t) {
    if (t < 1 / d1) {
        return n1 * t * t;
    } else if (t < 2 / d1) {
        return n1 * (t -= 1.5 / d1) * t + 0.75;
    } else if (t < 2.5 / d1) {
        return n1 * (t -= 2.25 / d1) * t + 0.9375;
    } else {
        return n1 * (t -= 2.625 / d1) * t + 0.984375;
    }
}
```

**Explanation:** Multiple parabolic segments with decreasing amplitude, each representing a bounce.

**Rust implementation:**
```rust
fn ease_out_bounce(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;

    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}
```

**Performance cost:**
- Multiple branches (4-way)
- Simple arithmetic otherwise
- Harder to vectorize due to branches
- Roughly 10-20 cycles

**Characteristics:**
- Playful, physical feel
- Multiple impact points
- Amplitude decreases geometrically
- Good for notification animations

### easeInOutBack

**Definition:** Pulls back before going forward, overshoots at end

**Constants:**
```javascript
const c1 = 1.70158;      // Amount of overshoot
const c2 = c1 * 1.525;   // Adjusted for InOut
```

**Formula:**
```javascript
function easeInOutBack(x) {
    if (x < 0.5) {
        return (Math.pow(2 * x, 2) * ((c2 + 1) * 2 * x - c2)) / 2;
    } else {
        return (Math.pow(2 * x - 2, 2) * ((c2 + 1) * (x * 2 - 2) + c2) + 2) / 2;
    }
}
```

**Rust implementation:**
```rust
fn ease_in_out_back(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C2: f32 = C1 * 1.525;

    if t < 0.5 {
        let x = 2.0 * t;
        (x * x * ((C2 + 1.0) * x - C2)) / 2.0
    } else {
        let x = 2.0 * t - 2.0;
        (x * x * ((C2 + 1.0) * x + C2) + 2.0) / 2.0
    }
}
```

**Performance cost:**
- Branch
- `Math.pow` can be replaced with manual multiplication (x * x)
- Roughly 5-10 cycles

**Characteristics:**
- Anticipation (windup before action)
- Overshoot creates emphasis
- Popular for button presses, drawer animations
- Overshoot amount tunable via c1

## 3. CSS cubic-bezier() Implementation

**MDN Reference:** https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/easing-function/cubic-bezier

### The Problem

Given cubic Bezier curve with control points `(p1x, p1y, p2x, p2y)` and input time `t`, find output value `y`.

**Challenge:** Bezier curve is parameterized by parameter `s`, not input `x`. Must solve for `s` where `Bx(s) = x`, then compute `By(s)`.

### Browser Implementations

**Chrome/WebKit approach:**
1. **Newton-Raphson method** for fast convergence
2. **Fallback to bisection** if Newton-Raphson fails or doesn't converge

**Source insight from Chromium:** "searches for the t value that corresponds with the given x, using the Newton-Raphson method for better performance, but falls back to bisection as necessary."

### bezier-easing Library

**Repository:** https://github.com/gre/bezier-easing

**Tagline:** "cubic-bezier implementation for your JavaScript animation easings – MIT License"

**Based on:** Firefox and Chrome implementations

### Algorithm Details

**Three-phase approach:**

1. **Sampling phase (precomputation):**
```javascript
const NEWTON_ITERATIONS = 4;
const NEWTON_MIN_SLOPE = 0.001;
const SUBDIVISION_PRECISION = 0.0000001;
const SUBDIVISION_MAX_ITERATIONS = 10;
const SAMPLE_SIZE = 11;

// Precompute sample table
const sampleValues = new Float32Array(SAMPLE_SIZE);
for (let i = 0; i < SAMPLE_SIZE; i++) {
    sampleValues[i] = calcBezier(i / (SAMPLE_SIZE - 1), p1x, p2x);
}
```

2. **Initial guess from samples:**
```javascript
function getTForX(x) {
    // Use samples to find approximate interval
    let intervalStart = 0.0;
    let currentSample = 1;
    const lastSample = SAMPLE_SIZE - 1;

    for (; currentSample !== lastSample && sampleValues[currentSample] <= x; currentSample++) {
        intervalStart += kSampleStepSize;
    }

    // Initial guess from linear interpolation
    const dist = (x - sampleValues[currentSample - 1]) /
                 (sampleValues[currentSample] - sampleValues[currentSample - 1]);
    const guessForT = intervalStart + dist * kSampleStepSize;

    // Now refine...
}
```

3. **Newton-Raphson refinement:**
```javascript
function newtonRaphsonIterate(x, guessT) {
    for (let i = 0; i < NEWTON_ITERATIONS; i++) {
        const currentSlope = getSlope(guessT, p1x, p2x);
        if (currentSlope === 0.0) return guessT;

        const currentX = calcBezier(guessT, p1x, p2x) - x;
        guessT -= currentX / currentSlope;
    }
    return guessT;
}
```

4. **Bisection fallback:**
```javascript
function binarySubdivide(x, a, b) {
    let currentX, currentT, i = 0;
    do {
        currentT = a + (b - a) / 2.0;
        currentX = calcBezier(currentT, p1x, p2x) - x;
        if (currentX > 0.0) {
            b = currentT;
        } else {
            a = currentT;
        }
    } while (Math.abs(currentX) > SUBDIVISION_PRECISION &&
             ++i < SUBDIVISION_MAX_ITERATIONS);
    return currentT;
}
```

### Performance Analysis

**Newton-Raphson:**
- Converges quadratically (doubles precision each iteration)
- 4 iterations typically sufficient
- Requires derivative calculation
- Can fail on flat regions (slope ≈ 0)

**Binary search/bisection:**
- Converges linearly (halves interval each iteration)
- More iterations needed (10 typical)
- Always works, very robust
- No derivative needed

**Benchmark:** Newton-Raphson is "blindingly fast" - significantly faster than bisection for typical animation curves.

**Strategy:** Use Newton-Raphson first, fallback to bisection only when slope is too small or convergence fails.

### Rust Implementation Approach

```rust
pub struct CubicBezier {
    p1x: f32,
    p1y: f32,
    p2x: f32,
    p2y: f32,
    sample_values: [f32; 11],
}

impl CubicBezier {
    pub fn new(p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> Self {
        let mut sample_values = [0.0; 11];
        for i in 0..11 {
            let t = i as f32 / 10.0;
            sample_values[i] = calc_bezier(t, p1x, p2x);
        }
        Self { p1x, p1y, p2x, p2y, sample_values }
    }

    pub fn solve(&self, x: f32) -> f32 {
        // 1. Get initial guess from samples
        // 2. Try Newton-Raphson (4 iterations)
        // 3. Fallback to bisection if needed
        // 4. Calculate y from solved t
    }
}

#[inline]
fn calc_bezier(t: f32, a: f32, b: f32) -> f32 {
    // B(t) = 3*(1-t)^2*t*a + 3*(1-t)*t^2*b + t^3
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * a + 3.0 * mt * t2 * b + t3
}

#[inline]
fn get_slope(t: f32, a: f32, b: f32) -> f32 {
    // dB/dt = 3*(1-t)^2*a + 6*(1-t)*t*(b-a) + 3*t^2*(1-b)
    let mt = 1.0 - t;
    3.0 * mt * mt * a + 6.0 * mt * t * (b - a) + 3.0 * t * t * (1.0 - b)
}
```

### Performance Target

Single bezier evaluation: **< 100ns** (Newton-Raphson with 4 iterations + sampling overhead)

## 4. AnimeJS Easing Implementation

**Repository:** https://github.com/juliangarnier/anime

### Easing System

**Built-in easings:** Available via `anime.easings` object

**Custom easing support:**
```javascript
anime.easings['myCustomEasingName'] = function(t) {
    return Math.pow(Math.sin(t * 3), 3);
}
```

**Bezier curve support:** Easing parameter accepts array of Bezier coordinates:
```javascript
easing: [0.42, 0, 0.58, 1]  // cubic-bezier control points
```

### V4 Changes

**New power parameter:**
```javascript
ease: 'in(3)'      // Cubic in
ease: 'out(4)'     // Quartic out
ease: 'inOut(2)'   // Quadratic in-out
ease: 'outIn(5)'   // Quintic out-in
```

**Implementation:**
```javascript
function powerIn(p) {
    return function(t) {
        return Math.pow(t, p);
    }
}
```

### Source Code Location

Look in the main anime repository for:
- `easings` module/file
- Built-in easing function definitions
- Bezier curve solver implementation

**Does AnimeJS use Penner or custom?** Appears to use **Penner-based equations** with extensions for power curves and bezier.

## 5. GSAP CustomEase

**Documentation:** https://gsap.com/docs/v3/Eases/CustomEase/

### Capabilities

Creates custom easing functions from:
- SVG path data (`"M0,0 C0.5,0 0.5,1 1,1"`)
- Array of Bezier control points `[x1, y1, cp1x, cp1y, cp2x, cp2y, x2, y2, ...]`
- Simplified 4-value array `[x1, y1, x2, y2]` (auto-expanded)

### Core Algorithm

**Bezier to Points Conversion:**

`_bezierToPoints()` method recursively subdivides Bezier curves until deviation threshold is met.

**Subdivision algorithm:**
1. Evaluate bezier at midpoint
2. Check if midpoint deviates from linear interpolation
3. If deviation > threshold, split into two curves and recurse
4. Otherwise, add as linear segment

This creates an optimized lookup table of linear segments.

### Processing Pipeline

1. **Parse** SVG path or point array
2. **Process curves** - Subdivide beziers into linear segments
3. **Normalize** - Ensure curve goes from (0,0) to (1,1)
4. **Generate lookup table** - Array of x,y points
5. **Runtime:** Binary search lookup table + linear interpolation

### Supported SVG Commands

- `M` (moveTo)
- `C` (curveTo - cubic bezier)
- `S` (smoothCurveTo)
- `Q` (quadraticCurveTo)
- `A` (arc)

### Performance

CustomEase converts arbitrary curves to **optimized lookup tables** for fast runtime evaluation:
- Preprocessing: expensive (bezier subdivision)
- Runtime: cheap (binary search + lerp)

**Trade-off:** Higher memory usage for lookup table vs computation time.

## 6. Performance Cost Analysis

### Branch-Free Easings (Fastest)

**Linear:** `return t;` - 0 cycles

**Simple polynomial (no branches):**
- `easeInQuad`: `t * t` - 1 cycle (1 mul)
- `easeInCubic`: `t * t * t` - 2 cycles (2 mul)
- `easeInQuart`: `t * t * t * t` - 3 cycles (3 mul)

### Branching Easings (Moderate)

**easeInOutCubic:** 1 branch + 3-4 muls = 5-10 cycles

**easeOutBounce:** 4-way branch + arithmetic = 10-20 cycles

### Transcendental Easings (Expensive)

**easeOutElastic:** 1 `powf` + 1 `sin` = 70-150 cycles

**easeOutExpo:** 1 `powf` = 20-50 cycles

**easeOutCirc:** 1 `sqrt` = 10-20 cycles

### Bezier (Variable)

**cubic-bezier:** Newton-Raphson (4 iterations) = 50-100 cycles
- Includes bezier evaluation (3-4 per iteration)
- Derivative evaluation (1 per iteration)
- Convergence check

**Bisection fallback:** 10 iterations × 2 evaluations = 100-200 cycles

### SIMD Vectorization Potential

**Easy to vectorize (branch-free):**
- Linear
- Simple polynomials (quad, cubic, quart, quint)
- easeInOutCubic (with masked select)

**Hard to vectorize (branchy):**
- easeOutBounce
- easeInOutBack
- Any with edge-case checks (elastic, expo)

**SIMD strategy:** Use branch-free variants when possible, or accept scalar fallback for complex easings.

### GPU Considerations

For GPU compute:
- **Prefer lookup tables** for complex easings
- **Inline simple polynomials** (cubic, quart)
- **Avoid transcendentals** (sin, pow) if possible - use approximations or tables
- **Texture sampling** can be faster than compute for bezier curves

**Recommendation:** Provide both compute and lookup table paths.

## 7. What to Extract for uzor-animation

### Core Easing Set (Penner-based)

Implement **18 essential functions:**

**Polynomial:**
- `ease_in_quad`, `ease_out_quad`, `ease_in_out_quad`
- `ease_in_cubic`, `ease_out_cubic`, `ease_in_out_cubic`
- `ease_in_quart`, `ease_out_quart`, `ease_in_out_quart`

**Sinusoidal:**
- `ease_in_sine`, `ease_out_sine`, `ease_in_out_sine`

**Special:**
- `ease_out_elastic`
- `ease_out_bounce`
- `ease_in_out_back`

**Exponential:**
- `ease_in_expo`, `ease_out_expo`, `ease_in_out_expo`

### Bezier Solver

Implement bezier-easing approach:
- Precomputed sample table (11 samples)
- Newton-Raphson with bisection fallback
- Match browser implementations

### Custom Easing Support

Allow user-defined functions:
```rust
pub trait EasingFunction: Send + Sync {
    fn ease(&self, t: f32) -> f32;
}
```

### Optimization Flags

```rust
#[derive(Copy, Clone)]
pub enum EasingVectorization {
    Scalar,      // Always use scalar path
    SIMD,        // Use SIMD when available
    LookupTable, // Precompute to lookup table
}
```

### Performance Targets

- **Simple easing** (cubic): < 5ns per call
- **Complex easing** (elastic): < 100ns per call
- **Bezier solve**: < 100ns per call
- **SIMD batch (8x cubic)**: < 20ns total

### Testing

Include test suite comparing against:
- Robert Penner reference implementations
- CSS cubic-bezier for bezier curves
- easings.net visualization tool

## Sources

- [Robert Penner's Easing Functions](https://robertpenner.com/easing/)
- [Easing Equations TXT](https://robertpenner.com/scripts/easing_equations.txt)
- [Easings.net - Cheat Sheet](https://easings.net/)
- [bezier-easing - GitHub](https://github.com/gre/bezier-easing)
- [Using Bézier curves as easing functions](https://probablymarcus.com/blocks/2015/02/26/using-bezier-curves-as-easing-functions.html)
- [MDN: cubic-bezier()](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/easing-function/cubic-bezier)
- [AnimeJS - GitHub](https://github.com/juliangarnier/anime)
- [GSAP CustomEase](https://gsap.com/docs/v3/Eases/CustomEase/)
- [Improved Easing Functions](https://joshondesign.com/2013/03/01/improvedEasingEquations)
