//! Error type returned by [`crate::renderer::NativeUrxRenderer`].

/// Errors [`crate::renderer::NativeUrxRenderer::render_into_encoder`] can
/// return. Both variants are caller-bug conditions (bad viewport, wrong
/// target format) — never a mid-render GPU failure, so there is no
/// "recoverable at a later frame" variant here.
#[derive(Debug, thiserror::Error)]
pub enum NativeRenderError {
    /// `Viewport { width, height }` had a zero dimension.
    #[error("viewport width/height must be > 0 (got {width}x{height})")]
    ZeroViewport { width: u32, height: u32 },
    /// The caller's target `wgpu::TextureView` was created from a
    /// texture whose format doesn't match the format this renderer's
    /// pipelines were built for (`NativeUrxRenderer::new`/`with_sample_count`).
    #[error("target view format must match renderer format {expected:?} (view was created from a texture with a different format — caller bug, not a runtime GPU condition)")]
    FormatMismatch { expected: wgpu::TextureFormat },
}
