//! GPU pipeline setup, buffer management, and draw calls.
//!
//! `InstancedRenderer` owns all GPU resources (pipelines, buffers, atlas) and
//! exposes a single `render()` method that executes in three draw calls:
//!
//! 1. Quad instances  (filled/bordered rounded rectangles)
//! 2. Line instances  (capsule SDF segments)
//! 3. Glyph instances (text, sampled from a R8Unorm atlas)

use bytemuck::cast_slice;
use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use wgpu::util::DeviceExt;

use crate::glyph_instance::GlyphInstance;
use crate::instances::{LineInstance, QuadInstance};
use crate::shaders::{GLYPH_SHADER, LINE_SHADER, QUAD_SHADER};
use crate::text::TextAreaData;
use crate::text_atlas::GlyphAtlas;

// ── Embedded Roboto fonts (same bytes already included by context.rs) ──────
static ROBOTO_REGULAR: &[u8]     = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD: &[u8]        = include_bytes!("../fonts/Roboto-Bold.ttf");
static ROBOTO_ITALIC: &[u8]      = include_bytes!("../fonts/Roboto-Italic.ttf");
static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

/// Initial capacity (number of instances) for quad, line, and glyph buffers.
const INITIAL_CAPACITY: usize = 1024;

/// Glyph atlas texture size (pixels per side).
const ATLAS_SIZE: u32 = 2048;

/// Uniform data sent to all shaders.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

// ── Vertex buffer layout helpers ──────────────────────────────────────────

