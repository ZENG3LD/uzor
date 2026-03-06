# Stagger Patterns Research

## Overview

Research into stagger implementations from AnimeJS, GSAP, and Framer Motion. Goal: understand the math behind grid-based stagger, distance calculations, and propagation patterns.

## 1. AnimeJS Stagger

**Documentation:** https://animejs.com/documentation/utilities/stagger/stagger-parameters/stagger-grid/

**CodePen Demo:** https://codepen.io/juliangarnier/pen/XvjWvx

### Basic Stagger

**Simple sequential delay:**
```javascript
anime({
    targets: '.box',
    translateX: 250,
    delay: anime.stagger(100)  // 100ms delay between each
});
```

**Effect:** First element starts at 0ms, second at 100ms, third at 200ms, etc.

### Grid Stagger

**Specify grid dimensions:**
```javascript
anime({
    targets: '.grid-item',
    scale: [0, 1],
    delay: anime.stagger(50, { grid: [14, 5] })
});
```

**Parameters:**
- `[14, 5]` - 14 columns × 5 rows
- Delay based on distance from origin in grid space

### From Parameter

**Starting point for stagger:**
```javascript
delay: anime.stagger(100, {
    grid: [14, 5],
    from: 'center'
})
```

**Options:**
- `'first'` (default) - Top-left corner
- `'center'` - Center of grid
- `'last'` - Bottom-right corner
- `number` - Specific index (e.g., `from: 42`)
- `[col, row]` - Specific grid position (e.g., `from: [7, 2]`)

### Grid Stagger with from: 'center'

**Example from documentation:**
```javascript
anime({
    targets: '.grid-item',
    scale: [0, 1],
    delay: anime.stagger(200, {
        grid: [14, 5],
        from: 'center'
    })
});
```

**How it works:**

1. **Calculate center position:**
   ```javascript
   centerCol = Math.floor(14 / 2) = 7
   centerRow = Math.floor(5 / 2) = 2
   ```

2. **For each element at grid position `(col, row)`:**
   ```javascript
   distance = Math.sqrt(
       Math.pow(col - centerCol, 2) +
       Math.pow(row - centerRow, 2)
   );
   delay = distance * 200;  // 200ms per distance unit
   ```

3. **Elements closer to center start first**

### Direction and Easing

**Control propagation direction:**
```javascript
delay: anime.stagger(100, {
    grid: [14, 5],
    from: 'center',
    direction: 'reverse'  // Further from center starts first
})
```

**Apply easing to stagger:**
```javascript
delay: anime.stagger(100, {
    grid: [14, 5],
    from: 'center',
    easing: 'easeInOutQuad'
})
```

**Effect:** Instead of linear delay = distance × baseDelay, apply easing function:
```javascript
normalizedDistance = distance / maxDistance;
easedDistance = easing(normalizedDistance);
delay = easedDistance * maxDelay;
```

### Advanced Example (17×17 Grid)

**CodePen source:** https://codepen.io/juliangarnier/pen/XvjWvx

```javascript
anime({
    targets: '.grid-item',
    translateY: anime.stagger(4, { grid: [17, 17], from: 'center', axis: 'y' }),
    translateX: anime.stagger(4, { grid: [17, 17], from: 'center', axis: 'x' }),
    rotateZ: anime.stagger([0, 90], { grid: [17, 17], from: 'center' }),
    delay: anime.stagger(50, { grid: [17, 17], from: 'center' }),
    easing: 'easeInOutQuad'
});
```

**Multiple stagger applications:**
- Different properties can have different stagger patterns
- `translateY` staggers based on vertical distance
- `translateX` staggers based on horizontal distance
- `rotateZ` staggers rotation value (0° to 90°)
- All share the same delay stagger

### Source Code Location

Main repository: https://github.com/juliangarnier/anime

**Look for:**
- `stagger` function implementation
- Grid distance calculation
- From position resolution

**Gists with examples:**
- https://gist.github.com/TimyIsCool/7884590d22fd0667f5fd1cd6ffd6c22a
- https://gist.github.com/pixelsum/d7d70ba484e9a51429c8f17c0eb91229

## 2. GSAP Stagger

