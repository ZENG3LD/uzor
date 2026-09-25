//! `wgpu::PipelineCache` disk persistence helpers.
//!
//! On wgpu 28+, `PipelineCache` lets the Vulkan driver skip
//! shader→ISA compilation on subsequent runs by re-using a cached
//! binary blob from disk. Effect on a Pixel 6: 2000 ms → 30 ms
//! cold-start (~66×). On desktop the Vulkan driver maintains its
//! own implicit cache, so the visible gain is mostly first-run
//! after a driver update or first cold start.
//!
//! The cache is **Vulkan-only**. On Metal / DX12 / GL backends the
//! `device.create_pipeline_cache` call returns a stub that does
//! nothing — these helpers are still safe to call.
//!
//! ## Directory is host-supplied, never chosen here
//!
//! `urx-core` is a library — it must not decide where on disk it
//! writes (plan rev 2 §2). Every function here takes the cache
//! directory as a parameter; the host application resolves the
//! platform-specific base directory (e.g. `uzor-desktop`'s own
//! `window::creation::platform_pipeline_cache_dir` helper) and passes
//! it down, typically via `UrxConfig::pipeline_cache_dir`. Pass
//! `None` (to [`load_or_create`] / [`save_to_disk`]) to skip disk I/O
//! entirely — the pipeline cache still works as an in-memory-only
//! `fallback: true` cache, just never persisted. Headless / test
//! callers should always pass `None`.
//!
//! ## Usage
//!
//! ```ignore
//! use std::path::Path;
//! use uzor_urx_core::pipeline_cache as pc;
//!
//! let cache_dir = Path::new("/home/user/.cache"); // host-resolved
//! let cache = pc::load_or_create(&device, &adapter, Some(cache_dir), "urx-uzor");
//!
//! // pass to every create_render_pipeline:
//! let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
//!     cache: cache.as_ref(),
//!     ..
//! });
//!
//! // ...later, on graceful shutdown / interval flush:
//! let _ = pc::save_to_disk(&cache, Some(cache_dir), &adapter_info, "urx-uzor");
//! ```
//!
//! Failures (no Vulkan, cache corrupt, disk write error) silently
//! fall back to cold compile — that's the whole point of the
//! `fallback: true` flag passed to `create_pipeline_cache`.

use std::path::{Path, PathBuf};

/// Compute the on-disk path for a pipeline cache blob keyed on the
/// adapter (vendor/device/driver) so a driver update invalidates the
/// stale blob automatically.
///
/// Format: `<cache_dir>/<app_id>/pipeline-cache/<app_id>-<vendor>-<device>-<driver>.bin`
///
/// `cache_dir` is entirely the caller's choice — this crate never
/// resolves a platform base directory itself (see module doc).
#[cfg(feature = "pipeline-cache")]
pub fn cache_path_for_adapter(cache_dir: &Path, app_id: &str, info: &wgpu::AdapterInfo) -> PathBuf {
    let base = cache_dir.join(app_id).join("pipeline-cache");
    let key = format!(
        "{}-{:04x}-{:04x}-{}.bin",
        info.backend.to_str(),
        info.vendor,
        info.device,
        // Driver string can contain spaces / colons; flatten.
        info.driver
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>(),
    );
    base.join(key)
}

/// Pure variant for tests / callers that don't have an AdapterInfo
/// yet. Inputs are stringly-typed so this stays cfg-independent.
pub fn cache_path_for_key(
    cache_dir: &Path,
    app_id:    &str,
    backend:   &str,
    vendor:    u32,
    device:    u32,
    driver:    &str,
) -> PathBuf {
    let base = cache_dir.join(app_id).join("pipeline-cache");
    let key = format!(
        "{}-{:04x}-{:04x}-{}.bin",
        backend,
        vendor,
        device,
        driver
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>(),
    );
    base.join(key)
}

