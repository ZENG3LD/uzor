//! GPU-side tile assignment + per-tile sort + fine raster, as the compute
//! dispatches in URX 1.6's full-GPU pipeline.
//!
//! ## Layout
//!
//! Screen is divided into 16×16 pixel tiles. For an 1920×1080 viewport
//! that's 120×68 = 8160 tiles. Each tile stores a fixed-capacity list
//! of cmd indices that touch it (default capacity = 64 per tile, capped
//! to avoid runaway memory).
//!
//! ## Buffers
//!
//! - `cmds`:        StorageBuffer<SceneCmd>     — input, read-only
//! - `tile_counts`: StorageBuffer<atomic<u32>>  — one u32 per tile, init 0
//! - `tile_lists`:  StorageBuffer<u32>          — tile_count × CAP slots
//! - `output_tex`:  StorageTexture<rgba8unorm>  — per-pixel RGBA output
//!
//! ## Coarse pass (v1.6.0 rect-only)
//!
//! No separate coarse.wgsl for rect-only v1.6.0. The sorted `tile_lists`
//! buffer IS the PTCL — fine.wgsl reads it directly. Add coarse.wgsl
//! when implementing gradient/glyph variants.
//!
//! ## Dispatch
//!
//! `tile_assign.wgsl`: workgroup size 64; one invocation per cmd. Each
//! invocation iterates the tiles its bbox overlaps and atomically
//! appends its index to those tiles' lists.
//!
//! `tile_sort.wgsl`: workgroup size 64; one invocation per tile. Sorts
//! that tile's list (small list, insertion sort) so painter-order is
//! preserved after the unordered atomic-append.
//!
//! `fine.wgsl`: workgroup size (16, 16, 1) = 256 per workgroup; one
//! workgroup per tile; one invocation per pixel. Walks the tile's sorted
//! cmd list, composites rect coverage, writes rgba8unorm output texture.
//!
//! ## Workgroup size note
//!
//! 16×16 = 256 invocations per workgroup — exactly at the wgpu default
//! limit (`Limits::max_compute_invocations_per_workgroup = 256`). This
//! is valid on all modern GPU tiers. If a device reports a lower limit
//! the pipeline creation will surface a wgpu validation error; in that
//! case fall back to 8×8 workgroups with each invocation covering a 2×2
//! pixel block.

use bytemuck::{Pod, Zeroable};

use crate::cmd::SceneCmd;

/// Tile size in pixels (16×16).
pub const TILE_SIZE: u32 = 16;
/// Maximum commands per tile slot.
pub const TILE_CMD_CAP: u32 = 64;

const TILE_ASSIGN_WGSL: &str = include_str!("shaders/tile_assign.wgsl");
const TILE_SORT_WGSL:   &str = include_str!("shaders/tile_sort.wgsl");
const FINE_WGSL:        &str = include_str!("shaders/fine.wgsl");
const BLIT_WGSL:        &str = include_str!("shaders/blit.wgsl");

/// Default capacity for the `path_points` storage buffer (16 384 vec2<f32>s
/// = 128 KB). Override with `allocate_with` when you need more.
pub const DEFAULT_PATH_POINTS_CAP: u32 = 16_384;

/// GPU buffers for the tile binning pipeline.
pub struct TileBuffers {
    pub cmds_buf:        wgpu::Buffer,
    pub tile_counts_buf: wgpu::Buffer,
    pub tile_lists_buf:  wgpu::Buffer,
    /// Path-point storage: array<vec2<f32>>, indexed by Path cmds via
    /// (point_offset, point_count). Allocated at `path_points_cap` vec2s.
    pub path_points_buf: wgpu::Buffer,
    pub path_points_cap: u32,
    pub tile_count_x:    u32,
    pub tile_count_y:    u32,
}

