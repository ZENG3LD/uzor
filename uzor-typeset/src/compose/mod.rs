//! Compose plane — `compose()` flows a scene's blocks through
//! [`crate::region::RegionSequence`], producing one
//! [`crate::region::Frame`] per region consumed (design doc §3.2).

mod flow;
mod island_layout;
pub mod keep_break;
mod list_layout;
mod paragraph_split;
mod table_layout;

pub use flow::{compose, ComposeStyle};
pub use keep_break::BreakControl;
pub(crate) use paragraph_split::{lines_fitting, slice_layout_lines, widow_orphan_count};
