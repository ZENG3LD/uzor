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
//!
//! ## Wave 3 Commit 3 — the multi-pass blend-layer executor
//!
//! `render_into_encoder` used to open exactly ONE render pass per frame
//! (Wave 1 through Wave 3 Commit 2). Blend layers (design §3,
//! `docs/uzor-engines/plans/urx-wave3-clip-blend-design-2026-07-25.md`)
//! need MULTIPLE passes — one per `PushLayer`/`PopLayer` bracket, each
//! rendering into its own offscreen target, composited back onto its
//! parent when popped. `replay_ops` (below) is that executor: it walks
//! `EncodedFrame::ops` (`encode::FrameOp` — `Draw`/`PushLayer`/`PopLayer`)
//! and opens/closes/reopens passes exactly per design §3.5's pause/
//! resume/close table.
//!
//! **Implementation note (a disclosed refinement of §3.5's literal
//! phrasing, not a behavioural deviation):** a `wgpu::RenderPassDescriptor`
//! fixes its `store`/`resolve_target` at `begin_render_pass` time, but
//! whether a given pass-open's eventual close will be a PAUSE (a nested
//! `PushLayer` comes next) or the FINAL close (this depth's own
//! `PopLayer`, or end-of-stream for root) depends on what op comes
//! *after* it — not knowable purely from the transition that triggered
//! the open. Since `EncodedFrame::ops` is a fully materialized `&[FrameOp]`
//! (not a lazy stream), this executor resolves that ambiguity with a
//! plain forward index scan (`peek_close_kind`) from the position a
//! stint's content starts, skipping `Draw` entries until the next
//! marker — the FIRST marker found (`PushLayer` → pause, `PopLayer` or
//! end-of-stream → final) is exactly the answer, no true "lookahead
//! budget" or separate pre-pass required. This keeps every open/close
//! decision local and single-scan while still producing the load/store/
//! resolve sequence design §3.5's table specifies literally.
//!
//! For a scene with ZERO blend layers (every Wave 1/2/3-Commit-1/2
//! fixture), `peek_close_kind` immediately finds no marker at all and
//! returns `CloseKind::Final` for the very first (root) stint — the
//! executor then opens exactly one pass, replays every batch, and does
//! the final close/resolve, byte-for-byte the same sequence Commit 2's
//! single-pass code already produced. This is the parity gate's load-
//! bearing claim (design §9 Commit 3): the executor is a true no-op
//! refactor of the single-pass case.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use uzor_urx_core::config::UrxConfig;

use crate::atlas::{AtlasStats, NativeGlyphAtlas};
use crate::encode::{self, BatchKind, EncodedFrame, FrameOp, MaskOp};
use crate::gradient_lut::GradientLutAtlas;
use crate::image_cache::NativeImageCache;
use crate::msaa::MsaaTarget;
use crate::native_error::NativeRenderError;
use crate::pipelines::blend_composite::BlendCompositePipeline;
use crate::pipelines::glyph::GlyphPipeline;
use crate::pipelines::gradient::GradientPipeline;
use crate::pipelines::image::ImagePipeline;
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

/// One blend-layer nesting depth's offscreen target (design §3.4) —
/// lazily built the first time a `PushLayer` at that depth is
/// processed, then REUSED across frames at a stable viewport size
/// (exact-match reallocation on any size change, same discipline as
/// `MsaaTarget`/`StencilTarget`). Every layer covers the full viewport
/// (design §0.2 — no bounds-derived sub-rect), so one target PER DEPTH
/// (not per Push/Pop occurrence) suffices: sibling layers pushed and
/// popped in sequence at the same depth safely reuse the same GPU
/// resources — they are never open simultaneously.
struct BlendLayerTarget {
    /// `None` when `sample_count == 1` — this layer then renders
    /// directly into `resolve_view` (design's `sample_count == 1` path:
    /// "layers render directly into their resolve textures, no resolve
    /// steps").
    msaa_view: Option<wgpu::TextureView>,
    resolve_view: wgpu::TextureView,
    resolve_bind_group: wgpu::BindGroup,
    /// This layer's OWN stencil sibling (design §3.4 Risk 4) — only
    /// ever actually allocated (via `StencilTarget::ensure`) when a
    /// frame is armed; an unarmed frame's layer never pays for one.
    stencil: StencilTarget,
    width: u32,
    height: u32,
}

impl BlendLayerTarget {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        resolve_bgl: &wgpu::BindGroupLayout,
        resolve_sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Self {
        // Texture handles are dropped as soon as their view is built —
        // the view holds the GPU resource alive internally (same idiom
        // `msaa.rs`/`stencil.rs` already use: neither stores a `Texture`
        // field, only the derived `TextureView`); nothing here ever
        // needs to `write_texture`/`copy_texture_to_buffer` against the
        // raw `Texture` handle directly, unlike `atlas.rs`.
        let resolve_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor_urx_wgpu.native_blend_layer_resolve"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let resolve_view = resolve_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let resolve_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uzor_urx_wgpu.native_blend_layer_resolve_bg"),
            layout: resolve_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&resolve_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(resolve_sampler) },
            ],
        });
        let msaa_view = (sample_count > 1).then(|| {
            let msaa_tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("uzor_urx_wgpu.native_blend_layer_msaa"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            msaa_tex.create_view(&wgpu::TextureViewDescriptor::default())
        });

        Self { msaa_view, resolve_view, resolve_bind_group, stencil: StencilTarget::new(sample_count), width, height }
    }

    /// The view actually bound as this layer's color attachment: the
    /// MSAA view when multisampling, else `resolve_view` directly.
    fn color_view(&self, sample_count: u32) -> &wgpu::TextureView {
        if sample_count > 1 {
            self.msaa_view.as_ref().expect("BlendLayerTarget built with sample_count > 1 must have an msaa_view")
        } else {
            &self.resolve_view
        }
    }
}

