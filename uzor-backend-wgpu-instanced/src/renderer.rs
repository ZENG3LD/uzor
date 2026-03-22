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
use crate::instances::{DrawCmd, LineInstance, QuadInstance, TriangleInstance};
use crate::shaders::{GLYPH_SHADER, LINE_SHADER, QUAD_SHADER, TRIANGLE_SHADER};
use crate::text::TextAreaData;
use crate::text_atlas::GlyphAtlas;

// ── Batch type tag ────────────────────────────────────────────────────────────

/// Identifies which GPU pipeline a batch uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchType {
    Quad,
    Triangle,
    Line,
    Glyph,
}

/// A contiguous range of same-type instances in the typed upload buffer.
struct Batch {
    batch_type: BatchType,
    /// Start index in the per-type typed vec (quads / triangles / lines / glyphs).
    start: u32,
    /// Number of instances in this batch.
    count: u32,
}

// ── Embedded Roboto fonts (same bytes already included by context.rs) ──────
static ROBOTO_REGULAR: &[u8]     = include_bytes!("../fonts/Roboto-Regular.ttf");
static ROBOTO_BOLD: &[u8]        = include_bytes!("../fonts/Roboto-Bold.ttf");
static ROBOTO_ITALIC: &[u8]      = include_bytes!("../fonts/Roboto-Italic.ttf");
static ROBOTO_BOLD_ITALIC: &[u8] = include_bytes!("../fonts/Roboto-BoldItalic.ttf");

// ── Unicode fallback fonts ─────────────────────────────────────────────────
// cosmic_text uses these automatically when Roboto lacks a glyph.
static SYMBOLS_FONT: &[u8] = include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf");
static EMOJI_FONT: &[u8]   = include_bytes!("../fonts/NotoEmoji-Regular.ttf");

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

    // LineInstance fields (64 bytes):
    //  0: start      [f32;2]  @ offset  0
    //  1: end        [f32;2]  @ offset  8
    //  2: color      [f32;4]  @ offset 16
    //  3: width       f32     @ offset 32
    //  4: cap_flags   f32     @ offset 36
    //  5: _pad0      [f32;2]  @ offset 40
    //  6: clip_rect  [f32;4]  @ offset 48
    static ATTRS: &[wgpu::VertexAttribute] = &[
        wgpu::VertexAttribute { shader_location: 0, format: Float32x2, offset:  0 },
        wgpu::VertexAttribute { shader_location: 1, format: Float32x2, offset:  8 },
        wgpu::VertexAttribute { shader_location: 2, format: Float32x4, offset: 16 },
        wgpu::VertexAttribute { shader_location: 3, format: Float32,   offset: 32 },
        wgpu::VertexAttribute { shader_location: 4, format: Float32,   offset: 36 },
        wgpu::VertexAttribute { shader_location: 5, format: Float32x2, offset: 40 },
        wgpu::VertexAttribute { shader_location: 6, format: Float32x4, offset: 48 },
    ];

    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LineInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: ATTRS,
    }
}

