# Timeline Architecture Research

## Overview

Analysis of timeline/sequencing systems from GSAP, AnimeJS, Motion One, and vector animation formats (Rive/Lottie). Goal: design minimal, performant timeline architecture for uzor-animation.

## 1. GSAP Timeline

**Documentation:** https://gsap.com/docs/v3/GSAP/gsap.timeline()

**Learning Center:** https://gsap.com/community/position-parameter/

### Core Concept

GSAP timeline is a **container for animations** that controls their playback as a group.

**Key features:**
- Sequence animations
- Control multiple tweens as one
- Repeat/yoyo entire sequences
- Nest timelines within timelines

### Position Parameter (The Magic)

**Default:** `"+=0"` - Inserts at end of timeline

**The position parameter controls WHEN a tween is inserted:**

**Absolute time:**
```javascript
tl.to("#box", { x: 100, duration: 1 }, 3);  // At exactly 3 seconds
```

**Relative to end (gap):**
```javascript
tl.to("#box", { x: 100 }, "+=1");  // 1 second after previous ends
```

**Relative to end (overlap):**
```javascript
tl.to("#box", { x: 100 }, "-=0.5");  // 0.5 seconds before previous ends (overlap)
```

**Labels:**
```javascript
tl.addLabel("scene1")
  .to("#box", { x: 100 }, "scene1")        // At label
  .to("#box2", { y: 50 }, "scene1+=1");    // 1 second after label
```

**Relative to start:**
```javascript
tl.to("#box", { x: 100 }, "<");     // Start of previous tween
tl.to("#box", { x: 100 }, "<+=0.5"); // 0.5s after start of previous
```

**Percentage of previous:**
```javascript
tl.to("#box", { x: 100 }, "<50%");   // 50% through previous tween
```

### Implementation Details

**TypeScript definition:**
```typescript
interface Timeline {
    to(targets: TweenTarget, vars: TweenVars, position?: Position): this;
    from(targets: TweenTarget, vars: TweenVars, position?: Position): this;
    addLabel(label: string, position?: Position): this;
}

type Position = number | string;
```

**Position parsing algorithm:**
1. If number: absolute time
2. If string starting with "+=" or "-=": relative to timeline end
3. If "<": relative to previous tween start
4. If label: lookup label time
5. If label with offset (e.g., "scene1+=2"): label time + offset

### Overlapping Tweens Example

```javascript
const tl = gsap.timeline();

tl.to("#green", { duration: 2, x: 750 })
  .to("#blue", { duration: 2, x: 750 }, "-=1")    // Overlap by 1s
  .to("#orange", { duration: 2, x: 750 }, "-=1"); // Overlap by 1s
```

**Timeline structure:**
- `#green`: 0s → 2s
- `#blue`: 1s → 3s (starts 1s before green ends)
- `#orange`: 2s → 4s (starts 1s before blue ends)

### Nested Timelines

```javascript
const innerTL = gsap.timeline();
innerTL.to("#box1", { x: 100 })
       .to("#box2", { y: 100 });

const masterTL = gsap.timeline();
masterTL.add(innerTL, 2)  // Insert entire inner timeline at 2s
        .to("#box3", { rotation: 360 });
```

Nested timelines maintain their own playback state but are controlled by parent.

### How GSAP Handles Overlapping Tweens

**Timeline maintains:**
- Sorted array of tweens by start time
- Each tween knows its `startTime` and `duration`
- Timeline's playhead position

**On tick:**
```javascript
function tick(currentTime) {
    for (let tween of tweens) {
        if (currentTime >= tween.startTime &&
            currentTime <= tween.startTime + tween.duration) {
            const localTime = currentTime - tween.startTime;
            tween.render(localTime);
        }
    }
}
```

Multiple tweens can be active simultaneously, each rendering at their local time.

### Source Code Structure

**TypeScript definitions:** https://github.com/greensock/GSAP/blob/master/types/timeline.d.ts

**Key methods:**
- `.to()`, `.from()`, `.fromTo()` - Add tweens
- `.add()` - Add tween or timeline
- `.addLabel()` - Add time marker
- `.set()` - Instant value change (duration: 0)
- `.call()` - Execute callback

