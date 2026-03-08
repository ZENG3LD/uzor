# Uzor Core — Part 3: Panel System and Animation Engine

**Source:** `uzor/uzor/src/panels/`, `uzor/uzor/src/panel_api/`, `uzor/uzor/src/animation/`

---

## Table of Contents

1. [Panel System (`panels/`)](#1-panel-system-panels)
   - [DockPanel trait](#11-dockpanel-trait)
   - [Tree data structures](#12-tree-data-structures)
   - [DockingTree operations](#13-dockingtree-operations)
   - [DockingManager](#14-dockingmanager)
   - [Tab system](#15-tab-system)
   - [Separator hit testing and resize](#16-separator-hit-testing-and-resize)
   - [Drag-and-drop with drop zone detection](#17-drag-and-drop-with-drop-zone-detection)
   - [Floating windows](#18-floating-windows)
   - [Snap-back animations](#19-snap-back-animations)
   - [Layout serialization](#110-layout-serialization)

2. [Panel API (`panel_api/`)](#2-panel-api-panel_api)
   - [PanelApp trait](#21-panelapp-trait)
   - [Toolbar definitions](#22-toolbar-definitions)
   - [Input and theme types](#23-input-and-theme-types)
   - [Orchestrator integration](#24-orchestrator-integration)

3. [Animation Engine (`animation/`)](#3-animation-engine-animation)
   - [AnimationCoordinator](#31-animationcoordinator)
   - [Spring physics](#32-spring-physics)
   - [Easing functions](#33-easing-functions)
   - [Decay (flick physics)](#34-decay-flick-physics)
   - [Timeline and keyframes](#35-timeline-and-keyframes)
   - [Color interpolation](#36-color-interpolation)
   - [Stagger patterns](#37-stagger-patterns)
   - [Scroll-linked animations](#38-scroll-linked-animations)
   - [Blending and composition](#39-blending-and-composition)
   - [Recipe categories](#310-recipe-categories)

---

## 1. Panel System (`panels/`)

The panel system implements a generic docking layout engine. It handles N-ary panel trees, tabs, splits, grids, separators, drag-and-drop, floating windows, and layout persistence. The engine is fully rendering-agnostic: it computes geometry and hit-test results; rendering is the caller's responsibility.

### 1.1 `DockPanel` Trait

Every panel type stored in the docking tree must implement `DockPanel`. This is the minimum contract the engine requires.

```rust
pub trait DockPanel: Clone + Send + Sync {
    /// Title shown in the tab bar
    fn title(&self) -> &str;

    /// String key used for serialization routing.
    /// The deserializer factory matches on this string to reconstruct panels.
    fn type_id(&self) -> &'static str;

    /// Minimum content size (width, height) in pixels. Default: (200.0, 200.0).
    fn min_size(&self) -> (f32, f32) { (200.0, 200.0) }

    /// Whether the user can close this panel. Default: true.
    fn closable(&self) -> bool { true }
}
```

Minimal implementation:

```rust
#[derive(Clone)]
struct ChartPanel {
    symbol: String,
}

impl DockPanel for ChartPanel {
    fn title(&self) -> &str { &self.symbol }
    fn type_id(&self) -> &'static str { "chart" }
    fn min_size(&self) -> (f32, f32) { (400.0, 300.0) }
}
```

### 1.2 Tree Data Structures

The panel tree uses two complementary representations.

#### `PanelTree<P>` — arena-based (from `tree.rs`)

`PanelTree` is a flat arena: a `HashMap<NodeId, Tile<P>>` with an optional root. Nodes are either panels (leaves) or containers (branches).

```rust
pub struct PanelStore<P: DockPanel> {
    tiles: HashMap<NodeId, Tile<P>>,
    next_id: u64,
}

pub struct PanelTree<P: DockPanel> {
    pub root: Option<NodeId>,
    pub tiles: PanelStore<P>,
}

pub enum Tile<P: DockPanel> {
    Panel(P),
    Container(Container),
}

pub enum Container {
    Tabs(Tabs),       // N children, one active
    Linear(Linear),   // Horizontal or vertical splits
    Grid(Grid),       // 2D grid layout
}
```

`Tabs`, `Linear`, and `Grid` store `Vec<NodeId>` pointing into the `PanelStore`. Space allocation for `Linear` uses `Shares`:

```rust
pub struct Shares {
    shares: HashMap<NodeId, f32>,
}

impl Shares {
    // Proportional distribution: shares=[1,2,1] → 25%, 50%, 25%
    pub fn split(&self, children: &[NodeId], total_size: f32) -> Vec<f32> { ... }
}
```

#### `DockingTree<P>` — recursive tree (from `grid.rs`)

`DockingTree` is the structure `DockingManager` actually operates on. It uses recursive enum nodes:

```rust
pub enum PanelNode<P: DockPanel> {
    Leaf(Leaf<P>),
    Branch(Branch<P>),
}

pub struct Leaf<P: DockPanel> {
    pub id: LeafId,
    pub panels: Vec<P>,     // Multiple panels = tabs
    pub active_tab: usize,
    pub hidden: bool,
    pub color_tag: Option<u8>, // Domain-agnostic grouping token
}

pub struct Branch<P: DockPanel> {
    pub id: BranchId,
    pub children: Vec<PanelNode<P>>,
    pub layout: WindowLayout,
    pub proportions: Vec<f64>,          // Per-child proportions (0..1)
    pub cross_ratio: Option<(f64, f64)>, // Grid2x2 crosshair position
    pub custom_rects: Vec<PanelRect>,    // Explicit override rects
}

pub struct DockingTree<P: DockPanel> {
    root: Branch<P>,      // Root is always a Branch (even for a single leaf)
    active_leaf: Option<LeafId>,
    next_id: u64,         // Monotonically increasing ID counter
}
```

**Node IDs** are typed newtype wrappers:

```rust
pub struct LeafId(pub u64);
pub struct BranchId(pub u64);
```

`PanelNode::raw_id()` returns the inner `u64` for cases where the distinction doesn't matter (separator generation).

**Layout variants** (`WindowLayout`):

| Variant | Children | Shape |
|---|---|---|
| `Single` | 0–1 | Full area |
| `SplitHorizontal` | 2 | Left / Right |
| `SplitVertical` | 2 | Top / Bottom |
| `Grid2x2` | 4 | 2×2 grid |
| `ThreeColumns` | 3 | Three equal columns |
| `ThreeRows` | 3 | Three equal rows |
| `OneLeftTwoRight` | 3 | 1 wide + 2 stacked |
| `TwoLeftOneRight` | 3 | 2 stacked + 1 wide |
| `OneTopTwoBottom` | 3 | 1 wide + 2 side-by-side |
| `TwoTopOneBottom` | 3 | 2 side-by-side + 1 wide |
| `Custom` | any | Arbitrary proportions |

Layout is inferred automatically from child count via `DockingTree::infer_layout(n)`.

### 1.3 `DockingTree` Operations

```rust
// Construction
let tree = DockingTree::<MyPanel>::new();
let tree = DockingTree::with_single_leaf(panel);

// Leaf management
let leaf_id: LeafId = tree.add_leaf(panel);
let leaf_id: LeafId = tree.add_leaf_near(panel, sibling_id); // Insert near sibling
tree.remove_leaf(leaf_id);                                     // Auto-collapses tree

// Tab management
tree.add_tab(leaf_id, panel);        // Append tab; sets active to new tab
tree.remove_tab(leaf_id, tab_idx);   // Remove tab; removes leaf if empty

// Split a leaf into N sub-panels
let new_ids: Vec<LeafId> = tree.split_leaf(leaf_id, SplitKind::Horizontal, w, h);

// Active leaf
tree.set_active_leaf(leaf_id);
let leaf: Option<&Leaf<P>> = tree.active_leaf();

// Drag-and-drop restructure
tree.move_leaf_to_branch(dragged_id, target_id, DropZone::Left);
tree.move_leaf_to_root_split(dragged_id, DropZone::Up); // Edge drop

// Proportions (0.5 each for equal halves)
tree.set_proportions(vec![0.3, 0.7]);
tree.set_branch_proportions(branch_id, vec![0.5, 0.5]);
tree.set_cross_ratio(0.6, 0.4); // Grid2x2 crosshair

// Visibility
tree.hide_leaf(leaf_id); // Will refuse if only one visible leaf remains
tree.show_leaf(leaf_id);

// Query
let leaves: Vec<&Leaf<P>> = tree.leaves();
let count: usize = tree.leaf_count();
let visible: usize = tree.visible_leaf_count();
let layout: WindowLayout = tree.layout();
```

**Branch collapse**: After `remove_leaf`, the tree automatically collapses single-child branches (`collapse_single_children_branch`) and fixes layouts to match the new child count (`fix_branch_layouts`). Layout transitions are smart — e.g., removing one panel from `Grid2x2` (4 panels) selects an L-shaped 3-panel layout based on which corner was removed.

### 1.4 `DockingManager`

`DockingManager<P: DockPanel>` is the orchestration layer. It wraps a `DockingTree` and derives all computed state from it after each call to `layout()`.

```rust
pub struct DockingManager<P: DockPanel> {
    tree: DockingTree<P>,
    separators: Vec<Separator>,
    panel_rects: HashMap<LeafId, PanelRect>,
    panel_headers: HashMap<LeafId, PanelRect>,
    tab_bars: Vec<TabBarInfo>,
    corners: Vec<CornerHandle>,        // Separator intersection points
    layout_area: PanelRect,
    window_edge_rects: Option<[PanelRect; 4]>, // [top, bottom, left, right]
    panel_drag: Option<PanelDragState>,
    tab_reorder: Option<TabReorderState>,
    snap_animations: Vec<SnapBackAnimation>,
    floating_windows: Vec<FloatingWindow<P>>,
    floating_drag: Option<FloatingDragState>,
    next_floating_id: u64,
    hovered_header: Option<LeafId>,
    active_leaf: Option<LeafId>,
    header_height: f32,               // Default: 24px
}
```

**Construction:**

```rust
// Empty manager
let mut mgr = DockingManager::<MyPanel>::new();

// Single panel
let mut mgr = DockingManager::with_panel(panel);

// From restored tree (after deserialization)
let tree = snapshot.restore_tree(|type_id| create_panel(type_id))?;
let mut mgr = DockingManager::from_tree(tree);
```

**Frame lifecycle:**

```rust
// 1. Layout — call every time the window resizes or tree changes
mgr.layout(PanelRect::new(0.0, 0.0, 1920.0, 1080.0));

// 2. Update window edge indicators (for edge-drop during drag)
mgr.compute_window_edge_rects();

// 3. Hit test cursor to dispatch events
match mgr.hit_test(cursor_x, cursor_y) {
    HitResult::Panel(leaf_id) => { /* panel content interaction */ }
    HitResult::Separator(idx) => { /* resize handle */ }
    HitResult::Corner(idx) => { /* bidirectional resize */ }
    HitResult::None => {}
}

// 4. Update separator hover for cursor change
let any_hovered = mgr.update_separator_hover(cursor_x, cursor_y);
if any_hovered {
    // Change cursor to resize arrow based on orientation
    match mgr.hovered_separator_orientation() {
        Some(SeparatorOrientation::Vertical) => set_cursor(CursorIcon::ColResize),
        Some(SeparatorOrientation::Horizontal) => set_cursor(CursorIcon::RowResize),
        None => {}
    }
}

// 5. Update snap-back animations (dt in seconds)
mgr.update_snap_animations(1.0 / 60.0);

// 6. Read derived state for rendering
for (&leaf_id, &rect) in mgr.panel_rects() { ... }
for (&leaf_id, &header_rect) in mgr.panel_headers() { ... }
for tab_bar in mgr.tab_bars() { ... }
for sep in mgr.separators() { ... }
for corner in mgr.corners() { ... }
for fw in mgr.floating_windows() { ... }
```

**Hit test priority:** corners > separators > panels > none.

**Accessors:**

```rust
mgr.tree()            -> &DockingTree<P>
mgr.tree_mut()        -> &mut DockingTree<P>
mgr.panel_rects()     -> &HashMap<LeafId, PanelRect>
mgr.panel_headers()   -> &HashMap<LeafId, PanelRect>
mgr.tab_bars()        -> &[TabBarInfo]
mgr.separators()      -> &[Separator]
mgr.corners()         -> &[CornerHandle]
mgr.floating_windows() -> &[FloatingWindow<P>]
mgr.snap_animations() -> &[SnapBackAnimation]
mgr.active_leaf()     -> Option<LeafId>
mgr.layout_area()     -> PanelRect
mgr.window_edge_rects() -> Option<&[PanelRect; 4]>
mgr.hovered_header()  -> Option<LeafId>
```

### 1.5 Tab System

When a `Leaf` has more than one panel, `DockingManager::layout()` generates a `TabBarInfo` instead of a single header rect.

```rust
pub struct TabBarInfo {
    pub container_id: LeafId,   // Which leaf owns this tab bar
    pub rect: PanelRect,         // Full tab bar area
    pub tabs: Vec<TabItem>,
}

pub struct TabItem {
    pub panel_id: LeafId,        // Synthetic tab ID (leaf_id * 100 + tab_idx)
    pub title: String,
    pub rect: PanelRect,         // This tab's click area
    pub is_active: bool,
    pub close_rect: PanelRect,   // Close button (14×14px, right-aligned in tab)
}
```

Tab width is estimated at `8px + text_len * 7px + 24px + 8px`, clamped to `[80, 200]px`.

**Tab operations:**

```rust
// Switch active tab
mgr.set_active_tab(container_id, tab_id);

// Close a tab
mgr.close_tab(container_id, tab_id);

// Reorder tabs via drag
mgr.start_tab_reorder(container_id, tab_id, start_x);
mgr.update_tab_reorder(cursor_x);  // Computes insert_index based on midpoints
mgr.end_tab_reorder();             // Applies the reorder
```

The low-level `TabBar` controller (in `tabs.rs`) can also be used standalone for custom tab rendering:

```rust
let mut bar = TabBar::new(32.0); // height=32px

bar.set_tabs(vec![
    (LeafId(1), "Chart".to_string(), true),
    (LeafId(2), "Table".to_string(), true),
]);
bar.set_active(Some(LeafId(1)));
bar.layout(PanelRect::new(0.0, 0.0, 800.0, 32.0));

// Hit test
match bar.hit_test(mouse_x, mouse_y) {
    Some(TabHit::Tab(idx)) => { /* select tab */ }
    Some(TabHit::Close(idx)) => { /* close tab */ }
    None => {}
}
```

The `TabDragController` handles tab-to-tab dragging with cross-container drops:

```rust
let mut drag_ctrl = TabDragController::new();

// On mouse down in tab
drag_ctrl.start_drag(tab_idx, container_id, mouse_pos, tab_rect);

// On mouse move
drag_ctrl.update_drag(mouse_pos);
let state = drag_ctrl.drag_state().unwrap();
// state.preview_rect follows the cursor for ghost rendering

// On mouse up
if let Some((src, tab_idx, target, zone)) = drag_ctrl.complete_drag(target_id, DropZone::Center) {
    // Move tab from src[tab_idx] into target using zone
}

// On Escape
drag_ctrl.cancel();
```

### 1.6 Separator Hit Testing and Resize

Separators are generated by `DockingManager::layout()` from the tree structure. Each separator sits between two adjacent children of a branch.

```rust
pub struct Separator {
    pub orientation: SeparatorOrientation, // Vertical (|) or Horizontal (—)
    pub position: f32,    // Position along axis (absolute pixels)
    pub start: f32,       // Perpendicular axis start
    pub length: f32,      // Perpendicular axis length
    pub state: SeparatorState, // Idle, Hover, Dragging
    pub level: SeparatorLevel, // Which branch/children this separator controls
    // Private: thickness: 2.0, hit_width: 8.0
}

pub enum SeparatorLevel {
    Node {
        parent_id: BranchId,
        child_a: u64,  // Left/top child raw ID
        child_b: u64,  // Right/bottom child raw ID
    },
}
```

**Visual thickness** changes with state:
- `Idle` → 2px
- `Hover` / `Dragging` → 4px

**Hit testing** uses an 8px interaction zone around the separator centerline, making it easy to grab without pixel-perfect accuracy:

```rust
if separator.hit_test(mouse_x, mouse_y) {
    // Within ±4px of separator position, AND within separator's length range
}
```

**Corner handles** are generated at every vertical×horizontal separator intersection:

```rust
pub struct CornerHandle {
    pub v_separator_idx: usize,
    pub h_separator_idx: usize,
    pub x: f32,
    pub y: f32,
}

// Hit test with 10px radius
corner.hit_test(mouse_x, mouse_y, 10.0)
```

Grabbing a corner allows simultaneous bidirectional resize of all four adjacent panels.

**Resize via `SeparatorController`:**

```rust
let mut ctrl = SeparatorController::new();

// On mouse down on separator idx=0
ctrl.start_drag(
    0,          // separator_idx (between children[0] and children[1])
    container_id,
    sep_position,
    current_shares, // Vec<f32> of all children's current shares
);

// On mouse move — returns None if any panel would go below min_size
match ctrl.update_drag(delta_pixels, &children, &min_sizes, total_size) {
    Some(new_shares) => {
        tree.set_branch_proportions(parent_id, new_shares.iter()
            .map(|&s| s as f64)
            .collect());
        // Trigger relayout
    }
    None => {
        // Constraint violated — trigger snap-back animation
        snap_animations.push(SnapBackAnimation::new(sep_idx, delta_pixels));
    }
}

// On mouse up
ctrl.end_drag();
```

### 1.7 Drag-and-Drop with Drop Zone Detection

**Drop zone detection** uses a VSCode-style 5-zone algorithm:

```rust
pub enum DropZone {
    Center,  // Add as tab
    Left,    // Split, new panel left
    Right,   // Split, new panel right
    Up,      // Split, new panel top
    Down,    // Split, new panel bottom
}
```

The `DropZoneDetector` uses two thresholds:
- **edge_threshold** (10%): Outer band that activates directional zones
- **split_threshold** (33%): Determines Left vs Right vs Up vs Down

```rust
let detector = DropZoneDetector::new(); // 10% edge, 33% split
let zone = detector.detect(mouse_x, mouse_y, target_rect);

// Or with custom thresholds:
let detector = DropZoneDetector::with_thresholds(0.15, 0.40);
```

`DropZone::preview_rect(container_rect)` computes the highlight rectangle to show the user where the panel will land (50% of the container on the appropriate side for directional zones, full container for center).

**Panel drag lifecycle** via `DockingManager`:

```rust
// On mouse down on a panel header
mgr.start_panel_drag(leaf_id, cursor_x, cursor_y);

// On mouse move — updates target_leaf_id and drop_zone automatically
mgr.update_panel_drag(cursor_x, cursor_y);

// Read drag state for rendering the ghost/highlight
if let Some(drag) = mgr.panel_drag_state() {
    let ghost_pos = (drag.current_x, drag.current_y);
    if let (Some(target), Some(zone)) = (drag.target_leaf_id, drag.drop_zone) {
        let target_rect = mgr.panel_rects()[&target];
        let preview = zone.preview_rect(target_rect);
        // Render blue highlight at preview
    }
}

// On mouse up — performs the drop, or floats if no valid target
let maybe_fw_id: Option<FloatingWindowId> = mgr.end_panel_drag(area_w, area_h);

// On Escape
mgr.cancel_panel_drag();
```

**Drop priority** (highest to lowest):
1. Panel headers (always creates tabs)
2. Tab bars (always creates tabs)
3. Window edge indicators (root-level splits)
4. Panel body (5-zone detection)

### 1.8 Floating Windows

A floating window is a panel (or stack of tabbed panels) extracted from the docking tree that hovers above the layout at an arbitrary position.

```rust
pub struct FloatingWindow<P: DockPanel> {
    pub id: FloatingWindowId,    // FloatingWindowId(u64)
    pub panels: Vec<P>,
    pub active_tab: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,              // Default: 300px
    pub height: f32,             // Default: 300px
}

impl<P: DockPanel> FloatingWindow<P> {
    pub fn title(&self) -> &str      // From active panel
    pub fn contains(&self, x, y) -> bool
    pub fn active_panel(&self) -> Option<&P>
    pub fn tab_count(&self) -> usize
    pub fn rect(&self) -> PanelRect
}
```

**Float a leaf out of the tree:**

```rust
// Returns None if the tree would be left with zero visible leaves
let fw_id: Option<FloatingWindowId> = mgr.float_leaf(leaf_id, x, y, area_w, area_h);
```

`float_leaf` clamps the initial position to `[0, area_size - 300]`.

**Dock a floating window back:**

```rust
mgr.dock_floating(fw_id, target_leaf_id, DropZone::Left, is_window_edge);
```

This re-inserts all panels from the floating window into the tree (first panel as new leaf, rest as tabs), then applies the drop restructuring.

**Floating window drag:**

```rust
mgr.start_floating_drag(fw_id, cursor_x, cursor_y);
mgr.update_floating_drag(cursor_x, cursor_y, area_w, area_h);
mgr.update_floating_dock_target(cursor_x, cursor_y); // For docking preview

// On release — returns dock info if hovering a target
if let Some((fw_id, target_id, zone, is_edge)) = mgr.end_floating_drag() {
    mgr.dock_floating(fw_id, target_id, zone, is_edge);
    mgr.layout(area);
}
```

**Floating hit testing:**

```rust
mgr.hit_test_floating_header(x, y) -> Option<FloatingWindowId>
mgr.hit_test_floating_body(x, y)   -> Option<FloatingWindowId>
mgr.hit_test_floating_close(x, y)  -> Option<FloatingWindowId>
```

Hit tests scan in reverse order (topmost window first). Header = top `header_height` pixels. Close button = 20×20px at top-right corner.

### 1.9 Snap-Back Animations

When a separator drag would violate a panel's minimum size, the caller should trigger a snap-back animation to smoothly return the separator to its last valid position.

```rust
pub struct SnapBackAnimation {
    pub separator_idx: usize,
    pub done: bool,
    // (private spring state)
}

impl SnapBackAnimation {
    // initial_offset: the pixel violation amount (how far past the limit the user dragged)
    pub fn new(separator_idx: usize, initial_offset: f32) -> Self

    // Returns the current visual offset from the valid position
    pub fn offset(&self) -> f32

    // Advance the animation by dt seconds
    pub fn update(&mut self, dt: f32)
}
```

Internally uses `Spring::new().stiffness(300.0).damping(20.0).mass(1.0)` — a stiff spring that settles in ~0.3 seconds with no visible oscillation.

Integration via `DockingManager`:

```rust
// In render loop
mgr.update_snap_animations(dt_seconds);

// During separator drag — when constraint violated:
// The caller is responsible for creating the animation and adding it to a list.
// DockingManager stores them internally via snap_animations.
for anim in mgr.snap_animations() {
    let sep = &mgr.separators()[anim.separator_idx];
    let visual_position = sep.position + anim.offset();
    // Render separator at visual_position instead of sep.position
}
```

An animation with `initial_offset < 0.5` is marked `done` immediately (no visible effect).

### 1.10 Layout Serialization

The `LayoutSnapshot` serializes tree structure only — not panel content. Panel content is restored via a factory closure.

```rust
pub struct LayoutSnapshot {
    pub version: String,            // "1.0"
    pub name: String,
    pub nodes: Vec<SerializedNode>,
    pub root_id: u64,
    pub active_leaf_id: Option<u64>,
}

pub struct SerializedNode {
    pub id: u64,
    pub node_type: SerializedNodeType,
}

pub enum SerializedNodeType {
    Leaf {
        panel_type_ids: Vec<String>,  // DockPanel::type_id() for each tab
        active_tab: usize,
        hidden: bool,
        color_tag: Option<u8>,
    },
    Branch {
        children: Vec<u64>,
        layout: String,              // WindowLayout name
        proportions: Vec<f64>,
        cross_ratio: Option<(f64, f64)>,
    },
}
```

**Save:**

```rust
let snapshot = LayoutSnapshot::from_tree(&mgr.tree(), "my_layout");
let json: String = snapshot.to_json()?;
std::fs::write("layout.json", &json)?;
```

**Restore:**

```rust
let json = std::fs::read_to_string("layout.json")?;
let snapshot = LayoutSnapshot::from_json(&json)?;

let tree = snapshot.restore_tree(|type_id| {
    match type_id {
        "chart"     => Some(MyPanel::Chart(ChartPanel::default())),
        "watchlist" => Some(MyPanel::Watchlist(WatchlistPanel::default())),
        _           => None,  // Unknown type — panel silently dropped
    }
})?;

let mut mgr = DockingManager::from_tree(tree);
mgr.layout(window_rect);
```

If a `type_id` returns `None`, `restore_tree` returns an `Err`. If the saved `active_leaf_id` no longer exists (e.g., panel type was removed), it falls back to the first available leaf.

---

## 2. Panel API (`panel_api/`)

The panel API defines the contract between the terminal orchestrator and individual panel crates. Each panel crate (chart, map, watchlist, DOM, etc.) implements `PanelApp` to become a self-contained application.

### 2.1 `PanelApp` Trait

```rust
pub trait PanelApp {
    /// Display title (shown in tab bar / header)
    fn title(&self) -> &str;

    /// Type key: "chart", "map", "dom", "watchlist", ...
    fn type_id(&self) -> &'static str;

    /// Minimum panel size (width, height)
    fn min_size(&self) -> (f64, f64) { (200.0, 200.0) }

    /// Toolbar definition — None if this panel has no toolbar.
    fn toolbar_def(&self) -> Option<PanelToolbarDef> { None }

    /// Where the toolbar should be placed
    fn toolbar_position(&self) -> ToolbarPosition { ToolbarPosition::Top }

    /// Render the panel's toolbar. ctx is translated to toolbar-local coords.
    /// Returns hit zones for click dispatch.
    fn render_toolbar(
        &self,
        ctx: &mut dyn RenderContext,
        rect: PanelRect,
        theme: &PanelTheme,
        input: &PanelInput,
    ) -> Vec<HitZone> { vec![] }

    /// Render main content. ctx is translated to content-local coords.
    fn render_content(
        &mut self,
        ctx: &mut dyn RenderContext,
        rect: PanelRect,
        input: &PanelInput,
    ) { }

    /// Downcast to concrete type. Override in every concrete impl.
    fn as_any_mut(&mut self) -> &mut dyn Any {
        panic!("as_any_mut not overridden for {}", self.type_id())
    }

    /// Handle click on a toolbar item. Returns optional action string for orchestrator.
    fn handle_toolbar_click(&mut self, item_id: &str) -> Option<String> { None }

    /// Handle dropdown item selection
    fn handle_dropdown_select(&mut self, dropdown_id: &str, item_id: &str) -> Option<String> { None }

    /// Whether this panel can share a toolbar with siblings
    fn supports_toolbar_grouping(&self) -> bool { false }
}
```

**`ToolbarPosition`** variants: `Top` (default), `Left`, `Right`, `Bottom`.

**`as_any_mut` pattern** — the terminal orchestrator stores panels as `Box<dyn PanelApp>`. When it needs to call panel-specific methods (e.g., `ChartPanelApp::render_chart()`), it downcasts:

```rust
// In the orchestrator:
if let Some(chart) = panel.as_any_mut().downcast_mut::<ChartPanelApp>() {
    chart.render_chart_content(ctx, chart_area, &data);
}

// In ChartPanelApp:
impl PanelApp for ChartPanelApp {
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    // ...
}
```

**Lifecycle per frame:**

```text
orchestrator → panel.toolbar_def()           → carve out toolbar space
orchestrator → panel.render_toolbar(...)     → panel draws toolbar, returns HitZone list
orchestrator → panel.render_content(...)     → panel draws content
user click in toolbar:
orchestrator → panel.handle_toolbar_click(item_id)  → panel updates state, may return action
action "open_modal:symbol_search":
orchestrator → shows modal, feeds result back to panel
```

**Action string protocol:** The return value of `handle_toolbar_click` is a freeform string that the orchestrator interprets. By convention: `"open_modal:MODAL_NAME"` to show a modal, empty string or `None` for local-only state changes.

### 2.2 Toolbar Definitions

Toolbars are described with `PanelToolbarDef`, composed of `ToolbarSectionDef` groups, which contain `ToolbarItemDef` items.

```rust
pub struct PanelToolbarDef {
    pub sections: Vec<ToolbarSectionDef>,
    pub orientation: ToolbarOrientation, // Horizontal or Vertical
    pub size: f64,       // Height (horizontal) or width (vertical). Default: 32px
    pub item_size: f64,  // Button height/width. Default: 28px
    pub icon_size: f64,  // Icon size within button. Default: 16px
    pub spacing: f64,    // Gap between items. Default: 2px
    pub padding: f64,    // Edge padding. Default: 4px
}
```

**Constructors:**

```rust
PanelToolbarDef::horizontal(sections)  // size=32px
PanelToolbarDef::vertical(sections)    // size=40px, item_size=32px
PanelToolbarDef::default()             // horizontal, empty sections
```

**Sections** group items with an optional divider:

```rust
pub struct ToolbarSectionDef {
    pub items: Vec<ToolbarItemDef>,
    pub show_separator: bool,
    pub align: SectionAlign, // Start (left/top) or End (right/bottom)
}

// Builder:
ToolbarSectionDef::new(items)
    .with_separator()  // Show divider at start of this section
    .align_end()       // Right-align (for settings/close buttons)
```

**Item types:**

```rust
pub enum ToolbarItemDef {
    Button {
        id: &'static str,
        icon: Option<ToolbarIconId>,
        text: Option<String>,
        active: bool,       // Highlighted/toggled state
        disabled: bool,
        min_width: f64,
    },
    IconButton {
        id: &'static str,
        icon: ToolbarIconId,
        active: bool,
        disabled: bool,
    },
    Dropdown {
        id: &'static str,
        icon: Option<ToolbarIconId>,
        text: Option<String>,
        active: bool,
        show_chevron: bool,
        items: Vec<DropdownItemDef>,
        quick_select: bool,  // First click = last tool; second click = dropdown
        grid_columns: Option<u8>, // Render items in grid instead of list
        min_width: f64,
    },
    Separator,  // Visual divider
    Spacer,     // Flex spacer
}
```

**Builder pattern:**

```rust
// Icon button
ToolbarItemDef::icon_button("zoom_in", "zoom-in")

// Button with icon and text
ToolbarItemDef::button("timeframe")
    .with_icon("clock")
    .with_text("1H")
    .with_active(self.timeframe == Timeframe::H1)
    .with_min_width(48.0)

// Dropdown
ToolbarItemDef::dropdown("chart_type", vec![
    DropdownItemDef::action("candlestick", "Candlestick")
        .with_icon("candle"),
    DropdownItemDef::action("bar", "Bar Chart")
        .with_icon("bar"),
    DropdownItemDef::Separator,
    DropdownItemDef::action("line", "Line")
        .with_icon("line"),
])
.with_icon("candle")
.with_text("Candle")
.with_active(true)

// Quick select (first click = last used tool)
ToolbarItemDef::quick_select("drawing_tool", vec![...])
    .with_icon("pencil")
```

**Dropdown items:**

```rust
pub enum DropdownItemDef {
    Action {
        id: String,
        label: String,
        icon: Option<ToolbarIconId>,
        shortcut: Option<String>,
    },
    Submenu {
        id: String, label: String,
        items: Vec<DropdownItemDef>,
        grid_columns: Option<u8>,
    },
    Header { label: String },
    Separator,
}

// Builder:
DropdownItemDef::action("1h", "1 Hour")
    .with_icon("clock")
    .with_shortcut("Alt+1")
```

Full example — a chart panel's toolbar definition:

```rust
fn toolbar_def(&self) -> Option<PanelToolbarDef> {
    Some(PanelToolbarDef::horizontal(vec![
        // Left section: core controls
        ToolbarSectionDef::new(vec![
            ToolbarItemDef::button("symbol")
                .with_text(&self.symbol)
                .with_min_width(80.0),
            ToolbarItemDef::dropdown("timeframe", vec![
                DropdownItemDef::action("1m", "1 Minute"),
                DropdownItemDef::action("5m", "5 Minutes"),
                DropdownItemDef::action("1h", "1 Hour"),
                DropdownItemDef::action("1d", "1 Day"),
            ])
            .with_text(&self.timeframe.to_str()),
            ToolbarItemDef::Separator,
            ToolbarItemDef::icon_button("indicators", "layers"),
        ]),
        // Right section: settings
        ToolbarSectionDef::new(vec![
            ToolbarItemDef::icon_button("settings", "settings"),
        ]).align_end(),
    ]))
}
```

### 2.3 Input and Theme Types

**`PanelInput`** — passed every frame to both `render_toolbar` and `render_content`:

```rust
pub struct PanelInput {
    pub mouse_x: f64,          // Panel-local mouse X (0 = left edge)
    pub mouse_y: f64,          // Panel-local mouse Y (0 = top edge)
    pub mouse_screen_x: f64,   // Screen-space mouse X (for popups)
    pub mouse_screen_y: f64,
    pub mouse_in_panel: bool,
    pub focused: bool,
    pub toolbar_visible: bool,
    pub time_ms: u64,          // Monotonic time for animations
    pub dpr: f64,              // Device pixel ratio
    pub clicked: Option<(f64, f64)>,   // Click position if clicked this frame
    pub button_down: bool,
    pub button_released: bool,
    pub scroll_delta: (f64, f64),      // (horizontal, vertical) scroll
    pub input_consumed: bool,  // True if a higher-priority layer consumed input
}
```

**`HitZone`** — returned by `render_toolbar` to enable click dispatch:

```rust
pub struct HitZone {
    pub id: String,    // Must match an item's id string
    pub rect: PanelRect,
}
```

The orchestrator iterates `HitZone` list to find the zone containing the click, then calls `panel.handle_toolbar_click(&zone.id)`.

**`PanelTheme`** — colors for consistent styling:

```rust
pub struct PanelTheme {
    pub toolbar_bg: String,        // "#1e1e2e"
    pub toolbar_separator: String, // "#333346"
    pub item_bg_hover: String,     // "#2a2a3e"
    pub item_bg_active: String,    // "#3b82f6"
    pub item_text: String,         // "#cdd6f4"
    pub item_text_muted: String,   // "#6c7086"
    pub item_text_hover: String,   // "#ffffff"
    pub item_text_active: String,  // "#ffffff"
    pub accent: String,            // "#3b82f6"
    pub sidebar_style: bool,       // Use vertical accent bars
}
```

### 2.4 Orchestrator Integration

The orchestrator's render loop per visible panel:

```rust
fn render_panel(panel: &mut dyn PanelApp, alloc_rect: PanelRect, ...) {
    let theme = PanelTheme::default();
    let input = build_panel_input(cursor, events, alloc_rect, time);

    if let Some(def) = panel.toolbar_def() {
        let (toolbar_rect, content_rect) = split_rect(
            alloc_rect,
            panel.toolbar_position(),
            def.size,
        );

        let hit_zones = panel.render_toolbar(ctx, toolbar_rect, &theme, &input);

        if let Some((x, y)) = input.clicked {
            for zone in &hit_zones {
                if zone.rect.contains(x, y) {
                    if let Some(action) = panel.handle_toolbar_click(&zone.id) {
                        orchestrator.handle_action(action);
                    }
                }
            }
        }

        panel.render_content(ctx, content_rect, &input);
    } else {
        panel.render_content(ctx, alloc_rect, &input);
    }
}
```

---

## 3. Animation Engine (`animation/`)

The animation engine provides spring physics, 30 Penner easings, decay (flick physics), keyframe timelines, perceptual color interpolation, stagger patterns, scroll-linked animations, and blending. The `AnimationCoordinator` is the central runtime that manages all active animations keyed by `(WidgetId, property)`.

### 3.1 `AnimationCoordinator`

```rust
pub struct AnimationCoordinator {
    active: HashMap<AnimationKey, ActiveAnimation>,
    default_interruption: InterruptionStrategy,
}

// AnimationKey uniquely identifies one animated property on one widget:
pub struct AnimationKey {
    pub widget_id: WidgetId,
    pub property: String,
}
```

**Typical integration in a render loop:**

```rust
let mut anim = AnimationCoordinator::new();
let widget_id = WidgetId::new("btn_hover");

// Start animation (time in seconds since program start)
anim.tween(widget_id.clone(), "opacity", 0.0, 1.0, 0.2, Easing::EaseOutCubic, current_time);

// Each frame:
let needs_repaint = anim.update(current_time); // returns true while animations active

// Read value (None if no animation running — use your static value instead)
let opacity = anim.get_or(&widget_id, "opacity", 1.0);
```

**Three animation drivers:**

```rust
// 1. Tween — interpolate from→to over duration with easing
anim.tween(
    widget_id.clone(),
    "x",
    0.0,    // from
    100.0,  // to
    0.3,    // duration_secs
    Easing::EaseOutQuart,
    current_time,
);

// 2. Spring — physics-driven approach to target
anim.spring(
    widget_id.clone(),
    "scale",
    Spring::stiff(),
    1.0,         // target value
    current_time,
);

// 3. Decay — friction-based deceleration (flick scroll)
anim.decay(
    widget_id.clone(),
    "scroll_y",
    Decay::ios_scroll(velocity),
    current_scroll_y,
    current_time,
);
```

**`ActiveAnimation` internal state:**

```rust
pub struct ActiveAnimation {
    pub driver: AnimationDriver,  // Tween / Spring / Decay
    pub current_value: f64,
    pub completed: bool,
}
```

Completed animations are removed from `active` during `update()`. After removal, `get()` returns `None` — the caller falls back to the static final value.

**Interruption:** Starting a new animation on an already-animating property replaces the old one immediately (Instant strategy). The property `default_interruption` can be changed, but only `Instant` is fully implemented in the coordinator; `Blend`, `InheritVelocity`, and `Queue` require the higher-level `AnimationSlot` (see §3.9).

**Management:**

```rust
anim.cancel_widget(&widget_id);           // Cancel all properties for a widget
anim.cancel(&widget_id, "opacity");       // Cancel one property
anim.has_active() -> bool
anim.is_animating(&widget_id) -> bool
anim.active_count() -> usize              // For diagnostics
```

### 3.2 Spring Physics

`Spring` uses the closed-form analytical solution to the damped harmonic oscillator, not numerical integration. This means:
- No time-step drift
- Any time value can be evaluated independently
- Frame-rate independent by definition

```rust
pub struct Spring {
    pub stiffness: f64,         // Rigidity. Higher = snappier. Default: 100
    pub damping: f64,           // Friction. Higher = less oscillation. Default: 10
    pub mass: f64,              // Inertia. Higher = slower. Default: 1
    pub initial_velocity: f64,  // Velocity at t=0. Default: 0
    pub rest_threshold: f64,    // Stop when |pos|+|vel| < this. Default: 0.001
}
```

**Damping ratio** `ζ = damping / (2 * sqrt(stiffness * mass))`:
- `ζ < 1` — under-damped (oscillates, bouncy)
- `ζ = 1` — critically damped (fastest no-overshoot)
- `ζ > 1` — over-damped (no oscillation, slow approach)

**Evaluation** returns `(position, velocity)` where position goes from `1.0` (full displacement) toward `0.0` (rest):

```rust
let (pos, vel) = spring.evaluate(t_secs);
// Map to actual value: actual = target - pos * displacement_magnitude
```

The three analytical case formulas:

| Case | Formula |
|---|---|
| Under-damped | `x(t) = e^(-ζω₀t) * (A·cos(ωd·t) + B·sin(ωd·t))` |
| Critically-damped | `x(t) = (A + B·t) * e^(-ω₀t)` |
| Over-damped | `x(t) = A·e^(r₁t) + B·e^(r₂t)` |

Where `ωd = ω₀·sqrt(1 - ζ²)` (damped angular frequency) and `r₁,r₂ = -ω₀(ζ ± sqrt(ζ²-1))` (overdamped roots).

**Preset springs:**

```rust
Spring::gentle()  // stiffness=120, damping=14 — iOS-style, subtle bounce
Spring::bouncy()  // stiffness=180, damping=12 — visible oscillation
Spring::stiff()   // stiffness=300, damping=20 — fast, minimal overshoot
Spring::slow()    // stiffness=60,  damping=14 — deliberate, soft
```

**Usage patterns:**

```rust
// Snap-back separator (used internally by SnapBackAnimation)
let spring = Spring::new()
    .stiffness(300.0)
    .damping(20.0)
    .mass(1.0);

// Hover scale with velocity inheritance (flick-to-hover)
let spring = Spring::gentle()
    .initial_velocity(previous_velocity);

// Modal open with overshoot
let spring = Spring::bouncy();

// Convert to easing curve for use in tween
let easing_table: Vec<f64> = spring.as_easing(100);

// Duration estimation (for knowing when to stop polling)
let duration = spring.estimated_duration(); // seconds

// Rest check
if spring.is_at_rest(elapsed) { /* done */ }
```

### 3.3 Easing Functions

`Easing` covers all 30 Penner equations plus CSS-compatible `cubic-bezier()` and `steps()`.

```rust
pub enum Easing {
    Linear,
    // Quadratic (t²)
    EaseInQuad, EaseOutQuad, EaseInOutQuad,
    // Cubic (t³)
    EaseInCubic, EaseOutCubic, EaseInOutCubic,
    // Quartic (t⁴)
    EaseInQuart, EaseOutQuart, EaseInOutQuart,
    // Quintic (t⁵)
    EaseInQuint, EaseOutQuint, EaseInOutQuint,
    // Sinusoidal
    EaseInSine, EaseOutSine, EaseInOutSine,
    // Exponential (2^x)
    EaseInExpo, EaseOutExpo, EaseInOutExpo,
    // Circular (sqrt)
    EaseInCirc, EaseOutCirc, EaseInOutCirc,
    // Back (overshoot)
    EaseInBack, EaseOutBack, EaseInOutBack,
    // Elastic (spring-like sinusoidal)
    EaseInElastic, EaseOutElastic, EaseInOutElastic,
    // Bounce (multiple impact simulation)
    EaseInBounce, EaseOutBounce, EaseInOutBounce,
    // CSS timing functions
    CubicBezier(f64, f64, f64, f64),  // x1, y1, x2, y2
    Steps(u32, StepPosition),          // N steps, Start/End
}

// CSS standard constants:
Easing::EASE         // cubic-bezier(0.25, 0.1, 0.25, 1.0)
Easing::EASE_IN      // cubic-bezier(0.42, 0, 1.0, 1.0)
Easing::EASE_OUT     // cubic-bezier(0, 0, 0.58, 1.0)
Easing::EASE_IN_OUT  // cubic-bezier(0.42, 0, 0.58, 1.0)
```

**Evaluation:**

```rust
let t: f64 = 0.5; // normalized progress 0.0..1.0
let eased: f64 = Easing::EaseOutCubic.ease(t);
let eased_f32: f32 = Easing::EaseOutCubic.ease_f32(t as f32); // convenience wrapper
```

Input is clamped to `[0, 1]` before evaluation. Back and elastic easings may return values outside `[0, 1]` due to overshoot.

**Cubic-bezier solver** uses Newton-Raphson (up to 8 iterations) with bisection fallback, matching Firefox/Chrome implementations. Precomputes 11-sample lookup table for initial guess.

**Steps function:**

```rust
// 4 steps, jump at end of each interval: 0, 0, 0.25, 0.5, 0.75, 1.0
Easing::Steps(4, StepPosition::End)

// 4 steps, jump at start: 0.25, 0.5, 0.75, 1.0, 1.0
Easing::Steps(4, StepPosition::Start)
```

**Recommended choices by use case:**

| Use case | Easing |
|---|---|
| UI element entering screen | `EaseOutCubic`, `EaseOutQuart` |
| UI element leaving screen | `EaseInCubic` |
| Value toggle / switch | `EaseInOutCubic` |
| Button press | `EaseInQuad` |
| Button release | `EaseOutBack` |
| Modal appear | `EaseOutQuint` |
| Number counter | `EaseOutExpo` |
| Loading spinner | `Linear` or `EaseInOutSine` |
| Playful animations | `EaseOutElastic`, `EaseOutBounce` |
| CSS-compatible | `CubicBezier(...)` |

### 3.4 Decay (Flick Physics)

`Decay` models friction-based inertia: exponential velocity decay after a flick or drag release.

```rust
pub struct Decay {
    pub velocity: f64,          // Initial velocity (units/sec)
    pub friction: f64,          // Per-frame friction coefficient (0..1). Default: 0.998
    pub rest_threshold: f64,    // Stop when velocity < this. Default: 0.01
    pub min_bound: Option<f64>, // Optional lower bound with spring bounce-back
    pub max_bound: Option<f64>, // Optional upper bound with spring bounce-back
    pub bounce_stiffness: f64,  // Bound spring stiffness. Default: 400
    pub bounce_damping: f64,    // Bound spring damping. Default: 30
}
```

**Physics formulas** (applied 60 times per second):

```
velocity(t) = v0 * friction^(t * 60)
position(t) = v0 * (friction^(t*60) - 1) / (60 * ln(friction))
```

**Evaluation:**

```rust
let (position, velocity) = decay.evaluate(t_secs);
// position is the displacement from start (not absolute position)
// final_position in coordinator: initial_value + position
```

**Presets:**

```rust
Decay::ios_scroll(velocity)  // friction=0.998 — iOS-like feel
Decay::heavy(velocity)       // friction=0.99  — stops quickly
Decay::light(velocity)       // friction=0.999 — slides far
```

**Builder pattern:**

```rust
let decay = Decay::new(500.0)  // 500 units/sec initial velocity
    .friction(0.998)
    .rest_threshold(0.5)
    .bounds(0.0, max_scroll)   // Spring bounce at list edges
    .bounce_stiffness(300.0);
```

**When to use vs Spring:**
- `Decay`: After gesture release — the animation starts moving and slows to a stop. No target.
- `Spring`: Moving toward a target — will overshoot/oscillate then settle.

**Practical example — flick scroll:**

```rust
// On touch/mouse release:
let decay = Decay::ios_scroll(fling_velocity_y);
coord.decay(scroll_widget_id, "scroll_y", decay, current_scroll_y, now);

// Each frame:
let scroll_y = coord.get_or(&scroll_widget_id, "scroll_y", current_scroll_y);
render_list_at(scroll_y);
```

### 3.5 Timeline and Keyframes

`Timeline` orchestrates multiple animations with precise timing (GSAP-inspired).

```rust
pub struct Timeline {
    entries: Vec<TimelineEntry>,  // sorted by start time
    labels: HashMap<String, Duration>,
    total_duration: Duration,
    repeat: u32,
    yoyo: bool,    // Reverse direction on each repeat
    speed: f64,    // Playback speed multiplier
}

// A single animation entry:
pub struct TimelineEntry {
    pub start: Duration,
    pub duration: Duration,
    pub id: u64,           // Returned by add(), used to query progress
}
```

**Building a timeline:**

```rust
let mut timeline = Timeline::new();

// Add entries at positions
let id1 = timeline.add(Duration::from_millis(300), Position::Absolute(Duration::ZERO));
let id2 = timeline.add(Duration::from_millis(300), Position::AfterPrevious(Duration::from_millis(50)));
let id3 = timeline.add(Duration::from_millis(200), Position::AtLabel("midpoint".to_string()));

// Add labels
timeline.label("midpoint"); // at current end = after id2
timeline.label_at("start", Duration::ZERO);

// Configure repeating/yoyo
let timeline = timeline
    .repeat(2)         // play 3 times total
    .yoyo(true)        // ping-pong
    .speed(1.5);       // 1.5x speed
```

**Playback via `TimelinePlayback`:**

```rust
let mut pb = TimelinePlayback::new(timeline);
pb.play();

// Each frame:
pb.tick(Duration::from_millis(16));

// Query progress of specific entry (0..=1)
let progress = pb.progress_of(id1);

// Check if entry is currently animating
let active = pb.is_active(id2);

// Check if timeline is done
let done = pb.is_complete();

// Control
pb.pause();
pb.restart();
pb.reverse();
pb.seek(Duration::from_millis(500));
```

**`Tween<T>`** — the typed animation primitive underlying timelines:

```rust
pub struct Tween<T: Animatable> {
    pub from: T,
    pub to: T,
    pub duration: Duration,
    pub easing: Easing,
    pub delay: Duration,
}

let tween = Tween::new(0.0f64, 1.0f64)
    .duration(Duration::from_millis(300))
    .easing(Easing::EaseOutCubic)
    .delay(Duration::from_millis(100));

// Evaluate at elapsed time (including delay)
let value: Option<f64> = tween.evaluate(Duration::from_millis(250));
// Returns None if elapsed < delay
```

**`Animatable` trait** — implement for any interpolatable type:

```rust
pub trait Animatable: Clone + Send + Sync + 'static {
    fn lerp(&self, target: &Self, t: f64) -> Self;
}

// Built-in impls: f64, f32, (f64, f64), (f64, f64, f64, f64)
// Also: Color (uses OKLCH by default)

// Custom impl:
#[derive(Clone)]
struct Vec2 { x: f64, y: f64 }

impl Animatable for Vec2 {
    fn lerp(&self, target: &Self, t: f64) -> Self {
        Vec2 {
            x: self.x + (target.x - self.x) * t,
            y: self.y + (target.y - self.y) * t,
        }
    }
}
```

### 3.6 Color Interpolation

`Color` in sRGB, with conversion to Oklab and OKLCH for perceptually uniform interpolation.

```rust
pub struct Color { pub r: f64, pub g: f64, pub b: f64, pub a: f64 }
pub struct Oklab { pub l: f64, pub a: f64, pub b: f64 }      // Cartesian
pub struct Oklch  { pub l: f64, pub c: f64, pub h: f64 }     // Polar

pub enum ColorSpace { Srgb, LinearRgb, Oklab, Oklch }
```

**Why OKLCH?** sRGB interpolation of e.g. `red → blue` passes through an unsaturated grayish purple. OKLCH interpolates along the hue wheel while preserving chroma, producing a vivid purple midpoint.

**Creating colors:**

```rust
Color::rgb(0.2, 0.4, 0.8)                        // sRGB, opaque
Color::rgba(0.2, 0.4, 0.8, 0.5)                  // sRGB, semi-transparent
Color::from_hex("#3b82f6").unwrap()               // #RRGGBB
Color::from_hex("#3b82f680").unwrap()             // #RRGGBBAA
let hex: String = color.to_hex();                 // "#3B82F6" or "#3B82F680"

// Constants
Color::BLACK, Color::WHITE, Color::RED, Color::GREEN, Color::BLUE
```

**Interpolation:**

```rust
let from = Color::RED;
let to   = Color::BLUE;

// OKLCH — best for UI (preserves saturation)
let mid = from.lerp_oklch(&to, 0.5);

// Choose color space explicitly:
let mid = from.lerp(&to, t, ColorSpace::Oklch);
let mid = from.lerp(&to, t, ColorSpace::Oklab);
let mid = from.lerp(&to, t, ColorSpace::LinearRgb);
let mid = from.lerp(&to, t, ColorSpace::Srgb);
```

**Hue interpolation** uses shortest-path around the 360° wheel:
- `350° → 10°` goes through `0°` (20° arc), not `180°`
- `10° → 350°` also goes through `0°`

**Color space conversions:**

```rust
let lab: Oklab  = color.to_oklab();
let lch: Oklch  = color.to_oklch();
let (r, g, b)   = color.to_linear();            // Linear RGB (gamma-decoded)
let back        = Color::from_oklab(lab, alpha);
let back        = Color::from_oklch(lch, alpha);
let back        = Color::from_linear(r, g, b, alpha);
```

**Integration with timelines:**

```rust
// Color implements Animatable (via OKLCH lerp)
let color_tween = Tween::new(Color::from_hex("#1e1e2e").unwrap(), Color::from_hex("#3b82f6").unwrap())
    .duration(Duration::from_millis(200))
    .easing(Easing::EaseOutCubic);

let current_color = color_tween.evaluate(elapsed).unwrap_or(Color::BLACK);
// Render with current_color.r, .g, .b, .a
```

### 3.7 Stagger Patterns

Stagger distributes animation delays across a set of elements so they animate one after another (or in waves) rather than all at once.

#### `LinearStagger`

For 1D lists:

```rust
pub struct LinearStagger {
    pub delay: Duration,        // Base delay between consecutive elements
    pub from: StaggerOrigin,    // First, Last, Center, or Index(n)
    pub easing: Option<Easing>, // Optional easing on delay distribution
}

pub enum StaggerOrigin {
    First,      // Sequential from index 0
    Last,       // Sequential from last index backwards
    Center,     // Radiates from middle outward
    Index(n),   // Radiates from specific index
}
```

**Usage:**

```rust
// Sequential from first: delays = [0, 100, 200, 300, 400]ms
let stagger = LinearStagger::new(Duration::from_millis(100));
let delays: Vec<Duration> = stagger.delays(5);

// Ripple from center: delays = [200, 100, 0, 100, 200]ms
let stagger = LinearStagger::new(Duration::from_millis(100))
    .from(StaggerOrigin::Center);

// With easing on the distribution (EaseInQuad → delays accelerate)
let stagger = LinearStagger::new(Duration::from_millis(50))
    .from(StaggerOrigin::First)
    .easing(Easing::EaseInQuad);

// Single element delay:
let delay = stagger.delay_for(index, total_count);
```

Apply to a list:

```rust
let stagger = LinearStagger::new(Duration::from_millis(50));
let delays = stagger.delays(items.len());

for (i, (item, &delay)) in items.iter().zip(delays.iter()).enumerate() {
    let start_time = current_time + delay.as_secs_f64();
    coord.tween(item.id.clone(), "opacity", 0.0, 1.0, 0.3, Easing::EaseOutCubic, start_time);
}
```

#### `GridStagger`

For 2D grids with three distance metrics:

```rust
pub struct GridStagger {
    pub delay: Duration,
    pub cols: usize,
    pub rows: usize,
    pub from: GridOrigin,           // TopLeft, TopRight, BottomLeft, BottomRight, Center, Cell(col, row)
    pub metric: DistanceMetric,     // Euclidean, Manhattan, Chebyshev
    pub easing: Option<Easing>,
}

pub enum DistanceMetric {
    Euclidean,  // sqrt(dx² + dy²) — circular wave
    Manhattan,  // |dx| + |dy| — diamond wave
    Chebyshev,  // max(|dx|, |dy|) — square wave
}
```

**Usage:**

```rust
// Ripple from center of a 14×5 grid, Euclidean distance
let stagger = GridStagger::new(Duration::from_millis(30), 14, 5)
    .from(GridOrigin::Center)
    .metric(DistanceMetric::Euclidean);

// Get all delays: Vec<Duration> of length cols*rows, in row-major order
let delays: Vec<Duration> = stagger.delays();
// delays[row * cols + col]

// Or query one cell:
let delay = stagger.delay_for(col, row);
```

Distance metric comparison for diagonal cell (2,2) from origin (0,0):
- Euclidean: `sqrt(8) ≈ 2.83` — circular propagation
- Manhattan: `4` — diamond propagation
- Chebyshev: `2` — square propagation

### 3.8 Scroll-Linked Animations

Scroll-linked animations are deterministic: the same scroll position always produces the same animation state. No time component.

#### `ScrollTimeline`

Maps a scroll range to animation progress `0.0..1.0`:

```rust
let timeline = ScrollTimeline::new(start_px, end_px)
    .with_easing(Easing::EaseOutCubic)
    .unclamped(); // Allow progress < 0 or > 1

let progress: f64 = timeline.progress(current_scroll_y);
timeline.is_active(scroll_y) -> bool
timeline.is_complete(scroll_y) -> bool
```

#### `ScrollTween<T>`

Combines `ScrollTimeline` with value interpolation:

```rust
let tween = ScrollTween::new(
    0.0f64,               // from
    1.0f64,               // to
    ScrollTimeline::new(100.0, 500.0),
);

let opacity: f64 = tween.value_at(current_scroll_y);
```

Works with any `Animatable` type:

```rust
let position_tween = ScrollTween::new(
    (0.0, 0.0),
    (100.0, 50.0),
    ScrollTimeline::new(0.0, 1000.0),
);
let (tx, ty): (f64, f64) = position_tween.value_at(scroll);
```

#### `ViewTimeline`

Animation based on element visibility in viewport ("reveal on scroll"):

```rust
let timeline = ViewTimeline::new(
    element_top,     // absolute Y position
    element_bottom,
    viewport_height,
)
.with_thresholds(0.0, 1.0)  // 0.0 = enters, 1.0 = fully entered
.with_easing(Easing::EaseOutCubic);

let progress = timeline.progress(scroll_top); // viewport_top scroll position
timeline.is_visible(scroll_top) -> bool
timeline.is_fully_visible(scroll_top) -> bool
timeline.is_exiting(scroll_top) -> bool
```

#### `ParallaxLayer`

Depth-based parallax offset:

```rust
let background = ParallaxLayer::new(0.0);  // static
let midground  = ParallaxLayer::new(0.5);  // half speed
let foreground = ParallaxLayer::new(1.0);  // full speed

let bg_offset  = background.offset(scroll);  // 0px
let mid_offset = midground.offset(scroll);   // scroll * 0.5
let fg_offset  = foreground.offset(scroll);  // scroll * 1.0

// Relative to anchor:
let offset = layer.offset_relative(scroll, anchor_scroll);
```

### 3.9 Blending and Composition

#### `blend` / `blend_weighted`

```rust
// Simple lerp between two Animatable values
let result: f64 = blend(&0.0f64, &10.0f64, 0.5); // 5.0

// Weighted blend of multiple values (sequential progressive blend)
let result = blend_weighted(&[
    (0.0f64, 0.5),   // 50% weight
    (10.0f64, 0.3),  // 30% weight
    (20.0f64, 0.2),  // 20% weight
]).unwrap();
```

#### `AnimationLayer<T>` / `resolve_layers`

Layered animation composition (CSS Web Animations style):

```rust
pub struct AnimationLayer<T: Animatable> {
    pub value: T,
    pub weight: f64,          // 0..1
    pub mode: CompositeMode,  // Replace, Add, or Accumulate
}

pub enum CompositeMode {
    Replace,     // lerp(accumulated, layer.value, layer.weight)
    Add,         // accumulated + layer.value * weight
    Accumulate,  // same as Add (full impl needs iteration count)
}

let base = 10.0f64;
let layers = vec![
    AnimationLayer::new(20.0f64).with_weight(0.5),       // Replace 50%
    AnimationLayer::new(30.0f64).with_weight(0.3),       // Replace 30% of result
];
let result = resolve_layers(&base, &layers); // 9.5

// Additive blend (f64 specialization):
let result = additive_blend_f64(base, additive_delta, weight);
// result = base + additive_delta * weight
```

#### `InterruptionStrategy`

Controls what happens when a new animation starts while one is already running:

```rust
pub enum InterruptionStrategy {
    Instant,                         // Jump to new animation immediately (default in coordinator)
    Blend { duration_secs: f64 },    // Crossfade from current to new over duration_secs
    InheritVelocity,                 // New animation uses current velocity for smooth continuation
    Queue,                           // Wait for current to finish, then start new
}
```

#### `AnimationSlot<T>` — high-level property management

For properties that need smooth interruption handling:

```rust
let mut slot = AnimationSlot::new(0.0f64)
    .with_strategy(InterruptionStrategy::Blend { duration_secs: 0.2 });

// Trigger new target
slot.set(100.0f64);

// Each frame: provide new animation output, get blended result
let display_value = slot.update(animated_value, dt_secs);

// Or use Instant (jump):
let mut slot = AnimationSlot::new(0.0f64)
    .with_strategy(InterruptionStrategy::Instant);
slot.set(100.0f64);
let value = slot.update(100.0f64, 0.0); // jumps immediately
```

`AnimationTransition` handles the crossfade internally. When `transition.is_complete()`, the slot commits to the new value and clears the transition.

### 3.10 Recipe Categories

The `recipes/` module provides pre-configured animation patterns organized into 8 categories. Each category is a typed enum with variants for specific interaction patterns.

#### `ButtonAnimation`

```rust
pub enum ButtonAnimation {
    Hover { duration_ms, easing, opacity_from, opacity_to },
    Press { duration_ms, easing, scale },         // Scale down on click
    Release { spring },                            // Spring-back after press
    Ripple { duration_ms, easing, scale_from, scale_to, opacity_from, opacity_to },
    Toggle { duration_ms, easing },                // On/off state
    ElasticScale { spring, target_scale },         // Bouncy hover scale
    GlowPulse { duration_ms, easing, intensity_from, intensity_to },
    UnderlineSlide { duration_ms, easing, origin: SlideOrigin },  // Left/Right/Center
    FillSweep { duration_ms, easing, direction: SweepDirection },
    BorderDraw { duration_ms, easing, stagger_delay_ms },
    MagneticPull { spring, max_distance, strength },
    LiftShadow { duration_ms, easing, shadow_y_from, shadow_y_to, shadow_blur_from, shadow_blur_to, lift_distance },
}

// Query duration:
let ms: u64 = anim.duration_ms();
let dur: Duration = anim.duration();
```

#### `ChartAnimation`

```rust
pub enum ChartAnimation {
    BarGrow { duration_ms, stagger_delay_ms, easing, count },
    BarUpdate { spring, stagger_delay_ms, count },
    LineDrawIn { duration_ms, easing, path_length },
    CandlestickReveal { wick_duration_ms, body_duration_ms, stagger_delay_ms, wick_easing, body_easing, count },
    NumberCounter { duration_ms, easing, from, to, decimals },
    DataMorph { duration_ms, easing, data_points },
    AreaFill { line_duration_ms, fill_duration_ms, fill_delay_ms, line_easing, fill_easing, path_length },
    PieSliceGrow { duration_ms, stagger_delay_ms, easing, count },
    HeatmapFade { cell_duration_ms, stagger_delay_ms, easing, rows, cols },
    TickerFlash { flash_duration_ms, fade_duration_ms, easing, direction: TickerDirection },
}

// Duration accounts for stagger:
let ms: u64 = anim.total_duration_ms();
```

#### Other Recipe Categories

| Category | Type | Typical patterns |
|---|---|---|
| `lists::ListAnimation` | `ListAnimation` | Item enter/exit, reorder, skeleton loading, filter transition |
| `loading::LoadingAnimation` | `LoadingAnimation` | Spinner, progress bar, skeleton shimmer, dots bounce |
| `modals::ModalAnimation` | `ModalAnimation` | Slide in/out, fade scale, backdrop blur |
| `scroll::ScrollAnimation` | `ScrollAnimation` | Reveal, parallax, sticky header |
| `toasts::ToastAnimation` | `ToastAnimation` | Slide in, stagger, dismiss |
| `transitions::TransitionAnimation` | `TransitionAnimation` | Page slide, cross-fade, shared element |

**Pattern — ticker price flash:**

```rust
use uzor::animation::recipes::ChartAnimation;
use uzor::animation::{Easing, AnimationCoordinator};

let flash = ChartAnimation::TickerFlash {
    flash_duration_ms: 150,
    fade_duration_ms: 400,
    easing: Easing::EaseOutCubic,
    direction: TickerDirection::Up,
};

// Schedule animation:
// 1. Flash in (green background appears)
coord.tween(ticker_id.clone(), "flash_opacity", 0.0, 1.0,
    0.15, Easing::Linear, now);

// 2. Fade out after flash
coord.tween(ticker_id.clone(), "flash_opacity", 1.0, 0.0,
    0.4, Easing::EaseOutCubic, now + 0.15);

// Render based on direction:
let color = match direction {
    TickerDirection::Up   => Color::from_hex("#22c55e").unwrap(), // green
    TickerDirection::Down => Color::from_hex("#ef4444").unwrap(), // red
};
let opacity = coord.get_or(&ticker_id, "flash_opacity", 0.0);
// Draw color rect with opacity
```

**Pattern — bar chart grow with stagger:**

```rust
use uzor::animation::stagger::{LinearStagger, StaggerOrigin};
use std::time::Duration;

let stagger = LinearStagger::new(Duration::from_millis(30))
    .from(StaggerOrigin::First);
let delays = stagger.delays(bars.len());

for (i, (bar, &delay)) in bars.iter().zip(delays.iter()).enumerate() {
    let start_time = current_time + delay.as_secs_f64();
    coord.tween(
        bar.widget_id.clone(),
        "height_progress",
        0.0, 1.0,
        0.4,
        Easing::EaseOutCubic,
        start_time,
    );
}

// In render:
for bar in &bars {
    let progress = coord.get_or(&bar.widget_id, "height_progress", 1.0);
    let render_height = bar.target_height * progress;
    draw_bar(bar.x, bar_bottom - render_height, bar.width, render_height);
}
```

**Pattern — modal appear with spring:**

```rust
let spring = Spring::bouncy();
coord.spring(modal_id.clone(), "scale", spring, 1.0, now);
coord.tween(modal_id.clone(), "opacity", 0.0, 1.0, 0.15, Easing::Linear, now);

// In render:
let scale = coord.get_or(&modal_id, "scale", 1.0);
let opacity = coord.get_or(&modal_id, "opacity", 1.0);
// Apply transform: scale around center
// Apply opacity as alpha
```

---

## Module Dependency Summary

```
panels/
  manager.rs          ← DockingManager (uses grid.rs, separator.rs, snap_back.rs, tabs.rs, floating.rs)
  grid.rs             ← DockingTree, Leaf, Branch, PanelNode
  tree.rs             ← PanelTree, PanelStore, Tile, Container (arena-based alt representation)
  tabs.rs             ← TabBar, TabDragController, TabBarInfo, TabItem, TabReorderState
  separator.rs        ← Separator, SeparatorController
  snap_back.rs        ← SnapBackAnimation (uses animation::Spring)
  drop_zone.rs        ← DropZone, DropZoneDetector, CompassZone
  floating.rs         ← FloatingWindow, FloatingDragState
  serialize.rs        ← LayoutSnapshot, SerializedNode
  mod.rs              ← DockPanel trait + all re-exports

panel_api/
  traits.rs           ← PanelApp trait, ToolbarPosition
  toolbar.rs          ← ToolbarItemDef, DropdownItemDef, PanelToolbarDef, ToolbarSectionDef
  types.rs            ← PanelRect, PanelInput, PanelOutput, PanelTheme, HitZone

animation/
  coordinator.rs      ← AnimationCoordinator
  types.rs            ← AnimationKey, AnimationDriver, ActiveAnimation
  spring.rs           ← Spring (analytical damped harmonic oscillator)
  easing.rs           ← Easing (30 Penner + CubicBezier + Steps)
  decay.rs            ← Decay (exponential friction)
  timeline.rs         ← Timeline, TimelinePlayback, Tween<T>, Animatable
  color.rs            ← Color, Oklab, Oklch, ColorSpace
  stagger.rs          ← LinearStagger, GridStagger, StaggerOrigin, GridOrigin, DistanceMetric
  scroll.rs           ← ScrollTimeline, ViewTimeline, ScrollTween<T>, ParallaxLayer
  blend.rs            ← blend, blend_weighted, AnimationLayer, resolve_layers,
                         InterruptionStrategy, AnimationSlot, AnimationTransition
  path.rs             ← MotionPath, PathSegment, PathSample
  stroke.rs           ← StrokeAnimation, StrokeState
  layers.rs           ← LayerStack, ManagedLayer
  recipes/            ← ButtonAnimation, ChartAnimation, ListAnimation, LoadingAnimation,
                         ModalAnimation, ScrollAnimation, ToastAnimation, TransitionAnimation
```