impl TileBuffers {
    /// Allocate all storage buffers for the given screen dimensions
    /// with `path_points_cap = DEFAULT_PATH_POINTS_CAP`.
    pub fn allocate(
        device:   &wgpu::Device,
        cmds_n:   u32,
        screen_w: u32,
        screen_h: u32,
    ) -> Self {
        Self::allocate_with(device, cmds_n, screen_w, screen_h, DEFAULT_PATH_POINTS_CAP)
    }

    /// Allocate storage buffers with an explicit `path_points_cap`.
    /// Use this when you know up-front your scene needs more than the
    /// default 16K vec2s (≈ 16K polyline vertices total per frame).
    pub fn allocate_with(
        device:          &wgpu::Device,
        cmds_n:          u32,
        screen_w:        u32,
        screen_h:        u32,
        path_points_cap: u32,
    ) -> Self {
        let tx = (screen_w + TILE_SIZE - 1) / TILE_SIZE;
        let ty = (screen_h + TILE_SIZE - 1) / TILE_SIZE;
        let tiles = (tx * ty) as u64;

        let cmds_size = (cmds_n as u64).max(1) * 32;
        let cmds_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx-fullgpu-cmds"),
            size: cmds_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let tile_counts_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx-fullgpu-tile-counts"),
            size: tiles * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let tile_lists_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx-fullgpu-tile-lists"),
            size: tiles * TILE_CMD_CAP as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let pp_cap = path_points_cap.max(2);
        let path_points_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx-fullgpu-path-points"),
            size: (pp_cap as u64) * 8, // vec2<f32> = 8 bytes
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            cmds_buf,
            tile_counts_buf,
            tile_lists_buf,
            path_points_buf,
            path_points_cap: pp_cap,
            tile_count_x: tx,
            tile_count_y: ty,
        }
    }

    /// Create tile buffers AND an rgba8unorm storage+copy_src output texture.
    ///
    /// Returns `(TileBuffers, Texture, TextureView)`. The texture is sized
    /// to `(tile_count_x * TILE_SIZE, tile_count_y * TILE_SIZE)` — always
    /// a multiple of 16 that covers the full viewport.
    pub fn with_output_texture(
        device:   &wgpu::Device,
        cmds_n:   u32,
        screen_w: u32,
        screen_h: u32,
    ) -> (Self, wgpu::Texture, wgpu::TextureView) {
        let bufs = Self::allocate(device, cmds_n, screen_w, screen_h);
        let tex_w = bufs.tile_count_x * TILE_SIZE;
        let tex_h = bufs.tile_count_y * TILE_SIZE;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx-fullgpu-output"),
            size: wgpu::Extent3d { width: tex_w, height: tex_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          wgpu::TextureFormat::Rgba8Unorm,
            // TEXTURE_BINDING added so BlitPipeline can sample this as a
            // regular texture_2d<f32> in the blit fragment shader.
            usage:           wgpu::TextureUsages::STORAGE_BINDING
                           | wgpu::TextureUsages::COPY_SRC
                           | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats:    &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (bufs, texture, view)
    }
}

/// Uniform block fed to all compute shaders.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DispatchUniforms {
    pub cmd_count:    u32,
    pub tile_count_x: u32,
    pub tile_count_y: u32,
    pub tile_cmd_cap: u32,
}

/// Compiled compute pipelines for tile-assign, tile-sort, and fine raster.
pub struct TilePipeline {
    pub assign_pipeline: wgpu::ComputePipeline,
    pub sort_pipeline:   wgpu::ComputePipeline,
    pub fine_pipeline:   wgpu::ComputePipeline,
    pub bgl:             wgpu::BindGroupLayout,
    pub uniforms_buf:    wgpu::Buffer,
    glyph_sampler:       wgpu::Sampler,
}