**Playback control:**
- `.play()`, `.pause()`, `.resume()`
- `.reverse()` - Play backwards
- `.seek(time)` - Jump to time
- `.progress(ratio)` - Jump to percentage (0-1)
- `.timeScale(factor)` - Speed multiplier

**Timeline properties:**
- `.duration()` - Total timeline length
- `.time()` - Current playhead position
- `.paused()` - Is paused?

### Data Structure (Inferred)

```rust
struct Timeline {
    tweens: Vec<TimelineTween>,
    labels: HashMap<String, f32>,
    duration: f32,
    playhead: f32,
    time_scale: f32,
    paused: bool,
}

struct TimelineTween {
    tween: Tween,
    start_time: f32,
    duration: f32,
}
```

**Sorted invariant:** `tweens` maintained sorted by `start_time` for efficient seek operations.

## 2. AnimeJS Timeline

**Documentation:** https://animejs.com/documentation/

**Repository:** https://github.com/juliangarnier/anime

### Approach vs GSAP

**Similarities:**
- Sequences animations
- Controls group playback
- Supports overlapping

**Differences:**
- Simpler API (fewer position options)
- Offset parameter instead of position string
- No percentage-based positioning

### Timeline Creation

```javascript
const tl = anime.timeline({
    easing: 'easeOutExpo',
    duration: 750
});
```

Default easing and duration apply to all child animations unless overridden.

### Adding Animations

**Relative offset:**
```javascript
tl.add({
    targets: '.box1',
    translateX: 250
})
.add({
    targets: '.box2',
    translateX: 250
}, '-=500');  // Start 500ms before previous ends
```

**Absolute offset:**
```javascript
tl.add({
    targets: '.box3',
    translateX: 250
}, 1000);  // Start at 1000ms
```

### Position Parameter

AnimeJS supports:
- `number` - Absolute time in ms
- `string` - Relative offset: `"+=500"`, `"-=500"`

**Does NOT support:**
- Labels
- Percentage positioning
- Start-of-previous (`<`)

**Simpler than GSAP but less flexible.**

### Implementation Notes

Timeline likely maintains:
- Array of animations with start times
- Current playhead
- Inherited properties (easing, duration)

**On playback:**
- Iterate animations
- Check if in active range
- Render with local time offset

### Source Code Location

Main repository: https://github.com/juliangarnier/anime

Look for:
- Timeline class/function
- Animation sequencing logic
- Offset parsing

## 3. Motion One Timeline

**Documentation:** https://motion.dev/docs/migrate-from-gsap-to-motion

**Repository:** https://github.com/motiondivision/motionone

### Philosophy: Declarative vs Imperative

**GSAP's imperative approach:**
```javascript
const tl = gsap.timeline();
tl.to("#id", { x: 100, duration: 1 });
tl.addLabel("My label");
tl.to("#id", { y: 50, duration: 1 });
```

Build timeline progressively with methods.

**Motion One's declarative approach:**
```javascript
const timeline = [
    ["#id", { x: 100, duration: 1 }],
    "My label",
    ["#id", { y: 100, duration: 1 }]
];

animate(timeline, options);
```

Define entire timeline as data structure.

### Architecture Differences

**GSAP:**
- Timeline is mutable object
- Add animations dynamically
- Easier to modify during playback

**Motion One:**
- Timeline is immutable array
- Defined upfront
- Smaller code, less boilerplate

### Declarative Timeline Structure

```javascript
animate([
    [element, { x: 100 }, { duration: 1, at: 0 }],
    [element, { y: 100 }, { duration: 1, at: 0.5 }],  // Starts at 0.5s
    [element, { rotate: 90 }, { duration: 0.5, at: "-0.2" }]  // 0.2s before end
]);
```

**Position specified via `at` option:**
- Number: absolute time
- String: relative to previous (`"-0.2"` overlaps by 0.2s)

### Labels as Strings

```javascript
const timeline = [
    [el1, { x: 100 }],
    "scene1",              // Label
    [el2, { y: 100 }],
    [el3, { rotate: 90 }, { at: "scene1" }]  // Reference label
];
```

