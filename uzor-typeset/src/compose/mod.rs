//! Compose plane — `compose()` flows a scene's blocks through
//! [`crate::region::RegionSequence`], producing one
//! [`crate::region::Frame`] per region consumed (design doc §3.2).

mod flow;
mod paragraph_split;

pub use flow::{compose, ComposeStyle};
pub(crate) use paragraph_split::{lines_fitting, slice_layout_lines};
