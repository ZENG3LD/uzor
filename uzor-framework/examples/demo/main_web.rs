//! Web entry (brief F11): one `DemoApp`, `start_web` on the page canvas.
//!
//! `cdylib` for `wasm32-unknown-unknown`. The page provides
//! `<canvas id="uzor-demo">`.
//!
//! ```text
//! cargo check -p uzor-framework --release --example demo_web \
//!     --target wasm32-unknown-unknown --features web
//! ```

#[path = "mod.rs"]
mod demo;

use demo::{DemoApp, CANVAS_ID};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Browser start. Calls [`uzor_framework::host::start_web`].
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn start_web() {
    let _ = uzor_framework::host::start_web(CANVAS_ID, DemoApp::new(), DemoApp::runtime_config());
}
