//! `NativeUrxRenderer` — owns every native-pipeline GPU resource and
//! drives one `Scene` → pixels frame via [`NativeUrxRenderer::render_into_encoder`].
//!
//! Device/queue ownership shape matches `uzor_urx_wgpu_full::WgpuFullBackend`
//! (`uzor-urx-wgpu-full/src/backend.rs:33-92`, design §0 house style 1):
//! cloned once at construction (cheap — `wgpu::Device`/`Queue` are
//! Arc-backed), never borrowed from the caller again. The caller owns
//! the `wgpu::Instance`/adapter/surface acquisition and the per-frame
//! `CommandEncoder` + target `TextureView`; this type owns pipelines,
//! instance buffers, the MSAA offscreen target, and the uniform bind
//! group.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use uzor_urx_core::config::UrxConfig;

use crate::atlas::{AtlasStats, NativeGlyphAtlas};
use crate::encode::{self, BatchKind};
use crate::msaa::MsaaTarget;
use crate::native_error::NativeRenderError;
use crate::pipelines::glyph::GlyphPipeline;
use crate::pipelines::line::LinePipeline;
use crate::pipelines::path::PathPipeline;
use crate::pipelines::quad::QuadPipeline;
use crate::tessellate::{TessCache, TessCacheStats};

/// Target dimensions in physical pixels for one `render_into_encoder`
/// call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

/// Owns every native-pipeline GPU resource: the Quad SDF pipeline
/// (Wave 1 Commit 1), the Line/capsule pipeline (Commit 2), the Path/
/// triangle pipeline (Commit 3), and the Glyph pipeline + its
/// `NativeGlyphAtlas` (Wave 2 Commit 2) — the shared uniform bind
/// group (group 0, `screen_size`), the MSAA offscreen color target
/// (exact-match reallocation on resize, see `msaa.rs`'s module doc),
/// the tessellation cache, and the glyph atlas.
pub struct NativeUrxRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    sample_count: u32,

    quad: QuadPipeline,
    line: LinePipeline,
    path: PathPipeline,
    glyph: GlyphPipeline,

    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    msaa: MsaaTarget,
    tess_cache: TessCache,
    glyph_atlas: NativeGlyphAtlas,
}

