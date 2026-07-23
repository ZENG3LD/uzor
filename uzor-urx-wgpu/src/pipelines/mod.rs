//! Native wgpu pipelines — one module per primitive family.
//!
//! Commit 1 shipped the Quad SDF pipeline (`FillRect`/`StrokeRect`,
//! solid brush). Commit 2 added the Line/capsule pipeline. Commit 3
//! adds the Path/triangle pipeline (lyon tessellation).

pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod quad;
