# Missing Animation Engine Primitives

Research on fundamental animation building blocks beyond our current 4 modules (Easing, Spring, Timeline, Stagger).

**Evaluation Criteria:**
1. **FUNDAMENTAL**: Is this engine-level or can it be built on existing primitives?
2. **UI VALUE**: Does this add real value for UI framework (not just games)?
3. **COMPLEXITY**: Implementation difficulty (Low/Medium/High)
4. **REAL USAGE**: Where is this actually used in UI?

---

## 1. Physics-Based Primitives (Beyond Springs)

### 1.1 Inertia / Decay Animation ⭐⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Should be added

**What It Is**: Momentum-based deceleration using exponential decay. Element continues motion after gesture release, slowing down under virtual friction.

**Technical Details**:
- **Exponential decay formula**: velocity *= friction_coefficient (per frame)
- Framer Motion: `transition={{ type: "inertia" }}`
- iOS PastryKit: 0.95 friction coefficient per 16.7ms tick (325ms time constant)
- Parameters: initial velocity, friction coefficient, optional bounds with spring snap

**Implementation Complexity**: Low-Medium
- Core: Exponential decay is simple (`v *= friction`)
- Advanced: Velocity tracking from gestures, boundary springs

**Real UI Usage**:
- Flick scrolling (iOS Contacts, Safari)
- Swipe-to-dismiss cards
- Drawer/sheet momentum
- Drag-and-release interactions

**Why Add It**:
- Cannot be built from spring (different physics model)
- Essential for modern touch UIs
- Complements spring physics for release gestures

