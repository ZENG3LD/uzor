//! # `uzor-graph` engine demo — force-directed, live, draggable
//!
//! Synthetic clustered graph (~530 nodes, 6 clusters bridged by a hub
//! ring), generated **deterministically** (index-seeded, no
//! time/`Math.random`) so the engine is proven against a graph shape
//! that isn't pathologically skewed toward the one forensic case that
//! motivated it (see `uzor-graph/RUN.md` / the engine design doc).
//!
//! Watch it settle live, drag a node (it pins + the sim re-settles
//! around it), scroll to zoom, drag empty canvas to pan, click a node
//! to select it and see facts in the right sidebar.
//!
//! Run:
//! ```sh
//! cargo run -p uzor-examples --bin force-graph-demo
//! ```
//!
//! Agent-api verification: see `uzor-graph/RUN.md`.

use std::sync::{Arc, Mutex, MutexGuard};

use uzor::core::types::Rect;
use uzor::framework::app::{App, NoPanel};
use uzor::framework::builder::AppBuilder;
use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
use uzor::input::PlatformEvent;
use uzor::layout::{EdgeSide, EdgeSlot, LayoutManager};
use uzor::platform::types::CornerStyle;
use uzor::render::{RenderContext, RenderRegion};
use uzor::types::unsafe_widget_id;
use uzor_desktop::AppRun as _;

use uzor_graph::{ForceDirectedLayout, Graph, GraphEngine, NodeIndex};

const AGENT_PORT: u16 = 17481;
const BLACKBOX_SLOT: &str = "graph";
const SIDEBAR_SLOT: &str = "sidebar";
const SIDEBAR_WIDTH: f32 = 320.0;

const NUM_CLUSTERS: usize = 6;
const CLUSTER_SIZE: usize = 88;
const EDGES_PER_NODE: usize = 3;

// ── Deterministic PRNG — index-seeded, no time/OS randomness ────────────

/// splitmix64-style generator. Seeded purely by caller-supplied indices
/// so the whole demo graph is byte-for-byte reproducible.
struct DetRng(u64);

impl DetRng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range_usize(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next_u64() % n as u64) as usize }
    }
}

// ── Synthetic graph ──────────────────────────────────────────────────────

type DemoGraph = Graph<(), ()>;

/// 6 clusters of `CLUSTER_SIZE` members each, plus one hub node per
/// cluster. Intra-cluster edges are deterministic-random; the hub fans
/// out to a subset of its cluster; hubs form a ring — the only
/// inter-cluster edges, so clusters stay visually separable once
/// settled (unlike the rejected MVP's hop-depth-0 megacolumn).
fn build_demo_graph() -> (DemoGraph, Vec<(f32, f32)>) {
    let mut graph = DemoGraph::new();
    let mut positions = Vec::new();
    let mut cluster_members: Vec<Vec<NodeIndex>> = vec![Vec::new(); NUM_CLUSTERS];
    let mut hubs = Vec::with_capacity(NUM_CLUSTERS);

    for cluster in 0..NUM_CLUSTERS {
        // Golden-angle ring placement spreads cluster centers apart
        // before the sim even starts.
        let angle = cluster as f32 * 2.399_963;
        let cx = angle.cos() * 420.0;
        let cy = angle.sin() * 420.0;

        for member in 0..CLUSTER_SIZE {
            let mut rng = DetRng::new((cluster as u64) << 32 | member as u64);
            let jitter_r = rng.next_f32() * 140.0;
            let jitter_a = rng.next_f32() * std::f32::consts::TAU;
            let x = cx + jitter_a.cos() * jitter_r;
            let y = cy + jitter_a.sin() * jitter_r;

            let id = graph.push_node((), format!("c{cluster}n{member}"), format!("cluster-{cluster}"), 4.0);
            cluster_members[cluster].push(id);
            positions.push((x, y));
        }

        let hub_id = graph.push_node((), format!("hub-{cluster}"), "hub", 4.0);
        hubs.push(hub_id);
        positions.push((cx, cy));
    }

    for cluster in 0..NUM_CLUSTERS {
        let members = &cluster_members[cluster];
        for (i, &node) in members.iter().enumerate() {
            let mut rng = DetRng::new(0xC0FF_EE00 ^ ((cluster as u64) << 20) ^ i as u64);
            for _ in 0..EDGES_PER_NODE {
                let j = rng.range_usize(members.len());
                if j != i {
                    graph.push_edge(node, members[j], 1.0, ());
                }
            }
        }
        for (i, &node) in members.iter().enumerate() {
            if i % 6 == 0 {
                graph.push_edge(hubs[cluster], node, 1.0, ());
            }
        }
    }

    for cluster in 0..NUM_CLUSTERS {
        let next = (cluster + 1) % NUM_CLUSTERS;
        graph.push_edge(hubs[cluster], hubs[next], 0.6, ());
    }

    // Radius by degree, computed after all edges are known.
    let ids: Vec<NodeIndex> = graph.nodes().map(|(id, _)| id).collect();
    for id in ids {
        let degree = graph.degree(id);
        graph.set_radius(id, 3.0 + (degree as f32).sqrt() * 1.6);
    }

    (graph, positions)
}

// ── App ───────────────────────────────────────────────────────────────

struct SelectedFacts {
    label: String,
    category: String,
    degree: u32,
    position: (f32, f32),
    pinned: bool,
}

type Engine = GraphEngine<(), (), ForceDirectedLayout>;