/// Depth-indexed pool of `BlendLayerTarget`s (design §3.4) — index
/// `depth - 1` holds nesting depth `depth`'s target (`depth` is
/// 1-indexed, matching `FrameOp::PushLayer::depth`). Grows on demand,
/// never shrinks — a scene that nests 5 layers deep once keeps those 5
/// targets allocated for the renderer's lifetime, the same amortizing
/// trade-off `TessCache`/`NativeGlyphAtlas` already make elsewhere in
/// this crate.
struct BlendLayerPool {
    targets: Vec<Option<BlendLayerTarget>>,
}

impl BlendLayerPool {
    fn new() -> Self {
        Self { targets: Vec::new() }
    }

    /// Ensure `depth`'s target exists and is sized EXACTLY `width x
    /// height` (reallocates on any size change — same discipline as
    /// `MsaaTarget`/`StencilTarget`); also ensures its stencil sibling
    /// when `armed` (design §3.4 Risk 4). Called once per First-open of
    /// `depth` — cheap/no-op on every repeat call at a stable size.
    fn ensure(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        sample_count: u32,
        resolve_bgl: &wgpu::BindGroupLayout,
        resolve_sampler: &wgpu::Sampler,
        depth: u32,
        width: u32,
        height: u32,
        armed: bool,
    ) {
        let idx = (depth - 1) as usize;
        if self.targets.len() <= idx {
            self.targets.resize_with(idx + 1, || None);
        }
        let needs_new = match &self.targets[idx] {
            Some(t) => t.width != width || t.height != height,
            None => true,
        };
        if needs_new {
            self.targets[idx] =
                Some(BlendLayerTarget::new(device, format, sample_count, resolve_bgl, resolve_sampler, width, height));
        }
        if armed {
            self.targets[idx]
                .as_mut()
                .expect("just ensured above")
                .stencil
                .ensure(device, width, height);
        }
    }

    /// Borrow `depth`'s target — only ever called for a depth the
    /// executor already `ensure`'d at its matching `PushLayer` (a
    /// `PopLayer`/composite can only reference a depth its own push
    /// already built).
    fn get(&self, depth: u32) -> &BlendLayerTarget {
        self.targets[(depth - 1) as usize]
            .as_ref()
            .expect("BlendLayerPool::get called for a depth its matching PushLayer never ensured")
    }
}

