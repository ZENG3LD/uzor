//! Slice targets — one composed scene, sliced into pages | slides | frames
//! (design doc §4). P0 builds [`pages`] only; `slides`/`frames` are P3/P4
//! additions.

pub mod pages;

pub use pages::{slice_pages, Margins, Page, PageMaster};
