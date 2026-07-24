//! `uzor-proof-harness` — dev-only multi-backend proof-render harness.
//!
//! **Why this exists**: every visual proof render across the figures/
//! text/typeset stack rasterizes through `uzor-export::render_to_png`,
//! which is HARDCODED to `tiny-skia` (`uzor-export/src/lib.rs:46,162`) —
//! the workspace's last-resort fallback rasterizer family, not one of the
//! two production families (vello, URX). Two real backend-specific
//! defects (tiny-skia's `fill_text` ignoring a rotation transform; a
//! shaper outline-rounding bug) were invisible to single-backend proofs
//! before this crate existed. Owner directive: "если может быть какой-то
//! диф в работе бекендов — тестируй на всех семьях; везде есть цпу
//! варианты" — render every proof through all three CPU family legs and
//! let divergence surface loudly, not silently.
//!
//! **GPU legs added** (owner follow-up: "могут ли быть расхождения на
//! гпу? надо бы тоже включить их" — the CPU legs already caught a
//! world-frame CTM composition bug and a missing-vertical-AA defect;
//! the GPU legs test a DIFFERENT axis — the actual rasterisers (compute
//! pipeline vs scanline, SDF/MSAA vs analytic, glyph atlas vs CPU glyph
//! rasterisation) and the URX GPU degrade counters).
//! [`render_vello_gpu`] and [`render_urx_wgpu`] each return an
//! `Option`/a struct wrapping one, and [`MultiLegRender::capture`] SKIPS
//! a leg gracefully (printing why) when no GPU/software adapter is
//! available — the whole suite still runs, minus those two legs, on a
//! GPU-less machine. The composite PNG and every pairwise diff table
//! then only cover legs that actually rendered.
//!
//! ```no_run
//! use std::path::Path;
//! use uzor_proof_harness::{ChannelTolerance, MultiLegDiff, MultiLegRender, write_composite_png};
//!
//! let render = MultiLegRender::capture(200, 100, |ctx| {
//!     ctx.set_fill_color("#ff0000");
//!     ctx.fill_rect(10.0, 10.0, 50.0, 50.0);
//! });
//! let diff = MultiLegDiff::compute(&render, ChannelTolerance::default());
//! for line in diff.report_lines() {
//!     println!("{line}");
//! }
//! write_composite_png(&render, Path::new("out/example_backends.png")).expect("write composite");
//! assert!(diff.all_within_budget(), "structural backend divergence detected");
//! ```

mod compare;
mod composite;
mod render;
mod urx_degrade_recorder;

pub use compare::{compare_tight, ChannelTolerance, DiffReport, MultiLegDiff, STRUCTURAL_DEFECT_FRACTION, STRUCTURAL_EDGE_TOLERANCE};
pub use composite::{write_composite_png, CompositeError};
pub use render::{render_tiny_skia, render_urx_cpu, render_urx_wgpu, render_vello_cpu, render_vello_gpu, MultiLegRender, UrxWgpuRender};

// Re-exported so a consumer's own proof test can write
// `uzor_proof_harness::RenderContext` without a separate `uzor`
// dev-dependency purely for the trait bound on its own draw closures.
pub use uzor::render::RenderContext;
