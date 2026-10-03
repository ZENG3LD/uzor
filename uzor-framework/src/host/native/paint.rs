//! Host paint path through `uzor-render-hub` (design §5.5).

use color::{AlphaColor, Srgb};
use uzor::render::RetainedSurface;
use uzor_render_hub::{submit_frame, RenderHub, SubmitParams, WindowRenderState};

use crate::runtime::Runtime;
use crate::types::error::FrameworkError;
use crate::types::frame::FrameRequest;
use crate::{App, Spec};

/// Paint one [`FrameRequest`] into the window's render state and submit.
pub fn paint_frame<S, A>(
    runtime: &mut Runtime<S, A>,
    hub: &mut RenderHub,
    render_state: &mut WindowRenderState,
    req: &FrameRequest,
) -> Result<(), FrameworkError>
where
    S: Spec,
    A: App<Spec = S>,
{
    render_state.begin_frame();

    for (scope, bits) in &req.invalidations {
        render_state.retained_cache_mut().invalidate(*scope, *bits);
    }

    // Inline backends: all regions in one context (design §5.5). The
    // VelloGpu multi-scene branch is equivalent when regions.len() <= 1;
    // for multiple regions we still paint each into the same context so
    // every compiled backend works without a second Scene allocation.
    let painted = render_state.with_render_context(|ctx| {
        if req.regions.is_empty() {
            runtime.paint(req, ctx);
        } else {
            for r in &req.regions {
                let mut region_req = req.clone();
                region_req.regions = smallvec::smallvec![*r];
                runtime.paint(&region_req, ctx);
            }
        }
    });
    if painted.is_none() {
        log::warn!(
            "paint: no render context for active backend {:?}",
            hub.active()
        );
    }

    let msaa = hub.settings().msaa_samples;
    let outcome = submit_frame(
        render_state,
        SubmitParams {
            base_color: color_from_u32(req.background),
            msaa_samples: msaa,
        },
    );
    hub.update_metrics(outcome.metrics);
    if outcome.surface_lost {
        return Err(FrameworkError::Host(format!(
            "surface lost for window {:?}",
            req.window
        )));
    }
    Ok(())
}

/// `FrameRequest.background` is `0xRRGGBBAA`.
fn color_from_u32(rgba: u32) -> AlphaColor<Srgb> {
    let r = ((rgba >> 24) & 0xff) as f32 / 255.0;
    let g = ((rgba >> 16) & 0xff) as f32 / 255.0;
    let b = ((rgba >> 8) & 0xff) as f32 / 255.0;
    let a = (rgba & 0xff) as f32 / 255.0;
    AlphaColor::new([r, g, b, if a == 0.0 { 1.0 } else { a }])
}
