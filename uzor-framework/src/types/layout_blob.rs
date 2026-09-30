//! Serialized dock layout, opaque to the app.
//!
//! The app asks for a blob (`LayoutCmd::RequestBlob`), receives it in
//! `DockIntent::LayoutBlob`, stores the bytes wherever it likes and hands them
//! back in `LayoutCmd::Restore`. The framework never writes a file. The
//! encoder / decoder (a versioned postcard envelope around the lib
//! `LayoutSnapshot` structure plus floating windows) belongs to the
//! LayoutEngine (`engine::layout`); here the blob is only bytes.

use crate::types::spec::PanelHome;
use crate::types::window::SizePx;
use serde::{Deserialize, Serialize};

/// Opaque serialized layout of one window.
///
/// Produced by the LayoutEngine on request; consumed by the app (storage) and
/// by the LayoutEngine again on restore.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LayoutBlob(Vec<u8>);

impl LayoutBlob {
    /// Wrap bytes previously obtained from [`LayoutBlob::into_bytes`] /
    /// [`LayoutBlob::as_bytes`] (e.g. loaded by the app from its storage).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Borrow the bytes for storage.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Take the bytes for storage.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Byte length.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` when the blob holds no bytes.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<Vec<u8>> for LayoutBlob {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl From<LayoutBlob> for Vec<u8> {
    fn from(blob: LayoutBlob) -> Self {
        blob.0
    }
}

/// Window geometry saved alongside the dock structure in a layout blob.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometrySnapshot {
    /// Outer size, physical pixels.
    pub outer_size: SizePx,
    /// Outer position, physical screen pixels, if known.
    pub position: Option<(i32, i32)>,
    /// Window was maximized.
    pub maximized: bool,
}

/// Why a layout blob could not be produced or restored.
///
/// Carried by `DockIntent::LayoutRestoreFailed` / `LayoutBlobFailed`; on a
/// restore failure the window keeps the layout it had, never a half-restored
/// tree.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LayoutCodecError {
    /// The blob was written by an unknown envelope version.
    #[error("layout blob version {found} is not supported (expected {supported})")]
    Version {
        /// Version found in the blob.
        found: u16,
        /// Version this build reads.
        supported: u16,
    },
    /// The bytes are not a valid layout envelope.
    #[error("layout blob is malformed: {0}")]
    Malformed(String),
    /// The app's panel factory returned no panel for a stored tab.
    #[error("no panel for {home:?} with type id {type_id:?}")]
    Panel {
        /// Where the panel was going.
        home: PanelHome,
        /// The stored panel type id.
        type_id: String,
    },
    /// The layout could not be encoded.
    #[error("layout could not be encoded: {0}")]
    Encode(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_bytes_round_trip() {
        let bytes = vec![1u8, 2, 3, 255];
        let blob = LayoutBlob::from_bytes(bytes.clone());
        assert_eq!(blob.as_bytes(), &bytes[..]);
        assert_eq!(blob.len(), 4);
        assert!(!blob.is_empty());
        let back: Vec<u8> = blob.clone().into();
        assert_eq!(back, bytes);
        assert_eq!(LayoutBlob::from(back), blob);
        assert!(LayoutBlob::default().is_empty());
    }

    #[test]
    fn codec_error_messages() {
        let e = LayoutCodecError::Version {
            found: 9,
            supported: 1,
        };
        assert_eq!(
            e.to_string(),
            "layout blob version 9 is not supported (expected 1)"
        );
        let e = LayoutCodecError::Panel {
            home: PanelHome::Leaf(uzor::layout::docking::LeafId(3)),
            type_id: "chart".into(),
        };
        assert_eq!(
            e.to_string(),
            "no panel for Leaf(LeafId(3)) with type id \"chart\""
        );
    }
}
