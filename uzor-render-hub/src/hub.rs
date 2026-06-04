//! [`RenderHub`] — unified rendering backend manager.
//!
//! `RenderHub` replaces the old loose combination of factory + backend enum.
//! It:
//!
//! - **Probes** available GPU adapters at construction time.
//! - **Maintains** a [`BackendPool`] of what is hardware-available.
//! - **Tracks** the currently active backend (switchable at runtime).
//! - **Owns** [`PerfSettings`] (fps limit, MSAA, vsync).
//! - **Accumulates** [`crate::RenderMetrics`] across frames.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Full autodetect — picks best backend, enables live switching.
//! let hub = RenderHub::autodetect();
//!
//! // Fixed backend — no adapter probe, no live switching.
//! let hub = RenderHub::fixed(RenderBackend::VelloGpu);
//!
//! // Then pass to AppBuilder:
//! AppBuilder::new(MyApp)
//!     .render_hub(hub)
//!     .run()?;
//! ```

use std::collections::HashSet;

use crate::backend::RenderBackend;
use crate::detect::default_perf;
#[cfg(not(target_arch = "wasm32"))]
use crate::detect::detect_backend;
use crate::metrics::RenderMetrics;

// ── HubError ──────────────────────────────────────────────────────────────────

/// Errors produced by [`RenderHub`].
#[derive(Debug, thiserror::Error)]
pub enum HubError {
    /// The requested backend is not present in the pool.
    #[error("backend {0:?} not in pool")]
    NotAvailable(RenderBackend),
    /// No wgpu adapter was found (GPU-less machine, no display).
    #[error("no wgpu adapter found")]
    NoAdapter,
}

// ── BackendPool ───────────────────────────────────────────────────────────────

/// The set of backends that are hardware-available on this machine.
///
/// Populated by [`RenderHub::autodetect`] via a wgpu adapter probe, or
/// constructed synthetically by [`RenderHub::fixed`].
#[derive(Debug, Clone)]
pub struct BackendPool {
    /// `true` when a wgpu adapter was successfully found.
    pub has_gpu: bool,
    /// Which backends have been confirmed (or assumed) available.
    pub initialized: HashSet<RenderBackend>,
    /// Recommended backend derived from the adapter's device type.
    pub recommended: RenderBackend,
}

impl BackendPool {
    /// Build a pool from a detected adapter (native only).
    #[cfg(not(target_arch = "wasm32"))]
    fn from_gpu(recommended: RenderBackend) -> Self {
        let mut initialized = HashSet::new();
        // GPU adapter present — all backends are potentially usable.
        initialized.insert(RenderBackend::VelloGpu);
        initialized.insert(RenderBackend::VelloHybrid);
        initialized.insert(RenderBackend::InstancedWgpu);
        initialized.insert(RenderBackend::VelloCpu);
        initialized.insert(RenderBackend::TinySkia);
        Self { has_gpu: true, initialized, recommended }
    }

    /// Build a pool when no GPU is available — software-only (native only).
    #[cfg(not(target_arch = "wasm32"))]
    fn software_only() -> Self {
        let mut initialized = HashSet::new();
        initialized.insert(RenderBackend::VelloCpu);
        initialized.insert(RenderBackend::TinySkia);
        Self {
            has_gpu: false,
            initialized,
            recommended: RenderBackend::TinySkia,
        }
    }

    /// Build a single-backend pool (for [`RenderHub::fixed`]).
    fn single(backend: RenderBackend) -> Self {
        let mut initialized = HashSet::new();
        initialized.insert(backend);
        // For a fixed pool, assume GPU presence iff the backend is GPU-based.
        Self {
            has_gpu: backend.is_gpu_swapchain(),
            initialized,
            recommended: backend,
        }
    }
}

// ── PerfSettings ──────────────────────────────────────────────────────────────

/// Performance control settings for the rendering pipeline.
///
/// Owned by [`RenderHub`]; read by the framework runtime each frame.
#[derive(Debug, Clone)]
pub struct PerfSettings {
    /// Target frames per second.  `0` = uncapped.
    pub fps_limit: u32,
    /// MSAA sample count.  `0` = area AA (vello), `8` = MSAA8, `16` = MSAA16.
    pub msaa_samples: u8,
    /// Whether VSync is enabled.
    pub vsync: bool,
    /// Emit per-frame timing to stderr when `true`.
    pub perf_log: bool,
    /// Scene recalculation mode: `"always"` | `"on_change"` | etc.
    pub recalc_mode: String,
}

impl Default for PerfSettings {
    fn default() -> Self {
        Self {
            fps_limit: 60,
            msaa_samples: 8,
            vsync: true,
            perf_log: false,
            recalc_mode: "on_change".into(),
        }
    }
}