impl NativeUrxRenderer {
    /// Default 4x MSAA (`uzor-urx-3d`'s `MSAA_SAMPLE_COUNT`,
    /// `uzor-urx-3d/src/pipeline.rs:392`) — the MSAA plumbing is wired
    /// completely from Commit 1 (coordinator Amendment A), not deferred
    /// to Commit 3. Uses `UrxConfig::default()` — see [`Self::with_config`]
    /// to tune the tessellation-cache cap or any other family-wide knob.
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        Self::with_sample_count(device, queue, format, 4)
    }

    /// `sample_count = 1` disables MSAA — the render pass then draws
    /// straight into the caller's view with no resolve step. Any other
    /// value arms MSAA at exactly that sample count for every pipeline
    /// built by this renderer. Uses `UrxConfig::default()`.
    pub fn with_sample_count(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        Self::with_config(device, queue, format, sample_count, &UrxConfig::default())
    }

    /// The full constructor — every other constructor delegates here.
    /// Naming matches `uzor_urx_cpu::CpuBackend::with_config`'s house
    /// style (a config-accepting sibling of the config-less
    /// convenience constructors), extended with this renderer's own
    /// GPU-specific parameters (`device`/`queue`/`format`/`sample_count`)
    /// that `UrxConfig` deliberately doesn't own (it's a plain-data,
    /// backend-agnostic struct shared with the CPU family — see its
    /// own module doc).
    ///
    /// `cfg` is read ONCE, here, at construction — every knob this
    /// renderer consumes from it (`path_tess_cache_cap`,
    /// `wgpu_glyph_atlas_w`/`_h`) is baked into the owned resource it
    /// configures (`TessCache::with_cap`, `NativeGlyphAtlas::new`) and
    /// is NOT hot-swappable for the lifetime of this renderer.
    /// Re-construct to pick up a changed config.
    pub fn with_config(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        sample_count: u32,
        cfg: &UrxConfig,
    ) -> Self {
        let sample_count = sample_count.max(1);

        let uniform_data = Uniforms { screen_size: [1.0, 1.0], _pad: [0.0; 2] };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uzor_urx_wgpu.native_uniforms"),
            contents: bytemuck::bytes_of(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uzor_urx_wgpu.native_uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uzor_urx_wgpu.native_uniform_bg"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() }],
        });

        let quad = QuadPipeline::new(&device, format, sample_count, &uniform_bgl);
        let line = LinePipeline::new(&device, format, sample_count, &uniform_bgl);
        let path = PathPipeline::new(&device, format, sample_count, &uniform_bgl);
        // The atlas must be built BEFORE the Glyph pipeline — the
        // pipeline's layout borrows the atlas's `BindGroupLayout` at
        // pipeline-creation time (design §4), fixed for the pipeline's
        // lifetime (this crate has no atlas-resize path).
        let glyph_atlas = NativeGlyphAtlas::new(&device, cfg.wgpu_glyph_atlas_w, cfg.wgpu_glyph_atlas_h);
        let glyph = GlyphPipeline::new(&device, format, sample_count, &uniform_bgl, glyph_atlas.bind_group_layout());
        let msaa = MsaaTarget::new(sample_count, format);
        let tess_cache = TessCache::with_cap(cfg.path_tess_cache_cap);

        Self {
            device,
            queue,
            format,
            sample_count,
            quad,
            line,
            path,
            glyph,
            uniform_buffer,
            uniform_bind_group,
            msaa,
            tess_cache,
            glyph_atlas,
        }
    }

    /// `format` this renderer's pipelines were built for.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Viewport hint — pre-allocates the MSAA target for `width x
    /// height` so the next `render_into_encoder` at that size doesn't
    /// pay a realloc mid-frame. `render_into_encoder` calls this
    /// internally if skipped. Reallocates on any size CHANGE, not just
    /// growth (`msaa.rs`'s module doc) — required for the multisample
    /// resolve step, which needs the MSAA and destination views to
    /// match dimensions exactly.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.msaa.ensure(&self.device, width, height);
    }

    /// Read-only tessellation-cache telemetry — hit/miss/entry counts
    /// (design §2). Not a hot-path cost: three `usize`/`u64` loads.
    pub fn tess_cache_stats(&self) -> TessCacheStats {
        self.tess_cache.stats()
    }

    /// Read-only glyph-atlas telemetry — hit/miss/eviction/entry counts
    /// (Wave 2 design §3/§9 Commit 3, pulled forward into Commit 2 —
    /// a 3-line accessor, no reason to defer it). Not a hot-path cost:
    /// four `usize`/`u64` loads.
    pub fn glyph_atlas_stats(&self) -> AtlasStats {
        self.glyph_atlas.stats()
    }

    /// Walk `scene.commands`, encode into the Quad pipeline's instance
    /// buffer, and render into `view` (resolving through the MSAA
    /// target first when `sample_count > 1`). Never panics on scene
    /// content — invalid primitives are validated via
    /// `uzor_urx_core::validate::validate_command` and skipped +
    /// countered inside `encode::encode_scene`, same policy as
    /// `uzor-urx-cpu`.
    pub fn render_into_encoder(
        &mut self,
        scene: &uzor_urx_core::scene::Scene,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: Viewport,
    ) -> Result<(), NativeRenderError> {
        if viewport.width == 0 || viewport.height == 0 {
            return Err(NativeRenderError::ZeroViewport { width: viewport.width, height: viewport.height });
        }
        if view.texture().format() != self.format {
            return Err(NativeRenderError::FormatMismatch { expected: self.format });
        }

        self.resize(viewport.width, viewport.height);

        // Advance the atlas's per-frame tick BEFORE walking the scene —
        // every `get_or_insert` this frame stamps its slot with the NEW
        // tick, which is what makes the never-evict-this-frame
        // invariant work (`atlas.rs`'s module doc).
        self.glyph_atlas.begin_frame();
        let frame = encode::encode_scene(scene, viewport, &mut self.tess_cache, Some(&mut self.glyph_atlas));

        let uniforms = Uniforms { screen_size: [viewport.width as f32, viewport.height as f32], _pad: [0.0; 2] };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        self.quad.upload(&self.device, &self.queue, &frame.quads);
        self.line.upload(&self.device, &self.queue, &frame.lines);
        self.path.upload(&self.device, &self.queue, &frame.triangles);
        self.glyph.upload(&self.device, &self.queue, &frame.glyphs);
        // Drain every glyph bitmap queued by this frame's `encode_scene`
        // call into the atlas texture — after encode returns (every
        // glyph this frame has already been placed) and before the
        // render pass begins (so it samples up-to-date contents).
        self.glyph_atlas.flush_uploads(&self.queue);

        // `sample_count > 1`: render into the MSAA color target and let
        // the pass epilogue hardware-resolve into the caller's `view`
        // (matches `uzor-urx-3d`'s `resolve_target` shape,
        // `pipeline.rs:2717-2735` — zero extra encoder submissions).
        // `sample_count == 1`: render straight into `view`, no resolve.
        let (color_view, resolve_target, store_op) = if self.sample_count > 1 {
            (self.msaa.color_view(), Some(view), wgpu::StoreOp::Discard)
        } else {
            (view, None, wgpu::StoreOp::Store)
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("uzor_urx_wgpu.native_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: store_op,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Replay batches in scan order — this is what preserves
            // painter's order across a Quad/Line interleave (design
            // §5; legacy `renderer.rs:1008-1061` pattern). Every batch
            // is guaranteed to differ in kind from its predecessor
            // (coalescing already merged same-kind runs at encode
            // time), so `current` always triggers a (re)bind here —
            // kept anyway to document the intent and stay correct if
            // a future pipeline ever breaks that invariant.
            let mut current: Option<BatchKind> = None;
            for batch in &frame.batches {
                if batch.count == 0 {
                    continue;
                }
                match batch.kind {
                    BatchKind::Quad => {
                        if current != Some(BatchKind::Quad) {
                            self.quad.bind(&mut pass);
                            current = Some(BatchKind::Quad);
                        }
                        self.quad.draw_range(&mut pass, batch.start, batch.count);
                    }
                    BatchKind::Line => {
                        if current != Some(BatchKind::Line) {
                            self.line.bind(&mut pass);
                            current = Some(BatchKind::Line);
                        }
                        self.line.draw_range(&mut pass, batch.start, batch.count);
                    }
                    BatchKind::Triangle => {
                        if current != Some(BatchKind::Triangle) {
                            self.path.bind(&mut pass);
                            current = Some(BatchKind::Triangle);
                        }
                        self.path.draw_range(&mut pass, batch.start, batch.count);
                    }
                    BatchKind::Glyph => {
                        if current != Some(BatchKind::Glyph) {
                            self.glyph.bind(&mut pass, self.glyph_atlas.bind_group());
                            current = Some(BatchKind::Glyph);
                        }
                        self.glyph.draw_range(&mut pass, batch.start, batch.count);
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_is_plain_copy_data() {
        let v = Viewport { width: 100, height: 200 };
        let v2 = v;
        assert_eq!(v, v2);
    }
}
