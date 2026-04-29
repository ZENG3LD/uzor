//! Common event surface for all window providers.
//! Re-exports uzor's `PlatformEvent` as the canonical type used by
//! `WindowProvider::poll_events` and the `uzor-framework` runtime.

pub use uzor::input::PlatformEvent;
