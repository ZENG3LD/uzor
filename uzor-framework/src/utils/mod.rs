//! Utility modules: screenshot capture, single-instance guard, and Windows
//! resource embedding helpers.

#[cfg(not(target_arch = "wasm32"))]
pub mod resource;
#[cfg(not(target_arch = "wasm32"))]
pub mod screenshot;
#[cfg(not(target_arch = "wasm32"))]
pub mod single_instance;

#[cfg(not(target_arch = "wasm32"))]
pub use screenshot::{
    add_copy_src_to_target_texture, capture_screenshot, encode_png,
    is_leap_year, now_ms, screenshot_save_dir, timestamp_for_filename,
};
#[cfg(not(target_arch = "wasm32"))]
pub use single_instance::{single_instance, SingleInstanceGuard};
#[cfg(not(target_arch = "wasm32"))]
pub use resource::embed_icon_and_manifest;
