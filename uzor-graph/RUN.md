# uzor-graph — run + verify

`uzor-graph` is the reusable engine (this crate). `force-graph-demo`
(in `uzor-examples`) is the runnable proof: a synthetic ~534-node
clustered graph settling live via Barnes-Hut force simulation,
draggable, zoom/pan, click-to-select-into-sidebar.

No release build has been run from this harness (Windows linker
LNK1104 issues per project rule) — `cargo check` is the build gate;
the owner runs/visually verifies the desktop app.

## Launch

```
cd "C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor"
cargo run -p uzor-examples --bin force-graph-demo
```

Window title: "uzor-graph — force graph demo". Canvas on the left,
node-facts sidebar docked on the right (populates on click).

- **Pan**: left-drag on empty canvas.
- **Zoom**: scroll wheel, zooms around the cursor.
- **Drag a node**: left-drag directly on a node — it pins to the
  cursor, the simulation reheats and re-settles around it; on release
  it rejoins the simulation (unless persistently pinned via the
  `pin_node` agent action).
- **Select**: click a node (or click empty canvas to clear selection)
  — facts (label, category, degree, position, pinned) appear in the
  right sidebar.
- **Hover**: hovering a node dims everything outside its 1-hop
  neighborhood.

The graph is generated **deterministically** (index-seeded PRNG, no
time/OS randomness) — the same layout seed positions and topology
every run; only the *settled* positions differ run-to-run because the
force simulation itself is what moves them.

## Verify — Tier 1 (semantic, no screenshots)

Agent-api HTTP control plane on `:17481` (same pattern as
`l4-dashboard`'s `:17480`, see `uzor-agent-api/README.md`):

```bash
curl -s :17481/blackboxes                          # -> ["graph"]
curl -s :17481/blackbox/graph/state | jq .

# node_count / visible_node_count / alpha / hot should all be present;
# alpha should trend toward 0 and "hot" toward false over a few seconds
# as the sim settles.
curl -s :17481/blackbox/graph/state | jq '{node_count, visible_node_count, alpha, hot}'

# Select a node by index (0..node_count-1) or by its deterministic label
# (e.g. "hub-0", "c0n0" .. "c5n87").
curl -s -X POST :17481/blackbox/graph/action \
  -d '{"name":"select_node","args":{"label":"hub-0"}}'
curl -s :17481/blackbox/graph/state | jq .selected     # -> {"label":"hub-0", ...}

curl -s -X POST :17481/blackbox/graph/action -d '{"name":"clear_selection","args":{}}'
curl -s :17481/blackbox/graph/state | jq .selected     # -> null

# Pin/unpin by index.
curl -s -X POST :17481/blackbox/graph/action -d '{"name":"pin_node","args":{"index":0}}'
curl -s -X POST :17481/blackbox/graph/action -d '{"name":"select_node","args":{"index":0}}'
curl -s :17481/blackbox/graph/state | jq '.selected.pinned'   # -> true
curl -s -X POST :17481/blackbox/graph/action -d '{"name":"unpin_node","args":{"index":0}}'

curl -s -X POST :17481/blackbox/graph/action -d '{"name":"fit_view","args":{}}'
curl -s :17481/blackbox/graph/state | jq .camera

curl -s -X POST :17481/blackbox/graph/action \
  -d '{"name":"set_camera","args":{"pan_x":0,"pan_y":0,"zoom":1.0}}'
```

`select_node` should flip `.selected` in the state snapshot — the
same mutation a real mouse click on that node performs (`GraphEngine::select`,
called from both `GraphEngine::on_event` and
`GraphEngine`'s `BlackboxAgentSurface::apply_agent_action`).

Unknown actions return `ok:false`:

```bash
curl -s -X POST :17481/blackbox/graph/action -d '{"name":"expand_cluster","args":{}}'
# -> {"ok":false,"message":"unknown action \"expand_cluster\""}
# (clustering/collapse is explicitly deferred this run — see cluster.rs)
```

## Verify — Tier 2 (visual, before/after screenshot diff)

Per project rule, one screenshot is never proof — diff before/after:

```bash
curl -s :17481/screenshot/main -o before.png
curl -s -X POST :17481/blackbox/graph/action \
  -d '{"name":"select_node","args":{"label":"hub-0"}}'
curl -s :17481/screenshot/main -o after.png
# expect: sidebar populated with "selected: hub-0" / "category: hub" /
# a white selection ring around the hub-0 node; 6 visually-separated
# clusters once the sim has settled (not a single stripe/smear).
```

Watch the window directly for a few seconds after launch — nodes
should visibly spread out from their seeded scatter into 6 separated
clusters bridged by a hub ring, then stop moving (freeze) once
settled. Drag any node — the local neighborhood should visibly
re-settle around it, then freeze again on release.

## What's implemented this run

- `Graph<N, E>` — generic node/edge model, opaque `payload`/`category`,
  adjacency + degree cache (`graph.rs`).
- `Particle` SoA position/velocity/pin store (`particle.rs`).
- `Layout` trait + `ForceDirectedLayout` (`layout/`): many-body
  repulsion (brute-force under 500 particles, Barnes-Hut θ=1.0 above),
  link spring, center force, optional pairwise collision,
  semi-implicit-Euler integration, alpha cooling, framerate-independent
  step normalization, `reheat`.
- `Camera2D` — pan/zoom/fit-view, plus the single shared
  `node_screen_radius` helper render AND hit-test both call (the
  concrete fix for the rejected MVP's click-miss root cause).
- `interaction::pick` — screen-space nearest-node hit-test using that
  shared helper.
- `interaction::drag` — `DragController` built on native
  `WidgetResponse`/`InputState`/`DragState`/`create_response`, not a
  hand-rolled drag state machine.
- `interaction::focus::FocusSet` — shared hover-neighborhood
  dim/highlight set.
- `render.rs` — node/edge draw via `BatchPainter`, viewport culling,
  zoom-faded labels, deterministic category-color palette.
- `agent.rs` — `BlackboxAgentSurface` impl: state (node/visible counts,
  selected, hovered, camera, alpha/hot) + actions (`select_node`,
  `clear_selection`, `pin_node`, `unpin_node`, `set_camera`,
  `fit_view`).
- `engine.rs` — `GraphEngine` facade: owns everything above, drives the
  sim from real elapsed time, raw-`PlatformEvent` pan/zoom/drag/click
  handling, freeze/wake `RenderRegion` selection.
- `force-graph-demo` (`uzor-examples/src/l4/force_graph_demo.rs`) —
  deterministic ~534-node synthetic clustered graph, `lm::sidebar`
  facts panel, agent-api on `:17481`.

## What's explicitly deferred (see in-tree comments)

- **Clustering / group-collapse** — `cluster.rs` has the `GroupId`/
  `GroupNode` seam types only; no `ActiveView` projection, no
  collapse/expand mechanism, nothing constructs or reads them yet.
- **`HierarchicalLayout` / `RadialLayout`** — trait stubs in
  `layout/stubs.rs`; both compile and report settled immediately
  (safe no-op), real algorithms not implemented.
- **Porting the Yozarest forensic app onto this engine** — separate
  later run; this crate has zero forensic/case-specific types by
  design.
- **3D** — 2D only, per the engine design doc.
- **Direct `uzor-render-wgpu-instanced` `QuadInstance`/`LineInstance`
  wiring** — deliberately not built; the design doc frames it as an
  escape valve gated on measured frame time on a fully-expanded
  worst case, not a speculative baseline. `BatchPainter` already
  gives batched draw calls on every backend.
