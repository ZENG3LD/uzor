//! uzor-window-hub: unified window provider hub.
//!
//! Single abstraction layer over the platform window crates
//! (uzor-desktop / uzor-web / uzor-mobile). Apps and `uzor-framework`
//! depend on this crate only — they never depend on a platform crate
//! directly.
//!
//! Mirrors the `uzor-render-hub` pattern: features select which platform
//! is compiled in, the active provider is re-exported as `platform`, and
//! shared cross-platform helpers live in the modules below as they are
//! discovered.

pub mod events;
pub mod input;
pub mod lifecycle;
pub mod metrics;

#[cfg(feature = "desktop")]
pub use uzor_desktop as platform;

#[cfg(all(feature = "web", not(feature = "desktop")))]
pub use uzor_web as platform;

#[cfg(all(feature = "mobile", not(any(feature = "desktop", feature = "web"))))]
pub use uzor_mobile as platform;

/// Cross-platform window event (re-export from `uzor` core).
pub use uzor::platform::PlatformWindowEvent;
