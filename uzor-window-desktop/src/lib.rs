//! Desktop backend helpers for uzor using winit.
//!
//! This crate keeps the event mapper, winit window provider, and Win32 DWM
//! helpers that `uzor-framework`'s native host calls. The old
//! `ApplicationHandler` shells and the hand-wired input bridge were removed
//! in brief C1 — the native host owns the event loop now.

pub use uzor;

// Re-export windowing dependency to avoid version conflicts
pub use winit;

pub mod event_mapper;
pub mod winit_provider;
pub mod win_dwm;

pub use winit_provider::{SendSyncHandlePair, WinitSoftbufferPresenter, WinitWindowProvider};
