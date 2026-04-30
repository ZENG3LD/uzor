//! Windows resource embedding helpers for `build.rs`.
//!
//! Wraps `winresource::WindowsResource` so apps can embed a `.ico` and an
//! optional application manifest into their `.exe` in one line from `build.rs`.
//!
//! # Usage
//!
//! In `build.rs`:
//!
//! ```no_run
//! fn main() {
//!     uzor_framework::utils::resource::embed_icon_and_manifest(
//!         "assets/icon.ico",
//!         Some(include_str!("assets/manifest.xml")),
//!     ).expect("failed to embed Windows resources");
//! }
//! ```
//!
//! On non-Windows platforms the function is a no-op and always returns `Ok(())`.

/// Embed a `.ico` file and an optional application manifest into the Windows
/// `.exe` at build time.
///
/// - `ico_path` — path to the `.ico` file, relative to `CARGO_MANIFEST_DIR`.
/// - `manifest_xml` — optional UTF-8 XML string to embed as the application
///   manifest.  Typically used to set the DPI awareness level or request
///   administrator privileges.
///
/// # Errors
///
/// Returns `Err` if `winresource::WindowsResource::compile()` fails (e.g. the
/// `.ico` file does not exist, or `windres.exe` is not on `PATH`).
///
/// # Platform behaviour
///
/// On **Windows** the function calls `winresource` to embed the icon and
/// manifest.  On **all other platforms** it is a no-op and returns `Ok(())`.
#[allow(unused_variables)] // `ico_path` and `manifest_xml` unused on non-Windows
pub fn embed_icon_and_manifest(
    ico_path: &str,
    manifest_xml: Option<&str>,
) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico_path);
        if let Some(m) = manifest_xml {
            res.set_manifest(m);
        }
        res.compile()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    }

    Ok(())
}