**Documentation:** https://gsap.com/resources/getting-started/Staggers/

### Basic Stagger

**Simple value:**
```javascript
gsap.to(".box", {
    y: 100,
    stagger: 0.1  // 0.1 seconds between starts
});
```

### Advanced Stagger Object

**Full configuration:**
```javascript
gsap.to(".box", {
    y: 100,
    stagger: {
        amount: 1.5,      // Total time span (not per-element)
        from: "center",
        grid: "auto",     // Auto-detect grid layout
        ease: "power2.inOut"
    }
});
```

### Key Differences from AnimeJS

**Amount vs Each:**
- **AnimeJS:** Specifies delay per element
- **GSAP:** Specifies total time span across all elements

**Example:**
```javascript
// AnimeJS: 10 elements × 100ms = 1000ms total span
anime.stagger(100)

// GSAP: 1000ms total span ÷ 10 elements = 100ms each
stagger: { amount: 1.0 }
```

### Grid Stagger

**Auto-detect grid:**
```javascript
stagger: {
    grid: "auto",
    from: "center"
}
```

GSAP inspects element positions in DOM and determines grid structure automatically.

**Manual grid specification:**
```javascript
stagger: {
    grid: [9, 15],  // 9 columns, 15 rows
    from: "end"
}
```

### From Options

- `"start"` - First element
- `"center"` - Middle element(s)
- `"edges"` - Outer elements first, work inward
- `"random"` - Random order
- `"end"` - Last element
- `number` - Specific index
- `[x, y]` - Grid coordinates (0-1 normalized: [0.5, 0.5] = center)

**Normalized grid coordinates:**
```javascript
stagger: {
    grid: [9, 15],
    from: [0.5, 0.5]  // Center (same as "center")
    from: [1, 0]      // Top-right corner
    from: [0, 1]      // Bottom-left corner
}
```

### Axis Parameter

**Stagger along specific axis only:**
```javascript
stagger: {
    axis: "y",     // Only vertical distance matters
    from: "center"
}
```

Elements at same vertical position get same delay, regardless of horizontal distance.

### distributeByPosition Helper

**Repository example:** https://codepen.io/GreenSock/pen/KKaQoLq

**For non-uniform grids:**
```javascript
function distributeByPosition(vars) {
    let ease = vars.ease,
        from = vars.from || 0,
        base = vars.base || 0,
        axis = vars.axis,
        ratio = {center: 0.5, end: 1, edges: 0.5}[from] || 0,
        distances;

    return function(i, target, a) {
        let l = a.length,
            originX, originY, x, y, d, j, minX, maxX, minY, maxY;

        if (!distances) {
            // Calculate bounding box
            minX = minY = Infinity;
            maxX = maxY = -minX;
            for (j = 0; j < l; j++) {
                x = a[j].getBoundingClientRect().left;
                y = a[j].getBoundingClientRect().top;
                minX = Math.min(minX, x);
                maxX = Math.max(maxX, x);
                minY = Math.min(minY, y);
                maxY = Math.max(maxY, y);
            }

            // Calculate origin point based on 'from'
            originX = minX + (maxX - minX) * ratio;
            originY = minY + (maxY - minY) * ratio;

            // Calculate distances for all elements
            distances = a.map(el => {
                x = el.getBoundingClientRect().left;
                y = el.getBoundingClientRect().top;

                if (axis === "y") {
                    return Math.abs(y - originY);
                } else if (axis === "x") {
                    return Math.abs(x - originX);
                } else {
                    return Math.sqrt(
                        Math.pow(x - originX, 2) +
                        Math.pow(y - originY, 2)
                    );
                }
            });
        }

        d = distances[i] / Math.max(...distances);  // Normalize to 0-1
        return base + (ease ? ease(d) : d) * vars.amount;
    };
}
```

**Usage:**
```javascript
const distribute = distributeByPosition({
    amount: 2,
    from: "center",
    ease: "power2.inOut"
});

const targets = gsap.utils.toArray("#grid div i");
targets.forEach((el, i) => {
    tl.to(el, {
        duration: 1,
        scale: 0.1,
        y: 40,
        yoyo: true,
        repeat: 1,
        ease: "power1.inOut"
    }, distribute(i, el, targets));
});
```

