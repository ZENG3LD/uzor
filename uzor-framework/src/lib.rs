//! `uzor-framework` — application runtime + builder for uzor apps.
//!
//! Provides the full lifecycle glue between [`uzor-window-hub`], the uzor core
//! engine, and [`uzor-render-hub`]:
//!
//! - [`App`] — lifecycle trait (init → ui → shutdown).
//! - [`AppConfig`] — window / rendering / single-instance configuration.
//! - [`AppBuilder`] — fluent builder that produces a [`Runtime`].
//! - [`Runtime`] — drives the event loop until all windows close.
//! - [`utils`] — GPU screenshot capture, single-instance guard, timestamp helpers.
//! - [`platform`] — Win32 cursor polling and DWM border colour helpers.
//! - [`window`] — winit window creation, per-window GPU state, multi-window manager.

pub mod app;
pub mod builder;
pub mod layout;
pub mod runtime;
pub mod utils;
pub mod widgets;

#[cfg(not(target_arch = "wasm32"))]
pub mod chrome;
#[cfg(not(target_arch = "wasm32"))]
pub mod tray;
#[cfg(not(target_arch = "wasm32"))]
pub mod window;
#[cfg(not(target_arch = "wasm32"))]
pub mod platform;

// ── Primary re-exports ────────────────────────────────────────────────────────

pub use app::{App, AppConfig, ClosureApp, NoPanel};
pub use builder::{AppBuilder, BuildError, run_closure};
pub use runtime::{Runtime, RuntimeError};

// ── Utility re-exports (desktop only) ────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub use utils::single_instance::{single_instance, SingleInstanceGuard};
#[cfg(not(target_arch = "wasm32"))]
pub use utils::screenshot::{
    add_copy_src_to_target_texture, capture_screenshot, encode_png, screenshot_save_dir,
};

// ── Tray re-exports (desktop only) ───────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub use tray::{TrayBuilder, TrayError, TrayEvent, TrayHandle};

// ── Tier-organised widget shortcuts ──────────────────────────────────────────

/// Tier-organised widget registration shortcuts: `coord` (L1), `ctx` (L2), `lm` (L3).
pub use widgets::{coord, ctx, lm};

/// JSX-mimicking macro DSL — see `uzor-framework-macros` for grammar.
///
/// ```ignore
/// view! {
///     <col rect={body} gap=8>
///         <button text="Save" on_click={|| self.save()} />
///         <checkbox bind={&mut self.dark} label="Dark"/>
///     </col>
/// }
/// ```
pub use uzor_framework_macros::view;

// ── Hub re-exports ────────────────────────────────────────────────────────────

/// Re-export of `uzor-render-hub` for consumers that only depend on
/// `uzor-framework` and do not want a separate direct dependency.
pub use uzor_render_hub as render_hub;

/// Re-export of `uzor-window-hub` for consumers that only depend on
/// `uzor-framework` and do not want a separate direct dependency.
pub use uzor_window_hub as window_hub;
