# SVG Path Parser Specification - Research Document

**Date**: 2026-02-16
**Purpose**: Implement SVG path string parser in Rust for converting to RenderContext draw calls
**Target**: Parse `d` attribute → move_to, line_to, bezier_curve_to, quadratic_curve_to, arc, close_path

---

## 1. All SVG Path Commands with Parameters

### Command Summary Table

| Command | Type | Absolute? | Parameters | Parameter Count | Implicit Repeat |
|---------|------|-----------|-----------|-----------------|-----------------|
| **M/m** | MoveTo | M=yes | `(x, y)+` or `(dx, dy)+` | 2 | Yes (as L/l) |
| **L/l** | LineTo | L=yes | `(x, y)+` or `(dx, dy)+` | 2 | Yes |
| **H/h** | Horizontal | H=yes | `x+` or `dx+` | 1 | Yes |
| **V/v** | Vertical | V=yes | `y+` or `dy+` | 1 | Yes |
| **C/c** | Cubic Bézier | C=yes | `(x1, y1, x2, y2, x, y)+` | 6 | Yes |
| **S/s** | Smooth Cubic | S=yes | `(x2, y2, x, y)+` | 4 | Yes |
| **Q/q** | Quadratic Bézier | Q=yes | `(x1, y1, x, y)+` | 4 | Yes |
| **T/t** | Smooth Quadratic | T=yes | `(x, y)+` or `(dx, dy)+` | 2 | Yes |
| **A/a** | Elliptical Arc | A=yes | `(rx, ry, angle, large-arc, sweep, x, y)+` | 7 | Yes |
| **Z/z** | ClosePath | Either | (none) | 0 | No |

---

### 1.1 MoveTo (M, m)

**Purpose**: Move current point without drawing.

```
M x y           # Absolute: move to (x, y)
m dx dy         # Relative: move by offset (dx, dy)
```

**Implicit Behavior**:
- `M 10 20 30 40` → `M 10 20 L 30 40` (subsequent pairs become LineTo!)
- `m 10 20 30 40` → `m 10 20 l 30 40`

**Formula**:
- `M`: P_new = {x, y}
- `m`: P_new = {P_old.x + dx, P_old.y + dy}

**Critical**: Paths MUST start with MoveTo. `L 30 40` without prior M is invalid.

---

### 1.2 LineTo (L, l, H, h, V, v)

**Purpose**: Draw straight lines.

```
L x y           # Absolute: line to (x, y)
l dx dy         # Relative: line by offset (dx, dy)
H x             # Horizontal line to absolute x (y unchanged)
h dx            # Horizontal line by relative dx
V y             # Vertical line to absolute y (x unchanged)
v dy            # Vertical line by relative dy
```

**Example**:
```svg
M 10,10 L 90,90 V 10 H 50
<!-- Equivalent to: M 10,10 L 90,90 L 90,10 L 50,10 -->
```

**Implicit Repetition**:
- `L 1,2 3,4 5,6` → `L 1,2 L 3,4 L 5,6`

**RenderContext Mapping**: → `line_to(x, y)` (after coordinate conversion)

---

### 1.3 Cubic Bézier Curve (C, c, S, s)

**Purpose**: Draw smooth cubic curves using two control points.

```
C x1 y1 x2 y2 x y       # Absolute: cubic to (x,y) with control points
c dx1 dy1 dx2 dy2 dx dy # Relative version
S x2 y2 x y             # Smooth cubic (first control point reflected)
s dx2 dy2 dx dy         # Relative smooth cubic
```

**Parameters**:
- `x1, y1`: First control point
- `x2, y2`: Second control point
- `x, y`: End point

**Smooth Command (S/s)**: First control point is reflected from previous curve's second control point:
```
x1 = current.x * 2 - prev_x2
y1 = current.y * 2 - prev_y2
```

**Gotcha**: If previous command wasn't a cubic curve, "assume the first control point is coincident with the current point" (i.e., no reflection).

**RenderContext Mapping**: → `bezier_curve_to(x1, y1, x2, y2, x, y)`

---

### 1.4 Quadratic Bézier Curve (Q, q, T, t)

**Purpose**: Draw smooth curves using a single control point.

```
Q x1 y1 x y     # Absolute: quadratic to (x,y) with control point
q dx1 dy1 dx dy # Relative version
T x y           # Smooth quadratic (control point reflected)
t dx dy         # Relative smooth quadratic
```

