//! GPU-side tile assignment + per-tile sort, as the first compute
//! dispatch in URX 1.6's full-GPU pipeline.
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

use bytemuck::{Pod, Zeroable};

use crate::cmd::SceneCmd;

/// Tile size in pixels (16×16).
pub const TILE_SIZE: u32 = 16;
/// Maximum commands per tile slot.
pub const TILE_CMD_CAP: u32 = 64;

const TILE_ASSIGN_WGSL: &str = include_str!("shaders/tile_assign.wgsl");
const TILE_SORT_WGSL:   &str = include_str!("shaders/tile_sort.wgsl");

/// GPU buffers for the tile binning pipeline.
pub struct TileBuffers {
    pub cmds_buf:        wgpu::Buffer,
    pub tile_counts_buf: wgpu::Buffer,
    pub tile_lists_buf:  wgpu::Buffer,
    pub tile_count_x:    u32,
    pub tile_count_y:    u32,
}

impl TileBuffers {
    /// Allocate all three storage buffers for the given screen dimensions.
    pub fn allocate(
        device:   &wgpu::Device,
        cmds_n:   u32,
        screen_w: u32,
        screen_h: u32,
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
        Self {
            cmds_buf,
            tile_counts_buf,
            tile_lists_buf,
            tile_count_x: tx,
            tile_count_y: ty,
        }
    }
}

/// Uniform block fed to both compute shaders.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DispatchUniforms {
    pub cmd_count:    u32,
    pub tile_count_x: u32,
    pub tile_count_y: u32,
    pub tile_cmd_cap: u32,
}

/// Compiled compute pipelines for tile-assign + tile-sort.
pub struct TilePipeline {
    pub assign_pipeline: wgpu::ComputePipeline,
    pub sort_pipeline:   wgpu::ComputePipeline,
    pub bgl:             wgpu::BindGroupLayout,
    pub uniforms_buf:    wgpu::Buffer,
}

impl TilePipeline {
    /// Compile both WGSL shaders and build the bind group layout.
    pub fn new(device: &wgpu::Device) -> Self {
        // Bind group layout: 4 entries shared by both shaders.
        //   0 — uniform  (DispatchUniforms)
        //   1 — storage read-only (cmds)
        //   2 — storage read-write (tile_counts, atomic<u32>)
        //   3 — storage read-write (tile_lists)
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("urx-fullgpu-pipeline-layout"),
            bind_group_layouts: &[&bgl],
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

        let uniforms_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("urx-fullgpu-uniforms"),
            size: std::mem::size_of::<DispatchUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self { assign_pipeline, sort_pipeline, bgl, uniforms_buf }
    }

    /// Upload cmds, dispatch tile_assign, then tile_sort.
    ///
    /// `tile_counts_buf` is cleared to zero before the assign dispatch
    /// so this method is safe to call every frame.
    pub fn dispatch(
        &self,
        device:  &wgpu::Device,
        queue:   &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        bufs:    &TileBuffers,
        cmds:    &[SceneCmd],
    ) {
        // 1. Upload cmd bytes.
        if !cmds.is_empty() {
            queue.write_buffer(&bufs.cmds_buf, 0, bytemuck::cast_slice(cmds));
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

        // 4. Build bind group.
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("urx-fullgpu-bg"),
            layout:  &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniforms_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: bufs.cmds_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: bufs.tile_counts_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: bufs.tile_lists_buf.as_entire_binding() },
            ],
        });

        // 5. Compute pass: assign then sort.
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
        }
    }
}
