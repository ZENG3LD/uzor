//! Round-trip: encode 3 overlapping rects → tile_assign + tile_sort → readback
//! → assert each tile's list contains the right cmd indices in painter order.
//!
//! Run with: `cargo test --test tile_dispatch -- --ignored --nocapture`

use uzor_urx_wgpu_full::{
    SceneCmd, TileBuffers, TilePipeline, TILE_CMD_CAP, TILE_SIZE,
};

// 64×64 viewport: 4×4 = 16 tiles total, each 16×16 px.
const W: u32 = 64;
const H: u32 = 64;

fn init_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("urx-fullgpu-test-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()
}

/// Read a storage buffer as `Vec<u32>` using map_async.
fn readback_u32(device: &wgpu::Device, queue: &wgpu::Queue, src: &wgpu::Buffer, count: usize) -> Vec<u32> {
    let byte_size = (count * 4) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("urx-test-staging"),
        size: byte_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_buffer_to_buffer(src, 0, &staging, 0, byte_size);
    queue.submit(Some(enc.finish()));

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let out: Vec<u32> = data
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    drop(data);
    staging.unmap();
    out
}

#[test]
#[ignore = "needs gpu adapter; run with --ignored"]
fn tile_assign_buckets_three_rects_into_centre_tile() {
    let (device, queue) = match init_device() {
        Some(d) => d,
        None => {
            eprintln!("no wgpu adapter — skipping");
            return;
        }
    };

    // Viewport 64×64 → 4×4 tiles.
    // Centre tile (tx=1, ty=1) → tile_id = 1*4 + 1 = 5.
    // Its pixel range: x [16..32), y [16..32).
    //
    // 3 rects that all overlap the centre tile:
    //   cmd 0: (8, 8) → (24, 24)   — spans tiles (0,0)→(1,1), includes (1,1)
    //   cmd 1: (16, 16) → (32, 32) — spans tiles (1,1)→(1,1), only (1,1)
    //   cmd 2: (0, 0) → (63, 63)   — spans all 16 tiles
    let cmds = vec![
        SceneCmd::rect( 8.0,  8.0, 24.0, 24.0, [255,   0, 0, 255]),
        SceneCmd::rect(16.0, 16.0, 32.0, 32.0, [  0, 255, 0, 255]),
        SceneCmd::rect( 0.0,  0.0, 63.0, 63.0, [  0,   0, 255, 255]),
    ];

    let tile_count_x = (W + TILE_SIZE - 1) / TILE_SIZE; // 4
    let tile_count_y = (H + TILE_SIZE - 1) / TILE_SIZE; // 4
    let total_tiles  = (tile_count_x * tile_count_y) as usize; // 16

    let bufs     = TileBuffers::allocate(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);

    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    pipeline.dispatch(&device, &queue, &mut enc, &bufs, &cmds);
    queue.submit(Some(enc.finish()));

    // --- readback tile_counts ---
    let counts = readback_u32(&device, &queue, &bufs.tile_counts_buf, total_tiles);
    eprintln!("tile_counts: {:?}", counts);

    // Centre tile id = ty*tile_count_x + tx = 1*4 + 1 = 5
    let centre_id = 5_usize;
    assert_eq!(
        counts[centre_id], 3,
        "centre tile must have 3 cmds, got {} (all 3 rects overlap it)",
        counts[centre_id],
    );

    // Corner tile (0,0) id = 0 must have cmds 0 + 2.
    assert_eq!(counts[0], 2, "corner tile (0,0) must have 2 cmds (rects 0 and 2), got {}", counts[0]);

    // --- readback tile_lists for centre tile ---
    let cap = TILE_CMD_CAP as usize;
    let all_lists = readback_u32(&device, &queue, &bufs.tile_lists_buf, total_tiles * cap);
    let base = centre_id * cap;
    let centre_list = &all_lists[base .. base + counts[centre_id] as usize];
    eprintln!("centre tile list: {:?}", centre_list);

    // After tile_sort the 3 cmd indices must be in ascending (painter) order.
    let mut expected = vec![0u32, 1u32, 2u32];
    expected.sort_unstable();
    let mut got: Vec<u32> = centre_list.to_vec();
    got.sort_unstable();
    assert_eq!(
        got, expected,
        "centre tile must contain cmd indices 0, 1, 2 — got {:?}",
        centre_list,
    );

    // Verify ascending order (insertion sort must not scramble painter order).
    for w in centre_list.windows(2) {
        assert!(
            w[0] <= w[1],
            "centre tile list must be sorted ascending (painter order), got {:?}",
            centre_list,
        );
    }
}