struct DemoApp {
    engine: Arc<Mutex<Engine>>,
    did_init_camera: bool,
}

impl DemoApp {
    fn new() -> Self {
        let (graph, positions) = build_demo_graph();
        let mut engine = GraphEngine::new(graph, ForceDirectedLayout::default());
        engine.seed_positions(&positions);
        engine.set_agent_slot_id(BLACKBOX_SLOT);
        Self { engine: Arc::new(Mutex::new(engine)), did_init_camera: false }
    }

    fn lock(engine: &Arc<Mutex<Engine>>) -> MutexGuard<'_, Engine> {
        match engine.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn draw_row(
    layout: &mut LayoutManager<NoPanel>,
    render: &mut dyn RenderContext,
    body_rect: Rect,
    cy: &mut f64,
    row_h: f64,
    pad: f64,
    w: f64,
    id: &str,
    text: &str,
) {
    let r = Rect { x: body_rect.x + pad, y: *cy, width: w, height: row_h };
    uzor::framework::widgets::lm::text(unsafe_widget_id(id), r, text).build(layout, render);
    *cy += row_h;
}

impl App<NoPanel> for DemoApp {
    fn init(&mut self, _key: &WindowKey, layout: &mut LayoutManager<NoPanel>) {
        layout.register_blackbox_agent(BLACKBOX_SLOT, self.engine.clone());
    }

    fn ui(&mut self, win: &mut WindowCtx<'_, NoPanel>) {
        win.layout.edges_mut().clear();
        win.layout.edges_mut().add(EdgeSlot {
            id: SIDEBAR_SLOT.to_owned(),
            side: EdgeSide::Right,
            thickness: SIDEBAR_WIDTH,
            visible: true,
            order: 0,
            ..Default::default()
        });

        let win_rect = win.layout.last_window().unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
        if win_rect.width > 0.0 && win_rect.height > 0.0 {
            win.layout.solve(win_rect);
        }

        let canvas_rect = win.layout.last_solved().map(|s| s.dock_area).unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));

        win.render.set_fill_color("#0d0f14");
        win.render.fill_rect(canvas_rect.x, canvas_rect.y, canvas_rect.width, canvas_rect.height);

        let (hot, alpha, node_count, visible_count, facts_owned) = {
            let mut engine = Self::lock(&self.engine);
            engine.set_canvas_rect(canvas_rect);

            if !self.did_init_camera && canvas_rect.width > 0.0 {
                engine.fit_view();
                self.did_init_camera = true;
            }

            engine.tick_real_time();

            if canvas_rect.width > 0.0 && canvas_rect.height > 0.0 {
                win.render.save();
                win.render.clip_rect(canvas_rect.x, canvas_rect.y, canvas_rect.width, canvas_rect.height);
                engine.draw(win.render);
                win.render.restore();
            }

            let facts = engine.selected_facts().map(|f| SelectedFacts {
                label: f.label.to_owned(),
                category: f.category.to_owned(),
                degree: f.degree,
                position: f.position,
                pinned: f.pinned,
            });
            let snapshot = (engine.is_hot(), engine.last_tick().alpha, engine.graph.node_count(), engine.visible_nodes().len(), facts);
            engine.clear_dirty();
            snapshot
        };

        let sb_handle = win.layout.add_sidebar(SIDEBAR_SLOT);
        {
            let layout = &mut *win.layout;
            let render = &mut *win.render;
            uzor::framework::widgets::lm::sidebar(&sb_handle, SIDEBAR_SLOT)
                .header_title("Graph")
                .content_height(360.0)
                .build_with_body(layout, render, |layout, render, body_rect| {
                    let pad = 12.0_f64;
                    let row_h = 20.0_f64;
                    let w = body_rect.width - 2.0 * pad;
                    let mut cy = body_rect.y + pad;

                    draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:nodes", &format!("nodes: {node_count}  visible: {visible_count}"));
                    draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:alpha", &format!("alpha: {alpha:.4}  hot: {hot}"));
                    cy += 8.0;

                    match &facts_owned {
                        Some(f) => {
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:label", &format!("selected: {}", f.label));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:category", &format!("category: {}", f.category));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:degree", &format!("degree: {}", f.degree));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:pos", &format!("pos: ({:.1}, {:.1})", f.position.0, f.position.1));
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:pinned", &format!("pinned: {}", f.pinned));
                        }
                        None => {
                            draw_row(layout, render, body_rect, &mut cy, row_h, pad, w, "graph:sel:none", "no selection — click a node");
                        }
                    }
                });
        }
    }

    fn regions(&mut self) -> Vec<RenderRegion> {
        let engine = Self::lock(&self.engine);
        vec![engine.render_region("force-graph-demo:main")]
    }

    fn on_event(&mut self, event: &PlatformEvent) -> bool {
        let mut engine = Self::lock(&self.engine);
        engine.on_event(event)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    AppBuilder::new(DemoApp::new())
        .agent_api(AGENT_PORT)
        .window(
            WindowSpec::new(WindowKey::new("main"), "uzor-graph — force graph demo")
                .size(1400, 900)
                .min_size(900, 600)
                .decorations(false)
                .background(0xFF_0d_0f_14)
                .corner_style(CornerStyle::Rounded)
                .border_color(0x00_4d_90_fe),
        )
        .icon_from_png(include_bytes!("../../assets/icon.png"))?
        .run()?;
    Ok(())
}
