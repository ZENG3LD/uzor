//! Extension trait that adds `.run()` onto `uzor::framework::AppBuilder<A, P>`.
//!
//! Import [`AppRun`] to gain `.run()` on any `AppBuilder`:
//!
//! ```rust,ignore
//! use uzor::framework::AppBuilder;
//! use uzor_desktop::AppRun as _;
//!
//! // Backend autodetected — no .backend() or .surface_factory() needed.
//! AppBuilder::new(MyApp::new())
//!     .title("my app")
//!     .size(1280, 720)
//!     .run()
//!     .expect("runtime error");
//! ```

use uzor::docking::panels::DockPanel;
use uzor::framework::app::{App, AppConfig};
use uzor::framework::builder::AppBuilder;
use uzor_render_hub::{RenderBackend, RenderSurfaceFactory};

use crate::manager::{Manager, ManagerError};
use crate::scene3d_app::{scene3d_frame_submission, Scene3DApp};

// ── AppRun ────────────────────────────────────────────────────────────────────

/// Extension trait that adds `.run()` onto [`AppBuilder`].
///
/// This is the primary entry point for running an uzor desktop app.
pub trait AppRun {
    /// Consume the builder and run the application.
    ///
    /// Blocks until all windows close (on native desktop targets).
    fn run(self) -> Result<(), ManagerError>;
}

impl<A, P> AppRun for AppBuilder<A, P>
where
    A: App<P>,
    P: DockPanel + Default + Clone + 'static,
{
    fn run(self) -> Result<(), ManagerError> {
        let built = self.build().map_err(ManagerError::Build)?;
        // Single-instance guard lives for the duration of the event loop.
        let _single_instance_guard = built
            .config
            .single_instance
            .as_deref()
            .map(crate::utils::single_instance::single_instance);
        Manager::from_built(built).run()
    }
}

// ── AppRun3D ──────────────────────────────────────────────────────────────────

/// Additive sibling of [`AppRun`] for apps implementing
/// [`crate::scene3d_app::Scene3DApp`] (W3D arc plan §1.7, Wave 2).
///
/// A SEPARATE trait, not a new method on [`AppRun`] itself — [`AppRun`]
/// is implemented for every `A: App<P>`, and `.scene3d()` only exists on
/// `A: Scene3DApp<P>`; putting `run_with_3d` there would either need a
/// default body with nothing to call, or narrow `AppRun`'s own bound and
/// break every ordinary 2D app in the workspace (l1-l4 demos,
/// mylittlechart's own consumers). This mirrors the plan's own "additive
/// second entry point" framing exactly: `.run()` is completely
/// untouched, `.run_with_3d()` is a new, narrower-bounded door next to
/// it.
pub trait AppRun3D {
    /// Consume the builder and run the application with the 3D dispatch
    /// hook armed — see `crate::manager`'s divergence log for exactly
    /// what changes in the per-frame loop once this hook is set.
    fn run_with_3d(self) -> Result<(), ManagerError>;
}

impl<A, P> AppRun3D for AppBuilder<A, P>
where
    A: Scene3DApp<P>,
    P: DockPanel + Default + Clone + 'static,
{
    fn run_with_3d(self) -> Result<(), ManagerError> {
        let built = self.build().map_err(ManagerError::Build)?;
        let _single_instance_guard = built
            .config
            .single_instance
            .as_deref()
            .map(crate::utils::single_instance::single_instance);
        let mut mgr = Manager::from_built(built);
        // The monomorphized dispatch helper is a plain function item
        // with no captured environment, returning either a retained Arc
        // scene or the original owned frame. `Manager::scene3d_hook`
        // stores it so the per-frame dispatch code
        // (bound only by `A: App<P>`) can call through it without itself
        // needing the narrower `Scene3DApp<P>` bound.
        mgr.scene3d_hook = Some(scene3d_frame_submission::<A, P>);
        mgr.run()
    }
}

// ── run_closure ───────────────────────────────────────────────────────────────

/// Quick prototype helper — build and run an app from a closure.
///
/// The closure receives `&mut LayoutManager<P>` and `&mut dyn RenderContext`.
///
/// # Example
///
/// ```rust,ignore
/// uzor_desktop::run_closure::<uzor::framework::app::NoPanel, _>(
///     |layout, render| { /* draw something */ },
///     AppConfig::default(),
///     RenderBackend::VelloGpu,
///     Box::new(uzor_render_hub::VelloGpuSurfaceFactory::new()),
/// ).expect("runtime error");
/// ```
pub fn run_closure<P, F>(
    ui: F,
    config: AppConfig,
    backend: RenderBackend,
    factory: Box<dyn RenderSurfaceFactory>,
) -> Result<(), ManagerError>
where
    P: DockPanel + Default + Clone + Send + Sync + 'static,
    F: FnMut(&mut uzor::layout::LayoutManager<P>, &mut dyn uzor::render::RenderContext)
        + 'static,
{
    use uzor::framework::app::ClosureApp;
    let mut mgr = Manager::new(
        ClosureApp::<P, F>::new(ui),
        config,
        backend,
        None,
    );
    mgr.set_surface_factory(factory);
    // Queue a default window from config defaults.
    use uzor::framework::multi_window::{WindowKey, WindowSpec};
    mgr.queue_window_spec(
        WindowSpec::new(WindowKey::new("main"), "uzor")
            .size(1280, 720),
    );
    mgr.run()
}
