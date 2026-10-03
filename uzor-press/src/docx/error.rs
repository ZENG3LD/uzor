//! `DocxError` — everything that can go wrong building a `.docx`, matching
//! [`crate::press::PressError`]'s own shape. No `unwrap`/`expect` anywhere
//! in the `docx` module reaches this type via anything but an explicit
//! `Result` propagation.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum DocxError {
    /// `--format deck` was asked to emit DOCX — deck stays PDF-only.
    UnsupportedFormat(crate::press::Format),
    /// Reading/decoding an embedded image failed — wraps the same
    /// [`crate::press::PressError`] the PDF path already reports for the
    /// identical failure mode (missing file, bad PNG, ...).
    Image { path: PathBuf, source: crate::press::PressError },
    /// Rasterizing a `:::diagram` figure to PNG failed — a distinct case
    /// from `Image` because the source is `uzor_export::ExportError`, not
    /// a [`crate::press::PressError`] (no file/path is involved).
    Raster(String),
    Zip(zip::result::ZipError),
    Io(std::io::Error),
}

impl fmt::Display for DocxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocxError::UnsupportedFormat(format) => {
                write!(f, "--format {format:?} does not support --emit docx (deck stays PDF-only)")
            }
            DocxError::Image { path, source } => write!(f, "failed to embed image {}: {source}", path.display()),
            DocxError::Raster(message) => write!(f, "failed to rasterize a diagram figure to PNG: {message}"),
            DocxError::Zip(e) => write!(f, "DOCX zip packaging error: {e}"),
            DocxError::Io(e) => write!(f, "DOCX I/O error: {e}"),
        }
    }
}

impl std::error::Error for DocxError {}

impl From<zip::result::ZipError> for DocxError {
    fn from(e: zip::result::ZipError) -> Self {
        DocxError::Zip(e)
    }
}

impl From<std::io::Error> for DocxError {
    fn from(e: std::io::Error) -> Self {
        DocxError::Io(e)
    }
}