/// Creates the vertex buffer layout for `QuadInstance`.
fn quad_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    // QuadInstance fields in order:
    //  0: pos           [f32;2]  @ offset  0
    //  1: size          [f32;2]  @ offset  8
    //  2: color         [f32;4]  @ offset 16
    //  3: corner_radius  f32     @ offset 32
    //  4: border_width   f32     @ offset 36
    //  5: _pad0         [f32;2]  @ offset 40
    //  6: border_color  [f32;4]  @ offset 48
    //  7: clip_rect     [f32;4]  @ offset 64
    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset:  0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset:  8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x4, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32,   offset: 32 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32,   offset: 36 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32x2, offset: 40 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32x4, offset: 48 },
        wgpu::VertexAttribute { shader_location: 7, format: Float32x4, offset: 64 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<QuadInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Creates the vertex buffer layout for `LineInstance`.
fn line_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    // LineInstance fields:
    //  0: start      [f32;2]  @ offset  0
    //  1: end        [f32;2]  @ offset  8
    //  2: color      [f32;4]  @ offset 16
    //  3: width       f32     @ offset 32
    //  4: _pad0      [f32;3]  @ offset 36
    //  5: clip_rect  [f32;4]  @ offset 48
    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset:  0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset:  8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x4, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32,   offset: 32 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32x3, offset: 36 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32x4, offset: 48 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LineInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Creates the vertex buffer layout for `GlyphInstance`.
///
/// GlyphInstance fields (64 bytes):
///  0: pos        [f32;2]  @ offset  0
///  1: size       [f32;2]  @ offset  8
///  2: uv_pos     [f32;2]  @ offset 16
///  3: uv_size    [f32;2]  @ offset 24
///  4: color      [f32;4]  @ offset 32
///  5: clip_rect  [f32;4]  @ offset 48
fn glyph_instance_layout() -> wgpu::VertexBufferLayout<'static> {
    use wgpu::VertexFormat::*;

    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset:  0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset:  8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x2, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32x2, offset: 24 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32x4, offset: 32 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32x4, offset: 48 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GlyphInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Creates an instance buffer sized for `capacity` elements of `elem_size` bytes.
fn make_instance_buffer(device: &wgpu::Device, capacity: usize, elem_size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instance_buffer"),
        size: (capacity * elem_size) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

// ── InstancedRenderer ────────────────────────────────────────────────────

/// Owns all GPU resources and drives rendering each frame.
pub struct InstancedRenderer {
    // Quad pipeline
    quad_pipeline: wgpu::RenderPipeline,
    quad_buffer:   wgpu::Buffer,
    quad_capacity: usize,

    // Line pipeline
    line_pipeline: wgpu::RenderPipeline,
    line_buffer:   wgpu::Buffer,
    line_capacity: usize,

    // Shared uniform buffer (screen_size) — group 0 for all pipelines
    uniform_buffer:     wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // Glyph pipeline
    glyph_pipeline:      wgpu::RenderPipeline,
    glyph_buffer:        wgpu::Buffer,
    glyph_capacity:      usize,
    atlas_bind_group:    wgpu::BindGroup,
    atlas_bind_group_layout: wgpu::BindGroupLayout,

    // Text subsystem
    font_system:  FontSystem,
    swash_cache:  SwashCache,
    glyph_atlas:  GlyphAtlas,
}

impl InstancedRenderer {
    /// Create the renderer and all GPU resources.
    ///
    /// `format` must match the swap-chain / surface texture format.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        // ── Uniform buffer (group 0) ──────────────────────────────────────
        let uniform_data = Uniforms { screen_size: [1.0, 1.0], _pad: [0.0; 2] };
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("instanced_uniforms"),
            contents: cast_slice(&[uniform_data]),
            usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("instanced_uniform_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding:    0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty:                 wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size:   None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("instanced_uniform_bg"),
            layout:  &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding:  0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // ── Shared pipeline layout (group 0 only — for quad + line) ───────
        let base_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label:                Some("instanced_pipeline_layout"),
            bind_group_layouts:   &[&uniform_bgl],
            push_constant_ranges: &[],
        });

        // ── Alpha blend state ─────────────────────────────────────────────
        let blend = wgpu::BlendState::ALPHA_BLENDING;

        // ── Quad pipeline ─────────────────────────────────────────────────
        let quad_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("quad_shader"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
        });
        let quad_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("quad_pipeline"),
            layout: Some(&base_pipeline_layout),
            vertex: wgpu::VertexState {
                module:               &quad_shader,
                entry_point:          Some("vs_main"),
                buffers:              &[quad_instance_layout()],
                compilation_options:  Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:              &quad_shader,
                entry_point:         Some("fs_main"),
                targets:             &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        // ── Line pipeline ─────────────────────────────────────────────────
        let line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("line_shader"),
            source: wgpu::ShaderSource::Wgsl(LINE_SHADER.into()),
        });
        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("line_pipeline"),
            layout: Some(&base_pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &line_shader,
                entry_point:         Some("vs_main"),
                buffers:             &[line_instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:              &line_shader,
                entry_point:         Some("fs_main"),
                targets:             &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        // ── Glyph atlas ───────────────────────────────────────────────────
        let glyph_atlas = GlyphAtlas::new(device, ATLAS_SIZE, ATLAS_SIZE);

        // ── Atlas bind group layout (group 1: texture + sampler) ──────────
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("atlas_bgl"),
                entries: &[
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
                    wgpu::BindGroupLayoutEntry {
                        binding:    1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("atlas_bg"),
            layout:  &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(glyph_atlas.texture_view()),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&glyph_atlas.sampler),
                },
            ],
        });

        // ── Glyph pipeline layout (group 0: uniforms, group 1: atlas) ─────
        let glyph_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label:                Some("glyph_pipeline_layout"),
                bind_group_layouts:   &[&uniform_bgl, &atlas_bind_group_layout],
                push_constant_ranges: &[],
            });

        let glyph_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("glyph_shader"),
            source: wgpu::ShaderSource::Wgsl(GLYPH_SHADER.into()),
        });
        let glyph_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("glyph_pipeline"),
            layout: Some(&glyph_pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &glyph_shader,
                entry_point:         Some("vs_glyph"),
                buffers:             &[glyph_instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:              &glyph_shader,
                entry_point:         Some("fs_glyph"),
                targets:             &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample:   wgpu::MultisampleState::default(),
            multiview:     None,
            cache:         None,
        });

        // ── Instance buffers ──────────────────────────────────────────────
        let quad_buffer = make_instance_buffer(
            device, INITIAL_CAPACITY, std::mem::size_of::<QuadInstance>());
        let line_buffer = make_instance_buffer(
            device, INITIAL_CAPACITY, std::mem::size_of::<LineInstance>());
        let glyph_buffer = make_instance_buffer(
            device, INITIAL_CAPACITY, std::mem::size_of::<GlyphInstance>());

        // ── FontSystem with embedded Roboto fonts ─────────────────────────
        let font_system = FontSystem::new_with_fonts([
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_REGULAR)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_BOLD)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_ITALIC)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_BOLD_ITALIC)),
        ]);
        let swash_cache = SwashCache::new();

        // Silence the unused `queue` warning — it's used by the atlas later
        let _ = queue;

        Self {
            quad_pipeline,
            quad_buffer,
            quad_capacity: INITIAL_CAPACITY,

            line_pipeline,
            line_buffer,
            line_capacity: INITIAL_CAPACITY,

            uniform_buffer,
            uniform_bind_group,

            glyph_pipeline,
            glyph_buffer,
            glyph_capacity: INITIAL_CAPACITY,
            atlas_bind_group,
            atlas_bind_group_layout,

            font_system,
            swash_cache,
            glyph_atlas,
        }
    }

    /// Upload instances to a GPU buffer, growing it if needed.
    fn upload_instances<T: bytemuck::Pod>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &mut wgpu::Buffer,
        capacity: &mut usize,
        data: &[T],
    ) {
        if data.is_empty() {
            return;
        }
        let needed = data.len();
        if needed > *capacity {
            while *capacity < needed {
                *capacity *= 2;
            }
            *buffer = make_instance_buffer(device, *capacity, std::mem::size_of::<T>());
        }
        queue.write_buffer(buffer, 0, cast_slice(data));
    }

    /// Shape `text_areas` into `GlyphInstance`s via cosmic-text + swash atlas.
    fn build_glyph_instances(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text_areas: &[TextAreaData],
    ) -> Vec<GlyphInstance> {
        let mut instances = Vec::new();

        for ta in text_areas {
            if ta.text.is_empty() {
                continue;
            }

            // Choose font family based on bold/italic flags.
            let family_name = match (ta.bold, ta.italic) {
                (true, true)   => "Roboto BoldItalic",
                (true, false)  => "Roboto Bold",
                (false, true)  => "Roboto Italic",
                (false, false) => "Roboto",
            };

            let line_height = ta.font_size * 1.2;
            let mut buffer = Buffer::new(
                &mut self.font_system,
                Metrics::new(ta.font_size, line_height),
            );

            // Single-line: no wrap, unlimited width.
            buffer.set_size(&mut self.font_system, None, None);

            let attrs = Attrs::new().family(Family::Name(family_name));
            buffer.set_text(&mut self.font_system, &ta.text, attrs, Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.font_system, false);

            // Compute text-area top-left for alignment.
            let ascent  = ta.font_size * 0.8; // approximate
            let descent = ta.font_size * 0.2;
            let (base_x, base_y) = ta.top_left(ascent, descent);

            // Iterate shaped glyphs.
            for run in buffer.layout_runs() {
                for glyph in run.glyphs.iter() {
                    // Obtain the physical (pixel-snapped) cache key.
                    let physical = glyph.physical((0.0, 0.0), 1.0);
                    let ck = physical.cache_key;

                    let entry = self.glyph_atlas.get_or_insert(
                        ck,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        device,
                        queue,
                    );
                    let entry = match entry {
                        Some(e) => e,
                        None => continue, // whitespace or zero-size glyph
                    };

                    // Position: run.line_y is the baseline Y within the buffer.
                    // physical.x / physical.y are integer offsets within the run.
                    let glyph_x = base_x + physical.x as f32 + entry.placement_left as f32;
                    let glyph_y = base_y + run.line_y + physical.y as f32 - entry.placement_top as f32;

                    instances.push(GlyphInstance {
                        pos:       [glyph_x, glyph_y],
                        size:      [entry.width as f32, entry.height as f32],
                        uv_pos:    [entry.uv_x, entry.uv_y],
                        uv_size:   [entry.uv_w, entry.uv_h],
                        color:     ta.color,
                        clip_rect: ta.clip,
                    });
                }
            }
        }

        instances
    }

    /// Render a complete frame.
    ///
    /// `clear_color` is the background color applied via `LoadOp::Clear`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        quads: &[QuadInstance],
        lines: &[LineInstance],
        text_areas: &[TextAreaData],
        clear_color: wgpu::Color,
    ) {
        // ── Update uniforms ───────────────────────────────────────────────
        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));

        // ── Upload quad + line instances ──────────────────────────────────
        Self::upload_instances(
            device, queue,
            &mut self.quad_buffer, &mut self.quad_capacity,
            quads,
        );
        Self::upload_instances(
            device, queue,
            &mut self.line_buffer, &mut self.line_capacity,
            lines,
        );

        // ── Build glyph instances (shapes text, uploads to atlas) ─────────
        let glyph_instances = self.build_glyph_instances(device, queue, text_areas);

        // Rebuild atlas bind group after atlas uploads (texture is stable, just
        // re-bind in case the atlas was recreated — currently it is not, but
        // this keeps the code safe).
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("atlas_bg"),
            layout:  &self.atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding:  0,
                    resource: wgpu::BindingResource::TextureView(self.glyph_atlas.texture_view()),
                },
                wgpu::BindGroupEntry {
                    binding:  1,
                    resource: wgpu::BindingResource::Sampler(&self.glyph_atlas.sampler),
                },
            ],
        });
        self.atlas_bind_group = atlas_bind_group;

        Self::upload_instances(
            device, queue,
            &mut self.glyph_buffer, &mut self.glyph_capacity,
            &glyph_instances,
        );

        // ── Command encoder + render pass ─────────────────────────────────
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("instanced_encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("instanced_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view:           target,
                    resolve_target: None,
                    depth_slice:    None,
                    ops: wgpu::Operations {
                        load:  wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes:         None,
                occlusion_query_set:      None,
            });

            pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Draw call 1: quads
            if !quads.is_empty() {
                pass.set_pipeline(&self.quad_pipeline);
                pass.set_vertex_buffer(0, self.quad_buffer.slice(..));
                pass.draw(0..6, 0..quads.len() as u32);
            }

            // Draw call 2: lines
            if !lines.is_empty() {
                pass.set_pipeline(&self.line_pipeline);
                pass.set_vertex_buffer(0, self.line_buffer.slice(..));
                pass.draw(0..6, 0..lines.len() as u32);
            }

            // Draw call 3: glyphs
            if !glyph_instances.is_empty() {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, self.glyph_buffer.slice(..));
                // Triangle strip: 4 vertices per quad (0-1-2-3 → two triangles).
                pass.draw(0..4, 0..glyph_instances.len() as u32);
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
