//! Golden PNG compare/bless for `uzor-render-tiny-skia` regression tests.
//!
//! Compares a rendered frame against a committed reference PNG under
//! `tests/goldens/`, and can (re)write that reference on demand via
//! `UZOR_BLESS=1`.
//!
//! **PNG decode** — `tiny-skia = "0.11"` ships `Pixmap::decode_png` /
//! `Pixmap::load_png` / `Pixmap::save_png` behind its own `png-format`
//! feature, which is a *default* feature of the crate
//! (`tiny-skia-0.11.4/Cargo.toml`'s `[features] default = ["std", "simd",
//! "png-format"]`). This crate's `tiny-skia = "0.11"` dependency has no
//! `default-features = false`, so decode is already available — **no new
//! dependency** (`png` or otherwise) was needed for this module.
//!
//! **Colour format — premultiplied throughout.** `tiny_skia::Pixmap`
//! always stores premultiplied-alpha RGBA8 internally
//! (`Pixmap::data()`'s own doc: "byteorder: RGBA", and its struct doc: "A
//! container that owns premultiplied RGBA pixels" —
//! `tiny-skia-0.11.4/src/pixmap.rs:26,224-229`), and
//! `TinySkiaCpuRenderContext::pixels()` returns exactly that buffer
//! (`context.rs:808-810`, `self.pixmap.data()`).
//! `Pixmap::decode_png` premultiplies on load (`pixmap.rs:138-151`) and
//! `Pixmap::encode_png`/`save_png` *de*-multiplies before writing
//! (`pixmap.rs:399-407`) — so the **PNG file on disk** stores straight
//! alpha (ordinary PNG semantics), but every buffer this module compares
//! in memory (`actual` from the render context, and the golden once
//! decoded) is premultiplied. `compare_or_bless` never converts between
//! the two representations itself; it only ever compares premultiplied
//! bytes against premultiplied bytes.

mod compare;
mod io;

pub use compare::{GoldenReport, GoldenTolerance};
pub use io::GoldenError;

