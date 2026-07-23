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
use crate::encode::{self, BatchKind, MaskOp};
use crate::msaa::MsaaTarget;
use crate::native_error::NativeRenderError;
use crate::pipelines::glyph::GlyphPipeline;
use crate::pipelines::line::LinePipeline;
use crate::pipelines::path::PathPipeline;
use crate::pipelines::quad::QuadPipeline;
use crate::pipelines::stencil_mask::StencilMaskPipeline;
use crate::stencil::StencilTarget;
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
/// triangle pipeline (Commit 3), the Glyph pipeline + its
/// `NativeGlyphAtlas` (Wave 2 Commit 2), and the stencil mask-write
/// pipeline + `StencilTarget` (Wave 3 Commit 2) — the shared uniform
/// bind group (group 0, `screen_size`), the MSAA offscreen color
/// target (exact-match reallocation on resize, see `msaa.rs`'s module
/// doc), the tessellation cache, and the glyph atlas.
pub struct NativeUrxRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    sample_count: u32,

    quad: QuadPipeline,
    line: LinePipeline,
    path: PathPipeline,
    glyph: GlyphPipeline,
    stencil_mask: StencilMaskPipeline,

    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    msaa: MsaaTarget,
    stencil: StencilTarget,
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
        let stencil_mask = StencilMaskPipeline::new(&device, format, sample_count, &uniform_bgl);
        let msaa = MsaaTarget::new(sample_count, format);
        // Sample count MUST match whichever color target this same
        // pass binds (`msaa`'s at `sample_count > 1`, or `view` itself
        // directly at `sample_count == 1`) — both are already built
        // from the SAME `sample_count`, so this is consistent by
        // construction (design §2.1). No texture is allocated here —
        // `ensure` is only ever called when a frame is actually armed
        // (design §2.5).
        let stencil = StencilTarget::new(sample_count);
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
            stencil_mask,
            uniform_buffer,
            uniform_bind_group,
            msaa,
            stencil,
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
    ///
    /// **Wave 3 Commit 2 — stencil arming (design §2.5)**: this is
    /// still a SINGLE-pass function (the multi-pass executor for blend
    /// layers is Commit 3) — the only new decision is whether THIS ONE
    /// pass carries a `depth_stencil_attachment` at all, decided ONCE
    /// from `frame.has_rounded_clip` before the pass opens. `false`
    /// (no scene this crate's own fixtures use today) means no stencil
    /// texture is even allocated and every pipeline uses its `_off`
    /// variant — byte-identical cost/behaviour to pre-Wave-3.
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
        self.stencil_mask.upload(&self.device, &self.queue, &frame.stencil_masks);
        // Drain every glyph bitmap queued by this frame's `encode_scene`
        // call into the atlas texture — after encode returns (every
        // glyph this frame has already been placed) and before the
        // render pass begins (so it samples up-to-date contents).
        self.glyph_atlas.flush_uploads(&self.queue);

        // Frame-wide arming decision (design §2.5) — made ONCE, before
        // the pass opens. `has_rounded_clip == false` (the overwhelming
        // common case): no stencil texture is even allocated.
        if frame.has_rounded_clip {
            self.stencil.ensure(&self.device, viewport.width, viewport.height);
        }
        let depth_stencil_attachment = frame.has_rounded_clip.then(|| wgpu::RenderPassDepthStencilAttachment {
            view: self.stencil.view(),
            depth_ops: None,
            // `Clear(0)` every frame this pass is armed — a fresh,
            // all-zero buffer is the base case the increment/decrement-
            // with-Equal-gate protocol's correctness proof assumes
            // (design §2.4).
            stencil_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0), store: wgpu::StoreOp::Discard }),
        });

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
            let mut pass = open_pass(
                encoder,
                color_view,
                resolve_target,
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store_op,
                depth_stencil_attachment,
            );

            pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Replay batches in scan order — this is what preserves
            // painter's order across a Quad/Line/.../StencilMask
            // interleave (design §5; legacy `renderer.rs:1008-1061`
            // pattern). `current` now tracks `(kind, stencil_ref)`, not
            // just `kind` (Wave 3 Commit 2) — two adjacent batches of
            // the SAME kind but DIFFERENT depth are never coalesced at
            // encode time (`bump_batch`'s extended rule), so comparing
            // only `kind` here would incorrectly skip the
            // `set_stencil_reference` call a depth CHANGE needs even
            // when the pipeline itself doesn't need re-binding.
            //
            // `effective_ref` is the design item 4 / wgpu-pass-
            // compatibility resolution: mixing `_off` (`depth_stencil:
            // None`) and `_test` (`depth_stencil: Some(_)`) pipelines
            // within ONE pass that HAS a stencil attachment is a wgpu
            // validation error (`RenderPassCompatibilityError::IncompatibleDepthStencilAttachment`,
            // confirmed by reading `wgpu-core-29.0.3/src/device/mod.rs`'s
            // `RenderPassContext::check_compatible`) — so the `_off`-
            // vs-`_test` choice is made ONCE for the WHOLE frame here
            // (`frame.has_rounded_clip`), never per-batch. When armed,
            // every draw batch's `None` (depth 0) is defaulted to
            // `Some(0)`, which correctly passes `Equal(0)` everywhere
            // the stencil buffer is still at its fresh-`Clear`ed value.
            let effective_ref = |raw: Option<u32>| -> Option<u32> {
                if frame.has_rounded_clip { Some(raw.unwrap_or(0)) } else { None }
            };

            let mut current: Option<(BatchKind, Option<u32>)> = None;
            for batch in &frame.batches {
                if batch.count == 0 {
                    continue;
                }
                match batch.kind {
                    BatchKind::Quad => {
                        let sr = effective_ref(batch.stencil_ref);
                        if current != Some((batch.kind, sr)) {
                            self.quad.bind(&mut pass, sr);
                            current = Some((batch.kind, sr));
                        }
                        self.quad.draw_range(&mut pass, batch.start, batch.count);
                    }
                    BatchKind::Line => {
                        let sr = effective_ref(batch.stencil_ref);
                        if current != Some((batch.kind, sr)) {
                            self.line.bind(&mut pass, sr);
                            current = Some((batch.kind, sr));
                        }
                        self.line.draw_range(&mut pass, batch.start, batch.count);
                    }
                    BatchKind::Triangle => {
                        let sr = effective_ref(batch.stencil_ref);
                        if current != Some((batch.kind, sr)) {
                            self.path.bind(&mut pass, sr);
                            current = Some((batch.kind, sr));
                        }
                        self.path.draw_range(&mut pass, batch.start, batch.count);
                    }
                    BatchKind::Glyph => {
                        let sr = effective_ref(batch.stencil_ref);
                        if current != Some((batch.kind, sr)) {
                            self.glyph.bind(&mut pass, sr, self.glyph_atlas.bind_group());
                            current = Some((batch.kind, sr));
                        }
                        self.glyph.draw_range(&mut pass, batch.start, batch.count);
                    }
                    BatchKind::StencilMask(op) => {
                        // Mask batches always carry `Some(gate)` by
                        // construction (design §2.3/§2.4 — a mask write
                        // never has a "no stencil" state); `unwrap_or(0)`
                        // is a defensive fallback, never reachable on
                        // well-formed scene content, not a panic either
                        // way.
                        let gate = batch.stencil_ref.unwrap_or(0);
                        if current != Some((batch.kind, batch.stencil_ref)) {
                            match op {
                                MaskOp::Increment => self.stencil_mask.bind_increment(&mut pass, gate),
                                MaskOp::Decrement => self.stencil_mask.bind_decrement(&mut pass, gate),
                            }
                            current = Some((batch.kind, batch.stencil_ref));
                        }
                        self.stencil_mask.draw_range(&mut pass, batch.start, batch.count);
                    }
                }
            }
        }

        Ok(())
    }
}

