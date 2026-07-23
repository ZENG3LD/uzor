//! Backend auto-detection from a wgpu adapter.
//!
//! `detect_backend` (Vello-family decision tree, logic copied verbatim
//! from mlc `chart-app-vello/src/main.rs`) and `detect_backend_urx`
//! (URX-family decision tree) live side by side here — **not** a
//! flip-in-progress with one destined to replace the other.
//!
//! Owner decision 2026-07-24: **no default flip, ever.** Which tree a
//! given app/window walks is resolved from [`RenderFamily`] — a
//! per-app/per-window flag, not a workspace migration. The two call
//! sites (`uzor-render-hub::hub::RenderHub::autodetect`,
//! `uzor-desktop::window::creation::create_window`) both take a
//! [`RenderFamily`] parameter and dispatch through
//! [`detect_backend_for_family`] / [`no_adapter_backend_for_family`]
//! rather than calling either named tree directly — this replaces the
//! earlier Wave 6 "FLIP POINT" doc comments that described this as a
//! one-line future call-site swap (that framing is now wrong: the flag
//! stays, permanently, by owner decision).
//!
//! # Family resolution precedence
//!
//! Four levels, highest first:
//! 1. **Explicit backend** — `.backend(Scene2DBackend::X)` on the app
//!    builder (or `WindowConfig::backend_hint`) skips family resolution
//!    entirely; this module never even runs in that case.
//! 2. **`UZOR_RENDER_FAMILY` env var** (`vello` / `urx`, case-
//!    insensitive) — [`resolve_render_family_from_process_env`].
//! 3. **Builder's own `.render_family(...)`** setting.
//! 4. **[`RenderFamily::default`]** (`Vello`).
//!
//! [`resolve_render_family`] is the PURE core of that precedence chain
//! (levels 2-4 only — level 1 is checked by the caller before this
//! module is ever consulted) — it takes an `Option<&str>` instead of
//! reading `std::env` itself, specifically so the precedence order is
//! unit-testable without mutating the real process environment.
//! [`resolve_render_family_from_process_env`] is the thin `std::env`-
//! reading wrapper actual call sites use.

use serde::{Deserialize, Serialize};

use crate::backend::{RenderBackend, RenderFamily};

/// Per-backend performance defaults (fps target + MSAA).
#[derive(Debug, Clone, Copy)]
pub struct PerfDefaults {
    /// Target frames per second.
    pub fps_limit: u32,
    /// MSAA sample count (1 = disabled, 4/8/16 = enabled).
    pub msaa_samples: u8,
}

/// Pick a [`RenderBackend`] from wgpu adapter info.
///
/// Match arms copied verbatim from mlc.
pub fn detect_backend(info: &wgpu::AdapterInfo) -> RenderBackend {
    match info.device_type {
        wgpu::DeviceType::DiscreteGpu   => RenderBackend::VelloGpu,
        wgpu::DeviceType::IntegratedGpu => RenderBackend::VelloGpu,
        wgpu::DeviceType::VirtualGpu    => RenderBackend::VelloCpu,
        wgpu::DeviceType::Cpu           => RenderBackend::TinySkia,
        _                               => RenderBackend::VelloGpu,
    }
}

/// URX-family autodetect decision tree — lives permanently ALONGSIDE
/// [`detect_backend`] (owner decision 2026-07-24: no default flip,
/// ever; both trees are first-class, selected per-app/per-window via
/// [`RenderFamily`], never a migration where one replaces the other).
/// Reached through [`detect_backend_for_family`] at both real call
/// sites (`RenderHub::autodetect`, `uzor-desktop`'s `create_window`)
/// rather than named directly by application code.
///
/// Mapping vs [`detect_backend`]: `DiscreteGpu`/`IntegratedGpu` (Vello:
/// `VelloGpu`) → [`RenderBackend::UrxWgpu`] — the Wave 6 Commit 1
/// gate-verified native GPU path. `VirtualGpu` (Vello: `VelloCpu`) →
/// [`RenderBackend::UrxCpu`] — `VirtualGpu` denotes a software/
/// virtualized adapter, functionally CPU-class despite the GPU-shaped
/// enum name; the brief's "CPU-only → urx-cpu" arm covers it directly
/// (this specific remap is unverified live on an actual
/// `VirtualGpu`-reporting box as of this pass — verify on a VM/CI
/// runner before trusting broadly). `Cpu` (Vello: `TinySkia`) →
/// [`RenderBackend::UrxCpu`]. Unknown/`_` (Vello: `VelloGpu`, an
/// optimistic default) → [`RenderBackend::UrxWgpu`], keeping the same
/// "assume GPU-capable" policy the Vello-family tree has for that
/// fallthrough arm.
pub fn detect_backend_urx(info: &wgpu::AdapterInfo) -> RenderBackend {
    match info.device_type {
        wgpu::DeviceType::DiscreteGpu   => RenderBackend::UrxWgpu,
        wgpu::DeviceType::IntegratedGpu => RenderBackend::UrxWgpu,
        wgpu::DeviceType::VirtualGpu    => RenderBackend::UrxCpu,
        wgpu::DeviceType::Cpu           => RenderBackend::UrxCpu,
        _                                => RenderBackend::UrxWgpu,
    }
}

