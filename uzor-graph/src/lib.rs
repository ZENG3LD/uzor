//! `uzor-graph` — reusable, live force-directed graph visualization
//! engine for uzor.
//!
//! Generic node/edge model (opaque `payload`/`category`, no
//! forensic/case-specific types), pluggable [`Layout`] algorithms
//! ([`ForceDirectedLayout`] — d3-force-style many-body/link/center
//! forces, Barnes-Hut above ~500 particles, velocity-Verlet-style
//! semi-implicit-Euler integration, alpha cooling with freeze/wake;
//! [`HierarchicalLayout`]/[`RadialLayout`] — one-shot Sugiyama-lite
//! layering, cartesian or polar; [`GraphLayoutMode`] — runtime dispatch
//! between all three), [`Camera2D`] pan/zoom, native-uzor drag/pick
//! interaction (built on `WidgetResponse`/`Sense`/`InputState`, not a
//! bespoke drag machine), cluster collapse ([`cluster::ClusterRegistry`]
//! — one super-node per collapsed cluster, aggregated cross-cluster
//! edges, exact-position expand), and a
//! [`uzor::layout::agent::BlackboxAgentSurface`] impl so the whole thing
//! is driveable headlessly over `uzor-agent-api`.
//!
//! `FocusSet` is re-pointed onto [`uzor_figures::interact::FocusSet`]
//! (Phase D, 2026-07-17) — see [`interaction`]'s module doc and
//! `graph.rs`'s `From<NodeIndex/EdgeIndex> for u64` key scheme. 3D stays
//! explicitly out of scope (2D only, per the engine design doc).
//!
//! See `RUN.md` for the runnable demo (`uzor-examples --bin force-graph-demo`)
//! and agent-api verification steps.

pub mod agent;
pub mod camera;
pub mod cluster;
pub mod engine;
pub mod graph;
pub mod interaction;
pub mod label_grid;
pub mod layout;
pub mod particle;
pub mod render;

pub use camera::{Aabb, Camera2D};
pub use cluster::{AggregatedEdge, ClusterRegistry, ClusterState, GroupId};
pub use engine::{DragEndPolicy, GraphEngine, NodeFacts};
pub use graph::{EdgeIndex, Graph, GraphEdge, GraphNode, NodeIndex, SimEdge, SimTopology};
pub use layout::{
    ForceDirectedLayout, ForceParams, GraphLayoutMode, HierarchicalLayout, HierarchicalParams, Layout, LayoutKind,
    LayoutTickResult, RadialLayout, RadialParams,
};
pub use particle::Particle;
pub use uzor_figures::interact::FocusSet;

#[cfg(test)]
mod proof_tests {
    //! Headless proof: render [`HierarchicalLayout`]/[`RadialLayout`]/a
    //! collapsed cluster via `uzor-export` at a fixed resolution with
    //! deterministic (seeded, no RNG/time) fixtures, assert a valid PNG
    //! comes out, and ALSO write it to `uzor/out/` so a human can
    //! eyeball the result (same convention `uzor-figures`'s own
    //! `proof_tests` module follows).

    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    use crate::engine::GraphEngine;
    use crate::graph::{Graph, NodeIndex};
    use crate::layout::force_directed::ForceDirectedLayout;
    use crate::layout::hierarchical::{HierarchicalLayout, HierarchicalParams};
    use crate::layout::radial::{RadialLayout, RadialParams};

    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 600;

    fn export_spec() -> ExportSpec {
        ExportSpec { width_px: WIDTH, height_px: HEIGHT, dpr: 1.0, background: Some([13, 15, 20, 255]) }
    }

    fn out_dir() -> std::path::PathBuf {
        // Fixed path (not CARGO_MANIFEST_DIR-relative) — `uzor/out/` is
        // the shared human-eyeball drop point for every headless proof
        // render in this workspace.
        std::path::PathBuf::from(r"C:\Users\VA PC\CODING\ML_TRADING\nemo\uzor\out")
    }

    fn write_proof_png(name: &str, bytes: &[u8]) {
        let dir = out_dir();
        std::fs::create_dir_all(&dir).expect("create uzor/out/ proof directory");
        std::fs::write(dir.join(name), bytes).expect("write proof PNG");
    }

    fn decoded_png_dims(bytes: &[u8]) -> (u32, u32) {
        let decoder = png::Decoder::new(bytes);
        let reader = decoder.read_info().expect("valid PNG header");
        let info = reader.info();
        (info.width, info.height)
    }

    type DemoGraph = Graph<(), ()>;