/// Compare `actual` (premultiplied RGBA8, `width`×`height`, e.g.
/// `TinySkiaCpuRenderContext::pixels()`) against the committed golden PNG
/// at `tests/goldens/<name>.png` (resolved via `CARGO_MANIFEST_DIR`, so it
/// works regardless of the test binary's cwd).
///
/// - `UZOR_BLESS=1` set → write/overwrite the golden, `Ok(())`
///   unconditionally (bless mode never fails the test — that is its job).
/// - golden missing, bless NOT set → `GoldenError::Missing`, forcing an
///   explicit first bless rather than silently creating one.
/// - golden present → decode, diff via the same max-channel-diff +
///   differing-fraction algorithm as `compare::diff`, and on mismatch
///   write `target/golden-failures/<name>.actual.png` +
///   `target/golden-failures/<name>.diff.png` before returning
///   `GoldenError::Mismatch`.
pub fn compare_or_bless(
    name: &str,
    actual: &[u8],
    width: u32,
    height: u32,
    tol: GoldenTolerance,
) -> Result<(), GoldenError> {
    let expected_len = width as usize * height as usize * 4;
    if actual.len() != expected_len {
        return Err(GoldenError::Decode(format!(
            "actual buffer is {} bytes, expected {width}x{height}x4 = {expected_len}",
            actual.len()
        )));
    }

    let path = io::golden_path(name);

    if io::bless_enabled() {
        io::write_png(&path, actual, width, height)?;
        return Ok(());
    }

    if !path.exists() {
        return Err(GoldenError::Missing(path));
    }

    let golden = io::read_png(&path)?;
    if golden.width() != width || golden.height() != height {
        return Err(GoldenError::SizeMismatch {
            expected: (golden.width(), golden.height()),
            actual: (width, height),
        });
    }

    let (report, within_tolerance) = compare::diff(golden.data(), actual, tol);
    if within_tolerance {
        return Ok(());
    }

    let actual_path = io::failure_path(name, ".actual.png")?;
    io::write_png(&actual_path, actual, width, height)?;

    let diff_pixels = compare::diff_image(golden.data(), actual);
    let diff_path = io::failure_path(name, ".diff.png")?;
    io::write_png(&diff_path, &diff_pixels, width, height)?;

    Err(GoldenError::Mismatch(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    /// `cargo test` runs this module's tests concurrently on separate
    /// threads within one process, but `UZOR_BLESS` is process-global
    /// state — every test that sets it, or relies on it being unset, must
    /// serialize against every other one to avoid a thread-interleaving
    /// race (one test's bless leaking into another's "not blessed" check).
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_GUARD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// RAII: sets `UZOR_BLESS=1` for the guarded critical section, always
    /// clearing it again on drop (including on an assertion panic mid-test,
    /// so one failing test can't leave bless mode on for the next).
    struct BlessGuard<'a> {
        _lock: std::sync::MutexGuard<'a, ()>,
    }

    impl<'a> BlessGuard<'a> {
        fn enable(lock: std::sync::MutexGuard<'a, ()>) -> Self {
            std::env::set_var("UZOR_BLESS", "1");
            Self { _lock: lock }
        }
    }

    impl Drop for BlessGuard<'_> {
        fn drop(&mut self) {
            std::env::remove_var("UZOR_BLESS");
        }
    }

    /// Unique name per test call, in its OWN directory component (not a
    /// directory shared across tests) — `cargo test` runs this module's
    /// tests concurrently, and a shared parent directory would race
    /// between one test's `create_dir_all` + write and a sibling's
    /// `remove_dir` cleanup once its own file is gone. A per-call unique
    /// directory means `remove_dir` in [`cleanup_golden`] only ever
    /// touches a directory this exact call owns. No `tempfile` dependency
    /// needed — a unique subpath is enough.
    fn unique_name(label: &str) -> String {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("__compare_or_bless_unit_test__-{label}-{n}/golden")
    }

    fn solid_rgba(color: [u8; 4], width: u32, height: u32) -> Vec<u8> {
        color.repeat((width * height) as usize)
    }

    /// Best-effort cleanup: remove the golden PNG (and its shared parent
    /// test-scratch directory once every sibling test has also cleaned up).
    fn cleanup_golden(name: &str) {
        let path = io::golden_path(name);
        let _ = std::fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }

    #[test]
    fn missing_golden_without_bless_reports_missing() {
        let _lock = lock_env();
        std::env::remove_var("UZOR_BLESS");

        let name = unique_name("missing");
        let actual = solid_rgba([10, 20, 30, 255], 2, 2);
        let err = compare_or_bless(&name, &actual, 2, 2, GoldenTolerance::default()).unwrap_err();
        assert!(matches!(err, GoldenError::Missing(_)));
    }

    #[test]
    fn bless_then_compare_round_trips_to_ok() {
        let name = unique_name("round-trip");
        let actual = solid_rgba([200, 100, 50, 255], 4, 3);

        {
            let _bless = BlessGuard::enable(lock_env());
            compare_or_bless(&name, &actual, 4, 3, GoldenTolerance::default())
                .expect("bless must always succeed");
        }

        compare_or_bless(&name, &actual, 4, 3, GoldenTolerance::default())
            .expect("comparing the just-blessed golden against the same buffer must pass");

        cleanup_golden(&name);
    }

    #[test]
    fn mismatch_writes_failure_pngs_and_reports_mismatch() {
        let name = unique_name("mismatch");
        let golden_pixels = solid_rgba([0, 0, 0, 255], 3, 3);

        {
            let _bless = BlessGuard::enable(lock_env());
            compare_or_bless(&name, &golden_pixels, 3, 3, GoldenTolerance::default())
                .expect("bless must always succeed");
        }

        let actual_pixels = solid_rgba([255, 255, 255, 255], 3, 3);
        let err = compare_or_bless(&name, &actual_pixels, 3, 3, GoldenTolerance::default())
            .expect_err("a fully different image must not match the tight default tolerance");
        assert!(matches!(err, GoldenError::Mismatch(_)));

        let actual_path = io::failure_path(&name, ".actual.png").expect("failure dir must be creatable");
        let diff_path = io::failure_path(&name, ".diff.png").expect("failure dir must be creatable");
        assert!(actual_path.exists(), "actual.png must be written on mismatch");
        assert!(diff_path.exists(), "diff.png must be written on mismatch");

        cleanup_golden(&name);
        let _ = std::fs::remove_file(&actual_path);
        let _ = std::fs::remove_file(&diff_path);
        if let Some(parent) = actual_path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }

    #[test]
    fn size_mismatch_is_reported_before_pixel_diffing() {
        let name = unique_name("size-mismatch");
        let golden_pixels = solid_rgba([0, 0, 0, 255], 2, 2);

        {
            let _bless = BlessGuard::enable(lock_env());
            compare_or_bless(&name, &golden_pixels, 2, 2, GoldenTolerance::default())
                .expect("bless must always succeed");
        }

        let differently_sized = solid_rgba([0, 0, 0, 255], 4, 4);
        let err = compare_or_bless(&name, &differently_sized, 4, 4, GoldenTolerance::default())
            .expect_err("a size mismatch must not fall through to pixel diffing");
        assert!(matches!(err, GoldenError::SizeMismatch { .. }));

        cleanup_golden(&name);
    }
}
