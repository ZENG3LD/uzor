//! `WgpuFullBackend` — owns the full-GPU pipeline + per-frame buffers.
//!
//! Stage 1a (2026-06-05) wrapper introduced so `submit_urx_wgpu_full`
//! in `uzor-render-hub` can lazy-init once and reuse the heavy
//! resources (pipeline, storage buffers, output texture) across
//! frames. Before this wrapper the encoder existed but every caller
//! had to wire device/queue/format/pipeline/buffers by hand.
//!
//! ## Frame lifecycle
//!
//! ```ignore
//! let mut backend = WgpuFullBackend::new(device, queue, surface_format);
//! backend.resize(width, height);
//! backend.submit(&scene, &mut encoder, &target_view);
//! ```
//!
//! `submit` does:
//! 1. encode Scene → `Vec<SceneCmd>`
//! 2. realloc TileBuffers if cmd-count exceeds capacity
//! 3. run `TilePipeline::dispatch_full` (assign + sort + fine)
//! 4. run `BlitPipeline::blit` into the caller's `target_view`

use crate::encoder::encode_scene_with_paths;
use crate::tile::{BlitPipeline, TileBuffers, TilePipeline, TILE_SIZE};
use uzor_urx_core::scene::Scene;

/// Owned wrapper around the URX full-GPU pipeline.
///
/// All wgpu resources (device, queue, pipelines, buffers, output
/// texture) are held internally so `submit_urx_wgpu_full` in
/// uzor-render-hub can call `submit()` with just the per-frame
/// encoder + target view.
pub struct WgpuFullBackend {
    device:    wgpu::Device,
    queue:     wgpu::Queue,
    /// Swapchain colour format (passed to the BlitPipeline at construction
    /// — re-create the backend if the swapchain format ever changes).
    format:    wgpu::TextureFormat,

    tile_pipeline: TilePipeline,
    blit_pipeline: BlitPipeline,

    /// Current TileBuffers + output texture. `None` until first `submit`
    /// (or `resize`).
    state: Option<BackendState>,
    /// Most-recent (width, height) requested via `resize`. Drives the
    /// realloc path in `submit`.
    pending_size: Option<(u32, u32)>,
    /// Maximum `cmds_n` (commands per frame) the current buffers can
    /// hold. Grows monotonically — we re-allocate when a frame overflows
    /// this cap, but never shrink. Default starts at 1024.
    cmds_capacity: u32,

    glyph_dummy_view: wgpu::TextureView,
    image_dummy_view: wgpu::TextureView,
}

struct BackendState {
    bufs:        TileBuffers,
    output_view: wgpu::TextureView,
    width:       u32,
    height:      u32,
}

const INITIAL_CMDS_CAP: u32 = 1024;

impl WgpuFullBackend {
    /// Construct with device / queue clones + the swapchain format.
    /// Compiles pipelines eagerly (one-time cost, ~50-200 ms on cold
    /// driver cache; warm after first run).
    pub fn new(
        device: wgpu::Device,
        queue:  wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Self {
        let tile_pipeline = TilePipeline::new(&device);
        let blit_pipeline = BlitPipeline::new(&device, format);
        let (_g_tex, glyph_dummy_view) = TilePipeline::dummy_glyph_atlas(&device);
        let (_i_tex, image_dummy_view) = TilePipeline::dummy_image_atlas(&device);
        Self {
            device,
            queue,
            format,
            tile_pipeline,
            blit_pipeline,
            state: None,
            pending_size: None,
            cmds_capacity: INITIAL_CMDS_CAP,
            glyph_dummy_view,
            image_dummy_view,
        }
    }

    /// Stash a (width, height) for the next `submit` to (re)allocate
    /// buffers + output texture at. Cheap if dims unchanged.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.pending_size = Some((width, height));
    }

    /// Current swapchain colour format (read-only).
    pub fn format(&self) -> wgpu::TextureFormat { self.format }

    /// Borrow the device (so callers can build companion resources).
    pub fn device(&self) -> &wgpu::Device { &self.device }

    /// Borrow the queue.
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }

    /// Encode the scene + dispatch + blit to `target_view`. Allocates /
    /// reallocates internal buffers as needed.
    ///
    /// `target_view` must be a render-attachment-compatible view whose
    /// texture format matches the `format` passed to [`Self::new`].
    pub fn submit(
        &mut self,
        scene:        &Scene,
        encoder:      &mut wgpu::CommandEncoder,
        target_view:  &wgpu::TextureView,
    ) -> Result<(), WgpuFullSubmitError> {
        // 1. Pull pending resize (if any) into state, allocating fresh
        //    TileBuffers + output texture.
        if let Some((w, h)) = self.pending_size.take() {
            self.realloc_state(w, h);
        }
        if self.state.is_none() {
            return Err(WgpuFullSubmitError::NotSized);
        }

        // 2. Encode Scene → flat SceneCmd vec + path_points.
        let (cmds, path_points) = encode_scene_with_paths(scene, 0);

        // 3. If cmd count exceeds capacity, grow + realloc buffers.
        if cmds.len() as u32 > self.cmds_capacity {
            let new_cap = (cmds.len() as u32).next_power_of_two().max(self.cmds_capacity * 2);
            self.cmds_capacity = new_cap;
            let (w, h) = {
                let s = self.state.as_ref().unwrap();
                (s.width, s.height)
            };
            self.realloc_state(w, h);
        }

        // 4. Dispatch the three compute stages — path_points already
        //    shaped as `Vec<[f32; 2]>` by the encoder.
        let state = self.state.as_ref().unwrap();
        self.tile_pipeline.dispatch_full(
            &self.device,
            &self.queue,
            encoder,
            &state.bufs,
            &cmds,
            &path_points,
            &state.output_view,
            &self.glyph_dummy_view,
            &self.image_dummy_view,
        );

        // 6. Blit the output texture into the caller's target view.
        let src_w = state.bufs.tile_count_x * TILE_SIZE;
        let src_h = state.bufs.tile_count_y * TILE_SIZE;
        self.blit_pipeline.blit(
            &self.device,
            encoder,
            &state.output_view,
            target_view,
            src_w,
            src_h,
            &self.queue,
        );

        Ok(())
    }

    fn realloc_state(&mut self, width: u32, height: u32) {
        let (bufs, _texture, output_view) = TileBuffers::with_output_texture(
            &self.device,
            self.cmds_capacity,
            width,
            height,
        );
        self.state = Some(BackendState {
            bufs,
            output_view,
            width,
            height,
        });
    }
}

/// Errors `WgpuFullBackend::submit` can return.
#[derive(Debug, thiserror::Error)]
pub enum WgpuFullSubmitError {
    /// `submit` called before `resize` (or with zero-area surface).
    #[error("WgpuFullBackend: submit called before resize / surface has zero area")]
    NotSized,
}
