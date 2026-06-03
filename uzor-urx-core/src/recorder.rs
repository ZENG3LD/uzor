//! `metrics::Recorder` snapshot impl + install gate.
//!
//! Lifted from tessera-kernel's recorder pattern (which itself was
//! written against the `metrics` 0.24 facade). Pattern lessons baked in:
//!
//! - `OnceLock::get_or_init` for race-safe global install
//! - Atomic counter / atomic-bits gauge / ring-buffer histogram
//! - Same recorder reused across many engine instances
//!
//! Consumers call `install_recorder()` ONCE on process boot; URX
//! backends call `metrics::counter!()` / `gauge!()` / `histogram!()`
//! through the facade and the recorder collects.
//!
//! `metrics_snapshot()` returns the current state for HTTP dashboards
//! (similar to `perfwatch` in tessera-playground).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use metrics::{Counter, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit};

// ── Histogram ring buffer ────────────────────────────────────────────────────

const HIST_CAP: usize = 1024;

struct HistRing {
    buf:  [f64; HIST_CAP],
    head: AtomicUsize,
    len:  AtomicUsize,
    sum:  AtomicU64,   // float bits — added via fetch_add on raw bits is wrong;
                       // we just write the latest sum as bits and accept races.
    min:  AtomicU64,   // f64 bits
    max:  AtomicU64,   // f64 bits
    count: AtomicU64,
}

impl HistRing {
    fn new() -> Self {
        Self {
            buf: [0.0; HIST_CAP],
            head: AtomicUsize::new(0),
            len:  AtomicUsize::new(0),
            sum:  AtomicU64::new(0u64),
            min:  AtomicU64::new(f64::INFINITY.to_bits()),
            max:  AtomicU64::new(f64::NEG_INFINITY.to_bits()),
            count: AtomicU64::new(0),
        }
    }

