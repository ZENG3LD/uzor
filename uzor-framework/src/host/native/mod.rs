//! Native (winit) host shell — brief F9 (design §2.3, §5.4, §5.5, §7.3).
//!
//! Modules: mapper, app_handler, executor, paint via hub, window create,
//! clipboard, tray, single-instance, screenshot, Win32 NCHITTEST subclass.
//!
//! ## `uzor-render-hub` wasm32 verdict (F9 done-condition)
//!
//! Recorded 2026-10-04 (`rustup target add wasm32-unknown-unknown`, then
//! `cargo check -p uzor-render-hub --target wasm32-unknown-unknown`):
//! **FAILS** (7 errors). Notable: `recreate_target_with_cpu_usage` missing
//! under wasm cfg gates in `factory.rs`, `ensure_backend_slot` / `gpu_handles`
//! absent on the wasm `WindowRenderState` shape, and a non-exhaustive
//! `SurfaceKind` match in `submit_urx.rs` (Software arm). F10 must paint
//! through `uzor-render-canvas2d` (or a hub build with
//! `default-features = false` plus a wasm-safe feature set).

mod app_handler;
mod clipboard;
mod executor;
mod mapper;
mod paint;
mod screenshot;
mod single_instance;
mod tray;
mod win32;
mod window;
mod window_create;

pub use app_handler::{
    run_native, run_native_default, HostWake, NativeHost, NativeOptions, RenderPreference,
};
pub use clipboard::NativeClipboard;
pub use executor::{execute, ExecutorFollowUp, OsWindow, RecordingWindow};
pub use mapper::WinitMapper;
pub use screenshot::{
    add_copy_src_to_target_texture, capture_screenshot, capture_screenshot_texture, encode_png,
};
pub use single_instance::{single_instance, SingleInstanceGuard};
pub use tray::{TrayBuilder, TrayError, TrayEvent, TrayHandle, TrayMenuItem, TraySpec};
pub use win32::{
    extract_hwnd, get_cursor_pos, install_borderless_resize, set_border_color, set_corner_style,
};
pub use window::NativeWindow;
pub use window_create::{create_window, icon_from_rgba, platform_pipeline_cache_dir};
