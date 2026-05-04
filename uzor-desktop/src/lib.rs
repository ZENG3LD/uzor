//! `uzor-desktop` — winit-based desktop runtime for uzor apps.
//!
//! Provides the full native desktop lifecycle for uzor applications:
//!
//! - [`manager::Manager`] — the L4 event-loop driver (winit `ApplicationHandler`).
//! - [`builder_run::AppRun`] — extension trait that adds `.run()` to
//!   `uzor::framework::AppBuilder`.
//! - [`tray`] — system tray icon + context menu wrapper.
//! - [`chrome`] — desktop chrome wiring helpers.
//! - [`utils`] — screenshot, single-instance guard, Win32 resource embedding.
//! - [`platform`] — Win32 cursor polling and DWM border helpers.
//! - [`window`] — lower-level window creation, per-window state, window registry.

pub mod manager;
pub mod builder_run;
pub mod utils;
pub mod platform;
pub mod window;

#[cfg(not(target_arch = "wasm32"))]
pub mod chrome;
#[cfg(not(target_arch = "wasm32"))]
pub mod tray;

// ── Primary re-exports ────────────────────────────────────────────────────────

pub use manager::{Manager, ManagerError};
pub use builder_run::{AppRun, run_closure};

#[cfg(not(target_arch = "wasm32"))]
pub use tray::{TrayBuilder, TrayError, TrayEvent, TrayHandle};