/// Owns every native-pipeline GPU resource: the Quad SDF pipeline
/// (Wave 1 Commit 1), the Line/capsule pipeline (Commit 2), the Path/
/// triangle pipeline (Commit 3), the Glyph pipeline + its
/// `NativeGlyphAtlas` (Wave 2 Commit 2), the stencil mask-write
/// pipeline + `StencilTarget` (Wave 3 Commit 2), and the blend-layer
/// composite pipeline + `BlendLayerPool` (Wave 3 Commit 3) — the shared
/// uniform bind group (group 0, `screen_size`), the MSAA offscreen
/// color target (exact-match reallocation on resize, see `msaa.rs`'s
/// module doc), the tessellation cache, and the glyph atlas.
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
    blend_composite: BlendCompositePipeline,
    gradient: GradientPipeline,
    image: ImagePipeline,

    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    /// Group-1 bind group layout + sampler every `BlendLayerTarget`'s
    /// `resolve_bind_group` is built from (design §3.6) — owned here
    /// (not per-target) since every layer at every depth shares the
    /// exact same layout/sampler, only the texture view differs.
    resolve_bgl: wgpu::BindGroupLayout,
    resolve_sampler: wgpu::Sampler,

    msaa: MsaaTarget,
    stencil: StencilTarget,
    layer_pool: BlendLayerPool,
    tess_cache: TessCache,
    glyph_atlas: NativeGlyphAtlas,
    gradient_lut: GradientLutAtlas,
    image_cache: NativeImageCache,
    blend_layer_max_depth: usize,
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
    /// `wgpu_glyph_atlas_w`/`_h`, `blend_layer_max_depth`) is baked into
    /// the owned resource/value it configures and is NOT hot-swappable
    /// for the lifetime of this renderer. Re-construct to pick up a
    /// changed config.
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

        // Group-1 layout/sampler shared by every `BlendLayerTarget`'s
        // resolve bind group (design §3.6) — texture + sampler, same
        // shape `atlas.rs`'s glyph-atlas bind group uses. `Nearest`
        // filtering (unlike the atlas's `Linear`): a composite quad
        // samples 1:1, viewport-pixel-for-viewport-pixel, against its
        // own viewport-sized resolve texture — there is never a scale
        // factor between sample and source pixel, so filtering mode is
        // inert; `Nearest` documents that intent (no blending across
        // texel boundaries is ever actually exercised).
        let resolve_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("uzor_urx_wgpu.native_blend_layer_resolve_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let resolve_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uzor_urx_wgpu.native_blend_layer_resolve_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blend_composite = BlendCompositePipeline::new(&device, format, sample_count, &uniform_bgl, &resolve_bgl);

        // Gradient LUT atlas + Radial/Sweep pipeline (Wave 4 Commit 3,
        // design §2.2/§2.3) — same "build the shared group-1 resource
        // BEFORE the pipeline that borrows its BindGroupLayout" order
        // the glyph atlas established above.
        let gradient_lut = GradientLutAtlas::new(&device, cfg.wgpu_gradient_lut_rows);
        let gradient = GradientPipeline::new(&device, format, sample_count, &uniform_bgl, gradient_lut.bind_group_layout());

        // Per-image texture cache + Image pipeline (Wave 4 Commit 3,
        // design §4.2/§4.3) — same ordering precedent.
        let image_cache = NativeImageCache::new(&device, cfg.wgpu_image_cache_cap);
        let image = ImagePipeline::new(&device, format, sample_count, &uniform_bgl, image_cache.bind_group_layout());

        let msaa = MsaaTarget::new(sample_count, format);
        // Sample count MUST match whichever color target this same
        // pass binds (`msaa`'s at `sample_count > 1`, or `view` itself
        // directly at `sample_count == 1`) — both are already built
        // from the SAME `sample_count`, so this is consistent by
        // construction (design §2.1). No texture is allocated here —
        // `ensure` is only ever called when a frame is actually armed
        // (design §2.5).
        let stencil = StencilTarget::new(sample_count);
        let layer_pool = BlendLayerPool::new();
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
            blend_composite,
            gradient,
            image,
            uniform_buffer,
            uniform_bind_group,
            resolve_bgl,
            resolve_sampler,
            msaa,
            stencil,
            layer_pool,
            tess_cache,
            glyph_atlas,
            gradient_lut,
            image_cache,
            blend_layer_max_depth: cfg.blend_layer_max_depth,
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

    /// Read-only gradient-LUT-atlas telemetry — hit/miss/eviction/entry
    /// counts (Wave 4 Commit 3, design §2.2/§10 Commit 2's own scope
    /// item). Same "3-line accessor, no reason to defer it" precedent
    /// as `glyph_atlas_stats` above. Not a hot-path cost: four
    /// `usize`/`u64` loads.
    pub fn gradient_lut_stats(&self) -> crate::gradient_lut::GradientLutAtlasStats {
        self.gradient_lut.stats()
    }

    /// Read-only image-cache telemetry — same shape/reasoning as
    /// [`Self::gradient_lut_stats`] above.
    pub fn image_cache_stats(&self) -> crate::image_cache::NativeImageCacheStats {
        self.image_cache.stats()
    }

    /// Walk `scene.commands`, encode into every native pipeline's
    /// instance buffer, and render into `view` — a single pass when
    /// the scene uses no blend layers (byte-identical to Wave 1/2/Wave
    /// 3 Commit 2), or the multi-pass executor (`replay_ops`) when it
    /// does (Wave 3 Commit 3, design §3.5). Never panics on scene
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

        // Advance the atlas's/LUT's per-frame tick BEFORE walking the
        // scene — every `get_or_insert` this frame stamps its slot/row
        // with the NEW tick, which is what makes the never-evict-this-
        // frame invariant work (`atlas.rs`'s module doc,
        // `gradient_lut.rs`'s own doc comment — Wave 4 Commit 3).
        self.glyph_atlas.begin_frame();
        self.gradient_lut.begin_frame();
        // `NativeImageCache`'s own `get_or_upload` isn't called until
        // `replay_ops` (below) — its never-evict-this-frame invariant
        // still needs the tick advanced exactly once per frame, before
        // any of those calls happen.
        self.image_cache.begin_frame();
        let frame = encode::encode_scene(
            scene,
            viewport,
            &mut self.tess_cache,
            Some(&mut self.glyph_atlas),
            Some(&mut self.gradient_lut),
            self.blend_layer_max_depth,
        );

        let uniforms = Uniforms { screen_size: [viewport.width as f32, viewport.height as f32], _pad: [0.0; 2] };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        self.quad.upload(&self.device, &self.queue, &frame.quads);
        self.line.upload(&self.device, &self.queue, &frame.lines);
        self.path.upload(&self.device, &self.queue, &frame.triangles);
        self.glyph.upload(&self.device, &self.queue, &frame.glyphs);
        self.stencil_mask.upload(&self.device, &self.queue, &frame.stencil_masks);
        self.blend_composite.upload(&self.device, &self.queue, &frame.composites);
        self.gradient.upload(&self.device, &self.queue, &frame.gradients);
        self.image.upload(&self.device, &self.queue, &frame.images);
        // Drain every glyph bitmap queued by this frame's `encode_scene`
        // call into the atlas texture — after encode returns (every
        // glyph this frame has already been placed) and before any
        // pass opens (so it samples up-to-date contents).
        self.glyph_atlas.flush_uploads(&self.queue);
        // Same timing for the gradient LUT atlas's queued rows (Wave 4
        // Commit 3) — `GradientLutAtlas::flush_uploads` is the SAME
        // "queue bytes at encode time, drain once here" shape as the
        // glyph atlas (see that module's own doc comment for why
        // `NativeImageCache` does NOT need an equivalent flush step:
        // its uploads are synchronous, resolved lazily at replay time
        // instead of during `encode_scene`).
        self.gradient_lut.flush_uploads(&self.queue);

        // Frame-wide arming decision (design §2.5) — made ONCE, before
        // any pass opens. `has_rounded_clip == false` (the overwhelming
        // common case): no stencil texture is even allocated (neither
        // root's nor any layer's).
        if frame.has_rounded_clip {
            self.stencil.ensure(&self.device, viewport.width, viewport.height);
        }

        self.replay_ops(&frame, encoder, view, viewport);

        Ok(())
    }

    /// The multi-pass executor (Wave 3 Commit 3, design §3.5) — see
    /// this module's doc comment for the `peek_close_kind` resolution
    /// of the "store is fixed at open time" wrinkle. Processes
    /// `frame.ops` left to right exactly once; opens/closes exactly one
    /// `wgpu::RenderPass` at a time (never two simultaneously — a
    /// `RenderPass` mutably borrows `encoder`, so this is required
    /// regardless of the design), resetting the pipeline-dedup tracker
    /// (`current`) on every new pass (pipeline/bind-group state does
    /// NOT persist across `begin_render_pass` calls).
    fn replay_ops(
        &mut self,
        frame: &EncodedFrame,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: Viewport,
    ) {
        let armed = frame.has_rounded_clip;
        let sample_count = self.sample_count;
        let ops = &frame.ops;

        // `effective_ref` is the design item 4 / wgpu-pass-compatibility
        // resolution carried over from Commit 2 (see `encode.rs`'s
        // `Batch::stencil_ref` doc comment for the full write-up): the
        // `_off`-vs-`_test` pipeline choice is a FRAME-WIDE decision
        // (`armed`), never per-batch: every draw's `None` (depth 0) is
        // defaulted to `Some(0)` whenever the frame is armed.
        let effective_ref = |raw: Option<u32>| -> Option<u32> { if armed { Some(raw.unwrap_or(0)) } else { None } };

        let mut idx = 0usize;
        let mut current_depth = 0u32;
        let mut open_kind = OpenKind::First;
        let mut composite_idx = 0u32;
        // Set by a `PopLayer` transition — the very first thing drawn
        // into the freshly reopened PARENT pass must be the composite
        // quad (design §3.5: "reopen parent pass ... draw composite
        // quad"), before any of the parent's own subsequent `Draw` ops.
        // ALWAYS consumed within the very next loop iteration (the one
        // that opens the parent's resumed pass) — never carries state
        // across more than one iteration.
        let mut pending_composite: Option<(u32, Option<u32>)> = None;

        loop {
            let close_kind = peek_close_kind(ops, idx);

            if current_depth > 0 {
                self.layer_pool.ensure(
                    &self.device,
                    self.format,
                    sample_count,
                    &self.resolve_bgl,
                    &self.resolve_sampler,
                    current_depth,
                    viewport.width,
                    viewport.height,
                    armed,
                );
            }

            let depth_stencil_attachment = armed.then(|| {
                stencil_attachment_for(current_depth, open_kind, close_kind, &self.stencil, &self.layer_pool)
            });
            let (color_view, resolve_target) =
                color_attachment_for(current_depth, close_kind, sample_count, &self.msaa, &self.layer_pool, view);
            let load_op = color_load_op(open_kind);
            let store_op = color_store_op(close_kind, sample_count);

            {
                let mut pass =
                    open_pass(encoder, color_view, resolve_target, load_op, store_op, depth_stencil_attachment);
                pass.set_bind_group(0, &self.uniform_bind_group, &[]);

                if let Some((popped_depth, sr)) = pending_composite.take() {
                    let resolve_bg = &self.layer_pool.get(popped_depth).resolve_bind_group;
                    self.blend_composite.bind(&mut pass, sr, resolve_bg);
                    self.blend_composite.draw_one(&mut pass, composite_idx);
                    composite_idx += 1;
                }

                let mut current: Option<(BatchKind, Option<u32>)> = None;
                while idx < ops.len() {
                    let FrameOp::Draw(batch) = &ops[idx] else { break };
                    idx += 1;
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
                            // construction; `unwrap_or(0)` is a
                            // defensive fallback, never reachable on
                            // well-formed scene content.
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
                        BatchKind::Gradient => {
                            // Same shared-group-1-resource shape as
                            // Glyph (Wave 4 Commit 3, design §2.3) — the
                            // SAME `GradientLutAtlas` bind group is
                            // rebound across every `Gradient` batch,
                            // never per-instance data.
                            let sr = effective_ref(batch.stencil_ref);
                            if current != Some((batch.kind, sr)) {
                                self.gradient.bind(&mut pass, sr, self.gradient_lut.bind_group());
                                current = Some((batch.kind, sr));
                            }
                            self.gradient.draw_range(&mut pass, batch.start, batch.count);
                        }
                        BatchKind::Image(id) => {
                            // A DIFFERENT `ImageId` never coalesces with
                            // another (design §4.3 — bind group per
                            // image); `current`'s equality check already
                            // compares the WRAPPED id (`BatchKind`
                            // derives `PartialEq`), so it naturally
                            // re-binds on every id change with no extra
                            // logic here. Resolved (and, on a genuine
                            // miss, uploaded) HERE, at replay time — see
                            // `image_cache.rs`'s own module doc for why
                            // that's correct for images specifically
                            // (unlike the atlas/LUT, an image slot is
                            // written at most once in its whole resident
                            // lifetime, so there's no encode-time
                            // pending-queue to batch away).
                            let sr = effective_ref(batch.stencil_ref);
                            let Some(slot) = self.image_cache.get_or_upload(&self.device, &self.queue, id) else {
                                // Honest miss (unregistered id) or
                                // within-frame cache oversubscription
                                // (never-evict-this-frame exhausted) —
                                // this cache emits no metrics of its own
                                // (same convention as the atlas/LUT);
                                // THIS is the one call site that can
                                // observe a replay-time miss (an
                                // encode-time `image_id_unknown` miss
                                // never reaches here at all — no
                                // `BatchKind::Image` batch was ever
                                // emitted for it), so it's counted here.
                                metrics::counter!(
                                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                                    "kind" => "native_image_cache_full_this_frame"
                                )
                                .increment(1);
                                current = None;
                                continue;
                            };
                            if current != Some((batch.kind, sr)) {
                                self.image.bind(&mut pass, sr, NativeImageCache::bind_group_of(slot));
                                current = Some((batch.kind, sr));
                            }
                            self.image.draw_range(&mut pass, batch.start, batch.count);
                        }
                    }
                }
            } // `pass` dropped here — applies (load_op, store_op, resolve_target) chosen above.

            if idx >= ops.len() {
                debug_assert_eq!(
                    current_depth, 0,
                    "the op stream must always end back at the root — encode.rs's end-of-scene force-close \
                     guarantees every PushLayer has a matching (real or synthetic) PopLayer"
                );
                break;
            }

            match &ops[idx] {
                FrameOp::PushLayer { depth } => {
                    current_depth = *depth;
                    open_kind = OpenKind::First;
                    idx += 1;
                }
                FrameOp::PopLayer { depth, stencil_ref, .. } => {
                    let parent_depth = depth - 1;
                    pending_composite = Some((*depth, effective_ref(*stencil_ref)));
                    current_depth = parent_depth;
                    open_kind = OpenKind::Resume;
                    idx += 1;
                }
                FrameOp::Draw(_) => unreachable!(
                    "the draw-replay loop above only breaks on a non-Draw op or end-of-ops; a Draw here would mean \
                     it broke early without consuming it"
                ),
            }
        }
    }
}

