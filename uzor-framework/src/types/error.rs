//! The crate's error type.

use crate::types::ids::WindowId;
use crate::types::layout_blob::LayoutCodecError;

/// Errors the framework reports to its caller (runtime paint, host start-up).
///
/// Produced by the runtime and hosts; consumed by the app's entry point.
/// Recoverable OS hiccups inside a host executor are not errors: they are
/// logged or echoed back as input events.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FrameworkError {
    /// A request named a window the framework does not know (closed or never
    /// created).
    #[error("unknown window {0:?}")]
    UnknownWindow(WindowId),
    /// A layout blob could not be encoded or restored.
    #[error(transparent)]
    LayoutCodec(#[from] LayoutCodecError),
    /// The host failed to start or lost its surface.
    #[error("host error: {0}")]
    Host(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_from() {
        assert_eq!(
            FrameworkError::UnknownWindow(WindowId(4)).to_string(),
            "unknown window WindowId(4)"
        );
        let e: FrameworkError = LayoutCodecError::Malformed("eof".into()).into();
        assert_eq!(e.to_string(), "layout blob is malformed: eof");
    }
}
