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

/// Cross-platform window event (type alias kept for back-compat).
pub use uzor::platform::PlatformWindowEvent;

// ── Public surface ────────────────────────────────────────────────────────────

pub use lifecycle::{RawHandle, ResizeDirection, RgbaIcon, SoftwarePresenter, WindowProvider};
pub use events::PlatformEvent;
pub use input::{EventProcessor, InputState};
