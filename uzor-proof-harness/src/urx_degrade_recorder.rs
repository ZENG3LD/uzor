//! Scoped `metrics::Recorder` that captures every URX native-pipeline
//! degrade counter fired while it's installed via
//! `metrics::with_local_recorder` — a promoted copy of
//! `uzor-urx-wgpu/src/encode.rs`'s own private
//! `metrics_recorder_proof::TestRecorder` (that one is `#[cfg(test)]`-
//! private to that crate, so this is a copy, not a new design),
//! generalized from "does THIS one label's counter equal N" to "list
//! every label that fired at all" — the shape [`crate::render::
//! render_urx_wgpu`]'s divergence reporting needs.
//!
//! Scoped (thread-local, via `metrics::with_local_recorder`) rather than
//! installed as the process-global recorder
//! (`uzor_urx_core::recorder::install_recorder`) so this crate never
//! interferes with, or is interfered by, any global recorder a real app
//! embedding `uzor-urx-wgpu` may already have installed.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use metrics::{Counter, CounterFn, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit};

struct RecordedCounter(AtomicU64);

impl CounterFn for RecordedCounter {
    fn increment(&self, value: u64) {
        self.0.fetch_add(value, Ordering::SeqCst);
    }
    fn absolute(&self, value: u64) {
        self.0.store(value, Ordering::SeqCst);
    }
}

/// Install via `metrics::with_local_recorder(&recorder, || { .. })`
/// around one `NativeUrxRenderer::render_into_encoder` call, then read
/// [`Self::fired`] afterward.
#[derive(Default)]
pub(crate) struct DegradeRecorder {
    counters: Mutex<HashMap<Key, Arc<RecordedCounter>>>,
}

impl DegradeRecorder {
    /// Every degrade counter that actually incremented while this
    /// recorder was installed, as `(kind, count)` — reads the `"kind"`
    /// label `uzor-urx-wgpu/src/encode.rs::degrade` attaches to every
    /// `metrics::counter!(KEY_RENDER_PRIMITIVES, "kind" => kind)` call,
    /// the SAME convention that crate's own test recorder's `value_for`
    /// reads. Filters to `KEY_RENDER_PRIMITIVES` specifically — this
    /// recorder is scoped to one render call, so no other metric key is
    /// expected to register here, but filtering explicitly documents the
    /// contract rather than assuming it.
    pub(crate) fn fired(&self) -> Vec<(String, u64)> {
        let map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .filter(|(key, _)| key.name() == uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES)
            .filter_map(|(key, counter)| {
                let count = counter.0.load(Ordering::SeqCst);
                if count == 0 {
                    return None;
                }
                key.labels().find(|l| l.key() == "kind").map(|l| (l.value().to_string(), count))
            })
            .collect()
    }
}

impl Recorder for DegradeRecorder {
    fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
    fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
    fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
        let mut map = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        let handle = map.entry(key.clone()).or_insert_with(|| Arc::new(RecordedCounter(AtomicU64::new(0))));
        Counter::from_arc(handle.clone())
    }
    fn register_gauge(&self, _: &Key, _: &Metadata<'_>) -> Gauge {
        Gauge::noop()
    }
    fn register_histogram(&self, _: &Key, _: &Metadata<'_>) -> Histogram {
        Histogram::noop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fired_reports_only_non_zero_counters_under_their_kind_label() {
        let recorder = DegradeRecorder::default();
        metrics::with_local_recorder(&recorder, || {
            metrics::counter!(uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES, "kind" => "native_line_square_cap_to_round").increment(1);
            metrics::counter!(uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES, "kind" => "native_line_square_cap_to_round").increment(2);
        });
        let fired = recorder.fired();
        assert_eq!(fired, vec![("native_line_square_cap_to_round".to_string(), 3)]);
    }

    #[test]
    fn fired_is_empty_when_nothing_was_recorded() {
        let recorder = DegradeRecorder::default();
        metrics::with_local_recorder(&recorder, || {
            // No degrade fired — an ordinary defect-free scene.
        });
        assert!(recorder.fired().is_empty());
    }
}
