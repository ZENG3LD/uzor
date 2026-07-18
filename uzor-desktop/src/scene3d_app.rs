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

/// Additive sibling of [`App`] for windows that want a 3D viewport.
///
/// `scene3d` is called once per frame by [`crate::manager::Manager`]
/// when the app was started via `.run_with_3d()`
/// (`crate::builder_run::AppRun3D`). Returning `Some(frame)` composes
/// that 3D content into the swapchain THIS frame — see
/// `crate::manager`'s divergence log for exactly what that replaces on
/// a 3D-active frame. Returning `None` leaves the window on its
/// ordinary 2D `App::ui` path, completely unchanged — an app can freely
/// toggle dimensions frame to frame (`force_graph_demo`'s
/// `set_dimension` agent action does exactly this).
pub trait Scene3DApp<P: DockPanel = NoPanel>: App<P> {
    fn scene3d(&mut self, surf_w: u32, surf_h: u32) -> Option<Scene3DFrame>;
}