impl TilePipeline {
    /// Compile all three WGSL shaders and build the shared bind group layout.
    pub fn new(device: &wgpu::Device) -> Self {
        // Shared BGL: 5 bindings used by at least one of the three shaders.
        //   0 — uniform  (DispatchUniforms)
        //   1 — storage read-only  (cmds)
        //   2 — storage read-write (tile_counts, atomic<u32>)
        //   3 — storage read-write (tile_lists)
        //   4 — storage texture rgba8unorm WriteOnly (output pixel buffer)
        //
        // tile_assign + tile_sort declare binding 4 as a write-only storage
        // texture to keep the BGL single. fine declares bindings 2+3 as
        // read-only (non-atomic); wgpu validates storage access mode against
        // the BGL's declared type, not the shader's sub-access.
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx-fullgpu-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access:        wgpu::StorageTextureAccess::WriteOnly,
                        format:        wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                // binding 5 — glyph atlas (r8unorm sampled texture).
                // Declared in fine.wgsl; declared as dummy in tile_assign + tile_sort
                // to keep a single shared BGL across all three passes.
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                // binding 6 — glyph atlas sampler.
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 7 — path_points (array<vec2<f32>>): polyline
                // vertices for CmdKind::Path. Read-only in all three
                // passes; declared once in the shared BGL.
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 8 — image_atlas (rgba8unorm sampled texture):
                // texture brush source for CmdKind::Image. Read-only.
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("urx-fullgpu-pipeline-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let assign_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("tile_assign"),
            source: wgpu::ShaderSource::Wgsl(TILE_ASSIGN_WGSL.into()),
        });
        let sort_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("tile_sort"),
            source: wgpu::ShaderSource::Wgsl(TILE_SORT_WGSL.into()),
        });
        let fine_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("fine"),
            source: wgpu::ShaderSource::Wgsl(FINE_WGSL.into()),
        });

        let assign_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label:  Some("urx-fullgpu-assign"),
            layout: Some(&pipeline_layout),
            module: &assign_module,
            entry_point: Some("assign"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let sort_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label:  Some("urx-fullgpu-sort"),
            layout: Some(&pipeline_layout),
            module: &sort_module,
            entry_point: Some("sort"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let fine_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label:  Some("urx-fullgpu-fine"),
            layout: Some(&pipeline_layout),
            module: &fine_module,
            entry_point: Some("fine"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let uniforms_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx-fullgpu-uniforms"),
            size: std::mem::size_of::<DispatchUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let glyph_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label:          Some("urx-fullgpu-glyph-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter:     wgpu::FilterMode::Linear,
            min_filter:     wgpu::FilterMode::Linear,
            mipmap_filter:  wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self { assign_pipeline, sort_pipeline, fine_pipeline, bgl, uniforms_buf, glyph_sampler }
    }

    /// Create a 1×1 fully-transparent R8Unorm texture to use as a dummy
    /// glyph atlas when no real atlas is available.
    ///
    /// Pass the returned `TextureView` to `dispatch` / `dispatch_full` /
    /// `render_to_target` when the scene contains no `Glyph` commands.
    pub fn dummy_glyph_atlas(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label:               Some("urx-fullgpu-dummy-glyph-atlas"),
            size:                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count:     1,
            sample_count:        1,
            dimension:           wgpu::TextureDimension::D2,
            format:              wgpu::TextureFormat::R8Unorm,
            usage:               wgpu::TextureUsages::TEXTURE_BINDING
                               | wgpu::TextureUsages::COPY_DST,
            view_formats:        &[],
        });
        // The 1×1 texel defaults to zero (transparent) on the GPU side.
        // No queue upload needed — GPU-side storage is zero-initialised on creation.
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Create a 1×1 fully-transparent RGBA8 texture to use as a dummy
    /// image atlas when the scene contains no `Image` commands.
    ///
    /// Pass the returned `TextureView` to `dispatch` / `dispatch_full` /
    /// `render_to_target` when no images are in use.
    pub fn dummy_image_atlas(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label:               Some("urx-fullgpu-dummy-image-atlas"),
            size:                wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count:     1,
            sample_count:        1,
            dimension:           wgpu::TextureDimension::D2,
            format:              wgpu::TextureFormat::Rgba8Unorm,
            usage:               wgpu::TextureUsages::TEXTURE_BINDING
                               | wgpu::TextureUsages::COPY_DST,
            view_formats:        &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Upload cmds, dispatch tile_assign, then tile_sort.
    ///
    /// `tile_counts_buf` is cleared to zero before the assign dispatch
    /// so this method is safe to call every frame.
    ///
    /// `glyph_atlas_view` must be an R8Unorm `TextureView`.  When the scene
    /// contains no `Glyph` commands, pass the view from
    /// `TilePipeline::dummy_glyph_atlas` — binding 5 must always be
    /// satisfied.
    ///
    /// No output texture is written — use `dispatch_full` for pixel output.
    ///
    /// Convenience: passes an empty `path_points` slice (scenes with no
    /// Path cmds). Use `dispatch_full` directly when paths are present.
    pub fn dispatch(
        &self,
        device:           &wgpu::Device,
        queue:            &wgpu::Queue,
        encoder:          &mut wgpu::CommandEncoder,
        bufs:             &TileBuffers,
        cmds:             &[SceneCmd],
        glyph_atlas_view: &wgpu::TextureView,
    ) {
        // Build a dummy 1×1 rgba8unorm storage texture so binding 4 is
        // satisfied when callers use dispatch() without a real output texture.
        let dummy_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("urx-fullgpu-dummy-output"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       wgpu::TextureDimension::D2,
            format:          wgpu::TextureFormat::Rgba8Unorm,
            usage:           wgpu::TextureUsages::STORAGE_BINDING,
            view_formats:    &[],
        });
        let dummy_view = dummy_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let (_dummy_img, dummy_img_view) = Self::dummy_image_atlas(device);
        self.dispatch_full(
            device, queue, encoder, bufs, cmds, &[],
            &dummy_view, glyph_atlas_view, &dummy_img_view,
        );
    }

    /// Full three-stage dispatch: tile_assign → tile_sort → fine raster.
    ///
    /// `output_view` must be a `Rgba8Unorm` storage texture view created
    /// via `TileBuffers::with_output_texture` (or any compatible view).
    /// After submission, the texture contains the composited pixel output.
    ///
    /// `glyph_atlas_view` must be an R8Unorm `TextureView` that the fine
    /// shader samples for `Glyph` commands.  When no glyph commands are
    /// present, pass the view from `TilePipeline::dummy_glyph_atlas`.
    ///
    /// `path_points` is a flat `[x0, y0, x1, y1, ...]` slice referenced
    /// by `CmdKind::Path` cmds via `(point_offset, point_count)`. Pass
    /// `&[]` when the scene contains no Path cmds.
    ///
    /// `image_atlas_view` is an RGBA8 sampled-texture view for
    /// `CmdKind::Image` cmds. Pass the view from
    /// `TilePipeline::dummy_image_atlas` when no Image cmds present.
    pub fn dispatch_full(
        &self,
        device:           &wgpu::Device,
        queue:            &wgpu::Queue,
        encoder:          &mut wgpu::CommandEncoder,
        bufs:             &TileBuffers,
        cmds:             &[SceneCmd],
        path_points:      &[[f32; 2]],
        output_view:      &wgpu::TextureView,
        glyph_atlas_view: &wgpu::TextureView,
        image_atlas_view: &wgpu::TextureView,
    ) {
        // 1. Upload cmd bytes.
        if !cmds.is_empty() {
            queue.write_buffer(&bufs.cmds_buf, 0, bytemuck::cast_slice(cmds));
        }
        // 1b. Upload path_points if present (caller-allocated capacity
        // checked at TileBuffers construction; we silently truncate if
        // someone passes more than path_points_cap — debug_assert catches
        // it in dev builds).
        if !path_points.is_empty() {
            let cap = bufs.path_points_cap as usize;
            debug_assert!(
                path_points.len() <= cap,
                "path_points slice ({}) exceeds path_points_cap ({}); \
                 re-allocate TileBuffers with allocate_with(..., path_points_cap=N)",
                path_points.len(), cap,
            );
            let take = path_points.len().min(cap);
            queue.write_buffer(
                &bufs.path_points_buf,
                0,
                bytemuck::cast_slice(&path_points[..take]),
            );
        }

        // 2. Upload uniforms.
        let uni = DispatchUniforms {
            cmd_count:    cmds.len() as u32,
            tile_count_x: bufs.tile_count_x,
            tile_count_y: bufs.tile_count_y,
            tile_cmd_cap: TILE_CMD_CAP,
        };
        queue.write_buffer(&self.uniforms_buf, 0, bytemuck::bytes_of(&uni));

        // 3. Zero tile_counts every frame before assign.
        encoder.clear_buffer(&bufs.tile_counts_buf, 0, None);

        // 4. Build bind group (bindings 0..6).
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("urx-fullgpu-bg"),
            layout:  &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniforms_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: bufs.cmds_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: bufs.tile_counts_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: bufs.tile_lists_buf.as_entire_binding() },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(glyph_atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&self.glyph_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: bufs.path_points_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(image_atlas_view),
                },
            ],
        });

        // 5. Compute pass: assign → sort → fine.
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("urx-fullgpu-pass"),
                timestamp_writes: None,
            });

            // tile_assign: one invocation per cmd.
            let cmd_groups = cmds.len().div_ceil(64).max(1) as u32;
            pass.set_pipeline(&self.assign_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(cmd_groups, 1, 1);

            // tile_sort: one invocation per tile.
            let tile_total = (bufs.tile_count_x * bufs.tile_count_y) as usize;
            let tile_groups = tile_total.div_ceil(64).max(1) as u32;
            pass.set_pipeline(&self.sort_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(tile_groups, 1, 1);

            // fine: one workgroup (16×16 = 256 threads) per tile.
            // dispatch_workgroups(tile_count_x, tile_count_y, 1)
            // → total invocations = tile_count_x * tile_count_y * 256
            // → covers every pixel of the padded viewport.
            pass.set_pipeline(&self.fine_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(bufs.tile_count_x, bufs.tile_count_y, 1);
        }
    }

    /// Convenience wrapper: `dispatch_full` followed immediately by
    /// `blit.blit(...)` — both encoded into the same `encoder`.
    ///
    /// `src_view` must be the `TextureView` returned by
    /// `TileBuffers::with_output_texture`. `src_w`/`src_h` are the padded
    /// tile-aligned dimensions (`tile_count_x * TILE_SIZE`, etc.).
    ///
    /// `target_view` is any render-attachment-compatible `TextureView`
    /// (e.g. a swapchain surface texture).
    ///
    /// `glyph_atlas_view` — same contract as `dispatch_full`.
    ///
    /// Call `queue.submit` once after this method returns — both passes are
    /// recorded in the same encoder.
    pub fn render_to_target(
        &self,
        device:           &wgpu::Device,
        queue:            &wgpu::Queue,
        encoder:          &mut wgpu::CommandEncoder,
        bufs:             &TileBuffers,
        cmds:             &[SceneCmd],
        path_points:      &[[f32; 2]],
        src_view:         &wgpu::TextureView,
        blit:             &BlitPipeline,
        target_view:      &wgpu::TextureView,
        src_w:            u32,
        src_h:            u32,
        glyph_atlas_view: &wgpu::TextureView,
        image_atlas_view: &wgpu::TextureView,
    ) {
        self.dispatch_full(
            device, queue, encoder, bufs, cmds, path_points,
            src_view, glyph_atlas_view, image_atlas_view,
        );
        blit.blit(device, encoder, src_view, target_view, src_w, src_h, queue);
    }
}

