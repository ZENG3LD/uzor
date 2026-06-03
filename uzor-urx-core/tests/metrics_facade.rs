//! Smoke test the metrics facade roundtrip.

use uzor_urx_core::{install_recorder, metrics_snapshot, metrics_reset};

#[test]
fn install_recorder_idempotent() {
    let a = install_recorder() as *const _;
    let b = install_recorder() as *const _;
    assert_eq!(a, b, "install_recorder must return same Arc on second call");
}

#[test]
fn counter_round_trip() {
    install_recorder();
    metrics_reset();
    metrics::counter!("urx.test.cnt").increment(5);
    metrics::counter!("urx.test.cnt").increment(3);
    let snap = metrics_snapshot();
    assert_eq!(snap.counters.get("urx.test.cnt").copied(), Some(8));
}

#[test]
fn gauge_round_trip() {
    install_recorder();
    metrics_reset();
    metrics::gauge!("urx.test.gauge").set(42.5);
    let snap = metrics_snapshot();
    let got = snap.gauges.get("urx.test.gauge").copied().unwrap_or(0.0);
    assert!((got - 42.5).abs() < 1e-6, "gauge readback: {got}");
}

#[test]
fn histogram_round_trip() {
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
