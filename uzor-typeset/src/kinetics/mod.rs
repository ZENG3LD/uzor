//! Kinetics — block-level morph between two ADJACENT [`crate::slice::
//! frames::ComposedFrame`]s (design doc §4.3, Arc 4 Phase P4).
//!
//! Two nested identity systems stay strictly layered, exactly as the
//! design doc's own P4 risk note requires: [`identity`] matches whole
//! BLOCKS by [`crate::scene::BlockId`] (this crate's own domain); when a
//! matched pair is both [`crate::scene::Block::Paragraph`] with
//! byte-identical text, [`morph`] hands the WHOLE `ParagraphLayout` pair to
//! `uzor_text::kinetics::{build_morph, sample_layout}` unchanged and takes
//! back its sampled output verbatim — this crate never reaches into a
//! matched paragraph's own glyphs itself.

mod identity;
mod morph;

pub use morph::{build_frame_morph, FrameBlockState, FrameMorph};
