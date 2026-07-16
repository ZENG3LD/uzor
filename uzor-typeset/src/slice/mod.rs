//! Slice targets — one composed scene, sliced into pages | slides | frames
//! (design doc §4). P0/P1/P2 build [`pages`] only; `slides`/`frames` are
//! P3/P4 additions.
//!
//! [`crate::master::PageMaster`]/[`crate::master::Margins`] moved to
//! `crate::master` this phase (P2) — re-exported here so every existing
//! `crate::slice::{Margins, PageMaster}` import path still resolves
//! unchanged (additive law).

pub mod pages;

pub use crate::master::{Margins, PageMaster};
pub use pages::{slice_pages, Page, PageNumberPlacement};
