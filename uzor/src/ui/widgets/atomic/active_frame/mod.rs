//! Active frame — non-interactive stroke overlay highlighting an active rect.
//!
//! Composites (dock leaves, panels, list rows…) attach one to mark the
//! currently-active item. It owns no input — purely paint, registered as a
//! Sense::NONE atomic so it never intercepts hits.

pub mod render;
pub mod types;
