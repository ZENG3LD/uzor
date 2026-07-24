//! Smoke test the metrics facade roundtrip.
//!
//! `install_recorder`/`metrics_reset`/`metrics_snapshot` all operate on
//! ONE process-wide global recorder (that's the whole point of the
//! facade) — `cargo test`'s default parallel runner races these 4
//! tests against each other: `metrics_reset()` is a global nuke, not
//! scoped to the calling test's own key, so e.g. `gauge_round_trip`'s
//! reset can wipe `counter_round_trip`'s own in-flight increments
//! between its two `counter!(...).increment(...)` calls, flaking the
//! `Some(8)` assertion down to `Some(3)`/`Some(5)`/`None` depending on
//! interleaving. Fixed with a file-local `std::sync::Mutex<()>` guard
//! every test acquires before touching the shared recorder — the
//! "serial_test-style mutex guard within the file" shape (no new dev
//! deps; the workspace has no existing test-serialization crate/helper
//! to reuse instead, confirmed by search).

use std::sync::Mutex;

use uzor_urx_core::{install_recorder, metrics_snapshot, metrics_reset};

/// Guards every test below — see this module's own doc comment for why
/// a real mutex (not just running tests single-threaded via `--test-
/// threads=1`) is the right fix: it's scoped to only the tests that
/// actually share the global recorder, not a blunt whole-binary
/// serialization flag a future unrelated test in this same file would
/// silently inherit.
static METRICS_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the shared guard, recovering from a poisoned lock the same
/// way a prior test's own panic would leave it — a panicking test
/// already failed the suite; a poisoned mutex must not ALSO fail every
/// later test in this file with an unrelated "lock poisoned" message.
fn lock_guard() -> std::sync::MutexGuard<'static, ()> {
    METRICS_TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn install_recorder_idempotent() {
    let _guard = lock_guard();
    let a = install_recorder() as *const _;
    let b = install_recorder() as *const _;
    assert_eq!(a, b, "install_recorder must return same Arc on second call");
}

#[test]
fn counter_round_trip() {
    let _guard = lock_guard();
    install_recorder();
    metrics_reset();
    metrics::counter!("urx.test.cnt").increment(5);
    metrics::counter!("urx.test.cnt").increment(3);
    let snap = metrics_snapshot();
    assert_eq!(snap.counters.get("urx.test.cnt").copied(), Some(8));
}

#[test]
fn gauge_round_trip() {
    let _guard = lock_guard();
    install_recorder();
    metrics_reset();
    metrics::gauge!("urx.test.gauge").set(42.5);
    let snap = metrics_snapshot();
    let got = snap.gauges.get("urx.test.gauge").copied().unwrap_or(0.0);
    assert!((got - 42.5).abs() < 1e-6, "gauge readback: {got}");
}

#[test]
fn histogram_round_trip() {
    let _guard = lock_guard();
    install_recorder();
    metrics_reset();
    for v in [100.0_f64, 200.0, 300.0, 400.0, 500.0] {
        metrics::histogram!("urx.test.hist").record(v);
    }
    let snap = metrics_snapshot();
    let h = snap.histograms.get("urx.test.hist").expect("hist exists");
    assert_eq!(h.count, 5);
    assert_eq!(h.min, 100.0);
    assert_eq!(h.max, 500.0);
    assert!((h.mean - 300.0).abs() < 1e-6, "mean = {}", h.mean);
}
