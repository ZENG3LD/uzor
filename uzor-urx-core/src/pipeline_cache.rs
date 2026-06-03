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
//! ## Usage
//!
//! ```ignore
//! use uzor_urx_core::pipeline_cache as pc;
//!
//! let cache = pc::load_or_create(&device, &adapter_info, "urx-uzor");
//!
//! // pass to every create_render_pipeline:
//! let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
//!     cache: cache.as_ref(),
//!     ..
//! });
//!
//! // ...later, on graceful shutdown / interval flush:
//! let _ = pc::save_to_disk(&cache, &adapter_info, "urx-uzor");
//! ```
//!
//! Failures (no Vulkan, cache corrupt, disk write error) silently
//! fall back to cold compile — that's the whole point of the
//! `fallback: true` flag passed to `create_pipeline_cache`.

use std::path::PathBuf;

/// Compute the on-disk path for a pipeline cache blob keyed on the
/// adapter (vendor/device/driver) so a driver update invalidates the
/// stale blob automatically.
///
/// Format: `<cache_dir>/<app_id>-<vendor>-<device>-<driver>.bin`
///
/// `cache_dir` resolves to:
/// - Windows: `%LOCALAPPDATA%\<app_id>\pipeline-cache\`
/// - Linux:   `$XDG_CACHE_HOME/<app_id>/pipeline-cache/`
///            or `~/.cache/<app_id>/pipeline-cache/`
/// - macOS:   `~/Library/Caches/<app_id>/pipeline-cache/`
///
/// All fallbacks: `<temp>/<app_id>/pipeline-cache/`.
#[cfg(feature = "pipeline-cache")]
pub fn cache_path_for_adapter(app_id: &str, info: &wgpu::AdapterInfo) -> PathBuf {
    let base = cache_dir(app_id);
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
pub fn cache_path_for_key(app_id: &str, backend: &str, vendor: u32, device: u32, driver: &str) -> PathBuf {
    let base = cache_dir(app_id);
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

fn cache_dir(app_id: &str) -> PathBuf {
    if let Some(base) = dirs_like_cache_dir() {
        return base.join(app_id).join("pipeline-cache");
    }
    std::env::temp_dir().join(app_id).join("pipeline-cache")
}

/// Mini `dirs::cache_dir` replacement (we don't want a new dep).
fn dirs_like_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches"))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
            Some(PathBuf::from(xdg))
        } else {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))
        }
    }
}

/// Load a pipeline cache from disk (or create a fresh fallback if no
/// disk blob exists or it's corrupt). Returns `None` if the wgpu
/// backend doesn't support `PipelineCache` (Metal/DX12/GL).
///
/// `app_id` is used to namespace the cache file (e.g. `"urx-1.5"`).
/// Bumping it on incompatible-version releases is the consumer's
/// responsibility.
#[cfg(feature = "pipeline-cache")]
pub fn load_or_create(
    device:  &wgpu::Device,
    adapter: &wgpu::Adapter,
    app_id:  &str,
) -> Option<wgpu::PipelineCache> {
    let info = adapter.get_info();
    // Vulkan-only feature; safe no-op on other backends — but only if
    // PIPELINE_CACHE feature was requested at device creation. If not,
    // create_pipeline_cache panics. Skip on non-Vulkan to be safe.
    if !adapter.features().contains(wgpu::Features::PIPELINE_CACHE) {
        return None;
    }

    let path = cache_path_for_adapter(app_id, &info);
    let data = std::fs::read(&path).ok();

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
/// don't want a shutdown hang on cache write).
#[cfg(feature = "pipeline-cache")]
pub fn save_to_disk(
    cache:   &wgpu::PipelineCache,
    adapter_info: &wgpu::AdapterInfo,
    app_id:  &str,
) -> std::io::Result<usize> {
    let data = match cache.get_data() {
        Some(d) => d,
        None => return Ok(0),
    };
    let path = cache_path_for_adapter(app_id, adapter_info);
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

    #[test]
    fn cache_path_is_deterministic() {
        let p1 = cache_path_for_key("test", "Vulkan", 0x10de, 0x2782, "550.54.15");
        let p2 = cache_path_for_key("test", "Vulkan", 0x10de, 0x2782, "550.54.15");
        assert_eq!(p1, p2);
    }

    #[test]
    fn cache_path_namespaces_by_app_id() {
        let p1 = cache_path_for_key("appA", "Vulkan", 0x10de, 0x2782, "550");
        let p2 = cache_path_for_key("appB", "Vulkan", 0x10de, 0x2782, "550");
        assert_ne!(p1, p2);
    }

    #[test]
    fn cache_path_changes_on_driver_update() {
        let p_old = cache_path_for_key("app", "Vulkan", 0x10de, 0x2782, "550.54.15");
        let p_new = cache_path_for_key("app", "Vulkan", 0x10de, 0x2782, "552.12.00");
        assert_ne!(p_old, p_new);
    }

    #[test]
    fn cache_path_contains_app_id_segment() {
        let p = cache_path_for_key("urx-uzor", "Vulkan", 0, 0, "x");
        assert!(p.to_string_lossy().contains("urx-uzor"));
    }
}