// ── RenderHub ─────────────────────────────────────────────────────────────────

/// Unified rendering backend hub.
///
/// A single `RenderHub` owns backend selection, performance settings, and
/// accumulated frame metrics.  The framework runtime holds one `RenderHub` per
/// app (not per window) and queries it each frame.
///
/// # Construction
///
/// Use [`RenderHub::autodetect`] for production use.  Use [`RenderHub::fixed`]
/// when the backend is known at compile time and adapter probing is unwanted.
///
/// # Live switching
///
/// [`set_active`](Self::set_active) changes the active backend at runtime.
/// The change takes effect on the next call to
/// [`create_window_render_state`](Self::create_window_render_state) — in-flight
/// frames for existing windows are not affected mid-frame.
pub struct RenderHub {
    pool: BackendPool,
    active: RenderBackend,
    settings: PerfSettings,
    metrics: RenderMetrics,
}

impl RenderHub {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Probe a wgpu adapter and select the best available backend.
    ///
    /// The probe is synchronous (via `pollster`) and takes a few milliseconds.
    /// If no adapter is found the hub falls back to software-only backends
    /// (`TinySkia` / `VelloCpu`).
    ///
    /// On `wasm32` targets the probe is skipped and [`RenderBackend::Canvas2d`]
    /// is selected immediately — it is the only available backend in a browser.
    pub fn autodetect() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            return Self::fixed(RenderBackend::Canvas2d);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let (pool, recommended) = match probe_adapter() {
                Some(info) => {
                    let rec = detect_backend(&info);
                    (BackendPool::from_gpu(rec), rec)
                }
                None => {
                    let rec = RenderBackend::TinySkia;
                    (BackendPool::software_only(), rec)
                }
            };

            let perf = default_perf(recommended);
            let settings = PerfSettings {
                fps_limit: perf.fps_limit,
                msaa_samples: perf.msaa_samples,
                ..PerfSettings::default()
            };

            Self {
                active: pool.recommended,
                pool,
                settings,
                metrics: RenderMetrics::default(),
            }
        }
    }

    /// Construct a single-backend hub without adapter probing.
    ///
    /// Use this when the backend is hardcoded and no live switching is needed.
    /// [`is_available`](Self::is_available) will return `false` for any backend
    /// other than `backend`.
    pub fn fixed(backend: RenderBackend) -> Self {
        let pool = BackendPool::single(backend);
        let perf = default_perf(backend);
        let settings = PerfSettings {
            fps_limit: perf.fps_limit,
            msaa_samples: perf.msaa_samples,
            ..PerfSettings::default()
        };
        Self {
            active: backend,
            pool,
            settings,
            metrics: RenderMetrics::default(),
        }
    }

    // ── Pool queries ──────────────────────────────────────────────────────────

    /// Reference to the backend pool (what hardware is available).
    pub fn pool(&self) -> &BackendPool {
        &self.pool
    }

    /// The currently selected backend.
    pub fn active(&self) -> RenderBackend {
        self.active
    }

    /// Returns `true` if `backend` is in the pool.
    pub fn is_available(&self, backend: RenderBackend) -> bool {
        self.pool.initialized.contains(&backend)
    }

    /// Returns all backends available in the pool as a sorted Vec.
    pub fn available_backends(&self) -> Vec<RenderBackend> {
        let mut v: Vec<RenderBackend> = self.pool.initialized.iter().copied().collect();
        // Stable sort so the list order is deterministic in the UI.
        v.sort_by_key(|b| b.as_str());
        v
    }

    /// Switch the active backend.
    ///
    /// # Errors
    ///
    /// Returns [`HubError::NotAvailable`] if `backend` is not in the pool.
    pub fn set_active(&mut self, backend: RenderBackend) -> Result<(), HubError> {
        if !self.pool.initialized.contains(&backend) {
            return Err(HubError::NotAvailable(backend));
        }
        self.active = backend;
        Ok(())
    }

    // ── Performance settings ──────────────────────────────────────────────────

    /// Reference to the current performance settings.
    pub fn settings(&self) -> &PerfSettings {
        &self.settings
    }

    /// Mutable reference to the performance settings.
    pub fn settings_mut(&mut self) -> &mut PerfSettings {
        &mut self.settings
    }

    /// Set the FPS cap.  `0` = uncapped.
    pub fn set_fps_limit(&mut self, fps: u32) {
        self.settings.fps_limit = fps;
    }

    /// Set the MSAA sample count.  `0` = area AA, `8` = MSAA8, `16` = MSAA16.
    pub fn set_msaa(&mut self, samples: u8) {
        self.settings.msaa_samples = samples;
    }

    /// Enable or disable VSync.
    pub fn set_vsync(&mut self, on: bool) {
        self.settings.vsync = on;
    }

    // ── Metrics ───────────────────────────────────────────────────────────────

    /// Reference to the latest frame metrics snapshot.
    pub fn metrics(&self) -> &RenderMetrics {
        &self.metrics
    }

    /// Update the stored metrics (called by the runtime after each frame).
    pub fn update_metrics(&mut self, m: RenderMetrics) {
        self.metrics = m;
    }

    // ── Factory ───────────────────────────────────────────────────────────────

    /// Return a fresh `Box<dyn RenderSurfaceFactory>` for `backend`.
    ///
    /// Returns `None` if `backend` is not in the pool or has no factory
    /// implementation available on this platform.
    ///
    /// Called by the platform runtime when no explicit factory was supplied via
    /// [`AppBuilder::surface_factory`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn factory_for(&self, backend: RenderBackend) -> Option<Box<dyn crate::surface::RenderSurfaceFactory>> {
        use crate::factories::{
            VelloGpuSurfaceFactory, VelloHybridSurfaceFactory,
            WgpuInstancedSurfaceFactory, TinySkiaSurfaceFactory, VelloCpuSurfaceFactory,
        };
        if !self.pool.initialized.contains(&backend) {
            return None;
        }
        let factory: Box<dyn crate::surface::RenderSurfaceFactory> = match backend {
            RenderBackend::VelloGpu      => Box::new(VelloGpuSurfaceFactory::new()),
            RenderBackend::VelloHybrid   => Box::new(VelloHybridSurfaceFactory::new(1.0)),
            RenderBackend::InstancedWgpu => Box::new(WgpuInstancedSurfaceFactory::new()),
            RenderBackend::TinySkia      => Box::new(TinySkiaSurfaceFactory::new()),
            RenderBackend::VelloCpu      => Box::new(VelloCpuSurfaceFactory::new(1.0)),
            _                            => return None,
        };
        Some(factory)
    }

    /// wasm32 stub — Canvas2d is the only backend, canvas factory needs the
    /// element which isn't available at hub construction time.
    #[cfg(target_arch = "wasm32")]
    pub fn factory_for(&self, _backend: RenderBackend) -> Option<Box<dyn crate::surface::RenderSurfaceFactory>> {
        None
    }
}

