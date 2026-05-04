//! `uzor-framework` — thin re-export shim.
//!
//! The actual implementation has been split:
//! - Core L4 API (traits, builder, multi-window types) → `uzor::framework::*`
//! - Desktop runtime (winit event loop, window manager, tray, chrome, utils) → `uzor-desktop`
//!
//! This crate re-exports both so existing callers continue to work unchanged.

// ── Core L4 API (from uzor::framework) ───────────────────────────────────────

pub use uzor::framework::app::{App, AppConfig, ClosureApp, NoPanel};
pub use uzor::framework::builder::{AppBuilder, BuildError};
pub use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};

// ── Desktop runtime (from uzor-desktop) ──────────────────────────────────────

pub use uzor_desktop::manager::{Manager as WindowManager, ManagerError as WindowManagerError};
pub use uzor_desktop::manager::{Manager as Runtime, ManagerError as RuntimeError};
pub use uzor_desktop::builder_run::{AppRun, run_closure};

#[cfg(not(target_arch = "wasm32"))]
pub use uzor_desktop::tray::{TrayBuilder, TrayError, TrayEvent, TrayHandle};

#[cfg(not(target_arch = "wasm32"))]
pub use uzor_desktop::chrome;

pub use uzor_desktop::utils;
pub use uzor_desktop::platform;

#[cfg(not(target_arch = "wasm32"))]
pub use uzor_desktop::utils::single_instance::{single_instance, SingleInstanceGuard};
#[cfg(not(target_arch = "wasm32"))]
pub use uzor_desktop::utils::screenshot::{
    add_copy_src_to_target_texture, capture_screenshot, encode_png, screenshot_save_dir,
};

// ── Layout re-export (from uzor-framework's own layout.rs) ───────────────────
pub mod layout;

// ── Widgets (from uzor::framework::widgets) ──────────────────────────────────
pub mod widgets {
    pub use uzor::framework::widgets::{coord, ctx, lm};
}
pub use widgets::{coord, ctx, lm};

// ── Back-compat re-export of app mod ─────────────────────────────────────────
pub mod app {
    pub use uzor::framework::app::{App, AppConfig, ClosureApp, NoPanel};
}

// ── Back-compat re-export of multi_window mod ────────────────────────────────
pub mod multi_window {
    pub use uzor::framework::multi_window::{WindowCtx, WindowKey, WindowSpec};
}

// ── Back-compat: window_manager mod alias ────────────────────────────────────
pub mod window_manager {
    pub use uzor_desktop::manager::{Manager as WindowManager, ManagerError as WindowManagerError};
    pub use uzor_desktop::manager::ManagerError;
    // Re-export types that window_manager.rs used to own
    pub use uzor_desktop::manager::Manager;
}

/// Backward-compatibility alias: the manager used to be called `runtime`.
pub use window_manager as runtime;

// ── Mirage-derived design tokens ──────────────────────────────────────────────

#[allow(dead_code, non_upper_case_globals)]
pub mod tokens {
    include!("tokens_generated.rs");
}

// ── JSX-mimicking macro DSL ───────────────────────────────────────────────────

pub use uzor_framework_macros::view;

// ── Hub re-exports ────────────────────────────────────────────────────────────

pub use uzor_render_hub as render_hub;
pub use uzor_window_hub as window_hub;

// ── Builder mod re-export for back-compat ────────────────────────────────────
pub mod builder {
    pub use uzor::framework::builder::{AppBuilder, BuildError, AnyFactory};
    pub use uzor_desktop::manager::ManagerError as RuntimeError;
}
