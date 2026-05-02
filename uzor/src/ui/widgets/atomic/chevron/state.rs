//! Chevron persistent state.
//!
//! Chevron is mostly stateless — interaction flags live on `ChevronView`
//! per frame. This struct exists so the registry has somewhere to anchor
//! per-id metadata if a future feature needs it (e.g. a press-animation
//! timestamp). Today it's empty.

#[derive(Debug, Default, Clone, Copy)]
pub struct ChevronState;
