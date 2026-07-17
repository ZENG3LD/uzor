//! Data-space transforms applied BEFORE a figure hands points to its own
//! mark/hit-test pipeline — currently just [`lttb`], a downsampling
//! transform. Kept separate from `scale`/`coord` (screen-space mapping)
//! and `mark` (pure draw) since a transform here operates on DOMAIN
//! points, before any [`crate::coord::PlotArea`] involvement.

pub mod lttb;

pub use lttb::lttb;