Labels are just strings in the array.

### Bundle Size Comparison

**GSAP Timeline:** ~28 KB minified

**Motion One `animate()`:** 18 KB minified (includes timeline sequencing)

**Motion One `mini` version:** 2.3 KB (no timeline, single animations only)

Motion is **37% smaller** for equivalent timeline functionality.

### Modern Browser API Integration

Motion One uses **Web Animations API (WAAPI)** under the hood, enabling:
- Browser-native animations
- Hardware acceleration
- Better performance for transforms/opacity
- Scroll-linked animations via Scroll Timeline API

GSAP uses custom rendering engine, giving more control but larger bundle.

### What's Different in Architecture

**GSAP:**
- Custom tick loop
- Frame-by-frame rendering
- Compatible with older browsers

**Motion One:**
- Delegates to WAAPI when possible
- Browser handles ticking/rendering
- Requires modern browsers

**Trade-off:** Motion is smaller and faster for modern browsers, GSAP is more compatible and flexible.

## 4. Rive vs Lottie Timeline Formats

### Lottie Format

**Wikipedia:** https://en.wikipedia.org/wiki/Lottie_(file_format)

**Format:** JSON (vector animation)

**Workflow:**
1. Design in Adobe After Effects
2. Export via Bodymovin plugin
3. Lottie player renders JSON

**Timeline structure:**
- **Fixed timeline** - Animations are baked keyframes
- **Time-based** - Frame-by-frame data
- **Easing functions** - Captured from After Effects
- **No state machines** - Pure playback

**JSON structure (simplified):**
```json
{
  "fr": 60,  // Frame rate
  "ip": 0,   // In point
  "op": 120, // Out point
  "layers": [
    {
      "ks": {  // Keyframes
        "p": {  // Position
          "a": 1,  // Animated
          "k": [   // Keyframe array
            {
              "t": 0,   // Time (frame)
              "s": [0, 0],  // Start value
              "e": [100, 100],  // End value
              "i": { "x": 0.42, "y": 0 },  // In tangent (easing)
              "o": { "x": 0.58, "y": 1 }   // Out tangent
            }
          ]
        }
      }
    }
  ]
}
```

**Timeline model:**
- Keyframes at specific frames
- Bezier easing between keyframes
- All timing predetermined

**File size:** JSON is verbose, but compresses well (gzip).

**Use case:** Playback of baked animations, like video but vector.

### Rive Format

**Format:** Lightweight binary

**Workflow:**
1. Design in Rive editor
2. Export .riv file
3. Rive runtime renders

**Timeline structure:**
- **State machines** - Logic-driven animation
- **States** - Each state can contain a timeline animation
- **Transitions** - Conditional switching between states
- **Parameters** - Runtime inputs (numbers, bools, triggers)

**State machine example:**
```
States:
  - idle: loop animation (frames 0-30)
  - hover: play once (frames 31-50)
  - click: play once (frames 51-70)

Transitions:
  - idle → hover: when mouseEnter
  - hover → idle: when mouseLeave
  - hover → click: when mouseDown
  - click → idle: on animation end
```

**Timeline vs State Machine:**
- **Timeline:** Fixed playback sequence
- **State machine:** Dynamic, reactive to inputs

**File size:** Binary format, 10-15× smaller than Lottie.

**Use case:** Interactive animations, UI components, games.

### Comparison for uzor-animation

**Lottie approach:**
- Good for: Non-interactive playback
- Simple to implement: keyframe array + bezier interpolation
- Data structure: `Vec<Keyframe>` per property

**Rive approach:**
- Good for: Interactive UI
- Complex to implement: state machine + transitions
- Data structure: State graph + timeline per state

**Recommendation for uzor-animation:**

Start with **Lottie-style keyframe timeline**:
- Simpler implementation
- Covers 90% of use cases
- Can layer state machine on top later

## 5. Minimal Timeline Data Structure

### Core Requirements

1. **Sequence animations** - Multiple animations in order
2. **Overlapping support** - Animations can run concurrently
3. **Labels** - Named time points for reference
4. **Seek/playback control** - Jump to time, play/pause
5. **Nesting** - Timelines within timelines

