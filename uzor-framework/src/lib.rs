//! uzor-framework — end-to-end app runner crate.
//!
//! Builds on top of `uzor` (headless input/widgets) by providing a
//! complete window/event/render runtime so consumer apps only need to
//! implement business logic.
//!
//! Status: under construction. Currently provides only platform helpers
//! lifted from mylittlechart's production code.

pub mod platform;
