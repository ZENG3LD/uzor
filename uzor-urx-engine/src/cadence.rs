//! Per-region cadence hints. Consumer declares intent; engine
//! validates + auto-promotes between cached/direct paths.
//!
//! Doctrine: cadence policy lives OUTSIDE the engine (driver/kernel).
//! These hints only tell the engine WHICH RENDERING STRATEGY to use
//! per region (cached texture vs direct re-raster). The "should this
//! frame paint at all?" question is the driver's, not the engine's.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderCadence {
    /// Region content never changes after first paint. Rasterise once,
    /// cache as texture, composite from cache forever.
    #[default]
    Static,
    /// Region updates at most a few times/sec. Rasterise on dirty,
    /// cache between paints.
    LowHz(u8),
    /// Region repaints every frame (animation, video). Skip cache —
    /// direct re-raster every paint. "Volatile" in Unreal UMG terms.
    HighHz,
    /// Override: force GPU direct, no cache. For benchmarks /
    /// dev rigs that want to measure raw GPU cost.
    GpuForced,
    /// Override: force CPU direct, no cache. For headless / test /
    /// no-GPU contexts.
    CpuForced,
}

impl RenderCadence {
    /// Can this cadence reuse a cached texture across frames?
    pub fn allows_cache(self) -> bool {
        matches!(self, Self::Static | Self::LowHz(_))
    }

    /// Does this cadence force a specific backend?
    pub fn forces_backend(self) -> Option<ForcedBackend> {
        match self {
            Self::GpuForced => Some(ForcedBackend::Gpu),
            Self::CpuForced => Some(ForcedBackend::Cpu),
            _               => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForcedBackend { Gpu, Cpu }