// ---------------------------------------------------------------------------
// BlitPipeline
// ---------------------------------------------------------------------------

/// Render pipeline that blits from an rgba8unorm storage texture to any
/// render-attachment-compatible target format (e.g. bgra8unorm for a
/// swapchain surface, or rgba8unorm-srgb for a HDR surface).
///
/// # Format binding
///
/// The target format is fixed at pipeline creation time (WGSL / SPIR-V
/// pipeline compilation bakes the attachment format). If the consumer's
/// surface format changes (window resize, DPI change, monitor switch),
/// create a new `BlitPipeline` with the updated format.
///
/// # Usage
///
/// ```ignore
/// let blit = BlitPipeline::new(&device, surface_format);
/// // inside render loop:
/// pipeline.render_to_target(
///     &device, &queue, &mut encoder,
///     &bufs, &cmds,
///     &storage_view, &blit, &surface_view,
///     tex_w, tex_h,
/// );
/// queue.submit(Some(encoder.finish()));
/// ```
pub struct BlitPipeline {
    pipeline:  wgpu::RenderPipeline,
    sampler:   wgpu::Sampler,
    bgl:       wgpu::BindGroupLayout,
    /// 16-byte uniform: `[src_w: u32, src_h: u32, 0u32, 0u32]`.
    size_buf:  wgpu::Buffer,
}