### Minimal Rust Implementation

```rust
pub struct Timeline {
    entries: Vec<TimelineEntry>,
    labels: HashMap<String, f32>,
    duration: f32,
    playhead: f32,
    speed: f32,
    playing: bool,
}

pub enum TimelineEntry {
    Tween {
        animation: Animation,
        start: f32,
        duration: f32,
    },
    Nested {
        timeline: Timeline,
        start: f32,
    },
    Callback {
        func: Box<dyn FnMut()>,
        time: f32,
    },
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            labels: HashMap::new(),
            duration: 0.0,
            playhead: 0.0,
            speed: 1.0,
            playing: false,
        }
    }

    pub fn add_tween(&mut self, animation: Animation, position: Position) -> &mut Self {
        let start = self.resolve_position(position);
        let duration = animation.duration();

        self.entries.push(TimelineEntry::Tween {
            animation,
            start,
            duration,
        });

        self.duration = self.duration.max(start + duration);
        self.sort_entries();
        self
    }

    pub fn add_label(&mut self, label: impl Into<String>, position: Position) -> &mut Self {
        let time = self.resolve_position(position);
        self.labels.insert(label.into(), time);
        self
    }

    fn resolve_position(&self, position: Position) -> f32 {
        match position {
            Position::Absolute(time) => time,
            Position::RelativeToEnd(offset) => self.duration + offset,
            Position::Label(label) => *self.labels.get(&label).unwrap_or(&0.0),
            Position::LabelOffset(label, offset) => {
                self.labels.get(&label).unwrap_or(&0.0) + offset
            }
        }
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| {
            a.start_time().partial_cmp(&b.start_time()).unwrap()
        });
    }

    pub fn tick(&mut self, delta: f32) {
        if !self.playing { return; }

        self.playhead += delta * self.speed;

        for entry in &mut self.entries {
            match entry {
                TimelineEntry::Tween { animation, start, duration } => {
                    if self.playhead >= *start && self.playhead <= start + duration {
                        let local_time = self.playhead - start;
                        animation.update(local_time);
                    }
                }
                TimelineEntry::Nested { timeline, start } => {
                    if self.playhead >= *start {
                        let local_time = self.playhead - start;
                        timeline.playhead = local_time;
                        timeline.tick(0.0);  // Don't add delta, set directly
                    }
                }
                TimelineEntry::Callback { func, time } => {
                    // Execute callback when crossing time point
                    if self.playhead >= *time {
                        func();
                    }
                }
            }
        }

        if self.playhead >= self.duration {
            self.playing = false;
        }
    }

    pub fn play(&mut self) { self.playing = true; }
    pub fn pause(&mut self) { self.playing = false; }
    pub fn seek(&mut self, time: f32) { self.playhead = time.clamp(0.0, self.duration); }
}

pub enum Position {
    Absolute(f32),
    RelativeToEnd(f32),  // +=1.0 or -=0.5
    Label(String),
    LabelOffset(String, f32),
}
```

### Builder API

```rust
impl Timeline {
    pub fn at(time: f32) -> Position {
        Position::Absolute(time)
    }

    pub fn offset(delta: f32) -> Position {
        Position::RelativeToEnd(delta)
    }

    pub fn label(name: impl Into<String>) -> Position {
        Position::Label(name.into())
    }
}

// Usage
let mut tl = Timeline::new();
tl.add_tween(anim1, Timeline::at(0.0))
  .add_label("scene1", Timeline::offset(0.0))
  .add_tween(anim2, Timeline::label("scene1"))
  .add_tween(anim3, Timeline::offset(-0.5));  // Overlap by 0.5s
```

### Declarative Alternative (Motion One Style)

