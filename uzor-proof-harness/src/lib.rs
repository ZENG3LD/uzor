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
//! **Shape**: a new, tiny, `publish = false` dev-only crate — not a
//! `#[cfg(feature = ...)]` module bolted onto an existing crate. Reasons:
//! - `uzor-figures` (the first consumer) has ZERO runtime `[dependencies]`
//!   beyond `uzor` itself — a deliberate property (see that crate's own
//!   `CLAUDE.md` "Owns"/"Forbidden" contract). A feature flag on
//!   `uzor-figures` pulling in `uzor-render-vello-cpu`/`uzor-render-urx`/
//!   `uzor-urx-cpu` behind `#[cfg(feature = ...)]` would still register
//!   those as real (optional) entries in `[dependencies]`, visible to
//!   every downstream consumer's dependency graph and lockfile even when
//!   the feature is off — a `[dev-dependencies]` entry on a SEPARATE
//!   crate never does.
//! - `uzor-figures` is itself `publish = false` (incubating) and this is
//!   pure dev/test tooling — the same tier `uzor-graph`/`uzor-export`
//!   already occupy, and exactly the shape `uzor-examples/src/
//!   parity_harness.rs` already proved out for the URX Wave 6 arc; this
//!   crate promotes that SAME machinery (not a reimplementation) so
//!   `uzor-text`/`uzor-typeset` can reach it too without depending on
//!   `uzor-examples` (a binary-only crate with its own heavy GUI/wgpu
//!   dependency graph — completely wrong shape for a `[dev-dependencies]`
//!   entry on an engine crate).
//! - A single shared crate also means the divergence-policy tolerance
//!   ([`STRUCTURAL_DEFECT_FRACTION`]) and the composite-PNG layout are
//!   defined ONCE, reused identically by every consumer, rather than each
//!   crate copying its own slightly-different version.
//!
//! **Scope**: three CPU legs only — `tiny-skia` (`uzor-render-tiny-skia`),
//! `vello-cpu` (`uzor-render-vello-cpu`), and URX (`uzor-render-urx`
//! recording a `Scene`, rasterized by `uzor-urx-cpu::CpuBackend`). No GPU,
//! no window, fully headless — same discipline `uzor-export`'s own
//! render-to-file path already follows.
//!
//! ```no_run
//! use std::path::Path;
//! use uzor_proof_harness::{ChannelTolerance, ThreeLegDiff, ThreeLegRender, write_composite_png};
//!
//! let render = ThreeLegRender::capture(200, 100, |ctx| {
//!     ctx.set_fill_color("#ff0000");
//!     ctx.fill_rect(10.0, 10.0, 50.0, 50.0);
//! });
//! let diff = ThreeLegDiff::compute(&render, ChannelTolerance::default());
//! for line in diff.report_lines() {
//!     println!("{line}");
//! }
//! write_composite_png(&render, Path::new("out/example_backends.png")).expect("write composite");
//! assert!(diff.all_within_budget(), "structural backend divergence detected");
//! ```

mod compare;
mod composite;
mod render;

pub use compare::{compare_tight, ChannelTolerance, DiffReport, ThreeLegDiff, STRUCTURAL_DEFECT_FRACTION, STRUCTURAL_EDGE_TOLERANCE};
pub use composite::{write_composite_png, CompositeError};
pub use render::{render_tiny_skia, render_urx_cpu, render_vello_cpu, ThreeLegRender};

// Re-exported so a consumer's own proof test can write
// `uzor_proof_harness::RenderContext` without a separate `uzor`
// dev-dependency purely for the trait bound on its own draw closures.
pub use uzor::render::RenderContext;
