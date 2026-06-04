//! # uzor-urx-region-mixer
//!
//! Default `MixDispatcher` implementation that closes the Wave 9b
//! gap from `uzor-urx-engine`: per-region `BackendHint` routing
//! through a single dispatcher built from consumer-supplied
//! per-backend callbacks.
//!
//! ## Motivation
//!
//! `uzor-urx-engine` already groups regions by `BackendHint` and
//! emits a callback per region when rendered with
//! `RenderTarget::Mixed`, but it doesn't ship a dispatcher because
//! the actual rendering backend (CPU pixmap, Full-GPU, Hybrid) is a
//! consumer choice. Without a default implementation every consumer
//! had to write the same dispatch switch.
//!
//! `RegionMixer` is that default: you give it one callback per
//! backend (Cpu / FullGpu / Hybrid / FullGpuInstanced), it routes
//! each region to the right one based on `BackendHint`. Hints set to
//! `Inherit` are routed to a fallback callback (or panic if none).
//!
//! ```text
//! use uzor_urx_engine::{BackendHint, RenderTarget, UrxEngine};
//! use uzor_urx_region_mixer::RegionMixer;
//!
//! let mut mixer = RegionMixer::new()
//!     .on_cpu(|region, bounds, scene| { /* draw with CpuBackend */ })
//!     .on_full_gpu(|region, bounds, scene| { /* draw with WgpuFull */ })
//!     .fallback(|region, bounds, scene| { /* default path */ });
//!
//! engine.render(RenderTarget::Mixed { dispatcher: &mut mixer }).unwrap();
//! ```
//!
//! The mixer also records what it dispatched for the frame (region id,
//! bounds, hint, sequence index) so consumers can inspect ordering,
//! tally per-backend region counts, or run instrumentation.

use uzor_urx_core::math::Rect;
use uzor_urx_core::region::RegionId;
use uzor_urx_core::scene::Scene;
use uzor_urx_engine::{BackendHint, MixDispatcher};

/// One dispatched region in the order the engine sent it.
#[derive(Debug, Clone, Copy)]
pub struct DispatchRecord {
    pub region: RegionId,
    pub bounds: Rect,
    pub hint: BackendHint,
    pub sequence: u32,
}

type Cb = Box<dyn FnMut(RegionId, Rect, &Scene)>;

/// Default per-region dispatcher. Construct empty, attach callbacks,
/// pass to `UrxEngine::render(RenderTarget::Mixed { dispatcher: &mut … })`.
pub struct RegionMixer {
    on_cpu:               Option<Cb>,
    on_full_gpu:          Option<Cb>,
    on_hybrid:            Option<Cb>,
    fallback:             Option<Cb>,
    records:              Vec<DispatchRecord>,
    seq:                  u32,
    /// When true, an unrouted hint (no callback + no fallback) panics
    /// in `dispatch`. When false it's silently skipped — useful for
    /// debugging which regions still need a backend wired up.
    pub strict: bool,
}

impl Default for RegionMixer {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionMixer {
    pub fn new() -> Self {
        Self {
            on_cpu: None,
            on_full_gpu: None,
            on_hybrid: None,
            fallback: None,
            records: Vec::new(),
            seq: 0,
            strict: true,
        }
    }

    pub fn on_cpu<F: FnMut(RegionId, Rect, &Scene) + 'static>(mut self, f: F) -> Self {
        self.on_cpu = Some(Box::new(f)); self
    }
    pub fn on_full_gpu<F: FnMut(RegionId, Rect, &Scene) + 'static>(mut self, f: F) -> Self {
        self.on_full_gpu = Some(Box::new(f)); self
    }
    pub fn on_hybrid<F: FnMut(RegionId, Rect, &Scene) + 'static>(mut self, f: F) -> Self {
        self.on_hybrid = Some(Box::new(f)); self
    }
    pub fn fallback<F: FnMut(RegionId, Rect, &Scene) + 'static>(mut self, f: F) -> Self {
        self.fallback = Some(Box::new(f)); self
    }
    pub fn lenient(mut self) -> Self { self.strict = false; self }

    /// All regions dispatched in the LAST `render()` call (cleared at
    /// the start of each frame by `begin_frame`).
    pub fn records(&self) -> &[DispatchRecord] { &self.records }

    /// Reset frame-local bookkeeping. Call once before `render()`.
    pub fn begin_frame(&mut self) {
        self.records.clear();
        self.seq = 0;
    }

    /// Count dispatched regions per hint kind across the last frame.
    pub fn counts_by_hint(&self) -> [(BackendHint, u32); 4] {
        let mut counts = [
            (BackendHint::Inherit, 0u32),
            (BackendHint::Cpu,     0u32),
            (BackendHint::FullGpu, 0u32),
            (BackendHint::Hybrid,  0u32),
        ];
        for r in &self.records {
            for slot in counts.iter_mut() {
                if slot.0 == r.hint { slot.1 += 1; }
            }
        }
        counts
    }
}

impl MixDispatcher for RegionMixer {
    fn dispatch(&mut self, id: RegionId, bounds: Rect, hint: BackendHint, scene: &Scene) {
        self.records.push(DispatchRecord {
            region: id, bounds, hint, sequence: self.seq,
        });
        self.seq += 1;

        // Strictly resolve the right callback in priority order.
        // (1) try the hint's own callback; (2) try fallback; (3) panic
        // or skip per `strict`.
        let cb = match hint {
            BackendHint::Inherit => self.fallback.as_mut(),
            BackendHint::Cpu     => self.on_cpu.as_mut().or(self.fallback.as_mut()),
            BackendHint::FullGpu => self.on_full_gpu.as_mut().or(self.fallback.as_mut()),
            BackendHint::Hybrid  => self.on_hybrid.as_mut().or(self.fallback.as_mut()),
        };
        if let Some(cb) = cb {
            cb(id, bounds, scene);
        } else if self.strict {
            panic!("RegionMixer: no callback for hint {:?} on region {:?} (set one with .on_*() or .fallback(), or call .lenient() to silently skip)", hint, id);
        }
        // else lenient — silently drop.
    }
}
