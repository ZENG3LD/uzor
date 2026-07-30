//! Error type returned by [`crate::renderer::NativeUrxRenderer`].

/// Errors [`crate::renderer::NativeUrxRenderer::render_into_encoder`] can
/// return.
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
    /// CPU-side path expansion exceeded the renderer's bounded frame
    /// geometry arena. The frame is rejected before a pathological
    /// allocation can abort the process; no partial geometry is drawn.
    #[error(
        "native URX frame {frame_id} capacity failure at {stage} for {buffer}: \
         requested {requested_instances} instances / {requested_bytes} bytes \
         ({instance_size} bytes each), current capacity {current_capacity_instances} instances, \
         per-buffer safety limit {max_bytes} bytes"
    )]
    GeometryCapacity {
        frame_id: u64,
        stage: &'static str,
        buffer: &'static str,
        requested_instances: usize,
        requested_bytes: usize,
        current_capacity_instances: usize,
        instance_size: usize,
        max_bytes: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_capacity_display_carries_frame_stage_and_bytes() {
        let error = NativeRenderError::GeometryCapacity {
            frame_id: 41,
            stage: "gpu_buffer_reserve",
            buffer: "path",
            requested_instances: 128,
            requested_bytes: 7_168,
            current_capacity_instances: 64,
            instance_size: 56,
            max_bytes: 4_096,
        };
        let message = error.to_string();

        assert!(message.contains("frame 41"));
        assert!(message.contains("gpu_buffer_reserve"));
        assert!(message.contains("path"));
        assert!(message.contains("7168 bytes"));
        assert!(message.contains("current capacity 64 instances"));
    }
}
