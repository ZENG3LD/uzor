# Spring Physics Implementation Research

## Overview

This document analyzes spring physics implementations from major animation libraries to inform uzor-animation's design. The key question: **analytical solution vs numerical integration?**

## 1. Framer Motion / Motion One

**Repository:** https://github.com/motiondivision/motion

### Spring Parameters

Framer Motion uses three physics parameters:
- `stiffness` (default: 100) - Controls spring rigidity, higher = more sudden movement
- `damping` (default: 10) - Dissipates energy, slows oscillation
- `mass` (default: 1) - Higher mass = more lethargic movement

### Duration Calculation

**Damping ratio formula:**
```javascript
dampingRatio = damping / (2 * Math.sqrt(stiffness * mass))
```

This determines oscillation behavior:
- `dampingRatio < 1` - Under-damped (bouncy)
- `dampingRatio = 1` - Critically damped (no overshoot)
- `dampingRatio > 1` - Over-damped (slow)

### Implementation Approach

Motion incorporates gesture velocity into spring animations and derives duration from physics rather than setting it manually. Springs are physics-based, not duration-based by default.

**Alternative: Duration-based springs**
Instead of tweaking stiffness/damping/mass, you can specify:
- `duration` - Target animation length
- `bounce` - Amount of overshoot (0-1)

The system automatically calculates physics parameters to achieve the target.

### Source Code Location

Look for spring solver in the Motion One monorepo: https://github.com/motiondivision/motionone

