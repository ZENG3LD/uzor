//! Native wgpu pipelines — one module per primitive family.
//!
//! Commit 1 ships the Quad SDF pipeline only (`FillRect`/`StrokeRect`,
//! solid brush). `line`/`path` modules land in Commit 2/3.

pub(crate) mod quad;