**Parameters**:
- `x1, y1`: Control point
- `x, y`: End point

**Smooth Command (T/t)**: Control point reflected from previous quadratic curve.

**RenderContext Mapping**: → `quadratic_curve_to(x1, y1, x, y)`

---

### 1.5 Elliptical Arc Curve (A, a) — THE COMPLEX ONE

**Purpose**: Draw arc segments of an ellipse.

```
A rx ry angle large-arc sweep x y       # Absolute
a rx ry angle large-arc sweep dx dy     # Relative
```

**Parameters** (7 total):
1. `rx`: Ellipse x-radius (semi-major axis)
2. `ry`: Ellipse y-radius (semi-minor axis)
3. `angle`: Rotation of ellipse's x-axis (degrees, 0-360)
4. `large-arc-flag`: 0 = arc ≤180°, 1 = arc >180°
5. `sweep-flag`: 0 = counterclockwise, 1 = clockwise
6. `x, y` (or `dx, dy`): End point

**Why 4 Possible Arcs?**
For given radii, there are 2 ellipses connecting start/end points, and 2 paths along each ellipse. Flags disambiguate:

```svg
<!-- Same rx, ry, angle, x, y but different flags -->
M 6,10 A 6 4 10 1 0 14,10  <!-- large-arc=1, sweep=0 -->
M 6,10 A 6 4 10 1 1 14,10  <!-- large-arc=1, sweep=1 -->
M 6,10 A 6 4 10 0 1 14,10  <!-- large-arc=0, sweep=1 -->
M 6,10 A 6 4 10 0 0 14,10  <!-- large-arc=0, sweep=0 -->
```

**Flag Parsing Gotcha**: Flags can be adjacent with no whitespace:
- `A 5 5 30 1 1 10 20` = `A 5 5 30 1110 20` (both valid!)

**RenderContext Mapping**: → `arc(...)` OR convert to bezier curves (see Section 3)

---

### 1.6 ClosePath (Z, z)

**Purpose**: Close current subpath by drawing line to starting point.

```
Z       # Case-insensitive (z is equivalent)
```

**Behavior**:
- Draws line from current point to most recent MoveTo point
- Line ends are joined according to `stroke-linejoin`
- Next MoveTo after ClosePath starts at the same initial point

**Gotcha**:
```svg
M 10 20 L 30 40 Z L 50 60
<!-- Z closes to (10,20), so L 50 60 starts from (10,20) -->
<!-- Equivalent: M 10 20 L 30 40 Z M 10 20 L 50 60 -->
```

**RenderContext Mapping**: → `close_path()`

---

## 2. Coordinate Rules and Number Parsing

### 2.1 Absolute vs Relative Coordinates

**Uppercase = Absolute**:
- `M 100 200` → move to absolute (100, 200)
- Coordinates are canvas positions

**Lowercase = Relative**:
- `m 100 200` → move 100px right, 200px down from current position
- Coordinates are offsets from current point