**Key files to examine:**
- Spring animation core logic
- Duration derivation from physics parameters
- Integration method (likely analytical solution given Framer's heritage)

## 2. React-Spring

**Repository:** https://github.com/pmndrs/react-spring

### Historical Evolution

React-spring went through multiple spring physics approaches:

**PR #792 - CASpringAnimation Algorithm**
- Introduced analytical solution inspired by Apple's CASpringAnimation
- Uses closed-form solution for damped harmonic oscillator
- Source: https://github.com/pmndrs/react-spring/pull/792

**PR #797 - Semi-Implicit Euler**
- Switched to semi-implicit Euler integration
- Minimizes steps needed to accomplish interpolation
- Sets `dt = 300 / naturalFrequency` (not always 1ms)
- Source: https://github.com/pmndrs/react-spring/pull/797

### Algorithm: Wobble-Based Analytical Solution

React-spring "stole from the library wobble" which represents "the most complete equation, differentiating under damped, critically damped and over damped systems."

The implementation references **skevy/wobble** algorithm.

### Key Implementation Files

- `packages/core/src/SpringValue.ts` - Main spring value class
- `SpringValue.advance(ms)` - Processes animation by milliseconds
- `AnimatedValue` - Stores `lastVelocity` for velocity tracking

## 3. Wobble Library (Foundation for CASpringAnimation)

**Repository:** https://github.com/skevy/wobble

### Design Philosophy

**Goal:** Replicate Apple's CASpringAnimation precisely.

After investigating QuartzCore.framework, discovered Apple uses **closed-form solution for damped harmonic oscillation** rather than numerical integration.

### Why Analytical Solution?

**Advantages over RK4 or Euler:**

1. **Perfect accuracy** - No drift, no energy loss
2. **Pure function of time** - `position = f(t)` and `velocity = g(t)`
3. **Easier keyframe generation** - Can sample at arbitrary points
4. **Better for interruptible animations** - Can resume from any state
5. **Performance** - Faster than 4x function evaluations of RK4

**Size:** Tiny (~1.7 KB gzipped)

### The Math: Damped Harmonic Oscillator

The differential equation:
```
m * x''(t) + c * x'(t) + k * x(t) = 0
```

Where:
- `m` = mass
- `c` = damping coefficient
- `k` = spring constant (stiffness)
- `x(t)` = position at time t
- `x'(t)` = velocity at time t
- `x''(t)` = acceleration at time t

### Analytical Solutions

**Define:**
```
ω₀ = sqrt(k/m)           // Natural angular frequency
ζ = c / (2 * sqrt(k*m))  // Damping ratio
```

**Case 1: Under-damped (ζ < 1) - Oscillatory**
```
ωd = ω₀ * sqrt(1 - ζ²)   // Damped angular frequency

x(t) = e^(-ζ*ω₀*t) * (A*cos(ωd*t) + B*sin(ωd*t))
```

**Case 2: Critically damped (ζ = 1) - Fastest settling**
```
x(t) = (A + B*t) * e^(-ω₀*t)
```

**Case 3: Over-damped (ζ > 1) - Slow, no oscillation**
```
r₁ = -ω₀ * (ζ + sqrt(ζ² - 1))
r₂ = -ω₀ * (ζ - sqrt(ζ² - 1))

x(t) = A*e^(r₁*t) + B*e^(r₂*t)
```

Constants A and B are determined from initial conditions (position, velocity).

### Source Code Structure

Check `src/index.ts` for:
- Omega calculations for angular frequency
- Separate paths for under/critical/over-damped
- Position and velocity as pure functions of time

**Source:** https://github.com/skevy/wobble/blob/develop/src/index.ts

## 4. Apple CASpringAnimation

**Documentation:** https://developer.apple.com/documentation/quartzcore/caspringanimation

### Parameters

Identical to Framer Motion:
- `mass` - Inertia of animated object
- `stiffness` - Spring rigidity
- `damping` - Friction force
- `initialVelocity` - Starting speed (default 0)

### Implementation

**Algorithm:** Analytical solution for damped harmonic oscillator (confirmed by wobble's QuartzCore investigation).

**Duration derivation:** The system properties determine duration automatically. Duration emerges from physics, not set manually.

### Quirks and Issues

Article: "Your Spring Animations Are Bad (And It's Probably Apple's Fault)"
- Source: https://medium.com/@flyosity/your-spring-animations-are-bad-and-it-s-probably-apple-s-fault-784932e51733
- Discusses issues with default parameters
- No public API to calculate velocity at arbitrary time

### Open Source Alternatives

**CocoaSprings:** https://github.com/MacPaw/CocoaSprings
- Interactive spring animations for macOS/iOS
- Based on Ryan Juckett's spring physics blog post

**Advance:** https://github.com/timdonnelly/Advance
- Physics-based animations for iOS, tvOS, macOS

## 5. RK4 vs Analytical Solution Performance

### Computational Cost

**RK4 (Runge-Kutta 4th order):**
- 4 evaluations of acceleration function per timestep
- Most accurate numerical method
- Most computationally expensive
- **Energy loss over time** - accumulates error

**Semi-Implicit Euler:**
```javascript
velocity += acceleration * dt;
position += velocity * dt;
```
- Simple, 1 evaluation per step
- Symplectic integrator, conserves energy better than explicit Euler
- Frequency drift - slightly different period than exact solution

**Analytical Solution:**
- Single evaluation of closed-form equation
- Perfect accuracy, no drift
- No energy loss
- Fastest

### Benchmark Results

After 90 seconds of spring simulation:
- **Semi-implicit Euler:** Drifted out of phase (frequency error)
- **RK4:** Matched frequency but lost energy
- **Analytical:** Perfect match

### Recommendation

> "If you can solve the differential equations to find a closed form time domain solution, then just evaluate that solution instead of integrating."

For general dynamics, Velocity Verlet with smaller timestep beats RK4 for accuracy/performance ratio.

**For springs specifically: Analytical solution is superior.**

**Sources:**
- https://gafferongames.com/post/integration_basics/
- Search results on RK4 vs analytical solutions

## 6. Which Approach for 120fps GPU-Rendered UI?

### Winner: Analytical Solution (Wobble-style)

**Reasons:**

1. **Deterministic** - Same result every frame, no accumulation error
2. **Frame-rate independent** - Works perfectly at variable FPS
3. **Resumable** - Can pause/resume without state corruption
4. **Scrubbing** - Can seek to arbitrary time for timeline editing
5. **Faster** - No iterative solving needed
6. **Smaller** - Wobble is 1.7 KB, contains all 3 damping cases
7. **GPU-friendly** - Can precompute keyframes for GPU upload

### Implementation Strategy for Rust

**Trait structure:**
```rust
pub trait SpringPhysics {
    fn position_at_time(&self, t: f32, initial_pos: f32, initial_vel: f32, target: f32) -> f32;
    fn velocity_at_time(&self, t: f32, initial_pos: f32, initial_vel: f32, target: f32) -> f32;
    fn is_at_rest(&self, t: f32, threshold: f32) -> bool;
}
```

**Three implementations:**
```rust
struct UnderDampedSpring { stiffness, damping, mass }
struct CriticallyDampedSpring { stiffness, mass }
struct OverDampedSpring { stiffness, damping, mass }
```

Select implementation based on damping ratio at spring creation.

### Optimization for Batch Evaluation

For GPU rendering, precompute keyframes:
```rust
// Sample spring curve into keyframe buffer
let keyframes: Vec<f32> = (0..num_samples)
    .map(|i| {
        let t = (i as f32) * dt;
        spring.position_at_time(t, 0.0, 0.0, 1.0)
    })
    .collect();

// Upload to GPU texture or buffer
// Shader samples with texture lookup or interpolation
```

### SIMD Opportunities

Batch multiple springs in parallel:
```rust
// SOA layout
struct SpringBatch {
    stiffness: [f32; 8],
    damping: [f32; 8],
    mass: [f32; 8],
    // ... other params
}

// SIMD evaluation
fn eval_batch_simd(&self, t: f32) -> [f32; 8] {
    // Use portable_simd or std::simd
    // Compute 8 springs in parallel
}
```

## 7. What to Extract for uzor-animation

### Core Algorithm

Port wobble's analytical solution:
1. Three separate implementations for under/critical/over damped
2. Closed-form equations from damped harmonic oscillator
3. Position and velocity as pure functions of time

### API Design

**Match Framer Motion's ergonomics:**
```rust
Spring::new()
    .stiffness(100.0)
    .damping(10.0)
    .mass(1.0)
```

**Or duration-based:**
```rust
Spring::with_duration(0.5)
    .bounce(0.3)
```

### Performance Targets

- Single spring evaluation: < 50ns
- Batch of 1000 springs: < 50μs
- SIMD batch of 8: < 100ns

### Testing

Include wobble's test cases to verify correctness against CASpringAnimation behavior.

## Sources

- [Motion - Modern animation library](https://github.com/motiondivision/motion)
- [React-spring repository](https://github.com/pmndrs/react-spring)
- [React-spring PR #792: CASpringAnimation algorithm](https://github.com/pmndrs/react-spring/pull/792)
- [React-spring PR #797: Semi-implicit Euler](https://github.com/pmndrs/react-spring/pull/797)
- [Wobble - Damped harmonic oscillator](https://github.com/skevy/wobble)
- [Apple CASpringAnimation docs](https://developer.apple.com/documentation/quartzcore/caspringanimation)
- [The physics behind spring animations](https://blog.maximeheckel.com/posts/the-physics-behind-spring-animations/)
- [CocoaSprings - Interactive springs](https://github.com/MacPaw/CocoaSprings)
- [Integration Basics - Gaffer on Games](https://gafferongames.com/post/integration_basics/)