    fn record(&mut self, v: f64) {
        let i = self.head.load(Ordering::Relaxed);
        self.buf[i] = v;
        self.head.store((i + 1) % HIST_CAP, Ordering::Relaxed);
        let prev_len = self.len.load(Ordering::Relaxed);
        if prev_len < HIST_CAP {
            self.len.store(prev_len + 1, Ordering::Relaxed);
        }
        // sum approx: read-modify-write on float bits is racy but
        // single-writer per HistRing (we own &mut here) so safe.
        let new_sum = f64::from_bits(self.sum.load(Ordering::Relaxed)) + v;
        self.sum.store(new_sum.to_bits(), Ordering::Relaxed);
        let cur_min = f64::from_bits(self.min.load(Ordering::Relaxed));
        if v < cur_min { self.min.store(v.to_bits(), Ordering::Relaxed); }
        let cur_max = f64::from_bits(self.max.load(Ordering::Relaxed));
        if v > cur_max { self.max.store(v.to_bits(), Ordering::Relaxed); }
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> HistSnapshot {
        let len = self.len.load(Ordering::Relaxed);
        let count = self.count.load(Ordering::Relaxed);
        if len == 0 {
            return HistSnapshot::default();
        }
        let mut sorted: Vec<f64> = self.buf.iter().take(len).copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p = |q: f64| -> f64 {
            if sorted.is_empty() { return 0.0; }
            let idx = ((sorted.len() as f64 - 1.0) * q).round() as usize;
            sorted[idx.min(sorted.len() - 1)]
        };
        let sum = f64::from_bits(self.sum.load(Ordering::Relaxed));
        HistSnapshot {
            count,
            ring_len: len,
            sum,
            mean: if count == 0 { 0.0 } else { sum / count as f64 },
            min: f64::from_bits(self.min.load(Ordering::Relaxed)),
            max: f64::from_bits(self.max.load(Ordering::Relaxed)),
            p50: p(0.50),
            p90: p(0.90),
            p99: p(0.99),
        }
    }

    fn reset(&mut self) {
        self.buf = [0.0; HIST_CAP];
        self.head.store(0, Ordering::Relaxed);
        self.len.store(0, Ordering::Relaxed);
        self.sum.store(0u64, Ordering::Relaxed);
        self.min.store(f64::INFINITY.to_bits(), Ordering::Relaxed);
        self.max.store(f64::NEG_INFINITY.to_bits(), Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct HistSnapshot {
    pub count:    u64,
    pub ring_len: usize,
    pub sum:      f64,
    pub mean:     f64,
    pub min:      f64,
    pub max:      f64,
    pub p50:      f64,
    pub p90:      f64,
    pub p99:      f64,
}

// ── Recorder ─────────────────────────────────────────────────────────────────

#[derive(Default)]
struct State {
    counters:   HashMap<String, u64>,
    gauges:     HashMap<String, u64>, // f64 bits
    histograms: HashMap<String, HistRing>,
}

#[derive(Clone)]
pub struct UrxRecorder {
    state: std::sync::Arc<Mutex<State>>,
}

impl UrxRecorder {
    pub fn new() -> Self {
        Self { state: std::sync::Arc::new(Mutex::new(State::default())) }
    }
}

impl Default for UrxRecorder {
    fn default() -> Self { Self::new() }
}

// Hand the recorder the value the consumer recorded — counters/gauges/
// histograms each go to their own map. We don't keep per-key metadata
// (units, descriptions) — the catalog is statically documented in
// metrics_keys.rs.

struct CounterHandle { state: std::sync::Arc<Mutex<State>>, key: String }
struct GaugeHandle   { state: std::sync::Arc<Mutex<State>>, key: String }
struct HistogramHandle { state: std::sync::Arc<Mutex<State>>, key: String }

impl metrics::CounterFn for CounterHandle {
    fn increment(&self, value: u64) {
        if let Ok(mut s) = self.state.lock() {
            *s.counters.entry(self.key.clone()).or_insert(0) += value;
        }
    }
    fn absolute(&self, value: u64) {
        if let Ok(mut s) = self.state.lock() {
            *s.counters.entry(self.key.clone()).or_insert(0) = value;
        }
    }
}

impl metrics::GaugeFn for GaugeHandle {
    fn increment(&self, value: f64) {
        if let Ok(mut s) = self.state.lock() {
            let entry = s.gauges.entry(self.key.clone()).or_insert(0u64);
            *entry = (f64::from_bits(*entry) + value).to_bits();
        }
    }
    fn decrement(&self, value: f64) {
        if let Ok(mut s) = self.state.lock() {
            let entry = s.gauges.entry(self.key.clone()).or_insert(0u64);
            *entry = (f64::from_bits(*entry) - value).to_bits();
        }
    }
    fn set(&self, value: f64) {
        if let Ok(mut s) = self.state.lock() {
            s.gauges.insert(self.key.clone(), value.to_bits());
        }
    }
}

impl metrics::HistogramFn for HistogramHandle {
    fn record(&self, value: f64) {
        if let Ok(mut s) = self.state.lock() {
            s.histograms.entry(self.key.clone()).or_insert_with(HistRing::new).record(value);
        }
    }
}

impl Recorder for UrxRecorder {
    fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
    fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}
    fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
        Counter::from_arc(std::sync::Arc::new(CounterHandle {
            state: self.state.clone(),
            key:   key.name().to_string(),
        }))
    }
    fn register_gauge(&self, key: &Key, _: &Metadata<'_>) -> Gauge {
        Gauge::from_arc(std::sync::Arc::new(GaugeHandle {
            state: self.state.clone(),
            key:   key.name().to_string(),
        }))
    }
    fn register_histogram(&self, key: &Key, _: &Metadata<'_>) -> Histogram {
        Histogram::from_arc(std::sync::Arc::new(HistogramHandle {
            state: self.state.clone(),
            key:   key.name().to_string(),
        }))
    }
}

// ── Install ──────────────────────────────────────────────────────────────────

static GLOBAL: OnceLock<UrxRecorder> = OnceLock::new();

/// Install URX recorder as the process-global `metrics::Recorder`.
/// Idempotent — many engine instances can call this; only the first
/// install wins, subsequent callers get the same Arc.
///
/// Race-safe via OnceLock::get_or_init (lessons learned from tessera's
/// e645b91 fix).
pub fn install_recorder() -> &'static UrxRecorder {
    GLOBAL.get_or_init(|| {
        let rec = UrxRecorder::new();
        // Set as global. If another crate already set one, we lose
        // and our recorder won't see metrics — but at least we
        // returned a valid Arc to the caller.
        let _ = metrics::set_global_recorder(rec.clone());
        rec
    })
}

// ── Snapshot for HTTP dashboards ────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct MetricsSnapshot {
    pub counters:   HashMap<String, u64>,
    pub gauges:     HashMap<String, f64>,
    pub histograms: HashMap<String, HistSnapshot>,
}

pub fn metrics_snapshot() -> MetricsSnapshot {
    let Some(rec) = GLOBAL.get() else { return MetricsSnapshot::default() };
    let Ok(s) = rec.state.lock() else { return MetricsSnapshot::default() };
    MetricsSnapshot {
        counters: s.counters.clone(),
        gauges:   s.gauges.iter().map(|(k, v)| (k.clone(), f64::from_bits(*v))).collect(),
        histograms: s.histograms.iter().map(|(k, h)| (k.clone(), h.snapshot())).collect(),
    }
}

/// Reset all counters/gauges/histograms. Useful for benchmark A/B —
/// reset between A and B samples so accumulated lifetime stats don't
/// pollute the comparison.
pub fn metrics_reset() {
    let Some(rec) = GLOBAL.get() else { return };
    let Ok(mut s) = rec.state.lock() else { return };
    s.counters.clear();
    s.gauges.clear();
    for h in s.histograms.values_mut() { h.reset(); }
}