// ── RenderFamily resolution + dispatch ─────────────────────────────────────

/// Dispatch to the family's own adapter-present decision tree —
/// [`detect_backend`] for [`RenderFamily::Vello`], [`detect_backend_urx`]
/// for [`RenderFamily::Urx`]. The one place both call sites
/// (`RenderHub::autodetect`, `uzor-desktop::create_window`) go through,
/// so neither one names either tree directly.
pub fn detect_backend_for_family(info: &wgpu::AdapterInfo, family: RenderFamily) -> RenderBackend {
    match family {
        RenderFamily::Vello => detect_backend(info),
        RenderFamily::Urx   => detect_backend_urx(info),
    }
}

/// The no-adapter-found fallback for a given family — [`RenderBackend::
/// TinySkia`] for [`RenderFamily::Vello`] (unchanged from today's
/// `RenderHub::autodetect` behavior; [`RenderBackend::TinySkia`] is the
/// Vello family's own no-GPU fallback, not a family itself — see
/// [`RenderFamily`]'s own doc comment), [`RenderBackend::UrxCpu`] for
/// [`RenderFamily::Urx`] (a genuinely no-adapter box is CPU-only by
/// definition, and [`detect_backend_urx`]'s own `Cpu` arm already
/// agrees — see that function's own mapping doc).
pub fn no_adapter_backend_for_family(family: RenderFamily) -> RenderBackend {
    match family {
        RenderFamily::Vello => RenderBackend::TinySkia,
        RenderFamily::Urx   => RenderBackend::UrxCpu,
    }
}

/// Parse the `UZOR_RENDER_FAMILY` env value. Pure — no `std::env` read
/// inside, so the four-level precedence chain (see this module's own
/// doc comment) is unit-testable without mutating the real process
/// environment.
///
/// `None` (env var unset) and a recognized value (`"vello"`/`"urx"`,
/// case-insensitive) both resolve cleanly to `Ok`; an unrecognized
/// non-empty value is `Err(the original string)` so the caller can
/// warn about a TYPO distinctly from "simply not set."
fn parse_render_family_env(value: Option<&str>) -> Result<Option<RenderFamily>, &str> {
    match value {
        None => Ok(None),
        Some(v) => match v.to_ascii_lowercase().as_str() {
            "vello" => Ok(Some(RenderFamily::Vello)),
            "urx" => Ok(Some(RenderFamily::Urx)),
            _ => Err(v),
        },
    }
}

/// Print the invalid-`UZOR_RENDER_FAMILY`-value warning at most once
/// per process — a print-dedup latch only, **not** a cache of the
/// resolved family itself (the family is still re-resolved fresh on
/// every call to [`resolve_render_family`]/
/// [`resolve_render_family_from_process_env`] — no global static holds
/// the DECISION, matching the "thread it as a param, never a global"
/// rule this whole mechanism follows).
fn warn_invalid_render_family_env_once(value: &str) {
    static WARNED: std::sync::Once = std::sync::Once::new();
    WARNED.call_once(|| {
        eprintln!(
            "[uzor-render-hub] UZOR_RENDER_FAMILY={value:?} is not \"vello\" or \"urx\" (case-insensitive) — \
             ignoring it and falling back to the app builder's own render_family (or RenderFamily::Vello if \
             that's unset too). This warning prints at most once per process."
        );
    });
}

/// Resolve the effective [`RenderFamily`] from an env override value
/// and the app builder's own `.render_family(...)` setting, in that
/// precedence order, falling back to [`RenderFamily::default`] (`Vello`)
/// when neither is set — precedence levels 2-4 of the four-level chain
/// documented at the top of this module (level 1, an explicit
/// `.backend(...)` selection, is checked by the CALLER before this
/// function is ever reached — see `uzor-desktop::Manager::from_built`'s
/// own `(Some(b), _)` arm).
///
/// Pure (`env_value` is handed in, never read from `std::env` here) so
/// the precedence order is unit-testable without mutating the real
/// process environment — see [`resolve_render_family_from_process_env`]
/// for the thin wrapper actual call sites use.
pub fn resolve_render_family(env_value: Option<&str>, builder_family: Option<RenderFamily>) -> RenderFamily {
    match parse_render_family_env(env_value) {
        Ok(Some(family)) => family,
        Ok(None) => builder_family.unwrap_or_default(),
        Err(invalid) => {
            warn_invalid_render_family_env_once(invalid);
            builder_family.unwrap_or_default()
        }
    }
}

