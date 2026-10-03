//! Native entry (brief F11). Thin: the app is [`demo::DemoApp`].
//!
//! ```text
//! cargo run -p uzor-framework --release --example demo_native --features native
//! ```

#[path = "mod.rs"]
mod demo;

use demo::DemoApp;
use uzor_framework::host::native::{run_native, NativeOptions};

fn main() -> Result<(), uzor_framework::FrameworkError> {
    run_native(
        DemoApp::new(),
        DemoApp::runtime_config(),
        NativeOptions::default(),
    )
}