**Key algorithm:**
1. Calculate bounding box of all elements
2. Determine origin point from `from` parameter
3. Measure Euclidean distance from each element to origin
4. Normalize distances to 0-1 range
5. Apply easing function to normalized distance
6. Scale by `amount` to get final delay

**When to use:** Elements positioned arbitrarily (not uniform grid).

### Stagger by Columns

**Discussion:** https://gsap.com/community/forums/topic/20473-stagger-by-columns/

To stagger column-by-column instead of element-by-element:
```javascript
// Group elements by column first
const columns = [];
for (let col = 0; col < numCols; col++) {
    columns.push(elements.filter((el, i) => i % numCols === col));
}

// Stagger column animations
columns.forEach((colElements, i) => {
    gsap.to(colElements, {
        y: 100,
        delay: i * 0.1  // Stagger columns
    });
});
```

## 3. Framer Motion staggerChildren

**Documentation:** https://www.framer.com/motion/transition/

**Guide:** https://motion.mighty.guide/some-examples/25-variants-staggered-animation/

### Concept

`staggerChildren` propagates delays down component tree through variants.

**Basic example:**
```javascript
const containerVariants = {
    hidden: { opacity: 0 },
    visible: {
        opacity: 1,
        transition: {
            staggerChildren: 0.1  // 0.1s between each child
        }
    }
};

const itemVariants = {
    hidden: { opacity: 0, y: 20 },
    visible: { opacity: 1, y: 0 }
};

<motion.ul variants={containerVariants} initial="hidden" animate="visible">
    <motion.li variants={itemVariants} />
    <motion.li variants={itemVariants} />
    <motion.li variants={itemVariants} />
</motion.ul>
```

**Effect:**
- First `<li>` animates at 0s
- Second `<li>` animates at 0.1s
- Third `<li>` animates at 0.2s

### delayChildren

**Add initial delay before stagger starts:**
```javascript
transition: {
    delayChildren: 0.3,      // Wait 0.3s before starting
    staggerChildren: 0.1      // Then 0.1s between each
}
```

**Timeline:**
- First child: 0.3s
- Second child: 0.4s
- Third child: 0.5s

### staggerDirection

**Reverse stagger order:**
```javascript
transition: {
    staggerChildren: 0.1,
    staggerDirection: -1  // Last child first
}
```

**Options:**
- `1` - Forward (default, first to last)
- `-1` - Reverse (last to first)

### Using stagger() Function

**More control with function:**
```javascript
import { stagger } from "framer-motion"

transition: {
    delayChildren: stagger(0.1, {
        from: "last"     // Start from last child
    })
}
```

**Options:**
- `from: "first"` (default)
- `from: "last"`
- `from: "center"`
- `from: <index>`

### How Propagation Works

**Variant propagation algorithm:**

1. Parent component receives animation trigger
2. Parent checks transition for `staggerChildren`
3. For each child with matching variant:
   ```javascript
   childDelay = delayChildren + (childIndex * staggerChildren)
   if (staggerDirection === -1) {
       childDelay = delayChildren + ((numChildren - 1 - childIndex) * staggerChildren)
   }
   ```
4. Child's transition is updated with computed delay
5. Child animates with delay applied

**Recursive nesting:**
Nested `motion` components can each have their own `staggerChildren`, creating hierarchical stagger patterns.

### Orchestration Example

**Stagger with custom timing:**
```javascript
const list = {
    visible: {
        opacity: 1,
        transition: {
            when: "beforeChildren",  // Parent animates first
            staggerChildren: 0.1
        }
    },
    hidden: {
        opacity: 0,
        transition: {
            when: "afterChildren"    // Children finish before parent hides
        }
    }
};
```

**when options:**
- `"beforeChildren"` - Parent completes before children start
- `"afterChildren"` - Children complete before parent starts
- Default - Parent and children animate simultaneously

### Grid Stagger in Framer Motion

**Not built-in**, but can implement with custom delays:

