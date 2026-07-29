//! `Scene3DApp` — additive second app hook for windows that want a
//! composed 3D viewport (W3D arc plan §1.7/§2, Wave 2).
//!
//! `uzor` core has zero dependency on `uzor-urx-3d`/wgpu (confirmed by
//! the plan's own §0.1 grep) — the base `App` trait cannot literally
//! return a `Scene3D` without breaking the core→backend dependency
//! direction (core is shared by wasm/mobile backends with no wgpu-3D
//! guarantee). This trait lives in `uzor-desktop` instead, which already
//! depends on `uzor-render-hub`/`uzor-urx-3d`.
//!
//! Additive, not a core-trait change: an app that never implements
//! [`Scene3DApp`] and keeps calling `.run()`
//! (`crate::builder_run::AppRun`) is completely unaffected — see
//! `crate::builder_run::AppRun3D` and `crate::manager`'s divergence log
//! for exactly how a `Scene3DApp` gets wired into the frame loop.

use std::sync::Arc;

use uzor::framework::app::{App, NoPanel};
use uzor::layout::docking::DockPanel;
pub use uzor_render_hub::CachedOverlayJob;

/// One composed 3D frame — the scene to render plus the camera to view
/// it through. Built fresh by the app every call to
/// [`Scene3DApp::scene3d`]; `Manager` pushes `scene` into the window's
/// `WindowRenderState` 3D slot and composes it with `camera` via
/// `uzor_render_hub::compose::submit_urx_composed`.
pub struct Scene3DFrame {
    pub scene: uzor_urx_3d::Scene3D,
    pub camera: uzor_urx_3d::PerspectiveCamera,
    /// Cache-keyed static chrome. Its paint closure runs only when the key
    /// changes or the surface is resized, before the dynamic overlay.
    pub cached_overlay: Option<CachedOverlayJob>,
    /// Optional 2D overlay painted ON TOP of the composed 3D frame this
    /// same tick (Wave 4 / W3D arc plan §1.3 label-overlay gap, closed
    /// here — see `crate::manager`'s divergence log for exactly how this
    /// forwards into `submit_urx_composed`'s new post-3D Phase 4.5).
    /// Receives `&mut dyn RenderContext` in the SAME 1:1 physical-pixel
    /// space as the `surf_w`/`surf_h` this `scene3d()` call was invoked
    /// with — a `uzor-graph::GraphEngine3D::draw_overlay` caller needs no
    /// coordinate translation between `project_world_to_screen`'s output
    /// and where it draws. `None` (the common case — 2D-chrome-free 3D
    /// content with nothing to overlay) skips Phase 4.5 entirely; a
    /// window that never sets this pays zero extra cost.
    pub overlay: Option<Box<dyn FnMut(&mut dyn uzor::render::RenderContext)>>,
}

/// One composed 3D frame whose immutable scene storage is retained by
/// the caller. Cloning the [`Arc`] for a new frame is O(1); the desktop
/// compositor borrows the same [`uzor_urx_3d::Scene3D`] allocation
/// directly and never clones its nodes.
pub struct RetainedScene3DFrame {
    pub scene: Arc<uzor_urx_3d::Scene3D>,
    pub camera: uzor_urx_3d::PerspectiveCamera,
    pub cached_overlay: Option<CachedOverlayJob>,
    pub overlay: Option<Box<dyn FnMut(&mut dyn uzor::render::RenderContext)>>,
}

pub(crate) enum Scene3DFrameSubmission {
    Owned(Scene3DFrame),
    Retained(RetainedScene3DFrame),
}

impl Scene3DFrameSubmission {
    pub(crate) fn scene(&self) -> &uzor_urx_3d::Scene3D {
        match self {
            Self::Owned(frame) => &frame.scene,
            Self::Retained(frame) => frame.scene.as_ref(),
        }
    }

    pub(crate) fn camera(&self) -> uzor_urx_3d::PerspectiveCamera {
        match self {
            Self::Owned(frame) => frame.camera,
            Self::Retained(frame) => frame.camera,
        }
    }

    pub(crate) fn take_cached_overlay(&mut self) -> Option<CachedOverlayJob> {
        match self {
            Self::Owned(frame) => frame.cached_overlay.take(),
            Self::Retained(frame) => frame.cached_overlay.take(),
        }
    }

    pub(crate) fn take_overlay(
        &mut self,
    ) -> Option<Box<dyn FnMut(&mut dyn uzor::render::RenderContext)>> {
        match self {
            Self::Owned(frame) => frame.overlay.take(),
            Self::Retained(frame) => frame.overlay.take(),
        }
    }
}

