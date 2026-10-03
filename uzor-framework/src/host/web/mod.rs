//! Web (browser) host shell — brief F10 (design §2.4, §5.4, §5.5).
//!
//! Modules: DOM listeners (including composition), one RAF driver, a
//! `WindowCommand` executor, and paint through `uzor-render-canvas2d`.
//!
//! `uzor-render-hub --target wasm32-unknown-unknown` does not build (verdict
//! recorded during F9 in `host/native/mod.rs`). This host does not try to
//! fix that; the wasm paint path is canvas2d directly.
//!
//! The driver compiles on any target the `web` feature builds. It only
//! *runs* in a browser (`wasm32-unknown-unknown`).

mod executor;
mod listeners;
mod paint;
mod raf;
mod translate;

pub use raf::{run_web, start_web};

use crate::types::bus::HostCaps;
use crate::types::error::FrameworkError;

/// What the web host reports on `WindowInput::Created`.
///
/// `multi_window: false` is the degradation flag: the kernel keeps drag-out
/// inside the page as a floating window instead of spawning a second OS window.
pub const WEB_CAPS: HostCaps = HostCaps {
    multi_window: false,
    os_resize_bezel: false,
    clipboard_async: true,
};

/// A DOM or canvas failure while installing the web host.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct WebHostError {
    message: String,
}

impl WebHostError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<WebHostError> for FrameworkError {
    fn from(err: WebHostError) -> Self {
        FrameworkError::Host(err.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_caps_are_the_single_canvas_contract() {
        assert!(!WEB_CAPS.multi_window);
        assert!(!WEB_CAPS.os_resize_bezel);
        assert!(WEB_CAPS.clipboard_async);
    }
}
