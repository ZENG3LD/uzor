//! ROLE host/* (shell): the loops that drive the runtime (design §9).
//! Hosts own the OS event loop, the paint surface and the clock; they see
//! the [`Runtime`](crate::Runtime) only (ban F10). The headless host is
//! the deterministic test host: it compiles without features, runs every
//! §9.2 script, and paints into a recording context.
//!
//! The native (winit) host lives behind feature `native` ([`native`]); the
//! web host lands in F10 behind feature `web`.

mod headless;

pub use headless::HeadlessHost;

#[cfg(feature = "native")]
pub mod native;

#[cfg(feature = "native")]
pub use native::{
    run_native, run_native_default, NativeOptions, RenderPreference, TraySpec, WinitMapper,
};

#[cfg(test)]
mod tests;
