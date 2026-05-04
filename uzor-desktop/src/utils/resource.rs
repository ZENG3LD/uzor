//! Windows resource embedding helpers for `build.rs`.
//!
//! Wraps `winresource::WindowsResource` so apps can embed a `.ico` and an
//! optional application manifest into their `.exe` in one line from `build.rs`.

/// Embed a `.ico` file and an optional application manifest into the Windows
/// `.exe` at build time.
///
/// - `ico_path` — path to the `.ico` file, relative to `CARGO_MANIFEST_DIR`.
/// - `manifest_xml` — optional UTF-8 XML string to embed as the application manifest.
///
/// # Platform behaviour
///
/// On **Windows** calls `winresource` to embed the icon and manifest.
/// On **all other platforms** it is a no-op and returns `Ok(())`.
#[allow(unused_variables)]
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