    /// Deterministic 3-root tree/DAG (3 roots x 3 mid-nodes x 2 leaves =
    /// 30 nodes, no RNG/time) shared by the hierarchical and radial
    /// proofs — the design brief's "same layering but polar".
    fn seeded_tree_fixture() -> DemoGraph {
        let mut graph = DemoGraph::new();
        for r in 0..3 {
            let root = graph.push_node((), format!("root-{r}"), format!("cluster-{r}"), 7.0);
            for c in 0..3 {
                let mid = graph.push_node((), format!("r{r}-c{c}"), format!("cluster-{r}"), 5.0);
                graph.push_edge(root, mid, 1.0, ());
                for l in 0..2 {
                    let leaf = graph.push_node((), format!("r{r}-c{c}-l{l}"), format!("cluster-{r}"), 4.0);
                    graph.push_edge(mid, leaf, 1.0, ());
                }
            }
        }
        graph
    }

    /// Deterministic 3-cluster x 8-node fixture (Phase D's own "seeded
    /// clustered fixture" brief) plus one connecting hub — reused for
    /// the collapse proof.
    fn seeded_cluster_fixture() -> (DemoGraph, [Vec<NodeIndex>; 3]) {
        let mut graph = DemoGraph::new();
        let hub = graph.push_node((), "hub", "hub", 5.0);
        let mut clusters: [Vec<NodeIndex>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for (c, cluster) in clusters.iter_mut().enumerate() {
            let mut members = Vec::new();
            for m in 0..8 {
                let id = graph.push_node((), format!("c{c}n{m}"), format!("cluster-{c}"), 4.0);
                members.push(id);
            }
            for i in 0..members.len() {
                graph.push_edge(members[i], members[(i + 1) % members.len()], 1.0, ());
            }
            graph.push_edge(hub, members[0], 1.0, ());
            graph.push_edge(members[1], hub, 0.6, ());
            *cluster = members;
        }
        (graph, clusters)
    }

    #[test]
    fn hierarchical_layout_renders_layered_graph_to_a_valid_png() {
        let graph = seeded_tree_fixture();
        let mut engine: GraphEngine<(), (), HierarchicalLayout> =
            GraphEngine::new(graph, HierarchicalLayout::new(HierarchicalParams::default()));
        engine.set_canvas_rect(Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64));
        engine.tick(1.0 / 60.0);
        engine.fit_view();

        let bytes = render_to_png(&export_spec(), |ctx| {
            engine.draw(ctx);
        })
        .expect("hierarchical graph should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("graph_hierarchical.png", &bytes);
    }

    #[test]
    fn radial_layout_renders_ringed_graph_to_a_valid_png() {
        let graph = seeded_tree_fixture();
        let mut engine: GraphEngine<(), (), RadialLayout> = GraphEngine::new(graph, RadialLayout::new(RadialParams::default()));
        engine.set_canvas_rect(Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64));
        engine.tick(1.0 / 60.0);
        engine.fit_view();

        let bytes = render_to_png(&export_spec(), |ctx| {
            engine.draw(ctx);
        })
        .expect("radial graph should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("graph_radial.png", &bytes);
    }

    #[test]
    fn collapsed_cluster_renders_supernode_to_a_valid_png() {
        let (graph, clusters) = seeded_cluster_fixture();
        let mut engine: GraphEngine<(), (), ForceDirectedLayout> = GraphEngine::new(graph, ForceDirectedLayout::default());

        // Deterministic seed scatter — golden-angle-ish ring per
        // cluster, same convention `force_graph_demo` uses.
        let mut positions = vec![(0.0f32, 0.0f32)]; // hub at origin
        for c in 0..3 {
            let angle = c as f32 * 2.094_395; // 2*pi/3
            let cx = angle.cos() * 160.0;
            let cy = angle.sin() * 160.0;
            for m in 0..8 {
                let a = m as f32 / 8.0 * std::f32::consts::TAU;
                positions.push((cx + a.cos() * 40.0, cy + a.sin() * 40.0));
            }
        }
        engine.seed_positions(&positions);

        let group_a = engine.define_cluster(clusters[0].clone()).expect("cluster 0 is non-empty");
        engine.define_cluster(clusters[1].clone());
        engine.define_cluster(clusters[2].clone());

        engine.set_canvas_rect(Rect::new(0.0, 0.0, WIDTH as f64, HEIGHT as f64));
        for _ in 0..200 {
            engine.tick(1.0 / 60.0);
        }
        assert!(engine.collapse_cluster(group_a));
        engine.fit_view();

        let bytes = render_to_png(&export_spec(), |ctx| {
            engine.draw(ctx);
        })
        .expect("collapsed cluster graph should render");
        assert_eq!(decoded_png_dims(&bytes), (WIDTH, HEIGHT));
        write_proof_png("graph_collapsed.png", &bytes);
    }
}
