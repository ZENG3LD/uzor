use std::sync::OnceLock;

use crate::cpu;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuMode {
    Auto,
    Gpu,
    Cpu,
}

pub struct GpuCtx {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub name: String,
    bilateral: wgpu::ComputePipeline,
    bilateral_bgl: wgpu::BindGroupLayout,
    slic: wgpu::ComputePipeline,
    slic_bgl: wgpu::BindGroupLayout,
}

static GPU: OnceLock<Option<GpuCtx>> = OnceLock::new();

pub fn ctx() -> Result<&'static GpuCtx, String> {
    GPU.get_or_init(init_gpu)
        .as_ref()
        .ok_or_else(|| "no gpu adapter".into())
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
        label: Some("uzor-vision"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    }))
    .ok()?;
    let (bilateral, bilateral_bgl) = make_compute(
        &device,
        "bilateral",
        BILATERAL_WGSL,
        &[
            wgpu::BufferBindingType::Storage { read_only: true },
            wgpu::BufferBindingType::Storage { read_only: false },
            wgpu::BufferBindingType::Uniform,
        ],
    );
    let (slic, slic_bgl) = make_compute(
        &device,
        "slic",
        SLIC_WGSL,
        &[
            wgpu::BufferBindingType::Storage { read_only: true },
            wgpu::BufferBindingType::Storage { read_only: false },
            wgpu::BufferBindingType::Storage { read_only: true },
            wgpu::BufferBindingType::Uniform,
        ],
    );
    Some(GpuCtx {
        device,
        queue,
        name: info.name,
        bilateral,
        bilateral_bgl,
        slic,
        slic_bgl,
    })
}

fn make_compute(
    device: &wgpu::Device,
    label: &'static str,
    wgsl: &'static str,
    bindings: &[wgpu::BufferBindingType],
) -> (wgpu::ComputePipeline, wgpu::BindGroupLayout) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(wgsl.into()),
    });
    let mut entries = Vec::new();
    for (i, ty) in bindings.iter().enumerate() {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: i as u32,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: *ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    (pipeline, bgl)
}

pub fn denoise(rgb: &[u8], w: usize, h: usize, mode: GpuMode) -> (Vec<u8>, String) {
    let rgb = cpu::median3(rgb, w, h);
    match mode {
        GpuMode::Cpu => (cpu::bilateral(&rgb, w, h, 3, 2.0, 16.0), "cpu".into()),
        GpuMode::Gpu | GpuMode::Auto => match gpu_bilateral(&rgb, w, h) {
            Ok((out, name)) => (out, format!("gpu:{name}")),
            Err(e) => {
                if mode == GpuMode::Gpu {
                    eprintln!("gpu denoise failed ({e}); cpu fallback");
                }
                (
                    cpu::bilateral(&rgb, w, h, 3, 2.0, 16.0),
                    format!("cpu(fallback:{e})"),
                )
            }
        },
    }
}

fn gpu_bilateral(rgb: &[u8], w: usize, h: usize) -> Result<(Vec<u8>, String), String> {
    let g = ctx()?;
    let n = w * h;
    let packed = cpu::pack_rgb(rgb, n);
    let src = buf_storage_dst(&g.device, "b-src", bytemuck::cast_slice(&packed));
    let dst = buf_rw(&g.device, "b-dst", (n * 4) as u64);
    let params = BiParams {
        width: w as u32,
        height: h as u32,
        radius: 3,
        _pad: 0,
        inv_2s: 0.5 / 4.0,
        inv_2r: 0.5 / 256.0,
        _pad2: 0.0,
        _pad3: 0.0,
    };
    let ub = buf_uniform(&g.device, "b-p", bytemuck::bytes_of(&params));
    dispatch_readback(
        g,
        &g.bilateral,
        &g.bilateral_bgl,
        &[&src, &dst, &ub],
        &dst,
        n * 4,
        w,
        h,
    )
    .map(|bytes| {
        let packed_out: &[u32] = bytemuck::cast_slice(&bytes);
        (cpu::unpack_rgb(packed_out), g.name.clone())
    })
}

pub fn gpu_slic_assign(
    rgb: &[u8],
    w: usize,
    h: usize,
    centers: &[Center],
    step: f32,
    compact: f32,
) -> Result<Vec<u32>, String> {
    let g = ctx()?;
    let n = w * h;
    let packed = cpu::pack_rgb(rgb, n);
    let src = buf_storage_dst(&g.device, "s-src", bytemuck::cast_slice(&packed));
    let labels = buf_rw(&g.device, "s-lab", (n * 4) as u64);
    let ctr = buf_storage_dst(&g.device, "s-ctr", bytemuck::cast_slice(centers));
    let params = SlicParams {
        width: w as u32,
        height: h as u32,
        k: centers.len() as u32,
        step,
        compact,
        _pad: 0.0,
        _pad2: 0.0,
        _pad3: 0.0,
    };
    let ub = buf_uniform(&g.device, "s-p", bytemuck::bytes_of(&params));
    let bytes = dispatch_readback(
        g,
        &g.slic,
        &g.slic_bgl,
        &[&src, &labels, &ctr, &ub],
        &labels,
        n * 4,
        w,
        h,
    )?;
    Ok(bytemuck::cast_slice(&bytes).to_vec())
}