// ── Adapter probe (desktop only) ──────────────────────────────────────────────

/// Synchronously request a wgpu adapter and return its info.
///
/// Returns `None` if no suitable adapter exists (pure software / headless).
/// Not available on `wasm32` — use [`RenderHub::fixed`]`(RenderBackend::Canvas2d)` instead.
#[cfg(not(target_arch = "wasm32"))]
fn probe_adapter() -> Option<wgpu::AdapterInfo> {
    // wgpu 29: Instance::new takes the descriptor by value; default() is gone,
    // use new_without_display_handle() instead.
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    })).ok()?;
    Some(adapter.get_info())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;

    #[test]
    fn autodetect_returns_nonempty_pool() {
        let hub = RenderHub::autodetect();
        assert!(!hub.pool().initialized.is_empty(), "pool must have at least one backend");
    }

    #[test]
    fn fixed_pool_has_exactly_one_backend() {
        let hub = RenderHub::fixed(RenderBackend::VelloGpu);
        assert_eq!(hub.pool().initialized.len(), 1);
        assert!(hub.is_available(RenderBackend::VelloGpu));
        assert!(!hub.is_available(RenderBackend::TinySkia));
    }

    #[test]
    fn set_active_rejects_non_pooled() {
        let mut hub = RenderHub::fixed(RenderBackend::VelloGpu);
        let result = hub.set_active(RenderBackend::TinySkia);
        assert!(matches!(result, Err(HubError::NotAvailable(RenderBackend::TinySkia))));
    }

    #[test]
    fn set_active_accepts_pooled() {
        let mut hub = RenderHub::autodetect();
        // autodetect always includes TinySkia on desktop.
        assert!(hub.set_active(RenderBackend::TinySkia).is_ok());
        assert_eq!(hub.active(), RenderBackend::TinySkia);
    }

    #[test]
    fn perf_settings_round_trip() {
        let mut hub = RenderHub::autodetect();
        hub.set_fps_limit(144);
        hub.set_msaa(16);
        hub.set_vsync(false);
        assert_eq!(hub.settings().fps_limit, 144);
        assert_eq!(hub.settings().msaa_samples, 16);
        assert!(!hub.settings().vsync);
    }

    #[test]
    fn perf_settings_mut() {
        let mut hub = RenderHub::autodetect();
        hub.settings_mut().recalc_mode = "always".into();
        assert_eq!(hub.settings().recalc_mode, "always");
    }
}