/// Whether a pass is being opened for the FIRST time at its depth
/// (`Clear`) or being RESUMED after a pause (`Load`) — design §3.5.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenKind {
    First,
    Resume,
}

/// Whether the pass about to be opened will be closed as a PAUSE
/// (`Store`, no resolve — something deeper is about to open) or as the
/// FINAL close for its depth (`Discard` + resolve at `sample_count >
/// 1`, or a plain `Store` at `sample_count == 1` — design §3.5).
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloseKind {
    Pause,
    Final,
}

/// Resolve `close_kind` for the stint about to open at `ops[start..]`
/// — see this module's doc comment for why this is a plain forward
/// scan, not real lookahead. Skips `Draw` entries; the first marker
/// found (`PushLayer` → `Pause`, `PopLayer` → `Final`) is the answer.
/// Running off the end of `ops` (no marker at all) also means `Final`
/// — only reachable for the root, guaranteed by encode.rs's end-of-
/// scene force-close (every `PushLayer` gets a matching `PopLayer`).
fn peek_close_kind(ops: &[FrameOp], mut i: usize) -> CloseKind {
    while i < ops.len() {
        match &ops[i] {
            FrameOp::Draw(_) => i += 1,
            FrameOp::PushLayer { .. } => return CloseKind::Pause,
            FrameOp::PopLayer { .. } => return CloseKind::Final,
        }
    }
    CloseKind::Final
}