**Exception**: Z/z are equivalent (case doesn't matter for ClosePath)

---

### 2.2 Number Format Rules

**Decimal Points**:
- Only Unicode U+0046 FULL STOP (".") allowed
- **INVALID**: `13,000.56` (comma as thousands separator)
- **VALID**: `13000.56`

**Scientific Notation**:
- Supported: `6.022e23`, `6.626e-34`
- Sign after 'e'/'E' is exponent, NOT a new number
- `1e-5` is ONE number, not `1e` and `-5`

**Negative Numbers**:
- Adjacent numbers can be separated by minus sign alone
- `M10-20` → `M 10 -20` (valid!)
- BUT: `1e-5-3` → `1e-5` and `-3` (exponent parsing takes precedence)

**Compact Notation**:
- No spaces required between unambiguous numbers
- `M100 100L200 200` = `M 100 100 L 200 200`
- `M10-20A5.5.3-4 110-.1` is valid (each number is parseable)

---

### 2.3 Whitespace and Separator Rules

**Allowed Separators**:
- Whitespace: space, tab, newline, carriage return
- Comma: `,`
- Both: `M 10, 20` = `M 10,20` = `M 10 20`

**Superfluous Whitespace**: Can be eliminated
- `M 100 100 L 200 200` → `M100 100L200 200`

**Flag Parsing**: Arc flags can have NO separator
- `A 5 5 30 1 1 10 20` valid
- `A 5 5 30 1110 20` ALSO valid (flags are single digits)

---

### 2.4 Implicit Command Repetition

**Rule**: Commands followed by multiple coordinate sets repeat implicitly.

**Examples**:
```svg
M 10,20 30,40           → M 10,20 L 30,40  (MoveTo becomes LineTo!)
L 1,2 3,4 5,6           → L 1,2 L 3,4 L 5,6
H 10 20 30              → H 10 H 20 H 30
C 1,2 3,4 5,6  7,8 9,10 11,12 → C 1,2 3,4 5,6  C 7,8 9,10 11,12
```

**MoveTo Exception**: Subsequent coordinates after MoveTo become LineTo, NOT another MoveTo.

---

## 3. Arc Command (A/a) Deep Dive

### 3.1 Endpoint vs Center Parameterization

**SVG Uses Endpoint Parameterization** (what you see in `d` attribute):
- Start point: current position
- End point: `(x, y)` parameter
- Ellipse radii: `rx, ry`
- Rotation: `angle`
- Flags: `large-arc-flag`, `sweep-flag`

**Center Parameterization** (better for math):
- Center point: `(cx, cy)`
- Radii: `rx, ry`
- Start angle: `θ1`
- End angle: `θ2`
- Rotation: `φ`

**Why Convert?** Graphics APIs often use center parameterization, or need to convert arcs to bezier curves.

---

### 3.2 Endpoint → Center Conversion Algorithm

**Input**: `(x1, y1)` current point, `(x2, y2)` end point, `rx, ry, φ, fA, fS`
**Output**: `(cx, cy, θ1, Δθ)`

#### Step 0: Out-of-Range Parameter Handling

**Ensure Non-Zero Radii**: If `rx = 0` or `ry = 0`, treat as straight line to `(x2, y2)`.

**Ensure Positive Radii**: `rx = |rx|`, `ry = |ry|`

**Ensure Radii Sufficiency**:
```rust
let x1_prime = cos(φ) * (x1 - x2)/2 + sin(φ) * (y1 - y2)/2;
let y1_prime = -sin(φ) * (x1 - x2)/2 + cos(φ) * (y1 - y2)/2;
let lambda = (x1_prime.powi(2) / rx.powi(2)) + (y1_prime.powi(2) / ry.powi(2));

if lambda > 1.0 {
    rx *= lambda.sqrt();
    ry *= lambda.sqrt();
}
```

**Why?** If radii are too small to connect endpoints, scale them up.

#### Step 1: Transform to Rotated Ellipse Space

```rust
let x1_prime = cos(φ) * (x1 - x2)/2 + sin(φ) * (y1 - y2)/2;
let y1_prime = -sin(φ) * (x1 - x2)/2 + cos(φ) * (y1 - y2)/2;
```

#### Step 2: Compute Center in Rotated Space

```rust
let sign = if large_arc_flag == sweep_flag { -1.0 } else { 1.0 };
let sq = ((rx * ry).powi(2) - (rx * y1_prime).powi(2) - (ry * x1_prime).powi(2))
         / ((rx * y1_prime).powi(2) + (ry * x1_prime).powi(2));
let sq = sq.max(0.0); // Clamp to prevent sqrt of negative due to float precision

let coeff = sign * sq.sqrt();
let cx_prime = coeff * rx * y1_prime / ry;
let cy_prime = -coeff * ry * x1_prime / rx;
```

#### Step 3: Transform Center Back to Original Space

```rust
let cx = cos(φ) * cx_prime - sin(φ) * cy_prime + (x1 + x2)/2;
let cy = sin(φ) * cx_prime + cos(φ) * cy_prime + (y1 + y2)/2;
```

#### Step 4: Compute Angles

```rust
fn angle_between(ux: f64, uy: f64, vx: f64, vy: f64) -> f64 {
    let dot = ux * vx + uy * vy;
    let det = ux * vy - uy * vx;
    let angle = det.atan2(dot); // Automatically handles sign
    angle
}

let theta1 = angle_between(1.0, 0.0, (x1_prime - cx_prime)/rx, (y1_prime - cy_prime)/ry);
let dtheta = angle_between(
    (x1_prime - cx_prime)/rx, (y1_prime - cy_prime)/ry,
    (-x1_prime - cx_prime)/rx, (-y1_prime - cy_prime)/ry
);

// Apply sweep direction
if sweep_flag && dtheta < 0.0 {
    dtheta += 2.0 * PI;
} else if !sweep_flag && dtheta > 0.0 {
    dtheta -= 2.0 * PI;
}
```

**Arc Cosine Clamping**: Clamp arccos operands to `[-1, 1]` to prevent float precision errors.

---

### 3.3 Arc to Bezier Curve Conversion

**Why?** Many renderers don't support elliptical arcs natively (only circular arcs or no arcs).

**Algorithm** (Maisonobe's approach):

1. **Subdivide Arc**: Split arc into sections ≤ 90° (π/2 radians) each
   - Recommendation: π/4 (45°) for smoother animation
   - Each section becomes one cubic bezier curve

2. **Parametric Ellipse Equation**:
   ```rust
   fn point_at_angle(t: f64, cx: f64, cy: f64, rx: f64, ry: f64, phi: f64) -> (f64, f64) {
       let x = cx + rx * phi.cos() * t.cos() - ry * phi.sin() * t.sin();
       let y = cy + rx * phi.sin() * t.cos() + ry * phi.cos() * t.sin();
       (x, y)
   }
   ```

3. **Control Points for Each Segment**:
   - For arc segment from angle `η1` to `η2`:
   - Start point: `E(η1)`
   - End point: `E(η2)`
   - Control point 1: `E(η1) + k * E'(η1)` (derivative at start)
   - Control point 2: `E(η2) - k * E'(η2)` (derivative at end)
   - Where `k = (4/3) * tan((η2 - η1) / 4)`

4. **Derivative Calculation**:
   ```rust
   fn derivative_at_angle(t: f64, rx: f64, ry: f64, phi: f64) -> (f64, f64) {
       let dx = -rx * phi.cos() * t.sin() - ry * phi.sin() * t.cos();
       let dy = -rx * phi.sin() * t.sin() + ry * phi.cos() * t.cos();
       (dx, dy)
   }
   ```

**Reference Implementation**: See [Rendering SVG Arcs as Bezier Curves](https://mortoray.com/rendering-an-svg-elliptical-arc-as-bezier-curves/)

---

## 4. Existing Rust Crates for SVG Path Parsing

### 4.1 svgtypes

**Crates.io**: [svgtypes](https://crates.io/crates/svgtypes)
**Docs**: [docs.rs/svgtypes](https://docs.rs/svgtypes/0.5.0/svgtypes/index.html)
**Status**: Active, verified with Rust 1.82+

**Features**:
- Complete SVG path parsing with all commands
- Automatic conversion of implicit commands to explicit
- Pull-based parser (iterator over path segments)
- Write support with formatting options
- Zero dependencies (pure Rust)

**API Example**:
```rust
use svgtypes::{PathParser, Path, PathSegment};

// Parse path string
let path: Path = "M10-20A5.5.3-4 110-.1".parse().unwrap();

// Iterate segments
for segment in PathParser::from("M10 20 L30 40") {
    match segment {
        PathSegment::MoveTo { abs, x, y } => println!("Move to {},{}", x, y),
        PathSegment::LineTo { abs, x, y } => println!("Line to {},{}", x, y),
        // ... other segments
    }
}

// Convert coordinates
let mut path = "m10 20 l30 40".parse::<Path>().unwrap();
path.conv_to_absolute(); // Convert all to absolute coords
```

**Path Segment Enum**:
- `MoveTo { abs: bool, x: f64, y: f64 }`
- `LineTo { abs: bool, x: f64, y: f64 }`
- `HorizontalLineTo { abs: bool, x: f64 }`
- `VerticalLineTo { abs: bool, y: f64 }`
- `CurveTo { abs: bool, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64 }`
- `SmoothCurveTo { abs: bool, x2: f64, y2: f64, x: f64, y: f64 }`
- `Quadratic { abs: bool, x1: f64, y1: f64, x: f64, y: f64 }`
- `SmoothQuadratic { abs: bool, x: f64, y: f64 }`
- `EllipticalArc { abs: bool, rx: f64, ry: f64, x_axis_rotation: f64, large_arc: bool, sweep: bool, x: f64, y: f64 }`
- `ClosePath { abs: bool }`

**Pros**:
- Pure parser (no dependencies)
- Handles all edge cases (compact notation, flags, etc.)
- Supports both parsing and writing
- Coordinate conversion utilities

**Cons**:
- Just parsing — doesn't render or convert to primitives
- You still need to implement arc→bezier conversion yourself

---

### 4.2 lyon_path

**Crates.io**: [lyon](https://crates.io/crates/lyon)
**Docs**: [lyon::svg::parser](https://nical.github.io/lyon-doc/lyon/svg/parser/path/index.html)

**Features**:
- Full path library (not just parsing)
- SVG path parsing module
- Path manipulation, tessellation, rendering
- Part of larger graphics ecosystem

**Use Case**: If you need full path operations (tessellation, stroking, etc.), not just parsing.

**Pros**:
- Comprehensive path operations
- Well-maintained
- Used in production (Firefox Servo project)

**Cons**:
- Heavier dependency (full graphics library)
- Overkill if you only need parsing

---

### 4.3 kurbo

**Crates.io**: [kurbo](https://crates.io/crates/kurbo)
**Docs**: [docs.rs/kurbo](https://docs.rs/kurbo)
**Used By**: Druid GUI toolkit, Piet 2D graphics

**Features**:
- 2D curve library (BezPath, CubicBez, QuadBez, Arc, Line)
- `SvgArc` type for SVG arc representation
- `SvgParseError` enum
- NO built-in SVG path string parsing

**Path Types**:
- `BezPath`: Bézier path (move, line, quad, cubic, close)
- `PathEl`: Path element enum
- `PathSeg`: Path segment

**Use Case**: You parse SVG strings yourself (with svgtypes), then build kurbo paths for manipulation.

**Already in Your Deps?** Check your `Cargo.toml` — if you have kurbo, you can use its types directly.

**Pros**:
- High-quality curve math
- Good API for path building
- Lightweight (just curves)

**Cons**:
- Doesn't parse SVG strings for you
- Need separate parser (combine with svgtypes)

---

### 4.4 svg-path-parser

**Crates.io**: [svg-path-parser](https://crates.io/crates/svg-path-parser)
**GitHub**: [UnicodingUnicorn/svg-path-parser](https://github.com/UnicodingUnicorn/svg-path-parser)

**Features**:
- Minimalist: returns lists of points + closed status
- Curves rendered as 64 line segments by default
- Very simple API

**Use Case**: When you don't care about curve fidelity and just want polylines.

**Pros**:
- Extremely simple
- No curve math needed

**Cons**:
- Loses curve information (converts to lines)
- Not suitable for high-quality rendering

---

### 4.5 Recommendation

**For Your Use Case** (RenderContext with bezier/quadratic/arc support):

1. **Use `svgtypes` for parsing** — it's the best pure parser
   - Handles all commands correctly
   - Zero dependencies
   - Returns structured `PathSegment` enum

2. **Combine with `kurbo` if you already have it** — for path building/manipulation

3. **Implement arc→bezier conversion yourself** OR:
   - Check if RenderContext supports native arcs
   - If not, implement Maisonobe algorithm (Section 3.3)

**Minimal Implementation** (if rolling your own):
- Use `svgtypes::PathParser` to tokenize
- Convert segments to absolute coordinates
- Map to RenderContext calls:
  - `MoveTo` → `move_to`
  - `LineTo` → `line_to`
  - `CurveTo` → `bezier_curve_to`
  - `Quadratic` → `quadratic_curve_to`
  - `EllipticalArc` → `arc` OR convert to beziers
  - `ClosePath` → `close_path`

---

## 5. Minimal Implementation — What Commands Are Essential?

### 5.1 Icon SVG Usage Statistics

**Most Common Commands** (no hard stats found, but consensus from docs):
- **M/m**: 100% (every path starts with MoveTo)
- **L/l**: ~90% (straight lines very common)
- **C/c**: ~80% (cubic curves for smooth shapes)
- **Z/z**: ~70% (closed paths for filled shapes)
- **Q/q**: ~30% (quadratic curves for simpler shapes)
- **A/a**: ~20% (arcs for circles/ellipses, often hand-optimized)
- **H/h, V/v**: ~40% (shorthand for axis-aligned lines)
- **S/s, T/t**: ~10% (smooth curve shorthands, less common)

**90% Coverage**: M, L, C, Z

**95% Coverage**: + H, V, Q

**99% Coverage**: + S, T, A

---

### 5.2 Minimal Viable Parser

**Phase 1** (essential):
- `M/m` — MoveTo (required for all paths)
- `L/l` — LineTo (straight lines)
- `Z/z` — ClosePath (closed shapes)

**Phase 2** (high value):
- `C/c` — Cubic Bézier (smooth curves)
- `H/h, V/v` — Horizontal/Vertical lines (convert to LineTo internally)

**Phase 3** (smooth curves):
- `Q/q` — Quadratic Bézier
- `S/s` — Smooth cubic (requires reflection logic)

**Phase 4** (complete):
- `T/t` — Smooth quadratic
- `A/a` — Elliptical arc (complex, but needed for circles)

---

### 5.3 Implementation Strategy

**Option A: Use svgtypes**
```rust
use svgtypes::{PathParser, PathSegment};

pub fn parse_svg_path(d: &str, ctx: &mut RenderContext) {
    for segment in PathParser::from(d) {
        match segment.unwrap() {
            PathSegment::MoveTo { abs, x, y } => {
                // Handle relative if !abs
                ctx.move_to(x, y);
            }
            PathSegment::LineTo { abs, x, y } => {
                ctx.line_to(x, y);
            }
            PathSegment::CurveTo { abs, x1, y1, x2, y2, x, y } => {
                ctx.bezier_curve_to(x1, y1, x2, y2, x, y);
            }
            PathSegment::Quadratic { abs, x1, y1, x, y } => {
                ctx.quadratic_curve_to(x1, y1, x, y);
            }
            PathSegment::EllipticalArc { abs, rx, ry, x_axis_rotation, large_arc, sweep, x, y } => {
                // Either call ctx.arc() if supported, or convert to beziers
                convert_arc_to_beziers(ctx, rx, ry, x_axis_rotation, large_arc, sweep, x, y);
            }
            PathSegment::ClosePath { .. } => {
                ctx.close_path();
            }
            // Handle H, V, S, T by converting to L, C, Q
            _ => {}
        }
    }
}
```

**Option B: Minimal Manual Parser**
- Tokenize numbers (regex: `[-+]?[0-9]*\.?[0-9]+([eE][-+]?[0-9]+)?`)
- State machine for commands
- Only support M, L, C, Z initially

**Recommendation**: Use `svgtypes` unless you have strong reasons to avoid dependencies. It handles all edge cases correctly.

---

## 6. Edge Cases and Gotchas

### 6.1 Parsing Edge Cases

**Compact Notation**:
```svg
M10-20A5.5.3-4 110-.1  <!-- Valid! -->
```
- Numbers can be adjacent if unambiguous
- Minus sign separates numbers EXCEPT after 'e'/'E' (exponent)

**Flag Concatenation**:
```svg
A 5 5 30 1110 20  <!-- Flags are 1,1 not 1110! -->
```
- Arc flags are single digits 0/1
- Can appear concatenated with no space

**MoveTo Becomes LineTo**:
```svg
M 10 20 30 40  <!-- = M 10 20 L 30 40 -->
m 10 20 30 40  <!-- = m 10 20 l 30 40 -->
```

**ClosePath Memory**:
```svg
M 10 20 L 30 40 Z L 50 60  <!-- L 50 60 starts from (10,20)! -->
```

---

### 6.2 Conversion Edge Cases

**Smooth Curves Without Previous Curve**:
```svg
M 10 20 S 30 40 50 60  <!-- No previous C, so first CP = current point -->
```
- Reflection requires previous curve command
- If none, first control point = current point

**Arc with Zero Radii**:
```svg
A 0 5 0 1 1 100 100  <!-- rx=0, treat as straight line -->
```

**Arc with Insufficient Radii**:
- Scale radii up by √λ where λ > 1
- Never reject arc, always find valid ellipse

---

### 6.3 Implementation Gotchas

**Float Precision**:
- Arc conversion: clamp sqrt operands to ≥0
- Angle calculation: clamp arccos operands to [-1, 1]

**Relative Coordinate Tracking**:
- Must track current position for all lowercase commands
- ClosePath returns to MoveTo position, not previous LineTo

**Subpath Tracking**:
- Each MoveTo starts new subpath
- ClosePath closes current subpath
- Important for fill rules and markers

---

## 7. Summary and Recommendations

### 7.1 For RenderContext Integration

**Recommended Approach**:
1. Add `svgtypes = "0.15"` to Cargo.toml
2. Create adapter function `parse_svg_path(d: &str, ctx: &mut RenderContext)`
3. Iterate `PathParser`, match on `PathSegment` enum
4. Convert relative→absolute coordinates on-the-fly
5. Handle smooth commands (S, T) by tracking previous control points
6. For arcs:
   - If RenderContext has native arc support → use it
   - Else → implement arc→bezier conversion (Maisonobe algorithm)

**Code Skeleton**:
```rust
struct PathState {
    current: (f64, f64),
    subpath_start: (f64, f64),
    last_cp: Option<(f64, f64)>, // For S/T reflection
}

impl PathState {
    fn to_absolute(&self, abs: bool, x: f64, y: f64) -> (f64, f64) {
        if abs { (x, y) } else { (self.current.0 + x, self.current.1 + y) }
    }

    fn reflect_cp(&self, x: f64, y: f64) -> (f64, f64) {
        match self.last_cp {
            Some((cpx, cpy)) => (x * 2.0 - cpx, y * 2.0 - cpy),
            None => (x, y), // No reflection, use current point
        }
    }
}
```

---

### 7.2 Testing Strategy

**Unit Tests**:
- Each command type (M, L, H, V, C, S, Q, T, A, Z)
- Absolute vs relative
- Implicit repetition
- Compact notation
- Flag concatenation

**Integration Tests**:
- Real icon SVGs (Material Icons, Feather Icons, etc.)
- Hand-crafted edge cases
- Visual regression (render to image, compare)

**Reference Data**:
- W3C SVG test suite
- Browser rendering (Chrome/Firefox as reference)

---

## 8. References and Sources

### Official Specifications
- [SVG Paths Specification - W3C](https://svgwg.org/specs/paths/)
- [SVG 2 Paths Chapter - W3C](https://www.w3.org/TR/SVG/paths.html)
- [SVG Implementation Notes - W3C](https://www.w3.org/TR/SVG/implnote.html)

### Documentation
- [d - SVG | MDN](https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Attribute/d)
- [Paths - SVG | MDN](https://developer.mozilla.org/en-US/docs/Web/SVG/Tutorial/Paths)
- [The SVG path Syntax: An Illustrated Guide | CSS-Tricks](https://css-tricks.com/svg-path-syntax-illustrated-guide/)
- [An Interactive Guide to SVG Paths - Josh W. Comeau](https://www.joshwcomeau.com/svg/interactive-guide-to-paths/)

### Arc Command Resources
- [Mastering SVG Arcs - Smashing Magazine](https://www.smashingmagazine.com/2024/12/mastering-svg-arcs/)
- [understand-svg-arcs - GitHub](https://github.com/waldyrious/understand-svg-arcs)
- [Rendering SVG Arcs as Bezier Curves - Musing Mortoray](https://mortoray.com/rendering-an-svg-elliptical-arc-as-bezier-curves/)
- [svg-arc-center-endpoint - GitHub](https://github.com/zjffun/svg-arc-center-endpoint)

### Parsing Implementation Notes
- [Notes on SVG Parsing - razrfalcon](https://razrfalcon.github.io/notes-on-svg-parsing/path-data.html)
- [SVG Path Commands - NaN.fyi](https://www.nan.fyi/svg-paths)

### Rust Crates
- [svgtypes - crates.io](https://crates.io/crates/svgtypes)
- [svgtypes documentation - docs.rs](https://docs.rs/svgtypes/0.5.0/svgtypes/index.html)
- [lyon - crates.io](https://crates.io/crates/lyon)
- [kurbo - docs.rs](https://docs.rs/kurbo)
- [svg-path-parser - crates.io](https://crates.io/crates/svg-path-parser)

### Number Parsing
- [Scientific Notation in SVG - W3C List](https://lists.w3.org/Archives/Public/www-svg/1999Nov/0027.html)
- [CSS Grammar for SVG Path Data - Tab Atkins](https://www.xanthir.com/b4NA0)

---

**Document Status**: Complete
**Next Steps**:
1. Add `svgtypes` to dependencies
2. Implement `parse_svg_path()` adapter
3. Test with real icon SVGs
4. Implement arc→bezier conversion if needed