```javascript
const items = [...Array(rows * cols)].map((_, i) => {
    const row = Math.floor(i / cols);
    const col = i % cols;
    const centerRow = Math.floor(rows / 2);
    const centerCol = Math.floor(cols / 2);

    const distance = Math.sqrt(
        Math.pow(col - centerCol, 2) +
        Math.pow(row - centerRow, 2)
    );

    return {
        index: i,
        delay: distance * 0.05  // 50ms per distance unit
    };
});

return (
    <div>
        {items.map(item => (
            <motion.div
                key={item.index}
                initial={{ scale: 0 }}
                animate={{ scale: 1 }}
                transition={{ delay: item.delay }}
            />
        ))}
    </div>
);
```

**Framer Motion expects you to calculate delays manually for grid patterns.**

## 4. Grid Stagger Math Deep Dive

### Euclidean Distance Formula

**For grid position `(col, row)` relative to origin `(originCol, originRow)`:**

```javascript
distance = Math.sqrt(
    Math.pow(col - originCol, 2) +
    Math.pow(row - originRow, 2)
);
```

**Example:** 14×5 grid, from center

Origin at `(7, 2)` (center)

| Position | Distance Calculation | Distance | Delay (×100ms) |
|----------|---------------------|----------|----------------|
| (7, 2) | `sqrt((7-7)² + (2-2)²)` | 0.0 | 0ms |
| (8, 2) | `sqrt((8-7)² + (2-2)²)` | 1.0 | 100ms |
| (7, 3) | `sqrt((7-7)² + (3-2)²)` | 1.0 | 100ms |
| (8, 3) | `sqrt((8-7)² + (3-2)²)` | 1.414 | 141ms |
| (10, 4) | `sqrt((10-7)² + (4-2)²)` | 3.606 | 361ms |

**Diagonal elements have distance = `sqrt(2) ≈ 1.414` per diagonal step.**

### Manhattan Distance (Alternative)

**Sum of axis differences instead of Euclidean:**

```javascript
distance = Math.abs(col - originCol) + Math.abs(row - originRow);
```

**Same examples:**

| Position | Manhattan Distance | Delay (×100ms) |
|----------|-------------------|----------------|
| (7, 2) | 0 | 0ms |
| (8, 2) | 1 | 100ms |
| (7, 3) | 1 | 100ms |
| (8, 3) | 2 | 200ms |
| (10, 4) | 5 | 500ms |

**Difference:** Manhattan creates diamond-shaped propagation waves, Euclidean creates circular waves.

**Use case:**
- **Euclidean:** More natural, circular spread
- **Manhattan:** Axis-aligned, simpler computation

### Axis-Only Distance

**Only consider one axis:**

**Horizontal (X-axis only):**
```javascript
distance = Math.abs(col - originCol);
```

**Vertical (Y-axis only):**
```javascript
distance = Math.abs(row - originRow);
```

**Effect:** Staggers in waves along that axis, ignoring perpendicular position.

### Normalization

**Convert distances to 0-1 range:**

```javascript
maxDistance = Math.max(...distances);
normalizedDistance = distance / maxDistance;
```

**Why normalize?**
- Apply easing functions (expect 0-1 input)
- Scale independently of grid size

### Easing Applied to Distance

**Linear (no easing):**
```javascript
delay = distance * baseDelay;
```

**With easing:**
```javascript
normalizedDistance = distance / maxDistance;
easedDistance = easing(normalizedDistance);  // easing: 0-1 → 0-1
delay = easedDistance * totalTime;
```

**Example with easeInOutQuad:**
```javascript
function easeInOutQuad(t) {
    return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
}

// Elements close to center start quickly
// Elements far from center are delayed more dramatically
```

**Effect:** Non-linear delay distribution - can create slow-then-fast or fast-then-slow propagation.

### From: 'edges'

**Start from perimeter, work inward:**

```javascript
// Calculate distance to nearest edge
const distToLeft = col;
const distToRight = numCols - 1 - col;
const distToTop = row;
const distToBottom = numRows - 1 - row;

const distanceToEdge = Math.min(distToLeft, distToRight, distToTop, distToBottom);

// Invert: further from edge = smaller delay
delay = (maxDistToEdge - distanceToEdge) * baseDelay;
```

**Elements on edges animate first, center animates last.**

### Random Stagger