/// Load a pipeline cache from disk (or create a fresh fallback if no
/// disk blob exists or it's corrupt). Returns `None` if the wgpu
/// backend doesn't support `PipelineCache` (Metal/DX12/GL).
///
/// `cache_dir` is the host-resolved base directory (see module doc);
/// `None` skips the disk read and returns an in-memory-only cache.
/// `app_id` is used to namespace the cache file (e.g. `"urx-1.5"`).
/// Bumping it on incompatible-version releases is the consumer's
/// responsibility.
#[cfg(feature = "pipeline-cache")]
pub fn load_or_create(
    device:    &wgpu::Device,
    adapter:   &wgpu::Adapter,
    cache_dir: Option<&Path>,
    app_id:    &str,
) -> Option<wgpu::PipelineCache> {
    let info = adapter.get_info();
    // Vulkan-only feature; safe no-op on other backends — but only if
    // PIPELINE_CACHE feature was requested at device creation. If not,
    // create_pipeline_cache panics. Skip on non-Vulkan to be safe.
    if !adapter.features().contains(wgpu::Features::PIPELINE_CACHE) {
        return None;
    }

    let data = cache_dir
        .map(|dir| cache_path_for_adapter(dir, app_id, &info))
        .and_then(|path| std::fs::read(&path).ok());

    // SAFETY: PipelineCache data is opaque, validated by `fallback: true`
    // — corrupt blobs fall back to cold compile, not crash.
    let cache = unsafe {
        device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
            label:    Some("urx-pipeline-cache"),
            data:     data.as_deref(),
            fallback: true,
        })
    };
    Some(cache)
}

/// Persist a pipeline cache to disk. Returns `Ok(bytes_written)` on
/// success, `Err(_)` on I/O failure (silent in caller's hot path — we
/// don't want a shutdown hang on cache write). `Ok(0)` when
/// `cache_dir` is `None` — nothing to write to.
#[cfg(feature = "pipeline-cache")]
pub fn save_to_disk(
    cache:        &wgpu::PipelineCache,
    cache_dir:    Option<&Path>,
    adapter_info: &wgpu::AdapterInfo,
    app_id:       &str,
) -> std::io::Result<usize> {
    let Some(cache_dir) = cache_dir else { return Ok(0) };
    let data = match cache.get_data() {
        Some(d) => d,
        None => return Ok(0),
    };
    let path = cache_path_for_adapter(cache_dir, app_id, adapter_info);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Atomic write: write to tmp + rename.
    let tmp = path.with_extension("bin.tmp");
    std::fs::write(&tmp, &data)?;
    std::fs::rename(&tmp, &path)?;
    Ok(data.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = "/test-cache-root";

    #[test]
    fn cache_path_is_deterministic() {
        let p1 = cache_path_for_key(Path::new(DIR), "test", "Vulkan", 0x10de, 0x2782, "550.54.15");
        let p2 = cache_path_for_key(Path::new(DIR), "test", "Vulkan", 0x10de, 0x2782, "550.54.15");
        assert_eq!(p1, p2);
    }

    #[test]
    fn cache_path_namespaces_by_app_id() {
        let p1 = cache_path_for_key(Path::new(DIR), "appA", "Vulkan", 0x10de, 0x2782, "550");
        let p2 = cache_path_for_key(Path::new(DIR), "appB", "Vulkan", 0x10de, 0x2782, "550");
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_changes_on_driver_update() {
        let p_old = cache_path_for_key(Path::new(DIR), "app", "Vulkan", 0x10de, 0x2782, "550.54.15");
        let p_new = cache_path_for_key(Path::new(DIR), "app", "Vulkan", 0x10de, 0x2782, "552.12.00");
        assert_ne!(p_old, p_new);
    }

    #[test]
    fn cache_path_contains_app_id_segment() {
        let p = cache_path_for_key(Path::new(DIR), "urx-uzor", "Vulkan", 0, 0, "x");
        assert!(p.to_string_lossy().contains("urx-uzor"));
    }

    #[test]
    fn cache_path_is_rooted_under_the_supplied_cache_dir() {
        let p = cache_path_for_key(Path::new(DIR), "app", "Vulkan", 0, 0, "x");
        assert!(p.starts_with(DIR), "path {:?} must be rooted under the caller-supplied dir", p);
    }

    #[test]
    fn cache_path_changes_with_the_supplied_cache_dir() {
        let p1 = cache_path_for_key(Path::new("/root-a"), "app", "Vulkan", 0, 0, "x");
        let p2 = cache_path_for_key(Path::new("/root-b"), "app", "Vulkan", 0, 0, "x");
        assert_ne!(p1, p2);
    }
}