impl BlitPipeline {
    /// Compile the blit render pipeline targeting `target_format`.
    ///
    /// `target_format` must be a render-attachment-capable format on the
    /// current adapter (e.g. `wgpu::TextureFormat::Bgra8Unorm`).
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("urx-blit-bgl"),
            entries: &[
                // binding 0 — source texture
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type:    wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled:   false,
                    },
                    count: None,
                },
                // binding 1 — sampler
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2 — src_size uniform (w, h, 0, 0)
                wgpu::BindGroupLayoutEntry {
                    binding:    2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty:                 wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size:   None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("urx-blit-pipeline-layout"),
            bind_group_layouts:   &[Some(&bgl)],
            immediate_size:       0,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("urx-blit-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &shader,
                entry_point:         Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers:             &[],
            },
            fragment: Some(wgpu::FragmentState {
                module:              &shader,
                entry_point:         Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format:     target_format,
                    blend:      None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive:    wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample:  wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache:          None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label:          Some("urx-blit-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter:     wgpu::FilterMode::Linear,
            min_filter:     wgpu::FilterMode::Linear,
            mipmap_filter:  wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // 16-byte uniform: [w, h, 0, 0] as u32 array.
        let size_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("urx-blit-size-buf"),
            size:               16,
            usage:              wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self { pipeline, sampler, bgl, size_buf }
    }

    /// Encode a single render pass that samples `src_view` and writes to
    /// `target_view`.
    ///
    /// - `src_view` must come from an rgba8unorm texture created with
    ///   `TextureUsages::TEXTURE_BINDING` (guaranteed by
    ///   `TileBuffers::with_output_texture`).
    /// - `target_view` must be a render-attachment-compatible view whose
    ///   format matches the `target_format` passed to `BlitPipeline::new`.
    /// - `src_w` / `src_h` are the padded tile-aligned dimensions of the
    ///   source texture (typically `tile_count_x * TILE_SIZE`).
    ///
    /// The `queue` parameter is used only to upload the size uniform; no
    /// separate submit is performed — the render pass is recorded into
    /// `encoder`.
    pub fn blit(
        &self,
        _device:     &wgpu::Device,
        encoder:     &mut wgpu::CommandEncoder,
        src_view:    &wgpu::TextureView,
        target_view: &wgpu::TextureView,
        src_w:       u32,
        src_h:       u32,
        queue:       &wgpu::Queue,
    ) {
        // Upload src_size uniform: [w, h, 0, 0] packed as 4×u32 = 16 bytes.
        let size_data: [u32; 4] = [src_w, src_h, 0, 0];
        queue.write_buffer(&self.size_buf, 0, bytemuck::cast_slice(&size_data));

        let bg = _device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("urx-blit-bg"),
            layout:  &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(src_view),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding:  2,
                    resource: self.size_buf.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("urx-blit-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view:           target_view,
                resolve_target: None,
                depth_slice:    None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes:         None,
            occlusion_query_set:      None,
            multiview_mask:           None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bg, &[]);
        // 3 vertices → fullscreen triangle (no index buffer, no vertex buffer).
        pass.draw(0..3, 0..1);
    }
}
