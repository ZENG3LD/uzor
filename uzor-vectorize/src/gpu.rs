//! GPU vision frontend. Primary path: wgpu compute. CPU is fallback
//! when no adapter is present. Reusable: any later machine-vision
//! stage (denoise, then labels) should call through here.

use std::sync::OnceLock;

use crate::quantize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuMode {
    Auto,
    Gpu,
    Cpu,
}

struct GpuCtx {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bgl: wgpu::BindGroupLayout,
    name: String,
}

static GPU: OnceLock<Option<GpuCtx>> = OnceLock::new();

pub fn denoise(
    rgb: &[u8],
    w: usize,
    h: usize,
    mode: GpuMode,
) -> (Vec<u8>, String) {
    let rgb = quantize::median3(rgb, w, h);
    match mode {
        GpuMode::Cpu => (quantize::bilateral(&rgb, w, h, 3, 2.0, 16.0), "cpu".into()),
        GpuMode::Gpu | GpuMode::Auto => match gpu_bilateral(&rgb, w, h) {
            Ok((out, name)) => (out, format!("gpu:{name}")),
            Err(e) => {
                if mode == GpuMode::Gpu {
                    eprintln!("gpu denoise failed ({e}); cpu fallback");
                }
                (
                    quantize::bilateral(&rgb, w, h, 3, 2.0, 16.0),
                    format!("cpu(fallback:{e})"),
                )
            }
        },
    }
}

fn ctx() -> Result<&'static GpuCtx, String> {
    GPU.get_or_init(init_gpu).as_ref().ok_or_else(|| "no gpu adapter".into())
}

fn init_gpu() -> Option<GpuCtx> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("uzor-vectorize-vision"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()?;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bilateral"),
        source: wgpu::ShaderSource::Wgsl(BILATERAL_WGSL.into()),
    });
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bilateral-bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bilateral-pl"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("bilateral-pipe"),
        layout: Some(&pl),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    Some(GpuCtx {
        device,
        queue,
        pipeline,
        bgl,
        name: info.name,
    })
}

fn gpu_bilateral(rgb: &[u8], w: usize, h: usize) -> Result<(Vec<u8>, String), String> {
    let g = ctx()?;
    let n = w * h;
    let mut packed = vec![0u32; n];
    for i in 0..n {
        packed[i] = (rgb[i * 3] as u32) << 16
            | (rgb[i * 3 + 1] as u32) << 8
            | rgb[i * 3 + 2] as u32;
    }
    let src = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bilateral-src"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    g.queue.write_buffer(&src, 0, bytemuck::cast_slice(&packed));
    let dst = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bilateral-dst"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params = Params {
        width: w as u32,
        height: h as u32,
        radius: 3,
        _pad: 0,
        inv_2s: 0.5 / (2.0 * 2.0),
        inv_2r: 0.5 / (16.0 * 16.0),
        _pad2: 0.0,
        _pad3: 0.0,
    };
    let ub = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bilateral-params"),
        size: std::mem::size_of::<Params>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    g.queue.write_buffer(&ub, 0, bytemuck::bytes_of(&params));
    let bg = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("bilateral-bg"),
        layout: &g.bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: src.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: dst.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: ub.as_entire_binding(),
            },
        ],
    });
    let staging = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bilateral-read"),
        size: (n * 4) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = g.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("bilateral-enc"),
    });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("bilateral-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&g.pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups(((w as u32) + 15) / 16, ((h as u32) + 15) / 16, 1);
    }
    enc.copy_buffer_to_buffer(&dst, 0, &staging, 0, (n * 4) as u64);
    g.queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    let _ = g.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    rx.recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("map {e}"))?;
    let raw = slice.get_mapped_range();
    let packed_out: &[u32] = bytemuck::cast_slice(&raw);
    let mut out = vec![0u8; n * 3];
    for i in 0..n {
        let p = packed_out[i];
        out[i * 3] = (p >> 16) as u8;
        out[i * 3 + 1] = (p >> 8) as u8;
        out[i * 3 + 2] = p as u8;
    }
    drop(raw);
    staging.unmap();
    Ok((out, g.name.clone()))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    width: u32,
    height: u32,
    radius: i32,
    _pad: i32,
    inv_2s: f32,
    inv_2r: f32,
    _pad2: f32,
    _pad3: f32,
}

const BILATERAL_WGSL: &str = r#"
struct Params {
    width: u32,
    height: u32,
    radius: i32,
    _pad: i32,
    inv_2s: f32,
    inv_2r: f32,
    _pad2: f32,
    _pad3: f32,
}

@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<storage, read_write> dst: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn unpack(p: u32) -> vec3<f32> {
    return vec3<f32>(
        f32((p >> 16u) & 255u),
        f32((p >> 8u) & 255u),
        f32(p & 255u),
    );
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if x >= params.width || y >= params.height {
        return;
    }
    let i = y * params.width + x;
    let c = unpack(src[i]);
    var acc = vec3<f32>(0.0);
    var wsum = 0.0;
    let r = params.radius;
    var dy = -r;
    loop {
        if dy > r { break; }
        let ny = i32(y) + dy;
        if ny >= 0 && ny < i32(params.height) {
            var dx = -r;
            loop {
                if dx > r { break; }
                let nx = i32(x) + dx;
                if nx >= 0 && nx < i32(params.width) {
                    let n = unpack(src[u32(ny) * params.width + u32(nx)]);
                    let ds2 = f32(dx * dx + dy * dy);
                    let d = n - c;
                    let dc2 = dot(d, d);
                    let wt = exp(-ds2 * params.inv_2s - dc2 * params.inv_2r);
                    acc += n * wt;
                    wsum += wt;
                }
                dx = dx + 1;
            }
        }
        dy = dy + 1;
    }
    let o = acc / max(wsum, 1e-6);
    let ru = u32(clamp(o.r, 0.0, 255.0));
    let gu = u32(clamp(o.g, 0.0, 255.0));
    let bu = u32(clamp(o.b, 0.0, 255.0));
    dst[i] = (ru << 16u) | (gu << 8u) | bu;
}
"#;
