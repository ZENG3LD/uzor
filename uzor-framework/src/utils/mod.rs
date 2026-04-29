//! Utility modules: screenshot capture and single-instance guard.

pub mod screenshot;
pub mod single_instance;

pub use screenshot::{
    add_copy_src_to_target_texture, capture_screenshot, encode_png,
    is_leap_year, now_ms, screenshot_save_dir, timestamp_for_filename,
};
pub use single_instance::{single_instance, SingleInstanceGuard};
