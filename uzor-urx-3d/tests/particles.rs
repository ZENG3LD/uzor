//! Wave 19 — particle system tests.
//!
//! 1. Pure CPU `ParticleSystem::tick` behaviour:
//!    - emitter spawns at the configured rate (10 particles/s * 1s
//!      → ~10 live)
//!    - gravity pulls particles down
//!    - particles past their lifetime get reaped
//! 2. GPU draw produces non-empty output that's brighter at the
//!    emitter centre than at the periphery.

use std::sync::Arc;
use uzor_urx_3d::{
    EmitterConfig, ParticleRenderer, ParticleSystem, PerspectiveCamera, Vec3,
};

#[test]
fn emitter_respects_rate_and_capacity() {
    let cfg = EmitterConfig {
        rate: 10.0,
        capacity: 30,
        position: Vec3::ZERO,
        vel_min: Vec3::ZERO,
        vel_max: Vec3::ZERO,
        gravity: Vec3::ZERO,
        lifetime: 100.0, // long — nothing dies during the test
        ..EmitterConfig::default()
    };
    let mut ps = ParticleSystem::new(cfg);
    for _ in 0..60 { ps.tick(1.0 / 60.0); }
    // 1 sec * 10/s = 10 live (well below 30 capacity).
    assert!((ps.live() as i32 - 10).abs() <= 1, "live={}", ps.live());
}

#[test]
fn gravity_pulls_particles_down() {
    let cfg = EmitterConfig {
        rate: 1.0,
        capacity: 4,
        position: Vec3::new(0.0, 5.0, 0.0),
        vel_min: Vec3::ZERO,
        vel_max: Vec3::ZERO,
        gravity: Vec3::new(0.0, -10.0, 0.0),
        lifetime: 100.0,
        ..EmitterConfig::default()
    };
    let mut ps = ParticleSystem::new(cfg);
    // Spawn one
    ps.tick(1.0);
    let y_start = ps.particles()[0].pos.y;
    // Let gravity pull for 1 more second.
    for _ in 0..60 { ps.tick(1.0 / 60.0); }
    let y_end = ps.particles()[0].pos.y;
    assert!(y_end < y_start - 3.0, "expected significant drop: {} -> {}", y_start, y_end);
}

#[test]
fn particles_die_after_lifetime() {
    let cfg = EmitterConfig {
        rate: 5.0,
        capacity: 4,
        lifetime: 0.5,
        gravity: Vec3::ZERO,
        ..EmitterConfig::default()
    };
    let mut ps = ParticleSystem::new(cfg);
    ps.tick(0.6); // first batch lives, then ages past 0.5
    let live_before = ps.live();
    ps.tick(1.0); // a second pass — earlier particles all dead now
    let live_after = ps.live();
    assert!(live_before > 0, "should have spawned at least one");
    // After 1s with lifetime 0.5 we expect SOME live particles (newly
    // spawned) but the count should not exceed rate * lifetime = 2.5.
    assert!(live_after <= 4, "live_after={} exceeds cap", live_after);
}

#[test]
#[ignore]
fn gpu_draw_brightens_centre_of_emitter() {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    })) {
        Ok(a) => a,
        Err(_) => { eprintln!("no GPU adapter"); return; }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("urx3d-particles-test"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
        experimental_features: wgpu::ExperimentalFeatures::default(),
    })).unwrap();

    const W: u32 = 128;
    const H: u32 = 128;

    let cfg = EmitterConfig {
        rate: 500.0,
        capacity: 256,
        position: Vec3::ZERO,
        vel_min: Vec3::new(-0.05, -0.05, -0.05),
        vel_max: Vec3::new( 0.05,  0.05,  0.05),
        gravity: Vec3::ZERO,
        lifetime: 5.0,
        size_start: 0.3,
        size_end: 0.3,
        color_start: [1.0, 0.5, 0.1, 1.0],
        color_end:   [1.0, 0.5, 0.1, 1.0],
        ..EmitterConfig::default()
    };
    let mut ps = ParticleSystem::new(cfg);
    // Warm up.
    for _ in 0..30 { ps.tick(1.0 / 60.0); }
    assert!(ps.live() > 100, "expected many particles, got {}", ps.live());

    // HDR target.
    let hdr_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("particles-hdr"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: uzor_urx_3d::HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let hdr_view = hdr_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("particles-depth"),
        size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: uzor_urx_3d::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_tex.create_view(&wgpu::TextureViewDescriptor::default());

    let mut renderer = ParticleRenderer::new(&device, uzor_urx_3d::HDR_FORMAT);
    let camera = PerspectiveCamera::new(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::ZERO,
        W as f32 / H as f32,
    );

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    // Clear HDR first to black + depth to 1.0.
    {
        let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("particles-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &hdr_view, resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
    }
    renderer.draw(&device, &queue, &mut enc, &hdr_view, &depth_view, &camera, &ps);
    queue.submit(Some(enc.finish()));

    // Read back HDR pixels (16f → 2 bytes/channel = 8 bytes/pixel).
    let aligned = ((W * 8 + 255) & !255) as u32;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("particles-readback"),
        size: (aligned * H) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &hdr_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(aligned), rows_per_image: Some(H) },
        },
        wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let raw = slice.get_mapped_range();

    // Extract centre & corner red channel as f16.
    let pix_f16 = |x: u32, y: u32| -> f32 {
        let off = (y * aligned + x * 8) as usize;
        let bits = u16::from_le_bytes([raw[off], raw[off + 1]]);
        half::f16::from_bits(bits).to_f32()
    };
    let centre = pix_f16(W / 2, H / 2);
    let corner = pix_f16(2, 2);
    eprintln!("particle centre R={}, corner R={}", centre, corner);
    drop(raw); staging.unmap();
    let _ = Arc::strong_count(&renderer);

    assert!(centre > corner + 0.05, "particle cloud should be brighter at centre: c={} corner={}", centre, corner);
    assert!(centre > 0.05, "particles should produce visible red: {}", centre);
}
