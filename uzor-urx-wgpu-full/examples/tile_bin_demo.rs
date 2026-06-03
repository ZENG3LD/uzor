//! Full-GPU pipeline demo stand — first stage (tile_assign + tile_sort).
//!
//! Builds a 1920×1080 scene with 100 deterministic rects, runs them
//! through the URX 1.6 compute pipeline, reads back tile occupancy,
//! prints a histogram. No window, no readback to PNG (no fine pass yet
//! — that's the next URX 1.6 stage).
//!
//! Purpose: prove the compute pipeline holds up at realistic scale +
//! report tile occupancy distribution as a sanity baseline for future
//! coarse / fine stage commits.
//!
//! Run:
//!   cargo run -p uzor-urx-wgpu-full --example tile_bin_demo --release
//!
//! Stdout summary:
//!   adapter / cmds / tiles / dispatch time / occupancy histogram

use uzor_urx_wgpu_full::{
    cmd::SceneCmd, tile::{TileBuffers, TilePipeline, TILE_CMD_CAP, TILE_SIZE},
};

const W: u32 = 1920;
const H: u32 = 1080;
const N_RECTS: u32 = 100;

fn main() {
    let exit = run();
    std::process::exit(exit);
}

fn run() -> i32 {
    println!("[full-gpu-demo] init wgpu device");
    let Some((device, queue, info)) = init_device() else {
        eprintln!("[full-gpu-demo] no GPU adapter — exit 1");
        return 1;
    };
    println!("[full-gpu-demo] adapter: {} ({:?})", info.name, info.backend);

    // Build deterministic command list — same scene every run for
    // bench reproducibility. Rects spread across viewport with sizes
    // varying so tile occupancy distribution is non-uniform.
    let cmds = build_scene();
    println!("[full-gpu-demo] scene: {} cmds", cmds.len());

    let bufs = TileBuffers::allocate(&device, cmds.len() as u32, W, H);
    let pipeline = TilePipeline::new(&device);
    println!("[full-gpu-demo] tiles: {} x {} = {}",
        bufs.tile_count_x, bufs.tile_count_y,
        bufs.tile_count_x * bufs.tile_count_y);

    // Warmup pass.
    run_dispatch(&device, &queue, &pipeline, &bufs, &cmds);

    // Timed: 60 dispatches.
    let t = std::time::Instant::now();
    for _ in 0..60 {
        run_dispatch(&device, &queue, &pipeline, &bufs, &cmds);
    }
    let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("[full-gpu-demo] 60 dispatches in {:.2} ms ({:.3} ms/frame)",
        elapsed_ms, elapsed_ms / 60.0);

    // Readback tile_counts buffer.
    let counts = readback_counts(&device, &queue, &bufs);
    let occupancy_hist = histogram(&counts);

    println!("[full-gpu-demo] tile occupancy histogram:");
    println!("  empty (0 cmds):     {}", occupancy_hist[0]);
    println!("  1-2 cmds:           {}", occupancy_hist[1]);
    println!("  3-8 cmds:           {}", occupancy_hist[2]);
    println!("  9-32 cmds:          {}", occupancy_hist[3]);
    println!("  33-{} cmds:         {}", TILE_CMD_CAP, occupancy_hist[4]);
    println!("  overflowed (>{}):  {}", TILE_CMD_CAP, occupancy_hist[5]);

    let total_assignments: u64 = counts.iter().map(|&c| c.min(TILE_CMD_CAP) as u64).sum();
    let total_overflows:   u64 = counts.iter().filter(|&&c| c > TILE_CMD_CAP).count() as u64;
    println!("[full-gpu-demo] total cmd-to-tile assignments: {}", total_assignments);
    println!("[full-gpu-demo] overflowed tiles:               {}", total_overflows);

    if total_assignments == 0 {
        eprintln!("[full-gpu-demo] no assignments produced — pipeline broken?");
        return 2;
    }

    println!("[full-gpu-demo] OK");
    0
}

fn build_scene() -> Vec<SceneCmd> {
    // Splitmix64 deterministic RNG.
    let mut s: u64 = 0xa3b1_c2d4_e5f6_0708;
    let mut next = || -> u32 {
        s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    };
    let f01 = |x: u32| (x & 0xffff) as f32 / 65535.0;

    let mut out = Vec::with_capacity(N_RECTS as usize);
    for _ in 0..N_RECTS {
        let x0 = f01(next()) * (W as f32 - 200.0);
        let y0 = f01(next()) * (H as f32 - 200.0);
        let w  = 20.0 + f01(next()) * 200.0;
        let h  = 20.0 + f01(next()) * 200.0;
        let r  = (next() & 0xff) as u8;
        let g  = (next() & 0xff) as u8;
        let b  = (next() & 0xff) as u8;
        out.push(SceneCmd::rect(x0, y0, x0 + w, y0 + h, [r, g, b, 255]));
    }
    out
}

fn run_dispatch(
    device:   &wgpu::Device,
    queue:    &wgpu::Queue,
    pipeline: &TilePipeline,
    bufs:     &TileBuffers,
    cmds:     &[SceneCmd],
) {
    let (_dummy_tex, dummy_atlas_view) = TilePipeline::dummy_glyph_atlas(device);
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    pipeline.dispatch(device, queue, &mut enc, bufs, cmds, &dummy_atlas_view);
    queue.submit(Some(enc.finish()));
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
}

fn readback_counts(
    device: &wgpu::Device,
    queue:  &wgpu::Queue,
    bufs:   &TileBuffers,
) -> Vec<u32> {
    let n = (bufs.tile_count_x * bufs.tile_count_y) as u64;
    let bytes = n * 4;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("counts-readback"),
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    enc.copy_buffer_to_buffer(&bufs.tile_counts_buf, 0, &readback, 0, bytes);
    queue.submit(Some(enc.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
    rx.recv().unwrap().unwrap();
    let data = slice.get_mapped_range();
    let counts: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();
    counts
}

fn histogram(counts: &[u32]) -> [usize; 6] {
    let mut h = [0usize; 6];
    for &c in counts {
        let idx = match c {
            0 => 0,
            1..=2 => 1,
            3..=8 => 2,
            9..=32 => 3,
            n if n <= TILE_CMD_CAP => 4,
            _ => 5,
        };
        h[idx] += 1;
    }
    h
}

fn init_device() -> Option<(wgpu::Device, wgpu::Queue, wgpu::AdapterInfo)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface:     None,
    })).ok()?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("full-gpu-demo-device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
            experimental_features: wgpu::ExperimentalFeatures::default(),
        },
    )).ok()?;
    let _ = TILE_SIZE; // silence unused if TILE_SIZE used only in shaders
    Some((device, queue, info))
}