/// Open one render pass — extracted (design §2.2/item 6) so Wave 3
/// Commit 3's multi-pass blend-layer executor can reuse the EXACT same
/// pass-opening shape for every layer push/pop, not just the single
/// root pass this commit still uses. `depth_stencil_attachment` is a
/// parameter (not baked in) for exactly that reason — a future layer
/// pass may or may not be armed independently of the root pass (design
/// §3.4's Risk 4 resolution: a freshly-opened layer replays the active
/// `ClipStack`'s cached masks into its OWN stencil sibling).
fn open_pass<'enc>(
    encoder: &'enc mut wgpu::CommandEncoder,
    color_view: &wgpu::TextureView,
    resolve_target: Option<&wgpu::TextureView>,
    load_op: wgpu::LoadOp<wgpu::Color>,
    store_op: wgpu::StoreOp,
    depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'_>>,
) -> wgpu::RenderPass<'enc> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("uzor_urx_wgpu.native_pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target,
            depth_slice: None,
            ops: wgpu::Operations { load: load_op, store: store_op },
        })],
        depth_stencil_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
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

    // ── Wave 3 Commit 2: standalone GPU sanity for real stencil clip ──
    //
    // Design §9 Commit 2's own gate: "a --ignored GPU test rendering
    // `rounded_clip_content_crosses_corner` standalone (pre-parity-
    // harness sanity)" — built INLINE here (the real
    // `tests/fixtures.rs`/`tests/parity.rs` additions are Commit 4).
    // This is the FIRST real-hardware proof that the item-7 wgpu-pass-
    // compatibility resolution (armed frames always bind `_test`,
    // `None` defaulted to `Equal(0)`) actually produces correct pixels,
    // not just a passing borrow-checker/validation-layer check — every
    // OTHER GPU test in this crate only exercises the UNARMED
    // (`has_rounded_clip == false`) path.

    /// Headless wgpu device — same shape as `atlas.rs`/`encode.rs`'s
    /// own `test_device` helpers (mirrors `tests/common/mod.rs::init_device`).
    fn test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("uzor-urx-wgpu-renderer-test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }))
        .ok()
    }

    /// Read back a `RENDER_ATTACHMENT | COPY_SRC` texture as a tight
    /// (no row padding) `Vec<u8>` of RGBA8 bytes — same shape as
    /// `tests/common/mod.rs::readback_rgba`.
    fn readback_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
        let aligned_stride = (width * 4 + 255) & !255;
        let buf_size = (aligned_stride * height) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uzor-urx-wgpu-renderer-test-readback"),
            size: buf_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(aligned_stride), rows_per_image: Some(height) },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        queue.submit(Some(enc.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        rx.recv()
            .expect("map_async callback channel closed before firing")
            .expect("staging buffer map failed");
        let raw = slice.get_mapped_range();
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height as usize {
            let row_start = row * aligned_stride as usize;
            let row_end = row_start + (width * 4) as usize;
            out.extend_from_slice(&raw[row_start..row_end]);
        }
        drop(raw);
        staging.unmap();
        out
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn rounded_clip_corner_is_background_and_center_is_fill() {
        let Some((device, queue)) = test_device() else { return };
        const NATIVE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
        const SIZE: u32 = 256;

        let mut renderer = NativeUrxRenderer::new(device.clone(), queue.clone(), NATIVE_FORMAT);

        use uzor_urx_core::math::{Affine, Brush, Color, Rect, RoundedRect};
        use uzor_urx_core::scene::{DrawCommand, Scene};

        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, SIZE as f64, SIZE as f64),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(32, 32, 32, 255)),
            transform: Affine::IDENTITY,
        });
        // Design §6.1's `rounded_clip_content_crosses_corner` fixture —
        // a large radius so the bbox-vs-real-shape corner-cut area is
        // generous. Fills the WHOLE clip bbox: under a bbox-approx clip
        // every pixel here would show fill color; under a REAL rounded
        // clip, the corner regions must show background instead.
        scene.push(DrawCommand::PushClipRoundedRect {
            rect: RoundedRect::new(50.5, 50.5, 200.5, 200.5, 40.0),
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(50.5, 50.5, 200.5, 200.5),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(220, 60, 60, 255)),
            transform: Affine::IDENTITY,
        });
        scene.push(DrawCommand::PopClip);

        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor-urx-wgpu-rounded-clip-test-target"),
            size: wgpu::Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: NATIVE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer
            .render_into_encoder(&scene, &mut encoder, &view, Viewport { width: SIZE, height: SIZE })
            .expect("a well-formed rounded-clip scene must not error");
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba(&device, &queue, &target, SIZE, SIZE);
        let px = |x: u32, y: u32| -> [u8; 4] {
            let i = ((y * SIZE + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        };

        // (52, 52): deep in the top-left CORNER-CUT region (only ~1.5px
        // from the bbox corner (50.5,50.5), well inside the 40px-radius
        // cut) — MUST be background. A bbox-approx clip gets this exact
        // pixel WRONG (fill color) — this is the whole point of the
        // fixture (design §6.1).
        assert_eq!(
            px(52, 52),
            [32, 32, 32, 255],
            "corner-cut pixel must be background — a bbox-approx clip would incorrectly show fill color here"
        );
        // (125, 125): dead center — must be fill color under either
        // mechanism, a sanity probe.
        assert_eq!(px(125, 125), [220, 60, 60, 255], "center of the rounded clip must show the fill color");
    }
}