**Sources**:
- [The Mighty Framer Motion Guide - Inertia Animations](https://motion.mighty.guide/dragging/inertia-animations/)
- [Framer Motion inertia docs](https://www.framer.com/motion/transition/)
- [Flick list with momentum scrolling and deceleration](https://ariya.io/2011/10/flick-list-with-its-momentum-scrolling-and-deceleration)
- [JavaScript Kinetic Scrolling](https://ariya.io/2013/08/javascript-kinetic-scrolling-part-1)
- [Creating Animations with Physical Models](https://iamralpht.github.io/physics/)

### 1.2 Gravity / Projectile Motion ⭐⭐

**Status**: Can be built on top, but could be utility

**What It Is**: Constant acceleration downward (or any direction). Objects fall, bounce.

**Implementation**: Acceleration applied per frame: `velocity += gravity * dt`

**Real UI Usage**:
- Particle effects (confetti, balloons)
- Notification "drops"
- Rare in standard UI components

**Verdict**: Optional utility function, not core primitive. Can be simulated with custom spring or easing.

### 1.3 Friction / Drag ⭐⭐⭐⭐

**Status**: FUNDAMENTAL (see Inertia above)

Friction is the core of inertia/decay. Already covered in 1.1.

### 1.4 Pendulum ⭐

**Status**: Not needed

**What It Is**: Oscillating motion around pivot point.

**Verdict**: Too specialized. Can be approximated with spring or custom easing. Rarely used in UI outside of playful animations.

### 1.5 Particle Systems ⭐⭐

**Status**: NOT engine-level

**What It Is**: Managing many animated particles (position, velocity, lifetime, color).

**Verdict**: This is a *system built on top* of primitives, not a primitive itself. Each particle uses springs/easing/decay. Out of scope for core engine.

### 1.6 Fluid Dynamics / Morphing ⭐

**Status**: Not engine-level (see SVG morphing in Section 2.3)

Complex simulation. Not a primitive. SVG morphing is separate (covered below).

### 1.7 Matter.js / Physics Engines ⭐⭐

**Status**: NOT engine-level for UI framework

**What They Provide**:
- Rigid body dynamics (collision, gravity, friction)
- Constraints (springs, hinges, distance)
- Continuous collision detection

**Verdict**: Overkill for 99% of UI animations. Matter.js is for games or playful website effects (bouncing logos, interactive backgrounds). UI frameworks need springs and inertia, not full physics simulations.

**Sources**:
- [Matter.js 2D physics engine](https://brm.io/matter-js/)
- [Introduction to Matter.js](https://medium.com/@codenova/an-introduction-to-matter-js-f91f3ec871a9)

---

## 2. Path-Based Animation

### 2.1 Motion Along SVG Path ⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Should be added

**What It Is**: Animate element position along an SVG `<path>` or bezier curve.

**Technical Details**:
- Input: SVG path data (`d` attribute) or array of points
- Output: `(x, y)` coordinates at progress `t` (0..1)
- Features:
  - Auto-rotate (element faces direction of path)
  - Align (position element relative to path - center, edge)
  - Speed normalization (constant speed despite control points)

**GSAP MotionPathPlugin**:
- `gsap.to(element, { motionPath: { path: "#svg-path", align: "self", autoRotate: true } })`
- Converts SVG path to raw cubic bezier data

**AnimeJS**:
- `anime.path('#svg-path')` returns `{ x, y }` functions for progress

**Implementation Complexity**: Medium-High
- SVG path parsing (cubic/quadratic bezier)
- Arc length parameterization (for constant speed)
- Tangent calculation (for auto-rotate)

**Real UI Usage**:
- Icon animations (checkmark drawing along path)
- Curved navigation transitions
- Logo reveals
- Data visualization (animated line charts)

**Why Add It**:
- Cannot be built from linear interpolation
- Common request for advanced UI animations
- Industry standard (GSAP, AnimeJS both have it)

**Sources**:
- [GSAP MotionPathPlugin](https://gsap.com/docs/v3/Plugins/MotionPathPlugin/)
- [GSAP SVG Motion Path tutorial](https://davidwalsh.name/gsap-svg)
- [AnimeJS createMotionPath](https://animejs.com/documentation/svg/)

### 2.2 Bezier Motion Paths ⭐⭐⭐⭐

**Status**: Same as 2.1 (SVG paths ARE bezier curves)

SVG paths use cubic/quadratic bezier internally. This is the same primitive.

### 2.3 SVG Morphing ⭐⭐⭐

**Status**: SPECIALIZED - Probably out of scope

**What It Is**: Interpolate between two SVG `<path>` shapes (shape A → shape B).

**Technical Details**:
- Requires equal number of points on both paths
- Interpolate each point pair
- GSAP MorphSVGPlugin (premium): handles point count mismatch
- AnimeJS `morphTo()`: basic morphing with precision parameter

**Implementation Complexity**: High
- Path normalization (matching point counts)
- Heuristics for "best" point matching
- Curved path interpolation (not just linear points)

**Real UI Usage**:
- Icon state transitions (hamburger → X, play → pause)
- Shape-based loading animations
- Data visualization shape changes

**Verdict**: Useful but niche. Complex to implement well. Consider later if demand exists.

**Sources**:
- [AnimeJS morphTo](https://animejs.com/documentation/svg/morphto/)
- [SVG Shape Morphing with AnimeJS](https://www.designdrastic.com/tutorial/svg-shape-morphing-using-anime-js)

### 2.4 Line Drawing (Stroke Animation) ⭐⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Should be added

**What It Is**: Progressive reveal of SVG stroke using `stroke-dashoffset` and `stroke-dasharray`.

**Technical Details**:
```css
/* Setup */
stroke-dasharray: pathLength; /* e.g., 1000 */
stroke-dashoffset: pathLength; /* Start hidden */

/* Animate to reveal */
stroke-dashoffset: 0; /* Fully visible */
```

**How It Works**:
- `getTotalLength()` gets path length
- Set both dasharray and dashoffset to path length
- Animate dashoffset from pathLength → 0

**GSAP DrawSVG**:
- `gsap.from(path, { drawSVG: "0%" })` (0% → 100%)
- Can draw from middle: `drawSVG: "50% 50%"` → `"0% 100%"`

**Implementation Complexity**: Low
- Just animate a single number (dashoffset)
- Need `getTotalLength()` equivalent or pre-computed length

**Real UI Usage**:
- Signature/handwriting animations
- Logo reveals
- Animated underlines
- Checkmark animations
- Loading spinners

**Why Add It**:
- Extremely common UI pattern
- Simple to implement
- High visual impact

**Sources**:
- [How SVG Line Animation Works](https://css-tricks.com/svg-line-animation-works/)
- [Animated line drawing in SVG](https://jakearchibald.com/2013/animated-line-drawing-svg/)
- [GSAP DrawSVG Plugin](https://gsap.com/docs/v3/Plugins/DrawSVGPlugin/)

---

## 3. Value Types / Interpolation

### 3.1 Color Interpolation (RGB vs HSL vs OKLCH) ⭐⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Should be added

**What It Is**: Perceptually uniform color transitions.

**The Problem with RGB**:
- RGB interpolation creates muddy midpoints
- Uneven brightness in gradients
- Hue shifts in unexpected ways

**OKLCH Advantages**:
- Perceptual uniformity (equal numeric changes = equal visual changes)
- No hue drift when changing lightness
- Smoother gradients (no gray zones)
- Oklab color space designed for interpolation

**Technical Details**:
- OKLCH: `oklch(L C H)` - Lightness (0-100%), Chroma, Hue (0-360deg)
- Default in Web Animations API (as of 2024+)
- ColorAide library: multiple interpolation spaces

**Implementation Complexity**: Medium
- Need OKLCH ↔ RGB conversion
- Hue interpolation (shortest path around 360deg circle)

**Real UI Usage**:
- Theme transitions
- Gradient animations
- Data visualization (heatmaps, color scales)
- Button hover states

**Why Add It**:
- Standard interpolation (sRGB) looks bad
- Industry moving to perceptual color spaces
- CSS default is now Oklab

**Sources**:
- [OKLCH Color Picker](https://oklch.fyi/)
- [Color experiments with OKLCH](https://clhenrick.io/blog/color-experiments-with-oklch/)
- [Oklab - A perceptual color space](https://bottosson.github.io/posts/oklab/)
- [Color Interpolation - ColorAide](https://facelessuser.github.io/coloraide/interpolation/)

### 3.2 Matrix / Transform Interpolation ⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Likely already needed for transforms

**What It Is**: Decompose transform matrix → interpolate components → recompose.

**The Problem**:
Naive matrix interpolation causes "flipping" and weird motion when rotation is involved.

**Solution - Decomposition**:
```
Matrix → Decompose → [translate, rotate, scale, skew]
Interpolate each component separately
Recompose → Matrix
```

**Technical Details**:
- W3C standard: "unmatrix" method from Graphics Gems II
- Decomposition order: translate → rotate → skew → scale
- Polar decomposition (expensive but accurate)

**Implementation Complexity**: Medium-High
- QR decomposition for scale/shear and rotation
- Handling non-commutative transforms
- Avoiding gimbal lock

**Real UI Usage**:
- Every CSS transform animation
- 3D rotations
- Complex layout transitions

**Verdict**: This is likely already implemented if you support transform animations. If not, it's critical.

**Sources**:
- [W3C FX 2D Transforms](https://dev.w3.org/Graphics-FX/modules/2D-transforms/spec/2DTransforms.html)
- [Decomposing 2D transformation matrix](https://gist.github.com/fwextensions/2052247)
- [Matrix Decomposition - Gabor Makes Games](https://gabormakesgames.com/blog_decomposition.html)

### 3.3 Gradient Interpolation ⭐⭐

**Status**: Can be built on top (using CSS @property)

**What It Is**: Animating between gradients (different colors/stops).

**The Problem**:
Browsers don't natively transition gradients smoothly.

**Solutions**:
- CSS `@property` (Houdini): Define gradient stop colors as custom properties, interpolate
- Pseudo-element with opacity crossfade
- JavaScript interpolation

**Complexity**: Medium (requires @property support or workarounds)

**Verdict**: Not a core primitive. Can be built with color interpolation + CSS @property. Specialized use case.

**Sources**:
- [We can finally animate CSS gradient](https://dev.to/afif/we-can-finally-animate-css-gradient-kdk)
- [Smooth CSS Gradient Transitions](https://fjolt.com/article/css-animating-transitions-gradients)

### 3.4 Blur / Filter Interpolation ⭐⭐⭐

**Status**: Simple interpolation, but performance matters

**What It Is**: Smooth blur radius changes (`filter: blur(0px)` → `blur(20px)`).

**Implementation**: Just interpolate the radius number.

**Performance Issues**:
- Blur is GPU-intensive (convolution filter)
- Smooth under 20px
- Above 20px causes rendering lag
- Large elements + large blur = expensive

**Optimization**:
- Pre-compute blurred copies, crossfade with opacity (faster than animating blur value)
- Use `will-change: filter` for GPU acceleration

**Verdict**: Simple primitive (just a number), but document performance warnings.

**Sources**:
- [Animating a blur](https://developer.chrome.com/blog/animated-blur)
- [CSS Blur Effect Examples](https://www.sliderrevolution.com/resources/css-blur-effect/)

---

## 4. Procedural / Noise-Based

### 4.1 Perlin Noise ⭐⭐⭐

**Status**: UTILITY - Not core engine, but useful

**What It Is**: Organic pseudo-random values that change smoothly over time/space.

**Technical Details**:
- Input: `(x, y, z)` or `(x, y, time)`
- Output: Smooth noise value (-1..1 or 0..1)
- Creates natural-looking randomness (not white noise)

**Implementation Complexity**: Medium
- Classic Perlin or Simplex noise algorithm
- 1D/2D/3D variants
- Octave layering (fractal noise)

**Real UI Usage**:
- Camera shake (randomize position with noise)
- "Breathing" animations (slight scale/position drift)
- Particle motion (rain, snow, confetti)
- Organic backgrounds
- Floating elements

**Why Consider It**:
- Creates "alive" feeling
- Smooth randomness (better than `Math.random()`)
- Common in creative coding

**Why NOT Core Engine**:
- Not fundamental to animation sequencing
- Can be implemented as utility function
- Not needed for 90% of UI work

**Verdict**: Consider as **separate utility module**, not core engine primitive.

**Sources**:
- [Perlin Noise - Wikipedia](https://en.wikipedia.org/wiki/Perlin_noise)
- [The Book of Shaders: Noise](https://thebookofshaders.com/11/)
- [Noise in Creative Coding](https://varun.ca/noise/)
- [Perlin Noise in Animation](https://research.cs.wisc.edu/graphics/Courses/cs-838-1999/Students/fruit/final_writeup.html)

### 4.2 Simplex Noise ⭐⭐⭐

**Status**: Same as Perlin (4.1)

Simplex is Perlin's successor (faster, fewer artifacts). If you implement noise, use Simplex.

### 4.3 Wobble / Jitter ⭐⭐⭐

**Status**: UTILITY - Built on noise or randomness

**What It Is**: Controlled randomness to make things feel "alive" or "hand-drawn".

**Techniques**:
- **Wiggle** (After Effects): `wiggle(frequency, amplitude)` - random jitter
- **Boiling lines**: Slight position shifts per frame (hand-drawn effect)
- **Wobble spring**: Spring with slight random perturbations

**Implementation**: Use Perlin noise or randomized keyframes

**Real UI Usage**:
- Hand-drawn animation style
- "Alive" idle animations (character breathing)
- Sketch-style graphics

**Verdict**: **Utility function** built on noise (4.1). Not core primitive.

**Sources**:
- [Wiggle Expression - After Effects](https://www.plainlyvideos.com/after-effects-expressions-library/wiggle)
- [Making components go alive with UIKit animations](https://medium.com/better-programming/making-components-go-alive-using-uikit-animations-24fa11d19c02)

### 4.4 Brownian Motion ⭐

**Status**: Too specialized

Random walk. Rarely used in UI. Perlin noise covers this use case better.

---

## 5. Constraint-Based

### 5.1 Follow / Attach ⭐⭐⭐⭐

**Status**: Can be built on top, but common pattern

**What It Is**: Element B follows element A with delay/spring.

**Techniques**:
- **Spring-based**: B's position springs toward A's position
- **Delay-based**: B copies A's position with time delay

**Implementation**: Apply spring/delay to target's position

**Real UI Usage**:
- Cursor followers (custom cursor with trail)
- Tooltip following mouse
- Draggable connected elements (flowchart nodes with edges)

**Verdict**: **Utility pattern**, not core primitive. Can be built with existing springs + position tracking.

### 5.2 Magnetic Snap ⭐⭐⭐⭐

**Status**: Can be built on top

**What It Is**: Element snaps to grid/anchor points when dragged nearby.

**Implementation**:
```rust
if distance_to_snap_point < threshold {
    spring_to(snap_point)
}
```

**Real UI Usage**:
- Drag-and-drop (snap to grid)
- Slider snap points
- Magnetic buttons (cursor attracts element)

**Verdict**: **Utility pattern**. Spring + distance check.

**Sources**:
- [Framer Motion drag snap points](https://sinja.io/blog/framer-motion-drag-snap-points)
- [Create magnetic effect with Motion/GSAP](https://motion.page/learn/magnetic-effect-button/)

### 5.3 Elastic Connections ⭐⭐

**Status**: Can be built with springs

**What It Is**: Two elements connected by virtual spring (distance constraint).

**Verdict**: Built with spring physics. Not a new primitive.

### 5.4 Parallax ⭐⭐⭐⭐

**Status**: Can be built on top, but could be utility

**What It Is**: Multi-layer depth effect - background moves slower than foreground.

**Implementation**:
```rust
for layer in layers {
    layer.offset = scroll_position * layer.depth_factor;
}
```

**Technical Details**:
- Depth factor: `0.0` (static background) to `1.0` (foreground)
- Can use `transform: translateZ()` for 3D parallax
- Scroll-linked or mouse-linked

**Real UI Usage**:
- Scroll parallax backgrounds
- Mouse-move depth effects
- Hero sections

**Verdict**: **Utility function**. Simple multiplication, not core primitive.

**Sources**:
- [Create parallax effect with layer depth](https://helpx.adobe.com/animate/using/layer-depth.html)
- [The Parallax Effect](https://garagefarm.net/blog/parallax-effect-best-practices-and-examples)
- [Use parallax to add depth - Windows apps](https://learn.microsoft.com/en-us/windows/apps/design/motion/parallax)

---

## 6. Blend / Composition

### 6.1 Animation Blending ⭐⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Should be added

**What It Is**: Crossfade between two animations (like game animation blending).

**Unity Blend Trees**:
- Smoothly blend between walk/run animations based on speed parameter
- Weight-based blending (70% idle + 30% walk)

**Implementation**:
```rust
result = animation_a.value * weight_a + animation_b.value * weight_b;
// where weight_a + weight_b = 1.0
```

**Real UI Usage**:
- Smooth state transitions (button idle → hover)
- Interrupting animations (start new animation mid-current)
- Gesture-driven blending (scrub between states)

**Why Add It**:
- Essential for smooth interruptions
- Game engines all have this
- Enables complex transitions

**Sources**:
- [Unity Animation Blend Trees](https://docs.unity3d.com/Manual/class-BlendTree.html)
- [Unreal Animation Blend Nodes](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-blueprint-blend-nodes-in-unreal-engine)

### 6.2 Additive Animation ⭐⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Should be added

**What It Is**: Multiple animations stack on same property instead of replacing.

**How It Works**:
```
Base value: translateX(50px)
Additive animation: translateX(100px)
Result: translateX(150px) [50 + 100]
```

**vs. Regular (Replace)**:
```
Base value: translateX(50px)
Replace animation: translateX(100px)
Result: translateX(100px) [replaces base]
```

**Web Animations API**:
- `composite: "replace"` (default)
- `composite: "add"` (additive)
- `composite: "accumulate"` (combines values intelligently)

**Real UI Usage**:
- Layered animations (idle wobble + user interaction)
- Adding detail to base animation (head nod + eye blink)
- Simultaneous independent effects (shake + fade)

**Why Add It**:
- Industry standard (Unity, Unreal, Web Animations API)
- Enables animation layering
- Critical for complex character/UI animation

**Sources**:
- [Additive Animation with Web Animations API](https://css-tricks.com/additive-animation-web-animations-api/)
- [CSS animation-composition](https://12daysofweb.dev/2023/animation-composition)
- [animation-composition - MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/animation-composition)
- [Using Layered Animations - Unreal](https://docs.unrealengine.com/4.27/en-US/AnimatingObjects/SkeletalMeshAnimation/AnimHowTo/AdditiveAnimations)

### 6.3 Animation Layers ⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Related to additive

**What It Is**: Priority system for animations (base + override layers).

**Unity Layers**:
- Base Layer: Full body idle
- Upper Body Layer: Shooting animation (override upper body only)
- Result: Running + shooting simultaneously

**Layer Properties**:
- Weight (0..1)
- Blending mode (Override or Additive)
- Mask (which properties/bones affected)

**Implementation**:
```rust
for layer in layers.sorted_by_priority() {
    if layer.mode == Override {
        value = layer.value * layer.weight;
    } else { // Additive
        value += layer.value * layer.weight;
    }
}
```

**Verdict**: Extension of additive animation (6.2). Critical for complex animation systems.

**Sources**:
- [Unity Animation Layers](https://docs.unity3d.com/550/Documentation/Manual/AnimationLayers.html)
- [Animancer - Layers](https://kybernetik.com.au/animancer/docs/manual/blending/layers/)

### 6.4 Interruption Handling ⭐⭐⭐⭐⭐

**Status**: FUNDAMENTAL - Should be added

**What It Is**: Behavior when starting new animation mid-current animation.

**Strategies**:
1. **Instant**: Jump to new animation (jarring)
2. **Blend**: Crossfade over time (smooth)
3. **Queue**: Wait for current to finish
4. **Interrupt + Continue Velocity**: New animation inherits current velocity (momentum)

**Unreal Inertialization**:
- Tracks pose velocity
- New animation inherits momentum for smooth transitions

**Framer Motion**:
- Automatic velocity inheritance in springs
- Smooth interruptions by default

**Real UI Usage**:
- Rapid button clicks (hover → press → release)
- Gesture interruptions (start swipe, change direction)
- State machine transitions

**Why Add It**:
- Determines animation "feel"
- Essential for responsive UI
- Prevents jarring jumps

**Verdict**: Core engine behavior. Needs configuration per animation.

**Sources**:
- [Animation Blend Nodes - Unreal](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-blueprint-blend-nodes-in-unreal-engine)
- [Framer Motion transitions](https://www.framer.com/motion/transition/)

---

## 7. State Machines

### 7.1 Animation State Machine ⭐⭐⭐⭐⭐

**Status**: HIGH-LEVEL SYSTEM - Not core primitive, but extremely valuable

**What It Is**: Graph of animation states with transition rules.

**Unity Mecanim**:
- States: Idle, Walk, Run, Jump
- Transitions: Conditions (speed > 5 → Run)
- Blend trees within states

**Rive State Machine**:
- Visual editor for animation logic
- Inputs (Number, Boolean, Trigger)
- Layers (base, override)

**Components**:
- States (timeline animation or blend tree)
- Transitions (conditions, duration, blend curve)
- Parameters (speed, is_grounded, etc.)

**Real UI Usage**:
- Button states (idle → hover → press → release)
- Loading states (idle → loading → success → error)
- Character animations
- Interactive mascots

**Why NOT Core Engine**:
- This is a *system built on top* of timeline/blending primitives
- Belongs in higher-level library (like `uzor-state-machine` crate)

**Why Still Valuable**:
- Industry standard pattern
- Simplifies complex animation logic
- Declarative vs imperative

**Verdict**: **Separate module** (not core engine). Build after blending/layers are solid.

**Sources**:
- [Unity Animation State Machines](https://docs.unity3d.com/Manual/AnimationStateMachines.html)
- [Rive State Machine Overview](https://help.rive.app/editor/state-machine)
- [Beginner's guide to Rive State Machine](https://rive.app/blog/how-state-machines-work-in-rive)

### 7.2 Gesture-Driven Animation ⭐⭐⭐⭐

**Status**: HIGH-LEVEL - Built on core primitives

**What It Is**: Animation progress tied to gesture input (drag, scroll).

**Examples**:
- Swipe-to-delete: Drag distance → animation progress (0..1)
- Scroll-linked: Scroll position → parallax/reveal progress
- Scrubbing: Drag to scrub through timeline

**Implementation**:
```rust
let progress = gesture_position / max_distance;
timeline.seek(progress); // 0..1
```

**Verdict**: **High-level pattern**. Core engine provides `timeline.seek()`, app handles gesture mapping.

### 7.3 Scroll-Linked Animation ⭐⭐⭐⭐⭐

**Status**: HIGH-LEVEL, but CSS now has native support

**What It Is**: Animation driven by scroll position (not time).

**CSS Scroll-Driven Animations** (2024-2026):
```css
@keyframes slide {
  from { transform: translateX(0); }
  to { transform: translateX(100px); }
}

.element {
  animation: slide;
  animation-timeline: scroll(root); /* NEW */
}
```

**Timeline Types**:
- **Scroll Progress**: 0% at top, 100% at bottom
- **View Progress**: Animation based on element's scroll position in viewport

**Real UI Usage**:
- Parallax backgrounds
- Scroll-triggered reveals
- Progress indicators
- Apple-style product showcases

**Why Important**:
- Becoming CSS standard (Chrome 145, Firefox with flag)
- Performant (runs off main thread)
- Common UI pattern

**For Rust UI Framework**:
- Provide scroll-linked timeline (not time-based)
- Map scroll position → animation progress

**Verdict**: **High-level feature**, but important enough to support natively.

**Sources**:
- [CSS scroll-driven animations - MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Scroll-driven_animations)
- [Scroll-driven animation timelines](https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Scroll-driven_animations/Timelines)
- [Scroll-driven Animations Module Level 1](https://drafts.csswg.org/scroll-animations-1/)
- [Animate elements on scroll - Chrome](https://developer.chrome.com/docs/css-ui/scroll-driven-animations)

---

## 8. Game Engine Primitives

### 8.1 Unity Mecanim ⭐⭐⭐⭐

Already covered in State Machines (7.1) and Blending (6.1-6.3).

**Core Primitives**:
- Animation clips (like our timeline)
- Blend trees (weighted blending)
- State machines (graph of states + transitions)
- Layers (override/additive)

**Verdict**: Most of these map to our needs. Blending + layers are the missing pieces.

**Sources**:
- [Unity Mecanim Animation System](https://docs.unity3d.com/Manual/AnimationOverview.html)
- [Unity Animation States](https://docs.unity3d.com/6000.2/Documentation/Manual/class-State.html)

### 8.2 Unreal Sequencer ⭐⭐⭐

**What It Is**: Timeline-based animation editor (like GSAP Timeline).

We already have timeline module. Unreal's blending is covered in 6.1-6.4.

### 8.3 Godot Tween vs AnimationPlayer ⭐⭐⭐⭐

**Godot Tween**:
- Code-driven interpolation
- Dynamic start/end values
- Like our spring/easing modules

**Godot AnimationPlayer**:
- Pre-authored animation tracks
- Like our timeline module

**Key Insight**:
- **Tween** = dynamic (values unknown at design time)
- **AnimationPlayer** = static (pre-defined keyframes)

**Verdict**: We have both patterns (spring/easing = tween, timeline = animation player).

**Sources**:
- [Godot Tween docs](https://docs.godotengine.org/en/stable/classes/class_tween.html)
- [Tween vs AnimationPlayer](https://forum.godotengine.org/t/what-is-the-difference-between-a-tween-and-an-animationplayer/22560)

---

## Summary: What Should We Add?

### Tier 1: CRITICAL (Add to Core Engine)

| Primitive | Why | Complexity | Impact |
|-----------|-----|------------|--------|
| **Inertia/Decay** | Essential for touch UIs, flick scrolling | Medium | High |
| **Color Interpolation (OKLCH)** | Perceptual uniformity, industry standard | Medium | High |
| **Line Drawing (SVG stroke)** | Extremely common, simple, high impact | Low | High |
| **Additive Animation** | Industry standard, enables layering | Medium | High |
| **Animation Blending** | Smooth interruptions, state transitions | Medium | High |
| **Interruption Handling** | Determines animation "feel" | Medium | High |

### Tier 2: IMPORTANT (Add Soon)

| Primitive | Why | Complexity | Impact |
|-----------|-----|------------|--------|
| **Motion Along Path** | Common request, advanced animations | High | Medium |
| **Animation Layers** | Extension of additive, complex systems | Medium | Medium |
| **Scroll-Linked Timelines** | CSS standard, common pattern | Medium | High |

### Tier 3: UTILITY (Separate Modules)

| Primitive | Why | Module Name |
|-----------|-----|-------------|
| **Perlin/Simplex Noise** | Creative coding, organic motion | `uzor-noise` |
| **State Machine** | High-level system, built on blending | `uzor-state-machine` |
| **Parallax** | Simple utility, built on scroll tracking | `uzor-effects` |
| **Magnetic Snap** | Utility pattern, built on springs | `uzor-constraints` |

### Tier 4: NOT NEEDED

- Gravity (too specialized)
- Pendulum (too niche)
- Particle Systems (not engine-level)
- Matter.js physics (overkill for UI)
- SVG Morphing (complex, niche)
- Gradient interpolation (can use CSS @property)
- Brownian motion (Perlin noise covers this)

---

## Implementation Priority

### Phase 1: Physics & Interpolation
1. **Inertia/Decay animation** - Essential for modern UI
2. **OKLCH color interpolation** - Better than sRGB
3. **SVG line drawing** - High impact, low complexity

### Phase 2: Composition
4. **Additive animation** - Foundation for layering
5. **Animation blending** - Crossfade between animations
6. **Interruption handling** - Smooth transitions

### Phase 3: Advanced
7. **Animation layers** - Priority system
8. **Motion along path** - Advanced path animations
9. **Scroll-linked timelines** - Scroll-driven animations

### Phase 4: Utilities (Separate Crates)
10. **Noise module** (`uzor-noise`)
11. **State machine** (`uzor-state-machine`)
12. **Effects library** (`uzor-effects`: parallax, magnetic, etc.)

---

## Key Insights

1. **Blending is the biggest gap** - Game engines have sophisticated blending (additive, layers, crossfade). We need this for complex UI.

2. **Inertia is fundamental** - Cannot be built from springs. Essential for touch UIs. Easy win.

3. **OKLCH is the future** - CSS is moving to perceptual color spaces. We should too.

4. **State machines are high-level** - Not a core primitive, but extremely valuable as separate module.

5. **Most "constraint" patterns are utilities** - Follow, snap, parallax can be built with existing primitives + helper functions.

6. **Game engines teach us a lot** - Unity/Unreal have solved animation composition. We should study their patterns (blend trees, layers, state machines).

7. **SVG animations are table stakes** - Path motion and line drawing are expected in modern UI frameworks.

---

## Sources

### Animation Engines
- [Framer Motion](https://www.framer.com/motion/)
- [GSAP Documentation](https://gsap.com/docs/)
- [AnimeJS Documentation](https://animejs.com/documentation/)

### Physics
- [Creating Animations with Physical Models](https://iamralpht.github.io/physics/)
- [JavaScript Kinetic Scrolling](https://github.com/ariya/kinetic)
- [Matter.js Physics Engine](https://brm.io/matter-js/)

### Color Science
- [OKLCH Color Picker](https://oklch.fyi/)
- [Oklab - A perceptual color space](https://bottosson.github.io/posts/oklab/)
- [ColorAide Documentation](https://facelessuser.github.io/coloraide/)

### Web Standards
- [Web Animations API - MDN](https://developer.mozilla.org/en-US/docs/Web/API/Web_Animations_API)
- [CSS Scroll-Driven Animations](https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Scroll-driven_animations)
- [CSS animation-composition](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/animation-composition)

### Game Engines
- [Unity Mecanim System](https://docs.unity3d.com/Manual/AnimationOverview.html)
- [Unreal Animation Blend Nodes](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-blueprint-blend-nodes-in-unreal-engine)
- [Godot Tween Documentation](https://docs.godotengine.org/en/stable/classes/class_tween.html)
- [Rive State Machine](https://help.rive.app/editor/state-machine)

### SVG Animation
- [How SVG Line Animation Works](https://css-tricks.com/svg-line-animation-works/)
- [GSAP MotionPathPlugin](https://gsap.com/docs/v3/Plugins/MotionPathPlugin/)
- [Animated line drawing in SVG](https://jakearchibald.com/2013/animated-line-drawing-svg/)

### Noise & Procedural
- [The Book of Shaders: Noise](https://thebookofshaders.com/11/)
- [Noise in Creative Coding](https://varun.ca/noise/)
- [Perlin Noise in Animation](https://research.cs.wisc.edu/graphics/Courses/cs-838-1999/Students/fruit/final_writeup.html)
