//! L4 dashboard binary (brief E1).
//!
//! ```sh
//! cargo run -p uzor-examples --release --features native --bin l4-dashboard
//! ```

use uzor_examples::DashboardApp;
use uzor_framework::host::native::{run_native, NativeOptions};

fn main() -> Result<(), uzor_framework::FrameworkError> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    run_native(
        DashboardApp::new(),
        DashboardApp::runtime_config(),
        NativeOptions::default(),
    )
}
