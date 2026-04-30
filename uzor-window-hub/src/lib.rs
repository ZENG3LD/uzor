//! uzor-window-hub: unified window provider hub.
//!
//! Single abstraction layer over the platform window crates
//! (`uzor-window-desktop` / `uzor-window-web` / `uzor-window-mobile`). Apps
//! and `uzor-framework` depend on this crate only — they never depend
//! on a platform crate directly.
//!
//! Mirrors the `uzor-render-hub` pattern: features select which platform
//! is compiled in, the active provider is re-exported as `platform`, and
//! shared cross-platform helpers live in the modules below.

pub mod events;
pub mod input;
pub mod lifecycle;
pub mod metrics;

#[cfg(feature = "desktop")]
pub mod winit_provider;

#[cfg(feature = "desktop")]
pub use uzor_window_desktop as platform;

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub use uzor_window_web as platform;

#[cfg(all(feature = "mobile", not(any(feature = "desktop", feature = "web"))))]
pub use uzor_window_mobile as platform;

/// Cross-platform window event (type alias kept for back-compat).
pub use uzor::platform::PlatformWindowEvent;

// ── New public surface ────────────────────────────────────────────────────────

pub use lifecycle::{RawHandle, WindowProvider};
pub use events::PlatformEvent;
pub use input::{EventProcessor, InputState};

#[cfg(feature = "desktop")]
pub use winit_provider::WinitWindowProvider;