fn color_load_op(open_kind: OpenKind) -> wgpu::LoadOp<wgpu::Color> {
    match open_kind {
        OpenKind::First => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        OpenKind::Resume => wgpu::LoadOp::Load,
    }
}

/// `sample_count > 1`: `Store` while paused (the MSAA content must
/// survive to be resumed), `Discard` at the final close (the resolve
/// step already captured what's needed into the single-sample resolve
/// target, so the multisampled buffer itself is disposable). `sample_count
/// == 1`: always `Store` — there is no MSAA buffer to discard, the
/// color view IS the final destination (caller's `view` for root, a
/// layer's own resolve texture for a layer) and must persist either
/// way (design's "no resolve steps" `sample_count == 1` path).
fn color_store_op(close_kind: CloseKind, sample_count: u32) -> wgpu::StoreOp {
    match (close_kind, sample_count > 1) {
        (CloseKind::Final, true) => wgpu::StoreOp::Discard,
        (CloseKind::Final, false) => wgpu::StoreOp::Store,
        (CloseKind::Pause, _) => wgpu::StoreOp::Store,
    }
}

/// Resolve the color attachment view + (only at `CloseKind::Final`)
/// resolve target for `depth`'s about-to-open pass (design §3.5).
/// `depth == 0` is the root (`msaa`/`caller_view`); `depth > 0` reads
/// from `layer_pool` (already `ensure`'d by the caller before this is
/// invoked).
fn color_attachment_for<'a>(
    depth: u32,
    close_kind: CloseKind,
    sample_count: u32,
    msaa: &'a MsaaTarget,
    layer_pool: &'a BlendLayerPool,
    caller_view: &'a wgpu::TextureView,
) -> (&'a wgpu::TextureView, Option<&'a wgpu::TextureView>) {
    if depth == 0 {
        if sample_count > 1 {
            let resolve = matches!(close_kind, CloseKind::Final).then_some(caller_view);
            (msaa.color_view(), resolve)
        } else {
            (caller_view, None)
        }
    } else {
        let target = layer_pool.get(depth);
        let color_view = target.color_view(sample_count);
        if sample_count > 1 {
            let resolve = matches!(close_kind, CloseKind::Final).then_some(&target.resolve_view);
            (color_view, resolve)
        } else {
            (color_view, None)
        }
    }
}

/// Resolve the stencil attachment for `depth`'s about-to-open pass —
/// only ever called when the frame is armed (`EncodedFrame::has_rounded_clip`).
/// `load`/`store` mirror the color attachment's own open/close kind
/// exactly (design §3.5's MSAA subtlety applies identically to the
/// stencil buffer: a paused layer's accumulated clip-nesting counts
/// must survive to be resumed, so `Pause` stores rather than discards).
fn stencil_attachment_for<'a>(
    depth: u32,
    open_kind: OpenKind,
    close_kind: CloseKind,
    root_stencil: &'a StencilTarget,
    layer_pool: &'a BlendLayerPool,
) -> wgpu::RenderPassDepthStencilAttachment<'a> {
    let view = if depth == 0 { root_stencil.view() } else { layer_pool.get(depth).stencil.view() };
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: None,
        stencil_ops: Some(wgpu::Operations {
            load: if open_kind == OpenKind::First { wgpu::LoadOp::Clear(0) } else { wgpu::LoadOp::Load },
            store: if close_kind == CloseKind::Final { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
        }),
    }
}