/// Creates the vertex buffer layout for `TriangleInstance`.
///
/// TriangleInstance fields (64 bytes):
///  0: v0         [f32;2]  @ offset  0
///  1: v1         [f32;2]  @ offset  8
///  2: v2         [f32;2]  @ offset 16
///  3: _pad0      [f32;2]  @ offset 24
///  4: color      [f32;4]  @ offset 32
///  5: clip_rect  [f32;4]  @ offset 48
fn triangle_instance_layout() -> wgpu::VertexBufferLayout<'static> {
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
        array_stride: std::mem::size_of::<TriangleInstance>() as wgpu::BufferAddress,
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

    // Triangle pipeline
    triangle_pipeline: wgpu::RenderPipeline,
    triangle_buffer:   wgpu::Buffer,
    triangle_capacity: usize,

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

        // ── Triangle pipeline ─────────────────────────────────────────────
        let triangle_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("triangle_shader"),
            source: wgpu::ShaderSource::Wgsl(TRIANGLE_SHADER.into()),
        });
        let triangle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:  Some("triangle_pipeline"),
            layout: Some(&base_pipeline_layout),
            vertex: wgpu::VertexState {
                module:              &triangle_shader,
                entry_point:         Some("vs_main"),
                buffers:             &[triangle_instance_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module:              &triangle_shader,
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
        let triangle_buffer = make_instance_buffer(
            device, INITIAL_CAPACITY, std::mem::size_of::<TriangleInstance>());
        let glyph_buffer = make_instance_buffer(
            device, INITIAL_CAPACITY, std::mem::size_of::<GlyphInstance>());

        // ── FontSystem with embedded fonts ────────────────────────────────
        // Roboto covers Latin/Cyrillic/Greek; fallback fonts cover symbols and
        // emoji via cosmic_text's built-in per-glyph font fallback mechanism.
        let font_system = FontSystem::new_with_fonts([
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_REGULAR)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_BOLD)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_ITALIC)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(ROBOTO_BOLD_ITALIC)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(SYMBOLS_FONT)),
            cosmic_text::fontdb::Source::Binary(std::sync::Arc::new(EMOJI_FONT)),
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

            triangle_pipeline,
            triangle_buffer,
            triangle_capacity: INITIAL_CAPACITY,

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

            // Iterate shaped glyphs.
            for run in buffer.layout_runs() {
                // Use real metrics from cosmic-text for precise baseline alignment.
                let real_ascent = run.line_y; // distance from buffer top to baseline
                let real_descent = ta.font_size * 1.2 - real_ascent; // line_height - ascent
                let (base_x, base_y) = ta.top_left(real_ascent, real_descent);

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

                    // Position: base_y is the top of the text area (computed from
                    // the real ascent), run.line_y is the baseline Y within the
                    // buffer. Together they place the glyph at the correct baseline.
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

    /// Render a complete frame from a unified draw command list.
    ///
    /// Commands are processed in submission order (painter's order / z-order).
    /// Consecutive same-type commands are coalesced into a single GPU draw call,
    /// while commands of different types trigger a pipeline switch.
    ///
    /// `clear_color`: `Some(color)` → `LoadOp::Clear`, `None` → `LoadOp::Load`
    /// (overlay on existing content).
    ///
    /// `scissor`: optional `(x, y, w, h)` scissor rect to restrict rendering
    /// to a sub-region of the target (e.g. chart area only, excluding chrome).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        commands: &[DrawCmd],
        clear_color: Option<wgpu::Color>,
        scissor: Option<(u32, u32, u32, u32)>,
    ) {
        // ── Update uniforms ───────────────────────────────────────────────
        let uniforms = Uniforms {
            screen_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, cast_slice(&[uniforms]));

        // ── Phase 1: Split commands into typed vecs, record batch boundaries ──
        //
        // We scan `commands` once and:
        // - Route each command into its typed vec (quads / triangles / lines).
        // - Rasterize text commands into glyph instances immediately.
        // - Record a Batch whenever the type changes, so we can replay the
        //   correct draw calls in order during the render pass.

        let mut quads:     Vec<QuadInstance>     = Vec::new();
        let mut triangles: Vec<TriangleInstance> = Vec::new();
        let mut lines:     Vec<LineInstance>     = Vec::new();
        let mut glyphs:    Vec<GlyphInstance>    = Vec::new();
        let mut batches:   Vec<Batch>            = Vec::new();

        // Collect TextAreaData references so we can rasterize them in one pass
        // (build_glyph_instances takes a slice).  We accumulate per-text-batch
        // ranges so we can record the correct (start, count) per batch.
        // For simplicity we rasterize each TextAreaData individually and track
        // the glyph range produced.
        for cmd in commands {
            match cmd {
                DrawCmd::Quad(q) => {
                    let needs_new_batch = batches
                        .last()
                        .map_or(true, |b: &Batch| b.batch_type != BatchType::Quad);
                    if needs_new_batch {
                        batches.push(Batch {
                            batch_type: BatchType::Quad,
                            start: quads.len() as u32,
                            count: 0,
                        });
                    }
                    quads.push(*q);
                    batches.last_mut().unwrap().count += 1;
                }
                DrawCmd::Triangle(t) => {
                    let needs_new_batch = batches
                        .last()
                        .map_or(true, |b: &Batch| b.batch_type != BatchType::Triangle);
                    if needs_new_batch {
                        batches.push(Batch {
                            batch_type: BatchType::Triangle,
                            start: triangles.len() as u32,
                            count: 0,
                        });
                    }
                    triangles.push(*t);
                    batches.last_mut().unwrap().count += 1;
                }
                DrawCmd::Line(l) => {
                    let needs_new_batch = batches
                        .last()
                        .map_or(true, |b: &Batch| b.batch_type != BatchType::Line);
                    if needs_new_batch {
                        batches.push(Batch {
                            batch_type: BatchType::Line,
                            start: lines.len() as u32,
                            count: 0,
                        });
                    }
                    lines.push(*l);
                    batches.last_mut().unwrap().count += 1;
                }
                DrawCmd::Text(ta) => {
                    // Rasterize this single text area into glyph instances.
                    // We always start a new Glyph batch here (text areas are
                    // never auto-coalesced across other command types).
                    let glyph_start = glyphs.len() as u32;
                    let new_glyphs = self.build_glyph_instances(device, queue, std::slice::from_ref(ta));
                    let glyph_count = new_glyphs.len() as u32;
                    glyphs.extend(new_glyphs);

                    if glyph_count > 0 {
                        // Coalesce consecutive text batches into one draw call
                        // (they share the same pipeline and bind groups).
                        let needs_new_batch = batches
                            .last()
                            .map_or(true, |b: &Batch| b.batch_type != BatchType::Glyph);
                        if needs_new_batch {
                            batches.push(Batch {
                                batch_type: BatchType::Glyph,
                                start: glyph_start,
                                count: glyph_count,
                            });
                        } else {
                            batches.last_mut().unwrap().count += glyph_count;
                        }
                    }
                }
            }
        }

        // ── Phase 2: Upload all typed buffers to GPU ──────────────────────

        Self::upload_instances(
            device, queue,
            &mut self.quad_buffer, &mut self.quad_capacity,
            &quads,
        );
        Self::upload_instances(
            device, queue,
            &mut self.triangle_buffer, &mut self.triangle_capacity,
            &triangles,
        );
        Self::upload_instances(
            device, queue,
            &mut self.line_buffer, &mut self.line_capacity,
            &lines,
        );

        // Rebuild atlas bind group after glyph atlas uploads.
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
            &glyphs,
        );

        // ── Phase 3: Command encoder + render pass with ordered batches ────
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
                        load: match clear_color {
                            Some(c) => wgpu::LoadOp::Clear(c),
                            None    => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes:         None,
                occlusion_query_set:      None,
            });

            pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            if let Some((sx, sy, sw, sh)) = scissor {
                pass.set_scissor_rect(sx, sy, sw, sh);
            }

            // Execute batches in submission order — this preserves painter's z-order.
            let mut current_pipeline = BatchType::Quad; // track to avoid redundant set_pipeline
            let mut pipeline_set = false;

            for batch in &batches {
                if batch.count == 0 {
                    continue;
                }
                let start = batch.start;
                let end = batch.start + batch.count;

                match batch.batch_type {
                    BatchType::Quad => {
                        if !pipeline_set || current_pipeline != BatchType::Quad {
                            pass.set_pipeline(&self.quad_pipeline);
                            pass.set_vertex_buffer(0, self.quad_buffer.slice(..));
                            current_pipeline = BatchType::Quad;
                            pipeline_set = true;
                        }
                        // 6 vertices per instance (2 triangles forming a quad).
                        pass.draw(0..6, start..end);
                    }
                    BatchType::Triangle => {
                        if !pipeline_set || current_pipeline != BatchType::Triangle {
                            pass.set_pipeline(&self.triangle_pipeline);
                            pass.set_vertex_buffer(0, self.triangle_buffer.slice(..));
                            current_pipeline = BatchType::Triangle;
                            pipeline_set = true;
                        }
                        // 3 vertices per triangle instance.
                        pass.draw(0..3, start..end);
                    }
                    BatchType::Line => {
                        if !pipeline_set || current_pipeline != BatchType::Line {
                            pass.set_pipeline(&self.line_pipeline);
                            pass.set_vertex_buffer(0, self.line_buffer.slice(..));
                            current_pipeline = BatchType::Line;
                            pipeline_set = true;
                        }
                        // 6 vertices per instance (oriented quad enclosing the segment).
                        pass.draw(0..6, start..end);
                    }
                    BatchType::Glyph => {
                        if !pipeline_set || current_pipeline != BatchType::Glyph {
                            pass.set_pipeline(&self.glyph_pipeline);
                            pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                            pass.set_vertex_buffer(0, self.glyph_buffer.slice(..));
                            current_pipeline = BatchType::Glyph;
                            pipeline_set = true;
                        }
                        // Triangle strip: 4 vertices per glyph quad.
                        pass.draw(0..4, start..end);
                    }
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
