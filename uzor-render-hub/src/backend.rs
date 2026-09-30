//! Render backend enum — canonical definition lives in `uzor::platform::types`.

pub use uzor::platform::types::{RenderBackend, RenderFamily};

/// `true` when the code that instantiates `backend` is compiled into this
/// build of `uzor-render-hub` (i.e. its Cargo feature is enabled).
///
/// [`RenderBackend`] is defined in `uzor` and always carries every variant,
/// so callers that pick a backend at run time can use this to skip variants
/// whose feature is off. With default features every native backend
/// returns `true`; `Canvas2d` is only ever `true` on `wasm32`.
pub fn is_backend_compiled(backend: RenderBackend) -> bool {
    match backend {
        RenderBackend::VelloGpu => cfg!(feature = "vello-gpu"),
        RenderBackend::VelloHybrid => cfg!(feature = "vello-hybrid"),
        RenderBackend::InstancedWgpu => cfg!(feature = "wgpu-instanced"),
        RenderBackend::VelloCpu => cfg!(feature = "vello-cpu"),
        RenderBackend::TinySkia => cfg!(feature = "tiny-skia"),
        RenderBackend::Canvas2d => cfg!(target_arch = "wasm32"),
        RenderBackend::UrxCpu
        | RenderBackend::UrxWgpu
        | RenderBackend::UrxHybrid
        | RenderBackend::UrxWgpuFull => cfg!(feature = "urx"),
    }
}