**Shuffle element order:**
```javascript
const shuffledIndices = [...Array(elements.length).keys()];
shuffleArray(shuffledIndices);

elements.forEach((el, originalIndex) => {
    const shuffledIndex = shuffledIndices[originalIndex];
    delay = shuffledIndex * baseDelay;
});
```

No spatial relationship, pure randomness.

## 5. Implementation for uzor-animation

### Core Trait

```rust
pub trait StaggerPattern {
    fn delay_for_index(&self, index: usize, total: usize) -> f32;
}
```

### Simple Sequential Stagger

```rust
pub struct SimpleStagger {
    pub base_delay: f32,
}

impl StaggerPattern for SimpleStagger {
    fn delay_for_index(&self, index: usize, _total: usize) -> f32 {
        index as f32 * self.base_delay
    }
}
```

### Grid Stagger

```rust
pub struct GridStagger {
    pub base_delay: f32,
    pub rows: usize,
    pub cols: usize,
    pub from: GridOrigin,
    pub distance_metric: DistanceMetric,
    pub easing: Option<Box<dyn EasingFunction>>,
}

pub enum GridOrigin {
    TopLeft,
    Center,
    BottomRight,
    Position(usize, usize),  // (col, row)
    Normalized(f32, f32),     // (0-1, 0-1)
}

pub enum DistanceMetric {
    Euclidean,
    Manhattan,
    AxisX,
    AxisY,
}

impl StaggerPattern for GridStagger {
    fn delay_for_index(&self, index: usize, _total: usize) -> f32 {
        let row = index / self.cols;
        let col = index % self.cols;

        let (origin_col, origin_row) = self.resolve_origin();

        let distance = match self.distance_metric {
            DistanceMetric::Euclidean => {
                let dx = (col as f32 - origin_col as f32).abs();
                let dy = (row as f32 - origin_row as f32).abs();
                (dx * dx + dy * dy).sqrt()
            }
            DistanceMetric::Manhattan => {
                let dx = (col as i32 - origin_col as i32).abs() as f32;
                let dy = (row as i32 - origin_row as i32).abs() as f32;
                dx + dy
            }
            DistanceMetric::AxisX => (col as i32 - origin_col as i32).abs() as f32,
            DistanceMetric::AxisY => (row as i32 - origin_row as i32).abs() as f32,
        };

        // Apply easing if present
        let effective_distance = if let Some(easing) = &self.easing {
            let max_distance = self.calculate_max_distance();
            let normalized = distance / max_distance;
            easing.ease(normalized) * max_distance
        } else {
            distance
        };

        effective_distance * self.base_delay
    }
}

impl GridStagger {
    fn resolve_origin(&self) -> (usize, usize) {
        match self.from {
            GridOrigin::TopLeft => (0, 0),
            GridOrigin::Center => (self.cols / 2, self.rows / 2),
            GridOrigin::BottomRight => (self.cols - 1, self.rows - 1),
            GridOrigin::Position(col, row) => (col, row),
            GridOrigin::Normalized(x, y) => (
                ((self.cols - 1) as f32 * x) as usize,
                ((self.rows - 1) as f32 * y) as usize,
            ),
        }
    }

    fn calculate_max_distance(&self) -> f32 {
        let (origin_col, origin_row) = self.resolve_origin();

        // Check all four corners for max distance
        let corners = [
            (0, 0),
            (self.cols - 1, 0),
            (0, self.rows - 1),
            (self.cols - 1, self.rows - 1),
        ];

        corners.iter()
            .map(|&(col, row)| {
                match self.distance_metric {
                    DistanceMetric::Euclidean => {
                        let dx = (col as f32 - origin_col as f32).abs();
                        let dy = (row as f32 - origin_row as f32).abs();
                        (dx * dx + dy * dy).sqrt()
                    }
                    DistanceMetric::Manhattan => {
                        (col as i32 - origin_col as i32).abs() as f32 +
                        (row as i32 - origin_row as i32).abs() as f32
                    }
                    DistanceMetric::AxisX => (col as i32 - origin_col as i32).abs() as f32,
                    DistanceMetric::AxisY => (row as i32 - origin_row as i32).abs() as f32,
                }
            })
            .fold(0.0_f32, f32::max)
    }
}
```