/// [`resolve_render_family`], reading `UZOR_RENDER_FAMILY` from the
/// real process environment. The actual entry point both call sites
/// (`RenderHub::autodetect`, `uzor-desktop::create_window`) use;
/// production code should call this, not [`resolve_render_family`]
/// directly (that one is the pure core kept separate for testability).
pub fn resolve_render_family_from_process_env(builder_family: Option<RenderFamily>) -> RenderFamily {
    let raw = std::env::var("UZOR_RENDER_FAMILY").ok();
    resolve_render_family(raw.as_deref(), builder_family)
}

/// Per-backend performance defaults. FPS values copied verbatim from mlc.
///
/// MSAA defaults are `0` (= vello `AaConfig::Area`) across the board: the
/// default GPU factory compiles vello shaders area-only
/// (`factories.rs` — `AaSupport { area: true, msaa8: false, msaa16: false }`),
/// so a non-zero default made every vello-gpu app panic at first submit
/// ("shaders not configured to support AA mode: msaa8") unless it carried a
/// `.msaa(0)` workaround. Explicit MSAA remains available via
/// `RenderControl::set_msaa_samples` for apps that install an
/// `AaSupport::all()` factory.
pub fn default_perf(backend: RenderBackend) -> PerfDefaults {
    let (fps_limit, msaa_samples) = match backend {
        RenderBackend::VelloGpu      => (120u32, 0u8),
        RenderBackend::VelloCpu      => (30,     0),
        RenderBackend::TinySkia      => (90,     0),
        RenderBackend::InstancedWgpu => (90,     0),
        RenderBackend::VelloHybrid   => (90,     0),
        RenderBackend::Canvas2d      => (60,     0),
        // URX family — mirrors Vello-equivalent defaults until real benches
        // give us numbers.
        RenderBackend::UrxCpu        => (30,     0),
        RenderBackend::UrxWgpu       => (90,     0),
        RenderBackend::UrxHybrid     => (90,     0),
        RenderBackend::UrxWgpuFull   => (120,    0),
    };
    PerfDefaults { fps_limit, msaa_samples }
}

/// Coarse-grained recommendation: GPU vs CPU vs fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedBackend {
    VelloGpu,
    VelloCpu,
    TinySkia,
}

/// GPU info extracted from a wgpu adapter.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub driver: String,
    pub device_type: wgpu::DeviceType,
    pub recommended: RecommendedBackend,
    pub backend: RenderBackend,
}

