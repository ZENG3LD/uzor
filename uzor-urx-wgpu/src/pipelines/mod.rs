//! Native wgpu pipelines — one module per primitive family.
//!
//! Commit 1 shipped the Quad SDF pipeline (`FillRect`/`StrokeRect`,
//! solid brush). Commit 2 adds the Line/capsule pipeline. `path` joins
//! in Commit 3.

pub(crate) mod line;
pub(crate) mod quad;
