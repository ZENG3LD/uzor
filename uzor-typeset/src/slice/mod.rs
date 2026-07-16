//! Slice targets — one composed scene, sliced into pages | slides | frames
//! (design doc §4). P0/P1/P2 built [`pages`] only; [`slides`] landed P3;
//! [`frames`] (build-steps) is this phase's own P4 addition.
//!
//! [`crate::master::PageMaster`]/[`crate::master::Margins`] moved to
//! `crate::master` this phase (P2) — re-exported here so every existing
//! `crate::slice::{Margins, PageMaster}` import path still resolves
//! unchanged (additive law).

pub mod frames;
pub mod pages;
pub mod slides;

pub use crate::master::{Margins, PageMaster};
pub use frames::{slice_build_steps, BlockOverride, BuildStep, ComposedFrame};
pub use pages::{slice_pages, Page, PageNumberPlacement};
pub use slides::{cards_to_slides, slice_cards, slice_slide_instance, slice_slides, slides_to_cards, Card, SliceError, Slide, SlideOverflow};