### Usage

```rust
// Simple stagger: 100ms between each
let stagger = SimpleStagger { base_delay: 0.1 };

// Grid stagger: 50ms per distance unit, from center, Euclidean
let grid_stagger = GridStagger {
    base_delay: 0.05,
    rows: 5,
    cols: 14,
    from: GridOrigin::Center,
    distance_metric: DistanceMetric::Euclidean,
    easing: None,
};

// Apply to animations
for (i, element) in elements.iter().enumerate() {
    let delay = grid_stagger.delay_for_index(i, elements.len());
    let mut anim = Animation::new(element);
    anim.set_delay(delay);
    timeline.add(anim);
}
```

### SIMD Batch Distance Calculation

**For large grids, vectorize:**

```rust
use std::simd::f32x8;

fn calculate_distances_simd(
    indices: &[usize],
    cols: usize,
    origin_col: usize,
    origin_row: usize
) -> Vec<f32> {
    let mut distances = vec![0.0; indices.len()];

    // Process 8 at a time
    for chunk in indices.chunks(8) {
        let mut cols_vec = [0.0; 8];
        let mut rows_vec = [0.0; 8];

        for (i, &idx) in chunk.iter().enumerate() {
            cols_vec[i] = (idx % cols) as f32;
            rows_vec[i] = (idx / cols) as f32;
        }

        let cols_simd = f32x8::from_array(cols_vec);
        let rows_simd = f32x8::from_array(rows_vec);

        let origin_col_simd = f32x8::splat(origin_col as f32);
        let origin_row_simd = f32x8::splat(origin_row as f32);

        let dx = (cols_simd - origin_col_simd).abs();
        let dy = (rows_simd - origin_row_simd).abs();

        let dist_squared = dx * dx + dy * dy;
        let dist = dist_squared.sqrt();

        // Extract results
        // ... store to distances vec
    }

    distances
}
```

**Performance:** 8× speedup for large grids (1000+ elements).

## 6. What to Steal

### From AnimeJS

**Grid stagger with from parameter:**
- Simple, intuitive API
- `stagger(delay, { grid, from })`
- Euclidean distance by default

**Easing on stagger:**
- Apply easing to delay distribution
- Creates non-linear propagation

### From GSAP

**distributeByPosition:**
- Handles arbitrary layouts, not just grids
- Calculates bounding box and origin
- Normalizes distances

**Amount instead of each:**
- Specify total time span
- Easier to control animation length
- Both approaches useful

### From Framer Motion

**staggerChildren for component trees:**
- Automatic propagation through hierarchy
- Works with React-style component structure
- uzor-animation could support similar for widget trees

## 7. Skip (Overengineered)

**Auto-detect grid from DOM positions:**
- Complex heuristics
- Fragile with flex/grid layouts
- Better to require explicit grid specification

**Random variations (jitter):**
- Rarely used
- Can add manually if needed

## Sources

- [AnimeJS Stagger Documentation](https://animejs.com/documentation/utilities/stagger/stagger-parameters/stagger-grid/)
- [AnimeJS Grid Stagger Demo](https://codepen.io/juliangarnier/pen/XvjWvx)
- [AnimeJS Advanced Stagger](https://codepen.io/juliangarnier/pen/MZXQNV)
- [Educative: Animate Grid in AnimeJS](https://www.educative.io/answers/how-to-animate-a-grid-of-elements-in-animejs)
- [GSAP Staggers Documentation](https://gsap.com/resources/getting-started/Staggers/)
- [GSAP Position-based Staggers CodePen](https://codepen.io/GreenSock/pen/KKaQoLq)
- [GSAP Advanced Stagger CodePen](https://codepen.io/GreenSock/pen/jdawKx)
- [Framer Motion Stagger Documentation](https://www.framer.com/motion/stagger/)
- [Framer Motion Staggered Animation Guide](https://motion.mighty.guide/some-examples/25-variants-staggered-animation/)
- [Creating Staggered Animations with Framer Motion](https://medium.com/@onifkay/creating-staggered-animations-with-framer-motion-0e7dc90eae33)