```rust
#[derive(Clone)]
pub enum TimelineItem {
    Tween(Animation, Option<f32>),  // (animation, optional_start_time)
    Label(String),
    Nested(Timeline),
}

impl Timeline {
    pub fn from_sequence(items: Vec<TimelineItem>) -> Self {
        let mut tl = Timeline::new();
        let mut cursor = 0.0;

        for item in items {
            match item {
                TimelineItem::Tween(anim, start_time) => {
                    let start = start_time.unwrap_or(cursor);
                    let duration = anim.duration();
                    tl.add_tween(anim, Position::Absolute(start));
                    cursor = start + duration;
                }
                TimelineItem::Label(name) => {
                    tl.add_label(name, Position::Absolute(cursor));
                }
                TimelineItem::Nested(nested_tl) => {
                    tl.entries.push(TimelineEntry::Nested {
                        timeline: nested_tl,
                        start: cursor,
                    });
                    cursor += nested_tl.duration;
                }
            }
        }

        tl
    }
}

// Usage
let tl = Timeline::from_sequence(vec![
    TimelineItem::Tween(anim1, None),
    TimelineItem::Label("scene1".to_string()),
    TimelineItem::Tween(anim2, Some(1.0)),  // Start at 1.0s
    TimelineItem::Tween(anim3, None),
]);
```

### Memory Layout: SOA vs AOS

**AOS (Array of Structs) - Current approach:**
```rust
struct TimelineEntry {
    animation: Animation,  // Could be large
    start: f32,
    duration: f32,
}

entries: Vec<TimelineEntry>
```

**SOA (Struct of Arrays) - Cache-friendly:**
```rust
struct Timeline {
    animations: Vec<Animation>,
    start_times: Vec<f32>,
    durations: Vec<f32>,
}
```

**When to use SOA:**
- Many entries (1000+)
- Frequent iteration
- Cache locality matters

**When to use AOS:**
- Few entries (<100)
- Random access common
- Simpler code

**For timelines: AOS is fine** - timelines rarely have 1000+ entries, and seek operations are more common than iteration.

## 6. What to Steal vs What's Overengineered

### Steal from GSAP

**Position parameter system:**
- Labels for named time points
- Relative offsets (`-=0.5`)
- Clean API for sequencing

**Nested timelines:**
- Compose complex animations from smaller pieces
- Essential for UI frameworks

**Playback control:**
- `.play()`, `.pause()`, `.seek(time)`
- `.progress(0-1)` for scrubbing
- `.timeScale()` for speed control

### Steal from Motion One

**Declarative timeline definition:**
- Simpler for static sequences
- Less boilerplate
- Immutable data structure

**Modern API design:**
- Smaller surface area
- Fewer methods

### Steal from Rive/Lottie

**Keyframe-based property animation:**
- Simple data structure
- Easy to serialize
- Works well for GPU upload

### Skip (Overengineered)

**GSAP's percentage positioning (`<50%`):**
- Rarely used
- Adds complexity
- Can achieve same with manual calculation

**AnimeJS's grid stagger in timeline:**
- Belongs in stagger system, not timeline
- Tight coupling

**Rive's full state machine:**
- Too complex for v1
- Can add later if needed

### Minimal Feature Set for v1

**Must have:**
1. Sequence animations with absolute/relative positioning
2. Labels for named time points
3. Basic playback control (play/pause/seek)
4. Overlapping support

**Nice to have (v2):**
5. Nested timelines
6. Callbacks at time points
7. Repeat/yoyo

**Skip for now:**
- State machines
- Percentage positioning
- Complex easing per-tween (can use animation's own easing)

## Sources

- [GSAP Timeline Documentation](https://gsap.com/docs/v3/GSAP/gsap.timeline/)
- [GSAP Position Parameter Guide](https://gsap.com/community/position-parameter/)
- [GSAP Timeline TypeScript Types](https://github.com/greensock/GSAP/blob/master/types/timeline.d.ts)
- [AnimeJS Documentation](https://animejs.com/documentation/)
- [AnimeJS GitHub](https://github.com/juliangarnier/anime)
- [Motion One vs GSAP Comparison](https://motion.dev/docs/gsap-vs-motion)
- [Motion One Migration Guide](https://motion.dev/docs/migrate-from-gsap-to-motion)
- [Lottie vs Rive Comparison](https://www.motiontheagency.com/blog/lottie-vs-rive)
- [Rive as Lottie Alternative](https://rive.app/blog/rive-as-a-lottie-alternative)
- [Lottie Wikipedia](https://en.wikipedia.org/wiki/Lottie_(file_format))
