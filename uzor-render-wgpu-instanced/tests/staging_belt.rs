//! Exercises the staging-belt upload path of `InstancedRenderer`.
//!
//! Marked `#[ignore]` — requires a real wgpu adapter (GPU or software).
//! Run with:
//!   cargo test -p uzor-render-wgpu-instanced --test staging_belt -- --ignored --nocapture

use uzor_render_wgpu_instanced::{DrawCmd, InstancedRenderer, QuadInstance};

const W: u32 = 64;
const H: u32 = 64;

/// Initialise a headless wgpu device, or return `None` if no adapter is available.
fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label:                 Some("staging-belt-test-device"),
            required_features:     wgpu::Features::empty(),
            required_limits:       wgpu::Limits::default(),
            memory_hints:          wgpu::MemoryHints::default(),
            trace:                 wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    ))
    .ok()?;
    Some((device, queue))
}

/// Verify the staging-belt path executes render() without panic.
///
/// This test does not validate pixel content — it confirms:
/// 1. `enable_staging_belt` wires up successfully post-construction.
/// 2. The belt-routed uniform + instance upload path does not panic.
/// 3. `belt.finish()` before submit and `belt.recall()` after submit complete.
#[test]
#[ignore = "needs wgpu adapter; run with --ignored"]
fn staging_belt_render_no_panic() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => {
            eprintln!("no wgpu adapter — skipping staging_belt test");
            return;
        }
    };

    let format = wgpu::TextureFormat::Rgba8Unorm;

    // Construct renderer normally (belt disabled by default).
    let mut renderer = InstancedRenderer::new(&device, &queue, format);

    // Enable the staging belt with the default 256 KiB chunk size.
    renderer.enable_staging_belt(&device, 256 * 1024);

    // Create an offscreen render target.
    let target_tex = device.create_texture(&wgpu::TextureDescriptor {
        label:            Some("staging-belt-test-target"),
        size:             wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count:  1,
        sample_count:     1,
        dimension:        wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target_tex.create_view(&wgpu::TextureViewDescriptor::default());

    // Build a small command list: two quads.
    let q1 = QuadInstance::from_float_color(
        [0.0, 0.0],
        [32.0, 32.0],
        [1.0, 0.0, 0.0, 1.0],
        4.0,
        0.0,
        [0.0; 4],
        [0.0, 0.0, W as f32, H as f32],
    );
    let q2 = QuadInstance::from_float_color(
        [32.0, 32.0],
        [32.0, 32.0],
        [0.0, 0.0, 1.0, 1.0],
        0.0,
        2.0,
        [1.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, W as f32, H as f32],
    );
    let commands = vec![DrawCmd::Quad(q1), DrawCmd::Quad(q2)];

    // This must not panic — belt path: uniform write + instance writes + render pass.
    renderer.render(
        &device,
        &queue,
        &target_view,
        W,
        H,
        &commands,
        Some(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
        None,
    );

    // A second frame exercises belt recall + re-use.
    renderer.render(
        &device,
        &queue,
        &target_view,
        W,
        H,
        &commands,
        None,
        None,
    );
}

/// Zero-size chunk arg maps to the 256 KiB default without panic.
#[test]
#[ignore = "needs wgpu adapter; run with --ignored"]
fn staging_belt_zero_chunk_defaults() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => {
            eprintln!("no wgpu adapter — skipping staging_belt_zero_chunk_defaults");
            return;
        }
    };

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = InstancedRenderer::new(&device, &queue, format);
    // Passing 0 → should default to 256 KiB chunk, no panic.
    renderer.enable_staging_belt(&device, 0);

    let target_tex = device.create_texture(&wgpu::TextureDescriptor {
        label:            Some("staging-belt-default-chunk-target"),
        size:             wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
        mip_level_count:  1,
        sample_count:     1,
        dimension:        wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let target_view = target_tex.create_view(&wgpu::TextureViewDescriptor::default());

    // Empty command list — exercises the empty-data early-return in write_into_buffer.
    renderer.render(&device, &queue, &target_view, W, H, &[], Some(wgpu::Color::BLACK), None);
}
