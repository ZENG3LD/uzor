//! `uzor-graph` — reusable, live force-directed graph visualization
//! engine for uzor.
//!
//! Generic node/edge model (opaque `payload`/`category`, no
//! forensic/case-specific types), pluggable [`Layout`] algorithms
//! (primary: [`ForceDirectedLayout`] — d3-force-style many-body/link/
//! center forces, Barnes-Hut above ~500 particles, velocity-Verlet-style
//! semi-implicit-Euler integration, alpha cooling with freeze/wake),
//! [`Camera2D`] pan/zoom, native-uzor drag/pick interaction (built on
//! `WidgetResponse`/`Sense`/`InputState`, not a bespoke drag machine),
//! and a [`uzor::layout::agent::BlackboxAgentSurface`] impl so the whole
//! thing is driveable headlessly over `uzor-agent-api`.
//!
//! Clustering/collapse, `HierarchicalLayout`/`RadialLayout`, and 3D are
//! explicitly out of scope this run — see [`cluster`] and
//! [`layout::stubs`] for the seams left in place.
//!
//! See `RUN.md` for the runnable demo (`uzor-examples --bin force-graph-demo`)
//! and agent-api verification steps.

pub mod agent;
pub mod camera;
pub mod cluster;
pub mod engine;
pub mod graph;
pub mod interaction;
pub mod layout;
pub mod particle;
pub mod render;

pub use camera::{Aabb, Camera2D};
pub use engine::{GraphEngine, NodeFacts};
pub use graph::{EdgeIndex, Graph, GraphEdge, GraphNode, NodeIndex, SimEdge, SimTopology};
pub use interaction::focus::FocusSet;
pub use layout::{ForceDirectedLayout, ForceParams, HierarchicalLayout, Layout, LayoutTickResult, RadialLayout};
pub use particle::Particle;
