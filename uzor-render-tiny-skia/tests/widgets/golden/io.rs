//! Path resolution + PNG read/write helpers for the golden module.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use tiny_skia::{IntSize, Pixmap};

use super::GoldenReport;

/// Failure modes from [`super::compare_or_bless`].
#[derive(Debug)]
pub enum GoldenError {
    /// The golden PNG does not exist yet — run once with `UZOR_BLESS=1`.
    Missing(PathBuf),
    /// The golden and actual images differ in size — not the "same
    /// picture" comparison this module makes at all.
    SizeMismatch { expected: (u32, u32), actual: (u32, u32) },
    /// Pixel diff exceeded the given `GoldenTolerance` — see the report
    /// for numbers, and `target/golden-failures/<name>.{actual,diff}.png`
    /// for images.
    Mismatch(GoldenReport),
    /// Filesystem I/O failure (read/write/create_dir_all).
    Io(std::io::Error),
    /// PNG encode/decode failure, or an internal buffer-size invariant
    /// violation (e.g. `actual.len() != width*height*4`).
    Decode(String),
}

impl std::fmt::Display for GoldenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoldenError::Missing(path) => write!(
                f,
                "golden PNG missing at {} — run once with UZOR_BLESS=1 to create it",
                path.display()
            ),
            GoldenError::SizeMismatch { expected, actual } => write!(
                f,
                "golden size {}x{} does not match actual size {}x{}",
                expected.0, expected.1, actual.0, actual.1
            ),
            GoldenError::Mismatch(report) => write!(
                f,
                "golden mismatch: {:.4}% of pixels differ (max channel diff {})",
                report.differing_fraction * 100.0,
                report.max_channel_diff
            ),
            GoldenError::Io(e) => write!(f, "golden I/O error: {e}"),
            GoldenError::Decode(msg) => write!(f, "golden PNG decode error: {msg}"),
        }
    }
}

impl std::error::Error for GoldenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GoldenError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GoldenError {
    fn from(e: std::io::Error) -> Self {
        GoldenError::Io(e)
    }
}

/// `true` when `UZOR_BLESS` is set to anything other than empty or `"0"`.
pub(super) fn bless_enabled() -> bool {
    match env::var("UZOR_BLESS") {
        Ok(v) => !v.is_empty() && v != "0",
        Err(_) => false,
    }
}

/// Resolves `tests/goldens/<name>.png` relative to this crate's own
/// manifest dir — independent of the test binary's cwd. `name` may
/// contain `/` (e.g. `"button/default"`), landing in a subdirectory.
pub(super) fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens").join(format!("{name}.png"))
}

/// The workspace's `target` directory — `CARGO_TARGET_DIR` if the
/// environment sets it, else `<workspace root>/target` (this crate's
/// manifest dir's parent, since `uzor-render-tiny-skia` is a direct
/// member of the `uzor-next` workspace).
fn target_dir() -> PathBuf {
    if let Ok(dir) = env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|workspace_root| workspace_root.join("target"))
        .unwrap_or_else(|| PathBuf::from("target"))
}

/// `target/golden-failures/<name><suffix>` (e.g. `suffix = ".actual.png"`)
/// — creates the parent directory, needed when `name` itself contains a
/// `/` (e.g. `"button/default"`).
pub(super) fn failure_path(name: &str, suffix: &str) -> Result<PathBuf, GoldenError> {
    let path = target_dir().join("golden-failures").join(format!("{name}{suffix}"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

/// Writes `data` (premultiplied RGBA8, `width`×`height`) as a PNG file at
/// `path`, creating parent directories as needed.
pub(super) fn write_png(path: &Path, data: &[u8], width: u32, height: u32) -> Result<(), GoldenError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let size = IntSize::from_wh(width, height)
        .ok_or_else(|| GoldenError::Decode(format!("invalid pixmap size {width}x{height}")))?;
    let pixmap = Pixmap::from_vec(data.to_vec(), size).ok_or_else(|| {
        GoldenError::Decode("actual buffer length does not match width*height*4".to_string())
    })?;
    pixmap.save_png(path).map_err(|e| GoldenError::Decode(e.to_string()))
}

/// Reads and decodes the golden PNG at `path` into a premultiplied RGBA8
/// [`Pixmap`].
pub(super) fn read_png(path: &Path) -> Result<Pixmap, GoldenError> {
    let bytes = fs::read(path)?;
    Pixmap::decode_png(&bytes).map_err(|e| GoldenError::Decode(e.to_string()))
}