/// Open one render pass — the single shared shape every stint in
/// `NativeUrxRenderer::replay_ops` uses, whether it's the degenerate
/// single-pass (no blend layers) case or one stint of a deeply nested
/// multi-pass frame.
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

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn gradient_lut_and_image_cache_stats_default_to_zero() {
        let Some((device, queue)) = test_device() else { return };
        let renderer = NativeUrxRenderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm);
        let g = renderer.gradient_lut_stats();
        assert_eq!(g.entries, 0);
        assert_eq!(g.hits, 0);
        assert_eq!(g.misses, 0);
        assert_eq!(g.evictions, 0);
        let i = renderer.image_cache_stats();
        assert_eq!(i.entries, 0);
        assert_eq!(i.hits, 0);
        assert_eq!(i.misses, 0);
        assert_eq!(i.evictions, 0);
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

    fn make_target(device: &wgpu::Device, format: wgpu::TextureFormat, size: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("uzor-urx-wgpu-renderer-test-target"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        (target, view)
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

        let (target, view) = make_target(&device, NATIVE_FORMAT, SIZE);
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

    // ── Wave 4 Commit 4: rotated-rect GPU-only correctness probe ─────
    //
    // CPU cannot render a genuinely rotated rect at all (design §0.3 —
    // `fill_rect_aa` snaps to the axis-aligned bbox of the transformed
    // corners), so this has no CPU parity counterpart; it directly
    // proves the Quad-SDF rotation math (`shaders.rs::QUAD_SHADER_NATIVE`)
    // is right by hand-computing which screen points an 80x80 square,
    // rotated 45 degrees about its own center, must and must not cover,
    // then reading back real rendered pixels.
    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn rotated_uniform_radius_rect_corner_regions_render_correctly() {
        let Some((device, queue)) = test_device() else { return };
        const NATIVE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
        const SIZE: u32 = 256;

        // sample_count = 1 — exact, filter-free readback (see the
        // blend-layer test's own doc comment for why): every probe
        // below is chosen with a solid multi-pixel margin from the true
        // edge, so MSAA wouldn't actually matter here, but an exact
        // readback removes any doubt.
        let mut renderer = NativeUrxRenderer::with_sample_count(device.clone(), queue.clone(), NATIVE_FORMAT, 1);

        use uzor_urx_core::math::{Affine, Brush, Color, Rect};
        use uzor_urx_core::scene::{DrawCommand, Scene};

        let mut scene = Scene::new();
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, SIZE as f64, SIZE as f64),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(32, 32, 32, 255)),
            transform: Affine::IDENTITY,
        });
        // An 80x80 square centered at (128,128), rotated 45 degrees
        // about its OWN center (`rotate_about` — the center stays fixed).
        scene.push(DrawCommand::FillRect {
            rect: Rect::new(88.0, 88.0, 168.0, 168.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(220, 60, 60, 255)),
            transform: Affine::rotate_about(std::f64::consts::FRAC_PI_4, (128.0, 128.0)),
        });

        let (target, view) = make_target(&device, NATIVE_FORMAT, SIZE);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer
            .render_into_encoder(&scene, &mut encoder, &view, Viewport { width: SIZE, height: SIZE })
            .expect("a well-formed rotated-rect scene must not error");
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba(&device, &queue, &target, SIZE, SIZE);
        let px = |x: u32, y: u32| -> [u8; 4] {
            let i = ((y * SIZE + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        };

        assert_eq!(px(128, 128), [220, 60, 60, 255], "dead center must be fill regardless of rotation");

        // Screen offset (35,-35) from center (i.e. absolute (163,93)):
        // INSIDE the un-rotated 40-half-extent axis-aligned square, but
        // its inverse-rotated local coordinate is (0,-49.5) — OUTSIDE
        // the true 45-degree-rotated square by a solid ~9.5px margin.
        // A renderer that rotates the mesh but evaluates the SDF
        // against the RAW (un-rotated) screen offset — the exact bug
        // found and fixed in this design's own WGSL sketch, see
        // `shaders.rs::QUAD_SHADER_NATIVE`'s doc comment — would
        // incorrectly show FILL here.
        assert_eq!(
            px(163, 93),
            [32, 32, 32, 255],
            "must be background: inside the UN-rotated bbox but outside the TRUE rotated shape"
        );
        // Screen offset (50,0) from center (absolute (178,128)):
        // OUTSIDE the un-rotated 40-half-extent square along +x, but
        // its inverse-rotated local coordinate is (35.36,-35.36) —
        // INSIDE the true rotated square by a solid ~4.6px margin. The
        // same bug class as above would incorrectly show BACKGROUND
        // here (the opposite failure direction), since a naive
        // non-rotated SDF check rejects this point outright.
        assert_eq!(
            px(178, 128),
            [220, 60, 60, 255],
            "must be fill: outside the UN-rotated bbox but inside the TRUE rotated shape"
        );
    }

    // ── Wave 3 Commit 3: the multi-pass blend-layer executor ─────────
    //
    // Design risk 2's dedicated gate: a nested layer scene must not
    // panic/hit a wgpu validation error, AND the composited pixels must
    // be numerically correct — computed by hand below, not just
    // "doesn't crash."

    use uzor_urx_core::math::{Affine, Brush, Color, Rect};
    use uzor_urx_core::scene::{DrawCommand, Scene};

    fn push_layer(alpha: f32) -> DrawCommand {
        DrawCommand::PushBlendLayer { mode: uzor_urx_core::math::BlendMode::default(), alpha, transform: Affine::IDENTITY }
    }

    fn solid_rect(x: f64, y: f64, w: f64, h: f64, rgba: [u8; 4]) -> DrawCommand {
        DrawCommand::FillRect {
            rect: Rect::new(x, y, x + w, y + h),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3])),
            transform: Affine::IDENTITY,
        }
    }

    /// Blend a STRAIGHT-alpha `src` (0..255 per channel, un-premultiplied)
    /// over an OPAQUE `dst` using premultiplied src-over, scaled by an
    /// extra `layer_alpha` multiplier — the exact composite this
    /// executor's `BlendCompositePipeline` performs (§3.6: `texel *
    /// alpha`, where `texel` is `src` already premultiplied by its own
    /// alpha from having been rendered through the SAME premultiplied
    /// blend state onto a transparent-cleared layer target). Used here
    /// to hand-compute this test's expected pixels.
    fn blend_straight_over_opaque(src: [u8; 4], dst: [u8; 3], layer_alpha: f32) -> [u8; 3] {
        let sa = (src[3] as f32 / 255.0) * layer_alpha;
        let mut out = [0u8; 3];
        for c in 0..3 {
            let s = src[c] as f32 / 255.0;
            let d = dst[c] as f32 / 255.0;
            let v = s * sa + d * (1.0 - sa);
            out[c] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        out
    }

    /// Every blend stage below lands on an exact `x.5` half-integer
    /// boundary at least once (0.5 layer alphas, 8-bit stops) — the
    /// GPU's fixed-function blend hardware and this test's own
    /// hand-computed reference round such ties independently (round-
    /// half-to-even vs round-half-away-from-zero, or vice versa,
    /// depending on driver), and with 3 STACKED stages those ties can
    /// legitimately land 1 LSB apart after quantizing to 8 bits at
    /// every intermediate stage (exactly the composite pipeline's own
    /// real behaviour: each layer's content really is stored as 8-bit
    /// UNORM between stages, this is not a test-only approximation).
    /// A tight ±1-per-channel tolerance still catches any REAL
    /// composite-math bug (wrong operand order, missing alpha scale,
    /// double-premultiply, etc. all miss by far more than 1 LSB) while
    /// tolerating this legitimate hardware-rounding non-determinism —
    /// same "tolerance budget" doctrine this crate's pixel-parity
    /// harness already uses elsewhere (`encode.rs`'s module doc).
    fn assert_close_rgb(got: [u8; 3], expected: [u8; 3], tol: i32, msg: &str) {
        for c in 0..3 {
            let diff = (got[c] as i32 - expected[c] as i32).abs();
            assert!(diff <= tol, "{msg} — channel {c}: got {} expected {} (diff {diff} > tol {tol})", got[c], expected[c]);
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn three_level_nested_blend_layers_do_not_panic_and_composite_correct_pixels() {
        let Some((device, queue)) = test_device() else { return };
        const NATIVE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
        const SIZE: u32 = 64;

        // sample_count = 1 — an exact, filter-free readback makes the
        // hand-computed pixel comparison exact rather than approximate
        // (MSAA edge blending would otherwise blur the probe pixel,
        // which sits well inside every rect's interior here anyway, but
        // sample_count = 1 keeps the arithmetic exact all the way
        // through: the whole point of this test is verifying the
        // COMPOSITE math, not edge AA).
        let mut renderer = NativeUrxRenderer::with_sample_count(device.clone(), queue.clone(), NATIVE_FORMAT, 1);

        let mut scene = Scene::new();
        // Opaque black background covering the whole viewport.
        scene.push(solid_rect(0.0, 0.0, SIZE as f64, SIZE as f64, [0, 0, 0, 255]));
        // Layer 1 (alpha 0.5): a full-viewport red rect.
        scene.push(push_layer(0.5));
        scene.push(solid_rect(0.0, 0.0, SIZE as f64, SIZE as f64, [255, 0, 0, 255]));
        // Layer 2 (alpha 0.5), nested inside layer 1: a full-viewport
        // green rect — exercises PAUSE/RESUME of layer 1's OWN pass
        // (opened, paused for layer 2's push, resumed after layer 2's
        // pop, since more content — layer 3 below — follows inside
        // layer 1 afterward).
        scene.push(push_layer(0.5));
        scene.push(solid_rect(0.0, 0.0, SIZE as f64, SIZE as f64, [0, 255, 0, 255]));
        scene.push(DrawCommand::PopBlendLayer); // closes layer 2
        // More layer-1 content AFTER layer 2 pops — proves layer 1's
        // pass genuinely resumed with its prior content (the red fill)
        // intact rather than losing it: a small blue rect in one
        // corner only.
        scene.push(solid_rect(0.0, 0.0, 8.0, 8.0, [0, 0, 255, 255]));
        scene.push(DrawCommand::PopBlendLayer); // closes layer 1
        // Layer 3 (alpha 0.5), a SIBLING of layer 1 (not nested) —
        // proves `BlendLayerPool`'s depth-1 slot is safely REUSED after
        // layer 1 already closed (design §3.4: siblings at the same
        // depth are never open simultaneously).
        scene.push(push_layer(0.5));
        scene.push(solid_rect(0.0, 0.0, SIZE as f64, SIZE as f64, [255, 255, 0, 255]));
        scene.push(DrawCommand::PopBlendLayer);

        let (target, view) = make_target(&device, NATIVE_FORMAT, SIZE);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer
            .render_into_encoder(&scene, &mut encoder, &view, Viewport { width: SIZE, height: SIZE })
            .expect("a well-formed nested blend-layer scene must not panic or hit a wgpu validation error");
        queue.submit(Some(encoder.finish()));

        let pixels = readback_rgba(&device, &queue, &target, SIZE, SIZE);
        // Probe (32, 32) — dead center, untouched by the 8x8 blue
        // corner rect, so only the full-viewport rects matter here.
        let i = ((32 * SIZE + 32) * 4) as usize;
        let got = [pixels[i], pixels[i + 1], pixels[i + 2]];

        // Hand-computed expected value, composited bottom-up:
        // background (black) <- layer1{ red <- layer2{ green } } <- layer3{ yellow }.
        let bg = [0u8, 0, 0];
        let after_layer2 = blend_straight_over_opaque([0, 255, 0, 255], [255, 0, 0], 0.5); // green over red, layer2 alpha 0.5
        // Layer 1's OWN content at this pixel is `after_layer2` (fully
        // opaque red+green mix, alpha still 255 since both fills were
        // opaque) — layer 1 composites onto the background at its own
        // alpha (0.5).
        let after_layer1 = blend_straight_over_opaque([after_layer2[0], after_layer2[1], after_layer2[2], 255], bg, 0.5);
        let after_layer3 = blend_straight_over_opaque([255, 255, 0, 255], after_layer1, 0.5); // yellow over the layer1 result

        assert_close_rgb(
            got,
            after_layer3,
            1,
            "composited pixel must match the hand-computed 3-level nested blend result \
             (bg <- layer1{red <- layer2{green}} <- layer3{yellow})",
        );

        // (4, 4) sits inside the 8x8 blue corner rect drawn into layer
        // 1 AFTER layer 2 popped — proves layer 1's pass genuinely
        // resumed with its prior red content intact (if the resume had
        // wrongly `Clear`ed instead of `Load`ed, this pixel would show
        // pure blue-over-background instead of blue-over-red).
        let j = ((4 * SIZE + 4) * 4) as usize;
        let got_corner = [pixels[j], pixels[j + 1], pixels[j + 2]];
        let layer1_corner = blend_straight_over_opaque([0, 0, 255, 255], [255, 0, 0], 1.0); // blue drawn opaquely over the red already in layer 1
        let expected_corner = blend_straight_over_opaque([layer1_corner[0], layer1_corner[1], layer1_corner[2], 255], bg, 0.5);
        let expected_corner = blend_straight_over_opaque([255, 255, 0, 255], expected_corner, 0.5);
        assert_close_rgb(
            got_corner,
            expected_corner,
            1,
            "the resumed layer-1 pass must retain its pre-pause red content under the blue corner rect",
        );
    }

    /// Design §6.4's isolation-differs proof: a blend-layer group
    /// (content drawn INSIDE a layer, composited back at less than full
    /// alpha) must produce DIFFERENT pixels than the same content drawn
    /// with NO layer at all (straight onto the target) — proving the
    /// layer's own offscreen isolation is actually taking effect, not
    /// silently degrading into "draw directly."
    #[test]
    #[ignore = "needs a headless GPU adapter"]
    fn blend_layer_group_differs_from_the_same_content_with_no_layer() {
        let Some((device, queue)) = test_device() else { return };
        const NATIVE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
        const SIZE: u32 = 64;

        let render = |use_layer: bool| -> [u8; 3] {
            let mut renderer = NativeUrxRenderer::with_sample_count(device.clone(), queue.clone(), NATIVE_FORMAT, 1);
            let mut scene = Scene::new();
            scene.push(solid_rect(0.0, 0.0, SIZE as f64, SIZE as f64, [10, 20, 30, 255]));
            if use_layer {
                scene.push(push_layer(0.5));
            }
            scene.push(solid_rect(0.0, 0.0, SIZE as f64, SIZE as f64, [200, 100, 50, 255]));
            if use_layer {
                scene.push(DrawCommand::PopBlendLayer);
            }

            let (target, view) = make_target(&device, NATIVE_FORMAT, SIZE);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            renderer
                .render_into_encoder(&scene, &mut encoder, &view, Viewport { width: SIZE, height: SIZE })
                .expect("well-formed scene must not error");
            queue.submit(Some(encoder.finish()));
            let pixels = readback_rgba(&device, &queue, &target, SIZE, SIZE);
            let i = ((32 * SIZE + 32) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };

        let with_layer = render(true);
        let without_layer = render(false);
        assert_ne!(
            with_layer, without_layer,
            "a layer composited at alpha 0.5 must produce visibly different pixels than drawing the same \
             content directly with no layer at all — otherwise the offscreen isolation isn't actually happening"
        );
        // Sanity: `without_layer` is fully opaque top content (no
        // blending at all — the fill is opaque, drawn with no active
        // layer above it).
        assert_eq!(without_layer, [200, 100, 50]);
        // `with_layer` must be the 0.5-alpha composite of the SAME top
        // content over the background — proves it's REAL blending, not
        // e.g. an accidentally-fully-transparent or fully-opaque
        // degenerate result.
        let expected = blend_straight_over_opaque([200, 100, 50, 255], [10, 20, 30], 0.5);
        assert_eq!(with_layer, expected);
    }
}