fn buf_storage_dst(device: &wgpu::Device, label: &str, data: &[u8]) -> wgpu::Buffer {
    let b = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: data.len() as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    b.slice(..).get_mapped_range_mut().copy_from_slice(data);
    b.unmap();
    b
}

fn buf_rw(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn buf_uniform(device: &wgpu::Device, label: &str, data: &[u8]) -> wgpu::Buffer {
    let b = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: data.len() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    b.slice(..).get_mapped_range_mut().copy_from_slice(data);
    b.unmap();
    b
}

fn dispatch_readback(
    g: &GpuCtx,
    pipe: &wgpu::ComputePipeline,
    bgl: &wgpu::BindGroupLayout,
    bufs: &[&wgpu::Buffer],
    copy_src: &wgpu::Buffer,
    nbytes: usize,
    w: usize,
    h: usize,
) -> Result<Vec<u8>, String> {
    let mut entries = Vec::new();
    for (i, b) in bufs.iter().enumerate() {
        entries.push(wgpu::BindGroupEntry {
            binding: i as u32,
            resource: b.as_entire_binding(),
        });
    }
    let bg = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vision-bg"),
        layout: bgl,
        entries: &entries,
    });
    let staging = g.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vision-read"),
        size: nbytes as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = g.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("vision-enc"),
    });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("vision-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups(((w as u32) + 15) / 16, ((h as u32) + 15) / 16, 1);
    }
    enc.copy_buffer_to_buffer(copy_src, 0, &staging, 0, nbytes as u64);
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
    let out = raw.to_vec();
    drop(raw);
    staging.unmap();
    Ok(out)
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BiParams {
    width: u32,
    height: u32,
    radius: i32,
    _pad: i32,
    inv_2s: f32,
    inv_2r: f32,
    _pad2: f32,
    _pad3: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Center {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub _p0: f32,
    pub _p1: f32,
    pub _p2: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SlicParams {
    width: u32,
    height: u32,
    k: u32,
    step: f32,
    compact: f32,
    _pad: f32,
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
    return vec3<f32>(f32((p >> 16u) & 255u), f32((p >> 8u) & 255u), f32(p & 255u));
}
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if x >= params.width || y >= params.height { return; }
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
                    let wt = exp(-ds2 * params.inv_2s - dot(d, d) * params.inv_2r);
                    acc += n * wt;
                    wsum += wt;
                }
                dx = dx + 1;
            }
        }
        dy = dy + 1;
    }
    let o = acc / max(wsum, 1e-6);
    dst[i] = (u32(clamp(o.r, 0.0, 255.0)) << 16u)
        | (u32(clamp(o.g, 0.0, 255.0)) << 8u)
        | u32(clamp(o.b, 0.0, 255.0));
}
"#;

const SLIC_WGSL: &str = r#"
struct Center { x: f32, y: f32, r: f32, g: f32, b: f32, _p0: f32, _p1: f32, _p2: f32 }
struct Params { width: u32, height: u32, k: u32, step: f32, compact: f32, _pad: f32, _pad2: f32, _pad3: f32 }
@group(0) @binding(0) var<storage, read> src: array<u32>;
@group(0) @binding(1) var<storage, read_write> labels: array<u32>;
@group(0) @binding(2) var<storage, read> centers: array<Center>;
@group(0) @binding(3) var<uniform> params: Params;
fn unpack(p: u32) -> vec3<f32> {
    return vec3<f32>(f32((p >> 16u) & 255u), f32((p >> 8u) & 255u), f32(p & 255u));
}
@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if x >= params.width || y >= params.height { return; }
    let i = y * params.width + x;
    let pix = unpack(src[i]);
    let gx = i32(floor(f32(x) / params.step));
    let gy = i32(floor(f32(y) / params.step));
    var best = 0u;
    var best_d = 1e20;
    let gw = i32(ceil(f32(params.width) / params.step));
    for (var oy = -1; oy <= 1; oy++) {
        for (var ox = -1; ox <= 1; ox++) {
            let cx = gx + ox;
            let cy = gy + oy;
            if cx < 0 || cy < 0 || cx >= gw { continue; }
            let ci = u32(cy * gw + cx);
            if ci >= params.k { continue; }
            let c = centers[ci];
            let dc = pix - vec3<f32>(c.r, c.g, c.b);
            let dx = f32(x) - c.x;
            let dy = f32(y) - c.y;
            let d = dot(dc, dc) + params.compact * (dx * dx + dy * dy);
            if d < best_d {
                best_d = d;
                best = ci;
            }
        }
    }
    labels[i] = best;
}
"#;