pub(crate) fn scene3d_frame_submission<A, P>(
    app: &mut A,
    surf_w: u32,
    surf_h: u32,
) -> Option<Scene3DFrameSubmission>
where
    A: Scene3DApp<P>,
    P: DockPanel,
{
    app.retained_scene3d(surf_w, surf_h)
        .map(Scene3DFrameSubmission::Retained)
        .or_else(|| {
            app.scene3d(surf_w, surf_h)
                .map(Scene3DFrameSubmission::Owned)
        })
}

/// Additive sibling of [`App`] for windows that want a 3D viewport.
///
/// The retained hook, then the owned `scene3d` fallback, is queried once
/// per frame by [`crate::manager::Manager`] when the app was started via
/// `.run_with_3d()` (`crate::builder_run::AppRun3D`). Returning
/// `Some(frame)` composes that 3D content into the swapchain THIS frame —
/// see
/// `crate::manager`'s divergence log for exactly what that replaces on
/// a 3D-active frame. Returning `None` leaves the window on its
/// ordinary 2D `App::ui` path, completely unchanged — an app can freely
/// toggle dimensions frame to frame (`force_graph_demo`'s
/// `set_dimension` agent action does exactly this).
pub trait Scene3DApp<P: DockPanel = NoPanel>: App<P> {
    /// Builds an owned scene frame. Existing implementations can keep
    /// returning the same [`Scene3DFrame`] struct literal unchanged.
    fn scene3d(&mut self, _surf_w: u32, _surf_h: u32) -> Option<Scene3DFrame> {
        None
    }

    /// Returns a frame backed by a caller-retained immutable scene.
    ///
    /// This hook is tried before [`Self::scene3d`]. Implement it when
    /// scene topology is unchanged across frames and only camera or
    /// overlay state changes.
    fn retained_scene3d(
        &mut self,
        _surf_w: u32,
        _surf_h: u32,
    ) -> Option<RetainedScene3DFrame> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::framework::multi_window::WindowCtx;
    use uzor_urx_3d::{PerspectiveCamera, Scene3D, Vec3};

    struct RetainedApp {
        scene: Arc<Scene3D>,
    }

    impl App<NoPanel> for RetainedApp {
        fn ui(&mut self, _win: &mut WindowCtx<'_, NoPanel>) {}
    }

    impl Scene3DApp for RetainedApp {
        fn retained_scene3d(
            &mut self,
            _surf_w: u32,
            _surf_h: u32,
        ) -> Option<RetainedScene3DFrame> {
            Some(RetainedScene3DFrame {
                scene: Arc::clone(&self.scene),
                camera: camera(),
                cached_overlay: None,
                overlay: None,
            })
        }
    }

    struct OwnedApp;

    impl App<NoPanel> for OwnedApp {
        fn ui(&mut self, _win: &mut WindowCtx<'_, NoPanel>) {}
    }

    impl Scene3DApp for OwnedApp {
        fn scene3d(&mut self, _surf_w: u32, _surf_h: u32) -> Option<Scene3DFrame> {
            let mut scene = Scene3D::new();
            scene.clear_color = [0.1, 0.2, 0.3, 1.0];
            Some(Scene3DFrame {
                scene,
                camera: camera(),
                cached_overlay: None,
                overlay: None,
            })
        }
    }

    fn camera() -> PerspectiveCamera {
        PerspectiveCamera::new(Vec3::new(0.0, 0.0, 2.0), Vec3::ZERO, 1.0)
    }

    #[test]
    fn retained_frames_borrow_the_same_arc_scene_across_frames() {
        let retained = Arc::new(Scene3D::new());
        let mut app = RetainedApp { scene: Arc::clone(&retained) };
        let first = scene3d_frame_submission(&mut app, 1280, 720)
            .expect("retained hook must produce the first frame");
        let second = scene3d_frame_submission(&mut app, 1280, 720)
            .expect("retained hook must produce the second frame");

        assert!(std::ptr::eq(first.scene(), retained.as_ref()));
        assert!(std::ptr::eq(second.scene(), retained.as_ref()));
        assert_eq!(Arc::strong_count(&retained), 4);
    }

    #[test]
    fn owned_scene3d_frame_struct_literal_remains_compatible() {
        let mut app = OwnedApp;
        let submission = scene3d_frame_submission(&mut app, 1280, 720)
            .expect("owned hook must remain the fallback");

        assert_eq!(submission.scene().clear_color, [0.1, 0.2, 0.3, 1.0]);
    }
}