/// Detect GPU + recommend a backend.
pub fn detect(info: &wgpu::AdapterInfo) -> GpuInfo {
    let backend = detect_backend(info);
    let recommended = match info.device_type {
        wgpu::DeviceType::DiscreteGpu   => RecommendedBackend::VelloGpu,
        wgpu::DeviceType::IntegratedGpu => RecommendedBackend::VelloGpu,
        wgpu::DeviceType::VirtualGpu    => RecommendedBackend::VelloCpu,
        wgpu::DeviceType::Cpu           => RecommendedBackend::TinySkia,
        _                               => RecommendedBackend::VelloGpu,
    };
    GpuInfo {
        name: info.name.clone(),
        driver: info.driver.clone(),
        device_type: info.device_type,
        recommended,
        backend,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(device_type: wgpu::DeviceType) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: "test-adapter".to_string(),
            vendor: 0,
            device: 0,
            device_type,
            device_pci_bus_id: String::new(),
            driver: String::new(),
            driver_info: String::new(),
            backend: wgpu::Backend::Noop,
            subgroup_min_size: 0,
            subgroup_max_size: 0,
            transient_saves_memory: false,
        }
    }

    /// Wave 6 Commit 2 — every `detect_backend_urx` arm (design §5.1),
    /// covering all 5 `wgpu::DeviceType` variants (the 5th, `Other`,
    /// exercises the `_` fallthrough).
    #[test]
    fn discrete_and_integrated_gpu_map_to_urx_wgpu() {
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::DiscreteGpu)), RenderBackend::UrxWgpu);
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::IntegratedGpu)), RenderBackend::UrxWgpu);
    }

    #[test]
    fn virtual_gpu_and_cpu_map_to_urx_cpu() {
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::VirtualGpu)), RenderBackend::UrxCpu);
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::Cpu)), RenderBackend::UrxCpu);
    }

    #[test]
    fn unknown_device_type_falls_through_to_urx_wgpu() {
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::Other)), RenderBackend::UrxWgpu);
    }

    /// `detect_backend_urx` must never return a non-URX variant — the
    /// whole point of this decision tree is that autodetect, once
    /// flipped onto it, only ever lands on `UrxCpu`/`UrxWgpu`.
    #[test]
    fn never_returns_a_non_urx_backend() {
        for dt in [
            wgpu::DeviceType::DiscreteGpu,
            wgpu::DeviceType::IntegratedGpu,
            wgpu::DeviceType::VirtualGpu,
            wgpu::DeviceType::Cpu,
            wgpu::DeviceType::Other,
        ] {
            let backend = detect_backend_urx(&info(dt));
            assert!(
                matches!(backend, RenderBackend::UrxCpu | RenderBackend::UrxWgpu),
                "device_type {dt:?} produced non-URX backend {backend:?}"
            );
        }
    }

    // ── RenderFamily resolution + dispatch (2026-07-24 no-default-flip) ──

    #[test]
    fn env_none_and_builder_none_falls_back_to_default_vello() {
        assert_eq!(resolve_render_family(None, None), RenderFamily::Vello);
    }

    #[test]
    fn env_none_falls_back_to_builder_family() {
        assert_eq!(resolve_render_family(None, Some(RenderFamily::Urx)), RenderFamily::Urx);
        assert_eq!(resolve_render_family(None, Some(RenderFamily::Vello)), RenderFamily::Vello);
    }

    #[test]
    fn valid_env_value_beats_builder_family() {
        // Env says urx, builder says vello -- env must win.
        assert_eq!(resolve_render_family(Some("urx"), Some(RenderFamily::Vello)), RenderFamily::Urx);
        // Env says vello, builder says urx -- env must still win.
        assert_eq!(resolve_render_family(Some("vello"), Some(RenderFamily::Urx)), RenderFamily::Vello);
    }

    #[test]
    fn env_value_parsing_is_case_insensitive() {
        assert_eq!(resolve_render_family(Some("URX"), None), RenderFamily::Urx);
        assert_eq!(resolve_render_family(Some("Vello"), None), RenderFamily::Vello);
        assert_eq!(resolve_render_family(Some("uRx"), None), RenderFamily::Urx);
    }

    #[test]
    fn invalid_env_value_falls_back_to_builder_family_not_default() {
        // A typo'd env value must not silently win, nor silently reset
        // to Vello when the builder explicitly asked for Urx.
        assert_eq!(resolve_render_family(Some("uwu"), Some(RenderFamily::Urx)), RenderFamily::Urx);
        assert_eq!(resolve_render_family(Some("bogus"), None), RenderFamily::Vello);
    }

    #[test]
    fn parse_render_family_env_is_pure_and_distinguishes_unset_from_invalid() {
        assert_eq!(parse_render_family_env(None), Ok(None));
        assert_eq!(parse_render_family_env(Some("vello")), Ok(Some(RenderFamily::Vello)));
        assert_eq!(parse_render_family_env(Some("urx")), Ok(Some(RenderFamily::Urx)));
        assert_eq!(parse_render_family_env(Some("nope")), Err("nope"));
    }

    /// Family -> adapter-present tree mapping (owner scope item 6):
    /// `Urx+DiscreteGpu -> UrxWgpu`, `Vello+DiscreteGpu -> VelloGpu`.
    #[test]
    fn detect_backend_for_family_dispatches_to_the_right_tree_on_discrete_gpu() {
        let i = info(wgpu::DeviceType::DiscreteGpu);
        assert_eq!(detect_backend_for_family(&i, RenderFamily::Urx), RenderBackend::UrxWgpu);
        assert_eq!(detect_backend_for_family(&i, RenderFamily::Vello), RenderBackend::VelloGpu);
    }

    /// `Urx+no-adapter -> UrxCpu`, `Vello+no-adapter -> TinySkia`.
    #[test]
    fn no_adapter_backend_for_family_matches_each_familys_own_cpu_fallback() {
        assert_eq!(no_adapter_backend_for_family(RenderFamily::Urx), RenderBackend::UrxCpu);
        assert_eq!(no_adapter_backend_for_family(RenderFamily::Vello), RenderBackend::TinySkia);
    }

    /// `detect_backend_urx`'s own `Cpu` arm and the no-adapter fallback
    /// must agree (owner scope item 4) -- both are "this box is
    /// CPU-only" verdicts and should never diverge.
    #[test]
    fn urx_cpu_device_type_and_urx_no_adapter_fallback_agree() {
        assert_eq!(detect_backend_urx(&info(wgpu::DeviceType::Cpu)), RenderBackend::UrxCpu);
        assert_eq!(no_adapter_backend_for_family(RenderFamily::Urx), RenderBackend::UrxCpu);
    }
}
